#!/usr/bin/env bash
# Run a hidden oracle against a finished clean room. The clean room's oracle
# CLI (oracle-interface.md) runs inside the same sandbox as the agent did; it
# only ever sees requests, never this repository's expected results.
#
#   scripts/cleanroom/oracle.sh vectors [--cli 'CMD'] NAME
#
# CMD runs from the clean room's workspace and is remembered in
# eval/cli for later runs, e.g. 'cargo run -q --release -p some-cli'.
set -euo pipefail
[ "${1:-}" = vectors ] || { sed -n '5,9p' "$0" >&2; exit 2; }
shift
cli=""
[ "${1:-}" != --cli ] || { cli="$2"; shift 2; }
[ $# -eq 1 ] || { sed -n '5,9p' "$0" >&2; exit 2; }
name="$1" here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
eval_dir="${CLEANROOM_ROOT:-$HOME/cleanroom}/$name/eval"
mkdir -p "$eval_dir/vectors"
[ -z "$cli" ] || printf '%s\n' "$cli" > "$eval_dir/cli"
[ -s "$eval_dir/cli" ] || { echo "no oracle CLI recorded; pass --cli 'CMD'" >&2; exit 1; }
python3 -I "$here/oracles/vectors.py" make "$repo" "$eval_dir/vectors"
rm -f "$eval_dir/vectors/responses.jsonl"
"$here/run.sh" --foreground --command \
  "( $(cat "$eval_dir/cli") ) < '$eval_dir/vectors/requests.jsonl' > '$eval_dir/vectors/responses.jsonl'" \
  "$name" >/dev/null
python3 -I "$here/oracles/vectors.py" check "$eval_dir/vectors" | tee "$eval_dir/vectors/report.txt"
