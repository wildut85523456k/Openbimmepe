#!/usr/bin/env bash
# PR #72 re-review regression driver (candidate 23f2df4).
# Authored literals only. Throwaway REPO_ROOT under $TMPDIR. No untrusted eval.
WT=C:/Users/wilfr/hermes-first-task/revit-alike-tools
T="$TMPDIR/pr72-r2"
R="$T/reg"
rm -rf "$R"; mkdir -p "$R"
cp "$WT/gh-ctl" "$R/gh-ctl"

echo "=== A5a: p2 never touches the lock ==="
REPO_ROOT="$R" bash "$WT/gh-ctl" p2 "echo P2-ONE"; echo "p2_exit=$?"
echo ".gh-ctl entries after p2: $(ls -A "$R/.gh-ctl" 2>/dev/null | wc -l) (expect 0)"

echo
echo "=== A5b: p0/p1 lock + priority-ordered queue drain ==="
REPO_ROOT="$R" bash "$WT/gh-ctl" p0 "sleep 4; echo HOLDER-DONE" &
HOLDER=$!
sleep 2
echo "--- status while holder active ---"
REPO_ROOT="$R" bash "$WT/gh-ctl" status
echo "--- queue a p1 then a p0 ---"
REPO_ROOT="$R" bash "$WT/gh-ctl" p1 "echo QUEUED-A"; echo "p1_exit=$?"
REPO_ROOT="$R" bash "$WT/gh-ctl" p0 "echo QUEUED-B"; echo "p0_exit=$?"
echo "--- queue contents while held ---"
REPO_ROOT="$R" bash "$WT/gh-ctl" status
wait "$HOLDER"; echo "holder_exit=$?"
echo "--- status after drain ---"
REPO_ROOT="$R" bash "$WT/gh-ctl" status

echo
echo "=== A5c: cargo lock + queue ==="
REPO_ROOT="$R" bash "$WT/gh-ctl" cargo "echo CARGO-ONE"; echo "cargo1_exit=$?"
echo ".gh-ctl entries after cargo: $(ls -A "$R/.gh-ctl" 2>/dev/null | wc -l) (expect 0)"

echo
echo "=== A5d: gate usage/exit codes ==="
REPO_ROOT="$R" bash "$WT/gh-ctl" gate; echo "gate_noargs_exit=$? (expect 2)"
REPO_ROOT="$R" bash "$WT/gh-ctl" gate t1 "echo G1"; echo "gate_t1_exit=$?"
REPO_ROOT="$R" bash "$WT/gh-ctl" gate t1 "echo G2"; echo "gate_t1_second_exit=$?"
echo "t1 still a dir? $([ -d "$R/t1" ] && echo YES || echo NO)"

echo
echo "=== A5e: status/health exit codes (free lock) ==="
REPO_ROOT="$R" bash "$WT/gh-ctl" status >/dev/null; echo "status_exit=$?"
REPO_ROOT="$R" bash "$WT/gh-ctl" health; echo "health_exit=$? (expect 1: REPO FAIL on non-git throwaway)"

echo
echo "=== A5f: p0 honors a failing queued job (non-zero propagation) ==="
REPO_ROOT="$R" bash "$WT/gh-ctl" p0 "sleep 4; echo HOLDER2-DONE" &
H2=$!
sleep 2
REPO_ROOT="$R" bash "$WT/gh-ctl" p1 "exit 7"; echo "queued_fail_exit=$?"
wait "$H2"; echo "holder2_exit=$? (expect 1: a queued job returned non-zero)"
REPO_ROOT="$R" bash "$WT/gh-ctl" status
