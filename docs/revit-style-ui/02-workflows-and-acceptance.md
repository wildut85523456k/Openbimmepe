# 02 — Reference user workflows and acceptance tests

## WF-ARCH-01 — Walls and hosted door
Start Level 1 plan → Wall → pick points using snaps/dimensions → Door → select type and host → change wall length → inspect host opening → undo.
**Done when:** wall/door remain parametrically associated, dependent geometry updates, save/reopen preserves IDs, IFC export has correct classifications/host/opening relations.

## WF-STR-01 — Structural grid and beam
Create levels/grids → choose structural family/type → place columns and beam between reference supports → modify elevation/section → inspect schedule.
**Done when:** placements and constraints persist in 2D/3D, geometry regenerates locally; schedule and IFC structural classification work. Currently a target, not a claimed implemented feature.

## WF-MEP-01 — Place Duct (PDF reference)
Select Systems > Duct → contextual **Modify | Place Duct** appears → choose Rectangular Duct type → set Width 200 mm, Height 300 mm, Middle Elevation 2700 mm → draw path with snaps → connect AHU/terminal → insert correct elbows/branches → change type/size → undo and export.
**Done when:** duct size/elevation is consistent in properties and preview; fittings/ports carry correct semantics; changed route recalculates dependent connections; IFC port directions/system names/angle units pass validation. Sample numbers are UI fixture values, not engineering design recommendations.

## WF-MEP-02 — Edit interconnected system
Select connected Supply Air segments → change width or route → system browser identifies impacted equipment, branches, clashes, disconnected ends → accept changes or cancel.
**Done when:** no hidden disconnected endpoints; edits are atomic; system geometry and metadata agree.

## WF-COLLAB-01 — Multi-user central model
Two users open same project revision. User A edits duct A, user B edits wall B; both commit. User A and B attempt conflicting edits of hosted geometry.
**Done when:** nonconflicting commits succeed; conflicts are explicit, no silent lost updates; crash/restart reconstructs committed state; offline unsynced changes are clearly labeled.

## WF-AUTO-01 — Python and visual nodes
Select all supply-air ducts → read velocities → calculate proposed sizes → preview changed elements → request commit through BIM transaction.
**Done when:** scripts and graphs run against typed public APIs; dry-run is possible; errors identify affected elements; undo works; unsanctioned server-side filesystem/model mutations are impossible.

## Cross-cutting tests
- Keyboard/mouse Revit-style behavior, conflict resolution and override persistence.
- Heavy project viewport responsiveness and incremental dirty-element regeneration.
- Unicode project/element names (including Khmer), units and localization.
- Save/load and IFC export/import round-trip invariants.
- Recovery after abrupt client termination.
- UI state restored per user without corrupting project model.
