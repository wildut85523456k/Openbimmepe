# Spatial model and document-driven views — 16 October 2026

## Implemented

The in-memory document owns level elevations, grid endpoints, and linear element
axes with explicit reference-level IDs. Walls, ducts and pipes derive their meshes
from their current parameters. Metres are canonical; inspectors display mm and %.
Plan, Level 2, Mechanical and 3D views read the same document and stable element IDs.
The old painted rooms/AHU illustration and its empty-space room selection are gone.

The sample contains two levels, six grids, ground-floor walls, an upper-level wall,
a rectangular duct and a round pipe. It is initialized through document commands;
initialization is not part of user Undo history. It is recreated on app launch.

## Interaction

- Click actual faces, grid bubbles/axes or 3D level datums to select an element.
  Click empty canvas or Clear selection to clear it. Tab cycles document elements.
- The Level button in a plan selects that view's reference level.
- Properties and the contextual options bar edit only applicable parameters:
  wall thickness/height/base offset; pipe diameter/elevation/slope; duct rectangular
  size/elevation/slope; level elevation; grid start/end coordinates.
- Location in Properties also edits wall, duct and pipe endpoints. Run length and
  end elevation are derived from committed geometry and shown read-only.
- Apply commits once and redraws both views. Reset discards the draft. Switching
  selection resets unapplied values and reports it. Undo restores the whole batch.
- Moving a level moves all elements that reference it. A referenced level cannot
  be deleted while its dependent elements remain in the document.
- Middle-drag pans; wheel zooms; Fit resets pan/zoom; right-drag orbits in 3D.
- Level 1/Level 2 plan tabs and browser entries filter linear elements by base level.
  Grids are vertical datums represented at the active plan elevation.

## Geometry and validation

Walls and ducts use rectangular prisms; pipes use 16-sided circular prisms.
Slope is signed rise divided by horizontal run, from start to end: 2% rises 120 mm
across the sample 6 m run. Duct/pipe cross-sections remain perpendicular to the
sloping axis. Wall elevation is a base offset; service elevation is its centerline
at the start. Round spatial pipes require equal width/height diameter parameters.

Spatial commands validate category compatibility, finite/limited coordinates,
nonzero axis length, reference-level existence and finite resulting elevations.
Validation examines the completed transaction, allowing datums and dependents to
be created or removed together while rejecting broken references atomically.

## Current renderer scope

The Rust/egui renderer projects document meshes orthographically and sorts faces
by depth. It is intended for this small sample and is not a full depth-buffered
GPU BIM renderer. Complex intersecting faces can display incorrectly. Plan views
show footprints of elements assigned to the active base level; full view ranges,
cut planes, overhead rules, hidden-line removal, wall joins/openings, layers,
slabs and automatic connections are not implemented. Grids and levels are simple
straight datums. Model creation tools, grips and temporary on-canvas dimensions
remain future work. No geometry cache or large-model performance claim is made.
File saving, IFC integration and central-model collaboration remain unconnected.

## Review

1. Open Level 1, select a wall, change thickness from 200 to 400 mm and Apply.
2. Open 3D; right-drag to orbit. Change wall height to 4000 mm, Apply, then Undo.
3. Select Level 1 with the plan's Level button, raise it by 1000 mm, Apply, and
   check its walls and services move in 3D. Undo.
4. Select a grid bubble; edit its endpoints and check the axis/bubbles move.
5. Select the duct or pipe, set slope to 2%, Apply, and inspect in 3D. The derived
   end elevation should rise by 120 mm on a 6 m run. Check Undo.
6. Switch Level 1 and Level 2 plans; confirm upper-level walls are separate.

Tests cover parameter-to-mesh changes, level dependencies, slope rise, atomic
invalid-geometry rollback, datatype restrictions, category-specific UI edits,
Undo and polygon picking. Headless plan/3D render and tessellation checks cover
both themes at 100%, 150% and 200%. Software diagnostic previews were inspected;
interactive native-app review is still required.

Verification: all Windows workspace targets passed cargo check. The desktop suite
passed 9 tests and document suite passed 11. These checks used the existing local
Windows dependency-lock correction, which this commit leaves unchanged.
