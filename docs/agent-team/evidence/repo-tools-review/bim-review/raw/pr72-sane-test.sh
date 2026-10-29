#!/usr/bin/env bash
# A3: is the `cargo` role reachable through automation.sh's parser?
# Build a Sane throwaway repo (real git repo + gh-ctl copy), then try the
# cargo role. Commands are authored literals (echo) — no untrusted input.
EV="$LOCALAPPDATA/Temp/pr72-evidence"
TH="$EV/throwaway-sane"
rm -rf "$TH"; mkdir -p "$TH"
cd "$TH" || exit 9
git init -q .
git -C "$TH" config user.email t@t; git -C "$TH" config user.name t
cp "C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72/gh-ctl" "$TH/gh-ctl"
cp "C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72/automation.sh" "$TH/automation.sh"
echo "=== health (throwaway-sane) ==="
REPO_ROOT="$TH" bash "$TH/gh-ctl" health; echo "HEALTH_EXIT=$?"
echo "=== automation.sh cargo 'echo hi' ==="
REPO_ROOT="$TH" bash "$TH/automation.sh" cargo "echo hi"; echo "CARGO_ROLE_EXIT=$?"
echo "=== automation.sh review 'echo hi' ==="
REPO_ROOT="$TH" bash "$TH/automation.sh" review "echo hi"; echo "REVIEW_ROLE_EXIT=$?"
echo "=== automation.sh impl on branch 'main' (shared-branch warning) ==="
git -C "$TH" checkout -q -b main
REPO_ROOT="$TH" bash "$TH/automation.sh" impl "echo hi" 2>&1; echo "IMPL_MAIN_EXIT=$?"
