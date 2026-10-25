> Historical evidence: this report describes its stated baseline and contains superseded status claims. See [Current capability register](capability-register.md) for the 25 October 2026 source review and explicitly pending validation. Do not use this report's scoreboard as current completion status.

# Review of the app against the Revit books (2026-10-19)

References (owner's copies, `D:\Miscellaneous`):
- *Exploring Autodesk Revit 2024 for MEP*, 10th edition (10 chapters, 2 case studies).
- *Exploring Autodesk Revit 2026 for Architecture*, 22nd edition (16 chapters).
- The Structure book was still downloading; it is not covered yet (see "Still to do").

Method: I read both tables of contents in full and the chapters on walls, doors and windows, modifying elements, datums and views, floors, roofs, ceilings, rooms, openings, dimensions, schedules, sheets, and the MEP chapters on ducts, pipes, spaces and electrical. Each behaviour was compared with `docs/revit-ux-parity-audit.md` and with a search of the code (ribbon, options bars, tools) at the head of `feature/viewport-3d-app`. The books are paraphrased here, not quoted. Nothing was compared side by side with a running Revit, so "matches" means "matches the book's description".

The audit so far followed the architecture side (walls, rooms, roofs, stairs, openings, views). It has **no rows for the MEP tools or for documentation**, which are the two largest gaps against the books.

## 1. What already matches the books

Wall sketching (Line, Rectangle, polygons, Circle, Arc, Chain, Location Line with the six positions, flip, exterior on the left), compound walls with layer functions, wall joins allow/disallow, doors and windows as hosted elements that cut the wall, wall opening and shaft, rooms with boundaries from walls, roof by footprint with slope edges, automatic and sketched ceilings, stairs (run, L, U) with railing, Move/Copy/Rotate/Mirror/Align/Offset/Trim-Extend corner, temporary dimensions with typed values, aligned dimension, tags, elevations and sections with the same edit tools, selection by box (window/crossing).

For MEP: duct and pipe drawn by clicking points with elbows and reducers made automatically, tees for a branch started on a run, a run that starts on an equipment connector leaves it straight and joins it, fittings stay attached, calculation modules (friction, sprinkler, plumbing, electrical cable sizing).

## 2. Gaps, MEP (priority for this owner)

| # | Revit behaviour (book) | App today | Priority |
|---|---|---|---|
| M1 | **Place Duct / Place Pipe contextual tab** (MEP ch5, ch7): Automatically Connect on/off, Inherit Elevation, Inherit Size, Justification (horizontal Left/Center/Right, vertical Top/Middle/Bottom), Lock/unlock specified elevation next to Offset, Tag on Placement, Type Selector (duct type, pipe type and segment/material), Reference Level, System Type | **Done 2026-10-19** except the type selector, Offset lock and Reference Level (see the audit). Was: options bar had System, Width, Height (Diameter for pipe), Middle elevation, Finish, Clear. No toggles, no justification, no types, no tag on placement. Snapping onto equipment connectors is always on | P1 |
| M2 | **Sloped piping panel** (ch7): Slope Off / Up / Down, Slope Value list from Mechanical Settings > Pipe Settings > Slopes (add/delete values), Show Slope Tooltip while drawing (start offset and slope) | **Done 2026-10-19** for plain pipe runs; the slope list is edited in Mechanical Settings. Was: the engine can grade a run (`route_graded`, `Command::SetSlope`) and a connected piece of equipment can set a slope; the Pipe tool cannot draw a slope and there is no slope list | P1 |
| M3 | **Select a duct/pipe**: offset value shown at each end and editable by click (changes the end's elevation), connectors at the ends, Modify tab with Justification, Slope, Cap Open Ends, Change Elevation | **Done 2026-10-19** in Properties (Run Ends), not on the canvas; no cap. Was: Properties showed the run; no editable end offsets on the canvas, no cap | P1 |
| M4 | **Insulation and lining**: Add / Edit / Remove Insulation (type, thickness) on ducts, pipes and fittings; Duct Lining | **Done 2026-10-19** (see the audit): Add / Edit / Remove on ducts, pipes and fittings, lining on ducts, types in Mechanical Settings; no IFC geometry for other programs, no thermal use. Was: ribbon icons only | P1 |
| M5 | **Create Mechanical / Piping System** from selected equipment (System type, System name), Edit System (Add to / Remove from System), System Browser, System Inspector (flow arrows, section tag with fixture units) | **Done 2026-10-19** except the System Inspector (see the audit): systems of equipment with a type and a name, Add / Remove, base, System Browser, saved and in the IFC file's own records. Was: systems are a property of each run; no system object, no browser | P1 |
| M6 | **Generate Layout / Generate Placeholder**: place a Base, Solution Type (Network, Perimeter, Intersections, Custom) with numbered solutions, Modify Base, Finish Layout, Convert Placeholder to duct/pipe with a chosen size | **Partly done 2026-10-19**: Network (two orientations) and, for two connectors, the obstacle-aware Routed solution; numbered solutions with arrows, trunk size, Finish Layout (see the audit for what differs). Missing: Perimeter, Intersections, Custom, Place/Modify Base on the plan, saved placeholders, equipment as obstacles, vertical runs | P1 |
| M7 | **Spaces and zones** (ch4): Space tool (distinct from Room, bounded by space separators, with airflow and load data), Place Spaces Automatically, Space Separator, Zone from spaces, HVAC zone, **Color Scheme** and legend, system analysis | Rooms only. No Space, no Zone, no color scheme | P1 |
| M8 | **Air terminals and devices placed on a face / on a work plane** (ch5, ch6): Place on Face, Flow (CFM/L/s) parameter, copy by snaps to midpoints; hosted lighting fixtures in a ceiling | Components can be placed; no host-on-ceiling mode, no Flow as a drive for duct sizing | P2 |
| M9 | **Duct and pipe sizing / checks**: Duct Sizing, Check Duct Systems (disconnected, no flow), Duct Legend, pressure drop by Colebrook or Haaland, fluids and temperatures table in Mechanical Settings | Friction/pressure drop calculations exist in `openbim-calc` and the Compute button; no Mechanical Settings dialog (fluids, slopes, sizes, materials, segments), no checks, no legend | P1 |
| M10 | **Electrical**: Power and Lighting circuits (Create Circuit from selected devices/fixtures, Select Panel, Edit Circuit), Panel Schedules, voltage definitions, distribution systems, load classifications and demand factors, wire types | Cable sizing exists (`elec_ui`: grouping, ambient, demand factor, RCD, fault check). No circuit object tied to a panel, no panel schedule, no load classification | P1 |
| M11 | **Fire protection** (ch8): place sprinkler heads per space (spacing from the space schedule), connect sprinklers with pipes, wet and dry systems, Sprinkler Libraries | Sprinkler hydraulics are in `openbim-calc`; a Sprinkler tool is in the ribbon list of placeable parts; no space-driven placement | P2 |
| M12 | **Fabrication parts** (ch5): Fabrication Settings (configuration, profile, services), Fabrication Part palette (PB), convert generic parts to fabrication parts, hangers | Missing; also the whole Revit add-in is needed for real fabrication content | P3 |
| M13 | **Linking Revit models** (ch2): Manage Links, visibility of linked models per view, copy/monitor linked elements, bind link | Link Model button only; no linking yet | P2 |
| M14 | **Mechanical and Electrical Settings dialogs** (ch5, 6, 7): sizes, segments, materials, fluids, slopes, voltage and distribution, load calc, panel schedule templates | **First part done 2026-10-19**: Manage > MEP Settings edits and saves the pipe slope, pipe diameter and duct width/height lists. Fluids, segments, materials, round/oval ducts and the electrical settings are missing; those values are fixed in the code or on the Compute dialog | P1 (pair with M1, M2, M9, M10) |

## 3. Gaps, documentation (all disciplines)

The Project Browser lists *Schedules/Quantities*, *Sheets* and *Legends* as empty folders.

| # | Revit behaviour | App today | Priority |
|---|---|---|---|
| D1 | **Schedules/Quantities** (arch ch12, MEP ch5/6): New Schedule by category, Fields, Filter, Sorting/Grouping, Formatting, Appearance; schedule view with live update; export to Excel (CSV); schedule keys; air-terminal, door, window, room, duct, pipe schedules | **Done 2026-10-19 (step 5):** 27 categories, Fields / Filter / Sorting-Grouping / Formatting, live update, itemize, footers and grand totals, CSV export, saved with the model and in the IFC file. Not built: calculated values, Material Takeoff, Sheet List, schedule keys, Format dialog, Appearance (see the audit, "Schedules/Quantities") | P1 (rest P2) |
| D2 | **Sheets** (ch13): New Sheet with a title block, drag views onto the sheet (viewports with title), view scale on the view, schedules on sheets, revision cloud and tag, guide grid, dependent views and matchlines, print/PDF setup | **Done 2026-10-19 (5c):** sheets with paper size and title block, views and schedules placed at view scale, viewport titles, Print to PDF (see the audit). Still no revisions, title block families, guide grid, matchlines, dependent views, crop region, legends | P1 (rest) |
| D3 | **Dimensions** (ch11, MEP ch9): Aligned, Linear, Angular, Radial, Diameter, Arc Length, Spot Elevation, Spot Coordinate, Spot Slope; permanent dimensions that follow the element (witness line to a face or centre, lock, EQ); baseline and ordinate; alternate units; tick/arrow type; automatic wall dimensions (Dimension > Auto) | Aligned only, point to point; not tied to elements | P1 (associative) / P2 (other types) |
| D4 | **Tags** (ch11): Tag by Category with leader, Tag All Not Tagged, Tag Multi-Category, Room Tag with area/number, Tag on Placement, Keynotes, Symbols, Material tag | Tag by element, no Tag All, no room tag family, cannot move a tag alone | P2 |
| D5 | **Text notes** (ch9, ch12): Text tool with leaders, text types (font, size), edit in place, spell-check; model text | Missing | P2 |
| D6 | **Detailing**: Detail Line (line styles, weights, patterns), Detail Component, Filled/Masking Region, Drafting View, Callout (rectangle or sketch) with its own crop and scale, Legend View, Revision Cloud | Only a Revision Cloud button; no drafting view, no callout, no detail line | P2 |
| D7 | **View properties**: View Scale, Detail Level (coarse/medium/fine), **View Range** (top, cut plane, bottom, view depth), Visibility/Graphics overrides by category and by element, Filters, Temporary Hide/Isolate, view templates, Scope Box, Crop Region, Duplicate View (with detailing / dependent), Underlay | **Done 2026-10-19 (5b):** scale, detail level, View Range, Visibility/Graphics by category and element, Hide in View, Reveal Hidden Elements, Temporary Hide/Isolate (see the audit). Still no scope box, view templates, crop region, filters, underlay or Duplicate View | P1 (rest) |
| D8 | **Color schemes and legends** (arch ch16, MEP ch4): by room/space/duct system | Missing | P2 |

## 4. Gaps, modelling (architecture and structure)

| # | Revit behaviour | App today | Priority |
|---|---|---|---|
| A1 | **Redo** (Ctrl+Y) | **Done 2026-10-19** (was a button with nothing behind it) | done |
| A2 | **Wall options bar**: Offset (distance to the location line), Radius (for circle/arc), Height/Depth list, **Top constraint** (Unconnected Height or up to a level), Base/Top offset, Room Bounding, Structural Usage; **Attach Top/Base** to roof or floor | Height and Base offset only. No Offset field, no top constraint to a level, no attach | P1 |
| A3 | **Layer priority and join cleanup** (arch ch3): the Priority number next to each layer decides how layers join at corners and intersections; Wrapping at Inserts/Ends; Join Geometry | Layers drawn straight through; no priority, no wrapping | P2 (already on the backlog) |
| A4 | **More wall kinds**: Slanted (angle from vertical), Tapered (exterior/interior angles), Stacked walls (several types by height), Wall Sweeps and Reveals, wall finishes *Place by Room* and *Place by Segment* (new in 2026), curtain walls, Ellipse / Tangent End Arc / Fillet Arc / Center-ends Arc sketch tools, **true curved walls** | Straight walls only (curves are facets). None of the others | P2 |
| A5 | **Doors and windows**: Tag on Placement, flip hand/facing by the blue arrows, Swing Angle in plan, Load Family, Place on Face, Orientation in a slanted wall, Mark numbering, sill/head height shown as instance parameters | Door swing is a stored choice in Properties and the door is hosted; no flip arrows, no tag on placement, no family loading | P2 |
| A6 | **Floors**: sketch boundary with Pick Walls, Span Direction, slope arrow, **shape editing** (Add Point, Add Split Line, Pick Supports, Modify Sub Elements, Reset Shape); floor types with layers | Slab from a rectangle/boundary; no shape editing | P2 |
| A7 | **Roofs**: by Extrusion, Join/Cut Roof, Attach Walls, soffit, fascia, gutter; **Ceiling types** and ceiling plan views; Dormer and By Face openings | Footprint roofs only | P2 |
| A8 | **Levels and grids**: Create Plan View check, plan view types, level copy/monitor, story elevation display; grid Multi-Segment and Pick Lines, bubbles and 2D/3D extents; **Reference Planes and Work Planes** (set work plane, show) | Level and Grid tools exist; two fixed level tabs; no reference planes, no work planes | P1 (level tabs, reference planes), P2 (rest) |
| A9 | **Modify tools** (ch5): Array (linear, radial), Match Type Properties, **Create Similar**, Pin, Scale, **Split Element** and Split with Gap, Trim/Extend Single and Multiple, Align Multiple Alignment, Copy/Cut/Paste (Paste Aligned to Selected Levels / Same Place), **Select All Instances**, Selection Filter, Save/Load selection, Groups (create, place, swap, ungroup), Parts, Measure along an element, Select by ID | **Done 2026-10-19:** Match Type Properties, Create Similar, Pin/Unpin, Cut/Copy/Paste (plan views, Same Place), Select All Instances. Missing: Split Element, Array, Scale, Filter, Groups, Parts, Trim single/multiple, Multiple Alignment, Paste to selected levels/views | P1: Split; P2: Array, Groups, Scale, Filter, Parts |
| A10 | **Stairs and ramps**: Stair by Sketch, Winder, Landing and Support tools, ramps | Stair by component only; no ramps | P3 |
| A11 | **Site** (ch9): Toposolid (points, imported data, subdivision, split, grading, excavation), property lines, site and parking components, contour labels | Missing | P3 |
| A12 | **Massing** (ch10): In-place mass, Building Maker (wall/floor/roof/curtain system by face) | Missing | P3 |
| A13 | **Structural elements** (arch ch16): structural walls, columns, beams, braces, openings in beams/braces/columns | Column, Beam, Brace, Foundation, footing, slab present; openings in beams and columns missing; Structure book to be read | P2 |
| A14 | **Project units** (ch2): Project Units dialog by discipline (length, area, volume, angle, slope, flow, pressure, electrical), rounding and format | Fixed metric; units appear on the Compute dialog only | P2 |
| A15 | **3D** (ch14): perspective camera, Orient, Fly, Lock 3D view, Section Box (partly there), Selection Box | Orbit and the 3D view exist; no camera or perspective; the 3D plan is in `docs/3d-view-parity-plan.md` | P2 |
| A16 | **Rendering, walkthrough, solar study, Route Analysis, point cloud** (ch15, ch16) | Missing | P3 |
| A17 | **Design options, phasing, worksets, links** (ch16) | Ribbon buttons without function; sync design is in `docs/revit-style-ui/05-backlog.md` COLLAB-001..003 | P2 |

## 5. Points where the books change what the audit says

- The audit asks for "Butt / Miter / Square off" wall join configuration. The Architecture book does not describe it; it describes **layer Priority** and **Default Wrapping** as what governs the joint. The wall join work should therefore start with priority and wrapping, then the join type.
- The wall options bar in the book includes **Offset**, **Radius**, and the **Height/Depth** direction (a wall can hang down from the level). The audit lists Offset as missing; Radius and Depth were not tracked.
- The book confirms the audit on temporary dimensions: the conversion symbol turns one into a permanent dimension (the audit already lists "convert to permanent" as missing; it belongs with the associative dimension work, D3).
- The 2026 edition adds **wall finishes placed by room or by segment**. These need rooms to expose their bounding faces, which the app already computes.
- The audit's "Redo" status line ("undo exists, no redo") is still correct; the ribbon button gives the impression it works.

## 6. Proposed order

Each step is a self-contained, tested change like the earlier ones.

1. ~~Redo and the Create Similar / Select All Instances / Copy-Paste / Match Type / Pin group (A1, A9)~~ done 2026-10-19; Split Element and Array are the next small ones.
2. **Duct and Pipe placement tab** (M1, M2, M3): ~~toggles, justification, slope up/down, editable end offsets, Tag on Placement~~ done 2026-10-19 (see the audit). **2b done 2026-10-19:** Mechanical Settings with editable, saved lists of pipe slopes, pipe diameters, duct widths and heights (M14, first part). Still to do: fluids, pipe segments and materials, round/oval ducts, electrical settings, Offset lock, the duct/pipe type selector.
3. ~~**Insulation and lining** for ducts, pipes and fittings (M4)~~ done 2026-10-19.
4. ~~**Generate Layout for a piping/duct system**: system object, Edit System, Place Base, solutions, Convert Placeholder (M5, M6)~~ done 2026-10-19 in part (Network and Routed solutions; no Perimeter / Intersections / Custom, no saved placeholders).
5. ~~**Schedules/Quantities** (D1)~~ done 2026-10-19 (5a). ~~**5b View range, scale, detail level, V/G, isolate** (D7)~~ done 2026-10-19. ~~**5c Sheets and printing** (D2)~~ done 2026-10-19. Next: **6 spaces/zones/color schemes (M7) and electrical circuits/panel schedules (M10)**.
6. **Spaces and zones with color schemes** (M7), then the electrical circuits and panel schedules (M10).
7. **Wall options bar** (Offset, top constraint, attach), layer priority/wrapping, then Slanted/Tapered/Stacked, sweeps and reveals, true curved walls (A2 to A4).
8. Dimensions: associative first, then the other types (D3). Tag All and room tags (D4), text notes (D5).
9. Floors with shape editing, roofs by extrusion, ceiling types (A6, A7). Levels/grids/reference planes (A8).

## 7. Still to do

- Read the **Structure** book once its download finishes and add a section for columns, beams, braces, foundations and rebar behaviours.
- Run a side-by-side check against a real Revit project when the owner supplies one (the parity rows remain "matches the book", not "matches Revit").
- The Revit 2027 SDK installer in the same folder is kept for the later Revit add-in work.
