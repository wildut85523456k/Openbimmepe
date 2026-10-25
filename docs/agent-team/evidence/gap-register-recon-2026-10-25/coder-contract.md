# Task Contract — Docs Reconciliation (register + progress)

## Role
You are the **coder**. Implement the docs reconciliation. Do NOT review or test — hand off.

## Environment
- Worktree: `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`
- Branch: `docs/gap-register-reconciliation-2026-10-25` (off `main` @ `b6ef0ee`)
- Windows host, git-bash. Pass `C:/...` paths to native tools. `GIT_TERMINAL_PROMPT=0` for any network.

## Background
The gap register (`docs/gap-remediation-plan-2026-10-21.md`) + progress snapshot (`docs/gap-progress-2026-10-23.md`) were last verified at `dcf8382`, 112 commits behind `main` @ `b6ef0ee`. A verification pass confirmed the §5.1 "Easy" tier is closed. Reconcile the two docs to the verified findings.

## The exact changes

### A. Add a "Verification status" note to BOTH docs (near the top, after the title/intro)
```
> **Verification status (2026-10-25).** Re-verified against `main` @ `b6ef0ee`.
> The §5.1 "Easy" tier (B2, B7–B11, B12–B16, B21–B24) is **closed**
> (`b17a1ff`, `fccee7b`, `91eb050`, `f84104a`, `7afe07b`). This doc was last
> verified at `dcf8382` (112 commits stale); rows updated below. Evidence:
> `docs/agent-team/evidence/gap-p0-2026-10-25/`.
```

### B. Register (`docs/gap-remediation-plan-2026-10-21.md`)
- R4 count row: change `19 members (18 crates + desktop)` → `20 members (19 crates + apps/desktop)`. (Appears ~lines 63, 70.)

### C. Progress (`docs/gap-progress-2026-10-23.md`)
- §4.1, §4.2, §4.3: add a `> **SUPERSEDED (2026-10-25):** these tables predate the model-from-scratch (B11); see §4.4.` banner at the top of each section.
- §4.3 table:
  - C3: `7 families open` → `19 members (18 crates + apps/desktop); families closed (B11)`.
  - C5, C16, C18: `open` → `closed`.
  - C28: `grids cannot be arcs` → `arc grids exist (crates/document/src/grid_arc.rs, 01ddf1f)`.
- §5.1 "Easy tier" row: mark it **closed** (with the closing commits `b17a1ff`/`fccee7b`/`91eb050`/`f84104a`/`7afe07b`).

## Constraints
- Docs only. LF line endings. Preserve each doc's structure — minimal edits, no reformatting.
- Read each doc first; find the exact target; edit precisely.
- Write a change-log to `docs/agent-team/evidence/gap-register-recon-2026-10-25/coder-log.md` (what you changed, per doc).
- Commit locally (message: `docs: reconcile gap register + progress to main @ b6ef0ee`). Do NOT push.

## STOP conditions
- Do NOT edit code, tests, or any doc other than the two named.
- Do NOT touch the office-P0 worktree (`D:/Codex/Revit-alike-P0`).
- Do NOT push or create a PR.
- If a target row/section isn't found, note it in the log and skip — don't guess.

## Deliverable
- The two docs edited + committed.
- `docs/agent-team/evidence/gap-register-recon-2026-10-25/coder-log.md`.
