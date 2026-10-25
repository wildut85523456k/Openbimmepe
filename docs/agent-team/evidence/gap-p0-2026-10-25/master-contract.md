# Task contract — gap-p0-2026-10-25 (P0 production backlog)

- **Task ID:** gap-p0-2026-10-25
- **Owner:** Wilfrid (wildut85523456k)
- **Base branch / SHA:** `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Worktree (read-only):** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog` (branch `docs/gap-p0-production-backlog-2026-10-25`)
- **Evidence root:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog/docs/agent-team/evidence/gap-p0-2026-10-25/`

## Approved scope

**Reading B — the next critical path** = the register's §5.1 "Easy" tier (days-scale wiring on
existing code). Rows:

- **IFC exchange honesty:** B7, B8, B9, B10, B11, B14, B15, B19
- **Shell + persistence:** B2, B12, B13, B16
- **Rooms / tags / hosting:** B21, B22, B23, B24
- **Docs:** R4, R5, R6

**Out of scope (STOP conditions):** implementing any row; class 3/4 rows (C*, D*); the walking
skeleton Step 0 (B1, B3, B4, B5, B6, B17, B18, B20, B25); any push / merge / commit.

## Coordination (parallel P0 workstream)

A second worktree `D:/Codex/Revit-alike-P0` (branch `ci/private-dependency-auth`, based on
`b6ef0ee`, +6 commits) is running a **parallel "office" P0** — the walking-skeleton event journey
+ native CI matrix + `docs/capability-register.md`. Owner decision 2026-10-25: **reference it as a
parallel workstream** in the final backlog doc — note it and its capability-register, avoid
duplicating it, and keep this run's verification on `main` (`b6ef0ee`). Shared-file boundary: the
office work edited `apps/desktop/src/model_view.rs` (+83, tests only); Reading-B code
(`export.rs`, `native.rs`, `compute_ui.rs`, `rooms.rs`) is untouched there.

## Why this run

The register (`docs/gap-remediation-plan-2026-10-21.md`) was verified at `dcf8382`. `main` has since
advanced **15 commits** to `b6ef0ee`, several touching the very files the rows cite (`export.rs`,
`model_edit.rs`, `model_view.rs`, `native.rs`, `crates/ifc/src/lib.rs`, `crates/document/src/mep.rs`,
`crates/document/src/lib.rs`, `crates/family/src/library.rs`). Every row must be re-verified against
the current tree before it is spliced into a production item — a stale row produces a wrong task
contract.

## Method constraints

- **READ-ONLY** on the source. No edits, no commits, no branches, no fetch, no push.
- Read the register at the pinned SHA (it is in the worktree): `docs/gap-remediation-plan-2026-10-21.md`.
- For each row: read the cited file/lines at `b6ef0ee` and compare. A claim whose citation **moved**
  is `APPROXIMATE`, not `WRONG`. A row the code now satisfies is `CLOSED`. Never guess a line you
  did not read.
- Windows git-bash: pass `C:/...` paths to native tools (`git`, `rg`).
- Write **ONLY** to your report path (below). Never inside the worktree source.

## Deliverable

One report per area at `reports/<area>.md`:

1. Table: `| Row | Register claim | Citation | State at b6ef0ee | Verdict | Real file:line |`
   Verdict ∈ `CONFIRMED` / `APPROXIMATE` / `WRONG` / `CLOSED`.
2. **Splice candidates:** for each still-open row, how it breaks into small production items
   (item, files, acceptance test, owner role, dependencies).
3. **Notes / blockers.**

## Gates

- Every row in the area has a verdict and a real `file:line`.
- No fabricated citations; a line you did not read is `BLOCKED` with the reason.
- Report written to `reports/<area>.md`.

## Handoff

Report path + a 5-line summary. No push, no merge.
