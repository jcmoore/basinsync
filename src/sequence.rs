//! Bucket-proven sequence sentinels, tombstones, and publication postconditions.
use anyhow::{Context, Result, ensure};
use axum::http::{HeaderMap, header};
use s3::{Bucket, request::ResponseData};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

pub const REFERENCE_KEY: &str = "sentinel/empty";
const ATTEMPTS: usize = 8;
#[derive(Clone)]
struct References {
    empty: String,
    tombstone: Vec<u8>,
    padding: Vec<u8>,
}
static REFERENCES: OnceLock<Mutex<BTreeMap<String, References>>> = OnceLock::new();
fn references() -> &'static Mutex<BTreeMap<String, References>> {
    REFERENCES.get_or_init(Default::default)
}
fn cached<T>(bucket: &Bucket, read: impl FnOnce(&References) -> T) -> Result<T> {
    references()
        .lock()
        .unwrap()
        .get(&bucket.url())
        .map(read)
        .context("bucket sentinel is not initialized")
}
pub fn reference(bucket: &Bucket) -> Result<String> {
    cached(bucket, |proof| proof.empty.clone())
}
pub fn tombstone(bucket: &Bucket) -> Result<Vec<u8>> {
    cached(bucket, |proof| proof.tombstone.clone())
}
pub fn padding(bucket: &Bucket) -> Result<Vec<u8>> {
    cached(bucket, |proof| proof.padding.clone())
}
fn etag(response: &ResponseData) -> Result<String> {
    response
        .headers()
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("etag"))
        .map(|(_, value)| value)
        .context("successful sequence write has no ETag")
}
async fn probe(bucket: &Bucket, key: &str, bytes: &[u8]) -> Result<String> {
    let result = bucket
        .put_object_with_headers(key, bytes, Some(crate::epilogue::conditional_headers()))
        .await;
    let existing = crate::object_read::if_present(bucket)?
        .get_object(key)
        .await?;
    ensure!(
        existing.status_code() == 200 && existing.as_slice() == bytes,
        "invalid bucket reference {key}: {result:?}"
    );
    etag(&existing)
}

/// Probe actual provider validators. A tombstone must stay below the EnvBuf floor.
pub async fn initialize(bucket: &Bucket) -> Result<String> {
    let result = bucket
        .put_object_with_headers(
            REFERENCE_KEY,
            &[],
            Some(crate::epilogue::conditional_headers()),
        )
        .await;
    let (head, status) = crate::object_read::if_present(bucket)?
        .head_object(REFERENCE_KEY)
        .await?;
    ensure!(
        status == 200 && head.content_length == Some(0),
        "invalid empty sentinel reference: {result:?}"
    );
    let empty = head.e_tag.context("empty sentinel reference has no ETag")?;
    let mut chosen = None;
    for length in (2..16).step_by(2) {
        let bytes = [0xde, 0xad].repeat(length / 2);
        if probe(bucket, &format!("sentinel/tombstone/{length}"), &bytes).await? != empty {
            chosen = Some(bytes);
            break;
        }
    }
    let tombstone =
        chosen.context("no distinct tombstone below 16 bytes; refusing 16-byte tombstone")?;
    let mut chosen = None;
    for message_length in 1..=8 {
        let mut bytes = vec![];
        crate::envbuf::append(
            &mut bytes,
            &[0xee; 8][..message_length],
            crate::envbuf::PADDING,
        )?;
        if probe(
            bucket,
            &format!("sentinel/padding/{message_length}"),
            &bytes,
        )
        .await?
            != empty
        {
            chosen = Some(bytes);
            break;
        }
    }
    let padding =
        chosen.context("cannot distinguish padding with message lengths 1–8 from sentinel")?;
    references().lock().unwrap().insert(
        bucket.url(),
        References {
            empty: empty.clone(),
            tombstone,
            padding,
        },
    );
    Ok(empty)
}

/// Only zero-byte sentinels may be adopted by another writer; never tombstones.
pub async fn replacement(
    bucket: &Bucket,
    key: &str,
    mut headers: HeaderMap,
) -> Result<Option<HeaderMap>> {
    let (head, status) = crate::object_read::if_present(bucket)?
        .head_object(key)
        .await?;
    match status {
        200 if head.content_length == Some(0) => {
            headers.remove(header::IF_NONE_MATCH);
            headers.insert(
                header::IF_MATCH,
                head.e_tag.context("sentinel has no ETag")?.parse()?,
            );
            Ok(Some(headers))
        }
        200 | 404 | 412 => Ok(None),
        _ => anyhow::bail!("sentinel HEAD returned HTTP {status}"),
    }
}
async fn send(
    bucket: &Bucket,
    key: &str,
    bytes: &[u8],
    headers: HeaderMap,
) -> Result<ResponseData> {
    ensure!(
        bytes.len() >= 16,
        "published sequence entry is below the EnvBuf floor"
    );
    if reference(bucket).is_err() {
        initialize(bucket).await?;
    }
    let response = bucket
        .put_object_with_headers(key, bytes, Some(headers.clone()))
        .await?;
    if response.status_code() == 412
        && let Some(headers) = replacement(bucket, key, headers).await?
    {
        return Ok(bucket
            .put_object_with_headers(key, bytes, Some(headers))
            .await?);
    }
    Ok(response)
}

/// For opaque legacy entries, fail closed if the response aliases the sentinel.
pub async fn put(
    bucket: &Bucket,
    key: &str,
    bytes: &[u8],
    headers: HeaderMap,
) -> Result<ResponseData> {
    let response = send(bucket, key, bytes, headers).await?;
    if (200..300).contains(&response.status_code()) {
        ensure!(
            etag(&response)? != reference(bucket)?,
            "published data shares the empty sentinel validator"
        );
    }
    Ok(response)
}

pub async fn put_entry(
    bucket: &Bucket,
    key: impl AsRef<str>,
    bytes: &[u8],
    headers: HeaderMap,
) -> Result<ResponseData> {
    if bytes.starts_with(b"enV") {
        put_feed(bucket, key.as_ref(), bytes, headers).await
    } else {
        put(bucket, key.as_ref(), bytes, headers).await
    }
}

/// Only the writer that observed a collision may replace its own colliding payload.
pub async fn put_feed(
    bucket: &Bucket,
    key: &str,
    bytes: &[u8],
    mut headers: HeaderMap,
) -> Result<ResponseData> {
    let mut bytes = Cow::Borrowed(bytes);
    for _ in 0..ATTEMPTS {
        let response = send(bucket, key, &bytes, headers.clone()).await?;
        if !(200..300).contains(&response.status_code()) || etag(&response)? != reference(bucket)? {
            return Ok(response);
        }
        bytes.to_mut().extend_from_slice(&padding(bucket)?);
        headers.remove(header::IF_NONE_MATCH);
        headers.insert(header::IF_MATCH, reference(bucket)?.parse()?);
    }
    anyhow::bail!("sequence write could not escape the empty sentinel validator")
}

/// Single-PUT files stay on disk; collision retries append only verified padding.
pub async fn put_file(
    bucket: &Bucket,
    key: &str,
    path: &std::path::Path,
    mut headers: HeaderMap,
) -> Result<()> {
    for _ in 0..ATTEMPTS {
        let length = tokio::fs::metadata(path).await?.len();
        ensure!(
            length >= 16,
            "published sequence entry is below the EnvBuf floor"
        );
        headers.insert(header::CONTENT_LENGTH, length.to_string().parse()?);
        let signed = headers
            .iter()
            .map(|(name, value)| Ok((name.to_string(), value.to_str()?.to_owned())))
            .collect::<Result<_>>()?;
        let access = crate::range_get::signed_request(bucket, key, "PUT", signed).await?;
        let mut request = bucket
            .http_client()
            .put(&access.url)
            .body(tokio::fs::File::open(path).await?);
        for (name, value) in access.headers {
            request = request.header(name, value);
        }
        if let Some(timeout) = bucket.request_timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await?;
        let status = response.status().as_u16();
        let observed = response
            .headers()
            .get("etag")
            .map(|value| value.to_str().map(str::to_owned))
            .transpose()?;
        response.bytes().await?;
        if status == 412
            && let Some(retry) = replacement(bucket, key, headers.clone()).await?
        {
            headers = retry;
            continue;
        }
        ensure!(status == 200, "sequence PUT returned HTTP {status}");
        if observed.context("successful sequence write has no ETag")? != reference(bucket)? {
            return Ok(());
        }
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .await?;
        file.write_all(&padding(bucket)?).await?;
        file.flush().await?;
        headers.remove(header::IF_NONE_MATCH);
        headers.insert(header::IF_MATCH, reference(bucket)?.parse()?);
    }
    anyhow::bail!("sequence file write could not escape the empty sentinel validator")
}
