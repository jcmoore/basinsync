#!/usr/bin/env bash
# Run Kani harnesses sequentially (one model checker at a time), each under a
# timeout and a virtual-memory cap; append one result line per harness.
# Usage: [HARNESSES="a b"] [MEM_KB=16000000] scripts/kani-all.sh [TIMEOUT_SECS] [LOG]
set -u
cd "$(dirname "$0")/.."
T=${1:-900}; LOG=${2:-.local/kani-results.txt}; MEM_KB=${MEM_KB:-16000000}
LIST=${HARNESSES:-$(grep -A3 "kani::proof" crates/codec/src/kani_proofs.rs | grep -oE 'fn [a-z0-9_]+' | awk '{print $2}')}
: > "$LOG"
for h in $LIST; do
  out=$( { /usr/bin/time -f "TIME %e s %M KB" bash -c "ulimit -v $MEM_KB; exec timeout $T cargo kani -p basinsync-codec --harness $h"; } 2>&1 )
  rc=$?
  echo "$h rc=$rc $(echo "$out" | grep -E '^VERIFICATION' | tail -1) $(echo "$out" | grep -E '^TIME') $(echo "$out" | grep -oE 'Complete - [0-9]+ successfully verified harnesses, [0-9]+ failures' | tail -1)" >> "$LOG"
  echo "$out" | grep -E "out of memory|Failed Checks|^error|memory allocation" | head -3 >> "$LOG"
done
echo DONE >> "$LOG"
