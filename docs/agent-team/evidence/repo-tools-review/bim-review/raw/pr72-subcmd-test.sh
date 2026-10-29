#!/usr/bin/env bash
# A1/A2/A5: exercise gate + cargo subcommands safely on a throwaway REPO_ROOT.
EV="$LOCALAPPDATA/Temp/pr72-evidence"
TH="$EV/throwaway-sane"
WT="C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72"
export REPO_ROOT="$TH"
cd "$WT" || exit 9

echo "=== gate with no args ==="
bash gh-ctl gate; echo "GATE_NOARG_EXIT=$?"
echo "=== gate target-x 'echo gated' ==="
bash gh-ctl gate target-x 'echo gated-ran'; echo "GATE_EXIT=$?"
echo "=== gate held-lock path (simulate contention) ==="
mkdir -p "$TH/target-y"
bash gh-ctl gate target-y 'echo gated-y'; echo "GATE_HELD_EXIT=$?"
rm -rf "$TH/target-y" "$TH/target-y.pid"

echo "=== cargo 'echo cargorun' ==="
CARGO_TIMEOUT=5 bash gh-ctl cargo 'echo cargorun'; echo "CARGO_EXIT=$?"
echo "=== cargo with queued job ==="
printf '0\techo queued-cargo-ran\n' > "$TH/.gh-ctl/cargo.queue"
CARGO_TIMEOUT=5 bash gh-ctl cargo 'echo cargorun2'; echo "CARGO_Q_EXIT=$?"
echo "=== gate lock pid file content check ==="
ls -A "$TH/.gh-ctl" 2>&1
echo "--- secrets scan (both scripts) ---"
grep -nEi 'token|secret|password|passwd|api[_-]?key|ghp_|github_pat_|BEGIN [A-Z ]*PRIVATE KEY' "$WT/gh-ctl" "$WT/automation.sh" || echo "no secret-like strings found"
echo "--- destructive-command scan ---"
grep -nE 'rm -rf|rm -r |git push|force|--force|reset --hard|clean -[a-z]*f|chmod -R|mv ' "$WT/gh-ctl" "$WT/automation.sh" || echo "none matched"
echo "--- eval sites ---"
grep -n 'eval' "$WT/gh-ctl" "$WT/automation.sh"
echo "--- stat/proc/sort constructs ---"
grep -n "stat -c\|/proc/\|sort -t" "$WT/gh-ctl" "$WT/automation.sh"
