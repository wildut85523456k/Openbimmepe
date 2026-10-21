# Revit 2026 Architecture textbook, part 2: chapters 6-10 (reference notes)

Source: "Exploring Autodesk Revit 2026 for Architecture" (CADCIM), text extraction `arch.txt`, lines 10368-19842.
Chapters covered (headings checked in the text):
- Ch 6, "Working with Datum Elements and Creating Views" (lines 10368-12527): levels, grids, reference planes, work planes, graphic overrides/hide-isolate, plan/elevation/section views, scope boxes. Tutorials 1-3 (apartment, club, office).
- Ch 7, "Adding Floors, Roofs, Ceilings, and Rooms" (12528-14400): floors, slab edge, roofs (footprint, extrusion), shape editing, ceilings, rooms, openings, attach/detach walls. Tutorials 1-3.
- Ch 8, "Adding Stairs, Railings, and Ramps" (14401-16420): components, stairs, railings, ramps, curtain walls/systems/grids/panels/mullions, copy-to-levels. Tutorials 1-4.
- Ch 9, "Designing Site Elements and Topography" (16421-17423): Toposolid (new in this edition, replaces toposurface), sub-divide, grading, import, contours, property lines, site/parking components. Tutorial 1.
- Ch 10, "Working with Conceptual Masses and Custom Families" (17424-19841): massing (family editor forms, in-place mass, conceptual design environment, Building Maker), custom families. Tutorials 1-2.

Presence check requested by the brief: MASSING / Conceptual mass / Building Maker / in-place mass are present in full (Ch 10); Toposolid / site is present in full (Ch 9). Structural elements (columns, beams, foundations) and walls/doors/windows are NOT in this slice (earlier chapters). Chapter-number caution: the book's exercise file names use "c05_..." for the previous chapter, which is just its own numbering.

Conventions: `[book]` = stated by the book; `[knowledge]` = my own Revit/IFC knowledge, not from the book. Text is paraphrased; cites are (Ch N, "section heading"). Imperial values are quoted with metric template values in brackets where the book gives them. Where the book is inconsistent it is listed under "Open questions".

Quick orientation to the Revit UI vocabulary used below: Ribbon tab > Panel > Button; Options Bar (context row under ribbon); Properties palette (instance parameters + Type Selector); Edit Type opens Type Properties (type parameters); Project Browser (views tree); View Control Bar (scale, detail, visual style, crop, hide/isolate, reveal hidden); sketch mode (contextual "Modify | Create ..." tab with Finish Edit Mode tick and Cancel X in the Mode panel).

## 2. Tutorial spine (build order from scratch)

The book's tutorials are a chain; each starts from the previous chapter's saved file. The three running projects are an Apartment, a Club and an Office. The order shows what depends on what.

### Chapter 6 (datum, then views)
Start from the finished Ch 5 model (walls, doors, windows present). Order:
1. Hide door/window tags in the plan via Visibility/Graphics > Annotation Categories (just to declutter).
2. Open an elevation (North). Level 1 and Level 2 already exist in the template (metric Level 2 sits at 4000 mm; set to 3000 or 3600 mm to suit). Drag level line ends to sit near the building.
3. Place more levels with the Level tool from the left end of Level 2: hover, type the spacing (e.g. 10'-0"), click to finish along the alignment line. Floor + ceiling plans appear automatically (Make Plan View on). Repeat for each storey; Esc twice.
4. Rename levels from the Project Browser (right-click plan > Rename; answer Yes to rename level and views): First Floor, Second Floor, ... Roof.
5. In the First Floor plan, place the Grid tool and draw grids as wall centrelines (alignment lines/extension snapping; Tab cycles snaps at an intersection); numbers auto-increment; rename grids (click name, type, Enter) e.g. A, B or A1.
6. Create sections (View > Section): click on one side, click on the other; the section appears in the browser as Section 1/2; rename (Section X, Section Y). If a grid bubble collides, use the elbow control on the grid.
7. Interior elevations: View > Elevation; place the tag inside the room, then tick the other three check boxes around the tag to get 4 walls; rename; crop each view with the crop region drag handles.
8. Office variant: set level 2 height, add two levels, set exterior walls' Top Constraint to the top level (Up to level) plus Top Offset 3'-0", then select the interior content (everything except exterior walls), Copy to Clipboard, Paste > Aligned to Selected Levels to the upper floors.

### Chapter 7 (floors, ceilings, roofs)
1. Hide Elevations, Grids, Sections annotations in the plan.
2. Floor: Floor: Architectural > Pick Walls (Tab chain, click) with Extend into wall (to core) on or off as specified; flip control to put the line on the exterior face when required; Finish; choose floor type in Type Selector; Modify to exit. (Office: Rectangle tool from the exterior faces instead.)
3. Switch to the Ceiling Plan; Ceiling tool (Automatic Ceiling), pick ceiling type, set Height Offset From Level (8'-6", 11'-0", 9'-0"), click inside each room.
4. Attach walls to the floor: open a section, select exterior wall chain with Tab, Attach Top/Base, Base radio, click the floor.
5. Club only: Roof by Footprint in the plan at the chosen level; Defines slope on, Overhang 4'-0", Pick Walls on each exterior wall; Trim/Extend to Corner the offset lines; set Base Level and Base Offset From Level; select all sketch lines (Ctrl) and set Slope to 4"/12"; Finish; answer Yes to attach walls; choose roof type. Flat lounge roof: Defines slope off, extra edges drawn with Line (Chain off), Base Offset From Level -1'-4". Second hall: Mirror - Draw Axis across grid intersections. Walls not under roof: Attach Top/Base to the floor base.
6. Office: copy the floor to the other levels with Copy to Clipboard + Paste > Aligned to Picked Level (pick level lines in an elevation); repeat ceiling per level.

### Chapter 8 (furnish, vertical circulation, curtain wall)
1. Place a Component with Load Family; check Rotate after placement; click; set rotation by cursor; adjust size by Duplicate type and edit type parameters (Cabinet Width, Height).
2. Walls up to the top floor: select exterior walls, set Top Constraint to Roof level and Top Offset 3'-0".
3. Copy all non-exterior-wall items up with Aligned to Selected Levels.
4. Roof for the top: Roof by Footprint in the roof plan, Defines slope off, Pick Walls (flip to inner face if needed), Finish, choose Generic flat roof type.
5. Curtain-wall conversion (club): select the two walls, change type to Curtain Wall: Exterior Glazing (accept Unjoin Elements), edit type spacing for vertical/horizontal grid, crossing-select the top row of panels with Filter on Curtain Panels, switch to Solid panel type, then Mullion tool > All Grid Lines on each curtain wall.
6. Stairs (lobby/office): in the ground plan, Stair tool, set run width 5'-0", stair type, Base Level, Top Level, Desired Number of Risers; draw Ref Planes first as guides; Run tool > click start (Intersection snap), move up, click end; click start of second run, move down, click end; auto landing results. Optionally Convert the landing to custom, Edit Sketch, replace with a Boundary > Pick Lines + arc semicircle. Finish Edit Mode twice. Then select the stair and Select Levels (Multistory Stairs) in an elevation to repeat to upper floors. Delete unwanted auto railings.
7. Railing: at a upper-level plan, Railing > Sketch Path, draw line, Finish. In a section confirm.
8. Floor opening for the stairs: Shaft tool, rectangle around the stair, Finish; set Base Constraint (Ground Floor) and Top Constraint (Roof) with offsets (-1'-0", +2'-0").

### Chapter 9 (site)
1. Open a project that already has reference planes, model lines and property lines in the Site Plan view.
2. Set the toposolid type first: Contour Display (primary 5', secondary 1'), Structure layer (Site-Grass, 1'-0" thick).
3. Toposolid Create from Sketch: choose type and Level; Rectangle boundary; Finish.
4. Select toposolid > Modify Sub Elements > Add Point: Elevation Base = Current Level; type 5'-0", click several points; then 10'-0", click points inside. Contours generate (bottom-up order matters).
5. Sub-divide for roads/paths: select toposolid > Sub-divide > Pick Lines with Tab to take the closed region; choose type (Path - 8" Concrete), edit its structure material, set Sub-divide Offset; Finish.
6. Parking Component (type Parking Space 9'x18' 90 deg, Offset from Host 3"), Array linear for rows. Site Component: load RPC tree, click to place; set Level of selected trees.
7. Sections across the site; renaming; edit contour interval/visibility via Visibility/Graphics > Topography subcategories.

### Chapter 10 (massing)
Office-2 tutorial (mass first, then shell):
1. New project from default template; add levels to make 6 (First Floor ... Roof at 10' spacing).
2. First Floor plan: In-Place Mass > name "Central Hall" > Rectangle 50'x50' > Modify > select > Solid Form; open 3D; click top face; click lower temp dimension, type 60'; Finish Mass.
3. In-Place Mass "Wings": chain Lines for the wing profile, Mirror - Draw Axis along the square's diagonal, Solid Form on each, set each height 40' by the top-face dimension; Finish Mass.
4. Cut: select Central Hall, Edit In-Place, Wireframe, locate centre with Plane tool diagonals, Circle R=15' as profile, Void Form, drag/raise its ends past the top face; second void (entrance) from Center-ends Arc R=20' plus chained lines, Void Form, set 30' height; Finish Mass.
5. Dome: In-Place Mass "Dome", set work plane by picking a wall face, draw a reference vertical axis line and a quarter arc (Model), select both, Solid Form (revolved), Finish Mass.
6. Convert: select masses, Mass Floors (tick all levels), Floor by Face (crossing select floor faces, Create Floor); Wall by Face for brick walls then Curtain Wall Exterior Glazing (edit type spacing first) for wings and entrance; Roof by Face for each top face; Curtain System by Face for dome (duplicate type, Grid 1/Grid 2 spacing, select both curved faces, Create System); curtain-panel door via Filter; Mullion tool All Grid Lines.
Family tutorial (architectural column): New Family from Generic Model template; Reference Planes in the Back/Front elevation with typed offsets; unpin and extend the Center plane; Extrusion tool (Rectangle) for shaft, base and crown with Extrusion Start/End; Void Extrusion (Chain) for fluting; set Material (duplicate/rename, appearance asset); Save As Family; Load into Project from the family window; Rotate/Move into place; roof footprint with arc via Line and Start-End-Radius Arc, Base Offset -2'-2".

## 3. Tool reference (one section per tool, template: Where / Preconditions / Interaction / Options bar / Properties / Rules / Gotchas / Outputs+IFC4)

## 3.1 Chapter 6 tools: datum elements and views

### Level
- **Where**: Architecture > Datum > Level; shortcut `LL`. Tool is greyed in plan views; only active in elevation or section views (Ch 6, "Adding Levels"). Template ships Level 1 and Level 2.
- **Preconditions**: an elevation/section view open. Level lines are drawn there; the level itself is an infinite horizontal plane (line length only matters for display) (Ch 6, "Adding Levels", Note).
- **Interaction**: Modify | Place Level tab. Draw panel: Line (default) or Pick Lines. Click start point (this fixes elevation + start together), move left/right, click end point. Hovering near an existing level shows a temporary perpendicular dimension; typing a number then gives the new level that offset from the nearest level. A dashed alignment line appears when the end lines up with an existing level end; click there to align. A typed elevation before the first click sets absolute elevation. Esc once ends the current level, Esc twice leaves the tool (Ch 6 Tut 1: "Press ESC twice"). New levels auto-named Level 3, Level 4... [book]
- **Options bar**: Make Plan View check (Plan Views panel) with a Plan View Types button that opens a dialog listing Ceiling Plan, Floor Plan, Structural Plan (all ticked by default); Offset edit box (offset from picked point/element). 
- **Type selector**: "Level: 1/4 Head" (bubble head shown) and "Level: No Head".
- **Instance properties**: Elevation (height above elevation base), Story Above, Computation Height (used for room area/perimeter/volume), Name, Structural (default off; e.g. top-of-foundation), Building Story (default on), Scope Box.
- **Type properties**: Elevation Base (project origin vs shared origin), Line Weight, Color (default black), Line Pattern, Symbol (head family, None allowed), Symbol at End 1 Default, Symbol at End 2 Default (bubble at left/right end).
- **Selected-level controls**: two bubble-visibility check boxes (one per end); two end drag circles (resize line); padlock = length alignment lock (locked: all level lines stretch together; unlock to resize one); 2D/3D extent toggle (3D: edit in any view propagates to all views; 2D: view-specific); a break/elbow control near the bubble moves bubble+name away, creates an extension line with blue drag dots.
- **Modify**: click name to rename (prompts "rename corresponding views?" Yes renames floor/ceiling plans in browser). Click the temporary dimension and type a new elevation: level moves. Drag a level; Shift constrains to vertical; elevation updates dynamically. Rename also possible via Project Browser right-click > Rename on the plan view (same prompt, Yes renames level and views). Right-click level: Go to Floor Plan, Find Referring Views (Go To View dialog > Open View). Hide in View > Elements/Category; Override Graphics in View > By Category > uncheck Visible hides all levels in that view. Scope Box limits where a level shows.
- **Rules**: Levels are the reference for level-hosted elements (floors, roofs, ceilings; walls base/top constraint). Changing level elevation moves hosted/constrained elements (implied by constraints) [knowledge]. Creating a level with Make Plan View off gives no plan views; later use View > Plan Views > Floor Plan (see New Plan View).
- **IFC4**: IfcBuildingStorey (Elevation attribute) [knowledge]; the book states levels and grids carry a unique IfcGUID [book].

### Grid
- **Where**: Architecture > Datum > Grid (also Structure tab) [knowledge]. Visible in plan, elevation, section and 3D views; Revit 2026 can show grids as 3D planes in 3D views (Ch 6, "Creating Grids").
- **Interaction (Line)**: Modify | Place Grid; Draw panel Line (default). Click start, move, click end. Temporary angular dimension shows angle to horizontal; click it to type an angle. Shift = ortho. Near an existing grid, a temporary distance dimension appears; type a number or click. Alignment line (extension) snaps ends to neighbours. Numbering is automatic (1, 2, 3...), the name increments from the last grid name. Esc twice leaves tool.
- **Draw tools**: Line, Start-End-Radius Arc, Center-ends Arc (radial grids; Radius edit box enabled only after ticking the Radius check), Pick Lines (snaps to wall interior/exterior faces, column centrelines; click when highlighted; curved elements give curved grids; Offset box lets you place a grid at e.g. wall centreline = half thickness offset from a face), Multi-Segment (enters Modify | Edit Sketch; options: Chain, Offset; click points for segments, then Finish Edit Mode on Mode panel).
- **Options bar**: Offset; Radius (with check); Chain (for Multi-Segment).
- **Selected-grid controls**: bubble visibility check at each end, end drag circles, 2D/3D extent toggle, elbow/break control to offset the bubble (name moves, extension line created), padlock alignment lock. Ctrl-click multiple grids then drag; Shift limits to orthogonal. Right-click menu: Select Previous, Select All Instances, Create Similar, Properties.
- **Type properties**: Symbol; Center Segment (Continuous / None / Custom); Center Segment Weight/Color/Pattern (only enabled when Custom); End Segment Weight/Color/Pattern (apply to whole line when Continuous, to the ends when None/Custom); End Segment Length (paper-space length, when None/Custom); Plan View Symbols End 1 (default off), End 2 (default on); Non-Plan View Symbols (Default): top/bottom/both/none in elevations and sections. Types named "Grid: 1/4 Bubble" and "1/4 Bubble Custom Gap".
- **Instance properties**: Name, Scope Box. Rename by clicking the name, or via Name in Properties.
- **Visibility**: right-click > Hide in View > Element/Category; Visibility/Graphics > Annotation Categories > Grids; Scope Box.
- **IFC4**: IfcGrid with IfcGridAxis curves [knowledge].

### Reference Plane
- **Where**: Architecture > Work Plane > Ref Plane (also Structure/Massing tabs [knowledge]); `RP`. Draw with the Draw panel (line / pick); click start, click end. Name via Properties > Name (Ch 6, "Reference Planes"). Named ref planes appear in the Work Plane "Name" list.

### Work Plane (Set / Show / Viewer)
- **Where**: Architecture > Work Plane panel: Set, Show, Viewer, Ref Plane.
- **Set**: opens Work Plane dialog with three choices: Name (drop-down of levels, grids, named reference planes), Pick a plane (then click a plane, e.g. a wall/floor/roof face), Pick a line and use the work plane it was sketched in. Plan views have an automatic work plane (the level); sections/elevations/3D need one set. Revit only lets you create/edit elements that lie in the current work plane (Ch 6, "Work Planes").
- **Show**: toggles display of the work-plane grid; spacing set by selecting the plane and editing Workplane Grid Spacing in Properties. Object snaps work on this grid.
- **Viewer**: opens a Workplane Viewer window to edit work-plane-dependent elements.

### Visibility / Graphic Overrides, Hide/Isolate (view tools used throughout)
- View > Graphics > Visibility/Graphics opens the dialog with Model Categories, Annotation Categories (and Imported categories) tabs; checkbox per category; per-category overrides for Projection/Cut lines, Patterns, Transparency (Surfaces), Halftone, Detail Level. Apply vs OK.
- Per element: right-click > Override Graphics in View > By Element (dialog: Visible, Halftone, Projection Lines weight/colour/pattern, Surface Patterns, Surface Transparency slider 0..100 where higher = more transparent, Cut Lines, Cut Patterns). By Category opens View-Specific Category Graphics with a button to open the full dialog.
- Hide in View > Elements/Category (permanent for that view). Reveal Hidden Elements (bulb icon on View Control Bar): magenta border, hidden items magenta; select > Unhide Element/Category; toggle button exits.
- Temporary Hide/Isolate (sunglasses icon, needs a selection): Hide Element, Isolate Element, Hide Category, Isolate Category, Reset Temporary Hide/Isolate, Apply Hide/Isolate to View. Cyan border while active. Transparent surfaces keep edges and patterns visible.

### Plan views (New Floor Plan / Ceiling Plan)
- Levels create floor + ceiling (+ structural) plans automatically if Make Plan View is ticked. Otherwise View > Create > Plan Views > Floor Plan: dialog with Type (Edit Type/Duplicate), level list (Shift multi-select), "Do not duplicate existing views" check (clear to make duplicates named with suffix (1), (2)...), Scale.
- View properties (select view in Project Browser): View Scale, Display Model, Detail Level, View Name, View Range (Extents; defines the horizontal cut), Crop Region Visible, etc.
- Default template has two floor plans and two ceiling plans; elevation tags N/E/S/W appear in plan.

### Elevation
- **Where**: View > Create > Elevation drop-down (Elevation; Framed Elevation [knowledge]). Type selector: Building Elevation or Interior Elevation.
- **Interaction**: move cursor in a plan; the symbol's arrowhead snaps perpendicular to nearby walls; click to place. Esc twice to exit. Select the symbol: four check boxes around it (one per side) create/delete additional elevations (a, b, c, d suffixes, e.g. 1-a); rotate control; clicking the arrowhead shows the clip plane (blue line with drag dots) to set width (and far clip). Interior elevations auto-extend the clip plane to the room extents. 
- **Browser organisation**: building elevations under Elevations (Building Elevation); interior ones under Elevations (Interior Elevation) (Note: the tutorial claims new ones appear under Building Elevation; see Open questions).
- Rename: Project Browser right-click > Rename. Open: double-click browser name; double-click arrowhead; right-click symbol > Find Referring Views; right-click arrowhead > Go to Elevation View.
- Gotcha: if the model extends past the elevation clip planes, the elevation shows a cut-through view; drag clip control beyond the building.
- Crop region: select its box in the view, drag controls; Show Crop Region button on View Control Bar.

### Section
- **Where**: View > Create > Section. Modify | Section tab; Reference panel has "Reference other view" check (creates a reference section not added as new view in browser). Type selector: Section: Building Section, Section: Wall Section, Detail View: Detail. These land under Sections (Building Sections), Sections (Wall Sections), Detail Views (Detail).
- **Interaction**: click start, drag across the building, click end; the head side indicates viewing direction. Any angle allowed. Sections can be created from plan, elevation, section. Created section appears in every view whose range intersects it.
- **Controls on selected section line**: flip arrows (flip viewing side), head/tail cycle control at each end (toggle display), break control mid-line (break into two independently adjustable pieces; click again to rejoin), view-depth line with blue drag arrows (far clip: only elements inside depth appear), drag the line to move the section and the view updates live.
- **Split Segment**: Modify | Views > Section panel > Split Segment: click on straight section line at the split point, then move perpendicular (head or tail side) and click to place the jog; produces a segmented (jogged) section with orthogonal segments.
- **Properties**: View Name, View Scale, Crop Region Visible...; Type Properties: Callout Tag, Section Tag, Reference Label.
- Gotcha: in-place elements are not cut in sections (they do not show cut).
- Hide: right-click > Hide in View > Element.

### Scope Box
- **Where**: View > Create > Scope Box; only enabled in plan views (Ch 6, "Using the Scope Box Tool").
- **Interaction**: Properties shows Name and Height before placement; click upper-left corner then lower-right corner of a rectangle. Drag controls resize it. Revit 2026: height editable in Properties anytime; scope box can be assigned to a view directly in a View List schedule.
- **Apply**: select a grid, level or reference plane; Properties > Scope Box parameter > pick the scope box > Apply. The datum then shows only in views whose cut plane intersects the scope box.
- **Views Visible**: select the scope box; Extents > Views Visible > Edit opens Scope Box Views Visible dialog listing view types/names, an Automatic visibility column and an Override column (None / Visible / Invisible).

### Hosting copy across levels (from Tut 3)
- Select elements except exterior walls; Copy to Clipboard; Paste > Aligned to Selected Levels; Select Levels dialog (Ctrl multi-select) pastes at the same relative elevation per level. Exterior walls are better extended: set wall Top Constraint = Up to level: Roof and Top Offset (3'0"/900 mm) instead of stacking walls.

### 3.1a Datum addendum (coordinator request): Level, Grid, Reference Plane, Work Plane, Scope Box in the full template
Datum elements start in Ch 6 of this book. Items the book does NOT cover are stated as such; no Copy/Monitor, no multi-level "dependent view" mechanics beyond the points below, and no column-grid association rules appear in the slice.

#### Level (full template)
- **Where**: Architecture > Datum > Level (also Structure tab [knowledge]); `LL`. Active only in elevation or section views, disabled in plans [book].
- **Preconditions**: none beyond an elevation/section; default template already holds Level 1 and Level 2 (so an app must not hard-code them: they are ordinary levels that can be renamed, moved, deleted [knowledge: deletion allowed if no dependants warning]). Metric template places Level 2 at 4000 mm [book].
- **Interaction**: (1) choose type in Type Selector (Level: 1/4" Head or Level: No Head); (2) optionally tick/untick Make Plan View and open Plan View Types (Ceiling Plan, Floor Plan, Structural Plan; all three ticked by default); (3) Draw panel Line (default) or Pick Lines; (4) hover near the end of an existing level: temp dimension shows the vertical offset to the nearest level; type a number + Enter to fix it, or click; (5) click = start point AND elevation; drag horizontally; dashed alignment line to existing level ends; click = end. Name auto-assigned (next number: Level 3...). Esc ends the current chain; second Esc leaves the tool. Typing an elevation before the first click is allowed [book]. Pick Lines with Offset option: offset from picked line/element [book options bar].
- **Options bar**: Make Plan View check + Plan View Types button (in the Plan Views panel); Offset edit box; Draw tool chooser.
- **Type properties**: Elevation Base (Project Base Point vs Shared), Line Weight, Color, Line Pattern, Symbol (head family or None), Symbol at End 1 Default, Symbol at End 2 Default.
- **Instance properties**: Elevation, Story Above, Computation Height, Name, Structural (default off), Building Story (default on), Scope Box.
- **Controls**: bubble check boxes (end 1 / end 2), end-drag circles, padlock alignment lock, 2D/3D extent toggle (3D propagates edits to all views; 2D is view-specific), elbow/break control to detach bubble with extension line.
- **Edit**: rename by clicking the label (prompt: also rename matching views; Yes renames floor/ceiling plans), or Project Browser > right-click plan > Rename (same prompt); edit elevation via temp dimension or by dragging (Shift = vertical only); right-click menu: Go to Floor Plan, Find Referring Views, Hide in View > Element/Category, Select All Instances, etc.
- **Dependent views**: plan views created with a level are bound to it and named after it (Floor Plans, Ceiling Plans, Structural Plans); renaming the level can rename them. Making a plan later: View > Plan Views > Floor Plan / Ceiling Plan with the level list (Shift multi-select; "Do not duplicate existing views" clear gives copies with suffix (1)).
- **Rules**: infinite horizontal plane; reference for level-hosted elements (floor, roof, ceiling, room Level/Upper Limit, wall Base/Top Constraint, stair Base/Top Level, shaft Base/Top Constraint, toposolid Level, component Level). Elements keep an offset from the level; moving a level moves hosted elements [knowledge]. Levels can be Structural and/or Building Story flags. Computation Height feeds room area/volume. Level lines need not span the model.
- **Gotchas**: non-story levels (sill, parapet) are possible by clearing Building Story [book: "non-story level or reference level"]; levels hidden in a view by Hide in View or Scope Box still exist; level bubble may collide with section/grid bubbles.
- **Not in the book**: copy/monitor of levels, level linking to link files, level deletion consequences, Revit's "Create Plan View" numbering rule beyond Level N.
- **IFC4**: IfcBuildingStorey with Elevation; level GUID noted as IfcGUID [book]. Non-story levels are not storeys [knowledge].

#### Grid (full template)
- **Where**: Architecture > Datum > Grid (Datum panel). Grids visible in plan, elevation, section and (2026) 3D views, optional 3D plane display [book]. 
- **Preconditions**: a plan view (book example) - grids are view-independent datum planes extending vertically; extents set per view with 2D/3D toggle.
- **Draw tools (Modify | Place Grid > Draw panel)**: Line; Start-End-Radius Arc; Center-ends Arc; Pick Lines; Multi-Segment (switches to sketch: draw chain/offset, Finish Edit Mode). Options bar: Offset, Radius (enabled by its check), Chain for multi-segment.
- **Line interaction**: click start; move; temp angular dimension (click to type angle); Shift = ortho; near an existing grid a temp distance dimension appears for typed spacing; click end. Automatic numbering continues from the last name (1,2,3; if the last name is A, next is B [knowledge]). Pick Lines snaps to wall faces (interior/exterior), column centrelines, curved elements give arc grids; Offset in options bar gives centreline grids (half wall thickness) [book].
- **Type properties**: Symbol; Center Segment (Continuous/None/Custom) + Center Segment Weight/Color/Pattern (Custom only); End Segment Weight/Color/Pattern; End Segment Length (None/Custom); Plan View Symbols End 1 (default off), End 2 (default on); Non-Plan View Symbols (default). Types: "1/4" Bubble", "1/4" Bubble Custom Gap".
- **Instance**: Name, Scope Box.
- **Controls**: bubble check boxes at each end, end-drag circles, 2D/3D extent, padlock alignment lock, elbow control (bubble offset), drag to move; Ctrl multi-select, Shift ortho drag; temp dimension typed value moves grid; right-click menu Select Previous / Select All Instances / Create Similar / Properties. Copy/array of grids is allowed [book answer key: "cannot copy or array grids" is False].
- **Rules**: grid is a vertical datum plane (infinite) displayed per view; visible in a view only if intersecting cut plane / within Scope Box; Hide in View > Element/Category; Visibility/Graphics > Annotation Categories > Grids.
- **Relation to columns**: only mention: Pick Lines snaps to column centrelines; structural placement at grid intersections stated in the intro (Ch 6, "Working with Grids"). Columns snapping/binding to grids is not described [knowledge: columns can be placed At Grids].
- **IFC4**: IfcGrid + IfcGridAxis (UAxes, VAxes, WAxes) [knowledge].

#### Reference Plane (full template)
- **Where**: Architecture > Work Plane > Ref Plane; `RP`; also Create > Datum in family editor and conceptual design [book]. 
- **Interaction**: Modify | Place Reference Plane; Draw panel: Line (default), Pick Lines [knowledge]; click start, click end; the plane is a vertical plane through the line in plan, or a plane perpendicular to the view in elevation/section (tutorial draws them in elevation for roofs and in family elevations). Offset typing via alignment snap (Ch 10 column tutorial: hover, type distance, Enter). Selected plane can be unpinned/pinned (pin symbol) and dragged beyond extents.
- **Properties**: Name (instance; named ref planes appear in the Work Plane "Name" list), Workplane Grid Spacing (when it's the active work plane).
- **Uses**: guide lines for sketching (stairs runs, roof extrusions profiles, openings, family geometry); work plane for extrusion/roof profiles; reference for dimensions/labels in families. Snapping to intersections via Intersection object snap.
- **IFC4**: not exported (no IFC class) [knowledge].

#### Work Plane Set / Show / Viewer (full template)
- **Where**: Architecture > Work Plane panel: Set, Show, Ref Plane, Viewer; same panel in Create tab of family/in-place editors.
- **Set dialog**: radio Name (drop-down: levels, grids, named reference planes), Pick a plane (then click a plane/face: wall, floor, roof face, ref plane), Pick a line and use the work plane it was sketched in. If the chosen plane is perpendicular to the current view, Go To View dialog asks which view to open to draw in (e.g. East elevation, 3D).
- **Show**: toggles a visible grid of the work plane in the current view; spacing = Workplane Grid Spacing of the selected plane; object snaps work on it.
- **Viewer**: Workplane Viewer window to edit work-plane-based elements.
- **Rules**: elements can only be created/edited in the current work plane [book]. Plan views have an implied work plane (the level); elevation/section/3D need one set (auto-prompted by tools such as Roof by Extrusion, Extrusion in families). Per-view work plane.

#### Scope Box (full template)
- **Where**: View > Create > Scope Box; plan views only; view-specific creation.
- **Interaction**: Properties shows Name and Height before drawing (2026: height editable later); click two opposite corners (upper-left, then lower-right). Selected: drag controls resize.
- **Assigning**: select grid/level/ref plane > Properties > Scope Box parameter drop-down > Apply. Datum then shows only in views whose cut plane intersects the scope box; views can also carry a Scope Box parameter (2026: assign in View List schedule). Views Visible > Edit: Scope Box Views Visible dialog: list of views, Automatic visibility column, Override column None/Visible/Invisible.
- **IFC4**: not exported [knowledge].
## 3.2 Chapter 7 tools: floors, roofs, ceilings, shape editing, rooms, openings, wall attach

### Floor: Architectural (sketch-based)
- **Where**: Architecture > Build > Floor drop-down > Floor: Architectural (other items: Floor: Structural, Floor: Slab Edge) (Ch 7, "Creating Architectural Floors"). No shortcut stated.
- **Preconditions**: a plan view of the target level (floor lives on that level). Sketch can also be done in 3D if the work plane is set to the plan. A closed boundary is mandatory; an open or self-overlapping loop gives a warning and no floor.
- **Interaction**: tool enters sketch mode (drawing fades; contextual tab Modify | Create Floor Boundary). Draw panel default = Pick Walls: hover a wall, face highlights; a magenta line with flip control and drag control appears on the picked face. Tab key chains all connected walls (Tut 1: hover wall, Tab, click). Flip control swaps the line between inner and outer wall face. Other Draw tools: Line (with Chain), Rectangle, Polygon, Circle, Arc, etc., same behaviour as walls. Edit tools in Modify panel (Trim/Extend to Corner, Offset, Move...). Finish Edit Mode (green tick, Mode panel) creates the floor; Cancel discards. Then type chosen in Type Selector. Esc / Modify button exits tool.
- **Options bar**: Offset (distance of sketch line from picked wall face), "Extend into wall (to core)" check (extends floor to core layer, giving a joint with the wall core), Chain (for Line).
- **Slope Arrow** (Draw panel): Line or Pick Lines; sets slope. Slope arrow instance properties: how the slope is specified (by height at tail/head, or by slope), level at tail, tail height offset, etc. [book: "specification method", "level at which tail rests", "height offset at tail"].
- **Properties**: level, Height Offset From Level (floor above/below level), type etc. Type Properties: Structure > Edit opens Edit Assembly (layers with Insert/Delete, material, thickness; total thickness = sum). Duplicate to create a new type. Parameters vary by type.
- **Modify | Floors tab** (after finish) has ten panels: Properties, Clipboard, Geometry, View, Modify, Measure, Create, Mode (Edit Boundary), Draw, Work Plane (the book's list); Shape Editing panel is present too.
- **Rules**: floor is not automatically visible in plan view cut from below (Default 3D view to see it). Walls are not auto-attached to floors (see Attach Top/Base). Floors may be copied between levels with Copy to Clipboard > Paste > Aligned to Picked Level (Tut 3).
- **Gotchas**: closed loop needed; "Ignore the warning" when floor overlaps/joins walls was shown in Tut 2; boundary should be on outer face of the exterior wall when needed (use flip).
- **IFC4**: IfcSlab, PredefinedType FLOOR (BASESLAB for slab on grade) [knowledge]; layers = IfcMaterialLayerSetUsage [knowledge].

### Floor: Structural
- **Where**: same Floor drop-down. Carries loads, participates in structural analysis (unlike architectural floor) [book]. IFC4: IfcSlab with Pset_SlabCommon.LoadBearing = TRUE [knowledge].

### Floor: Slab Edge
- **Where**: Floor drop-down > Floor: Slab Edge; Modify | Place Slab Edge. Hover a floor edge (highlights), click: slab edge profile added. Two flip arrows appear on the edge to flip direction. Adjacent slab-edge segments next to an existing one merge into one continuous edge. Type + instance properties editable (Properties / Edit Type) (Ch 7, "Creating Architectural Floors").
- IFC4: IfcSlab (edge) or IfcBuildingElementPart [knowledge].

### Roof by Footprint
- **Where**: Architecture > Build > Roof drop-down > Roof by Footprint (alongside Roof by Extrusion, Roof by Face, Roof Soffit/Fascia/Gutter in the same drop-down) (Ch 7, "Creating Roofs").
- **Preconditions**: plan view. First roof in project (or at lowest level) shows "Lowest Level Notice" dialog with a level drop-down (default offered Level 2) and Yes button: the footprint is placed on that level's plane.
- **Interaction**: Modify | Create Roof Footprint tab, sketch mode. Pick Walls is default; Tab chains; the dashed line shows the offset side. Alternative draw tools: line, rectangle, circle, ellipse, arc, Boundary Line list. Trim/Extend to Corner joins offset lines (Tut 2 steps 7: needed when overhang lines don't meet). Select boundary lines (Ctrl multi-select) then set Slope in Properties (e.g. 4"/12" = 18.43 deg [the book prints 18.46, 8.46 mm - typos]). Finish Edit Mode: Revit asks whether to attach highlighted walls to the roof; Yes attaches wall tops to roof underside. Esc to exit.
- **Options bar**: Defines slope (check; lines drawn with it get a slope triangle symbol; clear it for a flat roof), Overhang (distance outward from picked wall face), Extend to wall core (check). Chain when using Line.
- **Boundary-line properties**: Defines Roof Slope (check), Slope (rise per 12" / ratio), overhang, offset, length.
- **Closed loop(s)**: perimeter must be closed; extra inner closed loops are openings in the roof.
- **Shapes**: all long sides sloped, short sides unchecked -> gable; only one edge sloped -> shed; all four sloped -> hip; none sloped -> flat. A flat roof can also be made by typing 0 for slope. To make a gable later: Edit Footprint, select line, untick Defines Slope.
- **Instance properties** (headings Constraints, Construction, Dimensions, Identity Data, Phasing): Base Level (default topmost level), Room Bounding (check, default on), Related to Mass (read-only check), Base Offset From Level (+/-, from sketch level); Rafter Cut (Plumb Cut / Two-Cut Plumb / Two-Cut Square), Fascia Depth, Rafter or Truss, Maximum Ridge Height (read-only); Slope (blank and disabled when no slope-defining line), Thickness, Volume, Area (read-only).
- **Type properties**: Structure (Edit Assembly: layers, materials, thickness; total = sum), Coarse Scale Fill and colour, Structural flag, identity data, cost.
- **Modify | Roofs**: Edit Footprint (Mode panel), Copy, Rotate, Move, Mirror (Mirror-Draw Axis used in Tut 2 to copy Hall 1 roof to Hall 2: select roof, Mirror - Draw Axis, click 2 points for the axis), join/cut tools, shape editing tools (only for roofs without slope).
- **Gotchas**: a sloping roof can look cut off at the plan cut plane, open another view (e.g. site plan) to see it whole; to select roof, open the view in which it was created; the first roof asks Lowest Level Notice.
- **Outputs**: Area, Volume, Thickness (roof quantity takeoff), Room Bounding flag. IFC4: IfcRoof (with IfcSlab ROOF parts) [knowledge].

### Roof by Extrusion
- **Where**: Roof drop-down > Roof by Extrusion.
- **Sequence**: (optional) draw Reference Planes first in the elevation. Tool opens Work Plane dialog: Name (list), Pick a plane (default), Pick a line and use the work plane it was sketched in. If the plane is perpendicular to the current view a "Go to View" dialog lets you choose elevation/3D view to sketch in. Then "Roof Reference Level and Offset" dialog: Level (default = highest level) and Offset (+ above, - below); a reference plane is generated automatically. Then Modify | Create Extrusion Roof Profile: sketch an OPEN chain of connected lines/arcs (must not form a closed loop); Measure panel dimensions; trim/extend/align/split/offset; Move/Copy/Mirror/Array/Scale/Delete. Finish Edit Mode.
- **Properties**: Extrusion Start / End (extent along work plane normal), Reference Level, Level Offset, plus type. Type properties as above.
- **After creation**: walls usually stop short of the soffit; select walls > Attach Top/Base > Top > pick roof.

### Roof Soffit / Fascia / Gutter
- Roof drop-down items; add soffit, fascia, gutter to roof edges (details not given in this slice).

### Shape editing (floors, structural floors, roofs without slope)
- **Where**: select the element; Modify | Floors > Shape Editing panel: Modify Sub Elements, Add Point, Add Split Line, Pick Supports, Reset Shape. Only on horizontal slabs (roof with no slope-defining lines).
- **Modify Sub Elements**: shows green boundary and corner grips. Click a point or an edge (turns blue), drag the double-arrow vertical grip, or type Elevation in options bar. Dragging an edge's mid control raises the edge only; the end points stay.
- **Add Point**: options bar Elevation (default 0'-0") + Relative check (default on: elevation relative to the surface; clear it to use absolute project elevation). Click on the surface to add; then drag point vertically.
- **Add Split Line**: pick a vertex/edge/point/face, then a second one; the line becomes an edge splitting surface into sub-regions that slope independently when dragged.
- **Pick Supports**: pick beams; creates split edge using beam end elevations (shift from beam bottom to slab top/bottom by thickness).
- **Reset Shape**: revert to flat.

### Ceiling
- **Where**: Architecture > Build > Ceiling; Modify | Place Ceiling tab. Ceiling Panel: Automatic Ceiling (default), Sketch Ceiling.
- **Preconditions**: ceiling plan view of level (ceilings are invisible in floor plans; a warning appears if placed in floor plan). Walls/room-bounding elements must enclose area for Automatic.
- **Automatic Ceiling**: choose type in Type Selector, set Height Offset From Level in Properties (Tut 1: 8'6"), hover inside a closed region (red boundary highlight), click. Revit creates the ceiling covering that region. Repeat per room. Ceiling plan needed to view; section shows height.
- **Sketch Ceiling**: Modify | Create Ceiling Boundary: Boundary Line tool with list (Line default), or Pick Walls tool with Offset and "Extend into wall (to core)" option. Finish Edit Mode. Use Sketch for uneven rooms.
- **Properties**: Level, Height Offset From Level; Type Properties: Structure (Edit Assembly), Duplicate. Types seen: Compound Ceiling: GWB on Mtl. Stud / Plain; 2'x2' ACT System / 600x600 Grid; Basic Ceiling.
- **Modify**: Edit Boundary (Mode panel) to resketch; from a section view it asks which view to edit in (Go To View). Edge properties of boundary editable. Copy/Rotate/Move/Mirror.
- **Rules**: level-based; room-bounding element. IFC4: IfcCovering PredefinedType CEILING [knowledge].

### Room (Architecture > Room & Area > Room)
- **Where**: Architecture > Room & Area > Room; Modify | Place Room. Plan views only (also from room schedules).
- **Tools**: Highlight Boundaries (shows room-bounding elements; message box with Close); Place Rooms Automatically (fills all enclosed regions bigger than 0.25 sq ft [book]; dialog states count); Tag on Placement (Tag panel); Type Selector: Room Tag, Room Tag With Area, Room Tag With Volume.
- **Options bar**: Upper Limit (level drop-down), Offset (default 10'; positive up, negative down from the upper-limit level), tag orientation (Horizontal/Vertical/Model), Leader check, Room drop-down (lists rooms already in a schedule but not placed).
- **Interaction**: hover in enclosed region: crosshair graphic sized to region; click. Room can be placed in unenclosed space (warning "not in a properly enclosed region") and takes shape later when walls close it. Placing from schedule: choose Room in options bar, tag, click. If none chosen, new room gets next number.
- **Bounding elements**: walls, roofs, ceilings, curtain systems, floors, columns, room separation lines (Room & Area panel > Room Separator [knowledge]); a wall has a Room Bounding instance flag [knowledge].
- **Instance parameters**: Level, Upper Limit, Limit Offset (default 10'), Base Offset, Area, Perimeter, Unbounded Height, Volume (only when Areas and Volumes computation is on), Computation Height, Number, Name (default Room), Comments, Occupancy, Department, Base Finish/Ceiling Finish, Wall Finish/Floor Finish, Occupant, Phase.
- **Volume computation**: Room & Area drop-down > Area and Volume Computations: Computations tab radio Areas only (default) vs Areas and Volumes. Volume uses finish faces of bounding elements; engine looks up and down from the measurement height; Base Offset prevents the room leaking through floor openings.
- **IFC4**: IfcSpace (area/volume in Qto_SpaceBaseQuantities) [knowledge].

### Openings (Architecture > Opening panel): By Face, Shaft, Wall, Vertical, Dormer
- **By Face**: pick planar face of roof/floor/ceiling (also beam/column); sketch boundary on that face (Modify | Create Opening Boundary, use reference planes); Finish. Cut is perpendicular to the face (so tilted for slopes).
- **Vertical**: pick floor/roof/ceiling; sketch in plan; cut is vertical.
- **Wall**: pick wall; drag a rectangle (click-drag).
- **Dormer**: pick roof, enters sketch (Modify | Edit Sketch); Pick Roof/Wall Edges on Pick panel; boundary must be a closed loop of roof or wall edges. Finish.
- **Shaft**: Modify | Create Shaft Opening Sketch; sketch shape; Finish; cuts through all levels by default. After, select it and set Base Constraint and Top Constraint (levels) and offsets to limit range. Set correct work plane/view before sketching.
- IFC4: IfcOpeningElement with IfcRelVoidsElement [knowledge].

### Attach Top/Base and Detach Top/Base (walls)
- **Where**: select wall(s); Modify | Walls > Modify Wall panel > Attach Top/Base, Detach Top/Base.
- **Interaction**: options bar radios Top / Base; then click the target (roof, floor, ceiling). Floor -> Base; roof or ceiling -> Top. After one click the tool stays for more elements. Walls selected via Tab chain in 3D or section.
- **Status**: wall instance properties Top is Attached / Base is Attached (read-only checks). You can attach just the structural layer without finishing layers [book].
- **Rules**: walls do not auto-attach. On finishing a roof footprint, Revit offers to attach walls. A visible difference appears only when floor Height Offset From Level changes.
- **Gotcha**: Self-evaluation answer key says Attach tool (the quiz item "Join Geometry" is false).
## 3.3 Chapter 8 tools: components, stairs, railings, ramps, curtain systems, copy between levels

### Component (Place a Component) / Model In-Place
- **Where**: Architecture > Build > Component drop-down: Place a Component (shortcut `CM`), Model In-Place. Modify | Place Component tab; Mode panel has Load Family. (Ch 8, "Using Components")
- **Interaction**: pick type in Type Selector (families loaded from library folders e.g. English-Imperial > Furniture / Plumbing / Specialty Equipment; Load Family dialog; the loaded type becomes selected). Options bar "Rotate after placement": after the click the Rotate tool runs; move cursor and click to set angle (dashed rectangle shows direction). Temporary dimensions to nearby elements appear on placing; click one to type a distance. Esc twice/Modify to exit.
- **Properties**: Level (changing moves it to another level), Host info, phases; "Moves With Nearby Elements" check locks position to the nearest element (e.g. a sink follows its wall) while still sliding in the insertion plane. Type Properties: Duplicate to make new size type (e.g. Cabinet Width, Height); editing a type changes all instances.
- **Model In-Place**: Family Category and Parameters dialog -> Name dialog -> Family Editor ribbon; Finish Model in In-Place Editor panel. In-place elements do not cut in sections [book, Ch 6 note].
- **IFC4**: category-dependent IfcFurnishingElement / IfcSanitaryTerminal / etc. [knowledge].

### Stair (component-based; Architecture > Circulation > Stair)
- **Where**: Architecture > Circulation > Stair; opens Modify | Create Stair tab. Component panel: Run (default; drop-down: Straight, Full-Step Spiral, Center-Ends Spiral, L-Shape Winder, U-Shape Winder, Create Sketch), Landing (Pick Two Runs, Create Sketch), Support (stringer/carriage). Tools panel: Railings (choose railing type), Convert, Edit Sketch. Multistory Stairs panel: Connect Levels / Select Levels. Mode panel: Finish Edit Mode.
- **Preconditions**: plan or 3D view at the base level; Base Level and Top Level pickable in Properties (defaults: current level; next story level above).
- **Run, Straight**: options bar Location Line (Exterior Support: Left, Run: Center [default], Run: Left, Run: Right, Exterior Support: Right), Offset, Actual Run Width, Automatic Landing check (default on: landing auto-created between runs). Properties: Type, Base Level, Base Offset, Top Level, Top Offset, Desired Number of Risers, Actual Tread Depth, Tread/Riser Start Number... Click start point, move (preview shows risers created so far), click end point. A second run started from the end of the first creates a landing automatically (Tut 3: two runs with Intersection snaps from reference planes gave a rectangular mid-landing). Finish Edit Mode once to leave run mode, again to complete stair (Tut 3 pressed Finish twice).
- **Full-Step Spiral**: click centre, click radius point. **Center-Ends Spiral**: click centre, start, end (<360 deg; end side picks turn direction).
- **L-Shape Winder / U-Shape Winder**: click start of lower run; Spacebar changes direction; options bar Location Line, Relative Base Height, "Mirror Preview" (flips start/end of the U).
- **Landing, Pick Two Runs**: click first run then second run; landing created between. **Create Sketch**: Modify | Create Stair > Sketch Landing; Draw panel Boundary and Stairs Path tools; Properties Relative Height (from stair base); Finish. Convert tool on an auto-landing converts it to custom (warning), Edit Sketch to re-sketch (Tut 3 deletes auto sketch and uses Boundary > Pick Lines + Arc to form a semicircular landing, endpoints must meet the run boundaries).
- **Support**: pick a run/landing edge; adds stringer/carriage; Finish.
- **Connect Levels / Select Levels** (Multistory Stairs): Go To View dialog -> pick an elevation; Ctrl-click the target levels (highlight blue); Finish; the stair repeats up to the chosen top. Selecting existing stair then Select Levels also works (Tut 3).
- **Instance properties**: Base Level, Base Offset, Top Level, Top Offset, Desired Number of Risers, Actual Tread Depth, Actual Riser Height [knowledge], Tread/Riser Start Number.
- **Type properties**: Stair Calculator rule (Edit button: new rules for riser/tread relationship), Minimum Tread Depth, Minimum Run Width, Maximum Riser Height, Tread Thickness, Riser Thickness, Nosing Length, Monolithic Stairs check, run/landing/support types [partly knowledge]. Stair type used in Tut 3: "Cast-In-Place - Stair - monolithic".
- **Rules**: stairs are parametric from rules (risers/treads/stringers); railings auto-generate on both sides by default and can be deleted/selected separately; railing choice via Tools > Railings dialog before finishing. A warning appears if riser count and height/tread vs rules mismatch (Ramp warning also).
- **IFC4**: IfcStair with IfcStairFlight and IfcSlab LANDING parts [knowledge]; railings IfcRailing.

### Railing
- **Where**: Architecture > Circulation > Railing drop-down: Sketch Path, Place on Host (Stair/Ramp) (Ch 8, "Adding Railings").
- **Sketch Path**: Modify | Create Railing Path; Draw tools (Line default; may use Ref Plane); Tools panel Pick New Host (hosts: floor, stair, ramp, also, 2026: top faces of floors, slabs, slab edges, walls, roofs; balusters follow sloped surfaces [book]); Properties: Type Selector, Base Level, Base Offset, Tread/Stringer Offset; Finish Edit Mode.
- **Place on Host**: attaches railing to whole stair tower or one flight, or to toposurface for fencing/barriers.
- **Type properties**: Rail Structure (Edit: rails), Baluster Placement (Edit: balusters, spacing), Baluster Offset (from sketched path), plus top rail, handrail types. More balusters via Insert > Load from Library > Load Family > English-Imperial > Railings > Balusters.
- **Rules**: associated with host; deleting stairs deletes railing; the railing can be deleted alone.
- **Edit joins**: select railing > Edit Path (Mode panel) > Edit Joins (Tools panel) > hover path joints (snap marks) > click > options bar Rail Join drop-down (join type) > Finish.
- **IFC4**: IfcRailing [knowledge].

### Ramp
- **Where**: Architecture > Circulation > Ramp; Modify | Create Ramp Sketch. Tools panel Railing (choose railing type) and Draw tools (run / boundary / riser) [partly knowledge].
- **Properties**: Width, Base Level, Top Level (+ offsets); Type: Maximum Incline Length (length before landing required), Ramp Max Slope (1/x), Shape (solid vs thick/slab).
- **Interaction**: click start; a rectangle preview shows total size required for current slope; click end; Finish Edit Mode. If the length is too short or long, a warning dialog explains; fix sketch or properties, or ignore.
- **IFC4**: IfcRamp / IfcRampFlight [knowledge].

### Curtain Wall (via Wall tool) and Curtain System
- **Where**: Architecture > Build > Wall > Wall: Architectural; Type Selector under Curtain Wall: Curtain Wall 1 (single panel, no grid), Exterior Glazing and Storefront (predefined grids). Or change existing walls via Type Selector. Pick Lines draw tool makes a curtain wall along model lines/edges of roof/floor/curtain panels/other walls, including curves and lines on other levels. Curved curtain wall appears flat until grids are added [book].
- **Join warning**: converting a wall that is joined to another wall shows "cannot be joined" message; choose Unjoin Elements (Tut 2).
- **Curtain System (by face)**: Architecture > Build > Curtain System; Modify | Place Curtain System by Face; pick faces on mass geometry (flat, singly or doubly curved); Multiple Selection panel > Create System. Type: duplicate to make new; Type Properties have Grid 1 and Grid 2 parameter groups (Grid 1 = vertical lines, Grid 2 = horizontal lines [book]). Apply/OK.
- **Curtain Wall type parameters** (Tut 2): Vertical Grid pattern and Horizontal Grid pattern each with Layout and Spacing; edited via Edit Type; Apply then OK.
- **Curtain Grid**: Architecture > Build > Curtain Grid; Modify | Place Curtain Grid; Placement panel: All Segments (default), One Segment, All Except Picked. Hover near an edge: dashed preview line perpendicular; click. Snaps usable. Add/Remove Segments (Curtain Grid panel in Modify | Curtain Wall Grids): click segment to remove then click empty space; to restore select grid (removed part dashed) > Add/Remove Segments > click dashed part.
- **Panels**: each cell is a panel; default System Panel: Glazed; replace with System Panel: Empty or System Panel: Solid in Type Selector; multi-select by crossing window + Filter (keep Curtain Panels). Doors/awnings: load curtain-wall door/awning families (Insert > Load Family; Doors & Windows folders), make panel the right size by removing grid segments, select panel, choose the door/awning type in Type Selector.
- **Mullion**: Architecture > Build > Mullion; Modify | Place Mullion; types: Circular, L Corner, Quad Corner, Rectangular (default), Trapezoid Corner, V Corner; Placement panel: Grid Line (whole line), Grid Line Segment, All Grid Lines; hover grid line, click; length equals the grid segment. Mullion joins: select mullion > Mullion panel > Make Continuous or Break at Join. Edit Type for profile/size (Tut 2: Rectangular Mullion 1.5"x2.5" = 50x150 mm).
- **Rules**: curtain walls bound rooms [book Ch 7]. IFC4: IfcCurtainWall with IfcPlate (panels) and IfcMember (mullions) [knowledge].

### Copy between levels (Clipboard)
- Select elements (Filter tool narrows by category), Copy to Clipboard (Modify | contextual > Clipboard), Paste drop-down: Paste from Clipboard, Aligned to Current View (e.g. plan -> callout), Aligned to Same Place, Aligned to Picked Level (must be in elevation to click level line), Aligned to Selected Levels (Select Levels dialog, Ctrl multi-select; most used), Aligned to Selected Views (view-specific items such as dimensions). Cut to Clipboard also exists.
- Typical recipe (Tut 1, Ch 8): extend exterior walls (Top Constraint Up to level + Top Offset 3'), select all minus exterior walls and levels (window select, Shift-deselect, or Filter), copy, Aligned to Selected Levels to 2nd..4th floors, then roof on top plan.
## 3.4 Chapter 9 tools: site (Toposolid), property lines, site/parking components, contours

Book states clearly: Revit 2026 replaced the Toposurface category with Toposolid, which behaves like a shape-edited floor merged with the old toposurface (Ch 9, "Creating a Toposolid"). Toposurface-era tools (Split Surface, Merge Surface, Building Pad) are NOT described as tools; sub-division is the replacement, building pad is only mentioned as a comparison (see Open questions).

### Toposolid, Create from Sketch
- **Where**: Massing & Site tab > Model Site panel > Toposolid drop-down > Create from Sketch (other item: Create from Import). Modify | Create Toposolid Boundary tab.
- **Preconditions**: a site plan (plan) view; Level parameter set (Tut: Level 1, Height Offset From Level 0). Type chosen in Type Selector (Tut uses Landscape-Surface, found in the browser under Families > Toposolid > Toposolid).
- **Interaction**: Draw panel: Boundary Line tool (default) with sub-tools (Line, Rectangle, Pick etc.). Sketch a closed boundary (tutorial: Rectangle from upper-left to lower-right corner of the site). Finish Edit Mode on Mode panel; then Modify (Select panel) to exit. Closed loop required.
- **Initial surface**: flat at Level + Height Offset.

### Modify Sub Elements / Add Point / Add Split Line / Pick Supports
- **Where**: select toposolid; Modify | Toposolid > Shape Editing panel: Modify Sub Elements, Add Point, Add Split Line, Pick Supports (+ Reset Shape [knowledge]).
- **Modify Sub Elements**: shows edges and grips; same as for floors (drag vertical, type elevation).
- **Add Point**: contextual Add Point panel: Absolute (default; elevations from project absolute zero) / Relative [button]; Elevation Base drop-down (Current Level default; elevation edit box measured from current level); type Elevation (e.g. 5'0"), press Enter, click inside toposolid to drop points at that height; click more points; Modify to exit. Contours auto-generate between points. Tutorial builds contours low to high: base surface, then points at 5'0", then at 10'0"; intermediate contours appear at the minor interval (1'0").
- Book note: points joined automatically to form a loop and elevations may differ per point (Self-eval T).

### Toposolid properties
- **Instance** (Properties): Constraints: Level and Height Offset From Level (Ch 9); also slope, perimeter, area, volume and other measurement fields (read-only) [book: "slope, perimeter, area, and other measurements"].
- **Type** (Edit Type): Structure > Edit opens Edit Assembly with layers (depth of the ground cross-section = layer thickness; Tut: material Site-Grass, thickness 1'0"); Contour Display > Edit opens Contour Display dialog (see below). Other type parameters as per Duplicate.
- Only the toposolid created in the site view; contours must be built lowest first to be fully visible (tutorial advice).

### Sub-divide (roads, pavements, water)
- **Where**: select toposolid; Modify | Toposolid > Toposolid Shaping panel > Sub-divide; Modify | Create Sub-Division Boundary; Draw tools (Pick Lines used with Tab to chain/choose closed region: hover a line, Tab to select the enclosed region, click). Finish Edit Mode; Edit Sketch (Mode panel) re-enters.
- **Properties**: Type Selector subdivision type (e.g. "Path - 8" Concrete"), Sub-divide Offset (tutorial 0'-3", i.e. offset from parent surface). Type: Structure > Edit Assembly, layer Material column > Browse > Material Browser (e.g. Concrete, Lightweight on Finish 1 layer).
- Creates separate solid on the toposolid with its own material.

### Simplify Toposolid
- Toposolid Shaping panel > Simplify Toposolid; dialog "% points to keep" edit box; OK.

### Excavate / Remove Excavation
- Toposolid Shaping panel > Excavate drop-down: Excavate (toposolid greys out; prompts to pick overlapping floor, roof or other toposolid; Modify to exit); Remove Excavation (click the highlighted excavated solid). Subtractive operations tied to the cutter element.

### Graded Region
- **Where**: Massing & Site tab, Graded Region [book names the Analysis panel; in Revit it sits in the Modify Site panel, knowledge]. Dialog "Edit Graded Region": (1) Create a new topography exactly like the existing one (pick toposolid; copy with all points; add/edit points) or (2) Create a new topography based on the perimeter points only (pick toposolid; add/edit points). Modify to finish: new graded toposolid created. Works only on an existing toposolid.

### Create from Import (CSV / CAD)
- **Where**: Toposolid drop-down > Create from Import; Modify | Create Toposolid from Import: Create from CSV / Create from CAD.
- **CSV**: Select File dialog; file types CSV Files or Comma delimited Text; Format dialog "One unit in the file equals" (units); points imported and surface built from X,Y,Z; extra numeric columns must come after x,y,z.
- **CAD**: first Insert > Import CAD (DWG, DXF, DGN with 3D contour data); then Create from CAD, pick the import; "Add Points from Selected Layers" dialog; OK; surface from elevation points along contour lines.

### Contour display
- Type Properties > Contour Display (Edit): per-row Start (elevation where contours start), Stop (where they end), Interval (spacing), Range Type (Multiple Values vs Single Value; Single Value disables Stop and Increment), Subcategory (e.g. Primary Contours, Secondary Contours; also folding lines, split lines, common edges via Insert row). Tutorial: primary interval 5'0", secondary 1'0"; later secondary changed to 3". Visibility/Graphics > Model Categories > Topography > Primary/Secondary Contours subcategories: line colour via Line Graphics dialog (example blue).

### Label Contours
- Modify Site panel > Label Contours; Modify | Label Contours; Edit Type for font/size; sketch a line crossing the contours (one line across several contours labels each at its elevation).

### Property Line
- **Where**: Massing & Site > Modify Site (or Model Site) > Property Line; plan view only (site plan preferred). Dialog "Create Property Line": (a) Create by entering distances and bearings, (b) Create by sketching.
- **Sketch**: Modify | Create Property Line Sketch; draw tools; Finish. Displays as dashed line.
- **Table**: Property Lines dialog, Deed Data table with 7 columns: Distance, N/S, Bearing, E/W, Type (Line/Arc), Radius, L/R (arc side relative to the segment; enabled for Arc). Buttons: Add Line to Close (inactive until first segment distance is positive), Insert, Delete, Up, Down. OK then click in drawing to place. A sketched property line can be converted with Edit Table (Property Lines panel) - irreversible, drops sketch constraints. Bearings are relative to true north: set Orientation = True North in site view Properties, and Manage > Project Location > Position > Rotate True North.
- IFC4: IfcSite boundary (IfcSite.SiteAddress / IfcCurve in IfcSite.Representation) [knowledge].

### Site Component / Parking Component
- **Where**: Massing & Site > Model Site > Site Component, Parking Component. Modify | Site Component tab. Load Family (Mode panel) from English-Imperial > Site (subfolders Accessories, Logistics, Parking, Utilities (Fire Hydrant, Manhole...)) and English-Imperial > Planting (trees, RPC Tree - Deciduous).
- **Behaviour**: trees and parking spaces are hosted on the toposolid: they snap to the topography elevation at the click point; Properties Level changeable for a selection (Tut: all trees set to Level 2); Parking Component has "Offset from Host" (Tut 3" / 75 mm) and "Rotate after placement". Array tool (Linear) used to copy 7 parking spaces. Move/Flip to orient.
- IFC4: IfcGeographicElement (trees), IfcBuildingElementProxy etc. [knowledge].

### Tutorial note on Sections through the site
- Section tool from plan across the site (see Section), rename to Longitudinal/Transverse; adjust section crop/far clip; Visual Style Shaded; site plan Consistent Colors.
## 3.5 Chapter 10 tools: massing (in-place mass, conceptual mass, Building Maker), custom families

Chapter 10 is present in full. The book covers: three environments for mass creation; Forms (Extrusion, Revolve, Sweep, Blend, Swept Blend) + Void Forms in the Family Editor; In-Place Mass and Place Mass in the project; Conceptual Design Environment (Create Form, X-Ray, Add Profile, Add Edge, Dissolve, Divide Surface mentioned but not detailed beyond a list); Building Maker (Wall, Floor/Mass Floors, Roof, Curtain System by face); Show Mass visibility; families.

### Massing concept (Ch 10, "Understanding Massing Concepts")
- Mass = early-phase volumetric model, building elements can be generated from its faces ("Building Maker"). Limitation: not for detailed geometry (columns, cavity walls, footings). Inclined planes and curved surfaces convert to in-place roofs. Area figures can be extracted from the mass. Use the same model from concept to completion; mass vs shell visibility is controlled per view. 
- Environments: (1) Family Editor (File > New > Family, Generic Model.rft / Metric Generic Model.rft); (2) Conceptual Design Environment (File > New > Conceptual Mass, Mass.rft in the Conceptual Mass folder; ribbon tabs Create, Insert, View, Manage, Modify); (3) Project (Massing & Site > Conceptual Mass panel: In-Place Mass, Show Mass by View Settings, Place Mass; Model by Face panel = Building Maker tools). In the project environment you cannot make 3D reference planes or see 3D levels.

### Forms in Family Editor (Create > Forms panel; Void Forms drop-down same list)
- **Extrusion**: Set work plane first (Work Plane > Set), optionally Reference Planes (Datum panel); draw profile with Draw panel tools; must be a closed loop (several non-intersecting loops OK for extrusion [book Q]); options bar Chain, Depth, Offset; Properties: Extrusion Start and Extrusion End (depth = End - Start; start measured from work plane/base), Material, Visibility. Finish Edit Mode. Re-edit: select form > Edit Extrusion (Mode panel) > sketch tools; change Extrusion Start/End in Properties.
- **Revolve**: work plane perpendicular to horizontal; profile (single closed loop or multiple non-intersecting loops) with Boundary Line; then Axis Line tool (Line or Pick Line); Finish. Properties: Start Angle default 0 deg, End Angle default 360 deg (example 0-180 gives half). Dome/sphere/donut/cylinder.
- **Sweep**: Sketch Path (open or closed) -> Finish; then Edit Profile (Go To View dialog to choose view where the profile plane with a red dot at path intersection appears) or choose a built-in Profile from the Profile drop-down / Load Profile (English-Imperial > Profiles) -> Finish; Finish again. Edit Sweep: Properties Part Type (Mechanical group) and "Can host rebar" check.
- **Blend**: sketch base boundary (work plane set), Properties First End and Second End (heights) or Options bar Depth; Edit Top switches to Modify | Create Blend Top Boundary where the top profile is sketched; Edit Vertices (available once both profiles exist) sets vertex connectivity; Finish.
- **Swept Blend**: Sketch Path (single open segment) or Pick Path (existing open curve/edges); Select Profile 1 > Edit Profile (view chosen in Go To View; profile plane at path end) > Finish; Select Profile 2 likewise; Finish.
- **Void forms**: Void Extrusion / Void Blend / Void Revolve / Void Swept Blend / Void Sweep. Same sketch workflow. After Finish, the void subtracts from any solid it intersects automatically (click away/Esc). Copy/Mirror/Array to repeat voids. E.g. Void Sweep with semicircular profile makes a tunnel.
- **Edit a form**: select, drag arrow controls on faces to resize with live update; or Edit Extrusion etc. Standard Mirror, Copy, Group, Array work on masses.
- **Load into Project**: Modify > Family Editor panel > Load into Project; if several projects are open, Load into Projects dialog with check boxes; family then attached to cursor for placement (Place Component).

### Place Mass
- **Where**: Massing & Site > Conceptual Mass > Place Mass. First use: "Massing - Show Mass Enabled" dialog (Close) then Revit message (Yes) then Load Family dialog (English-Imperial > Mass). Modify | Place Mass: Placement panel Place on Face (hover a face; the mass aligns to it) or Place on Work Plane (options bar Placement Plane drop-down: level); Rotate after placement check. After placement edit instance parameters in Dimensions group.

### In-Place Mass
- **Where**: Massing & Site > Conceptual Mass > In-Place Mass. First use shows "Massing - Show Mass Enabled" (Close); Name dialog (name e.g. Central Hall); enters in-place editor, Create tab.
- **Interaction**: draw lines with Model tool in Draw panel (Modify | Place Lines: Line, Rectangle, Circle, arcs, etc.; Chain option) or Reference tool/ Plane tool for reference lines/planes. Select the sketched profile (clear the tool with Modify first), then Form panel > Create Form drop-down > Solid Form or Void Form. If the profile can make several forms a selection of preview images appears (Tut: two circular previews; choose one). The new form extrudes to a default height with temporary dimensions; click the top face, then click the lower temporary dimension value and type the height (e.g. 60'), Enter. Tutorial: for closed profile, face dragging via arrows or typing dimension. Finish Mass (In-Place Editor panel on Modify tab) exits.
- **Two-profile forms**: selecting two profiles/lines (profile + axis line selected together with Ctrl) and Solid Form gives revolved/lofted forms (dome in Tut: reference vertical axis line + quarter arc, both selected, Solid Form -> revolved dome). Work plane for dome: Set > Pick a plane (an existing wall face); Show confirms.
- **Mirror - Draw Axis** works on sketched profiles (Tut: wings mirrored across the square's diagonal).
- **Void in a mass**: select existing mass, Edit In-Place (Model panel of Modify | Mass), sketch (e.g. Circle from centre found with Plane tool reference planes diagonals; radius typed, Esc Esc to exit), select the circle, Void Form; then select void top/bottom segments and drag or type height. A void profile sketch built with Center-ends Arc + chained Lines can be turned into a void by selecting it and Void Form; set height via top-face temporary dimension.
- **Modify | Form tab** (selected form): Properties (Material, Visibility), Form Element panel: X-Ray (shows skeleton vertices/edges; one form at a time; toggle again to restore), Add Edge (click on a face to add edge), Add Profile (hover surface, preview profile, click; visible only in X-Ray), Dissolve (deletes surfaces but keeps profiles; profiles can be re-edited and re-formed), Edit Profile (Mode panel; select extruded face; sketch mode; Finish), Divide Path (Divide panel, on an edge in editable mass: shows 6 nodes by default; click the number to change count; also divide by distance or by intersections with levels/reference planes/other divided paths; nodes serve as component placement points), Divide Surface (listed, details not in this slice).
- **Edit existing**: select mass > Modify | Mass > Edit In-Place (Model panel).

### Conceptual Design Environment
- Create tab panels: Work Plane (Set, Show, Viewer), Datum (Level, Reference Plane), Draw (Model, Reference, Plane; Draw on Face / Draw on Work Plane toggles; sketching tools Line, Rectangle, Circle, Pick Lines, Point Elements; Chain; Modify tools Offset/Move/Copy), Dimension, Family Editor (Load into Project). Levels can be added in 3D and elevation views. Select sketch -> Modify | Line > Form > Create Form drop-down (Solid/Void). Edit Profile after selecting a face. Tools: Divide Surface, X-Ray, Add Profile, Add Edge, Dissolve.

### Building Maker (Massing & Site > Model by Face)
- **Wall (by Face)**: Modify | Place Wall; Type Selector; Draw panel Pick Faces; options bar Level, Height (Unconnected or level), Location Line; hover mass: convertible faces highlight; Tab selects all contiguous convertible faces. Click face(s) -> wall created.
- **Mass Floors**: select mass(es) (Ctrl) > Modify | Mass > Model panel > Mass Floors; dialog lists levels (check all desired); OK creates floor area faces at each chosen level (also gives area data). Then Floor tool (Model by Face): Modify | Place Floor by Face; choose floor type; hover to highlight area faces; Select Multiple (Multiple Selection panel) with crossing window; Create Floor button. 
- **Roof (by Face)**: Modify | Place Roof by Face; type; click face (horizontal, or from extrusion/revolve/blend/sweep/swept blend); Create Roof button in Multiple Selection panel.
- **Curtain System (by Face)**: Modify | Place Curtain System by Face; type (duplicate type with Grid 1/Grid 2 spacing); click faces (planar or double-curved e.g. dome); Create System. Then Mullion tool to add mullions (All Grid Lines etc.). Door insertion by replacing curtain panel with Door-Curtain-Wall-Double-Glass family type (Filter to Curtain Panels first).
- Association: building elements follow the mass faces; if mass is changed, the Update to Face command refreshes them [knowledge]. Book: after conversion, the mass is still a separate element (self-eval says it can still be viewed as a mass).

### Show Mass
- Massing & Site > Conceptual Mass > Show Mass by View Settings (temporary, not saved with project: masses hidden next open). Persistent way: View > Visibility/Graphics > Model Categories > Mass check box.
- Mass instances get Dimensions group instance parameters; Mass Floors gives gross floor area faces for area study [knowledge].
- IFC4: IfcBuildingElementProxy (mass) [knowledge]; the real converted walls/floors/roofs are IfcWall/IfcSlab/IfcRoof.

### Families (Ch 10, "Creating Families")
- Categories: System families (walls, floors, roofs, ceilings: cannot create new ones, only duplicate and edit types), In-place families (project specific; cannot be saved to library), Standard component families (.rfa, from libraries, loadable; load via Insert > Load from Library > Load Family; Revit 2026: Load Autodesk Family browser window with category tree, preview, Load). Save As > Library > Family saves one or all loaded families (not in-place or system). Nested families supported.
- **Model In-Place** (Component drop-down): Family Category and Parameters dialog (filter list, double-click category) -> Name dialog -> sketch mode with Model and Forms panels; Finish Model.
- **Standard template workflow (arch window example)**: File > New > Family; choose template (e.g. Generic Model wall based); Tile Views; add Reference Planes in the elevation (Create > Datum > Reference Plane; named via Name property: Sash, Glazing, Exterior); dimension reference planes (Annotate > Dimension); label a dimension (right-click > Label > Add Parameter, Parameter Properties dialog) making type/instance parameters; Opening tool (Create > Model > Opening; sketch boundary) for the wall cut; Sweep for frame (path + profile); Extrusion for sash/panels/glass with work plane set to named reference plane, Pick Lines + Tab and locking (padlock) lines to the sash so glass follows geometry; materials via Manage > Materials (Material Browser: duplicate material, rename, appearance asset, Graphics tab "Use Render Appearance"); set Family Category and Parameters (Windows); set element Subcategory (Frame/Mullion, Glass) and Material parameter; Family Types (New Type, Name, set Height/Width values); Save; Load into Project; check each type responds. If types don't respond, check that geometry is locked to reference planes.
- Architectural column tutorial uses unpin of Center reference plane, extrusions with Extrusion Start/End, Void Extrusion subtracting from crowns, Material assigned in Properties (Material and Finishes group), saved via Save As > Family, loaded into a project via Load into Project and placed with Move/Rotate.

### Component-vs-mass summary for IFC4 [knowledge]
- Mass -> no direct IFC class (IfcBuildingElementProxy), converted walls -> IfcWall(Standard Case), floor -> IfcSlab FLOOR, roof -> IfcRoof, curtain system -> IfcCurtainWall (+IfcPlate/IfcMember).

## 4. Acceptance checklist (testable statements)

Ch 6, Levels
- Ch 6 "Adding Levels": the Level tool is unavailable in a plan view and available in elevation and section views.
- Ch 6 "Adding Levels": a new project ships with exactly two levels named Level 1 and Level 2 (an app need not hard-code them but should offer this default).
- Ch 6 "Adding Levels": with Make Plan View ticked, creating a level creates a floor plan, a ceiling plan and a structural plan for it; with it cleared, none.
- Ch 6 "Adding Levels": typing a number while hovering near an existing level creates the new level that vertical distance from it.
- Ch 6 "Adding Levels": clicking the first point sets the start point and the elevation at once; the second click only sets the line length.
- Ch 6 "Adding Levels": a new level is automatically named by the next number (Level 3, Level 4).
- Ch 6 "Modifying Level Parameters": renaming a level prompts whether to rename its associated views; Yes renames plans in the browser.
- Ch 6 "Modifying Level Parameters": editing the elevation temp dimension moves the level; dragging with Shift moves only vertically.
- Ch 6 "Understanding Level Properties": level has 2D and 3D extent modes; a 3D edit changes every view, a 2D edit only the current one.
- Ch 6 "Understanding Level Properties": Level bubble visibility is per end; type has Symbol at End 1/End 2 defaults.
- Ch 6 "Understanding Level Properties": level instance has Elevation, Story Above, Computation Height, Structural (default off), Building Story (default on), Scope Box.
- Ch 6 Tut 1: changing the elevation of Level 2 and adding three levels 10' apart results in 20', 30', 40' elevations.
Ch 6, Grids and datums
- Ch 6 "Creating Grids": grids are auto-numbered 1,2,3 in creation order; renaming by clicking the name works.
- Ch 6 "Creating Grids": Shift while drawing restrains to horizontal/vertical; typing an angle after sketching is allowed.
- Ch 6 "Creating Grids": Line, Start-End-Radius Arc, Center-ends Arc, Pick Lines and Multi-Segment tools exist.
- Ch 6 "Creating Grids": Pick Lines on a wall face creates a grid aligned to that face; an Offset in the options bar shifts it (half thickness gives a centreline).
- Ch 6 "Grid Properties": grid Center Segment options Continuous/None/Custom gate the Custom weight/colour/pattern fields.
- Ch 6 "Controlling the Visibility of Grids": hide in view by element or by category does not affect other views.
- Ch 6 "Reference Planes": a reference plane can be named via the Name property, and named planes appear in the Set Work Plane list.
- Ch 6 "Work Planes": Set Work Plane offers Name, Pick a plane, and Pick a line options; Show toggles a visible grid.
- Ch 6 "Using the Scope Box Tool": scope box can only be created in a plan; a datum assigned to it appears only in views intersecting it; view override can force Visible/Invisible.
Ch 6, Views
- Ch 6 "Elevation Views": the default template has four elevation tags; Interior Elevation tags show four selectable sides.
- Ch 6 "Creating an Elevation View": placing a Building Elevation tag near a wall rotates the arrow perpendicular to it.
- Ch 6 "Creating a Section View": section line has flip, head/tail toggles, break control and a view-depth line.
- Ch 6 "Creating a Segmented Section": Split Segment turns a straight section into orthogonal jogs.
- Ch 6 "Creating a Detail and a Wall Section View": section types are Building, Wall, Detail and land in different browser branches.
- Ch 6 "Using the Temporary Hide/Isolate Tool": temporary hide shows a coloured border and can be reset or applied permanently.
- Ch 6 Tut 3: Paste > Aligned to Selected Levels places copies at the same relative height on each chosen level.
Ch 7, Floors
- Ch 7 "Sketching the Floor Boundary": floor needs a closed loop; an open loop gives a warning and no floor.
- Ch 7 "Sketching the Floor Boundary": Pick Walls picks the wall face nearest the cursor; the flip control switches face; Tab selects the wall chain.
- Ch 7 "Sketching the Floor Boundary": "Extend into wall (to core)" changes the offset reference to the wall core.
- Ch 7 "Sketching the Floor Boundary": a slope arrow can be added in sketch mode with tail level/offset settings.
- Ch 7 "Creating Architectural Floors": floor structure is a layered assembly; total thickness is the sum of layers.
- Ch 7 Tut 1: floor thickness change does not visibly change joins until walls are attached and Height Offset From Level changes.
Ch 7, Roofs
- Ch 7 "Creating Roofs by Footprint": first roof prompts for a level (Lowest Level Notice).
- Ch 7 "Creating Roofs by Footprint": with Defines slope on for all four edges the result is a hip, on two parallel edges a gable, on one edge a shed, none a flat roof.
- Ch 7 "Creating Roofs by Footprint": Overhang offsets the sketch outward from the wall face; Extend to wall core is an option.
- Ch 7 "Creating Roofs by Footprint": finishing the sketch offers to attach walls to the roof.
- Ch 7 "Creating Roofs By Extrusion": the profile is an open chain; a work plane and a reference level+offset are requested first.
- Ch 7 "Modifying Roof Properties": Room Bounding defaults on; Maximum Ridge Height, Volume and Area are read-only.
- Ch 7 "Modifying Roof Properties": Slope is blank and disabled when no edge defines slope.
Ch 7, Shape editing
- Ch 7 "Shape Editing Tools": tools are Modify Sub Elements, Add Point, Add Split Line, Pick Supports, Reset Shape and work on floors/roofs without slope.
- Ch 7 "Add Point": Relative is on by default; elevation 0 adds a point on the surface; clearing Relative uses project elevation.
- Ch 7 "Add Split Line": adding split lines divides the surface into sub-regions that slope independently.
Ch 7, Ceilings and rooms
- Ch 7 "Creating Ceilings": ceilings are visible only in ceiling plans; placement from a floor plan warns.
- Ch 7 "Creating an Automatic Ceiling": clicking inside a bounded region fills that region.
- Ch 7 "Modifying a Ceiling": Height Offset From Level sets ceiling height from its level.
- Ch 7 "Rooms": rooms exist only in plan views; placement in an unenclosed area warns and the room adapts once enclosed.
- Ch 7 "Rooms": automatic placement skips regions smaller than 0.25 sq ft.
- Ch 7 "Calculating Room Volumes": volume is computed only when "Areas and Volumes" is selected (default Areas only).
- Ch 7 "Room Bounding Elements": walls, roofs, ceilings, curtain systems, floors, columns, room separation lines bound rooms.
- Ch 7 "Understanding Room Instance Parameters": default Limit Offset is 10'.
Ch 7, Openings and attach
- Ch 7 "Cutting Openings": five opening types By Face, Wall, Vertical, Shaft, Dormer; shaft cuts all levels unless Base/Top Constraint set.
- Ch 7 "Cutting Openings": By Face cuts perpendicular to the face, Vertical cuts plumb.
- Ch 7 "Attach Top/Base": walls attach to the top of roof/ceiling or base of floor; status in Top is Attached / Base is Attached.
Ch 8
- Ch 8 "Adding Components": Rotate after placement starts a rotate step immediately after the click.
- Ch 8 "Adding Components": Moves With Nearby Elements locks a component to the nearest wall.
- Ch 8 "Adding Stairs": stair has Run, Landing, Support components; Straight, Full-Step Spiral, Center-Ends Spiral, L and U Winder runs.
- Ch 8 "Straight Tool": default Location Line is Run: Center; Automatic Landing is on by default.
- Ch 8 "Adding Stairs": stair base and top default to the current level and next storey; Desired Number of Risers and Actual Tread Depth are editable.
- Ch 8 "Connect Levels Tool": multi-level stairs repeat up to chosen levels.
- Ch 8 "Adding Railings": railing is created by sketch path or on a stair/ramp host; deleting the host deletes the railing.
- Ch 8 "Modifying Railing Joints": Edit Joins sets Rail Join per joint.
- Ch 8 "Adding Ramps": ramp type has Maximum Incline Length, Ramp Max Slope (1/x), Shape.
- Ch 8 "Creating a Curtain Wall": three curtain types exist: Curtain Wall 1, Exterior Glazing, Storefront; Curtain Wall 1 has one panel.
- Ch 8 "Adding Curtain Grids": All Segments, One Segment, All Except Picked placements.
- Ch 8 "Modifying Curtain System Panels": panel types Glazed (default), Empty, Solid.
- Ch 8 "Adding Mullions": six mullion types; Grid Line, Grid Line Segment, All Grid Lines placement; Make Continuous/Break at Join.
- Ch 8 "Using the Pasting Tools": six paste modes listed.
Ch 9
- Ch 9 "Creating a Toposolid": toposolid created in a plan by sketching a closed boundary on a level with a height offset.
- Ch 9 "Modifying Toposolid and Sub Elements": Add Point mode has Absolute/Relative choice and Elevation Base default Current Level.
- Ch 9 "Setting the Contour Display": primary/secondary contours have Start, Stop, Interval, Range Type, Subcategory.
- Ch 9 "Creating Sub-divisions": subdivision has own type, material and Sub-divide Offset.
- Ch 9 "Simplifying Toposolids": a percent of points to keep is requested.
- Ch 9 "Excavating Toposolids": excavation requires an overlapping floor, roof or toposolid and can be removed.
- Ch 9 "Creating a Toposolid from CSV": file needs numeric x,y,z first; import asks the file unit.
- Ch 9 "Adding Property Lines": property line by sketch or by a table with Distance, N/S, Bearing, E/W, Type, Radius, L/R; table conversion is irreversible.
- Ch 9 "Adding Site Components": trees attach to topography elevation at placement.
Ch 10
- Ch 10 "Creating an Extruded Geometry": profile must be closed; depth = Extrusion End - Extrusion Start.
- Ch 10 "Creating a Revolved Geometry": start angle default 0, end angle default 360.
- Ch 10 "Creating a Blend Geometry": Edit Vertices only available once both profiles exist.
- Ch 10 "Creating a Swept Blend": path is a single open segment.
- Ch 10 "Creating Cuts": void forms subtract from intersecting solids automatically.
- Ch 10 "Creating the In-Place Mass": first use shows Show Mass Enabled notice, then asks a name.
- Ch 10 "Creating the In-Place Mass": X-Ray shows only one form at a time; added profile visible only in X-Ray.
- Ch 10 "Dividing Edges or Paths": default 6 divisions, editable by clicking the number.
- Ch 10 "Creating Floors by Selecting Faces": Mass Floors must be generated first for chosen levels, then Floor by Face.
- Ch 10 "Creating Walls by Selecting Faces": Pick Faces with Level, Height, Location Line options; Tab selects all faces.
- Ch 10 "Controlling the Visibility of a Massing Geometry": Show Mass is not saved; persistent via Mass category visibility.
- Ch 10 "Creating Families": system families cannot be created, only duplicated.

## 5. Open questions (ambiguities and book errors)
1. Ch 6 Tut 1 says new interior elevations land under Elevations (Building Elevation) while the text says Interior Elevation has its own branch.
2. Ch 6: wall-section type listed as both Section: Wall Section and under Sections (Wall Sections); numbering of sub-views (1-a, 1-b) is by tag, behaviour of renumbering unknown.
3. Ch 7: slope 4"/12" is printed as 18.46 deg and "8.46 mm"; true value is 18.43 deg [knowledge]. Treat as typos.
4. Ch 7: the book says Maximum Ridge Height is read-only but also lists Slope as editable even with no slope edge; clarify with real Revit.
5. Ch 7: shape editing says available for "roof (without slope)"; unclear whether a roof with defined slope but with sub-element edits is blocked entirely.
6. Ch 7 rooms: the 0.25 sq ft automatic placement minimum and default Limit Offset 10' may differ in metric template.
7. Ch 7 Self-eval item says Join Geometry tool attaches walls to floors (answer False); real tool is Attach Top/Base; Join Geometry exists for floors/walls overlap [knowledge].
8. Ch 8 Tut 3 says railings are created on both sides by default and should be deleted manually; whether this is stair-type driven is not explained.
9. Ch 8: Stair "Convert" to custom stair loses parametric rules [knowledge], book only says message box appears.
10. Ch 9: Graded Region location is given as the Analysis panel (knowledge: Modify Site panel); Building Pad, Split Surface and Merge Surface are mentioned in the self-test but not taught since Toposolid replaced toposurface.
11. Ch 9: subdivision behaviour on edit of the parent toposolid points (does the subdivision follow) is not stated.
12. Ch 9: Property line table converts to true-north bearings; interaction with project base point vs survey point not explained.
13. Ch 10 self-test: 'a mass once converted cannot be viewed as a mass' has answer False, i.e. the mass stays; and the prose-vs-quiz wording on Blend vs Sweep differs. Follow the prose.
14. Ch 10: the book mentions Divide Surface, Dissolve etc. without parameters; Update to Face/Mass Floor re-sync after mass edits is not described.
15. Ch 6 level text says Elevation Base is a type parameter; in real Revit it is an instance/project level setting. 
16. Not in this slice (needs other parts of the book): walls, doors, windows, columns, beams, foundations, structural joins, grid-to-column association, Copy/Monitor, worksets.
