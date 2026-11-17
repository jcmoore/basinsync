//! Bounded downloads with one ordered, validated ingestion cursor.
use crate::{
    analogue::{self, Order},
    envbuf,
    timeline::{self, Datashed},
};
use anyhow::{Context, Result, ensure};
use num_bigint::BigUint;
use s3::Bucket;
use serde::Serialize;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap},
    ops::Range,
    sync::Arc,
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Sequential,
    Parallel,
    Bulk,
    Range,
    Hedged,
}
impl Mode {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "sequential" => Self::Sequential,
            "parallel" => Self::Parallel,
            "bulk" => Self::Bulk,
            "range" => Self::Range,
            "hedged" => Self::Hedged,
            _ => anyhow::bail!("unknown recovery mode"),
        })
    }
}
#[derive(Clone, Debug)]
pub struct Config {
    pub mode: Mode,
    pub concurrency: usize,
    pub max_bytes: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Bulk,
            concurrency: 4,
            max_bytes: 128 * 1024 * 1024,
        }
    }
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("BASINSYNC_RECOVERY_MODE") {
            config.mode = Mode::parse(&value)?;
        }
        if let Ok(value) = std::env::var("BASINSYNC_RECOVERY_CONCURRENCY") {
            config.concurrency = value.parse()?;
        }
        if let Ok(value) = std::env::var("BASINSYNC_RECOVERY_MAX_BYTES") {
            config.max_bytes = value.parse()?;
        }
        Ok(config)
    }
}
#[derive(Default, Debug, Serialize)]
pub struct Stats {
    pub timeline_gets: usize,
    pub bulk_gets: usize,
    pub bulk_events: usize,
    pub applied: usize,
    pub rejected_orders: usize,
    pub peak_buffer_bytes: usize,
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub key: String,
    pub order: Order,
    pub size: u64,
}

/// Each newly scanned band has descending lower bounds. A later interval can
/// contain earlier ones; retain their identities for future redundancy audits.
#[derive(Default)]
pub struct Coverage {
    upper: BinaryHeap<Reverse<(BigUint, String)>>,
    last_low: Option<BigUint>,
    group_high: Option<BigUint>,
    pub covered: Vec<String>,
}
impl Coverage {
    pub fn observe(&mut self, candidate: &Candidate) -> Result<()> {
        ensure!(
            self.last_low
                .as_ref()
                .is_none_or(|low| low >= &candidate.order.lowest),
            "unordered Analogue listing"
        );
        if self.last_low.as_ref() == Some(&candidate.order.lowest)
            && self
                .group_high
                .as_ref()
                .is_some_and(|high| high >= &candidate.order.highest)
        {
            self.covered.push(candidate.key.clone());
            return Ok(());
        }
        while self
            .upper
            .peek()
            .is_some_and(|Reverse((high, _))| high <= &candidate.order.highest)
        {
            self.covered.push(self.upper.pop().unwrap().0.1);
        }
        self.upper.push(Reverse((
            candidate.order.highest.clone(),
            candidate.key.clone(),
        )));
        self.last_low = Some(candidate.order.lowest.clone());
        self.group_high = Some(candidate.order.highest.clone());
        Ok(())
    }
}

async fn discover(
    bucket: Bucket,
    mut cursor: watch::Receiver<BigUint>,
    floor: BigUint,
    output: mpsc::Sender<Candidate>,
) -> Result<()> {
    let mut previous: Option<String> = None;
    loop {
        let current = cursor.borrow_and_update().clone();
        let boundary = analogue::seek(&current);
        if previous.as_ref().is_none_or(|old| boundary < *old) {
            let mut after = Some(boundary.clone());
            let mut coverage = Coverage::default();
            'pages: loop {
                let (page, status) = bucket
                    .list_page(
                        analogue::PREFIX.into(),
                        None,
                        None,
                        after.clone(),
                        Some(128),
                    )
                    .await?;
                ensure!(status == 200, "Analogue LIST returned HTTP {status}");
                for object in &page.contents {
                    ensure!(
                        after.as_ref().is_none_or(|old| object.key > *old),
                        "Analogue LIST did not advance"
                    );
                    if previous.as_ref().is_some_and(|old| object.key >= *old) {
                        break 'pages;
                    }
                    after = Some(object.key.clone());
                    let order = match Order::from_key(&object.key) {
                        Ok(order) => order,
                        Err(error) => {
                            tracing::warn!(%error, key=%object.key, "ignoring invalid BulkOrder key");
                            continue;
                        }
                    };
                    if object.size < 16 || order.highest <= floor {
                        continue;
                    }
                    ensure!(
                        order.lowest <= current,
                        "Analogue seek escaped its lower bound"
                    );
                    let candidate = Candidate {
                        key: object.key.clone(),
                        order,
                        size: object.size,
                    };
                    coverage.observe(&candidate)?;
                    if output.send(candidate).await.is_err() {
                        return Ok(());
                    }
                }
                if !page.is_truncated {
                    break;
                }
                ensure!(!page.contents.is_empty(), "truncated empty Analogue LIST");
            }
            tracing::debug!(
                covered = coverage.covered.len(),
                "Analogue discovery band complete"
            );
            previous = Some(boundary);
        }
        if cursor.changed().await.is_err() {
            return Ok(());
        }
    }
}

struct Entry {
    bytes: Arc<Vec<u8>>,
    range: Range<usize>,
    source: Option<String>,
}
struct Download {
    entries: Vec<(BigUint, Entry)>,
}

async fn get_event(bucket: &Bucket, milestone: BigUint, max_bytes: usize) -> Result<Download> {
    let response = crate::object_read::if_present(bucket)?
        .get_object(timeline::key(&milestone)?)
        .await?;
    ensure!(
        response.status_code() == 200,
        "Timeline GET at {milestone} returned HTTP {}",
        response.status_code()
    );
    ensure!(
        response.as_slice().len() >= 16,
        "Timeline tombstone at {milestone} requires a covering snapshot"
    );
    ensure!(
        response.as_slice().len() <= max_bytes,
        "Timeline event exceeds recovery buffer limit"
    );
    let bytes = Arc::new(response.to_vec());
    Ok(Download {
        entries: vec![(
            milestone,
            Entry {
                range: 0..bytes.len(),
                bytes,
                source: None,
            },
        )],
    })
}

async fn ranged_get(
    bucket: &Bucket,
    key: &str,
    range: Range<usize>,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    ensure!(
        range.start < range.end && range.len() <= max_bytes,
        "BulkOrder range exceeds buffer limit"
    );
    let mut reader = crate::object_read::if_present(bucket)?;
    reader.add_header("range", &format!("bytes={}-{}", range.start, range.end - 1));
    let response = reader.get_object(key).await?;
    ensure!(
        response.status_code() == 206 && response.as_slice().len() == range.len(),
        "BulkOrder Range GET failed"
    );
    Ok(response.to_vec())
}

async fn get_order(
    bucket: &Bucket,
    candidate: &Candidate,
    start: BigUint,
    tail: BigUint,
    range_reads: bool,
    max_bytes: usize,
) -> Result<Download> {
    let high = candidate.order.highest.clone().min(tail + 1u8);
    let (bytes, index, base) = if range_reads {
        let (head, status) = crate::object_read::if_present(bucket)?
            .head_object(&candidate.key)
            .await?;
        ensure!(status == 200, "BulkOrder HEAD failed");
        let text = head
            .metadata
            .as_ref()
            .and_then(|m| m.get("bulkorder-range"))
            .context("missing BulkOrder range metadata")?;
        let (low, end) = text
            .strip_prefix("bytes=")
            .context("invalid BulkOrder range")?
            .split_once('-')
            .context("invalid BulkOrder range")?;
        let range = low.parse::<usize>()?
            ..end
                .parse::<usize>()?
                .checked_add(1)
                .context("range overflow")?;
        let footer = ranged_get(bucket, &candidate.key, range.clone(), max_bytes).await?;
        let frame = envbuf::at(&footer, 0)?;
        ensure!(
            frame.end == footer.len() && frame.format == envbuf::FLEXBUFFERS,
            "invalid BulkOrder footer"
        );
        let index = analogue::Index::decode(frame.content)?;
        ensure!(
            index.order() == candidate.order && index.offsets.last() == Some(&range.start),
            "BulkOrder index/metadata mismatch"
        );
        let selected = index.range(&start, &high)?;
        let bytes = ranged_get(bucket, &candidate.key, selected.clone(), max_bytes).await?;
        (bytes, index, selected.start)
    } else {
        ensure!(
            candidate.size <= max_bytes as u64,
            "BulkOrder exceeds recovery buffer limit"
        );
        let response = crate::object_read::if_present(bucket)?
            .get_object(&candidate.key)
            .await?;
        ensure!(
            response.status_code() == 200 && response.as_slice().len() <= max_bytes,
            "BulkOrder GET failed"
        );
        let bytes = response.to_vec();
        let index = analogue::decode(&bytes, &candidate.order)?;
        (bytes, index, 0)
    };
    let first = usize::try_from(&start - &index.lowest)?;
    let count = usize::try_from(&high - &start)?;
    let bytes = Arc::new(bytes);
    let mut entries = Vec::with_capacity(count);
    for i in first..first + count {
        let range = index.offsets[i] - base..index.offsets[i + 1] - base;
        envbuf::nested_message(
            bytes
                .get(range.clone())
                .context("BulkOrder index outside response")?,
        )?;
        entries.push((
            &index.lowest + i,
            Entry {
                bytes: bytes.clone(),
                range,
                source: Some(candidate.key.clone()),
            },
        ));
    }
    Ok(Download { entries })
}

pub async fn replay(
    bucket: &Bucket,
    state: &mut Datashed,
    tail: BigUint,
    config: &Config,
) -> Result<Stats> {
    ensure!(
        (1..=32).contains(&config.concurrency) && config.max_bytes > 0,
        "invalid recovery limits"
    );
    let mut stats = Stats::default();
    let mut cursor = &state.milestone + 1u8;
    if cursor > tail {
        return Ok(stats);
    }
    let bulk = matches!(config.mode, Mode::Bulk | Mode::Range | Mode::Hedged);
    let (position, positions) = watch::channel(cursor.clone());
    let (output, mut discovered) = mpsc::channel(64);
    let listing =
        bulk.then(|| tokio::spawn(discover(bucket.clone(), positions, cursor.clone(), output)));
    let result = async {
        let mut candidates = HashMap::<String, Candidate>::new();
        let mut failed = BTreeSet::<String>::new();
        let mut waiting = BTreeMap::<BigUint, Entry>::new();
        let mut individuals = BTreeSet::<BigUint>::new();
        let mut active_bulk = false;
        let mut downloads = JoinSet::new();
        let mut discovery_open = bulk;
        loop {
            while let Ok(candidate) = discovered.try_recv() { candidates.insert(candidate.key.clone(), candidate); }
            while let Some(entry) = waiting.remove(&cursor) {
                match state.apply_entry(cursor.clone(), &entry.bytes[entry.range]) {
                    Ok(()) => { stats.applied += 1; if entry.source.is_some() { stats.bulk_events += 1; } cursor += 1u8; }
                    Err(error) => if let Some(source) = entry.source {
                        tracing::warn!(%error, key=%source, "invalid BulkOrder event; using Timeline");
                        failed.insert(source.clone()); stats.rejected_orders += 1;
                        waiting.retain(|_, entry| entry.source.as_ref() != Some(&source));
                        break;
                    } else { return Err(error); },
                }
            }
            if cursor > tail { break; }
            candidates.retain(|key, c| c.order.highest > cursor && !failed.contains(key));
            // Discovery can look ahead without advancing the applied cursor.
            if cursor > *position.borrow() {
                position.send_replace((&cursor + 31u8).min(&tail + 1u8));
            }
            // Metadata is bounded too; weaker orders remain useful fallbacks, not mandatory history.
            if candidates.len() > 128 {
                let mut ranked: Vec<_> = candidates.values().map(|c| (c.order.highest.clone(), c.key.clone())).collect(); ranked.sort();
                for (_, key) in ranked.into_iter().take(candidates.len()-128) { candidates.remove(&key); }
            }
            if bulk && !active_bulk && !waiting.contains_key(&cursor)
                && let Some(candidate) = candidates.values().filter(|c| c.order.covers(&cursor)).max_by_key(|c| &c.order.highest).cloned() {
                    let reader = bucket.clone(); let low = cursor.clone(); let high = tail.clone(); let key = candidate.key.clone(); let config = config.clone();
                    let frontier = candidate.order.highest.clone().min(&tail + 1u8);
                    if frontier > *position.borrow() { position.send_replace(frontier); }
                    downloads.spawn(async move { (Some(key), None, get_order(&reader, &candidate, low, high, config.mode == Mode::Range, config.max_bytes).await) });
                    active_bulk = true; stats.bulk_gets += 1;
            }
            let window = if config.mode == Mode::Parallel { config.concurrency } else { 1 };
            if !active_bulk || config.mode == Mode::Hedged {
                for offset in 0..window {
                    let milestone = &cursor + offset;
                    if milestone > tail { break; }
                    if individuals.len() >= window { break; }
                    if !waiting.contains_key(&milestone) && individuals.insert(milestone.clone()) {
                        let reader = bucket.clone(); let max_bytes = config.max_bytes;
                        downloads.spawn(async move { (None, Some(milestone.clone()), get_event(&reader, milestone, max_bytes).await) });
                        stats.timeline_gets += 1;
                    }
                }
            }
            tokio::select! {
                candidate = discovered.recv(), if discovery_open => match candidate {
                    Some(candidate) => { candidates.insert(candidate.key.clone(), candidate); },
                    None => discovery_open = false,
                },
                download = downloads.join_next(), if !downloads.is_empty() => {
                    let (source, milestone, result) = download.context("missing recovery download")?.context("recovery download panicked")?;
                    if let Some(milestone) = milestone { individuals.remove(&milestone); }
                    if source.is_some() { active_bulk = false; }
                    match result {
                        Ok(download) => for (milestone, entry) in download.entries {
                            if milestone >= cursor { waiting.entry(milestone).or_insert(entry); }
                        },
                        Err(error) => if let Some(key) = source {
                            tracing::warn!(%error, %key, "BulkOrder retrieval failed; using Timeline");
                            failed.insert(key); stats.rejected_orders += 1;
                        } else { return Err(error); },
                    }
                    let mut bodies = BTreeMap::new();
                    for entry in waiting.values() { bodies.insert(Arc::as_ptr(&entry.bytes) as usize, entry.bytes.len()); }
                    stats.peak_buffer_bytes = stats.peak_buffer_bytes.max(bodies.values().sum());
                },
            }
        }
        Ok(stats)
    }.await;
    if let Some(listing) = listing {
        listing.abort();
        match listing.await {
            Ok(Err(error)) => {
                tracing::warn!(%error, "Analogue discovery failed; Timeline remained authoritative")
            }
            Err(error) if !error.is_cancelled() => {
                tracing::warn!(%error, "Analogue discovery failed")
            }
            _ => {}
        }
    }
    result
}
