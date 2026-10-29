#!/usr/bin/env bash
# Safe coordinated lock/queue test for gh-ctl. Commands are authored literal
# strings (echo), so eval executes only what we wrote. No untrusted input.
EV="$LOCALAPPDATA/Temp/pr72-evidence"
TH=$(cat "$EV/throwaway-path.txt" | tr -d '\r')
WT="C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72"
export REPO_ROOT="$TH"
cd "$WT" || exit 9
echo "[t] START $(date +%H:%M:%S.%N)"
bash gh-ctl p0 'echo HOLDER-START; sleep 2; echo HOLDER-END' &
P0=$!
sleep 0.5
bash gh-ctl p1 'echo QUEUED-A' &
bash gh-ctl p1 'echo QUEUED-B' &
bash gh-ctl p2 'echo P2-PARALLEL'
wait $P0
echo "[t] P0 exited rc=$?"
echo "[t] .gh-ctl after: [$(ls -A "$TH/.gh-ctl")]"
echo "[t] status: $(REPO_ROOT="$TH" bash gh-ctl status | tr '\n' '|')"
echo "[t] DONE"
