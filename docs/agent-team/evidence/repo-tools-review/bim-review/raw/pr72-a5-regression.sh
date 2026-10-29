#!/usr/bin/env bash
# A5 regression check for the fixed gh-ctl (PR #72, candidate 23f2df4).
# Exercises the paths the fix could have broken. Read-only w.r.t. the real repo:
# every job runs against a throwaway REPO_ROOT.
set -u

SRC="${1:?usage: pr72-a5-regression.sh <path-to-fixed-gh-ctl>}"
T="$(mktemp -d)"
cp "$SRC" "$T/gh-ctl"
GH="$T/gh-ctl"

pass=0; fail=0
ok()   { echo "  PASS  $1"; pass=$((pass+1)); }
bad()  { echo "  FAIL  $1"; fail=$((fail+1)); }

echo "=== A5.1 p2 runs parallel (no lock) ==="
out="$(REPO_ROOT="$T" bash "$GH" p2 "echo P2-RAN" 2>&1)"; rc=$?
echo "$out" | sed 's/^/    /'
[ "$rc" -eq 0 ] && echo "$out" | grep -q 'P2-RAN' && ok "p2 ran (exit 0)" || bad "p2 did not run"
[ ! -e "$T/.gh-ctl/lock" ] && ok "p2 left no lock" || bad "p2 created a lock"

echo "=== A5.2 p0/p1 serialize + queue drain (priority order) ==="
# Hold the lock with a background p0 that sleeps, then enqueue p1 and p0 jobs.
REPO_ROOT="$T" bash "$GH" p0 "sleep 3; echo HOLDER-DONE" > "$T/holder.log" 2>&1 &
HPID=$!
sleep 0.6
q1="$(REPO_ROOT="$T" bash "$GH" p1 "echo P1-QUEUED" 2>&1)"; rc1=$?
q0="$(REPO_ROOT="$T" bash "$GH" p0 "echo P0-QUEUED" 2>&1)"; rc0=$?
echo "$q1" | sed 's/^/    /'; echo "$q0" | sed 's/^/    /'
[ "$rc1" -eq 0 ] && echo "$q1" | grep -q 'queued' && ok "p1 enqueued while held" || bad "p1 not enqueued"
[ "$rc0" -eq 0 ] && echo "$q0" | grep -q 'queued' && ok "p0 enqueued while held" || bad "p0 not enqueued"
wait "$HPID"
drain="$(cat "$T/holder.log")"
echo "$drain" | sed 's/^/    /'
echo "$drain" | grep -q 'P0-QUEUED' && echo "$drain" | grep -q 'P1-QUEUED' && ok "holder drained the queue" || bad "queue not drained"
# p0 (rank 0) must appear before p1 (rank 1) in the drain output
if echo "$drain" | grep -q 'P0-QUEUED' && echo "$drain" | grep -q 'P1-QUEUED'; then
  p0line=$(echo "$drain" | grep -n 'P0-QUEUED' | head -1 | cut -d: -f1)
  p1line=$(echo "$drain" | grep -n 'P1-QUEUED' | head -1 | cut -d: -f1)
  [ "$p0line" -lt "$p1line" ] && ok "drained in priority order (p0 before p1)" || bad "drain order wrong (p0=$p0line p1=$p1line)"
fi
[ ! -e "$T/.gh-ctl/lock" ] && ok "lock released after the holder finished" || bad "lock left behind"

echo "=== A5.3 status ==="
st="$(REPO_ROOT="$T" bash "$GH" status 2>&1)"; rcs=$?
echo "$st" | sed 's/^/    /'
[ "$rcs" -eq 0 ] && echo "$st" | grep -q 'LOCK:' && echo "$st" | grep -q 'QUEUE:' && ok "status reports lock + queue" || bad "status malformed"

echo "=== A5.4 health (throwaway dir -> REPO FAIL, exit 1) ==="
hl="$(REPO_ROOT="$T" bash "$GH" health 2>&1)"; rch=$?
echo "$hl" | sed 's/^/    /'
echo "$hl" | grep -q 'ENVIRONMENT:' && ok "health prints a verdict" || bad "health silent"
[ "$rch" -eq 1 ] && ok "health exit 1 on a non-git dir" || bad "health exit $rch (expected 1)"

echo "=== A5.5 gate usage + no-args exit codes ==="
gu="$(REPO_ROOT="$T" bash "$GH" gate 2>&1)"; rcg=$?
echo "$gu" | sed 's/^/    /'
[ "$rcg" -eq 2 ] && ok "gate with no target -> usage exit 2" || bad "gate no-args exit $rcg (expected 2)"

echo "=== A5.6 cargo lock acquire/release ==="
cl="$(REPO_ROOT="$T" CARGO_TIMEOUT=5 bash "$GH" cargo "echo CARGO-RAN" 2>&1)"; rcc=$?
echo "$cl" | sed 's/^/    /'
[ "$rcc" -eq 0 ] && echo "$cl" | grep -q 'CARGO-RAN' && ok "cargo ran" || bad "cargo did not run"
[ ! -e "$T/.gh-ctl/cargo.lock" ] && ok "cargo lock released" || bad "cargo lock left behind"

echo
echo "=== RESULT: $pass passed, $fail failed ==="
rm -rf "$T" 2>/dev/null
[ "$fail" -eq 0 ]
