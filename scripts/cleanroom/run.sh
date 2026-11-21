#!/usr/bin/env bash
# Run an agent inside a clean room created by setup.sh, sandboxed with srt.
#
#   scripts/cleanroom/run.sh [--agent claude|codex] [--check] [--continue] NAME
#   scripts/cleanroom/run.sh --command 'CMD' NAME
#
# The sandbox can read and write only the workspace, the agent's own config
# home, its TMPDIR, and the Quint cache; toolchains are read-only; everything
# else under $HOME, and this user's Claude scratch space under /tmp, is hidden.
# Network is limited to the agent's API, package registries, and tool docs.
# Every run first probes the sandbox and refuses to start if a check fails.
#
# Credentials are passed through the environment, never copied from your own
# agent config: claude needs CLAUDE_CODE_OAUTH_TOKEN (from `claude
# setup-token`) or ANTHROPIC_API_KEY; codex needs OPENAI_API_KEY.
set -euo pipefail

agent=claude check_only=0 resume=0 command=""
while [ $# -gt 1 ]; do
  case "$1" in
    --agent) agent="$2"; shift 2 ;;
    --command) agent=custom command="$2"; shift 2 ;;
    --check) check_only=1; shift ;;
    --continue) resume=1; shift ;;
    *) break ;;
  esac
done
[ $# -eq 1 ] || { sed -n '2,6p' "$0" >&2; exit 2; }
run="${CLEANROOM_ROOT:-$HOME/cleanroom}/$1"
ws="$run/workspace"
[ -d "$ws/.git" ] || { echo "no clean room at $run; run setup.sh first" >&2; exit 1; }

for tool in srt bwrap socat rg git cargo quint java node bun go; do
  command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 1; }
done
cargo kani --version >/dev/null 2>&1 || { echo "missing tool: cargo kani" >&2; exit 1; }

home="$HOME"
mkdir -p "$run/agent/$agent" "$run/tmp" "$run/logs" "$run/cache" "$home/.quint"
settings="$run/srt-settings.json"
SRT_HOME="$home" SRT_RUN="$run" SRT_AGENT="$agent" SRT_UID="$(id -u)" python3 -I - > "$settings" <<'EOF'
import json, os
home, run, agent = os.environ["SRT_HOME"], os.environ["SRT_RUN"], os.environ["SRT_AGENT"]
toolchains = [".cargo/bin", ".rustup", ".kani", ".quint", ".bun/bin", ".local/bin",
              ".local/node", ".local/jdk", ".local/go", ".local/share/claude",
              ".local/share/codex-cli"]
domains = {
    "claude": ["api.anthropic.com", "*.anthropic.com", "claude.ai", "*.claude.ai", "*.claude.com"],
    "codex": ["api.openai.com", "*.openai.com", "chatgpt.com", "*.chatgpt.com"],
    "custom": [],
}[agent]
print(json.dumps({
    "filesystem": {
        "denyRead": ["/home", "/root", f"/tmp/claude-{os.environ['SRT_UID']}"],
        "allowRead": [f"{run}/workspace", f"{run}/agent/{agent}", f"{run}/tmp", f"{run}/cache"]
                     + [f"{home}/{path}" for path in toolchains],
        "allowWrite": [f"{run}/workspace", f"{run}/agent/{agent}", f"{run}/tmp", f"{run}/cache",
                       f"{home}/.quint"],
    },
    "network": {
        "allowedDomains": domains + [
            "crates.io", "*.crates.io", "registry.npmjs.org", "proxy.golang.org",
            "sum.golang.org", "docs.rs", "doc.rust-lang.org", "quint.sh",
            "model-checking.github.io"],
        "deniedDomains": [],
    },
}, indent=2))
EOF

# A curated environment: nothing from this shell leaks in except what is listed.
envs=(
  "HOME=$home" "TMPDIR=$run/tmp" "TERM=${TERM:-dumb}" "LANG=${LANG:-C.UTF-8}"
  "PATH=$home/.local/bin:$home/.cargo/bin:$home/.bun/bin:$home/.local/go/bin:/usr/local/bin:/usr/bin:/bin"
  "RUSTUP_HOME=$home/.rustup" "CARGO_HOME=$run/cache/cargo"
  "GOPATH=$run/cache/go" "GOCACHE=$run/cache/go-build" "GOFLAGS=-modcacherw"
  "npm_config_cache=$run/cache/npm" "BUN_INSTALL_CACHE_DIR=$run/cache/bun"
  "XDG_CACHE_HOME=$run/cache/xdg" "XDG_CONFIG_HOME=$run/agent/$agent/config"
  "GIT_AUTHOR_NAME=basinsync clean room" "GIT_AUTHOR_EMAIL=cleanroom@localhost"
  "GIT_COMMITTER_NAME=basinsync clean room" "GIT_COMMITTER_EMAIL=cleanroom@localhost"
  "GIT_CONFIG_GLOBAL=/dev/null"
)
extra=()
in_sandbox() { (cd "$ws" && env -i "${envs[@]}" "${extra[@]}" srt --settings "$settings" "$@"); }

# Probe the sandbox before trusting it with an agent.
probe='set -u; fail=0
for path in "$HOME/git" "$HOME/.claude" "$HOME/.ssh" "$HOME/.local/share/keyrings" "/tmp/claude-$(id -u)"; do
  if ls "$path" >/dev/null 2>&1 && [ -n "$(ls -A "$path" 2>/dev/null)" ]; then echo "LEAK: $path is readable"; fail=1; fi
done
touch .cleanroom-probe && rm .cleanroom-probe || { echo "workspace is not writable"; fail=1; }
if curl -sS -m 10 -o /dev/null https://github.com 2>/dev/null; then echo "LEAK: github.com is reachable"; fail=1; fi
curl -sS -m 10 -o /dev/null https://index.crates.io/config.json || { echo "crates.io is unreachable"; fail=1; }
exit $fail'
if ! in_sandbox bash -c "$probe"; then
  echo "sandbox probe failed; refusing to run" >&2; exit 1
fi
echo "sandbox probe passed"
[ "$check_only" -eq 0 ] || exit 0

prompt="Read AGENTS.md and carry out the clean-room rebuild it describes. Work until every deliverable is done or blocked, committing as you go."
log="$run/logs/$agent-$(date -u +%Y%m%dT%H%M%SZ).jsonl"
case "$agent" in
  claude)
    [ -n "${CLAUDE_CODE_OAUTH_TOKEN:-}${ANTHROPIC_API_KEY:-}" ] ||
      { echo "set CLAUDE_CODE_OAUTH_TOKEN (claude setup-token) or ANTHROPIC_API_KEY" >&2; exit 1; }
    extra=("CLAUDE_CONFIG_DIR=$run/agent/claude")
    [ -z "${CLAUDE_CODE_OAUTH_TOKEN:-}" ] || extra+=("CLAUDE_CODE_OAUTH_TOKEN=$CLAUDE_CODE_OAUTH_TOKEN")
    [ -z "${ANTHROPIC_API_KEY:-}" ] || extra+=("ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY")
    flags=(-p "$prompt" --dangerously-skip-permissions --output-format stream-json --verbose)
    [ "$resume" -eq 0 ] || flags=(--continue "${flags[@]}")
    in_sandbox claude "${flags[@]}" > "$log" ;;
  codex)
    [ -n "${OPENAI_API_KEY:-}" ] || { echo "set OPENAI_API_KEY" >&2; exit 1; }
    extra=("CODEX_HOME=$run/agent/codex" "OPENAI_API_KEY=$OPENAI_API_KEY")
    in_sandbox codex exec --json --dangerously-bypass-approvals-and-sandbox "$prompt" > "$log" ;;
  custom)
    in_sandbox bash -c "$command" > "$log" ;;
  *) echo "unknown agent: $agent" >&2; exit 2 ;;
esac
echo "agent finished; transcript: $log"
