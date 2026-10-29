#!/usr/bin/env bash
# A1/A5: how does gh-ctl handle an UNRECOGNIZED tier? rank() returns 99 for
# unknown values, and `99 -gt 1` sends it down the p2 (unlocked, parallel) path.
EV="$LOCALAPPDATA/Temp/pr72-evidence"
TH="$EV/throwaway-badprio"
WT="C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72"
rm -rf "$TH"; mkdir -p "$TH"
cd "$WT" || exit 9
export REPO_ROOT="$TH"
echo "--- gh-ctl rank() ---"; sed -n '35p' "$WT/gh-ctl"
echo "--- gh-ctl p2 guard ---"; sed -n '179p' "$WT/gh-ctl"
echo "=== gh-ctl P0 'echo CAPITAL-P0' (uppercase typo) ==="
bash gh-ctl P0 'echo CAPITAL-P0'; echo "exit=$?"
echo "=== gh-ctl p0x 'echo TYPO' ==="
bash gh-ctl p0x 'echo TYPO'; echo "exit=$?"
echo "=== gh-ctl p3 'echo THREE' ==="
bash gh-ctl p3 'echo THREE'; echo "exit=$?"
echo "=== gh-ctl p1 with a lock held by a live holder (should queue, not run) ==="
mkdir "$TH/.gh-ctl"; mkdir "$TH/.gh-ctl/lock"; echo $$ > "$TH/.gh-ctl/lock.pid"
bash gh-ctl p1 'echo QUEUED-P1'; echo "exit=$?"
echo "--- queue ---"; cat "$TH/.gh-ctl/queue" 2>&1
echo "=== gh-ctl p3 with the SAME lock held (does it queue or run unlocked?) ==="
bash gh-ctl p3 'echo P3-UNLOCKED'; echo "exit=$?"
echo "--- queue after p3 ---"; cat "$TH/.gh-ctl/queue" 2>&1
echo "--- did P3-UNLOCKED run? (it printed above if so) ---"
