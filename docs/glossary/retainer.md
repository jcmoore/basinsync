[commit-marker]: <commit-marker.md>
[flow]: <flow.md>
[heap-reservation]: <heap-reservation.md>
[node]: <node.md>
[object-keys]: <../object-keys.md>
[viewpoint]: <viewpoint.md>

# Retainer

The source of an incoming edge to a *node*[~][node]: another `n-` *node*[~][node],
a *Flow*[~][flow] *Viewpoint's*[~][viewpoint] `_a_` staging area for
*reservations*[~][heap-reservation], or its `_m_` *CommitMarker*[~][commit-marker].
See the *retainer layouts*[~][object-keys].

Require `retainer < node`; `_a_`/`_m_` precede `n-`.
Reserve `_r_` for future retainers.
