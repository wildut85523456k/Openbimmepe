# Area 3 contract — rooms / tags / hosting (B21, B22, B23, B24)

- **Task:** gap-p0-2026-10-25 / **area-3**
- **Base SHA:** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog`
- **Report:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog/docs/agent-team/evidence/gap-p0-2026-10-25/reports/area-3.md`

## Rows to verify

Read the full row text in `docs/gap-remediation-plan-2026-10-21.md` §2.2 at the pinned SHA.
Citations below are the register's (verified at `dcf8382`) — re-check them at `b6ef0ee`.

| Row | Register claim (paraphrased) | Register citation |
|---|---|---|
| **B21** | Rooms cannot be named or numbered; the room name is fixed `"Room N"`. | `crates/document/src/rooms.rs`; `apps/desktop/src/panels.rs` |
| **B22** | Levels have no lines or bubbles in plans. | `apps/desktop/src/model_view.rs` |
| **B23** | Tags show the element name, not a parameter. | `crates/document/src/lib.rs` `tag_text`; `apps/desktop/src/model_view.rs` |
| **B24** | The equipment pad is not associated with the equipment. | `crates/document/src/lib.rs:844` |

## Note on staleness

`main` advanced 15 commits since the register was verified. Files touched in that window include
`apps/desktop/src/model_view.rs` (+19/-0), `crates/document/src/lib.rs` (+2/-2),
`apps/desktop/src/panels.rs` (+2/-0), `crates/document/src/mep.rs` (+87/-1),
`crates/family/src/library.rs` (+735/-3). **Re-check every citation against `b6ef0ee`.**

## Method

For each row: open the cited file(s) at `b6ef0ee`, read the cited lines, answer `CONFIRMED` /
`APPROXIMATE` / `WRONG` / `CLOSED` with the real `file:line`. If the code now satisfies the row,
mark `CLOSED` and quote the closing code. If a citation moved but the claim holds, `APPROXIMATE` +
the new line. **Do not build** — read-only citation verification.

## Deliverable — `reports/area-3.md`

1. Table: `| Row | Register claim | Citation | State at b6ef0ee | Verdict | Real file:line |`
2. **Splice candidates:** for each still-open row, how it breaks into small production items
   (item, files, acceptance test, owner role, deps).
3. **Notes / blockers.**

## Gates

- Every row has a verdict + a real `file:line` you read.
- No fabricated citations.
- Report written to `reports/area-3.md`.

## STOP conditions

- A row needs a file outside `crates/document/src`, `apps/desktop/src` → note it, do not expand scope.
- A citation you cannot find → `BLOCKED` with the reason; never guess.
