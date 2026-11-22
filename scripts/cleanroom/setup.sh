#!/usr/bin/env bash
# Create a clean-room workspace that holds only docs/ from a basinsync ref.
#
#   scripts/cleanroom/setup.sh [--ref REF] NAME
#
# The run lives in ${CLEANROOM_ROOT:-~/cleanroom}/NAME:
#   workspace/  fresh git repository: docs/, AGENTS.md brief, CLAUDE.md pointer
#   agent/      per-agent config homes (filled by run.sh)
#   tmp/        TMPDIR inside the sandbox
#   logs/       agent transcripts; never visible inside the sandbox
set -euo pipefail

ref=HEAD
while [ $# -gt 1 ]; do
  case "$1" in
    --ref) ref="$2"; shift 2 ;;
    *) echo "usage: $0 [--ref REF] NAME" >&2; exit 2 ;;
  esac
done
[ $# -eq 1 ] || { echo "usage: $0 [--ref REF] NAME" >&2; exit 2; }
name="$1"
[[ "$name" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "NAME must match [A-Za-z0-9._-]+" >&2; exit 2; }

repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
commit="$(git -C "$repo" rev-parse --verify "$ref^{commit}")"
root="${CLEANROOM_ROOT:-$HOME/cleanroom}"
run="$root/$name"
[ ! -e "$run" ] || { echo "$run already exists" >&2; exit 1; }

mkdir -p "$run/workspace" "$run/agent" "$run/tmp" "$run/logs"
git -C "$repo" archive "$commit" docs | tar -x -C "$run/workspace"
# The brief comes from the same ref, so a rebuild can be repeated exactly.
git -C "$repo" show "$commit:scripts/cleanroom/brief.md" > "$run/workspace/AGENTS.md"
git -C "$repo" show "$commit:scripts/cleanroom/oracle-interface.md" > "$run/workspace/ORACLE-INTERFACE.md"
printf '@AGENTS.md\n' > "$run/workspace/CLAUDE.md"
printf 'target/\nnode_modules/\n.cache/\n' > "$run/workspace/.gitignore"

cd "$run/workspace"
git init -q -b cleanroom/rebuild
git add -A
GIT_AUTHOR_NAME="basinsync clean room" GIT_AUTHOR_EMAIL=cleanroom@localhost \
GIT_COMMITTER_NAME="basinsync clean room" GIT_COMMITTER_EMAIL=cleanroom@localhost \
  git -c commit.gpgsign=false commit -q -m "Clean-room brief and basinsync documentation

Documentation and brief from basinsync $commit."
printf '{"name": "%s", "source": "%s", "created": "%s"}\n' \
  "$name" "$commit" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$run/run.json"
echo "$run"
