# Area 2 contract — shell + persistence (B2, B12, B13, B16)

- **Task:** gap-p0-2026-10-25 / **area-2**
- **Base SHA:** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog`
- **Report:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog/docs/agent-team/evidence/gap-p0-2026-10-25/reports/area-2.md`

## Rows to verify

Read the full row text in `docs/gap-remediation-plan-2026-10-21.md` §2 at the pinned SHA.
Citations below are the register's (verified at `dcf8382`) — re-check them at `b6ef0ee`.

| Row | Register claim (paraphrased) | Register citation |
|---|---|---|
| **B2** | Plan tabs hard-coded `"Level 1"`/`"Level 2"`/`"M-101"` to `ElementId(10)`/`(11)` (sample ids). | `apps/desktop/src/model_view.rs:68-75`; `apps/desktop/src/model_edit.rs:315`; `apps/desktop/src/panels.rs:941-948,1003`; `apps/desktop/src/view3d.rs:241` |
| **B12** | Compute inputs (size catalogue, curves, cable tables, last result) are not persisted. | `crates/document/src/native.rs`; `apps/desktop/src/compute_ui.rs`; `apps/desktop/src/file.rs` |
| **B13** | The MEP Settings size lists are ignored by Compute. | `apps/desktop/src/compute_ui.rs`; `apps/desktop/src/mep_settings_ui.rs`; `crates/calc/src/tree.rs:228` |
| **B16** | No Save As button (the `save_as` command id is handled but unreachable). | `apps/desktop/src/ribbon.rs:46-54`; `apps/desktop/src/main.rs:476` |

## Note on staleness

`main` advanced 15 commits since the register was verified. Files touched in that window include
`apps/desktop/src/model_view.rs` (+19/-0), `apps/desktop/src/model_edit.rs` (+13/-2),
`crates/document/src/native.rs` (+14/-2), `apps/desktop/src/main.rs` (+12/-1),
`crates/document/src/lib.rs` (+2/-2), `apps/desktop/src/panels.rs` (+2/-0),
`apps/desktop/src/view3d.rs` (+16/-5). **Re-check every citation against `b6ef0ee`.**
The progress report (§4) already records "New project, add levels, rename, delete → Works", so B2
may be partly closed — verify what remains.

## Method

For each row: open the cited file(s) at `b6ef0ee`, read the cited lines, answer `CONFIRMED` /
`APPROXIMATE` / `WRONG` / `CLOSED` with the real `file:line`. If the code now satisfies the row,
mark `CLOSED` and quote the closing code. If a citation moved but the claim holds, `APPROXIMATE` +
the new line. **Do not build** — read-only citation verification.

## Deliverable — `reports/area-2.md`

1. Table: `| Row | Register claim | Citation | State at b6ef0ee | Verdict | Real file:line |`
2. **Splice candidates:** for each still-open row, how it breaks into small production items
   (item, files, acceptance test, owner role, deps).
3. **Notes / blockers.**

## Gates

- Every row has a verdict + a real `file:line` you read.
- No fabricated citations.
- Report written to `reports/area-2.md`.

## STOP conditions

- A row needs a file outside `apps/desktop/src`, `crates/document/src`, `crates/calc/src` → note it, do not expand scope.
- A citation you cannot find → `BLOCKED` with the reason; never guess.
