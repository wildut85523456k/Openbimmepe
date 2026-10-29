#!/usr/bin/env bash
# A2: do `status` (kill -0) and `health` (/proc/$PID) agree on lock liveness?
# Fabricates a held lock with (a) a live PID (this shell) and (b) a dead PID.
TH="$LOCALAPPDATA/Temp/pr72-evidence/throwaway-liveness"
WT="C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72"
rm -rf "$TH"; mkdir -p "$TH/.gh-ctl"
cd "$WT" || exit 9
export REPO_ROOT="$TH"
mkdir "$TH/.gh-ctl/lock"
echo $$ > "$TH/.gh-ctl/lock.pid"
echo "=== status with live PID $$ ==="
bash gh-ctl status; echo "status_exit=$?"
echo "=== health with live PID $$ ==="
bash gh-ctl health; echo "health_exit=$?"
echo 999999 > "$TH/.gh-ctl/lock.pid"
echo "=== status with dead PID 999999 ==="
bash gh-ctl status; echo "status_exit=$?"
echo "=== health with dead PID 999999 ==="
bash gh-ctl health; echo "health_exit=$?"
rm -rf "$TH/.gh-ctl/lock" "$TH/.gh-ctl/lock.pid"
echo "cleaned"
