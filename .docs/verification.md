[epilogue-compaction]: <../docs/glossary/epilogue-compaction.md>
[promotion-sign]: <../docs/glossary/promotion-sign.md>
[tombstone]: <../docs/glossary/tombstone.md>
[verus]: <https://github.com/verus-lang/verus>

# Verification notes

Tests and Quint models check behavior; neither proves the Rust implementation.
Keep these options in mind as the riskiest pieces arrive.

## Verus for Epilogue compaction

When implementing *Epilogue compaction*[~][epilogue-compaction], consider
*Verus*[~][verus] to prove the implementation refines a small state machine,
in the IronFleet style, rather than only testing it. Compaction is the first
operation that destroys data, and its safety rests on interleavings that tests
sample poorly:

- compaction never replaces a CommitMarker at or after the retained Viewpoint;
- a *PromotionSign*[~][promotion-sign] whose marker became a
  *tombstone*[~][tombstone] is always removed, whether compaction or publication
  wins the race;
- LISTers never select a snapshot whose marker was compacted.

Verus requires writing the verified core in its Rust dialect with proof
annotations, so scope it to the compaction planner and the sign/tombstone
protocol, behind a narrow interface to the rest of the server. Model the same
protocol in Quint first; the Quint invariants become the Verus specification.
