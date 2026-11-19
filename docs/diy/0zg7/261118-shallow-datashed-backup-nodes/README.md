---
prerequisites:
  - 0zg6/261112-commit-marker-edges-with-node-promotion
  - 0zg4/261117-timeline-recovery-acceleration/0zg6/261117-recovering-from-bulk-orders
---

[access-token]: <../../../glossary/access-token.md>
[commit-marker]: <../../../glossary/commit-marker.md>
[conditional-write]: <../../../glossary/conditional-write.md>
[datashed]: <../../../glossary/datashed.md>
[datashed-backup]: <../../../datashed-backup.md>
[datashed-timeline]: <../../../datashed-timeline.md>
[development]: <../../../development.md>
[epilogue]: <../../../glossary/epilogue.md>
[etag]: <../../../glossary/etag.md>
[flow]: <../../../glossary/flow.md>
[heap]: <../../../glossary/heap.md>
[kernelspace]: <../../../glossary/kernelspace.md>
[lexidecimal]: <../../../glossary/lexidecimal.md>
[marking-task]: <../../../glossary/marking-task.md>
[milestone]: <../../../glossary/milestone.md>
[node]: <../../../glossary/node.md>
[node-promotion]: <../../../glossary/node-promotion.md>
[object-keys]: <../../../object-keys.md>
[old-generation]: <../../../glossary/old-generation.md>
[promotion-sign]: <../../../glossary/promotion-sign.md>
[retainer]: <../../../glossary/retainer.md>
[revision]: <../../../glossary/revision.md>
[timeline]: <../../../glossary/timeline.md>
[turso]: <../../../glossary/turso.md>
[turso-state]: <../../../turso-state.md>
[userspace]: <../../../glossary/userspace.md>
[viewpoint]: <../../../glossary/viewpoint.md>
[virtual-file-system]: <../../../glossary/virtual-file-system.md>
[young-generation]: <../../../glossary/young-generation.md>

# Shallow Datashed backup nodes

Add `kernel.turso` alongside `default.turso`, using the same *Turso*[~][turso]
settings and *Reservoir VFS*[~][virtual-file-system]. Keep client-driven
*UserSpace*[~][userspace] in the latter and server-driven *KernelSpace*[~][kernelspace]
in the former. Capture both databases' *deltas*[~][turso-state] in one
`ingestturso` tuple, sharing one *Timeline*[~][timeline] publication.

Pin both databases before each speculative group. If any participant fails,
discard the group and reopen every writer on sparse writable branches of those
pins, with fresh engine state and sidecars. Preserve the preceding prepared
prefix; published groups remain durable. Exercise failures before and after
local commits and checkpoints, without S3 recovery.

In `kernel.turso`, create `kernelspace_nodes` and `kernelspace_edges` as
*UserSpace's*[~][userspace] corresponding tables, with the same composite edge
indexes and `retainer < node` constraint. Add hexadecimal `sha384` to the
*node*[~][node] descriptor. Keep the shared applied-*Milestone*[~][milestone]
row, plus:

```sql
CREATE TABLE kernel_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  next_viewpoint TEXT NOT NULL,
  activity TEXT NOT NULL
);
INSERT INTO kernel_state VALUES (1, '0', '0');
CREATE TABLE kernel_commits (
  viewpoint TEXT PRIMARY KEY NOT NULL,
  upstream TEXT NOT NULL,
  covered TEXT NOT NULL,
  sha384 TEXT NOT NULL
);
```

`next_viewpoint` and `activity` are decimal; `viewpoint`, `upstream`, and `covered` use
positive *Lexidecimal*[~][lexidecimal]. `kernel_state` tracks the next *Datashed*[~][datashed] *Viewpoint*[~][viewpoint]
and the last non-backup mutation's *Milestone*[~][milestone] (`activity`), so
backup bookkeeping cannot continually trigger new backups. `kernel_commits`
indexes each *Viewpoint*[~][viewpoint] by its committing *Milestone*[~][milestone],
covered prefix, and hexadecimal root digest. Keep root messages only in a bounded
memory cache; recover misses from `commitkernel` on the *Timeline*[~][timeline]
or the published *Epilogue marker*[~][commit-marker].
Database images remain in *Heap*[~][heap] *nodes*[~][node].

The backup worker pins both images at durable *Milestone*[~][milestone] `M`,
replicates covered *Flow*[~][flow] *markers*[~][commit-marker] as before, and exports the images outside the
request handler. Generate height-zero *node*[~][node] ids and UUIDv4
*revisions*[~][revision], then reserve them through the handler's internal queue.
Persist descriptors and `_a_` staging *retainers*[~][retainer] in
*KernelSpace*[~][kernelspace]; never persist *access tokens*[~][access-token].

Await reservation durability before uploading both images to the
*Heap's young generation*[~][young-generation], using the *KernelSpace keys*[~][object-keys]
and single *conditional PUTs*[~][conditional-write]. HEAD the uploaded revisions,
then request `commitkernel`: recheck their descriptors and staging edges,
require the covered prefix to advance, cache *ETags*[~][etag], replace staging edges with `_m_` *retainers*[~][retainer], advance
the *Viewpoint*[~][viewpoint], and index the root in one transaction.
Journal both database changes and the command through the *Datashed Timeline*[~][datashed-timeline].

Publish the acknowledged root in the *Epilogue*[~][epilogue]; repair interrupted
publication using `kernel_commits` to locate the durable command on restart. The *backup root*[~][datashed-backup]
identifies both images and prefix `M`. Restore that pair and replay from `M + 1`,
including its own reservations and `commitkernel`. Backup bookkeeping must not
trigger another backup while otherwise idle.

Queue a *marking task*[~][marking-task] for every `commitkernel` in the same
transaction, using the internal `datashed` owner:

```sql
CREATE TABLE marking_tasks (
  flow TEXT NOT NULL,
  viewpoint TEXT NOT NULL,
  upstream TEXT NOT NULL,
  prioritized DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (flow, viewpoint)
);
CREATE INDEX marking_tasks_prioritized
  ON marking_tasks(prioritized, flow, viewpoint);
```

Here `viewpoint` and `upstream` use positive *Lexidecimal*[~][lexidecimal];
`upstream` identifies the committing *Milestone*[~][milestone], after `M`.
The backup worker performs bounded *marking*[~][marking-task] and
*promotion*[~][node-promotion] phases even when no new backup is needed.
Reuse the task, marking-batch, and object-I/O concurrency limits. These shallow
*nodes*[~][node] have no children: mark verified retained revisions, then consume
the `(promoted, marked)` queue. *Conditionally COPY*[~][conditional-write] to the
*old generation*[~][old-generation], preserving image checksums and source
*ETags*[~][etag]; confirm the destination before deleting young. Retry failures
without relaying database images through the server.

Durably record `marked` and `promoted` through the internal handler. Once every
root is promoted and the *CommitMarker*[~][commit-marker] is stored, create and
confirm its *PromotionSign*[~][promotion-sign], with the task's committing
*Milestone*[~][milestone] as `upstream`; only then durably remove the task.
Promotion bookkeeping must not recursively create backups. Shutdown drains
already-started work; `BASINSYNC_BACKUP_ON_SHUTDOWN=true` opts into an additional
final backup and promotion pass. Keep the internal handler available until those
producers finish their durable updates.

Test paired-image checksums, logical/physical replay, bounded marking,
failed and ambiguous COPYs, failed DELETEs and sign confirmation, interrupted
publication, prefix eviction, concurrent writers, restart, and foreground
progress during blocked uploads or COPYs. Run the *development checks*[~][development].
*KernelSpace*[~][kernelspace] reclamation follows later.
