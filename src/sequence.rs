//! Empty sequence objects are metadata-only sentinels, never published entries.
use anyhow::{Context, Result, ensure};
use axum::http::{HeaderMap, header};
use s3::Bucket;

pub const REFERENCE_KEY: &str = "sentinel/empty";
static REFERENCES: std::sync::OnceLock<
    std::sync::Mutex<std::collections::BTreeMap<String, String>>,
> = std::sync::OnceLock::new();
fn references() -> &'static std::sync::Mutex<std::collections::BTreeMap<String, String>> {
    REFERENCES.get_or_init(Default::default)
}

pub fn reference(bucket: &Bucket) -> Result<String> {
    references()
        .lock()
        .unwrap()
        .get(&bucket.url())
        .cloned()
        .context("bucket sentinel is not initialized")
}

/// Establish the bucket's empty-object validator without overwriting an existing reference.
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
    let etag = head.e_tag.context("empty sentinel reference has no ETag")?;
    references()
        .lock()
        .unwrap()
        .insert(bucket.url(), etag.clone());
    Ok(etag)
}

/// Obtain a provider's actual validator; never assume encrypted ETags are MD5.
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

/// Normal writes need only one PUT. A failed creation may replace an observed sentinel.
pub async fn put(
    bucket: &Bucket,
    key: &str,
    bytes: &[u8],
    headers: HeaderMap,
) -> Result<s3::request::ResponseData> {
    ensure!(
        !bytes.is_empty(),
        "empty sequence payload is reserved for sentinels"
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
        let response = bucket
            .put_object_with_headers(key, bytes, Some(headers))
            .await?;
        validate_response(bucket, &response)?;
        return Ok(response);
    }
    validate_response(bucket, &response)?;
    Ok(response)
}
fn validate_response(bucket: &Bucket, response: &s3::request::ResponseData) -> Result<()> {
    if (200..300).contains(&response.status_code()) {
        for (name, value) in response.headers() {
            if name.eq_ignore_ascii_case("etag") {
                ensure!(
                    value != reference(bucket)?,
                    "published data shares the empty sentinel validator"
                );
            }
        }
    }
    Ok(())
}

/// Single-PUT file publication with a retryable, ETag-guarded sentinel replacement.
pub async fn put_file(
    bucket: &Bucket,
    key: &str,
    path: &std::path::Path,
    mut headers: HeaderMap,
) -> Result<()> {
    let length = std::fs::metadata(path)?.len();
    ensure!(
        length > 0,
        "empty sequence payload is reserved for sentinels"
    );
    headers.insert(header::CONTENT_LENGTH, length.to_string().parse()?);
    for attempt in 0..2 {
        let signed = headers
            .iter()
            .map(|(name, value)| Ok((name.to_string(), value.to_str()?.to_owned())))
            .collect::<Result<_>>()?;
        let access = crate::range_get::signed_request(bucket, key, "PUT", signed).await?;
        let reader = tokio::fs::File::open(path).await?;
        let mut request = bucket.http_client().put(&access.url).body(reader);
        for (name, value) in access.headers {
            request = request.header(name, value);
        }
        if let Some(timeout) = bucket.request_timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await?;
        let status = response.status().as_u16();
        if status == 200
            && let Some(etag) = response.headers().get("etag")
        {
            ensure!(
                etag.to_str()? != reference(bucket)?,
                "published data shares the empty sentinel validator"
            );
        }
        response.bytes().await?;
        if status == 412
            && attempt == 0
            && let Some(retry) = replacement(bucket, key, headers.clone()).await?
        {
            headers = retry;
            continue;
        }
        ensure!(status == 200, "sequence PUT returned HTTP {status}");
        return Ok(());
    }
    unreachable!()
}
