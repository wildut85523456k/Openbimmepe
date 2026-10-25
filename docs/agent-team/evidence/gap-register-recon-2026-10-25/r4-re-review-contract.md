# Re-review Contract — R4 README crate table (fixed)

## Candidate
- **SHA:** `a8acb3bf65c68e6453f356e4b6de10676c4f2380`
- **Base:** `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Branch:** `docs/gap-register-reconciliation-2026-10-25`
- **Worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`

## Context
The prior R4 review (candidate `ebb043d`) returned **FAIL** with one finding:
- **R4-01:** README `crates/view` description said "cut, projected and hidden elements", but `crates/view/src/lib.rs` documents Cut/Below/Overhead/Beyond (the `Class` enum has no "hidden" class).

Fixed in the new candidate: the row now reads "cut, below-cut, overhead and beyond geometry".

## Re-review scope
1. Verify the `crates/view` row now matches `crates/view/src/lib.rs` (doc-comment + `Class` enum: Cut/Below/Overhead/Beyond/Symbol/Room/Datum; `classify`).
2. Re-confirm the prior PASS claims still hold at the new SHA: completeness (20 members + spikes row), the other 14 descriptions accurate, the register R4 row marked closed, structure/docs-only.

## Acceptance
- The `view` description is corrected and accurate; all other descriptions accurate.
- Docs-only; structure preserved.
- Write the verdict to `docs/agent-team/evidence/gap-register-recon-2026-10-25/reports/r4-re-review.md` with verdict PASS/FAIL/BLOCKED + per-claim findings.

## STOP conditions
- Review only. Do NOT edit any file.
- Do NOT touch the office-P0 worktree (a parallel workstream, never touched).
- Do NOT push or create a PR.
