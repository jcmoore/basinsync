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
    BulkParallel,
    RangeParallel,
    RangePrefetch,
}
impl Mode {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "sequential" => Self::Sequential,
            "parallel" => Self::Parallel,
            "bulk" => Self::Bulk,
            "range" => Self::Range,
            "hedged" => Self::Hedged,
            "bulk-parallel" => Self::BulkParallel,
            "range-parallel" => Self::RangeParallel,
            "range-prefetch" => Self::RangePrefetch,
            _ => anyhow::bail!("unknown recovery mode"),
        })
    }
}
#[derive(Clone, Debug)]
pub struct Config {
    pub mode: Mode,
    pub concurrency: usize,
    pub max_bytes: usize,
    pub bulk_concurrency: usize,
    pub bulk_buffer_bytes: usize,
    pub manifest_concurrency: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::BulkParallel,
            concurrency: 4,
            max_bytes: 128 * 1024 * 1024,
            bulk_concurrency: 2,
            bulk_buffer_bytes: 32 * 1024 * 1024,
            manifest_concurrency: 8,
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
        if let Ok(value) = std::env::var("BASINSYNC_RECOVERY_BULK_CONCURRENCY") {
            config.bulk_concurrency = value.parse()?;
        }
        if let Ok(value) = std::env::var("BASINSYNC_RECOVERY_BULK_BUFFER_BYTES") {
            config.bulk_buffer_bytes = value.parse()?;
        }
        if let Ok(value) = std::env::var("BASINSYNC_RECOVERY_MANIFEST_CONCURRENCY") {
            config.manifest_concurrency = value.parse()?;
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
    pub peak_bulk_downloads: usize,
    pub peak_bulk_reserved_bytes: usize,
    /// Independent look-ahead probes; coupled Range jobs fetch their own footer.
    pub manifest_gets: usize,
    pub peak_manifest_reads: usize,
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

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

async fn get_manifest(bucket: &Bucket, candidate: &Candidate) -> Result<analogue::Index> {
    let start = analogue::manifest_start(&candidate.key)?;
    let length = usize::try_from(candidate.size)?
        .checked_sub(start)
        .context("BulkManifest outside object")?;
    ensure!(
        length > 0 && length <= MAX_MANIFEST_BYTES,
        "BulkManifest tail exceeds limit"
    );
    let mut reader = crate::object_read::if_present(bucket)?;
    reader.add_header("range", &format!("bytes={start}-"));
    let response = reader.get_object(&candidate.key).await?;
    ensure!(
        response.status_code() == 206 && response.as_slice().len() == length,
        "BulkManifest tail GET failed"
    );
    let footer = response.as_slice();
    let frame = envbuf::at(footer, 0)?;
    ensure!(
        frame.format == envbuf::FLEXBUFFERS
            && envbuf::all(&footer[frame.end..])?
                .iter()
                .all(|frame| frame.format == envbuf::MARGIN),
        "invalid BulkManifest envelope"
    );
    let index = analogue::Index::decode(frame.content)?;
    ensure!(
        index.order() == candidate.order && index.offsets.last() == Some(&start),
        "BulkManifest key/index mismatch"
    );
    Ok(index)
}

enum Fetched {
    Data(Download),
    Manifest(analogue::Index),
}

async fn get_order(
    bucket: &Bucket,
    candidate: &Candidate,
    start: BigUint,
    tail: BigUint,
    range_reads: bool,
    max_bytes: usize,
    cached: Option<analogue::Index>,
) -> Result<Download> {
    let high = candidate.order.highest.clone().min(tail + 1u8);
    let max_bytes = max_bytes.min(usize::try_from(candidate.size)?);
    let (bytes, index, base) = if range_reads {
        let index = if let Some(index) = cached {
            index
        } else {
            get_manifest(bucket, candidate).await?
        };
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
        ensure!(
            analogue::footer(&bytes)?.1.start == analogue::manifest_start(&candidate.key)?,
            "BulkManifest range/key mismatch"
        );
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

struct ActiveOrder {
    range: Range<BigUint>,
    bytes: usize,
}

/// Bytes already buffered and those reserved by unfinished archive reads share a
/// budget. Count each shared body once; the individual fallback has its own
/// per-response limit and does not occupy an archive slot.
fn bulk_usage(
    waiting: &BTreeMap<BigUint, Entry>,
    active: &HashMap<String, ActiveOrder>,
) -> (usize, usize) {
    let mut bodies = BTreeMap::new();
    for entry in waiting.values().filter(|entry| entry.source.is_some()) {
        bodies.insert(Arc::as_ptr(&entry.bytes) as usize, entry.bytes.len());
    }
    (
        bodies.len() + active.len(),
        bodies.values().sum::<usize>() + active.values().map(|order| order.bytes).sum::<usize>(),
    )
}

fn next_uncovered(
    cursor: &BigUint,
    waiting: &BTreeMap<BigUint, Entry>,
    active: &HashMap<String, ActiveOrder>,
) -> BigUint {
    let mut next = cursor.clone();
    loop {
        if waiting.contains_key(&next) {
            next += 1u8;
        } else if let Some(order) = active.values().find(|order| order.range.contains(&next)) {
            next = order.range.end.clone();
        } else {
            return next;
        }
    }
}

fn next_probe(
    cursor: &BigUint,
    waiting: &BTreeMap<BigUint, Entry>,
    active: &HashMap<String, ActiveOrder>,
    probes: &HashMap<String, ActiveOrder>,
    manifests: &HashMap<String, analogue::Index>,
) -> BigUint {
    let mut next = cursor.clone();
    loop {
        next = next_uncovered(&next, waiting, active);
        if let Some(order) = probes.values().find(|order| order.range.contains(&next)) {
            next = order.range.end.clone();
        } else if let Some(index) = manifests.values().find(|index| index.order().covers(&next)) {
            next = index.order().highest;
        } else {
            return next;
        }
    }
}

pub async fn replay(
    bucket: &Bucket,
    state: &mut Datashed,
    tail: BigUint,
    config: &Config,
) -> Result<Stats> {
    ensure!(
        (1..=32).contains(&config.concurrency)
            && (1..=32).contains(&config.bulk_concurrency)
            && config.max_bytes > 0
            && config.bulk_buffer_bytes > 0
            && (1..=32).contains(&config.manifest_concurrency),
        "invalid recovery limits"
    );
    let mut stats = Stats::default();
    let mut cursor = &state.milestone + 1u8;
    if cursor > tail {
        return Ok(stats);
    }
    let parallel_bulk = matches!(
        config.mode,
        Mode::BulkParallel | Mode::RangeParallel | Mode::RangePrefetch
    );
    let bulk = parallel_bulk || matches!(config.mode, Mode::Bulk | Mode::Range | Mode::Hedged);
    let prefetch = config.mode == Mode::RangePrefetch;
    let bulk_window = if parallel_bulk {
        config.bulk_concurrency
    } else {
        1
    };
    let bulk_budget = if parallel_bulk {
        config.bulk_buffer_bytes
    } else {
        usize::MAX
    };
    let (position, positions) = watch::channel(cursor.clone());
    let (output, mut discovered) = mpsc::channel(64);
    let listing =
        bulk.then(|| tokio::spawn(discover(bucket.clone(), positions, cursor.clone(), output)));
    let result = async {
        let mut candidates = HashMap::<String, Candidate>::new();
        let mut failed = BTreeSet::<String>::new();
        let mut waiting = BTreeMap::<BigUint, Entry>::new();
        let mut individuals = BTreeSet::<BigUint>::new();
        let mut active = HashMap::<String, ActiveOrder>::new();
        let mut probes = HashMap::<String, ActiveOrder>::new();
        let mut manifests = HashMap::<String, analogue::Index>::new();
        let mut downloads = JoinSet::new();
        let mut discovery_open = bulk;
        loop {
            while let Ok(candidate) = discovered.try_recv() {
                candidates.insert(candidate.key.clone(), candidate);
            }
            while let Some(entry) = waiting.remove(&cursor) {
                match state.apply_entry(cursor.clone(), &entry.bytes[entry.range]) {
                    Ok(()) => {
                        stats.applied += 1;
                        if entry.source.is_some() { stats.bulk_events += 1; }
                        cursor += 1u8;
                    }
                    Err(error) => if let Some(source) = entry.source {
                        tracing::warn!(%error, key=%source, "invalid BulkOrder event; using Timeline");
                        failed.insert(source.clone());
                        stats.rejected_orders += 1;
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
            if candidates.len() > 128 {
                let mut ranked: Vec<_> = candidates.values().map(|c| (c.order.highest.clone(), c.key.clone())).collect();
                ranked.sort();
                for (_, key) in ranked.into_iter().take(candidates.len()-128) { candidates.remove(&key); }
            }
            manifests.retain(|key, index| candidates.contains_key(key) && index.order().highest > cursor && !failed.contains(key));
            if prefetch {
                while probes.len() + manifests.len() < config.manifest_concurrency {
                    let low = next_probe(&cursor, &waiting, &active, &probes, &manifests);
                    if low > tail { break; }
                    let Some(candidate) = candidates.values().filter(|candidate| {
                        candidate.order.covers(&low) && !probes.contains_key(&candidate.key)
                            && !manifests.contains_key(&candidate.key) && !active.contains_key(&candidate.key)
                            && analogue::manifest_start(&candidate.key).is_ok_and(|start| candidate.size.checked_sub(start as u64).is_some_and(|length| length > 0 && length <= MAX_MANIFEST_BYTES as u64))
                    }).max_by_key(|candidate| &candidate.order.highest).cloned() else { break; };
                    let high = candidate.order.highest.clone().min(&tail + 1u8);
                    if high > *position.borrow() { position.send_replace(high.clone()); }
                    probes.insert(candidate.key.clone(), ActiveOrder { range: low..high, bytes: 0 });
                    stats.manifest_gets += 1;
                    stats.peak_manifest_reads = stats.peak_manifest_reads.max(probes.len());
                    let reader = bucket.clone();
                    downloads.spawn(async move { (Some(candidate.key.clone()), None, get_manifest(&reader, &candidate).await.map(Fetched::Manifest)) });
                }
            }
            if bulk {
                loop {
                    let low = next_uncovered(&cursor, &waiting, &active);
                    let (slots, bytes) = bulk_usage(&waiting, &active);
                    if low > tail || slots >= bulk_window { break; }
                    let reservation = |candidate: &Candidate| -> Option<usize> {
                        if prefetch {
                            manifests.get(&candidate.key)?.range(&low, &candidate.order.highest.clone().min(&tail + 1u8)).ok().map(|range| range.len())
                        } else { usize::try_from(candidate.size).ok() }
                    };
                    let Some(candidate) = candidates.values().filter(|candidate| {
                        candidate.order.covers(&low) && !active.contains_key(&candidate.key)
                            && (matches!(config.mode, Mode::Range | Mode::RangeParallel) || reservation(candidate).is_some_and(|bytes| bytes <= config.max_bytes))
                            && reservation(candidate).is_some_and(|reserved| reserved <= bulk_budget.saturating_sub(bytes))
                    }).max_by_key(|candidate| &candidate.order.highest).cloned() else { break; };
                    let reserved = reservation(&candidate).context("missing BulkOrder reservation")?;
                    let high = candidate.order.highest.clone().min(&tail + 1u8);
                    if high > *position.borrow() { position.send_replace(high.clone()); }
                    active.insert(candidate.key.clone(), ActiveOrder {
                        range: low.clone()..high,
                        bytes: reserved,
                    });
                    stats.peak_bulk_downloads = stats.peak_bulk_downloads.max(active.len());
                    stats.peak_bulk_reserved_bytes = stats.peak_bulk_reserved_bytes.max(bulk_usage(&waiting, &active).1);
                    stats.bulk_gets += 1;
                    let reader = bucket.clone();
                    let tail = tail.clone();
                    let range_reads = matches!(config.mode, Mode::Range | Mode::RangeParallel | Mode::RangePrefetch);
                    let cached = manifests.remove(&candidate.key);
                    let max_bytes = config.max_bytes;
                    downloads.spawn(async move {
                        (Some(candidate.key.clone()), None, get_order(&reader, &candidate, low, tail, range_reads, max_bytes, cached).await.map(Fetched::Data))
                    });
                }
            }
            let window = if config.mode == Mode::Parallel { config.concurrency } else { 1 };
            // A future buffered order must not prevent filling an earlier gap.
            if (!active.values().any(|order| order.range.contains(&cursor)) && !probes.values().any(|order| order.range.contains(&cursor))) || config.mode == Mode::Hedged {
                for offset in 0..window {
                    let milestone = &cursor + offset;
                    if milestone > tail || individuals.len() >= window { break; }
                    if !waiting.contains_key(&milestone) && individuals.insert(milestone.clone()) {
                        let reader = bucket.clone();
                        let max_bytes = config.max_bytes;
                        downloads.spawn(async move { (None, Some(milestone.clone()), get_event(&reader, milestone, max_bytes).await.map(Fetched::Data)) });
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
                    if let Some(source) = &source { active.remove(source); probes.remove(source); }
                    match result {
                        Ok(Fetched::Manifest(index)) => { manifests.insert(source.context("manifest without source")?, index); },
                        Ok(Fetched::Data(download)) => for (milestone, entry) in download.entries {
                            if milestone >= cursor { waiting.entry(milestone).or_insert(entry); }
                        },
                        Err(error) => if let Some(key) = source {
                            tracing::warn!(%error, %key, "BulkOrder retrieval failed; using Timeline");
                            failed.insert(key);
                            stats.rejected_orders += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefetch_frontier_skips_downloads_and_buffered_events_without_applying_them() {
        let cursor = 42u8.into();
        let body = Arc::new(vec![0; 64]);
        let waiting = BTreeMap::from([
            (
                80u8.into(),
                Entry {
                    bytes: body.clone(),
                    range: 0..32,
                    source: Some("second".into()),
                },
            ),
            (
                81u8.into(),
                Entry {
                    bytes: body,
                    range: 32..64,
                    source: Some("second".into()),
                },
            ),
        ]);
        let active = HashMap::from([(
            "first".into(),
            ActiveOrder {
                range: 42u8.into()..80u8.into(),
                bytes: 100,
            },
        )]);
        assert_eq!(next_uncovered(&cursor, &waiting, &active), 82u8.into());
        assert_eq!(cursor, 42u8.into());
        assert_eq!(bulk_usage(&waiting, &active), (2, 164));
        assert_eq!(next_uncovered(&41u8.into(), &waiting, &active), 41u8.into());
    }

    #[test]
    fn completed_future_bodies_keep_their_slot_and_share_the_byte_budget() {
        let body = Arc::new(vec![0; 64]);
        let mut waiting = BTreeMap::from([
            (
                80u8.into(),
                Entry {
                    bytes: body.clone(),
                    range: 0..32,
                    source: Some("second".into()),
                },
            ),
            (
                81u8.into(),
                Entry {
                    bytes: body,
                    range: 32..64,
                    source: Some("second".into()),
                },
            ),
            (
                42u8.into(),
                Entry {
                    bytes: Arc::new(vec![0; 512]),
                    range: 0..512,
                    source: None,
                },
            ),
        ]);
        let active = HashMap::new();
        assert_eq!(bulk_usage(&waiting, &active), (1, 64));
        waiting.remove(&80u8.into());
        assert_eq!(bulk_usage(&waiting, &active), (1, 64));
        waiting.remove(&81u8.into());
        assert_eq!(bulk_usage(&waiting, &active), (0, 0));
    }
}
