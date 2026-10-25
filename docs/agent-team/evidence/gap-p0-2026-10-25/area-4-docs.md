# Area 4 contract — docs (R4, R5, R6)

- **Task:** gap-p0-2026-10-25 / **area-4**
- **Base SHA:** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog`
- **Report:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog/docs/agent-team/evidence/gap-p0-2026-10-25/reports/area-4.md`

This area verifies **documentation claims** against the current tree (not code citations).

## Items to verify

- **R4 — README crate table.** `docs/reference/README.md` should carry a complete, current table of
  every workspace crate (name, role, path). Compare it against the actual `[workspace] members` in
  the root `Cargo.toml` at `b6ef0ee`. List any crate missing from the table, any row whose path or
  role is wrong, and any crate that no longer exists.
- **R5 — superseded claims.** Grep the docs (`docs/revit-style-ui/`, `docs/reference/`,
  `docs/agent-team/`, top-level `*.md`) for claims the current code contradicts — e.g. "not
  implemented", "no support", "unimplemented", "does not exist", "cannot". For each hit, check the
  tree at `b6ef0ee` and mark `CONFIRMED` (still true) / `WRONG` (contradicted by code — quote the
  code) / `MISSING-CONTEXT`.
- **R6 — extraction manifest.** Check whether `docs/reference/revit-books/` carries a manifest naming
  the source of each `[book]` / `[knowledge]` mark (which book file feeds which citation). If absent,
  that is the gap; describe what the manifest must contain.

## Extra — the gap docs' own staleness

Re-verify the status claims in the two gap documents themselves at `b6ef0ee`:
`docs/gap-remediation-plan-2026-10-21.md` and `docs/gap-progress-2026-10-23.md`. Which of their rows
have moved since (main advanced 15 commits)? List them with the current state.

## Method

Read-only. Use `rg` for greps. For every claim you mark, quote the real `file:line` (or the exact
doc path + heading) you read. Never guess.

## Deliverable — `reports/area-4.md`

1. Per item: `| Item | Claim | Evidence at b6ef0ee | Verdict |` (Verdict ∈ CONFIRMED / WRONG /
   MISSING-CONTEXT / MISSING).
2. **Splice candidates:** which doc edits are small, independent items (item, files, acceptance
   test, owner role).
3. **Notes / blockers.**

## Gates

- Every item has a verdict + a real path/line you read.
- No fabricated citations.
- Report written to `reports/area-4.md`.

## STOP conditions

- A doc edit would touch production code → note it, do not expand scope.
- A claim you cannot resolve → `BLOCKED` with the reason; never guess.
