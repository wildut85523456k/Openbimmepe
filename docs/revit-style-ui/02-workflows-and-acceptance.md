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

## WF-COLLAB-01 — Discipline models on a shared folder
Two mechanical designers open the mechanical model on a shared folder. A borrows and edits duct A; B borrows and edits duct B; both sync. B tries to edit duct A while A holds it; then both edit a locked element. The plumbing and electrical models and the architect's model are linked to the central file, and clash detection runs there and in a standalone copy.
**Done when:** non-clashing edits arrive on both sides; editing a borrowed element is refused with its owner shown; an edit on a locked element becomes a documented change request; other clashes keep the last writer and flag the losing edit; linked models stay read only; crash/restart rebuilds the committed state; offline changes are clearly labelled. Run on a server share and on a cloud-synced folder.

## WF-AUTO-01 — Firestop records from a site inspection (OpenBIM Graph and MCP)
Read a site-inspection CSV of firestop records → match each row to its wall penetration by stable ID → preview the changed records → accept → check the IFC export → undo. Run once from an OpenBIM Graph graph and once from an MCP client.
**Done when:** both clients go through the same typed command API; values carry units; nothing changes before the user accepts; a locked wall turns its rows into change requests; errors name the affected elements; undo works; a graph or MCP client cannot act beyond its declared permissions.
Later (M6): supply-air ducts → read velocities → propose sizes from the app's calculation module → preview → commit.

## Cross-cutting tests
- Keyboard/mouse Revit-style behavior, conflict resolution and override persistence.
- Heavy project viewport responsiveness and incremental dirty-element regeneration.
- Unicode project/element names (including Khmer), units and localization.
- Save/load and IFC export/import round-trip invariants.
- Recovery after abrupt client termination.
- UI state restored per user without corrupting project model.
