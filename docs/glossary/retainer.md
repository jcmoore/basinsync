[commit-marker]: <commit-marker.md>
[flow]: <flow.md>
[lexidecimal]: <lexidecimal.md>
[node]: <node.md>
[userheap-reservation]: <userheap-reservation.md>
[viewpoint]: <viewpoint.md>

# Retainer

The source of an incoming edge to a *node*[^][node]: another `n-` *node*[^][node],
a *Flow*[^][flow] *Viewpoint's*[^][viewpoint] `_a_` staging area for
*reservations*[^][userheap-reservation], or its `_m_` *CommitMarker*[^][commit-marker].
With *Lexidecimal*[^][lexidecimal] `lex`, the latter identifiers are:

```text
_a_${flow}/v.${lex(version)}/e.${lex(viewpoint)}
_m_${flow}/v.${lex(version)}/e.${lex(viewpoint)}
```

Require `retainer < node`; `_a_`/`_m_` precede `n-`.
Reserve `_r_` for future retainers.
