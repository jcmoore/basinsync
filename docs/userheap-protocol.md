[conditional-read]: <glossary/conditional-read.md>
[conditional-write]: <glossary/conditional-write.md>
[context]: <glossary/context.md>
[epilogue-protocol]: <epilogue-protocol.md>
[etag]: <glossary/etag.md>
[flow]: <glossary/flow.md>
[json0]: <glossary/json0.md>
[node]: <glossary/node.md>
[node-height]: <glossary/node-height.md>
[node-promotion]: <glossary/node-promotion.md>
[object-keys]: <object-keys.md>
[old-generation]: <glossary/old-generation.md>
[revision]: <glossary/revision.md>
[userheap-reservation]: <glossary/userheap-reservation.md>
[viewpoint]: <glossary/viewpoint.md>
[young-generation]: <glossary/young-generation.md>

# UserHeap staging protocol

`POST /api/v0/userheap/reserve` accepts *JSON0*[^][json0] for the
*Flow's*[^][flow] expected next *Viewpoint*[^][viewpoint]:

```ts
{
  flow: string;
  viewpoint: bigint;
  nodes: Array<{
    auth: [height: number, version: number, token: Uint8Array];
    length: number;
    context?: Record<string, string>;
    edges: Array<{ auth: [height: number, version: number, token: Uint8Array] }>;
  }>;
}
```

Edges identify a child set: each child has lower *height*[^][node-height] and a
verified stored allocation in either generation. Existing *nodes*[^][node] require matching content
length, *context*[^][context], and child-id set, even when already referenced by other *nodes*[^][node].

A wrong *Viewpoint*[^][viewpoint] returns the *common 409 response*[^][epilogue-protocol].
Otherwise return HTTP 200 with nullable maps indexed by input position:

```ts
type Action = { url: string; headers: Record<string, string> };
{
  errors: Record<number, { code: string; message: string }> | null;
  results: Record<number, { actions: {
    write: Action | null; preview: Action | null; read: Action;
  } }> | null;
  timelines: { datashed: { milestone: bigint } };
}
```

Successful *reservations*[^][userheap-reservation] are durable before returning.
`write` is a *conditional PUT*[^][conditional-write] to the
*young generation*[^][young-generation], signing `If-None-Match: *` and the
specified `Content-Length` plus every `x-amz-meta-ctx-${key}` header from that
*node's*[^][node] *context*[^][context] (default `{}`). The first reservation fixes its *context*[^][context]; compare names
case-insensitively and values exactly. Mismatches return an indexed
`context_collision` error without actions, even before upload. `preview` and `read` are
*conditional GETs*[^][conditional-read] for that *node*[^][node] in the
*young*[^][young-generation] and *old*[^][old-generation] generations respectively.
Both *object-key patterns*[^][object-keys] share the stored UUIDv4
*revision*[^][revision]. Send each action's headers. A nonempty stored *etag*[^][etag] makes `write` null; a valid
*promotion*[^][node-promotion] timestamp makes `preview` null.

