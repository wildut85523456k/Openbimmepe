# IFC honesty bundle (B7, B8, B9, B10, B11, B14, B15, B19) - what changed, what is proven, what is left

Branch `feat/ifc-export-honesty`, on `main` at `4f24b00`. Gap ids are those of `docs/gap-remediation-plan-2026-10-21.md`.

## Changes

| Gap | Change | Where | Proof |
|---|---|---|---|
| B7 | A duct or pipe is given the system the tools recorded for it (`MepData.run_systems`) instead of `System::Other` | `apps/desktop/src/export.rs` | `a_duct_through_a_wall_cuts_an_opening_and_a_straight_run_keeps_its_system`: the group of the run's own kind holds one more piece after a straight duct is drawn |
| B8 (grids) | New `Scene.grids` / `GridItem`; one `IFCGRID` with an `IFCGRIDAXIS` per line (vertical lines U, horizontal V, all U when only one direction), contained in the lowest storey. IFC4, 4.3 and 2X3 | `crates/ifc/src/lib.rs`, `export.rs` | `crates/ifc/tests/grids.rs` (5 tests) |
| B9 | New `Scene.system_defs` / `SystemDef`; each named system is its own `IfcDistributionSystem` with the user's name, holding its equipment and every run connected to it. What no named system claims stays in the group of its kind. A system whose equipment is not joined to a run still exists. Ids come from the name | `crates/ifc/src/lib.rs`, `export.rs` | `tests/grids.rs::systems` (4 tests) and `a_named_system_leaves_the_file_under_its_own_name...` |
| B10 | Walls: the runs' crossings are found with `openbim_place::penetrations::find` and cut into the wall chain (an opening that would break the wall is left out and its record says `cut = false`). Floors: `find_floor` result passed on. Both are registered in the file | `export.rs` | `a_duct_through_a_wall_cuts_an_opening...`: `IFCOPENINGELEMENT` count rises |
| B11 (part) | A project has a persisted 48-bit key (`Document::project_key`, record `Y\|key=` in the `.obim` file, made on first save or export, carried by view copies). Export uses it instead of the constant 1 and names the project after the file. Storeys and named systems take their GlobalIds from their names, not from a counter | `crates/document`, `file.rs`, `model_edit.rs`, `ifc/src/lib.rs` | `a_project_keeps_its_key_in_its_file_and_two_projects_differ`, `the_global_ids_of_the_same_project_do_not_move_when_a_level_is_added` |
| B14 | Marking a Source on an end joined to equipment keeps the join | `compute_ui.rs` | assertion added to the equipment-end test in `elec_ui.rs` |
| B15 | The Air preset sets the velocity limit 8 m/s and friction limit 1 Pa/m (the Water preset restores 1.5 m/s and none); choosing a duct system after a pipe one (or back) applies the fitting preset. The values are common low-velocity duct criteria and are editable - **owner to confirm** | `compute_ui.rs` | `the_air_preset_brings_air_limits...` |
| B19 | New `Scene.assemblies` / `AssemblyDef`; a truss or beam system is an `IfcElementAssembly` (`.TRUSS.` or `.BEAM_GRID.`) aggregating its members; the members leave the storey's contained list | `crates/ifc/src/lib.rs`, `export.rs` | `tests/grids.rs::assemblies` (3 tests) |

## Test evidence (debug build, 2 cores)

- `openbim-ifc`, `openbim-view`, `openbim-viewport`: all pass. `cargo clippy -p openbim-ifc -p openbim-desktop --all-targets -- -D warnings`: clean.
- `openbim-desktop` (bin): 195 of 196 pass (1 ignored as before) after three existing tests were updated, see below.
- **Built with a local stand-in for the `edesign-alike` electrical module, which the authoring sandbox cannot fetch.** Because of that stand-in 9 electrical-stamp tests in `openbim-document` (`schematic_view::tests::*`) fail locally; they are unrelated to this change and must be run in CI with the real dependency.

## Existing tests that changed, and why

Real service crossings now cut openings, and the sample model already has ducts through its wall and floor. Three tests counted openings exactly and now leave out the service openings (named `W<n>-S<n>-P<n>` / `F<n>-P<n>`) via a test helper; the "nothing lost" import test also expects the new grid and assembly entities as unread classes (the native record restores them).

## Not done

- **Rooms (B8)** are still not exported: they need a conversion from the document's room shape to `openbim_arch::Room` (level index, outline direction, height).
- **Georeference** is still `None` in the scene.
- **B11, remainder:** relationships, property sets and the site/building still take GlobalIds from a running counter, so they shift when products are added. The fix is to derive them from the element they belong to.
- **B9, remainder:** Compute still works per kind of system, not per named system (B18).
- Doors/windows hosted in a wall that a duct also crosses are not checked against the duct opening for overlap.
