#!/usr/bin/env bash
# Consolidated evidence runner for the PR #72 review.
# All commands executed are authored literals (echo); no untrusted input is eval'd.
set +e
WT="C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72"
OUT="C:/Users/wilfr/hermes-first-task/revit-alike/docs/agent-team/evidence/repo-tools-review/bim-review/raw"
EV="$LOCALAPPDATA/Temp/pr72-evidence"
mkdir -p "$OUT"

{
echo "### E1 syntax + diff identity"
echo "\$ bash -n gh-ctl"; (cd "$WT" && bash -n gh-ctl); echo "exit=$?"
echo "\$ bash -n automation.sh"; (cd "$WT" && bash -n automation.sh); echo "exit=$?"
echo "\$ git diff --name-only f0dc0b7 799c1f0"; (cd "$WT" && git diff --name-only f0dc0b7 799c1f0); echo "exit=$?"
echo "\$ git diff --stat f0dc0b7 799c1f0"; (cd "$WT" && git diff --stat f0dc0b7 799c1f0); echo "exit=$?"
echo "\$ git rev-parse HEAD"; (cd "$WT" && git rev-parse HEAD); echo "exit=$?"
} > "$OUT/e1-syntax-and-diff.txt" 2>&1

{
echo "### E2 p2 never touches the lock"
TH="$EV/throwaway-p2"; rm -rf "$TH"; mkdir -p "$TH"
echo "\$ REPO_ROOT=\$TH bash gh-ctl p2 'echo p2ran'"
(cd "$WT" && REPO_ROOT="$TH" bash gh-ctl p2 'echo p2ran'); echo "exit=$?"
echo "contents of \$TH/.gh-ctl after p2:"; ls -A "$TH/.gh-ctl" 2>&1
} > "$OUT/e2-p2-parallel.txt" 2>&1

{
echo "### E3 lock + queue (p0 holder drains p1 queue; p2 parallel)"
bash "$OUT/pr72-lock-test.sh"; echo "exit=$?"
} > "$OUT/e3-lock-queue.txt" 2>&1

{
echo "### E4 stale-lock deadlock"
bash "$OUT/pr72-stale-test.sh"; echo "exit=$?"
} > "$OUT/e4-stale-lock.txt" 2>&1

{
echo "### E5 status vs health liveness"
bash "$OUT/pr72-liveness-test.sh"; echo "exit=$?"
} > "$OUT/e5-liveness.txt" 2>&1

{
echo "### E6 subcommands (gate/cargo) + secrets/destructive/eval scan"
bash "$OUT/pr72-subcmd-test.sh"; echo "exit=$?"
} > "$OUT/e6-subcommands.txt" 2>&1

{
echo "### E7 automation.sh role reachability"
bash "$OUT/pr72-sane-test.sh"; echo "exit=$?"
} > "$OUT/e7-automation-roles.txt" 2>&1

echo "evidence written to $OUT"
ls -la "$OUT"
