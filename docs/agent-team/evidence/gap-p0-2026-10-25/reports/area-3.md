# Area 3 verification — rooms / tags / hosting (B21, B22, B23, B24)

- **Task:** gap-p0-2026-10-25 / **area-3**
- **Base SHA (pinned):** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog` (branch `docs/gap-p0-production-backlog-2026-10-25`, `HEAD == b6ef0ee`, tree clean)
- **Register:** `docs/gap-remediation-plan-2026-10-21.md` §2.2 (lines 114–117)
- **Method:** read-only citation verification. No build, no edits outside this report.

## 0. Headline

**All four rows are CLOSED at `b6ef0ee`.** No row is still open, so there are no splices to propose
against the register's own acceptance criteria — see §2 for the four small residuals that remain
*inside* the closed work and would be the natural next production items if the owner wants them.

The closing work is commit **`91eb050`** — *"Shell blockers B5, B12, B13, B18, B20 to B25: compute inputs
saved, scoped compute, recovery copy, room number/name, parameter tags, level strip, equipment pad"* —
which is an ancestor of `b6ef0ee` (`git merge-base --is-ancestor 91eb050 HEAD` → YES). Its own audit,
`docs/audits/2026-10-23-shell-b2-b25.md` (lines 20–24), claims B21–B25 closed with named tests; every
one of those tests and every cited line was re-read here and holds.

## 1. Per-row verdicts

| Row | Register claim | Citation | State at `b6ef0ee` | Verdict | Real `file:line` |
|---|---|---|---|---|---|
| **B21** | Rooms cannot be named or numbered; the room name is fixed `"Room N"`. | `crates/document/src/rooms.rs`; `apps/desktop/src/panels.rs` | Number / Name / Department are instance data, edited in Properties, unique-per-level on Number, undoable in one step, in the room schedule, saved in the file, and read back from IFC. The `"Room N"` string survives only as the *seed* name a room is born with. | **CLOSED** | `crates/document/src/rooms.rs:125-141` (`room_text` / `set_room_data`), `:158` (seed `Room {n}`), `:217-243` (test); `apps/desktop/src/panels.rs:778` + `:1114-1129` (`room_identity`); `crates/document/src/schedule.rs:115`, `:227`, `:482-486` |
| **B22** | Levels have no lines or bubbles in plans. | `apps/desktop/src/model_view.rs` | A datum strip draws a dashed level line and a labelled bubble for every level in a plan, highest first, the view's own level marked. | **CLOSED** | `apps/desktop/src/model_view.rs:187-199` (draw), `:1255-1259` (`level_bubbles`), `:1276-1281` (test). Register citation `model_view.rs` is correct but unnumbered; the row was verified at `dcf8382` via `8.5` → `model_view.rs:220` (`Some(Geometry::Level) if three_d`), the only Level arm, which proves the absence in plans. |
| **B23** | Tags show the element name, not a parameter. | `crates/document/src/lib.rs` `tag_text`; `apps/desktop/src/model_view.rs` | `tag_text` resolves parameters at draw time: a duct says `<system> <W>x<H>mm`, a pipe `<system> <D>mm`, a room `<number> <name>`; the element name is only the fallback when no parameter applies. | **CLOSED** | `crates/document/src/lib.rs:840-861` (def at `:842`), `:847-852` (the parameter arms); `apps/desktop/src/model_view.rs:478` (`tag_text` read at draw time); `crates/document/src/rooms.rs:245-263` (test, incl. "the tag follows the parameter when it changes") |
| **B24** | The equipment pad is not associated with the equipment, so moving the equipment does not move the pad. | `crates/document/src/lib.rs:844` | `place_equipment_pad` takes the unit as `Command::SetParent`; `with_dependents` carries a parented pad on Move, Copy and Delete. | **CLOSED** | `crates/document/src/lib.rs:974-987` (`place_equipment_pad`, `SetParent` at `:984`); `crates/document/src/edit.rs:53-63` (`with_dependents`, `e.parent` at `:59`); `crates/document/src/edit.rs:783-801` (test); `apps/desktop/src/model_edit.rs:533-541` (click-on-unit wiring) + `:1350-1366` (test) |

### Notes on each citation

- **B21** — the register's two citations are both still correct as *files*. `rooms.rs:130`
  `set_room_data` and `panels.rs:1116` `room_identity` are the whole fix. The register's §8.5 entry
  for B21 was deliberately vague (`crates/document/src/rooms.rs`; "no Number/Name/Department instance
  parameters") — that description is now false, and the row is closed by code added after `dcf8382`.
- **B22** — the register gave `apps/desktop/src/model_view.rs` with no line, so there was nothing to
  move. The `8.5` proof of absence (`model_view.rs:220`, Level arm gated on `three_d`) is still the
  right citation and is still the only `Geometry::Level` arm in the file at `b6ef0ee` (`:258`).
- **B23** — `tag_text` was at `crates/document/src/lib.rs:744` at `dcf8382`; it is at `:842` now
  (the file grew 1442 → 1619 lines). **The citation moved, the claim is closed.** Register `8.5` has no
  B23 entry (only B21 and B26 in that block), so the register's in-row citation `lib.rs tag_text` is the
  one that needed re-checking — it resolves.
- **B24** — the register's `crates/document/src/lib.rs:844` is **WRONG at `b6ef0ee`**: `:844` is now
  `let Some(h) = self.get(host) else { return "?".into() };` inside `tag_text`. At `dcf8382` `:844`
  was `pub fn place_footing(...)` — also not a pad-equipment link, so the register's line was already
  approximate. The row itself is closed at `lib.rs:974-987`. Flagging this as the one citation in my
  area that does not resolve to the claimed thing; the row is closed regardless, on its own code.

## 2. Splice candidates (residuals *inside* the closed rows)

These are not register rows — every B21–B24 acceptance criterion passes. They are the small,
independent items the closing work left behind. Each is sized for one owner, one review, one test.

### S1 — B21 residual: no uniqueness check on the room *Number* against the schedule's own key
- **Gap:** `set_room_data` refuses a duplicate Number only within the same `level`
  (`rooms.rs:134`). Two rooms on different levels may share a Number; Revit's room schedule keys on
  Number globally.
- **Files:** `crates/document/src/rooms.rs` (`set_room_data`), `crates/document/src/schedule.rs:482-486`.
- **Acceptance test:** place two rooms on two levels, give both Number `101`, assert `Err`; extend
  `a_room_has_a_number_a_name_and_a_department_that_undo_and_survive_saving` (`rooms.rs:217`).
- **Owner role:** document-crate engineer. **Deps:** none (B21 already landed).
- **Risk:** low. Owner decision needed — Revit actually scopes room Number uniqueness to the level in
  some workflows; confirm the intended rule before coding.

### S2 — B22 residual: the datum strip is screen-fixed, not model-anchored
- **Gap:** `model_view.rs:193` pins each bubble to `rect.left() + 10.0` / `rect.top() + 10.0`, so the
  strip is drawn at a constant screen position rather than at the level's elevation in the plan, and
  it is not clipped or zoomed with the model. The row's acceptance ("a plan with a level above and
  below shows both bubbles") passes because `level_bubbles` (`:1255`) returns *all* levels regardless
  of range — but the strip does not read like Revit's, where the bubble sits on the level's own line.
- **Files:** `apps/desktop/src/model_view.rs:187-199`, `:1255-1259`.
- **Acceptance test:** a pure-function test already exists (`:1276`); the new work needs a
  `frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui))` assertion (pattern at
  `apps/desktop/src/view_ui.rs:530`) that the bubble positions move when the level elevations change.
- **Owner role:** desktop viewport engineer. **Deps:** none.

### S3 — B23 residual: the tag's parameter is hard-coded per category, not chosen
- **Gap:** `tag_text` (`lib.rs:846-860`) picks the parameter from a `match` on `Category`, so a tag
  cannot be told to show, say, a duct's length or a family's Mark. The register's acceptance ("a duct
  tag shows its size and system") passes, but the remediation wording — "make the tag text a parameter
  reference" — is only half met: the text is *resolved* from parameters, it is not *referenced* by one.
- **Files:** `crates/document/src/lib.rs:842-861` and `:863-872` (`place_tag` — a tag carries no
  parameter field today), `apps/desktop/src/model_view.rs:478`.
- **Acceptance test:** store a `TagParam` on the tag (a new `Command::SetTagParam` or a `texts` entry),
  assert `tag_text` returns that parameter's value, and assert an unknown key falls back to today's
  behaviour.
- **Owner role:** document-crate engineer. **Deps:** none, but touches `Command` (undo surface) —
  flag for review.

### S4 — B24 residual: only a *placed* family instance can host a pad
- **Gap:** `place_equipment_pad` (`lib.rs:977`) requires `Geometry::Placed`; a family in a wall
  (`Geometry::InWall`) or on a member is rejected. The audit already records this
  (`docs/audits/2026-10-23-shell-b2-b25.md:37`).
- **Files:** `crates/document/src/lib.rs:977`, `apps/desktop/src/model_edit.rs:533`.
- **Acceptance test:** `place_equipment_pad` on an in-wall family returns `Ok` and the pad follows it;
  add to `an_equipment_pad_is_made_by_its_unit_and_moves_copies_and_goes_with_it` (`edit.rs:783`).
- **Owner role:** document-crate engineer. **Deps:** none.

### S5 — cross-cutting: the level strip is not on sheets or in exported plans
- **Gap:** `level_bubbles` is called only from `model_view.rs:191`. `apps/desktop/src/plan_export.rs`
  has no `Geometry::Level` arm (levels are only used to pick the datum, `:77`/`:89`), and
  `crates/sheets` has a datum-bubble primitive (`crates/sheets/src/annotations.rs:1`) that the plan
  export does not feed levels into.
- **Files:** `apps/desktop/src/plan_export.rs:100-120`, `crates/sheets/src/annotations.rs`.
- **Acceptance test:** a plan exported to a sheet carries one datum line + bubble per level.
- **Owner role:** sheets/export engineer. **Deps:** none; independent of B22's closure.

## 3. Notes / blockers

- **No row in this area is open.** `open = 0`, `closed = 4`, `rows_verified = 4`.
- **No BLOCKED citation.** Every line cited above was read at `b6ef0ee` in the pinned worktree.
- **The register's B24 line `crates/document/src/lib.rs:844` does not resolve** — it is inside
  `tag_text` now. Recorded in §1; the row is closed on `lib.rs:974-987` instead.
- **The register's §8.5 "verified line references" block is not a per-row citation set.** It covers
  B1–B19, B21, B26, C*, D10 — B22, B23, B24, B25 have no entry there. For my area only B21 had one
  (`crates/document/src/rooms.rs`, now stale as a description). This is why the in-row citations had to
  be re-derived rather than looked up.
- **`dcf8382` is 112 commits behind `b6ef0ee`, not 15.** `git rev-list --count dcf8382..b6ef0ee` → 112.
  The "15 commits" in the area contract matches neither: `b6ef0ee~15..b6ef0ee` is 36 commits (merges).
  The file list in the contract does match `b6ef0ee~15..b6ef0ee` exactly (`model_view.rs`, `lib.rs`,
  `panels.rs`, `mep.rs`, `family/src/library.rs` are all in it), so the contract's *files* are right and
  its *count* is wrong. Harmless for this verification — the four rows were closed by `91eb050`, which
  is inside both windows.
- **The register file did not exist at `dcf8382`.** `git cat-file -e dcf8382:docs/gap-remediation-plan-2026-10-21.md`
  fails; it was added by `f7436e7` (21 Oct, after `dcf8382`). The "verified at `dcf8382`" provenance in
  the register's own §8.5 header therefore refers to the *gap analysis* and the *code*, not to the
  register file as it stands. Recorded, not treated as a blocker.
- **`docs/gap-progress-2026-10-23.md:74` still lists "B21-B24 … Open (not re-checked)"** — that report
  was written at `271c774`, before `91eb050` landed. It is stale, not wrong about the code at its own SHA.
- **No file outside `crates/document/src` and `apps/desktop/src` was needed.** S5 touches
  `crates/sheets/src` and is marked as a cross-cutting residual, not a row expansion.
