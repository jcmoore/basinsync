[milestone]: <milestone.md>
[twos-complement]: <twos-complement.md>
[viewpoint]: <viewpoint.md>

# Lexidecimal

A doubly size-prefixed, big-endian radix-16 text encoding of integers.
Lexicographic order follows numeric order, from lowest negative to highest positive.

For a nonnegative magnitude, form this body using minimal lowercase hex digits:

```text
h = hex(magnitude)                // zero is "0"
s = hex(length(h) - 1)
w = hex(length(s) - 1)            // one digit
body = w + s + h
```

Encode `n >= 0` as `0z + body(n)`. Encode `n < 0` as
`0y + complement(body(~n))`, replacing each body digit `d` with `15-d`.

Decode by reversing the complement when present, reading the two length prefixes,
and parsing the magnitude; complement the resulting integer for `0y`.
Canonical spellings reproduce themselves when re-encoded.

Examples: `0 → 0z000`, `16 → 0z0110`, `-1 → 0yfff`, `-17 → 0yfeef`.
Encoding *complemented*[~][twos-complement] *Viewpoints*[~][viewpoint] or
*Milestones*[~][milestone] reverses sequence order, so the minimum published
object key identifies the latest position.
