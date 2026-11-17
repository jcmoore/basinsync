//! Immutable, indexed feeds of complete Timeline events.
use crate::{
    envbuf, event,
    flexbuffer::{self, Extension},
    lexidecimal, timeline,
};
use anyhow::{Context, Result, ensure};
use num_bigint::{BigInt, BigUint};
use s3::Bucket;
use serde::Serialize;
use std::{ops::Range, time::Duration};
use tokio::sync::{mpsc, watch};

pub const PREFIX: &str = "analogue/";
pub const COMMAND: &str = "sts/0/databasin/bulkorder";
pub const RANGE_HEADER: &str = "x-amz-meta-bulkorder-range";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub lowest: BigUint,
    pub highest: BigUint,
}
impl Order {
    pub fn key(&self) -> Result<String> {
        ensure!(
            self.lowest > 0u8.into() && self.highest > self.lowest,
            "invalid BulkOrder interval"
        );
        let key = format!(
            "{PREFIX}{}/{}",
            lexidecimal::encode(&!BigInt::from(self.lowest.clone())),
            lexidecimal::encode(&!BigInt::from(self.highest.clone()))
        );
        ensure!(key.len() <= 1024, "BulkOrder exceeds S3 key limit");
        Ok(key)
    }
    pub fn from_key(key: &str) -> Result<Self> {
        let (low, high) = key
            .strip_prefix(PREFIX)
            .context("invalid Analogue prefix")?
            .split_once('/')
            .context("missing BulkOrder bound")?;
        let order = Self {
            lowest: (!lexidecimal::decode(low)?)
                .to_biguint()
                .context("invalid lower bound")?,
            highest: (!lexidecimal::decode(high)?)
                .to_biguint()
                .context("invalid upper bound")?,
        };
        ensure!(order.key()? == key, "noncanonical BulkOrder key");
        Ok(order)
    }
    pub fn covers(&self, milestone: &BigUint) -> bool {
        &self.lowest <= milestone && milestone < &self.highest
    }
}

pub fn seek(milestone: &BigUint) -> String {
    format!(
        "{PREFIX}{}/",
        lexidecimal::encode(&!BigInt::from(milestone.clone()))
    )
}

#[derive(Clone, Debug)]
pub struct Index {
    pub lowest: BigUint,
    pub offsets: Vec<usize>,
}
impl Index {
    pub fn order(&self) -> Order {
        Order {
            lowest: self.lowest.clone(),
            highest: &self.lowest + self.offsets.len() - 1usize,
        }
    }
    pub fn range(&self, lowest: &BigUint, highest: &BigUint) -> Result<Range<usize>> {
        let order = self.order();
        ensure!(
            order.covers(lowest) && highest > lowest && highest <= &order.highest,
            "range outside BulkOrder"
        );
        let start = usize::try_from(lowest - &self.lowest)?;
        let end = usize::try_from(highest - &self.lowest)?;
        Ok(self.offsets[start]..self.offsets[end])
    }
    pub fn decode(message: &[u8]) -> Result<Self> {
        let tuple = flexbuffers::Reader::get_root(message)?.get_vector()?;
        ensure!(
            tuple.len() == 4
                && flexbuffer::read(tuple.index(0)?)?
                    == Extension::WellKnownSymbol("species".into())
                && tuple.index(1)?.get_str()? == event::NAMESPACE
                && tuple.index(2)?.get_str()? == COMMAND,
            "invalid BulkOrder tuple"
        );
        let query = tuple.index(3)?.get_map()?;
        ensure!(query.len() == 2, "invalid BulkOrder query");
        let Extension::BigInt(lowest) = flexbuffer::read(query.index("lowest")?)? else {
            anyhow::bail!("lowest must be a bigint");
        };
        let lowest = lowest.to_biguint().context("negative lower bound")?;
        ensure!(lowest > 0u8.into(), "Milestones begin at one");
        let vector = query.index("offsets")?;
        ensure!(
            vector.flexbuffer_type() != flexbuffers::FlexBufferType::VectorUInt,
            "offsets must not use the reserved extension type"
        );
        let offsets = vector
            .get_vector()?
            .iter()
            .map(|v| Ok(usize::try_from(v.get_i64()?)?))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            offsets.len() >= 2
                && offsets[0] == 0
                && offsets
                    .windows(2)
                    .all(|p| p[0] < p[1] && p[1].is_multiple_of(8)),
            "invalid BulkOrder offsets"
        );
        Ok(Self { lowest, offsets })
    }
}

#[derive(Serialize)]
struct Query {
    lowest: Vec<u8>,
    offsets: Vec<i64>,
}

pub struct Builder {
    index: Index,
    bytes: Vec<u8>,
}
impl Builder {
    pub fn new(lowest: BigUint) -> Result<Self> {
        ensure!(lowest > 0u8.into(), "Milestones begin at one");
        Ok(Self {
            index: Index {
                lowest,
                offsets: vec![0],
            },
            bytes: vec![],
        })
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn append(&mut self, milestone: &BigUint, bytes: &[u8]) -> Result<()> {
        ensure!(
            milestone == &self.index.order().highest,
            "noncontiguous BulkOrder append"
        );
        envbuf::nested_message(bytes)?;
        event::entry_digest(bytes)?;
        self.bytes.extend_from_slice(bytes);
        self.index.offsets.push(self.bytes.len());
        Ok(())
    }
    pub fn finish(mut self) -> Result<(Order, Vec<u8>, Range<usize>)> {
        ensure!(!self.is_empty(), "empty BulkOrder");
        let order = self.index.order();
        let mut lowest = Extension::BigInt(self.index.lowest.into()).payload()?;
        let lowest_length = lowest.len();
        // The upstream builder turns 2/3/4-element UInt vectors into fixed types.
        // Reserve five elements, then shorten the extension's length word; its
        // unused tail remains unreachable, just like other unused buffer bytes.
        lowest.resize(lowest.len().max(5), 0);
        let query = Query {
            lowest,
            offsets: self
                .index
                .offsets
                .into_iter()
                .map(i64::try_from)
                .collect::<std::result::Result<_, _>>()?,
        };
        let species = Extension::WellKnownSymbol("species".into()).payload()?;
        let mut message = flexbuffers::to_vec((species, event::NAMESPACE, COMMAND, query))?;
        if lowest_length < 5 {
            let uint = |at: usize, width: usize| {
                let mut word = [0u8; 8];
                word[..width].copy_from_slice(&message[at..at + width]);
                usize::try_from(u64::from_le_bytes(word)).unwrap()
            };
            let root_width = *message.last().unwrap() as usize;
            let root_slot = message.len() - 2 - root_width;
            let root = root_slot - uint(root_slot, root_width);
            let tuple_width = 1usize << (message[message.len() - 2] & 3);
            let query_slot = root + 3 * tuple_width;
            let query = query_slot - uint(query_slot, tuple_width);
            let query_width = 1usize << (message[root + 4 * tuple_width + 3] & 3);
            let lowest = query - uint(query, query_width);
            let width = 1usize << (message[query + 2 * query_width] & 3);
            message[lowest - width..lowest]
                .copy_from_slice(&(lowest_length as u64).to_le_bytes()[..width]);
        }
        let start = self.bytes.len();
        let end = envbuf::append(&mut self.bytes, &message, envbuf::FLEXBUFFERS)?;
        envbuf::append_counter_margin(&mut self.bytes, 0)?;
        Ok((order, self.bytes, start..end))
    }
}

pub fn footer(bytes: &[u8]) -> Result<(Index, Range<usize>)> {
    let mut end = bytes.len();
    while end > 0 {
        let frame = envbuf::before(bytes, end)?;
        if frame.format != envbuf::MARGIN {
            ensure!(
                frame.format == envbuf::FLEXBUFFERS,
                "invalid BulkOrder footer format"
            );
            let index = Index::decode(frame.content)?;
            ensure!(
                index.offsets.last() == Some(&frame.start),
                "BulkOrder index does not meet its footer"
            );
            return Ok((index, frame.start..frame.end));
        }
        end = frame.start;
    }
    anyhow::bail!("missing BulkOrder footer")
}

pub fn decode(bytes: &[u8], order: &Order) -> Result<Index> {
    let (index, _) = footer(bytes)?;
    ensure!(index.order() == *order, "BulkOrder key/index mismatch");
    validate_events(&bytes[..*index.offsets.last().unwrap()], &index.offsets)?;
    Ok(index)
}

pub fn validate_events(bytes: &[u8], offsets: &[usize]) -> Result<()> {
    ensure!(
        offsets.last() == Some(&bytes.len()),
        "BulkOrder data length mismatch"
    );
    for pair in offsets.windows(2) {
        let frame = envbuf::at(bytes, pair[0])?;
        ensure!(
            frame.format == envbuf::NESTED && frame.end == pair[1],
            "BulkOrder event boundary mismatch"
        );
    }
    Ok(())
}

pub async fn publish(
    bucket: &Bucket,
    order: &Order,
    bytes: &[u8],
    range: Range<usize>,
) -> Result<()> {
    let key = order.key()?;
    let mut headers = crate::epilogue::conditional_headers();
    headers.insert(
        RANGE_HEADER,
        format!("bytes={}-{}", range.start, range.end - 1).parse()?,
    );
    let response = crate::sequence::put_feed(bucket, &key, bytes, headers).await?;
    if response.status_code() == 412 {
        let existing = crate::object_read::if_present(bucket)?
            .get_object(&key)
            .await?;
        ensure!(
            existing.status_code() == 200,
            "existing BulkOrder GET failed"
        );
        let validator = existing
            .headers()
            .into_iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("etag"))
            .map(|(_, value)| value)
            .context("existing BulkOrder has no ETag")?;
        ensure!(
            validator != crate::sequence::reference(bucket)?,
            "existing BulkOrder shares the empty validator"
        );
        verify_same_events(existing.as_slice(), bytes, order)?;
    } else {
        ensure!(
            response.status_code() == 200,
            "BulkOrder PUT returned HTTP {}",
            response.status_code()
        );
    }
    Ok(())
}

pub fn verify_same_events(actual: &[u8], expected: &[u8], order: &Order) -> Result<()> {
    let a = decode(actual, order)?;
    let b = decode(expected, order)?;
    for (a, b) in a.offsets.windows(2).zip(b.offsets.windows(2)) {
        ensure!(
            event::entry_digest(&actual[a[0]..a[1]])?
                == event::entry_digest(&expected[b[0]..b[1]])?,
            "conflicting BulkOrder event identity"
        );
    }
    Ok(())
}

#[derive(Clone)]
pub struct Config {
    pub target_bytes: usize,
    pub max_age: Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            target_bytes: 5 * 1024 * 1024,
            max_age: Duration::from_secs(30),
        }
    }
}

pub struct DurableEvent {
    pub milestone: BigUint,
    pub bytes: Vec<u8>,
}

async fn resume(bucket: &Bucket, target: &BigUint) -> Result<BigUint> {
    let mut next = BigUint::from(1u8);
    while &next <= target {
        let mut after = Some(seek(&next));
        let mut candidates = vec![];
        loop {
            let (page, status) = bucket
                .list_page(PREFIX.into(), None, None, after.clone(), Some(128))
                .await?;
            ensure!(status == 200, "Analogue resume LIST failed");
            for object in &page.contents {
                ensure!(
                    after.as_ref().is_none_or(|key| object.key > *key),
                    "Analogue LIST did not advance"
                );
                after = Some(object.key.clone());
                if object.size >= 16
                    && let Ok(order) = Order::from_key(&object.key)
                    && order.covers(&next)
                {
                    candidates.push((order, object.key.clone()));
                }
            }
            if !page.is_truncated {
                break;
            }
            ensure!(!page.contents.is_empty(), "truncated empty Analogue LIST");
        }
        candidates.sort_by(|(a, _), (b, _)| b.highest.cmp(&a.highest));
        let mut found = None;
        for (order, key) in candidates {
            let result = async {
                let response = crate::object_read::if_present(bucket)?
                    .get_object(&key)
                    .await?;
                ensure!(response.status_code() == 200, "BulkOrder resume GET failed");
                decode(response.as_slice(), &order)?;
                Ok::<_, anyhow::Error>(())
            }
            .await;
            if result.is_ok() {
                found = Some(order.highest);
                break;
            }
        }
        let Some(highest) = found else {
            break;
        };
        next = highest.min(target + 1u8);
    }
    Ok(next)
}

/// Coalesced durable targets never hold up the publisher. Timeline bytes are the queue.
pub async fn run(
    bucket: Box<Bucket>,
    mut targets: watch::Receiver<timeline::Prefix>,
    config: Config,
    mut events: mpsc::Receiver<DurableEvent>,
) {
    let target = targets.borrow().milestone().clone();
    let mut next = match resume(&bucket, &target).await {
        Ok(next) => next,
        Err(error) => {
            tracing::warn!(%error, "Analogue resume failed; reconstructing from Timeline");
            1u8.into()
        }
    };
    let mut cached = std::collections::BTreeMap::new();
    let mut builder: Option<Builder> = None;
    let mut started = tokio::time::Instant::now();
    let mut pending = None;
    let mut closing = false;
    loop {
        let target = targets.borrow_and_update().milestone().clone();
        while let Ok(event) = events.try_recv() {
            if event.milestone >= next && cached.len() < 2 {
                cached.insert(event.milestone, event.bytes);
            }
        }
        if pending.is_none() && next <= target {
            let result = async {
                let bytes = if let Some(bytes) = cached.remove(&next) {
                    bytes
                } else {
                    let response = crate::object_read::if_present(&bucket)?
                        .get_object(timeline::key(&next)?)
                        .await?;
                    ensure!(
                        response.status_code() == 200,
                        "BulkOrder source GET returned HTTP {}",
                        response.status_code()
                    );
                    response.to_vec()
                };
                if builder.is_none() {
                    builder = Some(Builder::new(next.clone())?);
                    started = tokio::time::Instant::now();
                }
                builder.as_mut().unwrap().append(&next, &bytes)?;
                Ok::<_, anyhow::Error>(())
            }
            .await;
            if let Err(error) = result {
                tracing::warn!(%error, milestone=%next, "BulkOrder source will retry");
                if closing || targets.has_changed().is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            next += 1u8;
        }
        let due = builder.as_ref().is_some_and(|b| {
            b.len() >= config.target_bytes
                || started.elapsed() >= config.max_age
                || closing && next > target
        });
        if due {
            match builder.take().unwrap().finish() {
                Ok(order) => pending = Some(order),
                Err(error) => {
                    tracing::error!(%error, "BulkOrder encoding stopped");
                    return;
                }
            }
        }
        if let Some((order, bytes, range)) = &pending {
            match publish(&bucket, order, bytes, range.clone()).await {
                Ok(()) => {
                    tracing::info!(lowest=%order.lowest, highest=%order.highest, bytes=bytes.len(), "BulkOrder published");
                    pending = None;
                }
                Err(error) => {
                    tracing::warn!(%error, "BulkOrder upload will retry");
                    if closing || targets.has_changed().is_err() {
                        return;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        }
        if next <= target {
            continue;
        }
        if closing {
            return;
        }
        if builder.is_some() {
            tokio::select! {
                changed = targets.changed() => closing = changed.is_err(),
                _ = tokio::time::sleep_until(started + config.max_age) => {},
            }
        } else {
            closing = targets.changed().await.is_err();
        }
    }
}
