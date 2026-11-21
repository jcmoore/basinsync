#!/usr/bin/env bash
# Show a clean room's state: unit, memory, transcript activity, commits, the
# agent's STATUS.md and blockers, recent actions, and recent errors.
#
#   scripts/cleanroom/status.sh [--brief] NAME
set -euo pipefail
brief=0
[ "${1:-}" != --brief ] || { brief=1; shift; }
[ $# -eq 1 ] || { echo "usage: $0 [--brief] NAME" >&2; exit 2; }
run="${CLEANROOM_ROOT:-$HOME/cleanroom}/$1"
[ -d "$run/workspace/.git" ] || { echo "no clean room at $run" >&2; exit 1; }
unit="cleanroom-$1.service"
systemd=""
if command -v systemctl >/dev/null; then
  systemd="$(systemctl --user show "$unit" -p ActiveState -p SubState -p Result \
    -p ExecMainStatus -p MemoryCurrent -p MemoryPeak -p MemoryMax \
    -p ActiveEnterTimestamp 2>/dev/null || true)"
fi
ooms="$(journalctl --user -u "$unit" --no-pager -q 2>/dev/null | grep -ci 'killed by the OOM killer' || true)"
RUN="$run" SYSTEMD="$systemd" OOMS="$ooms" BRIEF="$brief" python3 -I - <<'EOF'
import json, os, subprocess, time
run, brief = os.environ["RUN"], os.environ["BRIEF"] == "1"
ws = f"{run}/workspace"
unit = dict(line.split("=", 1) for line in os.environ["SYSTEMD"].splitlines() if "=" in line)
def size(value):
    try: return f"{int(value) / 2**30:.1f}G"
    except ValueError: return "-"
try: status = json.load(open(f"{run}/status.json"))
except (OSError, ValueError): status = {}
state = status.get("state", "unknown")
if unit.get("ActiveState") in ("active", "activating") and state != "running":
    state = "running"
if state == "running" and unit and unit.get("ActiveState") not in ("active", "activating"):
    state = f"stopped ({unit.get('Result', '?')})"
print(f"state: {state}" + (f", exit {status['exit']}" if "exit" in status else ""))
if unit:
    print(f"unit: {unit.get('ActiveState')}/{unit.get('SubState')} since "
          f"{unit.get('ActiveEnterTimestamp') or '-'}; memory {size(unit.get('MemoryCurrent', ''))}"
          f" (peak {size(unit.get('MemoryPeak', ''))}, cap {size(unit.get('MemoryMax', ''))});"
          f" OOM kills {os.environ['OOMS']}")
log = status.get("transcript", "")
events, actions, errors = 0, [], []
if log and os.path.exists(log):
    idle = time.time() - os.path.getmtime(log)
    for line in open(log, errors="replace"):
        try: event = json.loads(line)
        except ValueError: continue
        events += 1
        content = (event.get("message") or {}).get("content") or []
        if not isinstance(content, list): continue
        for part in content:
            if part.get("type") == "tool_use":
                actions.append(f"{part['name']}: {str(part.get('input'))[:110]}")
            elif part.get("type") == "text" and event.get("type") == "assistant":
                actions.append("says: " + part["text"][:110])
            elif part.get("type") == "tool_result" and part.get("is_error"):
                errors.append(str(part.get("content"))[:160])
        if event.get("type") == "result":
            actions.append(f"result: {event.get('subtype')} {str(event.get('result'))[:110]}")
    print(f"transcript: {events} events, last write {int(idle // 60)} min ago"
          + ("  <- possible stall" if state == "running" and idle > 1200 else ""))
def git(*args):
    return subprocess.run(["git", "-C", ws, *args], capture_output=True, text=True).stdout.strip()
print(f"commits: {git('rev-list', '--count', 'HEAD')}; last: {git('log', '-1', '--format=%cr: %s')};"
      f" uncommitted files: {len(git('status', '--short').splitlines())}")
try: notes = open(f"{ws}/STATUS.md").read().splitlines()
except OSError: notes = []
blockers = [line for line in notes if "BLOCKED" in line]
for line in blockers: print(f"BLOCKER: {line.strip()}")
if brief: raise SystemExit
print("--- STATUS.md" if notes else "--- no STATUS.md yet")
print("\n".join(notes[:25]))
print("--- recent actions"); print("\n".join(actions[-10:]) or "(none)")
print("--- recent tool errors"); print("\n".join(errors[-5:]) or "(none)")
EOF
