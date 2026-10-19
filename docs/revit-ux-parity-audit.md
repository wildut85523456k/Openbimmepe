# Revit UX parity audit (2026-10-18, updated at commit 8af490b)

Standing rule: every tool mimics Revit's user experience. Verified by automated event tests (simulated mouse and keys) and by reading the code. Not verified side by side with Revit, and not clicked by hand.

## Working tools
Wall, Wall Joins, Door, Window, Component, Column, Room, Roof, Ceiling, Stair, Shaft Opening, Wall Opening, Beam, Brace, Floor/Slab, Foundation, Wall Footing, Beam System, Truss, Rebar, Level, Grid, Tag, Aligned Dimension, Measure, Duct, Pipe, End Point, Electrical Node, Cable, Compute, Move, Copy, Rotate, Mirror, Align, Offset, Trim/Extend, Delete, IFC import/export, Select, Zoom in/out, Project Parameters, Automation items.

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

## Room
| Revit behaviour | State |
|---|---|
| Room tool: hover shows the closed space under the pointer with its area, click places a room there | done (outline preview in blue, "not enclosed" in red) |
| Boundary = faces of walls (and columns as holes), worked out from the walls each time, so the room follows moved, edited or deleted walls | done (mitred and T corners, split edges, opposite edges cancelled) |
| One room per space; a second click says so | done |
| Room not enclosed warning | done: the room shows "Not Enclosed" in red when its walls no longer close |
| Name and area shown at the room point; tinted floor; selectable, movable, deletable, undoable | done (label is drawn with the room; no separate Room Tag, no editable name/number/Department yet) |
| Saved in OpenBIM records and the automation snapshot | done; not exported to IFC (no IfcSpace yet) |
| Room separation lines, Area/Room schedules, Place Room Automatically, room bounding of floors, ceilings, doors | missing |

## Roof (by footprint)
| Revit behaviour | State |
|---|---|
| Roof by Footprint: click the corners at the eaves, close with Enter / first corner / Finish; options bar Base Offset, Thickness, Slope | done (the ribbon Roof button uses the slab drawing flow; no Pick Walls, no Overhang, no Pick Lines, no arc/circle sketch lines) |
| Every edge "Defines Slope" by default; each can be turned off in Properties | done: checkbox per edge in Properties; slope in degrees; changes are immediate and undoable |
| Hip, gable (two opposite edges), shed (one edge), flat (none) from the same rule: the roof is the lowest of the planes rising from the sloping edges | done for convex outlines; a non-convex outline (L, U, T) with every edge sloping gets hips and valleys from a straight skeleton (up to 64 corners; the faces are several coplanar trapezoids, so extra lines show in plan). A non-convex outline with some edges not sloping (gable on an L) is still refused with the reason |
| Thickness square to the slope | done (not checked against Revit) |
| Roof 3D solid, plan drawn under walls and picked last, moves/rotates/mirrors keeping the right edges sloping, saved, undo | done |
| Derived figures: surface area, plan area, rise above the eaves | done (Properties) |
| IFC: IfcRoof with HIP/GABLE/SHED/FLAT_ROOF and a closed faceted solid in IFC2X3, IFC4 and IFC4.3; the app's own file brings it back unchanged | done; a roof from another program is not read (reported as skipped); no layers, material layers or roof types |
| Roof by Extrusion, Roof Soffit, Fascia, Gutter, Join/Cut Roof, Attach Walls to Roof, Dormer, slope arrows, ridge/hip/valley editing, roof openings | missing |

## Elevations and sections (done 2026-10-19, read-only)
| Revit behaviour | State |
|---|---|
| Four building elevations (East, North, South, West) under Elevations in the Project Browser, with level lines and names/heights | done: walls, slabs, roofs, stairs, openings in slabs, families; hidden-line order far to near; no elevation markers in plans (the ribbon Elevation button opens East) |
| Section tool: two picks in a plan (click or press-drag-release), the view looks to the left of the line, tick shows the side; listed in the Project Browser and as a tab; Properties: View Depth, Open view | done; saved, undoable; section heads/bubbles, flipping, split segments, cut-plane offsets and crop regions are missing |
| Cut elements are filled dark, below/overhead/beyond are lines and tints, levels are dashed lines with labels | done (line weights are fixed, no view templates or detail levels) |
| Select by click in an elevation or section (walls, slabs, roofs, stairs, columns, beams, doors/windows, wall openings; the element turns blue; Properties edits it; Delete removes it with what it hosts) | done (2026-10-19); picking is by the drawn shapes, so a wall seen through a hole is not picked; level lines are not picked |
| Wall openings drawn in elevation | done: the opening is a hole in its wall in the view (walls parallel to the view only; a wall seen end on has no opening drawn) |
| Wall Opening tool in an elevation or section: click one corner on a wall, then the opposite corner; the rectangle becomes the opening (sill and head from its heights, along the wall from its width) | done; the wall must be parallel to the view; the rubber band shows the rectangle; it does not snap to other edges and the options-bar sill/head values are not used in this mode |
| Plans: section line with a head bubble (its number) at the start, a tick at the other end and an arrow for the side it looks to; Space or Properties > Flip direction turns it round (undoable); a double click on the line opens its view | done (2026-10-19); no flip control drawn on the line, no split segments or gaps. Select a section and drag the grip at either end: only that end moves (undoable, its view follows) |
| Plans: an elevation marker (circle with a pointer toward the building and the side's name) outside each of the four sides; a double click opens that elevation | the four building markers are worked out from the building (2 m outside it, 30 px off the line) and cannot be moved or deleted. See the next row for markers you place |
| Elevation tool: click in a plan to place a marker (circle with a pointer per view); select it to tick East / North / South / West on or off in Properties (at least one stays); each view is a tab and a Project Browser entry ("Elevation 1 - East"); double click a pointer opens it; drag the circle to move it; Delete; View Width and View Depth in Properties | done (2026-10-19), saved and undoable, not exported to IFC. A click places all four views (Revit starts with one and a rotation by the pointer); the marker cannot be rotated, the view is a straight cut 20 m wide and 20 m deep unless edited, and the pointers are not grips to drag |
| Press on an element in an elevation or section and drag: it moves along the view and up or down in 50 mm steps, with a ghost and the distances; Esc cancels; undo. Doors, windows and wall openings slide in their wall and change their sill (the model refuses a move out of the wall, with the reason); walls, beams, slabs and stairs also change their height offset; a column, footing or anything else moves sideways only (up and down alone says to use Properties); what stands in a moved wall goes with it | done (2026-10-19); only along the view (nothing in depth), a wall at an angle to the view cannot slide what stands in it, no Move tool with a typed distance, no snapping to other edges |
| Rotate, copy, mirror, align, tags, dimensions, drawing walls/floors in these views; callouts; legends | missing: other tools say they work in plan views |

## Ceiling
| Revit behaviour | State |
|---|---|
| Automatic Ceiling: click in a closed space, the ceiling takes its outline; the outline shows under the pointer | done (the outline is copied when placed: it does not follow the walls afterwards, as Revit's does; columns in the space are not cut out) |
| Sketch Ceiling: untick Automatic in the options bar, click the corners, Enter / first corner / Finish | done |
| Height Offset from Level (default 2700 mm) is the underside; thickness (default 20 mm) goes up from it | done (defaults are mine, not checked against Revit's template) |
| One ceiling per space and height | done (a second click says so) |
| Ceiling type with layers/materials, Ceiling Plan view (reflected ceiling plan), ceiling grid, slope, openings | missing; in a plan the ceiling is drawn faint under everything and picked last |
| IFC: IfcCovering CEILING (a room's ceiling for space boundaries), back from the app's own file unchanged | done; a covering that is not a ceiling is skipped on import |

## Stair (by component, one straight run)
| Revit behaviour | State |
|---|---|
| Stair > Run > Straight: click the foot of the first riser, then the direction; the number of risers follows from the height between the base and the top level and the Maximum Riser Height; treads are drawn as you move | done (press-drag-release works; Shift keeps it square) |
| Options bar: Width (1000), Max Riser Height (175), Tread Depth (280), Base Offset; Height when no level is above | done (defaults are mine, not checked against Revit's template) |
| Plan symbol: arrow up the centre line with UP at the foot | done (no cut line or break line, no riser/tread numbering) |
| 3D: a monolithic stepped block; the top riser is the edge of the floor above | done (no soffit slope, no stringers, no supports, no nosing) |
| Properties: Width, Total Rise, Base Offset editable; risers, treads, actual riser height and tread depth, run length shown | done (riser count is not editable: place the stair again) |
| Move / rotate / mirror / copy / delete / undo, saved | done |
| IFC: IfcStair STRAIGHT_RUN_STAIR with the side profile extruded across the width and Pset_StairCommon, in all three schemas; the app's own file brings it back unchanged | done; a stair written by another program is not read (skipped) |
| Winder, L and U shapes, landings, runs from sketch, stair by sketch, railings, top/base level choice, ramps | missing |

## Opening (Wall, Shaft; done 2026-10-19)
| Revit | This app |
|---|---|
| Architecture > Opening panel: By Face, Shaft, Wall, Vertical, Dormer | the five buttons are there; Shaft and Wall work, By Face, Vertical and Dormer say they are not implemented. All five use my placeholder copy of the opening icon (the approved set has one icon) |
| Wall Opening: pick a wall, sketch a rectangle (in an elevation or section view) | done in a plan view (click one end on a wall, then the other end on the same wall) and in an elevation or section view (two opposite corners); the sill and head heights are in the options bar (900 / 2100 are my defaults). The opening is a rectangle and follows its wall; deleting the wall deletes it |
| Wall Opening in 3D | done: the wall is cut (same cut as a door or window) |
| Wall Opening in plan | dashed jambs across the wall; it can be picked on them; the wall's own plan lines are not broken |
| Shaft Opening: sketch a boundary, Base Constraint / Base Offset / Top Constraint / Top Offset, Finish | done: click the corners (snapped to 50 mm), Enter / first corner / Finish closes; Base is the plan's level, Top defaults to the level above and is chosen in the options bar ("Up to level"), both offsets are editable there and in Properties afterwards |
| A shaft cuts every floor, foundation slab and ceiling it passes through | done, by height: a slab is cut when its thickness overlaps the shaft's range (so a shaft from Level 1 to Level 2 cuts the floor of Level 2, not the floor of Level 1). A shaft that overlaps only a corner of a slab notches it. Roofs are not cut yet |
| Shaft in plan | dashed outline crossed by two lines on every level it passes through; picked on its symbol |
| Shaft in 3D | the slabs have the hole or the notch; the shaft itself is not drawn |
| Move / rotate / mirror / copy / delete / undo, saved | done |
| IFC: IfcOpeningElement + IfcRelVoidsElement for the wall opening (in its wall) and for each shaft in each slab it cuts (the file keeps the whole slab outline and the opening is the part of the shaft over the slab, so Revit does the cut), all three schemas; slab quantities take the voids off; the app's own file brings every opening back unchanged | done; an opening written by another program is not read |
| Vertical opening (cuts one floor, roof or ceiling you pick), By Face, Dormer, openings in curved walls, arc / polygon / circle sketch of a shaft, shaft symbol options, a roof cut by a shaft, a slab split in two by a shaft (only the larger piece is drawn in 3D; the file is right) | missing |

## Still missing for every element
Split Element. The Offset and Wall Joins icons are placeholders I drew (the approved icon set has none): replace them with approved art. Level 1 and Level 2 are fixed tabs.

## Not working at all
All MEP fittings and equipment placement buttons except Duct/Pipe/Component, cable tray, conduit, lighting, devices, curtain wall, railing, ramp, area, opening by face / vertical / dormer, detail, callout, view templates, link model, clash checks, worksets, phases; dimension types other than Aligned.

## Suggested order
1. (Room, Roof, Ceiling, Stair, Opening, elevations/sections, selecting, wall openings and moving things in them, the Elevation tool and dragging section ends are done.)
2. Railings, L/U stairs, vertical opening and roof cuts.
3. Layered wall types, polygon/arc wall draw modes, Butt/Miter configuration.
4. Associative dimensions.

The 3D view (editing in 3D, the orbit control at the top right, GPU needs) is its own topic: `docs/3d-view-parity-plan.md` (project doc `claude/3d-view-parity-plan.md`). To be cleared at a later stage, on the owner's request of 2026-10-19.
