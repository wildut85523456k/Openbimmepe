# Revit 2026 Architecture reference, part 1 (Chapters 1-5)

Source: "Exploring Revit 2026 for Architecture" (CADCIM). Text extraction `arch.txt`, lines 1134-10368.

| Chapter | Title | Approx. line range in arch.txt |
|---|---|---|
| 1 | Getting started (concepts, UI, interoperability, worksharing) | 1134-2130 |
| 2 | Starting an architectural project (templates, units, snaps, save/open, navigation) | 2131-3807 |
| 3 | Placing architectural walls (wall types, sketching, stacked, sweeps, reveals) | 3808-6304 |
| 4 | Doors, windows, openings | 6305-7988 |
| 5 | Modifying building elements (selection, move/copy/trim/rotate/mirror/offset/array/align/split/groups/parts/pin/scale/assemblies) | 7989-10367 |

IMPORTANT SCOPE NOTE: contrary to the brief, **levels and grids (datums) are NOT covered in Ch 1-5**. The book only mentions levels in passing: "Level 1 / Level 2" come pre-made by the template, walls take Base/Top Constraint from them, and Split Element on a wall in elevation snaps to level lines (Ch 3, Ch 5). Level/Grid tools start in Ch 6 and belong to the next slice. Chapter 4 is doors/windows/openings and Chapter 5 is the modify tools. Dimension-edit, Level, Grid and Reference Plane behaviour must come from the part-2 file.

Conventions: `(Ch N, "heading")` cites the source. `[book]` = stated by the book. `[knowledge]` = my own Revit/IFC knowledge, not in the book. Imperial example values are quoted with metric equivalents where the book gives them. All text is paraphrased.

---

## 1. Tutorial spine (build order from scratch)

The book builds three running projects (Apartment 1, Club, Residential/Office) across chapters; each tutorial saves a file consumed by the next one. The dependency chain is the useful part.

### Ch 2 - project setup (no geometry)
1. Start Revit, File > New > Project, choose template (`.rte`): `default.rte` (metric: `DefaultMetric.rte`) for apartment; `Commercial-Default.rte` for club. [book]
2. Manage > Settings > Project Units: Length = Feet and fractional inches (metric: Millimeters), rounding e.g. nearest 1/2" (or 1 decimal place). [book]
3. Manage > Settings > Snaps: type custom Length dimension snap increments (e.g. `5'; 2'6"; 3"; 0'1/2"`; metric `1500; 750; 75; 10`), untick Quadrants/Centers/Tangents/Points object snaps for the apartment (all snaps on for the club). [book]
4. Manage > Settings > Additional Settings > Annotations > Temporary Dimensions: set Walls and Doors-and-Windows to **Centerlines**. [book] (Later notes say switch walls to Faces if temp dims do not update.)
5. Save As > Project (`.rvt`), then Close. [book]
Dependency: units, snaps, temp-dim reference mode live in the project file and are inherited by every later tutorial. Template supplies Level 1 and Level 2 and starter views.

### Ch 3 - walls
Tutorial 1 (Apartment exterior): open Ch2 file -> Architecture > Build > Wall > Wall: Architectural -> Type Selector = Basic Wall "Exterior - Brick on Mtl. Stud" -> Properties: Location Line = Wall Centerline, Top Constraint = Up to level: Level 2 -> Line tool, Chain off -> click start, move right (horizontal alignment dashed line appears), click past 40' -> select wall, click temp dim, type 40' Enter -> next wall starts by clicking the Endpoint snap of the first wall, move down, type 20' Enter (corner auto-cleans) -> enable Chain, click end snap, move left holding Shift, click at the Intersection snap shown from the first wall start (plumb alignment) -> type 25' up Enter -> Esc twice -> select the 4th wall, drag its top blue end-dot onto the first wall start (Endpoint+Vertical tooltip) to close the loop. [book]
Tutorial 2 (Club exterior, inclined): Wall type Split Face and CMU on Mtl. Stud, Unconnected Height 15'0", Location Line = Finish Face: Exterior, Chain on; draw first wall at 45 deg by typing length 14'; then right-click > Snap Overrides > Perpendicular for each following segment and type lengths (60', 40', 60', 60', 40', 60', 14'); Esc; then Center-ends Arc: click centre at outer intersection, type radius 27' when angle reads 135 deg, click end on endpoint snap. Then 3D view, Shaded visual style. [book]
Tutorial 3 (Apartment interior): open Ch3 T1 file; type Interior - 5" Partition (2-hr), Location Line Wall Centerline, Chain off; first wall starts by hovering near the top-right endpoint and typing the distance 15' then Enter (this "types the first point as an offset from a snap") then click on the opposite exterior wall at the Vertical+Nearest snap; further walls repeat with Shift to lock ortho. Because bath dimensions are clear (face-to-face), then select a wall and click the blue witness-line dots to move the dimension reference from centerline to the face, then edit the temp dim to the clear distance. [book]
Tutorial 4 (Club interior): same pattern with Unconnected height 12'0", walls at 135/45 deg with Nearest and Midpoint snaps. [book]
Tutorial 5 (Office/house with stacked wall): new project from default template; choose stacked wall type "Stacked Wall Exterior - Brick over CMU w Metal Stud"; Edit Type > Duplicate (new name) > Structure Edit > replace sub-wall types per row (top row variable height, other row fixed 5'); Unconnected Height 12'; Location Line Finish Face: Exterior; Rectangle tool; then select top wall, "Activate Dimensions" in Options Bar, edit temp dims to 45' and 60'; then interior walls Interior - 5 1/2" Partition (1-hr) with typed offsets. [book]

### Ch 4 - doors/windows (hosted on Ch3 walls)
Tutorial 1 (Apartment): open Ch3 T3 file -> Level 1 plan -> Architecture > Build > Door -> select door type -> click wall near target -> Modify -> select door -> click temp dim, type exact distance -> flip with arrows -> load extra door families via Load Family (Doors > Residential/Commercial) which opens "Specify Types" -> doors/passage openings: Insert > Load from Library > Load Family > Openings > Passage Opening-Cased -> Architecture > Build > Component > Place a Component -> Edit Type > Duplicate (e.g. 48"x84") and set Width -> windows: Window tool, Fixed 24"x48", duplicate to 48"x48" with Width changed -> renumber window tags by double-clicking the tag number (confirmation dialog, all instances of the type update). [book]
Tutorial 2 (Club): doors from Residential/Commercial families; entrance double-glass door; Spacebar before click flips swing; angular temp dim edited to place a window on the curved wall (angle value like 6). [book]

### Ch 5 - edit tools
Tutorial 1 (Apartment): place a window; Copy with Constrain on, using Midpoint snap start and typing 2'0"; select the two windows (pick box), Create Group (name 2XW3); Mirror - Pick Axis using a window midpoint as the axis; drag the group from Project Browser Groups > Model onto another wall; Activate (Controls panel) dimensions to position it. [book]
Tutorial 2 (Club): Linear Array (Move To = 2nd, Number = 4, Group and Associate on, Constrain on, spacing typed 4'0"); Radial Array (Last, Number 3, move centre with Place to the arc wall's centre, angle typed 27); Ctrl-click windows -> Create Group; Mirror - Pick Axis on interior wall centerline; Mirror - Draw Axis with two points; Copy group using group origin / midpoint to endpoint of another wall. [book]
Tutorial 3 (Office): place doors/windows (Tag on Placement off); Copy with Multiple on, Constrain off; Mirror pick axis; Filter dialog -> Check None -> tick Walls -> Create Assembly ("Wall Assembly") -> Create Views (plan detail, schedules, material takeoff, parts list) -> drag views onto a sheet. [book]
Exercise 5: select everything with a crossing window, Copy to Clipboard, Paste > Aligned to Selected Levels onto Level 2, delete one door. [book]

Overall build order recommended by the book (Ch 3, "Sequence of Creating a Building Model"): exterior walls on lowest level -> interior walls -> doors/windows -> floor -> roof -> grids/structure -> stand-alone components -> text/annotation -> dimensions -> details -> rendered views. Upper levels are defined from story height and walls linked to levels via constraints (Ch 1, "Creating a Building Model Using Parametric Building Elements").

---

## 2. Core concepts (Ch 1)

### 2.1 Element classification
- Categories: Model, Annotation, Datum, View (plus Imported/Workset/Filter/Revit Links when present). [book]
- Five element classes by usage: host (wall, floor, roof, ceiling), component (door, window, furniture; hosted or stand-alone), annotation (2D view-specific), view, datum (grid, level, reference plane). [book]
- Hierarchy: Category > Family (`.rfa`) > Type (specific size/style, e.g. "Double Hung: 36" x 48"") > Instance (a placement). All instances of a type share type properties; instance properties are per placement. Editing type properties updates all instances. [book]
- Walls: system family "Basic Wall" with types such as "Brick on Mtl. Stud". [book]
- Levels are described as infinite horizontal planes that host floors/roofs/ceilings; each element belongs to a level. [book]
- Parametric relationships: some automatic (door needs a host wall, door moves with the wall, floor edges follow walls), some user defined (constraints); bidirectional associativity ("change anywhere, change everywhere"). [book]
- Project = single database file `.rvt`; template = `.rte`. Any project can be saved as a template. Selecting template "None" yields Revit defaults. [book]

### 2.2 View concepts
- Scale per view; detail level Coarse/Medium/Fine (auto from scale, overridable per view); visual styles Wireframe, Hidden Line, Shaded, Consistent Colors, Realistic/Textures. Visibility/Graphics overrides per view and per category. [book]
- Scale is a view property, so annotation size derives from it. [book]

### 2.3 IFC statements in the book (Ch 1, "Interoperability")
- Revit certified for IFC import (IFC2x3, 2x2, 2x; IFC4 only as link) and export (IFC4, IFC2x3, IFC2x2). [book]
- Standard elements map to IFC automatically ("walls export as IfcWall"); other families (escalators) need explicit mapping. [book]
- Door/wall/window instance and type parameters expose: Export to IFC, Export to IFC As, IFC Predefined Type, IFC GUID (type level: Export Type to IFC As, Type IFC Predefined Type, Type ifcGUID). [book] (Ch 3, Ch 4 tables)

---

## 3. Application/UI behaviours (Ch 1-2)

### 3.1 Project Browser
- Tree of views, legends, schedules, sheets, families, groups, links; 2026 makes it tabbed per type, with a search box. Double-click a view name opens it; current view is bold. Plus/minus expands/collapses. Toggle via View > Windows > User Interface > Project Browser. [book]
- Level the active element was created on is shown in bold under the plan views (the book uses this to confirm the wall belongs to Level 1). [book]
- Template-provided views: Floor Plans (Level 1, Level 2...), elevations, 3D `{3D}`. [book]

### 3.2 Properties palette and Type Selector
- Modeless, dockable, resizable. Shows instance properties of the active tool/selection; with nothing selected it shows the active view's properties. [book]
- Type Selector at the top lists loaded types of the current family category; choosing a different type on a selected element replaces its type. Edit Type opens the Type Properties dialog (Duplicate, Rename, Load, Preview). [book]
- Properties are grouped in collapsible categories (Constraints, Structural, Dimensions, Identity Data, ...). Edits are applied with Apply (instance values). [book]
- Type Selector can be added to the quick access toolbar. [book]

### 3.3 Options Bar, Status Bar, View Control Bar
- Options Bar content depends on active tool (wall: Height/Depth, level or Unconnected, Location Line, Chain, Offset, Radius, Join Status). [book]
- Status Bar shows family:type of the element under the cursor and prompts. Bottom-right: selection filter icon with count, Select Links, Select Underlay Elements, Select Pinned Elements, Select Elements by Face, Drag Elements on Selection. [book]
- View Control Bar (bottom-left of view): Scale, Detail Level, Visual Style. [book]

### 3.4 Keyboard accelerators (all as in the book)
| Keys | Action |
|---|---|
| DR | Door |
| MD | Modify (exit tool) |
| MV | Move |
| RO | Rotate |
| MM / DM | Mirror Pick Axis / Draw Axis |
| OF | Offset |
| AR | Array |
| DE | Delete (also Delete key) |
| PN | Pin |
| GP | Create Group |
| SA | Select all instances |
| VG | Visibility/Graphics |
| SO | Snaps off/on toggle |
| SE | Temporary endpoint snap override (similar snap codes exist) |
| Ctrl+C/X/V | Clipboard |
| Ctrl+Left | Select previous (restore previous selection) |
| Ctrl+S, Ctrl+O, Ctrl+P | Save / Open / Print |
| F7 | Spell check |
| F8 | SteeringWheels |
| Tab | Cycle highlighted candidates; on a wall selects its chain |
| Shift | Constrain to orthogonal while drawing/moving |
| Spacebar | Flip orientation when placing |
| Esc | Cancel/exit; usually press twice to leave a tool and clear selection |
Keyboard shortcuts can be exported to / imported from an XML file in 2026. [book]

### 3.5 Contextual tabs
Selecting an element or invoking a tool shows a "Modify | <thing>" tab with the Modify, Clipboard, Geometry, Create, Properties, Measure panels etc. Option "Stay on Modify tab" vs "Return to previous tab" after finishing a tool. [book]

### 3.6 Options dialog (File > Options), requirements-relevant items
- General: save reminder interval default 30 minutes; user name from Autodesk account; journal-file cleanup thresholds; default view discipline (Architectural, Structural, Mechanical, Electrical, Plumbing, Coordination). [book]
- Graphics: "Simplify display during view navigation" and "Allow navigation during redraw" default on; temporary dimension text size/background; anti-aliasing; accelerated graphics preview toggled by right-click. [book]
- Hardware: use hardware acceleration default on; Draw visible elements only default on. [book]
- File Locations: default template, user-file path, family-template path, "Places" list for Open dialog. [book]
- ViewCube: show, position, size, inactive opacity, snap to closest view, fit-to-view on change, animated transitions, keep scene upright, compass. [book]
- SteeringWheels: size, opacity, invert vertical look, walk speed factor, move parallel to ground plane, zoom one increment per click, orbit keeps scene upright. [book]
- UI theme: Dark / Light / system. Tooltip assistance None/Minimal/Normal/High (default None). [book]
- Save options: max backups default 3 (non-workshared) / 20 (workshared); thumbnail source view. [book]

### 3.7 Project Units (Manage > Settings > Project Units)
- Disciplines: Common, Structural, HVAC, Electrical, Piping, Energy, Infrastructure. Common holds Angle, Area, Cost per Area, Distance, Length, Mass Density, Rotation Angle, Slope, Speed, Time, Volume, Currency. [book]
- Each parameter's Format dialog: Units, Rounding (including Custom with Rounding increment, default increment 1), Unit symbol, Suppress spaces (feet-inches). Custom rounding is inactive for Feet and fractional inches, Fractional inches, Meters and Centimeters. Decimal symbol / digit grouping selector at dialog bottom. [book]
- Default area unit square feet; default slope for imperial "Rise / 12"". [book]
- Display only: formatting affects how values are shown, not stored value (rounding wall length to nearest 1' does not truncate real length). Units may be changed any time. [book]

### 3.8 Snaps (Manage > Settings > Snaps)
- Dimension snaps: Length increments (semicolon-separated list; imperial defaults `4'; 0'6"; 0'1"; 0'0 1/4"`, metric `1000; 100; 20; 5`) and Angular increments. Cursor jumps through the largest increment first and finer ones as you zoom in [knowledge]. Custom lists such as `5'; 2'; 4"` are typical. [book]
- Object snaps: Endpoints, Midpoints, Nearest, Work Plane Grid, Quadrants, Intersections, Centers, Perpendicular, Tangents, Points, Snap to Remote Objects, Snap to Point Clouds; all enabled snaps work at once; Check All / Check None; "Snaps Off" master checkbox or type SO. [book]
- Marker shapes: endpoint square, midpoint triangle, nearest cross, intersection X (book gives these); tooltip with the snap name appears near the cursor. [book]
- Temporary overrides: type a two-letter code (e.g. SE for endpoint) for single use; also right-click > Snap Overrides > e.g. Perpendicular while drawing. Tab toggles among snap points at the same position. Snaps work only on visible objects. [book]
- Settings apply across the session but are stored in the project. [book]

### 3.9 Navigation
- Zoom (navigation bar): Zoom in Region (two diagonal corners), Zoom Out (2x), Zoom to Fit, Zoom All to Fit (all open windows), Zoom Sheet Size, Previous/Next Pan/Zoom. [book]
- ViewCube in 3D views: 26 hotspots (6 faces = orthographic, 12 edges + 8 corners = angled), home icon, compass ring, right-click > Orient to a Direction. [book]
- SteeringWheels (F8): 2D wheel (Zoom, Pan, Rewind); 3D Mini/Big View Object (Pan, Zoom, Rewind, Orbit), Tour Building (Up/Down, Look, Walk, Rewind), Full Navigation (8 wedges); hold left button on a wedge and drag, release to end. [book]
- Default 3D view: View > Create > 3D View > Default 3D View, or double-click {3D}. [book]

### 3.10 File operations
- Save As > Project; first Save prompts Save As; Save updates silently afterwards. Close prompts Yes/No/Cancel when unsaved. Opening an older-version file triggers a one-time upgrade. Opening an already open modified file asks to close it first. Multiple projects open in one session (Switch Windows, Tile, Tab); Cut/Copy/Paste between them. [book]

---

## 4. Tools (one section per tool)

### 4.1 Wall: Architectural
**Where:** Architecture tab > Build panel > Wall drop-down > Wall: Architectural. Contextual tab "Modify | Place Wall". Shortcut: not given in the book [knowledge: WA]. Walls can be drawn in plan or 3D. [book]

**Preconditions:** an active plan (or 3D) view; a level in the project (the template provides Level 1 and 2). No host needed. Interior walls drawn with the top constraint set to a level above the current one automatically. [book]

**Interaction sequence (Line tool, default)**
1. Pick type in the Type Selector; set Location Line, constraints in Properties or Options Bar.
2. Click start point. A wall rubber-bands with a live temporary dimension (length) and angle. [book]
3. Cursor moves in increments from the length dimension snaps; near horizontal/vertical a dashed alignment line and a tooltip with a two-arrow glyph appears. [book]
4. Finish by (a) clicking the end point, or (b) typing a length (an edit box appears on the temp dimension as soon as you type) and Enter, which creates a wall of that exact length in the cursor direction. [book]
5. Chain off: the tool is ready for a new first point after each wall. Chain on: each wall's end becomes next start. Esc ends the chain; Esc a second time leaves the tool (the book says press Esc twice). [book]
6. Shift while moving constrains to orthogonal; a plumb/aligned intersection snap (X) appears when aligned to another point. [book]
7. Angle can be typed after placing by selecting the wall and clicking the angular temp dim. [book]
8. Typing an offset distance from a snapped neighbour: hover a snap point on an existing wall, move along the wall, type a number and Enter to set the first point at that distance from the snap (used in tutorials 3 and 4). [book]
9. Wall faces: walls drawn left-to-right place their exterior on the upper side; top-to-bottom puts exterior on the right; right-to-left exterior is lower; bottom-to-top exterior left. Reduce flipping by choosing the drawing direction (see Open questions 1). [book]
10. After placement select the wall to show: two blue flip arrows on the exterior face (click, or hover until the colour changes then Spacebar, to flip), blue drag dots at both ends (stretch), temporary dimension with a conversion symbol (turn into a permanent dimension), and two witness-line dots on dimensions that you can click to switch the reference between centerline and faces. [book]

**Sketch tools (Draw panel):** Line; Rectangle (two diagonal corners; temp dims on both sides; 4 walls auto-joined); Inscribed Polygon and Circumscribed Polygon (Options Bar "Sides" count; click centre then radius); Circle (centre click, then radius; typed value = distance from centre to the location line); Start-End-Radius Arc (start, end, then a third click for radius); Center-ends Arc (centre, start/radius, end; swept up to 180 deg, more flips side); Ellipse (centre, first radius, second radius); Tangent End Arc (start at existing wall end, then end point); Fillet Arc (click two walls then fillet position; walls trimmed automatically; radius editable afterwards). Pick Lines/Pick Faces exist in Revit [knowledge] but are not described in these chapters. [book]
Modifying curved walls after placement: select, edit angular and radius dims; drag controls change extent; centre dot stretches while keeping the subtended angle. [book]

**Options Bar fields**
| Field | Meaning | Values/defaults |
|---|---|---|
| Height / Depth dropdown | wall goes up (Height) or down (Depth) from the base | Height default |
| Level / Unconnected | top reference: pick a level or Unconnected | Unconnected enables the height edit box; default unconnected height 20' (book Ch 3) |
| Location Line | which line of the wall coincides with the sketch line | Wall Centerline (default), Core Centerline, Finish Face: Exterior, Finish Face: Interior, Core Face: Exterior, Core Face: Interior |
| Chain | continuous segments | toggle, may be changed mid-tool |
| Offset | distance from the picked/sketched line to the wall's location line | numeric |
| Radius | radius for circle/arc/fillet | numeric |
| Join Status | Allow / Disallow joining while sketching | "Allow" used in tutorials |
| Sides | polygon tools | integer |
| Activate Dimensions | (for selected stacked wall / pasted) shows dims | button |
All [book].

**Instance properties** (Properties palette; Ch 3 table):
Location Line; Base Constraint (level or reference plane); Base Offset; Base is Attached; Base Extension Distance; Top Constraint (Up to level X, or Unconnected); Unconnected Height (default 20'0"); Top Offset; Top is Attached; Top Extension Distance; Room Bounding (checkbox: wall bounds rooms); Related to Mass; Cross-Section (Vertical/Slanted/Tapered); Angle From Vertical (slanted); Override Type Properties + Exterior/Interior Angle (tapered); Structural Usage (e.g. Non-bearing makes it architectural; Bearing, Shear, Structural combined); Area and Volume (read-only computed); Image; Comments; Mark; Export to IFC; Export to IFC As; IFC Predefined Type; IFC GUID. Phase Created / Demolished exist as for other elements [knowledge]. [book unless noted]

**Type properties** (Edit Type): Structure (Edit Assembly), Function (Interior, Exterior, Retaining, Foundation, ...), Coarse Scale Fill Pattern, Wrapping at Inserts / at Ends, Width (read from layers). Analytical Properties: Thermal Mass, Thermal Resistance (R), Heat Transfer Coefficient (U) read-only (computed from layer materials); Absorptance and Roughness editable. [book]
Edit Assembly dialog: layer table top = exterior side, bottom = interior side; each row has Function, Material, Thickness, Wraps, Variable; buttons Insert, Delete, Up, Down; Preview with Section view; "Total thickness" readout sums layers; Default Wrapping At Inserts = Exterior / Interior / Both / Do not wrap; At Ends = Exterior / Interior / None. [book]
Layer Functions: Structure [1], Substrate [2], Thermal/Air Layer [3], Membrane Layer (zero thickness), Finish 1 [4] (exterior side), Finish 2 [5] (interior side). Bracket numbers are default join priorities. A Priority column lets users override: a layer of priority N in wall A joins the same priority in wall B even through lower-priority layers, but never passes through an equal/higher priority layer. [book]

**Predefined wall type families:** Exterior (Brick on CMU, Brick on Mtl. Stud, CMU Insulated, ...), Interior partitions (drywall on metal stud, various thicknesses and fire ratings, named like "Interior - 5" Partition (2-hr)"), Retaining (e.g. "Retaining - 12" Concrete"), Foundation ("Foundation - 12" Concrete"), Curtain, Stacked. Prefix naming ("Exterior", "Interior") is convention only; Function is the data field. Any type can be switched to retaining by changing the Function [knowledge-ish; the book says select Retaining type from the palette]. [book]

**Rules & behaviours**
- Wall is a host for doors/windows; deleting a wall deletes dependent elements. [book]
- Joins: wall ends that meet are joined automatically ("intuitively filled"); layers merge by priority; interior walls and exterior walls with different layers do not form a neat junction without manual adjustment. [book]
- Moving a wall by drag auto-adjusts joined walls. Tab over a wall selects the entire chain of joined walls; works only if properly joined. [book]
- Location Line is the stable reference plane when thickness or composition later changes; changing the type's thickness keeps the location line fixed so the exterior/interior face moves, not the line. [book]
- Height = Base Constraint + Base Offset up to Top Constraint + Top Offset, or Base + Unconnected Height. Changing a level's elevation moves walls constrained to it (follows from constraints) [knowledge; the book says walls are linked to levels]. [knowledge]
- Walls are created in plan or 3D; sweeps and reveals only in 3D/elevation. [book]
- Tapered/slanted: Cross-Section = Slanted + Angle From Vertical; Tapered requires a type with variable thickness (layers marked variable), then Override Type Properties enables Exterior Angle and Interior Angle. [book]
- Place wall finish by Room (new in 2026): Placement panel > "Place by Room": hover inside a closed bounded space, all enclosing walls highlight, one click adds a finish wall on every interior face; heights/props editable per segment. "Place by Segment": hover a single face of an existing wall, click to add finish wall on that face. [book]
- Join status "Allow". [book]

**Gotchas (book):** end-to-end snapping needed or the Tab chain-select fails; dimensions typed refer to the Location Line (e.g. circle radius is to the location line); walls cannot be rotated in elevation; constraints are in plan terms when using dimension "witness lines" - temp-dim default reference is centerline so clear distances need switching to faces; exact dimension of a wall can differ from a rounded displayed value because of project unit rounding.

**BIM outputs:** Area, Volume, Structural Usage, Room Bounding, Function (Exterior/Interior), U/R/Thermal Mass (type, analytical), phases, fire-rating via type name/param [knowledge]. IFC: IfcWall (IfcWallStandardCase legacy in 2x3) with PredefinedType; Export to IFC As can override. [book: "walls export as IfcWall"; knowledge for predefined types]. Wall layers map to IfcMaterialLayerSet [knowledge]; Pset_WallCommon (FireRating, IsExternal, LoadBearing, ThermalTransmittance) [knowledge].

### 4.2 Stacked wall
**Where:** same Wall tool; choose a stacked type in Type Selector. Edit Type > Duplicate > Structure Edit. [book]
**Procedure:** in the Edit Assembly grid each row is a basic wall type with a Height; exactly one row (default first) is "Variable" and takes the leftover height; select another row and press Variable to move the variable role; Insert/Delete rows, Preview. OK twice. Draw as a normal wall; set Unconnected Height or constraints on the stacked instance. [book]
**Rules:** the facade behaves as one wall (one instance, selectable as stacked wall with contextual tab "Modify | Stacked Wall"). Component walls must be basic walls. [book]
**IFC:** [knowledge] exports as one IfcWall with layer material set or parts; not described in the book.

### 4.3 Wall Sweep
**Where:** Architecture > Build > Wall drop-down > Wall: Sweep. Tab "Modify | Place Wall Sweep" with Placement panel: Horizontal (default) or Vertical. [book]
**Preconditions:** must be in elevation or 3D view; a wall face to hover. [book]
**Sequence:** pick sweep type in Type Selector; hover a wall face - a preview and temp dims show; click at the height wanted (horizontal) or position (vertical); continue clicking other walls; Esc twice or Modify to leave. [book]
**Type properties:** Profile (a profile family; changes the cross-section), Material, Constraints/Construction/Material/Identity groups, Wall Subcategory ("Subcategory of Walls"); Duplicate/Rename. [book]
**Instance properties:** Offset From Wall, Level (horizontal only), Offset From Level, Length, Image, Comments, Mark, Phase Created, Phase Demolished, Orientation. [book]
**Behaviour:** purely a modifier of the host wall (adds baseboard, crown, projections). [book]

### 4.4 Wall Reveal
**Where:** Architecture > Build > Wall drop-down > Wall: Reveal; contextual "Modify | Place Reveal" with Horizontal / Vertical. Elevation or 3D only. [book]
**Type properties:** Default Setback (default 0'0"; sign changes the cut side; useful near trims), Profile. Instance: Offset From Wall, Level, Offset From Level, Length, Orientation. [book]
**Behaviour:** adds a decorative cutout groove. [book]

### 4.5 Door
**Where:** Architecture tab > Build panel > Door; shortcut DR. Contextual "Modify | Place Door". [book]
**Preconditions:** a wall must exist to host the door (cannot be placed otherwise, Ch 1/Ch 4). Works in plan, section, elevation or 3D. A door may go in any wall type; Revit warns if the placement is inappropriate. [book]
**Interaction:**
1. Choose door type in the Type Selector (loaded types only; more via Load Family in the Mode panel; Doors sub-folder Residential/Commercial; the Specify Types dialog lets you choose which sizes to load). [book]
2. Move over a wall: a preview of the door appears; the swing/hinge side and facing follow which face of the wall (upper/lower) the cursor is nearest to; temp dimensions show the distance from the door centre to adjacent walls/edges. [book]
3. Spacebar before clicking flips swing; click to place. A tag is placed if "Tag on Placement" is on (Tag panel; horizontal or vertical tag). [book]
4. Esc twice to leave. Selecting a door shows: horizontal twin arrow (flips hinge/swing side) and vertical twin arrow (flips facing, i.e. which side of the wall the swing is on), temp dims, tag. Click the temp dim number and type to position exactly; the door moves. [book]
5. Moving along the host wall by dragging closes the old opening and cuts a new one. To move to another wall use Move with Disjoin (see Move). [book]
6. Door in a slanted wall: after placement, select the door and set Orientation = Slanted (aligns to the wall slope). [book]
7. Door changes type via Type Selector on a selected door. [book]
8. Model In-Place (Mode panel) creates a one-off door in the family-editor-like in-place mode; Load Family adds types. [book]
**Instance parameters:** Sill Height (distance from level), Door Offset (from wall instance), Frame Type, Show Grill, Threshold, Frame Material, Finish, Image, Comments, Mark (increments automatically per door), Phase Created, Phase Demolished, Threshold Offset, Head Height, Orientation (Vertical / Slanted), Swing Angle (2D plan swing display), Level, IFC Predefined Type, Export to IFC As, Export to IFC, ifcGUID. "Level" and "Frame Material" are instance; "Thickness" and "Fire Rating" are type (self-test). [book]
**Type parameters (Edit Type):** Function (Interior/Exterior, used for scheduling), Wall Closure (layer wrapping), Construction Type, Door Material, Frame Material, Thickness, Height, Width, Trim Projection Ext / Int, Trim Width, Rough Width, Rough Height, Assembly Code, Keynote, Model, Manufacturer, Type Comments, Fire Rating, Cost, Assembly Description, Type Mark, OmniClass Number/Title; Analytical Properties: Thermal Resistance (R), Heat Transfer Coefficient (U), Solar Heat Gain Coefficient, Visual Light Transmittance; Operation (type), Export Type to IFC As, Type IFC Predefined Type, Type ifcGUID. [book]
**Rules:**
- Placing a door cuts an opening and the wall edges are completed automatically (opening depends on the door; delete the door and the wall closes). [book]
- Door tags: Mark increments with each placed or copied door; mark can be set manually; tag visibility is a per-view annotation category (VG > Annotation Categories > Door Tags); hiding tags keeps them in the project. [book]
- Renaming a type does not change its dimensions (e.g. renaming 34"x84" to 36"x84" keeps width 34"). The book recommends duplicating a type before editing a predefined one. [book]
- Type changes affect all instances; instance parameters affect only the instance.
**BIM outputs:** Fire Rating, Function, U-value, SHGC, VLT, Rough sizes for schedules, Cost, OmniClass, Mark/Type Mark; IFC: IfcDoor (Operation type -> PredefinedType / OperationType), Pset_DoorCommon [knowledge]; IfcOpeningElement created implicitly with IfcRelVoidsElement and IfcRelFillsElement [knowledge].

### 4.6 Window
**Where:** Architecture > Build > Window; contextual "Modify | Place Window". (No shortcut quoted in the book; WN in stock Revit [knowledge].) [book]
**Interaction:** same as the door: choose type, hover wall (preview + temp dim from window centre to neighbours), click on the face you want (upper/lower face determines the interior side), select to see flip arrow on the inner side, click temp dim to type exact distance. Window tags are auto-generated and, unlike doors, all windows of one type share the same mark; changing the tag number by double-clicking it asks for confirmation (Yes) and then renames the type mark for all windows of that type. [book]
**Instance:** Level, Sill Height, Image, Comments, Mark, Phase Created/Demolished, Head Height, Orientation. In the self-test: Assembly Code cannot be modified in Properties (it is a type value) while Level, Head Height, Mark can. [book]
**Type:** Wall Closure, Construction Type, Glass Pane Material, Sash Material, Height, Default Sill Height (default 3'0"), Width, Window Inset, Rough Width, Rough Height, Keynote, Model, Manufacturer, Type Comments, URL, Description, Assembly Description, Type Mark, Cost. [book]
**Slanted/tapered walls:** Orientation = Slanted aligns to the slope; for tapered walls Orientation = Slant along exterior / Slant along interior. [book]
**Rules:** inset is relative to wall face; width/height are type parameters (the Properties palette does not edit them), so to make a new size duplicate the type and set Width. [book]
**IFC:** IfcWindow (PartitioningType, PredefinedType) [knowledge].

### 4.7 Opening families as wall openings (Passage Opening, etc.)
**Where:** Insert > Load from Library > Load Family > Openings folder; then Architecture > Build > Component > Place a Component. Families named Opening with Trim, Opening, Opening-Door, Passage Opening-Cased, Opening-Window-Round, Passage Opening-Elliptical Arch, Opening-Window-Square. [book]
**Behaviour:** placed like a door by clicking on a wall; they belong to Generic Models category (tab "Modify | Generic Models"); width/height edited in type properties (Duplicate then set Width, e.g. 4'0" x 7'0"); position by temp dim. [book]

### 4.8 Wall Opening (rectangular)
**Where:** Architecture > Opening panel > Wall. Works in plan, elevation or section; elevation/section preferred. [book]
**Sequence:** invoke, click the host wall, click first corner, drag, click opposite corner; temp dims adjust; Modify to exit. Properties afterwards: height, Top Offset, Base Offset. Works on straight or curved walls (rectangular only). [book]

### 4.9 Selection
**Where:** Modify > Select; Esc/MD to exit tools. [book]
- Hover highlights; Status Bar names it; Tab cycles overlapped candidates; click selects (blue, semi-transparent). Ctrl+click adds; Shift+click removes [knowledge: the book only says Ctrl adds]. [book]
- Window selection: drag left-to-right = solid rectangle, selects only elements completely inside; drag right-to-left = dashed rectangle, selects anything touched ("crossing"). [book]
- Tab on a joined wall = selects the whole chain. [book]
- Right-click > Select All Instances > Visible in View / In Entire Project; keyboard SA. [book]
- Selection Box (View panel of the contextual tab) isolates selected items in 3D view. [book]
- Status bar toggles: Select Links, Select Underlay Elements, Select Pinned Elements, Select Elements by Face, Drag Elements on Selection (keep it OFF to avoid accidental moves). [book]
- Restore selection: right-click > Select Previous or Ctrl+Left. [book]
- Filter: Multi-Select tab > Filter (or filter icon bottom-right with count): list of categories with counts, uncheck to remove; Check All / Check None. [book]
- Save/Load selection sets: Save and Load buttons on the Selection panel (Save Selection dialog with name). Edit Filters: New filter (Define rules or Select via Edit Selection Set or Use current selection), category check-list, rule set with AND/OR, Add Rule (Parameter/Operator/Value), Add Set. [book]
- Select by ID: Manage > Inquiry > Select by ID (comma-separated list, Show, OK) and IDs of Selection. Element IDs appear in error messages. [book]

### 4.10 Move
**Where:** Modify panel of any contextual tab; MV. Requires a selection first (or selecting after invoking for some tools). [book]
**Methods:**
1. Edit a temporary dimension (doors/windows/walls). [book]
2. Drag the element with the left button; walls move perpendicular to themselves unless Shift removes that constraint; Shift also locks direction for other elements; joined walls follow and re-clean. [book]
3. Drag a joint: right-click a member > Select Joined Elements; circular drag handles appear at shared ends; drag the common joint and every wall attached to it follows. Also works for beams/braces. [book]
4. Drag Elements on Selection toggle allows drag without pre-selecting. [book]
5. Move tool: click a start point, then end point or type distance + Enter. Options Bar buttons: Constrain (restrict to collinear/perpendicular vectors), Disjoin (break joins; also allows moving a door or window to another host wall), Multiple (only on Copy). Selection shown in dashed box. [book]
**Rules:** doors/windows moving along host wall: old opening closes, new opens. Pinned elements cannot move. [book]

### 4.11 Copy
Same interaction as Move plus: Multiple option (each additional click or typed distance makes another copy), Constrain. Start point can be an object snap like Midpoint; type distance after setting the first point. In tutorials Constrain on for single direction and Multiple for several copies. [book]

### 4.12 Trim/Extend (3 tools, Modify panel)
- **Trim/Extend to Corner**: click first element on the side to keep, then the second; both trimmed/extended to form a corner; a dashed preview shows. Works for walls, lines, beams, braces, reference lines. [book]
- **Trim/Extend Single Element**: click the boundary element, then the element to trim or extend; side kept = side where you click; no corner created; dashed preview. [book]
- **Trim/Extend Multiple Elements**: click boundary once, then click each element (on the part to keep) in succession, or window-select several. [book]
Notes: the book lists the tools under the Modify panel and works for walls and lines of the same type. [book]

### 4.13 Cut / Copy / Paste (Clipboard panel)
- Cut to Clipboard (Ctrl+X) removes and stores; Copy to Clipboard (Ctrl+C) keeps; Paste from Clipboard (Ctrl+V). Pasted items can go to the same or another view or project. A single door/window pastes by hovering a wall and clicking. Multi-element paste shows a preview box; in the same project a temp dim of offset from original appears and a distance and angle can be typed. After placing, the pasted set is selected as a temporary group (Modify | Model Groups) with Finish (Edit Pasted panel) or Cancel; Activate Dimensions in the Controls panel. [book]
- Paste variants (Aligned to Selected Levels, etc.) are used in Exercise 5 ("Aligned to Selected Levels" pastes onto Level 2). [book]

### 4.14 Rotate
**Where:** Modify panel; RO. [book]
**Sequence:** a rotation centre glyph appears at selection centre; drag it or press Place in the Rotate panel to relocate, Center to reset; click first ray (start direction) then move and click for the second ray, or type the angle after the first click; or tick Angle and type the angle in the Options Bar before clicking the centre. Buttons: Disjoin, Copy (rotated copy), Angle, Place, Center. [book]
**Limits:** walls cannot be rotated in elevation; hosted elements cannot rotate separately from host. In 3D rotation is about an axis perpendicular to the work plane. Rotation is only in plan, elevation or section about perpendicular axis. [book]

### 4.15 Mirror
**Where:** Modify panel; MM = Pick Axis, DM = Draw Axis. Copy toggle on by default in the Options Bar/Mirror panel. [book]
- Pick Axis: hover a line/wall/reference plane/grid (snap), click. Cannot mirror about empty space. [book]
- Draw Axis: click two points. [book]
- Hosted doors/windows are mirrored with their walls (self-test true). [book]

### 4.16 Offset
**Where:** Modify panel; OF. Options Bar: Copy checkbox (default on; off moves the original instead), radio Graphical vs Numerical (Offset field must be positive). [book]
- Graphical: click a start point, then a second point or type a distance; the element offsets by that displacement. [book]
- Numerical: type distance; hover an element: dashed preview flips sides with the cursor; Tab picks the whole chain of connected walls; click to commit. [book]
- Doors/windows in the walls are copied too. Works within the same work plane; not for in-place families. [book]

### 4.17 Array
**Where:** Modify panel; AR. [book]
Panel options: Linear / Radial; Group and Associate (copies are a group; the number stays editable, shown in blue and click-to-edit); Number; Move To: "2nd" (spacing between consecutive items) or "Last" (total span, items equally distributed); Constrain (orthogonal); Activate dimensions (Controls panel). [book]
Linear sequence: click start point (e.g. midpoint snap), move and click the spacing or type it, edit box for the Number (default is the panel value), Enter. [book]
Radial: Angle field; centre glyph can be dragged or placed with Place button; click start ray, move rotationally, type angle (e.g. 27) then Enter, then count. With "2nd" the angle is between items; with "Last" it is the total span. Result has controls: end arc handles to resize, middle control to change radius, centre to drag. [book]

### 4.18 Match Type Properties
Clipboard panel: pick source element (paint brush fills), then click targets of the same family category; wall instance parameters are not changed, only the type. Multiple targets in succession. [book]

### 4.19 Align and constraints
Modify panel; Align tool only in plan/elevation (not 3D). Options: Multiple Alignment checkbox, Prefer drop-down (book: centerlines or faces; [knowledge] stock Revit also offers finish/core faces). Sequence: click the reference line first, then each element to align; a padlock icon appears - click to lock the alignment (locked items move together). Locking chains: if A-B locked and then aligned to C, both follow. [book]
Dimension locks ("constraint") also exist via the temp-to-permanent dimension padlock [knowledge].

### 4.20 Delete
Delete key, DE, or Modify panel button. Permanent (not clipboard). Pinned elements cannot be deleted (warning to unpin). [book]

### 4.21 Split Element
**Where:** Modify panel. Options: Delete Inner Segment checkbox (Adjustments panel). [book]
- Plan: hover a wall, a temp dim shows; click split point; edit the temp dim to move the split exactly. Second click makes another split; with Delete Inner Segment on the part between two splits is removed; otherwise three independent walls. [book]
- Elevation/3D: splits a wall horizontally at a height; snaps to level lines; wall split at Level 2: upper portion Base Constraint = Level 2, lower Top Constraint = Level 1. Splitting in the middle and deleting the lower middle piece creates an opening. [book]

### 4.22 Split with Gap
Modify panel; Joint Gap field (between 1/16" and 1'); click a wall: becomes two walls separated by the gap. [book]

### 4.23 Split Face / Paint-like material regions
Geometry panel > Split Face: pick a face of wall/column/floor/roof, sketch a boundary on it (Draw panel tools), Finish Edit Mode; face splits into regions that can take different materials. [book]

### 4.24 Create Parts
Modify > Create panel > Create Parts; select layered element (wall, floor, roof, ceiling, structural slab foundation), Enter. Parts are separate elements (schedulable, taggable, editable) dependent on the parent; they regenerate when the parent changes; deleting the parent's parts deletes all of them; Properties: Show Shape Handles (resize), Material By Original (untick to override material), Divide Parts (sketch a profile on a face plane, Finish Edit Mode twice), Parts Visibility (Show Parts / Show Original). [book]

### 4.25 Group
**Where:** Architecture > Model > Model Group drop-down (Create Group, Place Model Group, Load as Group into Open Projects); Create panel in contextual tab; GP. Detail groups: Annotate > Detail > Detail Group. [book]
- Select then Create Group -> dialog Create Model Group (name; default "Group 1", increments). Or Create Group first (type Model or Detail) -> group editor with yellow background, Edit Group panel (Add, Remove, Attach Detail, Properties, Finish, Cancel). Elements created while in the editor join automatically. [book]
- Groups appear under Groups > Model / Detail in the Project Browser; instances placed by dragging from the browser or Place Model Group (Type Selector lists groups). Group origin and rotation controls reposition without moving content. [book]
- Edits propagate to every instance of the group. Types: model group, detail group, attached detail group (details tied to a model group; an Attached Detail Groups dialog toggles display on placement). Group Duplicate (browser right-click or Type Properties). [book]
- Swap groups via Type Selector on a selected instance. Exclude: Tab to an element, click the exclude icon or right-click > Exclude; hidden and not scheduled; restore with Restore All Excluded. Hosted elements are re-hosted to adjacent walls when excluding. [book]
- Ungroup, Edit Group, Link (convert to linked model: replace with new project file or an existing one). Save group: File > Save As > Library > Group (`.rvt`, family groups `.rfa`); Load as Group into Open Projects with Include options Attached Details, Levels, Grids. [book]
- Delete group: all instances first (Select All Instances > In Entire Project), then Delete from Project Browser. [book]
- Mixing incompatible items (e.g. tags in a detail group) triggers a warning. Self-test: nested groups ARE possible? The answer key marks "cannot create nested groups" with no result given in this slice; see Open questions. [book]

### 4.26 Create Similar
Selected element > Create panel > Create Similar (or right-click): launches the same creation tool with the same type pre-selected. [book]

### 4.27 Pin / Unpin
Modify panel, PN. Pin symbol shows only while the element is selected; click it to toggle. Pinned elements cannot move or be deleted. Example: pin columns to grid intersections. Unpin replaces Pin once pinned. [book]

### 4.28 Scale
Modify panel. Only for walls, lines, images, DWG/DXF imports, reference planes, dimension positions. Graphical (origin click, then first vector length, second vector length; ratio = second/first) and Numerical (type factor then click origin). [book]

### 4.29 Measure / Inquiry
- Measure Between Two References (Modify > Measure panel): click two points, shows distance and angle until next action; Chain option accumulates Total Length. Measure Along an Element: pick an element, shows length along axis. [book]
- IDs of Selection / Select by ID: Manage > Inquiry. [book]

### 4.30 Assemblies
Create Assembly (Create panel, multi-select): New Assembly dialog; Naming Category dropdown appears when categories are mixed, else type name edit box. Listed in Project Browser. Edit Assembly (origin + planes shown, add/remove, Finish), Disassemble, Create Views (scale incl. Custom, views to create, schedule check-list, sheet dropdown incl. Load titleblock). Result: plan/elevation detail views, Material Takeoff, Parts List, Wall Schedule, drag onto a sheet. [book]

---

## 5. Cross-cutting behaviours worth coding exactly

1. Temporary dimensions are the primary way to place and re-place everything. After a placement and Modify (Esc), select the element: dims appear to the nearest references; clicking the number opens an edit box; Enter commits and moves the element. [book]
2. Typed-length workflow: while a placement/sketch tool is waiting for a second point, simply typing digits opens an edit box and Enter commits. [book]
3. Dimension reference mode (centerline vs faces) is a project setting under Temporary Dimensions; individual dims can be switched by clicking witness-line grips. [book]
4. Constraint padlocks appear on aligned/locked items; locked items travel together. [book]
5. The Modify tool cancels any active tool; Esc twice is the standard exit. [book]
6. Deleting a host removes dependent hosted elements. [book]
7. Type vs Instance split is strict: sizes of doors/windows live in the type; positions, offsets, marks and levels live on the instance. Marks for doors are per instance auto-incremented; window marks are shared by type. [book]
8. Tags are annotation elements visible per view/category, and still exist when hidden. [book]
9. Opening cutting is automatic and parametric (door/window/opening family), plus manual rectangular Wall Opening. [book]
10. Rounded units only affect display. [book]

---

## 6. IFC4 mapping hints (mostly [knowledge])

| Revit thing | IFC4 target |
|---|---|
| Basic Wall | IfcWall (PredefinedType STANDARD/PARTITIONING/SHEAR etc.) |
| Wall layers | IfcMaterialLayerSetUsage with IfcMaterialLayer (thickness, function name, priority is not a native IFC field) |
| Wall location line | placement axis; LayerSetDirection / OffsetFromReferenceLine in IfcMaterialLayerSetUsage |
| Curtain wall | IfcCurtainWall |
| Door | IfcDoor + IfcDoorType (OperationType) |
| Window | IfcWindow + IfcWindowType (PartitioningType) |
| Opening (host cut) | IfcOpeningElement via IfcRelVoidsElement; fill via IfcRelFillsElement |
| Wall sweep/reveal | usually geometry in IfcWall or IfcBuildingElementProxy |
| Group | IfcGroup / IfcSystem (not standardized) |
| Level | IfcBuildingStorey (next slice) |
| Export to IFC / Export to IFC As / IFC Predefined Type / IFC GUID parameters | override flags per element and type |
| Door/window U, SHGC, VLT, FireRating | Pset_DoorCommon, Pset_WindowCommon (ThermalTransmittance, FireRating, etc.) |
The book itself only states that standard elements export without user action, mapping is available for non-standard families, and elements carry the IFC parameter set listed above. [book + knowledge]

---

## 7. Acceptance checklist (testable statements)

### Ch 1-2 (project and setup)
- (Ch 1) Creating a door or window with no wall present is impossible; there is nothing to hover.
- (Ch 1) A door follows its host wall when the wall is moved; a floor edge follows its wall edge.
- (Ch 1, 3) Deleting a wall deletes every door/window hosted on it.
- (Ch 2) New project from template yields Level 1 and Level 2 and starter views; a template is `.rte` and a project `.rvt`.
- (Ch 2) Choosing template "None" gives a project with default settings and no template content.
- (Ch 2) Project Units: Length format accepts rounding option, custom rounding increment only for non-feet/inch unit kinds, display changes but stored length does not.
- (Ch 2) Snaps dialog accepts semicolon-separated increments and cursor moves through them.
- (Ch 2) All enabled object snaps are active at once; SO toggles all snaps; a typed two-letter override works for one pick only.
- (Ch 2) Tab at an overlapping snap location cycles among candidate snaps.
- (Ch 2) Shift while sketching locks horizontal/vertical.
- (Ch 2) First Save of a new project opens Save As; later Saves do not prompt; Close with changes prompts Yes/No/Cancel.
- (Ch 2) Save backups: default 3 for non-workshared projects.
- (Ch 2) Opening an older-version file triggers a one-time upgrade message.
- (Ch 2) Two or more projects can be open and Copy/Paste works between them.
- (Ch 2) Zoom In Region takes two clicks; Zoom Out is 2x; Zoom to Fit fits all elements; Previous/Next Pan/Zoom restore views.
- (Ch 2) ViewCube has 26 hotspots (6 face, 12 edge, 8 corner); clicking a face gives an orthographic view.
- (Ch 2) Visual styles: Wireframe, Hidden Line, Shaded, Consistent Colors, Realistic/Textures, applied per view.

### Ch 3 (walls)
- (Ch 3, "Sketching Straight Wall Profiles") Click-click draws a wall; a temp dim with length and angle updates as the cursor moves.
- (Ch 3) Typing digits after the first click opens an edit box; Enter commits a wall of that exact length in the cursor direction.
- (Ch 3) With Chain on, the end of each wall is the start of the next; Esc ends the chain; a second Esc exits the tool.
- (Ch 3) With Chain off, each wall needs a new start click.
- (Ch 3) Chain can be toggled without leaving the tool.
- (Ch 3) A dashed alignment line plus a horizontal/vertical tooltip appears when the sketch is within tolerance of the axis.
- (Ch 3) Shift locks to orthogonal; an alignment-line intersection snap appears when aligned to another wall's endpoint.
- (Ch 3) Selecting a wall shows flip arrows (exterior side), two end drag dots, temp dim, and a convert-to-permanent-dimension control.
- (Ch 3) Clicking the flip arrows, or Spacebar when hot, flips the exterior side without moving the Location Line.
- (Ch 3) Exterior-side default: wall drawn left to right has exterior on top; top to bottom exterior on right; right to left exterior below; bottom to top exterior left.
- (Ch 3) Editing the length temp dim moves the free end only.
- (Ch 3) Dragging an end dot onto another wall's end snaps with Endpoint tooltip and the two walls join.
- (Ch 3) Location Line options are exactly: Wall Centerline, Core Centerline, Finish Face Exterior, Finish Face Interior, Core Face Exterior, Core Face Interior; the dashed location line is visible during sketching.
- (Ch 3) Changing wall thickness in the type leaves the location line in place.
- (Ch 3) Top Constraint "Up to level: Level 2" gives wall height = Level 2 elevation - base +/- offsets; Unconnected uses Unconnected Height (default 20'0").
- (Ch 3) Rectangle tool creates four joined walls; temp dims on parallel pairs can be edited.
- (Ch 3) Circle tool: typed radius measures to the location line.
- (Ch 3) Center-ends Arc: the first click is the centre, second click start/radius, third end; arc beyond 180 deg flips side.
- (Ch 3) Fillet Arc: clicking two walls trims them and inserts an arc whose radius is editable.
- (Ch 3) Polygon tools take a Sides count and radius; inscribed vs circumscribed differ in what the radius refers to.
- (Ch 3) Offset field in the Options Bar starts the wall at that distance from the location of the click/pick.
- (Ch 3) Slanted: Cross-Section=Slanted plus Angle From Vertical; Tapered requires variable-thickness type and Override Type Properties to enable Exterior/Interior Angle.
- (Ch 3) Structural Usage "Non-bearing" makes it architectural; wall instance shows computed Area and Volume.
- (Ch 3) Edit Assembly: layers listed exterior (top) to interior (bottom); total thickness = sum; Insert/Delete/Up/Down work; zero-thickness membrane allowed.
- (Ch 3) Join priority: higher priority layers connect first; a lower layer cannot pass through an equal/higher layer.
- (Ch 3) Default wrapping at inserts (Exterior/Interior/Both/Do not wrap) and at ends (Exterior/Interior/None) alter wall end/opening cap geometry in plan.
- (Ch 3) Stacked wall: one row Variable, others fixed height; instance acts as one wall.
- (Ch 3) Wall Sweep/Reveal can only be placed in 3D or elevation views; horizontal/vertical orientation selectable; sweep/reveal types are driven by Profile.
- (Ch 3) Place wall finish by Room puts finish walls on all interior faces enclosing the clicked room; by Segment only on the clicked face.
- (Ch 3) Interior walls drawn on Level 1 take Top Constraint one level above automatically.

### Ch 4 (doors and windows)
- (Ch 4) Door and window tools show a preview and temp dims (centre to neighbours) while hovering a wall; the side/swing follows the cursor's wall side.
- (Ch 4) Spacebar before click flips door swing/facing; after placement the twin arrows flip hand and facing independently.
- (Ch 4) Clicking a temp dim number and typing a value moves the door/window to that exact center offset.
- (Ch 4) Dragging a door along its host wall closes the old opening and cuts a new one.
- (Ch 4) Door Mark auto-increments; window Mark is shared across windows of one type; renaming a window tag mark asks for confirmation and updates the type.
- (Ch 4) Hiding Door/Window Tags in VG hides them but leaves them in the project.
- (Ch 4) Duplicating a type requires a new unique name; Rename does not change the dimensions.
- (Ch 4) Width/Height are type parameters (edit via Edit Type); Sill Height, Head Height, Level, Mark are instance parameters.
- (Ch 4) Door Orientation=Slanted aligns the door to a slanted wall; windows additionally have Slant along exterior/interior for tapered walls.
- (Ch 4) Passage Opening families are placed with Place a Component and behave as wall cutters; their width can be changed by a duplicated type.
- (Ch 4) A rectangular Wall Opening is made by clicking two corners in elevation/section/plan on a selected host wall and has Base/Top Offset.
- (Ch 4) Load Family opens a Specify Types dialog when the family has a type catalog; chosen types appear in the Type Selector.
- (Ch 4) Window Default Sill Height is 3'0".

### Ch 5 (modify)
- (Ch 5) Left-to-right window selects only fully enclosed elements; right-to-left selects touched elements.
- (Ch 5) Ctrl+click adds to selection; Tab+click selects a wall chain; SA selects all instances of the type.
- (Ch 5) Ctrl+Left restores the previous selection.
- (Ch 5) Filter dialog lists category counts and total; unchecking a category removes its members.
- (Ch 5) Move: dragging a wall perpendicular keeps joined walls connected; Shift lifts the perpendicular constraint.
- (Ch 5) Move tool: first click, then second click or typed distance and Enter; Disjoin allows moving a door to another host wall.
- (Ch 5) Copy with Multiple creates a copy per click or typed value until Esc.
- (Ch 5) Trim/Extend to Corner keeps the sides clicked and forms a corner; Single trims one element against a boundary; Multiple trims many against one boundary.
- (Ch 5) Cut removes and Copy keeps; Paste with Ctrl+V; a pasted multi-selection previews and can be positioned by typed distance/angle; Finish/Cancel commit/abort.
- (Ch 5) Rotate: center glyph can be relocated; angle by two rays or typed value; Copy creates rotated copy; hosted elements do not rotate independently from the wall.
- (Ch 5) Mirror Pick Axis needs an existing line to snap; Draw Axis takes two points; Copy is on by default.
- (Ch 5) Offset: Copy on by default; Numerical requires positive distance; Tab selects connected chain; hosted doors/windows are copied too.
- (Ch 5) Array Linear: Move To=2nd gives pitch; Last gives span; Radial uses angle; Group and Associate keeps the count editable.
- (Ch 5) Match Type Properties changes the target's type only, not its instance parameters.
- (Ch 5) Align: pick reference then target; padlock locks; locked pairs follow together; Multiple Alignment aligns many to one reference; works in plan/elevation only.
- (Ch 5) Delete removes permanently (nothing goes to the clipboard); pinned elements cannot be deleted.
- (Ch 5) Split Element in plan breaks a wall at the clicked point; with Delete Inner Segment the span between two clicks is removed.
- (Ch 5) Split Element in elevation at Level 2 sets upper base constraint to Level 2 and lower top constraint to Level 1.
- (Ch 5) Split with Gap accepts joint gap between 1/16" and 1'.
- (Ch 5) Pin prevents move; the pin glyph appears only when selected.
- (Ch 5) Scale: Numerical factor 2.5 turns a 10' wall into 25'; Graphical uses the ratio of two vectors from an origin.
- (Ch 5) Groups: editing one instance updates all; exclusion hides an element and removes it from schedules; groups can only be deleted after all instances are deleted.
- (Ch 5) Parts regenerate when the parent changes; deleting a part deletes all sibling parts.
- (Ch 5) Assemblies: Create Views produces plan/elevation detail views and schedules; Disassemble reverts.

---

## 8. Open questions / ambiguities in the book

1. Exterior-side rule for drawn direction: the text says L->R puts exterior on top, T->B on the right, R->L below, B->T left. This looks like "exterior on the left of the direction of travel for L->R" in one place and the opposite elsewhere; in stock Revit the exterior is on the left of the drawing direction [knowledge]. The self-test answer (Ch 3 Q10, answer F, "lower face is external for left to right") confirms top for L->R. Treat the book's four statements as authoritative only if consistent; verify against the real Revit rule.
2. Default unconnected height is stated as 20'0" in one table; actual Revit defaults come from the template (typically 8'0"-ish) [knowledge]; the book's value is probably the 2026 default.
3. Join Status "Allow" appears in tutorials but the dropdown's other values and semantics are never described.
4. Snap list says Work Plane Grid snaps to "a reference plane"; real behaviour is the active work plane's grid [knowledge].
5. Nested groups: the Ch 5 self-test statement "cannot create nested groups" has no verifiable answer in this slice (answers list T/F by position and appear misaligned). Revit does allow nesting [knowledge].
6. "Lock" is named in the Ch 5 objectives but only the Align padlock and Pin are described; a stand-alone Lock tool is not covered.
7. Whether the dimension snap list scales with zoom is not stated.
8. Chapter 5 says the Trim tools can handle "lines, beams, braces" but never mentions what happens to hosted doors/windows when a wall is trimmed.
9. Window placement face: "click on the upper face or lower face" determines inside/outside, but the exact rule (which side is exterior for windows relative to the wall's flip state) is not explicit; for walls the exterior is defined by the wall's flip state [knowledge].
10. Levels, grids, reference planes, dimensions and floors/roofs/ceilings are not covered in these chapters; do not infer their behaviour from this file.
11. Several "Modify | ..." contextual tab names are listed but panel-by-panel contents are not (e.g. Create Similar, Select Previous menu locations); locations stated above are only what the text says.
