# Area 1 contract — IFC export honesty (B7, B8, B9, B10, B11, B14, B15, B19)

- **Task:** gap-p0-2026-10-25 / **area-1**
- **Base SHA:** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog`
- **Report:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog/docs/agent-team/evidence/gap-p0-2026-10-25/reports/area-1.md`

## Rows to verify

Read the full row text in `docs/gap-remediation-plan-2026-10-21.md` §2.2 at the pinned SHA.
The citations below are the register's (verified at `dcf8382`) — re-check them at `b6ef0ee`.

| Row | Register claim (paraphrased) | Register citation |
|---|---|---|
| **B7** | Every duct/pipe exported with `System::Other`; `MepData.run_systems` never read. | `apps/desktop/src/export.rs:205-207`; `crates/document/src/mep.rs:189` |
| **B8** | Rooms/grids/site dropped from IFC export; the IFC crate *can* write `IfcSpace`. | `apps/desktop/src/export.rs:198,291-293`; `crates/ifc/src/lib.rs:964,1497-1556` |
| **B9** | User systems not written as `IfcSystem`; one `IfcDistributionSystem` per `System` enum code. | `crates/ifc/src/lib.rs:1823-1838`; `apps/desktop/src/export.rs` |
| **B10** | Wall/floor penetrations never created; `openbim_place::penetrations` exists. | `apps/desktop/src/export.rs:290-291`; `crates/place/src/penetrations.rs` |
| **B11** | GlobalIds not stable; project key fixed at 1. | `crates/ifc/src/lib.rs:645`; `crates/document/src/lib.rs` `next_id`; `apps/desktop/src/model_edit.rs:305-306` |
| **B14** | Marking an End Point drops the equipment join. | `apps/desktop/src/compute_ui.rs:132` |
| **B15** | The Air preset does not change the velocity limit. | `apps/desktop/src/compute_ui.rs:69-70` |
| **B19** | Trusses and beam systems lose their parent on export. | `apps/desktop/src/export.rs:198`; `crates/ifc/src/lib.rs` |

## Note on staleness

`main` advanced 15 commits since the register was verified. Files touched in that window include
`apps/desktop/src/export.rs` (+14/-6), `crates/ifc/src/lib.rs` (+34/-1),
`crates/document/src/mep.rs` (+87/-1), `crates/document/src/lib.rs` (+2/-2),
`crates/family/src/library.rs` (+735/-3). **Re-check every citation against `b6ef0ee`.**

## Method

For each row: open the cited file(s) at `b6ef0ee`, read the cited lines, answer `CONFIRMED` /
`APPROXIMATE` / `WRONG` / `CLOSED` with the real `file:line`. If the code now satisfies the row,
mark `CLOSED` and quote the closing code. If a citation moved but the claim holds, `APPROXIMATE` +
the new line. If the claim is false at `b6ef0ee`, `WRONG` + what the code actually does.
**Do not build** — this is a read-only citation verification.

## Deliverable — `reports/area-1.md`

1. Table: `| Row | Register claim | Citation | State at b6ef0ee | Verdict | Real file:line |`
2. **Splice candidates:** for each still-open row, how it breaks into small production items
   (item, files, acceptance test, owner role, deps).
3. **Notes / blockers.**

## Gates

- Every row has a verdict + a real `file:line` you read.
- No fabricated citations.
- Report written to `reports/area-1.md`.

## STOP conditions

- A row needs a file outside `apps/desktop/src`, `crates/ifc/src`, `crates/document/src`,
  `crates/place/src`, `crates/family/src` → note it, do not expand scope.
- A citation you cannot find → `BLOCKED` with the reason; never guess.
