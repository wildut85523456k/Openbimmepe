#!/usr/bin/env bash
# A1/A2: does a stale lock (holder died without cleanup) ever get recovered?
# If not, p0/p1 jobs enqueue forever and nothing drains -> deadlock.
EV="$LOCALAPPDATA/Temp/pr72-evidence"
TH="$EV/throwaway-stale"
WT="C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72"
rm -rf "$TH"; mkdir -p "$TH/.gh-ctl"
cd "$WT" || exit 9
export REPO_ROOT="$TH"
# Fabricate a lock whose recorded owner is dead (simulates SIGKILL / crash)
mkdir "$TH/.gh-ctl/lock"
echo 999999 > "$TH/.gh-ctl/lock.pid"
echo "=== status (stale lock) ==="
bash gh-ctl status; echo "status_exit=$?"
echo "=== health (stale lock) ==="
bash gh-ctl health; echo "health_exit=$?"
echo "=== gh-ctl p0 'echo SHOULD-RUN' (stale lock held) ==="
timeout 20 bash gh-ctl p0 'echo SHOULD-RUN'; echo "P0_EXIT=$?"
echo "=== did SHOULD-RUN execute? ==="
echo "=== queue now ==="
cat "$TH/.gh-ctl/queue" 2>&1
echo "=== lock still present? ==="
ls -A "$TH/.gh-ctl"
echo "=== a second p0 job ==="
timeout 20 bash gh-ctl p0 'echo SECOND-RUN'; echo "P0B_EXIT=$?"
echo "=== queue after 2nd ==="
cat "$TH/.gh-ctl/queue" 2>&1
echo "=== lock dir contents ==="
ls -A "$TH/.gh-ctl"
