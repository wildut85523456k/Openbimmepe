# Re-review Contract — Docs Reconciliation (register + progress)

## Candidate
- **SHA:** `6e98ae91c979d4fb3eb08860739a6b0582fd5eb7`
- **Base:** `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Branch:** `docs/gap-register-reconciliation-2026-10-25`
- **Worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`

## Context
The prior review (candidate `88e8a89`) returned **FAIL** with three findings, now fixed in a new candidate:
- **P1:** C5 wrongly closed -> restored to `**Open**`.
- **P3:** family citation omitted the 19th entry -> both C3 citations changed `:481-500` -> `:481-503`.
- **P3:** R4 missing-member list omitted `commands` -> added.

## Re-review scope
Verify the three fixes, and re-confirm the prior PASS claims still hold at the new SHA:
1. **R4 count:** `README.md` lists 5 of **20** workspace members (`Cargo.toml`: 19 crates + `apps/desktop`); the missing-member list now includes all 15 (…, `commands`, `apps/desktop`).
2. **C3:** family registry holds exactly **19** families (`library.rs:481-503`); progress §3.3 says **Closed**; register §8 evidence row updated.
3. **C5:** `**Open**` — ceiling/face hosting is not implemented (`family_click` -> `place_family(level, at, spec)` -> `Geometry::Placed { at, level }`; `place_family_in_wall` is wall-only).
4. **C16/C18:** **Closed** (sprinkler `library.rs:1182`, hanger `:1363`).
5. **§5.1 Easy tier:** **CLOSED** with commits `b17a1ff`/`fccee7b`/`91eb050`/`f84104a`/`7afe07b` (ancestors of `b6ef0ee`).
6. **C28:** arc grids exist (`crates/document/src/grid_arc.rs`, `01ddf1f`).
7. **Banners:** §3.1/3.2/3.3 SUPERSEDED banners present; no dangling `§4.4`.

## Acceptance
- The three fixes are correct and the prior PASS claims still hold.
- Doc structure preserved; docs-only (no code/test changes).
- Write the verdict to `docs/agent-team/evidence/gap-register-recon-2026-10-25/reports/re-review.md` with verdict PASS/FAIL/BLOCKED + per-claim findings.

## STOP conditions
- Review only. Do NOT edit any file.
- Do NOT touch the office-P0 worktree (a parallel workstream, never touched).
- Do NOT push or create a PR.
