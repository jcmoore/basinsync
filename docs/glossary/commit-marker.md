[conditional-write]: <conditional-write.md>
[context]: <context.md>
[promotion-sign]: <promotion-sign.md>
[viewpoint]: <viewpoint.md>

# CommitMarker

The opaque blob committed at one *Viewpoint*[^][viewpoint]. *Conditional creation*[^][conditional-write]
makes it immutable and first-writer-wins. Its *context*[^][context] is stored as object metadata.

Its *PromotionSign*[^][promotion-sign] certifies that the entire graph is promoted.
