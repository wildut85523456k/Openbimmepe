# Revit UX parity audit (2026-10-18, updated at commit 8af490b)

Standing rule: every tool mimics Revit's user experience. Verified by automated event tests (simulated mouse and keys) and by reading the code. Not verified side by side with Revit, and not clicked by hand.

## Working tools (47 of 127 ribbon commands)
Wall, Wall Joins, Door, Window, Component, Column, Beam, Brace, Floor/Slab, Foundation, Wall Footing, Beam System, Truss, Rebar, Level, Grid, Tag, Aligned Dimension, Measure, Duct, Pipe, End Point, Electrical Node, Cable, Compute, Move, Copy, Rotate, Mirror, Align, Offset, Trim/Extend, Delete, IFC import/export, Select, Zoom in/out, Project Parameters, Automation items.

## Select-mode editing (done 2026-10-18)
| Revit behaviour | State |
|---|---|
| Multi-select: Ctrl or Cmd click adds, Shift click takes away | done |
| Selection box: left to right selects what it holds (solid), right to left what it touches (dashed); Ctrl/Cmd or Shift keeps the current selection | done; the properties panel shows the first element only; no Select All Instances, no filter |
| Delete key (and ribbon Delete) removes the selection with its doors, windows, tags and footings; one undo restores all | done; levels cannot be deleted |
| Move: base point then second point, or press-drag-release; typed distance + Enter; Shift = square; ghost preview | done, on the whole selection |
| Copy: stays active, each pick makes a copy of the originals; the copies are selected | done |
| Rotate: centre, start ray, end ray, or typed angle + Enter | done; columns, footings and slabs only by multiples of 180 degrees |
| Mirror: draw axis (two points), Copy option on by default | done; Pick Axis missing |
| Click an element when none is selected inside Move/Copy/Rotate/Mirror | done |
| Drag a selected or unselected element to move it (all selected move together) | done |
| Wall end grips: drag an end, joined walls follow | done; no flip arrow |
| Temporary dimensions on a selected wall: its length, and the distance to the nearest parallel wall, beam or grid on each side; click a number, type another, Enter: the wall changes in one undo step | done for one selected wall; no dimensions to ends or to perpendicular walls, no "convert to permanent", no witness-line grips |
| Spacebar on a selected wall flips it (ends swap, doors keep their place) | done |
| Joined duct or pipe run | refuses to move, with a message |
| Trim/Extend to Corner: two walls or beams, each picked on the part to keep | done; no Trim to single boundary, no multiple trim |
| Align: reference then element; reference stays until Esc | done for walls, beams, grids on their lines (axis to axis, not face to face); no Multiple Alignment, no lock |
| Offset: distance in options bar, side of the click, Copy option (off by default, as in Revit) | done, numerical mode only; no graphical mode; a copied wall also copies its doors and tags; no ghost preview |

## Annotation
| Revit behaviour | State |
|---|---|
| Aligned Dimension: point, point, then the place of the line; saved with the model, selectable, movable, deletable, undoable | done as point-to-point (snaps to wall corners, grid ends, 50 mm). Not associative: it does not follow the walls when they move. No chains, no baseline, no angular, radial or other dimension types, no editing the value, no equality |
| Measure between two points: distance, x and y parts, angle | done; no Measure Along an Element |
| Tag | done for any element; cannot be selected or moved by itself |
| Dimensions and tags in exports | not exported (kept in the OpenBIM records only) |

## Wall tool vs Revit
| Revit behaviour | State |
|---|---|
| Click start, click next, chain; Esc ends chain, Esc again leaves tool | done |
| Press-drag-release draws one wall | done |
| Location Line list (centerline, core, finish exterior/interior) | done (core = finish: one layer only) |
| Exterior on the left when drawn clockwise | done |
| Mitred corners, T-walls stop at the face | done (plan, 3D, IFC) |
| Chain meets at offset-line corners; clicking first point closes the loop | done |
| Shift = orthogonal; typed length + Enter | done |
| Snap to endpoints, grid ends/crossings, grid lines; marker | done (no midpoint, perpendicular, extension) |
| Options bar: Height, Base offset, Chain | done; no Offset field, no Level dropdown, no Unconnected/To-level height |
| Wall type selector | done: list of wall types, Custom, Add type from the thickness and height; a type holds thickness and height only (no layers, no material) and starts empty (no ready-made types) |
| Draw mode Rectangle: two corners, four joined walls in one undo step, exterior face on the picked rectangle | done |
| Draw mode Pick Lines: click a grid, wall or beam; the clicked side is the exterior | done (no Pick Faces) |
| Draw modes polygon, circle, arc | missing |
| Spacebar flips the wall while drawing | done (moves the axis to the other side of the drawn line) |
| Listening dimensions with angle while drawing | length only |
| Wall Joins: Allow / Disallow Join at a wall end (the end stays square, and so does the wall that met it); saved, undoable, round-trips through IFC with OpenBIM data | done; markers green (allowed) and red (disallowed) |
| Wall Joins configuration Butt / Miter / Square off per end | missing (the join is chosen automatically: miter for two walls, butt for a T) |
| Join Geometry / Unjoin Geometry (solids cut by each other) | missing |

## Still missing for every element
Split Element. The Offset and Wall Joins icons are placeholders I drew (the approved icon set has none): replace them with approved art. Level: not drawn in an elevation view (no elevation or section views). Level 1 and Level 2 are fixed tabs.

## Not working at all (80 ribbon buttons)
All MEP fittings and equipment placement buttons except Duct/Pipe/Component, cable tray, conduit, lighting, devices, roof, ceiling, curtain wall, stair, railing, ramp, room, area, opening, detail, section, elevation, view, link model, clash checks, worksets, phases; dimension types other than Aligned.

## Suggested order
1. Room, Roof, Ceiling, Stair, Opening.
2. Elevation/section views and levels drawn in them.
3. Layered wall types, polygon/arc wall draw modes, Butt/Miter configuration.
4. Associative dimensions.
