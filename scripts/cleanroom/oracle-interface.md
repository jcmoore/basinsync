# Oracle interface

Provide a command-line program that independent checks can drive. It reads
one JSON request per line on standard input and writes exactly one JSON
response per line on standard output, in order, then exits at end of input.

A response is `{"ok": RESULT}` on success, `{"error": "MESSAGE"}` when the
input is invalid or rejected, or `{"unsupported": true}` for an operation your
build does not implement. Never exit early on bad input.

Integers travel as decimal strings (they may exceed 64 bits); bytes travel as
arrays of numbers 0–255.

| `op` | Request fields | `ok` result |
|---|---|---|
| `lexidecimal.encode` | `value` | encoded text |
| `lexidecimal.decode` | `text` | decimal string |
| `lexidecimal.sizePrefix` | `size` (number), `negative` (bool) | size prefix text |
| `diffuse16.encode` | `value` | encoded text |
| `diffuse16.decode` | `text` | decimal string |
| `envbuf.encode` | `format`, `alignmentPower` (numbers), `content` (bytes) | envelope bytes |
| `envbuf.decode` | `wire` (bytes: one envelope) | `{"format", "content"}` |
| `flexbuffers.encodeExtension` | exactly one of `bigint`, `wellKnownSymbol`, `globalSymbol` | FlexBuffers bytes |
| `flexbuffers.decodeExtension` | `wire` (bytes) | object with the one matching field |
| `timeline.key` | `milestone` | object key |
| `timeline.milestone` | `key` | decimal string |
| `flow.normalize` | `id` | canonical Flow ID |
| `keys.prefixes` | none | object naming your object-key prefixes: `timeline`, `basinEpilogue`, `analogue`, `kernelHeap`, `userHeap`, `userFlows` (each ending in `/`), and `emptySentinel` (the full key) |

Name the program in `README.md` and make it buildable with your workspace's
normal build command.
