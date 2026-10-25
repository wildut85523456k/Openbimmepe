# Re-review contract — R5 (stale claims) + R6 (extraction manifest), review fixes

Reviewer: `bim-review` profile. READ-ONLY. Verdict PASS/FAIL with findings. Do NOT edit, commit, or push.

## Candidate — SCOPE IS THE R5/R6 SLICE ONLY
- Repo: `wildut85523456k/revit-alike`. Worktree: `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`.
- **Review the slice `b50f93c..86baaff` (NOT the whole branch `b6ef0ee..HEAD`).** `b50f93c` is the branch state before the R5/R6 coder; `86baaff` is the candidate (coder commit `a8158aa` + review-fix commit `86baaff`). The earlier reconciliation + R4 slices live below `b50f93c` and are OUT OF SCOPE here.
- Slice diff: `git -C "C:/Users/wilfr/hermes-first-task/wt-gap-register-recon" diff b50f93c..86baaff`.

## Prior review (FAIL) — confirm each finding is now resolved
1. **R5R6-03 (MEDIUM): register R5 row misclassifies MEP-002 as a stale claim.** Confirm `docs/gap-remediation-plan-2026-10-21.md` R5 row now states that five of the six are stale + marked, and that MEP-002 is **not** stale (mismatch still exists; `gap-review.md:17` discloses the label as gap-review.md's own assessment) — left unmarked.
2. **Gate 2 (office path): an office-worktree path string was committed.** Confirm `git -C "C:/Users/wilfr/hermes-first-task/wt-gap-register-recon" grep -nI "Codex\|Revit-alike-P0" b50f93c..86baaff -- docs/` returns NOTHING.
3. Prior review already PASSed: R5 claims 1,2,3,4,6 marks; claim 5 left unmarked; R6 manifest honesty (SHA-256/tool marked not-recorded, no fabrication). Spot-check they still hold at `86baaff`.

## Gates on the SLICE `b50f93c..86baaff`
1. **Docs-only:** every changed path is under `docs/` (markdown + the new `docs/reference/extraction-manifest.md` + evidence). No `.rs`/`.toml`/`Cargo.lock`, no `README.md`.
2. **Office isolation:** no office-worktree path string anywhere in the slice diff.
3. **Register/progress touch is bounded:** the slice touches `docs/gap-remediation-plan-2026-10-21.md` ONLY for the R5 row's resolution note (the R5R6-03 fix); it does NOT re-touch R4/C3/C5/C28 or `gap-progress-2026-10-23.md`.
