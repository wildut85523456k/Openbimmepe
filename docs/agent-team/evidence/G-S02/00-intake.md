# G-S02 — Intake: IFC export P1–P3 + round-trip tests

## Source
- Eval branch: `eval/external-bim-repos-2026-10-26` @ `2db9e391` (test: round-trip export→import for P1/P2/P3 entities)
- Worktree: `wt-g-m02` off `origin/update-core-deps` @ `a9b8689f`
- Gap register: G-S02 — IFC export P1–P3 (LoadBearing, IfcBeam/IfcRoof writers, footings, stairs, railings, masses, material colour, Pset_ProjectParameters) + round-trip tests

## Objective
Port eval-branch P1/P2/P3 IFC export features to openbim-core:
1. `FootingItem` + `write_footings` (IfcFooting)
2. `MassItem` + `write_masses` (IfcBuildingElementProxy + BIMCraft_Mass pset)
3. `project_parameters` field + Pset_ProjectParameters export
4. Round-trip export→import test

## Implementation
- Added `FootingItem` and `MassItem` structs to core ifc crate
- Added `project_parameters`, `footings`, `masses` fields to `Scene`
- Added `footings`/`masses` counters to `Report`
- Added `write_footings` and `write_masses` functions
- Added Pset_ProjectParameters export on IfcProject
- Updated all Scene construction sites in tests/examples
- All existing ifc tests pass (29 + 16 + 11 + 14 + 25 + 3 + 5 + ...)
