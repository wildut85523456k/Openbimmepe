> Historical evidence: this report describes its stated baseline and contains superseded status claims. See [Current capability register](capability-register.md) for the 25 October 2026 source review and explicitly pending validation. Do not use this report's scoreboard as current completion status.

# Revit UX parity audit (2026-10-18, updated 2026-10-19)

Standing rule: every tool mimics Revit's user experience. Verified by automated event tests (simulated mouse and keys) and by reading the code. Not verified side by side with Revit, and not clicked by hand. **[superseded 2026-10-21 — R5 register: the automated event tests now exercise the ribbon (ribbon_click, apps/desktop/src/model_view.rs:1406), the options bar (the_options_bar_and_the_window_show_the_lists_of_the_model, mep_settings_ui.rs) and the global keys (key_frame, model_view.rs:1416); the reading that they omit the ribbon, options bar or global keys is stale]**

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
| Redo (Ctrl+Y, also Ctrl+Shift+Z; the quick-access button) | done: undo puts what it took on a redo list, any new edit empties it, as in Revit; each takes a revision |
| Select All Instances (Modify > Select) | done for the same type, in the whole project (Visible in View exists in the document call but has no button yet; no right-click menu) |
| Cut, Copy, Paste (Ctrl+X, Ctrl+C, Ctrl+V; Modify > Clipboard) | done in plan views: the clipboard keeps the data, so a cut can be pasted; the pasted elements follow the pointer by their centre with an orange ghost and a click puts them down (tool stays on, Esc ends), doors, windows and tags go with their wall, a copy is not pinned, one undo takes a paste back. A door, window or tag alone cannot be copied (select its wall too) |
| Paste Aligned to Same Place | done: same position, on the level of the open plan view (so it also copies a floor plan to the other level). Aligned to Selected Levels / Selected Views / Current View: missing; paste in elevations, sections and 3D: missing |
| Match Type Properties (Modify > Clipboard) | done for elements that have a type (walls, ducts, pipes, ...) and for family instances of one family: click the source, then each target; stays on, Esc ends. Plan views only; an element with no type cannot be a source (make a type first) |
| Pin / Unpin (PN, UP; Properties "Pinned") | done: a pinned element cannot be moved, rotated, mirrored, trimmed, dragged by a wall end, aligned or deleted (nor can a wall joined to a pinned wall end be dragged); a pushpin is drawn on it in the plan; saved with the model. Revit's exact rule for deleting a pinned element was not checked (here it is refused). The PN and UP shortcuts are not bound |
| Create Similar (Modify > Create) | done for walls (type), ducts and pipes (size, elevation, system), components, columns, beams, beam systems, trusses, footings, slabs/roofs/ceilings (kind), railings, stairs, rooms, grids, rebar, levels: it chooses the tool that made the element with its type and sizes; the other values of the tool are the tool's own defaults |
| Joined duct or pipe run | refuses to move, with a message |
| Place Duct / Place Pipe options (MEP books ch5, ch7): Automatically Connect, Inherit Elevation, Inherit Size, Horizontal and Vertical Justification, Tag on Placement | done in the options bar. Automatically Connect off draws a plain run (no branch tee, no connector start, no join at the end). Inherit Elevation (on by default, as in Revit) and Inherit Size take the height and the size of the connector or the run the first click is on; with Inherit Elevation off a run that leaves a connector goes to the bar's elevation along its first leg (an inclined leg, no separate vertical riser) and cannot change size at the same bend. Horizontal Justification (Left/Center/Right of the drawn line) and Vertical Justification (Top/Middle/Bottom of the elevation) apply to plain runs only, not to runs that leave a connector or branch from a run, and a run with several sizes is justified by its widest size (reducers are concentric). Tag on Placement tags every straight run in its own undo step after the run. Not done: Offset lock, duct/pipe type selector, Reference Level box, Cap Open Ends |
| Mechanical Settings (Manage > MEP Settings): the lists the tools offer | done for four lists: pipe slopes (%), pipe diameters, duct widths and duct heights (mm). New / Delete / Defaults / OK / Cancel; OK cleans the lists (out-of-range values dropped, sorted, each once) and makes one undo step. The lists are kept with the model: saved in the native file, written into the IFC file's own data and read back by this app with nothing lost (a model that already has its own lists keeps them on import). The options bars offer them (a ▾ list next to Width, Height and Diameter, and the slope list); any other size can still be typed. Not done: round and oval duct sizes, pipe segments/materials with their own size tables and roughness, fluids (temperature, viscosity, density), angles and conversion settings, electrical settings (voltage, distribution), the Revit dialog's other tabs. The slopes are not yet used by anything but the Pipe tool |
| Insulation and Duct Lining (Modify | Ducts / Pipes: Add, Edit, Remove Insulation; Duct Lining) | done for ducts, pipes and their fittings: select runs, press Insulation on the Duct or Pipe panel, pick a type (from Manage > MEP Settings > Insulation Types / Duct Lining Types), a thickness (1 to 200 mm), Add / Edit or Remove; the fittings joined to the runs can go with them. One undo step each. Insulation is drawn in the plan as an outline round the run, and in the 3D view as its own mesh (ducts and pipes); lining (ducts only) is kept and shown in Properties but not drawn. Saved with the model and read back from the IFC file by this app. Not done: insulation as geometry in the IFC file for other programs (it travels only in this app's own data), fittings are not drawn in the 3D view at all yet so their insulation is not seen, insulation on equipment, a separate Edit Type dialog with a material and cost, thermal use (heat gain, lining reducing the free area), copy/paste of an insulated run does not carry its insulation, the type names have no material yet |
| Create Mechanical System / Piping System: select equipment, choose the type and a name (default: the type and a number); Edit System (Add to / Remove from System); System Browser listing every system with its equipment and whether each connector is joined | done 2026-10-19 (Systems tab, View > Windows): one system of each type per piece of equipment, saved with the model and in the IFC file's own records, one undo step each. Missing: System Inspector (flow arrows, section tag), IfcSystem groups for other programs (the grouping travels only in this app's own records), electrical systems, adding a duct or pipe (not only equipment) to a system, the system of a run shown in its properties |
| Generate Layout for a system: Solution Type, numbered solutions shown as placeholders, the arrows step through them, Finish Layout makes the real ducts or pipes | partly: Network (a trunk along X or along Y beside the equipment, the others tapped off it with tees; trunk takes the size of the largest connector, or a size typed in the window, branches the size of their connector) and, for two connectors, a Routed solution from `openbim_route::auto_route` that goes round walls (soft zones; crossing one costs). Each solution is tried out on a copy of the model first, so what is offered can be built; connectors left out are listed with the reason. Finish Layout is one undo step, joins the ends to the equipment connectors and selects what it made. NOT as in Revit: no Perimeter or Intersections type; no Modify Base / Place Base on the plan (the base is chosen in the System Browser); placeholders are drawn dashed but are not saved or editable, Revit keeps them as elements until Convert; equipment is not an obstacle; connectors must face along an axis, sideways, and be at the height of the base's connector (the model holds no vertical ducts or pipes yet), others are left out with the reason. The algorithm is mine, Revit's is not published |
| Pipe Slope (Off / Up / Down, value list, Show Slope Tooltip) | done for plain pipe runs: the pipe falls (Down) or rises (Up) along the way it is drawn by the percent chosen; the tooltip next to the pointer gives the percent and the height change over the leg. The list of values (starting with 0.2, 0.25, 0.5, 1, 1.5, 2, 3, 4, 5, 10 %, chosen here) is edited in Manage > MEP Settings (see the next row). Slope is not applied to runs that leave a connector or branch from a run |
| Editable end offsets of a selected duct or pipe (Properties > Run Ends: Start Offset, End Offset) | done in Properties (the book shows them on the canvas; here they are in the palette): the end goes to the typed height above the level, the slope follows, one undo step. An end joined to a fitting, equipment or another run, a run with a branch on it, and a pinned run are refused with a message (move the fitting instead) |
| Trim/Extend to Corner: two walls or beams, each picked on the part to keep | done; no Trim to single boundary, no multiple trim |
| Align: reference then element; reference stays until Esc | done for walls, beams, grids on their lines (axis to axis, not face to face); no Multiple Alignment, no lock |
| Offset: distance in options bar, side of the click, Copy option (off by default, as in Revit) | done, numerical mode only; no graphical mode; a copied wall also copies its doors and tags; no ghost preview |

## Annotation
| Revit behaviour | State |
|---|---|
| Aligned Dimension: point, point, then the place of the line; saved with the model, selectable, movable, deletable, undoable | done as point-to-point (snaps as in the plan snap row below). Not associative: it does not follow the walls when they move. No chains, no baseline, no angular, radial or other dimension types, no editing the value, no equality |
| Measure between two points: distance, x and y parts, angle | done; no Measure Along an Element |
| Tag | done for any element, in plans, elevations and sections (a tag belongs to the view it is placed in); cannot be selected or moved by itself in the plan |
| Dimensions and tags in exports | not exported (kept in the OpenBIM records only) |

## Wall tool vs Revit
| Revit behaviour | State |
|---|---|
| Click start, click next, chain; Esc ends chain, Esc again leaves tool | done |
| Press-drag-release draws one wall | done |
| Location Line list (centerline, core, finish exterior/interior) | done: for a layered type the core lines are where the core's faces are (not in the middle when the core is not), a plain type's core is its finish |
| Exterior on the left when drawn clockwise | done |
| Mitred corners, T-walls stop at the face | done (plan, 3D, IFC) |
| Chain meets at offset-line corners; clicking first point closes the loop | done |
| Shift = orthogonal; typed length + Enter | done |
| Snap to endpoints, grid ends/crossings, grid lines; marker | done: endpoint, intersection, midpoint, perpendicular and nearest of walls and grids (no extension, no tangent) |
| Options bar: Height, Base offset, Chain | done; no Offset field, no Level dropdown, no Unconnected/To-level height |
| Wall type selector | done: list of wall types, Custom, Add type from the thickness and height; starts empty (no ready-made types). A type is plain (thickness, height) or layered (next rows) |
| Compound walls: Edit Type > Structure > layers from exterior to interior with Function (Structure, Substrate, Thermal/Air Layer, Membrane Layer, Finish 1, Finish 2), Material, Thickness, Wraps; Insert, Delete, Up, Down; Core Boundary | done: window opened by Edit Type… in the Wall options bar, in Properties and in the Types window; OK / Apply / Cancel; the type and all its walls change in one undo step and the wall is as thick as its layers; a core with no structure, a membrane in the core, an empty material or no thickness are refused with the reason shown. Material is a name (typed or picked from the names in use), with no material library, colours or patterns; no Variable layer, no Sweeps/Reveals, no Preview/section sample in the window |
| Layers shown in plan | done: a thin line where each layer ends, heavier at the core boundaries, along every segment and through corners; not cleaned at joins by layer priority, not hidden at coarse detail, no fill patterns |
| Compound walls in IFC | done: IfcWallType with IfcMaterialLayerSet (functions, core, wraps in OpenBIM_WallType), walls with IfcMaterialLayerSetUsage, read back by this app with or without its own data; an IFC from another program with a layer set on its wall types gives layered types when the layers add up to the wall's thickness (core from the layers marked Core, else first to last structure layer; IFC2X3 files have no function, so every layer is structure) |
| Draw mode Rectangle: two corners, four joined walls in one undo step, exterior face on the picked rectangle | done |
| Draw mode Pick Lines: click a grid, wall or beam; the clicked side is the exterior | done (no Pick Faces) |
| Draw modes Inscribed Polygon, Circumscribed Polygon (sides in the options bar, 3 to 64), Circle, Arc (start, end, a point on it) | done as joined straight walls in one undo step, along the Location Line with the exterior outward (clockwise); a circle or arc is built of straight walls that stand within 25 mm of the curve (at least 12 for a circle, at most 360), so a curved wall is many walls and is not one element with a radius. Missing: true curved walls, Start-End-Radius and Centre-ends and Tangent and Fillet arcs, typed radius or side length, Pick Faces |
| Spacebar flips the wall while drawing | done as the other face being the reference (Exterior and Interior lines swap); the exterior stays on the left of the direction drawn: use Flip on the placed wall, or draw clockwise |
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
| Roof by Extrusion, Roof Soffit, Fascia, Gutter, Join/Cut Roof, Attach Walls to Roof, Dormer, slope arrows, ridge/hip/valley editing | missing; roof openings (Vertical Opening, Shaft) are done, see Opening |

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
| Press on an element in an elevation or section and drag: it moves along the view and up or down in 50 mm steps, with a ghost and the distances; Esc cancels; undo. Doors, windows and wall openings slide in their wall and change their sill (the model refuses a move out of the wall, with the reason); walls, beams, slabs and stairs also change their height offset; a column, footing or anything else moves sideways only (up and down alone says to use Properties); what stands in a moved wall goes with it | done (2026-10-19). Snapping, the Move tool with a typed distance, and sliding in a wall at an angle to the view are done too: see the next rows. Depth is not changed by a drag (see the Depth row) |
| Snaps in a view: the pointer snaps to Endpoint (square), Intersection (X), Midpoint (triangle) and Nearest of the drawn edges, with the name beside it; else to an increment that gets smaller as the view is zoomed in (Revit's rule); Shift keeps a move level or plumb | done (2026-10-19) for drags, Move, Copy, Mirror, Wall Opening corners. Perpendicular (from the base point of Move / Copy; a foot beyond the end of the edge is not offered) and Center (of drawn arcs and circles; no view shows one yet) added 2026-10-19. Missing: Tab to cycle snaps, snap overrides (SE, SM...), Tangent, Snaps dialog, Snap to Remote Objects |
| Move and Copy in a view: select, pick a base point, then a second point or type a distance in mm and Enter (towards the pointer); Copy goes on until Esc; Ctrl while dragging copies | done; Constrain / Disjoin options and Multiple are missing (Shift is the constraint) |
| Doors, windows and openings in a wall that is not parallel to the view slide along their wall so that the drawing moves by the dragged distance | done (my reading of Revit: Autodesk's help does not say); a wall seen almost end on (within about 3 degrees) cannot be slid along: the refusal says "This movement is not available in the current view because of the element's host constraints. Use a view facing the host or edit its position in another suitable view." Moving it up or down alone still works there. NOT verified in Revit: whether Revit refuses, slides or detaches in the end-on case, and whether depth or sliding takes priority; it needs a test in Revit (window straight on / oblique / edge on) before it is called the same |
| Depth: nothing moves toward or away from an elevation or section | the same as Revit as far as I know (a view is a plane; Autodesk's help does not state it); to move in depth, use a plan |
| Mirror in a view: Draw Axis by two points, which must be vertical (Shift keeps the second point plumb); Copy on by default; a door or opening on its own stays in its wall and goes to the other side of the axis | done; Pick Axis and a horizontal axis are missing |
| Rotate in a view (axis square to the view, through the middle of the element): by two rays or a typed angle. Walls do not rotate in these views and a door or window cannot without its wall (Autodesk's rules); a beam or brace parallel to the view tilts about its middle (its slope changes) | done; the centre of rotation cannot be moved; columns, footings, slabs, stairs, ducts and pipes cannot be turned in a view |
| Align in a view: reference edge (a level line too), then the edge of what moves; a wall end goes to the reference (the wall gets longer or shorter, joined walls follow), the top or base of a wall moves (the wall gets taller or lower), anything else moves; the reference stays until Esc | done; Multiple Alignment, the lock and Prefer (face / centerline) are missing |
| Tag in an elevation or section: click an element; the tag says its type, follows the element when it is moved in the view, belongs to the view it was placed in (another view does not show it), is deleted with its element, saved and read back | done. Revit also lets a tag follow when the element moves in the plan; here a plan move leaves the tag where it is in the view |
| Dimension in an elevation or section: two vertical (or two horizontal) edges of what is drawn, then the place of the line; belongs to its view, selectable, draggable, deletable, undoable, saved | done as a linear dimension between the picked places. Not associative (it does not follow the edges when they move); no chains, no dimension to a point, no angled edges |
| Plan snaps (Wall, Move, Copy, Rotate, Dimension and the sketch tools in a plan): Endpoint, Intersection, Midpoint, Perpendicular (from the last point picked) and Nearest of wall location lines, wall edges and grids, with the same markers and names as in a view; else 50 mm; Shift keeps level or plumb with the last point | done 2026-10-19 (before: only wall corners, grid ends and grid lines). Not every tool uses it yet: Column, Beam, Family, Duct and Pipe keep their own picking |
| Which tools work in an elevation or a section | Select, Move, Copy, Rotate, Mirror, Align, Tag, Dimension and Wall Opening work there (Rotate with Revit's element restrictions). The others say "not available in an elevation or a section yet". That is not a rule of Revit's: Revit has more of them there (Level, Grid, Array, Split, Offset, Trim, duct and pipe in a section) and some, such as Wall by Line, in plans and 3D only; each is to be decided tool by tool, not by view type. Drawing walls/floors in these views; callouts; legends are missing |

## Ceiling
| Revit behaviour | State |
|---|---|
| Automatic Ceiling: click in a closed space, the ceiling takes its outline; the outline shows under the pointer | done (the outline is copied when placed: it does not follow the walls afterwards, as Revit's does; columns in the space are not cut out) |
| Sketch Ceiling: untick Automatic in the options bar, click the corners, Enter / first corner / Finish | done |
| Height Offset from Level (default 2700 mm) is the underside; thickness (default 20 mm) goes up from it | done (defaults are mine, not checked against Revit's template) |
| One ceiling per space and height | done (a second click says so) |
| Ceiling type with layers/materials, Ceiling Plan view (reflected ceiling plan), ceiling grid, slope, openings | missing; in a plan the ceiling is drawn faint under everything and picked last |
| IFC: IfcCovering CEILING (a room's ceiling for space boundaries), back from the app's own file unchanged | done; a covering that is not a ceiling is skipped on import |

## Stair (by component: straight, L and U)
| Revit behaviour | State |
|---|---|
| Stair > Run > Straight: click the foot of the first riser, then the direction; the number of risers follows from the height between the base and the top level and the Maximum Riser Height; treads are drawn as you move | done (press-drag-release works; Shift keeps it square) |
| Options bar: Width (1000), Max Riser Height (175), Tread Depth (280), Base Offset; Height when no level is above | done (defaults are mine, not checked against Revit's template) |
| Plan symbol: arrow up the centre line with UP at the foot | done (no cut line or break line, no riser/tread numbering) |
| 3D: a monolithic stepped block; the top riser is the edge of the floor above | done (no soffit slope, no stringers, no supports, no nosing) |
| Properties: Width, Total Rise, Base Offset editable; risers, treads, actual riser height and tread depth, run length shown | done (riser count is not editable: place the stair again) |
| Move / rotate / mirror / copy / delete / undo, saved | done |
| IFC: IfcStair STRAIGHT_RUN_STAIR with the side profile extruded across the width and Pset_StairCommon, in all three schemas; the app's own file brings it back unchanged | done; a stair written by another program is not read (skipped) |
| L and U shapes (Autodesk: two straight runs joined by an automatic landing): the Shape choice in the options bar (Straight, L left/right, U left/right) places two runs and a landing as one stair; the risers are shared equally (the first run gets the odd one), the landing is the surface after the first run | done (the first click and direction are the first run; an L or U stair needs at least 4 risers) |
| The parts are one stair: pick, move, rotate, mirror, copy, delete and undo take all three; they are saved and exported together | done (the second run and the landing point at the first run, as the beams of a beam system point at their system) |
| Landing: as wide as the stair (L) or across both runs with a 100 mm gap between them (U), 150 mm thick | done, but the thickness, the gap and the odd-riser rule are my choices, not read from Revit; the landing and the runs cannot be edited apart (no Edit Sketch) |
| IFC: each run an IfcStair STRAIGHT_RUN_STAIR; the landing an IfcSlab LANDING that bounds no room; the app's own file brings all three back unchanged | done; a landing written by another program is skipped like its stair |
| Railing on the stair (Autodesk: a railing is created by default): the Railing choice in the options bar (Both sides, Left, Right, None; Both by default) puts a railing along the edge of each run, round the landing on the outside and straight across it on the inside, in the same step as the stair | done; the railing is the Railing element with the stair as its host, so it is picked, moved, turned, mirrored, copied, deleted and undone with the stair, and Properties edits its height, rail size and baluster spacing |
| The rail rises with the runs and is level over the landing; posts at the corners and ends, balusters at the spacing | done, simplified: the rail is a straight line from the foot to the top of each run (1100 mm over the base, so about 1100 mm minus one riser over the first nosing, and 1100 mm over the last), balusters stand on that line and not on each tread, and the railing cannot be moved off its stair or put on another |
| Winder, runs from sketch, stair by sketch, top/base level choice, ramps, railing types and Railing Placement dialog | missing |

## Railing (sketched path)
Source: Autodesk help, "Railing System Type Properties" and "Modify a Railing". Nothing here comes from my own reading except the default sizes.

| Revit | OpenBIM |
|---|---|
| Architecture > Railing > Sketch Path: click the points of the path, finish | done: Railing tool in a plan view, a click per point (50 mm snap), Enter or Finish ends it, Backspace takes a point back, Esc cancels. One railing is one element with a path of any number of straight stretches |
| Railing Height (the top rail), rail structure, balusters at a pattern with a spacing, posts at the ends and corners | done: top rail at the Railing Height, a post at every point of the path, balusters at an even spacing near the one asked for (0 for none). Defaults 1100 mm high, 50 mm rails and posts, 150 mm spacing |
| Properties: Railing Height, Base Offset, baluster spacing, edit at any time, undo | done: Railing Height, Rail and Post Size, Baluster Spacing, Base Offset in the Properties palette |
| Seen in plans, elevations, sections, 3D; cut by a section line | done (plan, elevation, section, 3D) |
| Move, Copy, Rotate, Mirror, Delete; save; IFC IfcRailing | done in plan views; saved with the model; IfcRailing (GUARDRAIL) with a solid for each part in IFC2x3, IFC4 and IFC4.3, and the app's own file brings it back unchanged |
| Railing types with several rails, handrails, Rail Structure and Baluster Placement dialogs (patterns, posts at set places, profiles), Baluster Offset, angled/tangent joins and mitered rails, Landing Height Adjustment | missing: one fixed pattern; rails are straight boxes that overlap at a corner (no miter). Slopes along a stair are done (see Stair) |
| Pick New Host (a railing placed on a stair, a floor or a ramp edge), Edit Path with Toggle Sketch Orientation, Reset Railing, Split | partly: a railing is made on its stair when the stair is placed; a sketched railing is free-standing on its level, there is no Pick New Host, and editing the path means deleting and drawing again |
| Moving or copying a railing in elevation and section views; a railing written by another program (read from IFC) | missing |

## Opening (Wall, Shaft; done 2026-10-19)
| Revit | This app |
|---|---|
| Architecture > Opening panel: By Face, Shaft, Wall, Vertical, Dormer | the five buttons are there; Shaft, Wall and Vertical work, By Face and Dormer say they are not implemented. All five use my placeholder copy of the opening icon (the approved set has one icon) |
| Wall Opening: pick a wall, sketch a rectangle (in an elevation or section view) | done in a plan view (click one end on a wall, then the other end on the same wall) and in an elevation or section view (two opposite corners); the sill and head heights are in the options bar (900 / 2100 are my defaults). The opening is a rectangle and follows its wall; deleting the wall deletes it |
| Wall Opening in 3D | done: the wall is cut (same cut as a door or window) |
| Wall Opening in plan | dashed jambs across the wall; it can be picked on them; the wall's own plan lines are not broken |
| Shaft Opening: sketch a boundary, Base Constraint / Base Offset / Top Constraint / Top Offset, Finish | done: click the corners (snapped to 50 mm), Enter / first corner / Finish closes; Base is the plan's level, Top defaults to the level above and is chosen in the options bar ("Up to level"), both offsets are editable there and in Properties afterwards |
| A shaft cuts every floor, foundation slab and ceiling it passes through | done, by height: a slab is cut when its thickness overlaps the shaft's range (so a shaft from Level 1 to Level 2 cuts the floor of Level 2, not the floor of Level 1). A shaft that overlaps only a corner of a slab notches it. A shaft also cuts a roof whose height range it overlaps (the roof is cut straight through, keeping its slope) |
| Shaft in plan | dashed outline crossed by two lines on every level it passes through; picked on its symbol |
| Shaft in 3D | the slabs have the hole or the notch; the shaft itself is not drawn |
| Move / rotate / mirror / copy / delete / undo, saved | done |
| IFC: IfcOpeningElement + IfcRelVoidsElement for the wall opening (in its wall) and for each shaft in each slab it cuts (the file keeps the whole slab outline and the opening is the part of the shaft over the slab, so Revit does the cut), all three schemas; slab quantities take the voids off; the app's own file brings every opening back unchanged | done; an opening written by another program is not read |
| Vertical Opening (Autodesk: cuts a vertical opening through one roof, floor or ceiling): click the Vertical button, click the floor, roof or ceiling in a plan (Tab takes the next one under the click), click the corners (50 mm snap), Enter / first corner / Finish closes it; Esc steps back to the host choice, then out | done; it cuts only that element, however much else is at its height. It is an Opening that points at its host: it goes with it (move, copy, delete, undo, save), Properties shows the host with a Select host button, and the plan shows it as a dashed outline crossed by two lines on the host's level |
| A vertical opening in 3D, elevation, section | done: a floor or ceiling has the hole; a roof is cut in its sloped shape with vertical walls the roof's thickness deep |
| IFC: a vertical opening in a floor is an IfcOpeningElement + IfcRelVoidsElement like a shaft's; in a roof the hole is in the roof's shape (the roof is a faceted solid), with no opening element; the app's own file brings every one back unchanged | done in all three schemas; a ceiling's hole is in the app but not in the file (the covering carries no voids) |
| By Face (perpendicular to the face of a sloping roof), Dormer, openings in curved walls, arc / polygon / circle sketch of a shaft, shaft symbol options, a slab split in two by a shaft (only the larger piece is drawn in 3D; the file is right) | missing |

## Schedules/Quantities (done 2026-10-19, step 5 of the books review)
| Revit behaviour | App |
|---|---|
| View > Schedules/Quantities (also Analyze > Reports & Schedules): New Schedule/Quantities with a category list and a name; Revit then opens Schedule Properties on the Fields tab | done, with 27 categories: Walls, Floors, Roofs, Ceilings, Structural Foundations, Columns, Framing, Stairs, Railings, Rooms, Doors, Windows, Furniture, Generic Models, Mechanical Equipment, Air Terminals, Duct and Pipe Accessories, Plumbing, Lighting, Electrical Equipment and Fixtures, Sprinklers, Ducts, Pipes, Duct Fittings, Pipe Fittings. **Departure:** a new schedule starts with the fields its quantities come from (Revit starts empty), so a schedule shows something at once; remove what is not wanted |
| Schedule Properties: Fields (add, remove, order), Filter (up to 8, equals / not equal / greater / less / contains / begins with / ends with...), Sorting/Grouping (up to 4 levels, ascending or descending, header, footer, blank line, grand totals, itemize every instance), Formatting (heading, hidden field, calculate totals) | done: all four tabs. Text filters ignore case; a hidden field still filters and sorts; a filter or sort on a field that is removed goes with it |
| Live update: the schedule follows the model | done: the table is worked out again whenever the model or the definition changes; the elements are looked at on all cores |
| Not itemized: elements that look alike are one line, quantities added up | done: lines are merged on every shown field that does not calculate totals; fields that do are added |
| Footers "Title, count and totals" and Grand total | done (title, count and the totals of the fields that calculate them) |
| Click a row: the elements are selected (Properties shows them); Show in model | done: the row selects its elements; Show in model opens the plan of the level of the first one. Revit highlights in the open views and zooms; this does not |
| Export to Excel / CSV | done as CSV: headings carry the unit, numbers are plain, text that a spreadsheet would run as a formula is marked as text |
| Project Browser: Schedules/Quantities folder, double-click opens, Rename, Duplicate, Delete, Properties | done (context menu); a schedule is part of the model: undo, save, and the IFC file (the app's own records) bring it back |
| Quantities | Wall length, height, width, area and volume are **net of doors, windows and wall openings** (area), volume = net area x thickness. Floor and ceiling area is net of shafts; a roof's area is its sloped surface. Column length is base to top level; framing length is the member's length along its slope; volumes use the exact section. Ducts and pipes: length along the slope, surface area, size, slope %, system, insulation and lining. Fittings: type, part number, kind, angle, size, system. Doors and windows: width, height, sill and head height, area, mark, host wall type. Equipment: family, type, level, mark, elevation and the systems it is in |

**Not the same as Revit, said plainly:**
- Wall volume and area come from the wall's axis length: joins between walls (the corner overlap) are not taken out, which, as far as I know, Revit does. The numbers are right for a straight wall and slightly off at a corner. Not checked against a Revit model.
- Units are fixed: length mm, area m2, volume m3. There is no Format dialog (units, decimals, symbol), no per-field alignment, no Appearance tab (grid lines, banding, text sizes), no header/footer text options other than "title, count and totals".
- Not built: calculated values (formulas), combined parameters, Material Takeoff, Note Block, Sheet List, View List, Keynote and Revision schedules, Schedule Keys, shared and project parameters as fields, schedule on a sheet (needs Sheets, D2), conditional formatting, a schedule of a linked model, grouping in the Revit "group headers" over columns, the images in a cell.
- Door and window "Mark" is the family's own text input; there is no Mark numbering command. Rooms have no Number yet (needs spaces and zones, step 6).
- The layout of the table in the window has only been checked by drawing it (no crash, rows and columns as expected from the tests); I have not looked at it on a screen.

## View settings: scale, detail level, View Range, Visibility/Graphics, Hide/Isolate (done 2026-10-19, step 5b of the books review)
| Revit behaviour | App |
|---|---|
| View Control Bar: Scale, Detail Level, Visibility/Graphics, Temporary Hide/Isolate, Reveal Hidden Elements | done, one bar under the view tabs of every plan, the 3D view, elevations and sections. **Departure:** Revit has it at the bottom of the view; here it is at the top |
| View Scale 1:1 to 1:10000 | done: the standard scales in a list and a custom value; saved per view. On screen it sets the size of the annotations (grid bubbles, section and elevation markers, room and tag text) as on paper (bubble 1.8 mm, text 2 mm), clamped so they stay legible. It does not change the size of the model on screen (the zoom does that) |
| Detail Level Coarse / Medium / Fine | saved per view. Coarse draws a layered wall as one shape (no layer lines); Medium and Fine draw the layers. **Fine is the same as Medium**: the families have only one level of detail |
| View Range (Top, Cut plane, Bottom, View depth) of a plan | done: the View Range window (View > Graphics, and the control bar) with Revit's rules (view depth <= bottom <= cut <= top) and Restore default. Defaults as Revit: 2300 / 1200 / 0 / 0 mm for a level plan; **3000 mm top for the Mechanical plan** (to see ducts under a ceiling). Each element is tested by its real height: out of the range it is not drawn, between the cut plane and the top it is faded, under the bottom but within the view depth it is faded more. The plan says how many elements of its level the range leaves out |
| What stands on the plan's level and lies below the range (a floor slab, a footing) is seen in a plan | done by a rule: on its own level and under the bottom, it is shown |
| Visibility/Graphics: model and annotation categories on or off, per view | done: All / None / Invert, the model categories of the schedules (25: all but the fittings) plus Openings and Rebar, and the annotation categories Grids, Levels, Tags, Dimensions, Sections, Elevations; saved per view |
| Hide in View > Elements / Category, Unhide, Reveal Hidden Elements (magenta) | done, saved per view; a hidden element cannot be picked; with Reveal on it is drawn in magenta and can be selected to unhide |
| Temporary Hide/Isolate (element or category), Apply to View, Reset | done: not saved, not an edit (no undo step, the model does not change); it belongs to the view it was made in; Apply makes it the view's own setting in one undo step |
| The settings are part of the model | done: save, undo, the IFC file (the app's own records `W|`). A view of a deleted element drops it |
| 3D view, elevations and sections follow their own settings | done: they are made from the model without the hidden elements |

**Not the same as Revit, said plainly:**
- **The Level 1 plan no longer shows the sample duct** (it hangs at 2.7 to 3.0 m, above the 2300 mm top, as in Revit); it is in the Mechanical plan. **Ceilings and roofs above the top of a plan are not drawn in it either**, which is what Revit does; they were drawn before.
- Not built: view templates, Scope Box, Crop Region, View Filters, Underlay, Duplicate View (with detailing / dependent), Visibility/Graphics overrides of colour, line weight and transparency (only on/off), per-type and per-subcategory switches, Revit links tab, Phases and Design Options filters.
- The plan does not draw the cut graphics of Revit (heavy cut lines, cut pattern); an element crossed by the cut plane is drawn as the others. There is no separate view range for a reflected ceiling plan (there is no such view yet).
- The compute and cable overlays, pushpins, insulation outlines and Generate Layout placeholders are not hidden by categories or Hide in View. Reveal Hidden Elements works in plans only (the 3D view and sections simply leave the elements out).
- The dimension text and the clash symbols do not follow the view scale; the annotation size is only approximately Revit's.
- In a section or an elevation the model is cut again every frame; with many hidden elements that costs a copy of the model each time.
- The windows and the bar have only been checked by drawing them in tests (no crash, the right numbers); I have not looked at them on a screen.

## Sheets and printing (done 2026-10-19, step 5c of the books review)
| Revit behaviour | App |
|---|---|
| New Sheet with a paper size and a title block | done: A0 to A4, landscape or portrait, a drawn title block (frame, name, number, date, drawn by, checked by, scale) |
| Sheet number unique in the project | done: refused with a message if used by another sheet |
| Place View / drag a view from the Project Browser onto the sheet | done: Add to Sheet in the Project Browser and Place View in the ribbon; the view lands in the first free spot of the drawing area, at the view's own scale |
| A view can be on one sheet only | done: refused with a message |
| Move a viewport with snap, Esc cancels | done |
| Activate View, Remove From Sheet, Sheet Properties | done |
| Title block texts | done: name, number, date, drawn by, checked by (the same fields as the Revit title block) |
| Viewport title with detail number and scale | done: number in a circle, view name, line, scale |
| Schedules on a sheet | done: a schedule is placed as a table, drawn with its heading, rows and grand total |
| Print to PDF, one page per sheet in the order of the sheet numbers, in colour or black lines | done: vector PDF, page size from the sheet |
| Project Browser has a Sheets folder | done |
| Part of the model | done: save, undo, the IFC file (the app's own records `R|`); a sheet drops a view or schedule that was deleted |

**Not the same as Revit, said plainly:**
- The 3D view on a sheet is a **fixed isometric**, not the camera you have in the 3D view.
- Text in the PDF is standard Helvetica: characters outside Latin-1 print as "?".
- Not built: revisions and revision clouds, title block families and editing of the title block, guide grid, matchlines, view references, dependent views, crop region (the viewport box is the extent of the content), viewport types, schedule split over several sheets, legends, sheet list schedule, duplicate sheet.
- A schedule can be on one sheet only. The scale of a viewport is the scale of the view (change it in the View Control Bar).
- In plans on a sheet, ducts, pipes, beams, stairs and railings are drawn from their meshes; fittings and insulation are not drawn. The compute and cable overlays are not on sheets.
- On screen the text uses the egui font, so widths differ slightly from the PDF.
- I looked at the sheet only as a PDF rendered to a picture (plan, 3D, elevation, schedule, title block); I have not looked at the windows on a real screen, and the black-and-white print has not been looked at.

## Books review (2026-10-19)
The Revit MEP 2024 and Architecture 2026 books were read against the app: `docs/revit-book-review.md` (project doc `claude/revit-book-review.md`). It adds the rows this audit lacked for the Duct/Pipe placement tab (toggles, justification, slope, insulation), systems and Generate Layout, spaces and zones, electrical circuits, schedules, sheets, view range and Visibility/Graphics, other dimension types, and the modify tools not yet built (Redo, Create Similar, Select All Instances, Copy/Paste, Match Type, Pin, Split, Array). Redo and the Create Similar group were built from it (see Select-mode editing).

## Still missing for every element
Split Element, Array, Scale, Group, Selection Filter, Save/Load selection. The Offset and Wall Joins icons are placeholders I drew (the approved icon set has none): replace them with approved art. Level 1 and Level 2 are fixed tabs.

## Not working at all
All MEP fittings and equipment placement buttons except Duct/Pipe/Component, cable tray, conduit, lighting, devices, curtain wall, ramp, area, opening by face / vertical / dormer, detail, callout, view templates, link model, clash checks, worksets, phases; dimension types other than Aligned.

## Suggested order
1. (Room, Roof, Ceiling, Stair, Opening, elevations/sections, selecting, wall openings and moving things in them, the Elevation tool and dragging section ends are done.)
2. (Railings, L and U stairs, the railing on a stair, vertical opening, roof cuts, layered wall types and polygon/circle/arc wall modes are done.) Butt/Miter joins and associative dimensions.
3. True curved walls, Butt/Miter configuration, layer cleanup at wall joins by priority.
4. Associative dimensions.
5. From the books review (order in section 6 of `docs/revit-book-review.md`): (Redo, Select All Instances, Cut/Copy/Paste, Match Type, Pin and Create Similar are done.) (The Duct/Pipe placement options, slope, editable end offsets and the saved size/slope lists are done; Offset lock and the rest of Mechanical Settings (segments, fluids, round/oval ducts, electrical) are still to do.) Mechanical Settings; insulation; Generate Layout and systems; schedules (done); sheets and view range; spaces and zones; electrical circuits.

The 3D view (editing in 3D, the orbit control at the top right, GPU needs) is its own topic: `docs/3d-view-parity-plan.md` (project doc `claude/3d-view-parity-plan.md`). To be cleared at a later stage, on the owner's request of 2026-10-19.
