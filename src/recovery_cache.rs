//! Disposable, checksummed recovery cache. Eviction sacrifices hits, never correctness.
use anyhow::{Context, Result, ensure};
use num_bigint::BigUint;
use sha2::{Digest, Sha384};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Default)]
pub(crate) struct Metrics {
    pub peak_bytes: AtomicUsize,
    pub evicted: AtomicUsize,
}
type Inventory = Arc<Mutex<BTreeMap<BigUint, (Option<BigUint>, usize)>>>;
struct Record {
    segment: u64,
    offset: u64,
    length: usize,
    digest: [u8; 48],
}
struct Segment {
    id: u64,
    file: File,
    length: usize,
}
struct Ring {
    directory: tempfile::TempDir,
    segments: VecDeque<Segment>,
    records: BTreeMap<BigUint, Record>,
    inventory: Inventory,
    limit: usize,
    target: usize,
    bytes: usize,
    next: u64,
    metrics: Arc<Metrics>,
}
fn checksum(milestone: &BigUint, bytes: &[u8]) -> [u8; 48] {
    let id = milestone.to_bytes_be();
    let mut hash = Sha384::new();
    hash.update((id.len() as u64).to_le_bytes());
    hash.update(id);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
impl Ring {
    fn new(path: Option<&Path>, limit: usize, target: usize, inventory: Inventory) -> Result<Self> {
        ensure!(limit > 0 && target > 0, "invalid recovery disk limits");
        let directory = if let Some(path) = path {
            tempfile::Builder::new()
                .prefix("basinsync-recovery-")
                .tempdir_in(path)?
        } else {
            tempfile::Builder::new()
                .prefix("basinsync-recovery-")
                .tempdir()?
        };
        Ok(Self {
            directory,
            segments: VecDeque::new(),
            records: BTreeMap::new(),
            inventory,
            limit,
            target: target.min(limit),
            bytes: 0,
            next: 0,
            metrics: Arc::new(Metrics::default()),
        })
    }
    fn path(&self, id: u64) -> PathBuf {
        self.directory.path().join(format!("{id}.cache"))
    }
    fn evict(&mut self) -> Result<()> {
        if let Some(segment) = self.segments.pop_front() {
            let mut inventory = self.inventory.lock().unwrap();
            self.records.retain(|milestone, record| {
                if record.segment == segment.id {
                    self.metrics.evicted.fetch_add(1, Ordering::Relaxed);
                    inventory.remove(milestone);
                    false
                } else {
                    true
                }
            });
            drop(inventory);
            self.bytes -= segment.length;
            let path = self.path(segment.id);
            drop(segment);
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
    fn put(&mut self, milestone: BigUint, bytes: &[u8]) -> Result<bool> {
        if bytes.len() > self.limit {
            return Ok(false);
        }
        let coordinate = crate::event::coordinate(bytes).ok().flatten();
        let digest = checksum(&milestone, bytes);
        while self.bytes.saturating_add(bytes.len()) > self.limit {
            self.evict()?;
        }
        if self
            .segments
            .back()
            .is_none_or(|s| s.length > 0 && s.length.saturating_add(bytes.len()) > self.target)
        {
            let id = self.next;
            self.next = self.next.checked_add(1).context("cache segment overflow")?;
            let file = std::fs::OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(self.path(id))?;
            self.segments.push_back(Segment {
                id,
                file,
                length: 0,
            });
        }
        let segment = self.segments.back_mut().unwrap();
        let offset = segment.length as u64;
        // Only completed writes become visible. Failed writes disable this cache.
        segment.file.seek(SeekFrom::Start(offset))?;
        segment.file.write_all(bytes)?;
        segment.length += bytes.len();
        self.bytes += bytes.len();
        self.metrics
            .peak_bytes
            .fetch_max(self.bytes, Ordering::Relaxed);
        self.records.insert(
            milestone.clone(),
            Record {
                segment: segment.id,
                offset,
                length: bytes.len(),
                digest,
            },
        );
        self.inventory
            .lock()
            .unwrap()
            .insert(milestone, (coordinate, bytes.len()));
        Ok(true)
    }
    fn take(&mut self, milestone: &BigUint) -> Result<Option<Vec<u8>>> {
        self.inventory.lock().unwrap().remove(milestone);
        let Some(record) = self.records.remove(milestone) else {
            return Ok(None);
        };
        let segment = self
            .segments
            .iter_mut()
            .find(|s| s.id == record.segment)
            .context("missing cache segment")?;
        let mut bytes = vec![0; record.length];
        segment.file.seek(SeekFrom::Start(record.offset))?;
        segment.file.read_exact(&mut bytes)?;
        ensure!(
            checksum(milestone, &bytes) == record.digest,
            "recovery cache checksum mismatch"
        );
        Ok(Some(bytes))
    }
}
struct WriteRequest {
    milestone: BigUint,
    bytes: Arc<Vec<u8>>,
}
struct ReadRequest {
    milestone: BigUint,
    reply: oneshot::Sender<Option<Vec<u8>>>,
}
/// All filesystem work runs off the async executor. Saturated/failed storage is bypassed.
pub struct Cache {
    writes: mpsc::Sender<WriteRequest>,
    reads: mpsc::Sender<ReadRequest>,
    pub changed: watch::Receiver<usize>,
    inventory: Inventory,
    pub metrics: Arc<Metrics>,
}
impl Cache {
    pub async fn new(path: Option<PathBuf>, limit: usize, target: usize) -> Result<Self> {
        let inventory = Arc::new(Mutex::new(BTreeMap::new()));
        let mirror = inventory.clone();
        let mut ring =
            tokio::task::spawn_blocking(move || Ring::new(path.as_deref(), limit, target, mirror))
                .await??;
        let metrics = ring.metrics.clone();
        let (writes, mut write_rx) = mpsc::channel::<WriteRequest>(2);
        let (reads, mut read_rx) = mpsc::channel::<ReadRequest>(1);
        let (notify, changed) = watch::channel(0usize);
        tokio::spawn(async move {
            loop {
                enum Job {
                    Read(ReadRequest),
                    Write(WriteRequest),
                }
                let job = tokio::select! { biased;
                    read=read_rx.recv()=>match read { Some(read)=>Job::Read(read),None=>break },
                    write=write_rx.recv()=>match write { Some(write)=>Job::Write(write),None=>break },
                };
                let result = tokio::task::spawn_blocking(move || {
                    let result = match job {
                        Job::Write(write) => ring.put(write.milestone, &write.bytes).map(|_| ()),
                        Job::Read(read) => {
                            let bytes = ring.take(&read.milestone);
                            match bytes {
                                Ok(bytes) => {
                                    let _ = read.reply.send(bytes);
                                    Ok(())
                                }
                                Err(error) => {
                                    let _ = read.reply.send(None);
                                    Err(error)
                                }
                            }
                        }
                    };
                    (ring, result)
                })
                .await;
                match result {
                    Ok((next, Ok(()))) => ring = next,
                    Ok((_, Err(error))) => {
                        tracing::warn!(%error,"discarding recovery disk cache");
                        break;
                    }
                    Err(error) => {
                        tracing::warn!(%error,"recovery disk cache worker failed");
                        break;
                    }
                }
                notify.send_modify(|n| *n = n.wrapping_add(1));
            }
        });
        Ok(Self {
            writes,
            reads,
            changed,
            inventory,
            metrics,
        })
    }
    pub fn insert(&self, milestone: BigUint, bytes: Arc<Vec<u8>>) -> bool {
        self.writes
            .try_send(WriteRequest { milestone, bytes })
            .is_ok()
    }
    pub fn contains(&self, milestone: &BigUint) -> bool {
        !self.reads.is_closed() && self.inventory.lock().unwrap().contains_key(milestone)
    }
    pub fn suffix(&self, tail: &BigUint, floor: &BigUint) -> Option<(BigUint, BigUint)> {
        if self.reads.is_closed() {
            return None;
        }
        let inventory = self.inventory.lock().unwrap();
        let mut next = tail.clone();
        let mut result = None;
        while &next >= floor {
            let Some((Some(coordinate), _)) = inventory.get(&next) else {
                break;
            };
            result = Some((next.clone(), coordinate.clone()));
            if next == BigUint::default() {
                break;
            }
            next -= 1u8;
        }
        result
    }
    pub fn reader(&self, milestone: BigUint) -> impl Future<Output = Option<Vec<u8>>> + use<> {
        let sender = self.reads.clone();
        async move {
            let (reply, result) = oneshot::channel();
            sender.send(ReadRequest { milestone, reply }).await.ok()?;
            result.await.ok().flatten()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_reuse_truncation_and_corruption_are_cache_misses() -> Result<()> {
        let inventory = Arc::new(Mutex::new(BTreeMap::new()));
        let mut ring = Ring::new(None, 64, 32, inventory.clone())?;
        // Plain fixture bytes have no manifest; use a structurally valid raw envelope.
        let bytes = crate::envbuf::wrap(b"a")?;
        ring.limit = bytes.len() * 2;
        ring.target = bytes.len();
        for n in 1u8..=3 {
            ring.put(n.into(), &bytes)?;
        }
        assert!(ring.take(&1u8.into())?.is_none());
        assert_eq!(ring.take(&2u8.into())?, Some(bytes.clone()));
        let segment = ring.segments.back_mut().unwrap();
        segment.file.seek(SeekFrom::Start(0))?;
        segment.file.write_all(&[0])?;
        assert!(ring.take(&3u8.into()).is_err());
        ring.put(4u8.into(), &bytes)?;
        ring.segments.back_mut().unwrap().file.set_len(1)?;
        assert!(ring.take(&4u8.into()).is_err());
        assert!(ring.bytes <= ring.limit);
        assert!(!ring.put(5u8.into(), &vec![0; ring.limit + 1])?);
        Ok(())
    }
    #[tokio::test]
    async fn unavailable_cache_is_optional_and_drop_cleans_segments() -> Result<()> {
        assert!(
            Cache::new(Some(PathBuf::from("/no/such/cache/directory")), 64, 32)
                .await
                .is_err()
        );
        let directory = tempfile::tempdir()?;
        let cache = Cache::new(Some(directory.path().into()), 1024, 512).await?;
        let bytes = Arc::new(crate::envbuf::wrap(b"a")?);
        cache.insert(1u8.into(), bytes.clone());
        let mut changed = cache.changed.clone();
        changed.changed().await?;
        assert_eq!(cache.reader(1u8.into()).await, Some(bytes.as_ref().clone()));
        drop(cache);
        for _ in 0..100 {
            if std::fs::read_dir(directory.path())?.next().is_none() {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        anyhow::bail!("cache directory leaked")
    }
}
