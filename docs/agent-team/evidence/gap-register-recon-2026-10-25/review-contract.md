# Review Contract — Docs Reconciliation (register + progress)

## Candidate
- **SHA:** `88e8a894ad34cb8e12e0cfad6d6e02201a0da7ed`
- **Base:** `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Branch:** `docs/gap-register-reconciliation-2026-10-25`
- **Worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`

## Files changed
- `docs/gap-remediation-plan-2026-10-21.md`
- `docs/gap-progress-2026-10-23.md`
- `docs/agent-team/evidence/gap-register-recon-2026-10-25/` (coder-contract.md, coder-log.md)

## Claims to verify (against `main` @ `b6ef0ee`)
1. **R4 count:** the register now says `README.md` lists 5 of **20** workspace members (was 19). Verify `Cargo.toml` has 20 members (19 crates + `apps/desktop`).
2. **C3:** progress §3.3 row + register §8 evidence now say the family library holds **19 families** (handle, door, window, arched_door, fan_coil, ahu, ceiling_diffuser, vav, pump, valve, wc, washbasin, sprinkler_head, luminaire, panel_board, hanger, pipe_elbow, pipe_reducer, pipe_tee), status **Closed**. Verify `crates/family/src/library.rs:481-500` lists exactly these 19.
3. **C5/C16/C18:** progress §3.3 says **Closed**. Verify (C5 hosting, C16 sprinkler `library.rs:1182`, C18 hangers `library.rs:1363`).
4. **§5.1 Easy tier:** progress §5.1 marked **CLOSED** with commits `b17a1ff`/`fccee7b`/`91eb050`/`f84104a`/`7afe07b`. Verify those are ancestors of `b6ef0ee`.
5. **C28:** progress §3.4 row says arc grids exist (`crates/document/src/grid_arc.rs`, `01ddf1f`). Verify.
6. **Banners:** the §3.1/3.2/3.3 SUPERSEDED banners are present and no longer reference a non-existent `§4.4`.

## Acceptance
- Every status claim above matches the code at `b6ef0ee`.
- Doc structure preserved (tables intact, no reformatting).
- Docs-only: no code/test changes.
- Write the verdict to `docs/agent-team/evidence/gap-register-recon-2026-10-25/reports/independent-review.md` with verdict PASS/FAIL/BLOCKED + per-claim findings.

## STOP conditions
- Review only. Do NOT edit any file.
- Do NOT touch the office-P0 worktree (`D:/Codex/Revit-alike-P0`).
- Do NOT push or create a PR.
