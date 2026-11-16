//! Conditional object reads; only existence probes interpret absent objects.
use anyhow::{Result, bail, ensure};
use axum::http::{HeaderValue, header};
use s3::Bucket;

pub fn if_present(bucket: &Bucket) -> Result<Bucket> {
    let mut headers = bucket.extra_headers().clone();
    headers.insert(header::IF_MATCH, HeaderValue::from_static("*"));
    Ok(bucket.with_extra_headers(headers)?)
}

pub async fn exists(bucket: &Bucket, key: &str) -> Result<bool> {
    let mut probe = if_present(bucket)?;
    probe.add_header("if-none-match", &crate::sequence::reference(bucket)?);
    let (head, status) = probe.head_object(key).await?;
    match status {
        // A provider that ignores preconditions can still answer correctly.
        200 => match head.content_length {
            Some(0) => Ok(false),
            Some(length) if (1..16).contains(&length) => {
                let expected = crate::sequence::tombstone(bucket)?;
                let object = if_present(bucket)?.get_object(key).await?;
                ensure!(
                    object.status_code() == 200 && object.as_slice() == expected,
                    "invalid sub-EnvBuf sequence object"
                );
                // An immutable tombstone occupies an older position: a lower bound.
                Ok(true)
            }
            Some(length) if length >= 16 => Ok(true),
            _ => bail!("sequence HEAD has no valid content length"),
        },
        304 => Ok(false),
        // Some providers check If-Match before absence; others return 404 first.
        404 | 412 => Ok(false),
        _ => bail!("object existence HEAD returned HTTP {status}"),
    }
}
