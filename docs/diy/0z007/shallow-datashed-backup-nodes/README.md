---
prerequisites:
  - 0z006/commit-marker-edges-with-node-promotion
  - 0z004/timeline-recovery-acceleration/0z006/recovering-from-bulk-orders
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
[milestone]: <../../../glossary/milestone.md>
[node]: <../../../glossary/node.md>
[object-keys]: <../../../object-keys.md>
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
  blob BLOB NOT NULL
);
```

`next_viewpoint` and `activity` are decimal; `viewpoint` and `upstream` use
positive *Lexidecimal*[~][lexidecimal]. The former table tracks the next *Datashed*[~][datashed]
*Viewpoint*[~][viewpoint] and last non-backup activity; the latter retains
*CommitMarker*[~][commit-marker] messages and their committing *Milestones*[~][milestone].

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
the *Viewpoint*[~][viewpoint], and retain the root message in one transaction.
Journal both database changes and the command through the *Datashed Timeline*[~][datashed-timeline].

Publish the acknowledged root in the *Epilogue*[~][epilogue]; repair interrupted
publication from `kernel_commits` on restart. The *backup root*[~][datashed-backup]
identifies both images and prefix `M`. Restore that pair and replay from `M + 1`,
including its own reservations and `commitkernel`. Backup bookkeeping must not
trigger another backup while otherwise idle.

Test paired-image checksums, logical/physical replay, failed uploads, interrupted
root publication, prefix eviction, concurrent writers, and foreground progress
with a blocked backup. Run the *development checks*[~][development].
*KernelSpace*[~][kernelspace] promotion and reclamation follow later; do not issue
*PromotionSigns*[~][promotion-sign] for graphs still in the *young generation*[~][young-generation].
