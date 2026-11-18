//! A coalescing background consumer: replicate Flow markers before backing up a Datashed.
use crate::{
    epilogue, event,
    lexidecimal::Viewpoint,
    timeline::{self, Snapshot},
    turso,
};
use anyhow::{Context, Result, ensure};
use num_bigint::BigUint;
use s3::Bucket;
use std::time::Duration;
use tokio::sync::watch;

/// Replicate every Flow marker through this immutable, durable snapshot.
async fn replicate_flows(bucket: &Bucket, snapshot: &Snapshot) -> Result<()> {
    let previous = epilogue::latest_checkpoint(bucket).await?;
    let mut milestone = previous
        .as_ref()
        .map_or_else(|| 0u8.into(), |cp| cp.milestone.clone());
    if milestone >= *snapshot.prefix.milestone() {
        return Ok(());
    }
    let conn = snapshot.view.connect()?;
    let mut parent = previous.as_ref().map(|cp| cp.digest.clone());
    while milestone < *snapshot.prefix.milestone() {
        let next = &milestone + 1u8;
        let response = crate::object_read::if_present(bucket)?
            .get_object(timeline::key(&next)?)
            .await?;
        ensure!(
            response.status_code() == 200,
            "replication Timeline GET returned HTTP {}",
            response.status_code()
        );
        let bytes = response.as_slice();
        let commands = match event::decode_entry(bytes)? {
            event::Entry::Legacy(command) => vec![event::FlowCommand {
                markers: command.markers,
            }],
            event::Entry::Wal {
                commands, delta, ..
            } => {
                delta.validate_version()?;
                delta
                    .physical
                    .validate_binding(&milestone, parent.as_deref())?;
                commands
            }
        };
        let mut flows = std::collections::BTreeMap::<String, Vec<event::Marker>>::new();
        for command in commands {
            for (flow, marker) in command.markers {
                flows.entry(flow).or_default().push(marker);
            }
        }
        let mut copies = tokio::task::JoinSet::new();
        for (flow, markers) in flows {
            let revision = turso::flow_revision(&conn, &flow)?.context("missing Flow revision")?;
            if copies.len() == 8 {
                let copy = copies.join_next().await.context("missing Flow copy")?;
                copy??;
            }
            let bucket = bucket.clone();
            copies.spawn(async move {
                for marker in markers {
                    epilogue::replicate(&bucket, &flow, &revision, &marker).await?;
                }
                Ok::<_, anyhow::Error>(())
            });
        }
        while let Some(copy) = copies.join_next().await {
            copy??;
        }
        milestone = next;
        parent = Some(event::entry_digest(bytes)?);
    }
    ensure!(
        parent.as_deref() == snapshot.prefix.digest(),
        "backup prefix disagrees with Timeline"
    );
    Ok(())
}

pub(crate) async fn internal(
    queue: &tokio::sync::mpsc::Sender<crate::kernel::Request>,
    operation: crate::kernel::Operation,
) -> Result<Viewpoint> {
    let (reply, complete) = tokio::sync::oneshot::channel();
    queue
        .try_send(crate::kernel::Request { operation, reply })
        .map_err(|_| anyhow::anyhow!("backup request queue busy or closed"))?;
    complete.await.context("backup handler stopped")?
}

async fn publish_roots(bucket: &Bucket, snapshot: &Snapshot) -> Result<()> {
    let Some(view) = &snapshot.kernel_view else {
        return Ok(());
    };
    let view = view.clone();
    let commits = tokio::task::spawn_blocking(move || crate::kernel::committed(&view)).await??;
    let latest = epilogue::latest_checkpoint(bucket).await?;
    for (command, blob) in commits {
        let viewpoint = Viewpoint::parse(&command.viewpoint)?;
        if latest.as_ref().is_some_and(|cp| cp.viewpoint >= viewpoint) {
            continue;
        }
        let cp = command.graph.checkpoint(viewpoint, &blob)?;
        let bytes = crate::envbuf::wrap(&blob)?;
        let key = cp.viewpoint.key()?;
        let result = crate::sequence::put_feed(bucket, &key, &bytes, cp.headers()?).await;
        match result {
            Ok(response) if (200..300).contains(&response.status_code()) => {}
            result => {
                let existing = crate::object_read::if_present(bucket)?
                    .get_object(&key)
                    .await?;
                ensure!(
                    existing.status_code() == 200
                        && crate::envbuf::message(existing.as_slice())? == blob,
                    "conflicting Datashed graph marker: {result:?}"
                );
                cp.verify_headers(&existing.headers())?;
            }
        }
        tracing::info!(viewpoint=%cp.viewpoint.decimal(),milestone=%cp.milestone,"Datashed backup graph published");
    }
    Ok(())
}

async fn backup_graph(
    bucket: &Bucket,
    snapshot: &Snapshot,
    queue: &tokio::sync::mpsc::Sender<crate::kernel::Request>,
) -> Result<()> {
    // Complete roots whose Timeline command survived a crash before Epilogue publication.
    publish_roots(bucket, snapshot).await?;
    let previous = epilogue::latest_checkpoint(bucket).await?;
    if previous
        .as_ref()
        .is_some_and(|cp| cp.milestone >= snapshot.activity)
    {
        return Ok(());
    }
    replicate_flows(bucket, snapshot).await?; // replicate Flow markers through the pinned prefix
    let views = [
        (turso::ID, snapshot.view.clone()),
        (
            crate::kernel::ID,
            snapshot
                .kernel_view
                .clone()
                .context("missing kernel snapshot")?,
        ),
    ];
    let images = tokio::task::spawn_blocking(move || {
        let mut images = std::collections::BTreeMap::new();
        for (id, view) in views {
            images.insert(id, view.export()?);
        }
        Ok::<_, anyhow::Error>(images)
    })
    .await??;
    let nodes = images
        .iter()
        .map(|(id, (file, sha384))| {
            Ok((
                (*id).into(),
                crate::kernel::Node::new(file.as_file().metadata()?.len(), sha384.clone()),
            ))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    let graph = crate::kernel::Graph::new(&snapshot.prefix, nodes)?;
    let viewpoint = internal(queue, crate::kernel::Operation::Reserve(graph.clone())).await?;
    for (id, (file, _)) in &images {
        let node = &graph.databases[*id];
        let mut headers = epilogue::conditional_headers();
        headers.insert("x-amz-meta-sha384", node.sha384.parse()?);
        crate::kernel::upload(bucket, node, file.path(), headers).await?;
    }
    let etags = crate::kernel::verify(bucket, &graph).await?;
    internal(
        queue,
        crate::kernel::Operation::Commit {
            command: crate::kernel::Commit {
                viewpoint: viewpoint.decimal(),
                graph,
            },
            etags,
        },
    )
    .await?;
    // The watch target now includes the acknowledged commitkernel event.
    Ok(())
}

/// One active snapshot and one replaceable pending target; errors never gate client acks.
pub async fn run(
    bucket: Box<Bucket>,
    mut targets: watch::Receiver<Option<Snapshot>>,
    interval: Duration,
    queue: tokio::sync::mpsc::Sender<crate::kernel::Request>,
    mut shutdown: watch::Receiver<bool>,
    backup_on_shutdown: bool,
    promotion: crate::kernel::promotion::Limits,
) {
    let mut completed: BigUint = 0u8.into();
    loop {
        let sleep = tokio::time::sleep(interval);
        tokio::pin!(sleep);
        let closing = loop {
            if *shutdown.borrow() {
                break true;
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break true,
                _ = &mut sleep, if !interval.is_zero() => break false,
                changed = targets.changed() => if changed.is_err() { break true; },
            }
        };
        // Finish active work, but never start an exit-triggered pass unless requested.
        if (closing || *shutdown.borrow()) && !backup_on_shutdown {
            return;
        }
        let target = targets.borrow_and_update().clone();
        let mut failed = false;
        if let Some(snapshot) = target.filter(|s| s.activity > completed) {
            match backup_graph(&bucket, &snapshot, &queue).await {
                Ok(()) => {
                    completed = snapshot.activity.clone();
                    let target = targets.borrow().clone();
                    if let Some(target) = target
                        && let Err(error) = publish_roots(&bucket, &target).await
                    {
                        completed = 0u8.into();
                        tracing::warn!(%error,"Datashed graph publication will retry");
                    }
                }
                Err(error) => {
                    failed = true;
                    tracing::warn!(%error, "Datashed backup will retry");
                }
            }
        }
        if targets.borrow().is_some()
            && let Err(error) =
                crate::kernel::promotion::cycle(&bucket, &targets, &queue, promotion).await
        {
            tracing::warn!(%error,"KernelSpace marking will retry");
        }
        // An opted-in final pass retains the internal handler until it finishes.
        if closing {
            let more = targets
                .borrow()
                .as_ref()
                .is_some_and(|s| s.activity > completed);
            if !more || failed {
                return;
            }
            // A failed final attempt is recoverable from the Timeline; avoid an endless shutdown retry.
            if completed == BigUint::from(0u8) {
                return;
            }
        }
    }
}
