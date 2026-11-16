[conditional-write]: <conditional-write.md>
[context]: <context.md>
[empty-sentinel]: <empty-sentinel.md>
[envbuf]: <envbuf.md>
[epilogue]: <epilogue.md>
[promotion-sign]: <promotion-sign.md>
[range-get]: <range-get.md>
[viewpoint]: <viewpoint.md>

# CommitMarker

The opaque blob committed at one *Viewpoint*[^][viewpoint], stored as a raw
*EnvBuf message*[^][envbuf] in its *Epilogue*[^][epilogue]. An empty message still
has framing, distinguishing it from an *empty sentinel*[^][empty-sentinel].
*Conditional publication*[^][conditional-write] makes it immutable and
first-writer-wins. Its *context*[^][context] is stored as object metadata.
GET capabilities select only its message bytes and convey the signed
*Range headers*[^][range-get].

Its *PromotionSign*[^][promotion-sign] certifies that the entire graph is promoted.
