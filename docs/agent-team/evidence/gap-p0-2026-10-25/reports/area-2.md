# Area 2 verification — shell + persistence (B2, B12, B13, B16)

- **Task:** gap-p0-2026-10-25 / **area-2**
- **Base SHA (pinned):** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog` (branch `docs/gap-p0-production-backlog-2026-10-25`, `HEAD == b6ef0ee`)
- **Register:** `docs/gap-remediation-plan-2026-10-21.md` §2.1 (B2, line 86) and §2.2 (B12 line 105, B13 line 106, B16 line 109). §8.5 verified-line references at lines 417, 427, 428.
- **Method:** read-only citation verification. No build, no edits outside this report.

## 0. Headline

**All four rows are CLOSED at `b6ef0ee`.** The closing work is commit **`91eb050`** — *"Shell
blockers B5, B12, B13, B18, B20 to B25: compute inputs saved, scoped compute, recovery copy, room
number/name, parameter tags, level strip, equipment pad"* — an ancestor of `b6ef0ee`
(`git merge-base --is-ancestor 91eb050 HEAD` → YES), plus `f84104a` (Save As quick-access button) and
`7afe07b`/`eb073a1` (tabs named after the document's levels). Each row's cited lines and every cited
file were re-read at `b6ef0ee`; the register's citations were all written against `dcf8382`, where the
code was still open.

Two residuals remain *inside* the closed work and are the natural next production items if the owner
wants them: (a) `plan_level()` still returns the `ElementId(0)` sentinel instead of `Option<ElementId>`
(B2), and (b) the last compute **result** and the **electrical** cable tables are still not persisted
(B12). Neither is a defect against the register's acceptance; both are small follow-on items. See §2.

## 1. Per-row verdicts

| Row | Register claim | Citation (register) | State at `b6ef0ee` | Verdict | Real `file:line` |
|---|---|---|---|---|---|
| **B2** | Plan tabs hard-coded `"Level 1"`/`"Level 2"`/`"M-101"` to `ElementId(10)`/`(11)` (sample ids); a tool points at a missing level. | `model_view.rs:68-75`; `model_edit.rs:315`; `panels.rs:941-948,1003`; `view3d.rs:241` | Tabs are derived from `document.levels()` by position; `plan_level()` resolves the view to a real level (sentinel `0` when none); every level-requiring tool refuses with "Add a level first"; plan export and 3D family placement both error/refuse instead of pointing at id 10. | **CLOSED** | `apps/desktop/src/model_view.rs:24-29` (`plan_tabs`), `:90` (call); `model_edit.rs:419-424` (`plan_level`, sentinel at `:423`); `main.rs:443-446` (refusal); `view3d.rs:255-259` (3D fallback via `snap_place.rs:190-197`); `plan_export.rs:76-86`; `panels.rs:999-1008` (`level_view_name`) |
| **B12** | Compute inputs (size catalogue, loading-unit curve, cable tables) not persisted; results not stored. | `crates/document/src/native.rs`; `apps/desktop/src/compute_ui.rs`; `apps/desktop/src/file.rs` | The thermal/hydraulic compute inputs are written as one `U|kind=compute|…` app-record line, saved in the `.obim`, and read back on open. Acceptance ("reopen and Compute runs with the same inputs") is met. | **CLOSED** | `apps/desktop/src/compute_ui.rs:74-83` (`record`), `:86-104` (`restore`), `:248-251` (read); `crates/document/src/native.rs:392` (app-records in file); `apps/desktop/src/file.rs:79` (`sync_app_records`), `:231-232` (restore on load); test `file.rs:371-391` |
| **B13** | MEP Settings size lists ignored by Compute; Compute demands a typed catalogue. | `apps/desktop/src/compute_ui.rs`; `mep_settings_ui.rs`; `crates/calc/src/tree.rs:228` | An empty catalogue in the Compute dialog is filled from the MEP Settings lists (`catalogue_from_settings`); the dialog label says so; a test sizes a run from the MEP Settings lists alone. | **CLOSED** | `apps/desktop/src/compute_ui.rs:250` (read), `:150-163` (`catalogue_from_settings`), `:341` (label), `:569-577` (test); `mep_settings_ui.rs:1-110`; `crates/calc/src/tree.rs:228` (`NoSize` — only when both lists are empty) |
| **B16** | No Save As button; `save_as` command id handled but unreachable. | `apps/desktop/src/ribbon.rs:46-54`; `apps/desktop/src/main.rs:476` | `save_as` is in the quick-access bar, enabled (`IMPLEMENTED`), wired to `save_file(true)`, which prompts for a new name and writes a second file. | **CLOSED** | `apps/desktop/src/ribbon.rs:46-55` (`QUICK_ACCESS`, `save_as` at `:50`); `apps/desktop/src/main.rs:320` (`IMPLEMENTED`), `:520-528` (render), `:631` (dispatch); `apps/desktop/src/file.rs:62-75` (`save_file(true)`); `icons.rs:212` (icon alias) |

### Notes on each citation

- **B2** — every register citation moved but the *defect* is gone. At `dcf8382`, `model_view.rs:68`
  filtered levels with `l.0 != ElementId(10) && l.0 != ElementId(11)` and `:72,74` hard-coded
  `("Level 1", View::Plan)` / `("Level 2", View::UpperPlan)` / `"M-101 - Mechanical"`; at `b6ef0ee`
  `plan_tabs` (`model_view.rs:24-29`) names each tab from the level's own name and maps the first two
  positions to `View::Plan`/`View::UpperPlan`, with **no** id filter. `model_edit.rs:315` `plan_level()`
  returned `ElementId(10)`/`(11)` at `dcf8382`; it now returns the view's own level or the
  lowest/second-lowest document level, `ElementId(0)` when there is none (`model_edit.rs:419-424`).
  `view3d.rs:241` had `…unwrap_or(ElementId(10))`; it now takes the `Option` from `family_hit_target`
  and refuses with "Add a level first" (`view3d.rs:255-259`). `plan_export.rs:76-86` derives the level
  from the document and errors `"The project has no level: add one before exporting a plan."` The
  `"M-101"` string that survives at `plan_export.rs:84` is the **sheet view number**, not a level id.
  Every remaining `ElementId(10)`/`(11)` in `apps/desktop/src` is inside a `#[cfg(test)]` module
  (the sample model's ids in test fixtures), not a production path.
- **B2 residual — `view_name` / browser literals.** `panels.rs:1010-1012` (`view_name`) and
  `panels.rs:1023` (the `TREE` "Floor Plans" folder) still carry the literal `"Level 1"` / `"Level 2"`
  / `"M-101 - Mechanical"`. These are now **fallbacks only**: every render site resolves the real name
  through `level_view_name` (`panels.rs:981,1035,1045`), which reads `app.levels()` first
  (`panels.rs:999-1008`); the browser's extra-level list starts at `skip(2)` so it never duplicates the
  first two (`panels.rs:1085-1092`). The literals are dead unless the document has no matching level.
- **B12 residual — the last result and the electrical tables are not persisted.** `ComputeDialog::record()`
  (`compute_ui.rs:74-83`) writes every *input* but not `report` (`compute_ui.rs:63`); after reopen the
  report is `None` (`compute_ui.rs:123`). The electrical `ElecDialog` (`elec_ui.rs:44-84`) has **no**
  `record`/`restore` at all — its CSV cable tables (`table`/`grouping`/`temperature`/`harmonics`/
  `devices`/`pe_rule`, loaded at `elec_ui.rs:453`) are session-only. Neither was in the register's cited
  scope for B12 (`native.rs`, `compute_ui.rs`, `file.rs` — the thermal/hydraulic path), which is why the
  row is CLOSED on the cited claim while these two words of its prose ("cable tables", "results are not
  stored") remain literally true.
- **B13** — `tree.rs:228` (`NoSize`) is unchanged and is correct: it fires only when the resolved
  catalogue is empty, which now means the MEP Settings lists are also empty. `mep_settings_ui.rs`
  edits the four numeric lists (`pipe_slopes_pct`, `pipe_diameters_mm`, `duct_widths_mm`,
  `duct_heights_mm`) plus two name lists, and they are saved with the model (`native.rs:552-563`,
  `M|` record; restored at `native.rs:791-818`).
- **B16** — the register cited `main.rs:476`; at `b6ef0ee` the dispatch arm is `main.rs:631`
  (`"save_as" => self.save_file(true)`), the file grew 452 lines. The quick-access entry is at
  `ribbon.rs:50` (register said `ribbon.rs:46-54`, the same const). `save_as` is in `IMPLEMENTED`
  (`main.rs:320`) so its button is **enabled**, not greyed. The icon is the Save icon aliased
  (`icons.rs:212`). `save_file(true)` differs from `save_file(false)` only by prompting for a name
  (`file.rs:63-72`), then `write_model` (`file.rs:77-92`) writes a `.saving` temp and renames.

## 2. Splice candidates (residuals of the closed rows)

No row is still open, so there is nothing to splice against the register's own acceptance. These are
the small follow-on items that the closed work leaves unfinished; each is hours, and each is optional.

### S1 — `plan_level()` returns `Option<ElementId>` (B2 signature cleanup)
- **Item.** Change `plan_level() -> ElementId` to `-> Option<ElementId>`; replace the `plan_level().0 == 0`
  sentinel test at `main.rs:444` and the `View::Level(id)` reachability checks with `None`/`Some`.
- **Files.** `apps/desktop/src/model_edit.rs:419-424`; callers `main.rs:444`, `mep_tool.rs:100`,
  `modify_tools.rs:63,82`, `model_view.rs:113`. Tests at `model_view.rs:1736-1739,1763-1770`.
- **Acceptance.** A test from an empty `Document::new()` asserts `plan_level() == None` and that the
  refusal message fires; no `ElementId(0)` sentinel appears in production code.
- **Owner role.** Desktop/BIM-gap. **Deps.** none. **Risk.** low (mechanical; many call sites).

### S2 — drop the dead `"Level 1"`/`"Level 2"`/`"M-101 - Mechanical"` literals (B2)
- **Item.** The `view_name` fallback (`panels.rs:1010-1012`) and the `TREE` "Floor Plans" entries
  (`panels.rs:1023`) can be made to read the document's first two levels directly, or documented as
  intentional placeholders for a project with zero levels.
- **Files.** `apps/desktop/src/panels.rs:1010-1012,1023`; render sites `:981,1035,1045`.
- **Acceptance.** A project with two renamed levels shows the renamed names in the browser and the
  Properties "View Name"; the literals never surface.
- **Owner role.** Desktop/BIM-gap. **Deps.** none. **Risk.** low.

### S3 — persist the last compute result (B12)
- **Item.** Serialize the `SystemReport` (and `ElecReport`) as an app-record line keyed to the revision
  it was produced at, and restore it on open so the panel shows the last run without re-computing.
- **Files.** `apps/desktop/src/compute_ui.rs:63,74-83` (add `report` to `record`/`restore`);
  `apps/desktop/src/elec_ui.rs:44-84` (same for elec); `crates/document/src/compute.rs:80-91` and
  `compute_elec.rs:60-72` (`SystemReport`/`ElecReport` need a text codec — they are not `Serialize`).
- **Acceptance.** Save, reopen: the Compute panel shows the previous run's numbers (not just re-runnable
  inputs). A test asserts the restored report equals the saved one.
- **Owner role.** Desktop + document-core. **Deps.** a text codec for the two report structs (they carry
  `Vec<RunReport>`/`Vec<EndReport>`); owner decision on whether to store by value or by (revision,
  recompute-on-load) reference. **Risk.** medium (new record format, versioning).

### S4 — persist the electrical cable tables (B12)
- **Item.** Give `ElecDialog` a `record()`/`restore()` mirroring `ComputeDialog`, writing its CSV tables
  as a `U|kind=elec|…` app-record line.
- **Files.** `apps/desktop/src/elec_ui.rs:44-84` (+ record/restore); `apps/desktop/src/file.rs:231-232`
  (call the elec restore alongside `compute.restore`); `georef_ui.rs:79` (add `"elec"` to the owned kinds).
- **Acceptance.** Load a CSV cable table, save, reopen: the table is present without re-loading. A test
  round-trips a non-default `ElecDialog`.
- **Owner role.** Desktop/BIM-gap. **Deps.** none (plain text fields). **Risk.** low.

## 3. Notes / blockers

- **Counts:** `closed = 4`, `open = 0`, `rows_verified = 4`. No row is still open against its register
  acceptance. The four residuals above are optional follow-ons, not open rows.
- **No BLOCKED citation.** Every line cited above was read at `b6ef0ee` in the pinned worktree.
- **The contract's "15 commits" is wrong; `dcf8382` is 112 commits behind `b6ef0ee`.**
  `git rev-list --count dcf8382..b6ef0ee` → **112**. `dcf8382` is a real ancestor
  (`git merge-base --is-ancestor dcf8382 b6ef0ee` → YES). The register file itself did not exist at
  `dcf8382` (`git cat-file -e dcf8382:docs/gap-remediation-plan-2026-10-21.md` fails); it was added by
  `f7436e7` (21 Oct). The "verified at `dcf8382`" provenance therefore refers to the *gap analysis* and
  the *code*, not to the register as a file. The contract's *file list* matches `b6ef0ee~15..b6ef0ee`
  exactly, so the files are right and the count is not. Harmless here: all four rows were closed by
  `91eb050`/`f84104a`/`7afe07b`, all inside both windows.
- **The progress report is stale for B12/B13.** `docs/gap-progress-2026-10-23.md:66-67` lists B12 and
  B13 as "Open (not re-checked)"; that report was written at `271c774`, **before** `91eb050` landed.
  Its B2 line (`:48`, "Mostly closed") is the closest prior state and is now fully closed.
- **The shell audit agrees.** `docs/audits/2026-10-23-shell-b2-b25.md` records B12 and B13 closed with
  the named tests (`the_compute_inputs_come_back_from_the_file_and_a_new_model_starts_clean`,
  `end_points_are_placed_on_open_ends_and_compute_sizes_the_drawn_duct`), B16 closed, and B2 "refused at
  tool choice, `plan_level` keeps the 0 sentinel". Each of those tests and lines was re-read here.
- **B16 has no end-to-end test, by construction.** `save_as` opens an `rfd` file dialog, which blocks a
  headless test, so it sits in the `DIALOG` exclusion list (`ribbon.rs:387`) alongside `save`/`open`/
  `ifc_export`/`ifc_import`. The wiring is closed and covered by `write_model` round-trip tests
  (`file.rs:307-330`); the "writes a second file, first unchanged" acceptance is only reachable
  interactively. Not a defect — a test-harness limit.
- **No file outside `apps/desktop/src` and `crates/document/src` was needed** for the four rows. S3
  touches `crates/calc`-adjacent report structs only through the document core's own `compute.rs` /
  `compute_elec.rs`, both already in scope.
