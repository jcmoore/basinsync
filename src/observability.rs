//! Process-local telemetry. Never pass client data as metric labels or Debug arguments.
use anyhow::{Context, Result, ensure};
use axum::{extract::Request, middleware::Next, response::Response};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;
use tracing::{Instrument, Span};

pub fn init() -> Result<PrometheusHandle> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "basinsync=info".into());
    match std::env::var("BASINSYNC_LOG_FORMAT")
        .as_deref()
        .unwrap_or("text")
    {
        "text" => tracing_subscriber::fmt().with_env_filter(filter).init(),
        "json" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init(),
        _ => anyhow::bail!("BASINSYNC_LOG_FORMAT must be text or json"),
    }
    let handle = PrometheusBuilder::new()
        .set_buckets(&[
            0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30.,
            60.,
        ])?
        .install_recorder()?;
    metrics::describe_histogram!(
        "basinsync_stage_seconds",
        metrics::Unit::Seconds,
        "Time spent in a processing stage; labels are a fixed vocabulary."
    );
    metrics::describe_histogram!(
        "basinsync_http_seconds",
        metrics::Unit::Seconds,
        "HTTP handler duration, including admission and durable acknowledgement waits."
    );
    metrics::describe_gauge!(
        "basinsync_background_pending",
        "Durable queued work in the last sampled database snapshot."
    );
    metrics::describe_gauge!(
        "basinsync_background_oldest_age_seconds",
        metrics::Unit::Seconds,
        "Age of oldest durable queued item in the last sampled database snapshot."
    );
    Ok(handle)
}

/// A drop records abandonment, never a successful completion.
pub struct Timer {
    stage: &'static str,
    operation: &'static str,
    started: Instant,
    outcome: &'static str,
}
impl Timer {
    pub fn new(stage: &'static str, operation: &'static str) -> Self {
        gauge!("basinsync_stage_inflight", "stage" => stage, "operation" => operation)
            .increment(1.);
        Self {
            stage,
            operation,
            started: Instant::now(),
            outcome: "abandoned",
        }
    }
    pub fn finish(mut self, ok: bool) {
        self.outcome = if ok { "ok" } else { "failed" };
    }
}
fn observation(
    stage: &'static str,
    operation: &'static str,
    outcome: &'static str,
    started: Instant,
) {
    gauge!("basinsync_stage_inflight", "stage"=>stage,"operation"=>operation).decrement(1.);
    histogram!("basinsync_stage_seconds", "stage"=>stage,"operation"=>operation,"outcome"=>outcome)
        .record(started.elapsed().as_secs_f64());
    counter!("basinsync_stage_total", "stage"=>stage,"operation"=>operation,"outcome"=>outcome)
        .increment(1);
}
impl Drop for Timer {
    fn drop(&mut self) {
        observation(self.stage, self.operation, self.outcome, self.started);
    }
}

pub async fn measured<T, E>(
    stage: &'static str,
    operation: &'static str,
    future: impl std::future::Future<Output = std::result::Result<T, E>>,
) -> std::result::Result<T, E> {
    let span = tracing::info_span!("operation", stage, operation);
    async move {
        let timer = Timer::new(stage, operation);
        let result = future.await;
        timer.finish(result.is_ok());
        result
    }
    .instrument(span)
    .await
}

struct Http {
    span: Span,
    operation: &'static str,
    started: Instant,
    status: Option<u16>,
}
impl Drop for Http {
    fn drop(&mut self) {
        let status = self
            .status
            .map_or_else(|| "cancelled".into(), |s| s.to_string());
        let seconds = self.started.elapsed().as_secs_f64();
        counter!("basinsync_http_total", "operation" => self.operation, "status" => status.clone())
            .increment(1);
        histogram!("basinsync_http_seconds", "operation" => self.operation, "status" => status.clone()).record(seconds);
        gauge!("basinsync_http_inflight", "operation" => self.operation).decrement(1.);
        tracing::info!(parent: &self.span, %status, seconds, "HTTP completed");
    }
}
pub async fn http(request: Request, next: Next) -> Response {
    if request.method() == axum::http::Method::GET && request.uri().path() == "/metrics" {
        return next.run(request).await;
    }
    let operation = match (request.method().as_str(), request.uri().path()) {
        ("PUT", "/api/v0/epilogue") => "commit",
        ("POST", "/api/v0/userspace/heap/reserve") => "reserve",
        ("GET", "/api/v0/epilogue/status") => "status",
        _ => "other",
    };
    let id = uuid::Uuid::new_v4().to_string();
    let span = tracing::info_span!(parent: None, "request", request_id=%id, operation, flow=tracing::field::Empty, viewpoint=tracing::field::Empty, milestone=tracing::field::Empty);
    gauge!("basinsync_http_inflight", "operation" => operation).increment(1.);
    let mut observation = Http {
        span: span.clone(),
        operation,
        started: Instant::now(),
        status: None,
    };
    let mut response = next.run(request).instrument(span).await;
    observation.status = Some(response.status().as_u16());
    response
        .headers_mut()
        .insert("x-request-id", id.parse().expect("UUID header"));
    response
}

/// Owned by queued work independently of the HTTP future.
pub struct Submission {
    pub span: Span,
    operation: &'static str,
    enqueued: Instant,
    queued: bool,
    done: bool,
    batch_id: Option<uuid::Uuid>,
}
impl Submission {
    pub fn new(operation: &'static str) -> Self {
        gauge!("basinsync_requests_queued", "operation" => operation).increment(1.);
        gauge!("basinsync_submissions_inflight", "operation" => operation).increment(1.);
        Self {
            span: Span::current(),
            operation,
            enqueued: Instant::now(),
            queued: true,
            done: false,
            batch_id: None,
        }
    }
    pub fn assigned(&mut self, batch: &BatchTrace) {
        batch.span.follows_from(self.span.id());
        self.batch_id = Some(batch.id);
        if self.queued {
            gauge!("basinsync_requests_queued", "operation" => self.operation).decrement(1.);
            histogram!("basinsync_stage_seconds", "stage" => "queue", "operation" => self.operation, "outcome" => "ok").record(self.enqueued.elapsed().as_secs_f64());
            self.queued = false;
        }
        tracing::debug!(parent: &self.span, batch_id=%batch.id, "request assigned to batch");
    }
    pub fn finish(
        mut self,
        outcome: &'static str,
        milestone: &num_bigint::BigUint,
        receiver_open: bool,
    ) {
        let seconds = self.enqueued.elapsed().as_secs_f64();
        histogram!("basinsync_stage_seconds", "stage" => "acknowledgement", "operation" => self.operation, "outcome" => outcome).record(seconds);
        counter!("basinsync_submissions_total", "operation" => self.operation, "outcome" => outcome).increment(1);
        let batch_id = self.batch_id.map(|id| id.to_string());
        tracing::info!(parent: &self.span, batch_id=batch_id.as_deref(), %milestone, outcome, receiver_open, seconds, "submission completed");
        self.done = true;
    }
}
impl Drop for Submission {
    fn drop(&mut self) {
        if self.queued {
            gauge!("basinsync_requests_queued", "operation" => self.operation).decrement(1.);
        }
        gauge!("basinsync_submissions_inflight", "operation" => self.operation).decrement(1.);
        if !self.done {
            counter!("basinsync_submissions_total", "operation" => self.operation, "outcome" => "abandoned").increment(1);
            tracing::warn!(parent: &self.span, "submission abandoned before completion");
        }
    }
}

pub struct BatchTrace {
    pub span: Span,
    id: uuid::Uuid,
    operation: &'static str,
    preparation: Option<Instant>,
    ready: Option<Instant>,
    publishing: bool,
    done: bool,
}
impl BatchTrace {
    pub fn new(operation: &'static str) -> Self {
        let id = uuid::Uuid::new_v4();
        let span = tracing::info_span!(parent: None, "batch", batch_id=%id, operation, milestone=tracing::field::Empty);
        gauge!("basinsync_stage_inflight", "stage"=>"prepare","operation"=>operation).increment(1.);
        Self {
            span,
            id,
            operation,
            preparation: Some(Instant::now()),
            ready: None,
            publishing: false,
            done: false,
        }
    }
    pub fn prepared(&mut self, milestone: &num_bigint::BigUint) {
        self.span
            .record("milestone", tracing::field::display(milestone));
        self.preparation_done("ok");
        self.ready = Some(Instant::now());
        gauge!("basinsync_batches_queued").increment(1.);
    }
    pub fn publishing(&mut self) {
        if let Some(ready) = self.ready.take() {
            gauge!("basinsync_batches_queued").decrement(1.);
            histogram!("basinsync_stage_seconds", "stage" => "publication_queue", "operation" => self.operation, "outcome" => "ok").record(ready.elapsed().as_secs_f64());
        }
        gauge!("basinsync_batches_publishing").increment(1.);
        self.publishing = true;
    }
    fn preparation_done(&mut self, outcome: &'static str) {
        if let Some(started) = self.preparation.take() {
            observation("prepare", self.operation, outcome, started);
        }
    }
    pub fn preparation_failed(&mut self) {
        self.preparation_done("failed");
    }
    pub fn finish(&mut self, ok: bool) {
        self.preparation_done(if ok { "ok" } else { "failed" });
        let outcome = if ok { "ok" } else { "failed" };
        counter!("basinsync_batches_total", "operation" => self.operation, "outcome" => outcome)
            .increment(1);
        tracing::debug!(parent: &self.span, outcome, "batch completed");
        self.done = true;
    }
}
impl Drop for BatchTrace {
    fn drop(&mut self) {
        self.preparation_done("abandoned");
        if self.ready.is_some() {
            gauge!("basinsync_batches_queued").decrement(1.);
        }
        if self.publishing {
            gauge!("basinsync_batches_publishing").decrement(1.);
        }
        if !self.done {
            counter!("basinsync_batches_total", "operation" => self.operation,"outcome" => "abandoned").increment(1);
        }
    }
}

/// Scraping reads only the recorder; periodic upkeep bounds histogram buffering even without scrapes.
pub async fn upkeep(handle: PrometheusHandle, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(1));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        handle.run_upkeep();
        if *shutdown.borrow() {
            return;
        }
        tokio::select! {_=shutdown.changed()=>{}, _=ticks.tick()=>{}}
    }
}

fn oldest_age(text: Option<String>) -> Result<f64> {
    let Some(text) = text else {
        return Ok(0.);
    };
    let at = time::OffsetDateTime::parse(&text, &time::format_description::well_known::Rfc3339)
        .or_else(|_| {
            time::PrimitiveDateTime::parse(
                &text,
                &time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
            )
            .map(|t| t.assume_utc())
        })?;
    Ok((time::OffsetDateTime::now_utc() - at)
        .as_seconds_f64()
        .max(0.))
}
fn sample_queue(
    conn: &std::sync::Arc<turso_core::Connection>,
    space: &'static str,
    queue: &'static str,
    sql: &str,
) -> Result<()> {
    crate::turso::rows(conn, sql, |row| {
        let count = row.get::<i64>(0)?;
        ensure!(count >= 0, "negative backlog count");
        let oldest = if matches!(row.get::<&turso_core::Value>(1)?, turso_core::Value::Null) {
            None
        } else {
            Some(row.get::<String>(1)?)
        };
        gauge!("basinsync_background_pending", "space"=>space,"queue"=>queue).set(count as f64);
        gauge!("basinsync_background_oldest_age_seconds", "space"=>space,"queue"=>queue)
            .set(oldest_age(oldest)?);
        Ok(())
    })
}
fn sample(snapshot: &crate::timeline::Snapshot) -> Result<()> {
    let user = snapshot.view.connect()?;
    sample_queue(
        &user,
        "userspace",
        "sweeping",
        "SELECT count(*),min(prioritized) FROM sweeping_tasks",
    )?;
    sample_queue(
        &user,
        "userspace",
        "marking",
        "SELECT count(*),min(prioritized) FROM marking_tasks",
    )?;
    sample_queue(
        &user,
        "userspace",
        "freed",
        "SELECT count(*),min(freed) FROM userspace_nodes WHERE freed>''",
    )?;
    sample_queue(
        &user,
        "userspace",
        "promotion",
        "SELECT count(*),min(marked) FROM userspace_nodes WHERE promoted='' AND marked>'' AND freed=''",
    )?;
    if let Some(view) = &snapshot.kernel_view {
        let kernel = view.connect()?;
        sample_queue(
            &kernel,
            "kernelspace",
            "marking",
            "SELECT count(*),min(prioritized) FROM marking_tasks",
        )?;
        sample_queue(
            &kernel,
            "kernelspace",
            "promotion",
            "SELECT count(*),min(marked) FROM kernelspace_nodes WHERE promoted='' AND marked>'' AND freed=''",
        )?;
    }
    gauge!("basinsync_background_sample_timestamp_seconds")
        .set(time::OffsetDateTime::now_utc().unix_timestamp() as f64);
    Ok(())
}
/// Read-only, pinned MVCC snapshots; a scrape never performs SQL or S3 I/O.
pub async fn sample_loop(
    mut snapshots: tokio::sync::watch::Receiver<Option<crate::timeline::Snapshot>>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    mut halted: tokio::sync::watch::Receiver<bool>,
) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(1));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    for (space, queue) in [
        ("userspace", "sweeping"),
        ("userspace", "marking"),
        ("userspace", "freed"),
        ("userspace", "promotion"),
        ("kernelspace", "marking"),
        ("kernelspace", "promotion"),
    ] {
        gauge!("basinsync_background_pending","space"=>space,"queue"=>queue).set(0.);
        gauge!("basinsync_background_oldest_age_seconds","space"=>space,"queue"=>queue).set(0.);
    }
    loop {
        if *shutdown.borrow() || *halted.borrow() || snapshots.has_changed().is_err() {
            return;
        }
        tokio::select! {
            _=shutdown.changed()=>return,
            _=halted.changed()=>return,
            _=ticks.tick()=>{},
            changed=snapshots.changed()=>{if changed.is_err(){return;} continue;},
        }
        let snapshot = snapshots.borrow().clone();
        if let Some(snapshot) = snapshot {
            let result = measured("sample", "backlog", async {
                tokio::task::spawn_blocking(move || sample(&snapshot))
                    .await
                    .context("telemetry sampler panicked")?
            })
            .await;
            if result.is_err() {
                tracing::warn!("background telemetry sample failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abandonment_and_failure_never_become_success_or_leak_inflight_counts() {
        let recorder = PrometheusBuilder::new()
            .set_buckets(&[0.01, 1.])
            .unwrap()
            .build_recorder();
        metrics::with_local_recorder(&recorder, || {
            drop(Timer::new("test", "timer"));
            Timer::new("test", "timer").finish(false);
            drop(Submission::new("commit"));
            let mut batch = BatchTrace::new("commit");
            let mut submitted = Submission::new("commit");
            submitted.assigned(&batch);
            batch.prepared(&1u8.into());
            batch.publishing();
            submitted.finish("failed", &0u8.into(), false);
            batch.finish(false);
        });
        let text = recorder.handle().render();
        assert!(text.lines().any(|l| l.starts_with("basinsync_stage_total{")
            && l.contains("stage=\"test\"")
            && l.contains("outcome=\"abandoned\"")
            && l.ends_with(" 1")));
        assert!(
            text.lines()
                .any(|l| l.starts_with("basinsync_submissions_total{")
                    && l.contains("outcome=\"failed\"")
                    && l.ends_with(" 1"))
        );
        assert!(
            !text
                .lines()
                .any(|l| l.starts_with("basinsync_submissions_total{")
                    && l.contains("outcome=\"ok\""))
        );
        for name in [
            "basinsync_stage_inflight",
            "basinsync_requests_queued",
            "basinsync_submissions_inflight",
            "basinsync_batches_queued",
            "basinsync_batches_publishing",
        ] {
            assert!(
                text.lines()
                    .filter(|l| l.starts_with(name))
                    .all(|l| l.ends_with(" 0")),
                "leaked gauge: {text}"
            );
        }
    }
    #[test]
    fn backlog_age_accepts_both_persisted_timestamp_formats() -> Result<()> {
        assert_eq!(oldest_age(None)?, 0.);
        let at = time::OffsetDateTime::now_utc() - time::Duration::seconds(10);
        for text in [
            at.format(&time::format_description::well_known::Rfc3339)?,
            at.format(&time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second]"
            ))?,
        ] {
            assert!((9. ..12.).contains(&oldest_age(Some(text))?));
        }
        let future = at + time::Duration::seconds(100);
        assert_eq!(
            oldest_age(Some(
                future.format(&time::format_description::well_known::Rfc3339)?
            ))?,
            0.
        );
        Ok(())
    }
}
