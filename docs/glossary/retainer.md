[commit-marker]: <commit-marker.md>
[flow]: <flow.md>
[lexidecimal]: <lexidecimal.md>
[node]: <node.md>
[userheap-reservation]: <userheap-reservation.md>
[viewpoint]: <viewpoint.md>

# Retainer

The source of an incoming edge to a *node*[^][node]: another `n-` *node*[^][node],
a *Flow*[^][flow] *Viewpoint's*[^][viewpoint] `A_` staging area for
*reservations*[^][userheap-reservation], or its `M_` *CommitMarker*[^][commit-marker].
With *Lexidecimal*[^][lexidecimal] `lex`, the latter identifiers are:

```text
A_${flow}/v.${lex(version)}/e.${lex(viewpoint)}
M_${flow}/v.${lex(version)}/e.${lex(viewpoint)}
```

Require `retainer < node`; uppercase `A_`/`M_` precede lowercase `n-`.
