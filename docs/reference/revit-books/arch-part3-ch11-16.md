# Revit behaviour reference, part 3: annotation, details, schedules, sheets, 3D views, rendering, advanced features

Source: "Exploring Autodesk Revit 2026 for Architecture" (CADCIM), extracted text `arch.txt`.
Slice covered: text lines 19842-30954 (chapters 11-16).

| Chapter | Title (as in book) | Line range |
|---|---|---|
| 11 | Applying Annotations, Tags, and Dimensions | 19842-21487 |
| 12 | Developing Construction Details and Generating Schedules | 21488-23558 |
| 13 | Composing and Printing Drawing Sheets | 23559-24510 |
| 14 | Creating and Managing 3D Views | 24511-25445 |
| 15 | Rendering and Creating Walkthroughs | 25446-27935 |
| 16 | Exploring Advanced Revit Features (structural, options, areas, phasing, links, worksets, solar, ...) | 27936-30954 |

Conventions
- `[book]` = stated in the text. `[knowledge]` = added from general Revit/IFC knowledge, not in the book.
- Everything is paraphrased. Citations are `(Ch N, "section heading")`.
- Imperial and metric tutorial variants exist in the book; only the semantics matter here.
- Ch 15 contains vendor-specific material (Enscape, Autodesk cloud render, mental ray). It is summarised briefly and flagged as out of scope for a Rust authoring core.
- Ch 16 structural content is shorter than the brief expects (walls, columns, beams, braces, openings; no isolated footings, slabs-on-grade, piles or rebar). See Open questions.

---------------------------------------------------------------------------

## 1. Tutorial spine (build order of each chapter's project)

Ch 11, tutorial 1 "Apartment 1" annotation (Ch 11, "TUTORIAL") [book]
1. Open the Ch 8 Apartment 1 project, open First Floor plan.
2. Room tool with Tag on Placement OFF: click inside each enclosed space (living room, kitchen, toilet, bedroom, dress, lobby); Esc after the last. Rooms show a shaded fill with a diagonal-cross extent marker while the tool is active.
3. Room Tag tool (Annotate > Tag): hover a room, click to place a tag per room; Esc; select a tag, click its text, type the name, Enter, Esc. Tags auto-number in placement order.
4. Tag by Category on furniture: first click on an item with no tag family loaded raises "No Tag Loaded"; answer Yes, load Furniture Tag from Annotations > Architectural; place; Esc twice; select the tag; swap type via Type Selector (boxed to standard); click "?" to enter text; confirm the "changing a type parameter" warning; Edit Type > Leader Arrowhead; drag tag with move control and leader elbow with its blue dot.
5. Aligned/Linear dimension: duplicate two dimension types (one for wall centerlines, one for wall faces); set Tick Mark and Witness Line Gap to Element in each.
6. Centerline dimensions: Prefer = Wall centerlines, Pick = Individual References; click two wall centerlines, then click again off the wall to place.
7. Pick = Entire Walls: click a wall, move off, click to place; set Prefer = Wall faces, open Options (Auto Dimension Options), tick Intersecting Walls, click wall, click to place.
8. Exit with Modify; Save As.

Ch 12 tutorials (Ch 12, "TUTORIALS") [book]
- T1 door-jamb detail: Callout (Rectangle) on plan > open callout view > hide Doors category in view > set View Name, View Scale, Display Model = Halftone, Detail Level = Fine > Detail Component (load wood frame / rough lumber sections, duplicate types and edit Width/Stop/Depth) > Detail Line with Thin Lines style tracing underlay > metal stud component (duplicate type, edit Width/Depth) > Insulation (Width, near-side offset, bulge ratio 3) > Text with two-segment leader > Break Line component (set Right/Left, Jag Depth, Jag Width, Masking depth) > set Display Model = Do not display.
- T2 schedules: Door schedule (Type Mark, Level, Width, Height, Count; sort by Type Mark; grand totals), Room schedule (Name, Number, Level, Area, Perimeter; sort by Number; totals on Area), Wall schedule (Family, Type, Length, Volume, Area; sort by Type; grand totals; Itemize every instance OFF; totals on Area/Length/Volume).
- T3 drafted road section: New Drafting View (name, scale) > Filled Region with line style and duplicated region types (fill pattern per type) > Offset + Array to repeat paver blocks > invisible-lines region for earth > aligned dimensions > two-segment-leader text > Break Line components.

Ch 13 tutorial (Ch 13, "TUTORIAL") [book]
1. Sheet tool > Load a titleblock family > OK. 2. Select titleblock; set Sheet Name and Sheet Number in Properties; click titleblock fields (Owner, Project Name) to edit project information in place. 3. Drag views from Project Browser onto the sheet (plan, elevation after renaming, section, detail). 4. Activate a viewport; in its own Visibility/Graphics turn off Grids and Sections annotation categories; resize section crop with drag handle; Deactivate View. 5. Drag schedules onto sheet; use the split control on the right edge to break a long schedule into side-by-side parts. 6. Save As.

Ch 14 tutorials (Ch 14, "TUTORIALS") [book]
- Interior camera from a floor plan: Camera tool, Perspective ON, click eye point inside a room, click target point across the room; crop region handles widen the field of view; rename 3D View 1 in the Project Browser.
- Exterior: eye point outside, target inside/beyond the building; separately Default 3D View, ViewCube Orient to a Direction > Northeast Isometric, orbit with SteeringWheels, rename {3D}.

Ch 15 tutorials (Ch 15, "TUTORIALS") [book]
- Interior render: Paint tool on a floor face in a section; edit wall/ceiling materials in Material Browser (replace appearance asset); load and place light fixtures (floor plan for floor-standing lamp with Offset, ceiling plan for pendant); place an RPC person with Rotate after placement; Rendering dialog: Quality = Best, Lighting scheme = Interior: Sun and Artificial, Sun setting Still with city/date/time, Adjust Exposure; Render; Save to Project. Then Walkthrough: path of clicks on a plan, Finish Walkthrough, Edit Walkthrough > Frame Settings > Total Frames; Export Walkthrough as AVI.
- Exterior render: create/duplicate material with new appearance asset; Paint exterior faces; Toposurface (points, Finish Surface) with material; Site Component for trees/people; render.

Ch 16 tutorials (Ch 16, "TUTORIALS") [book]
- T1 facade design options: Design Options dialog: New option set, New option (second), rename set and options, select primary option, Edit Selected, Close. In option 1: load Concrete-Square-Column, pick 18x18 type, click near a corner while temporary dimension reads 5 ft from a wall centerline, set Base Level/Top Level/Base Offset/Top Offset in 3D, Copy with Multiple to array columns; Beam tool with duplicated type (b, h parameters), Placement Plane = a level, sketch beams between columns (ignore warnings); copy to clipboard and Paste > Aligned to Selected Levels; drag beam ends in elevations; Floor and Railing sketch inside the option, answer No to the attach-walls prompt; Finish Editing. Option 2: Edit Selected on option 1, copy everything, switch Active Design Option to option 2, Paste > Aligned to Same Place, swap column and beam types via Type Selector, extend beams and slabs.
- T2 apartment cluster: Create Group for each project's elements, Copy to Clipboard, Paste from Clipboard (Duplicate Types dialog), Move/Rotate/Mirror (draw or pick axis), Ungroup, stretch and Split Element walls to join, Delete overlaps, Mirror design-option content separately by switching Active Design Option, Purge Unused, Save. Then in a site project: Transfer Project Standards (New Only), Link Revit with Manual - Base point positioning, place, Array (Radial) copies of the link, move links vertically.

---------------------------------------------------------------------------

## 2. Chapter 11: tags, rooms, keynotes, symbols, dimensions

### 2.1 Tag by Category
- Where: Annotate > Tag > Tag by Category (Ch 11, "Tagging Elements by Category") [book]. Contextual tab Modify | Tag.
- Preconditions: a tag family for the element's category must be loaded; otherwise a "No Tag Loaded" prompt offers to load one (Yes opens Load Family) [book]. Tags are view-specific annotations; visibility is per annotation category in Visibility/Graphics > Annotation Categories (Door Tags, Window Tags, Furniture Tags...) [book].
- Interaction: invoke tool; hover element (highlight); a tag preview follows the cursor; click to place; continue clicking other elements of the same or other categories; Esc to leave [book]. After placement select tag to expose move control, leader-elbow control and "?" text control [book].
- Options bar / tab fields [book]:
  - Orientation: Horizontal or Vertical (Properties).
  - Leader Line check box (default on); Leader Type: Attached End or Free End. Attached End enables a Leader Length edit box in the Placement panel; Free End lets the leader tip move off the host.
- Properties/type parameters [book]: instance Orientation, Leader Line, Leader Type, Angle (rotation). Type: Leader Arrowhead. Tag type is swapped in the Type Selector on a selected tag.
- Rules & behaviours [book]: entering label text on a tag family whose label is a type parameter raises a warning that a type parameter changes and may affect many elements; Yes proceeds. Door/window tags are preloaded in the default template and tag automatically on placement.
- Gotchas: typing a label through the "?" control edits the tag TYPE's label parameter, not an instance value [book]. [knowledge] The normal data-driven route is a label bound to a host parameter (for example Mark or Type Mark) so editing the host updates the tag.
- Outputs for BIM: annotation only; no IFC geometry. [knowledge] Annotations export as IfcAnnotation only for 2D drawings; tag text mirrors host properties.

### 2.2 Loaded Tags And Symbols
- Where: Annotate > Tag drop-down > Loaded Tags And Symbols [book].
- Dialog lists categories with their loaded tag/symbol families; "Filter list" restricts which disciplines/categories show; Load Family adds more (Library > English-Imperial > Annotations) [book].

### 2.3 Tag All Not Tagged
- Where: Annotate > Tag > Tag All [book].
- Dialog: radio "All objects in current view" (default) or "Only selected objects in current view" (enabled only if a selection existed before opening). Table of categories and their loaded tag types. Only ONE category row can be chosen per application [book]. Fields: Leader check box, Leader Length, Tag Orientation. Apply button runs it [book].
- Rules: also works for rooms and areas [book]. If target categories are hidden in the view a "Tag Visibility Enabled" message says visibility will be switched on while tagging [book]. If a category has no loaded tag it is absent from the list; load the family first [book].

### 2.4 Tread/Riser number
- Where: Annotate > Tag > Tread Number [book]. For component stairs only.
- Interaction: invoke tool; prompted to pick a stair reference line; hover along the stair; positions: Left, Right, Center, Left Quarter, Right Quarter; click the chosen reference line; treads or risers are numbered along the run [book]. View-specific (plan, elevation, section). Cannot annotate while the stair is in sketch mode [book].

### 2.5 Multiple leaders on a multi-category tag (new in 2026)
- Select the tag > Modify | Multi-Category Tags > Host panel > Add/Remove Host; then pick additional host objects; each gets a leader [book]. To show the count: Edit Family on the tag, select its label, Edit Label, pick category parameter "Multi Leader Tag" > Host Count, Add parameter, Apply, OK, Load into Project [book].

### 2.6 Room tool
- Where: Architecture > Room & Area > Room (contextual Modify | Place Room) [book].
- Panel: "Tag on Placement" toggle (default on). Off = places room without tag; tag later with Room Tag [book].
- Interaction [book]: hover inside an enclosed area (blue transparent preview with diagonal lines and extent box); click to create the room; keep clicking; Esc to exit.
- Rules [book]: rooms are model elements; a room is derived from the interior edges of bounding walls (and room separation lines). Rooms can be placed in unenclosed spaces and tagged, but area/perimeter (and similar computed values) are not produced for them in schedules. Moving a room away from its tag triggers an alert "room tag is outside of its room"; "Move to Room" repositions the tag, OK keeps it and adds a leader.
- Room boundary control [book]: wall instance parameter Room Bounding. Clearing it removes that wall from boundary computation (example: an intermediate wall with an arched opening ignored so the room extends to the outer walls). Retaining-type walls become non-room-bounding automatically (Ch 16).
- Outputs: [knowledge] IFC4 IfcSpace with Pset_SpaceCommon, Qto_SpaceBaseQuantities (NetFloorArea, GrossFloorArea, Height, NetVolume, GrossPerimeter). Rooms can bound ceilings/floors in the schedule via Level, Number, Name, Area, Perimeter [book fields].

### 2.7 Room Separator
- Where: Architecture > Room & Area > Room Separator [book]. Contextual tab Modify | Place Room Separation with the Draw panel (line sketch tools and Pick Lines).
- Visibility: Visibility/Graphics > Model Categories > Lines > Room Separation. Ticked = shown/printed; unticked = hidden but still honoured as boundaries [book].
- Use: split an open space into separate rooms without walls [book].

### 2.8 Room Tag
- Where: Annotate > Tag > Room Tag (also reachable from Architecture > Room & Area) [book]. Contextual Modify | Place Room Tag.
- Types in Type Selector: Room Tag, Room Tag With Area, Room Tag With Volume (default Room Tag) [book]. A boxed variant is used in the tutorial [book].
- Interaction: with the tool active, all rooms show a shaded fill; hover inside a room (boundaries highlighted); click to place; select a tag then click its text control to rename; Enter then Esc [book]. Options bar fields match Tag by Category (orientation, leader, leader length) [book].
- Rules: the tag reads room Name and Number; numbers auto-increment in placement order [book]. [knowledge] Name/Number are IfcSpace Name/LongName/number mapping targets.

### 2.9 Keynotes
- Concept [book]: keynote text lives in an external tab-separated text file (sample file Keynotes_Sample.txt, ASCII). Three keynote tag kinds: Element (reads the keynote parameter of the element type, not layers), Material (reads the material's keynote, e.g. a wall layer), User (always opens the picker dialog and overrides; does not update if type/material changes).
- Setup: Annotate > Tag > Keynote drop-down > Keynoting Settings: File location text box and Browse; on successful reload a "Reload Successful" message appears; View button shows the table read-only [book]. Settings apply to keynotes placed afterwards [book].
- Place: Element Keynote / Material Keynote / User Keynote tools. Type Selector offers four tag styles: Keynote Number, Keynote Number-Boxed-Large, Keynote Number-Boxed-Small, Keynote Text (metric names prefixed M_) [book]. Tag panel: Orient (horizontal/vertical); Placement panel: Leader toggle [book].
- Interaction [book]: click the element (arrowhead drops there), click the leader elbow, click final point = tag position. If the host already has a keynote value it shows immediately; otherwise the Keynotes dialog opens with a hierarchical Key Value list; pick one, OK.
- Assign without placing [book]: type parameter "Keynote" in Type Properties (browse button opens Keynotes dialog); or Edit Assembly > layer Material browse > Material Browser > Identity tab > Revit Annotation Information > Keynote (text or browse). Edit Assembly applies to wall types only.
- Keynote Legend [book]: View > Create > Legend/Schedules drop-down > Keynote Legend > name dialog > Keynote Legend Properties tabs (Fields, Filter, Sorting/Grouping, Formatting, Appearance). Two columns: Key Value, Keynote Text. Lists only keynotes actually used. Appears under Legends in Project Browser.

### 2.10 Symbol
- Where: Annotate > Symbol > Symbol [book]. Contextual Modify | Place Symbol.
- Interaction: pick symbol type in Type Selector (loaded symbols only); options bar Number of Leaders spinner; "Rotate after placement" check box starts Rotate right after click; click to place; repeat clicks place more instances (e.g. centerline symbol, north arrow, graphic scale) [book]. View-specific.

### 2.11 Temporary dimensions
- [book] Shown while drawing, selecting or placing; shown in blue; not view-specific; click the value to type a new one which moves/resizes the element; a conversion control converts the temporary dimension into a permanent one placed in the current view. When several elements are selected the temporary dimensions need to be activated explicitly (self-test mentions an Activate Dimensions button).
- Settings: Manage > Settings > Additional Settings > Annotations > Temporary Dimensions. Walls reference: Centerlines, Center of Core, Faces, Faces of Core. Doors and Windows reference: Centerlines, Openings [book].
- Snap increments exist for placement of components relative to the nearest perpendicular element [book].

### 2.12 Permanent dimension tools (Annotate > Dimension panel) [book]
Common: contextual tab Modify | Place Dimensions; type chosen in Type Selector; Place Dimensions panel has Prefer (Wall centerlines, Wall faces, Center of core, Faces of core) and Pick (Individual References, Entire Walls) and an Options button (Auto Dimension Options) that activates when Pick = Entire Walls.
- Aligned: dimension between two parallel/orthogonal references. Interaction: Prefer = snap target; Pick = Individual References; click ref 1, click ref 2, move away from the elements and click a third time to set the dimension line position [book]. Works on the centre of an arc wall too.
- Entire Walls option [book]: Options > Auto Dimension Options: check Openings and/or Intersecting Walls; choose Centers or Widths for openings. One click on a wall plus one click for placement creates a chain of segments (end walls, openings, intersecting walls).
- Linear: shortest distance between two picked points, click first point, click second, click to place; Esc or another tool to exit [book]. [knowledge] In real Revit Linear measures orthogonally to the view axes (horizontal or vertical), which differs from Aligned.
- Angular: dimension between two non-parallel elements; arc with ticks; pick the two elements then click to place [book].
- Radial: R prefix, click on arc/circle then place; centre mark is generated automatically [book].
- Diameter: diameter symbol prefix; click curve then drag along line to place [book].
- Arc Length: click the arc to set the radial point, pick start and end references, click to place [book].
- Spot Elevation / Spot Coordinate / Spot Slope: see 2.16.
- Alternate units [book]: duplicate a dimension type; Alternate Units = None/Right/Below; Alternate Units Format (e.g. metres with symbol), Alternate Prefix / Suffix (e.g. square brackets). Place with that type: primary and alternate values both display. Applies to permanent and spot dimensions.
- Baseline and Ordinate [book]: type parameter Dimension String Type = Continuous (default) / Baseline / Ordinate. Only for Linear and Aligned. Baseline: first click = origin (common witness line), subsequent clicks add stacked dimensions; Baseline Offset field in the Place Dimensions panel sets the stack spacing, adjustable before or after placement. Ordinate: first click = datum (0), each next click gives perpendicular distance from datum.
- Equality: dimension chain can show EQ; type parameter Equality Text customises the label; Equality Formula (dialog Dimension Equality Formula) can render a single label such as "3 x 5'" for 3 equal segments; works for aligned, linear, arc dimension types [book].

### 2.13 Dimension editing and type parameters
- Select dimension to get Modify | Dimensions; the dimension is "modifiable" when its referenced objects are selected [book].
- Witness lines: drag grips, or Edit Witness Lines ribbon button / context menu to reselect references [book].
- Instance: Leader check box [book].
- Type parameters named in the book [book]: Leader (Arc or Line), Leader Tick Mark, Show Leader When Text Moves, Tick Mark (many symbols incl. Dot Filled, Diagonal, Arrow Filled 30 Degree), Line Weight (1 to 16), Tick Mark Line Weight, Dimension Line Extension, Witness Line Gap to Element, Witness Line Extension, Text Size, Text Offset, Text Font, text style (bold/italic/underline), width factor, Equality Text, Equality Formula, Flipped Dimension Line Extension and Interior Tick Mark (both only enabled when Tick Mark = Arrow type), Dimension Prefix and Dimension Suffix (new in 2026), Alternate Units, Alternate Units Format, Alternate Prefix/Suffix, Dimension String Type.
- Rules [book]: dimension text value is computed and cannot be hand-typed, but prefix/suffix can be added; value updates when the element changes. Dimension size follows the view scale. When segments are too short for arrows, the arrows flip outside and interior diagonal ticks are used.
- Lock [book]: lock control appears on a placed dimension. Unlocked: the dimension follows element changes. Locked: the referenced distance is held; you can move the element together with the dimension but must unlock to change the value.
- Gotcha [book]: text height and similar sizes are derived from the view scale automatically; the tutorial says exact placement is unimportant but type parameters must be set on a duplicated type, not on the shared one.

### 2.14 Convert temporary to permanent
- Click the conversion symbol next to a temporary dimension to create a permanent dimension in the current view (Ch 11, "Converting Temporary Dimensions to Permanent Dimensions") [book].

### 2.15 Dimension type authoring flow (tutorial) [book]
Select Dimension tool > Type Selector pick base type > Edit Type > Duplicate > name > change Tick Mark and Witness Line Gap to Element > Apply > OK; then pick the new type from the Type Selector before placing.

### 2.16 Spot dimensions
- Where: Annotate > Dimension > Spot Elevation / Spot Coordinate / Spot Slope [book]. Valid in plan, elevation, 3D; can be placed on non-horizontal/non-planar surfaces (ramps, roads, topography, stairs) [book].
- Interaction [book]: choose symbol type; hover edge (highlight) and click the reference point; if the type has a leader, click the elbow then the final point; without a leader the symbol sits on the point. A temporary spot value previews under the cursor.
- Instance: Leader check box, text/leader settings, Prefix/Suffix [book]. Type: Symbol, Leader check box, Leader Line Weight, Leader Arrowhead, Text Size, Text Offset from Leader, Text Location, Text Font, Elevation Origin (default project origin; Relative lets you set another base), Rotate with Component [book].
- Rules: spot elevation can be taken at top or bottom of a thick element; spot coordinates show both coordinates and elevation [book].

---------------------------------------------------------------------------

## 3. Chapter 12: details, text, lines, revisions, schedules, legends

### 3.1 Callout
- Where: View > Create > Callout drop-down: Rectangle or Sketch (Ch 12, "Callout View") [book].
- Preconditions: parent view must be plan, section, elevation or detail view [book].
- Type Selector: "Detail View: Detail" or "Floor Plan: Floor Plan (for current view)" (the latter yields a view of the same type as the parent). Reference panel: "Reference Other View" check box with a drop-down of existing views, which makes a reference callout pointing at that view instead of creating a new one [book].
- Interaction, Rectangle [book]: drag from top-left to bottom-right, release. Sketch variant: Modify | Edit Profile with Draw panel (Line default), then Finish Edit Mode.
- Result [book]: new view listed under the SAME Project Browser heading as the parent (callout from a section appears under Sections). It is dependent on the parent and is deleted with it. Callout bubble drag handles resize the extent; rotation control; leader elbow control; Go To View on right-click.
- View properties [book]: View Name, Title on Sheet, Display Model (Normal / Halftone / Do not display), Detail Level (Coarse, Medium, Fine; "Fine" reveals layers of composite materials), Underlay Range: Base Level (None default), View Template (button opens Assign View Template dialog with Discipline filter, View type filter, Names list, Rename, Delete, Duplicate, Apply), Visibility/Graphics Overrides Edit, View Scale.
- Annotation vs model crop [book]: see 3.2.
- Gotcha [book]: callout tag symbols are linked to the view they create; detail number and sheet number show in the head once the view is placed on a sheet.

### 3.2 Crop regions
- Two crop regions per view [book]: model crop region (clips model elements) and annotation crop region (clips annotations; any annotation touched partly by its boundary is removed entirely). Annotation crop is on by default in callout/dependent views, off in the primary view. Toggle via View Control Bar Show Crop Region / Crop View and the Annotation Crop instance check box. Annotation hidden by model crop/hidden elements will not appear.

### 3.3 Detail Line
- Where: Annotate > Detail > Detail Line [book]. Contextual tab Modify | Place Detail Lines: Line Style panel drop-down, Draw panel.
- Line style can also be changed via the Line Style instance parameter [book].
- Rules: view-specific, visible only in the view drawn in. Trace over underlay/callout with snaps. Thick lines for masonry, thin for light materials [book]. Tools like Offset, Mirror, Copy work on detail lines [book tutorial].

### 3.4 Filled Region and Masking Region
- Where: Annotate > Detail > Region drop-down > Filled Region or Masking Region [book].
- Filled Region: sketch mode (Modify | Create Filled Region Boundary): Line Style panel, Draw panel, Mode panel (Finish/Cancel). Properties > Edit Type: Foreground Fill Pattern (browse opens Fill Patterns dialog with Pattern Type radio: Drafting vs Model) [book]. Drafting patterns scale to the sheet (fixed density at print scale); model patterns are fixed to model dimensions and scale with the model. On cut surfaces in plan/section only drafting patterns can be used [book]. Instance Area parameter shows the region area [book]. The boundary line style can be set to Invisible lines to show only the fill [book tutorial].
- Masking Region [book]: same sketch; produces an opaque region hiding what is behind it in the view. Can be added in a project, a detail family or a model family. In families, instance parameters include Visible, Draw in Foreground (selected: drawn at the detail plane of the view nearest the viewer; cleared: drawn on the work plane where it was sketched) and Visibility/Graphical Overrides (Family element visibility settings: detail levels). Must be a closed loop. After finishing, Edit Boundary on the Modify | Detail Items tab reopens the sketch.

### 3.5 Detail Component and Insulation and Break Line
- Detail Component: Annotate > Detail > Component drop-down > Detail Component (Modify | Place Detail Component) [book]. Load Family from Detail Items folders; Specify Types dialog lets you multi-select types with Ctrl. Type Selector picks the size; Type Properties > Duplicate to create new dimension-driven types (e.g. Width, Stop, Depth). Options bar "Rotate after placement" [book tutorial].
- Insulation: Annotate > Detail > Insulation (Modify | Place Insulation). Draw: Lines or Pick Lines. Options bar: Width, Chain, Offset with reference choice (to center / to near side / to far side). Click start, click end; Esc twice [book]. Instance: "Insulation Bulge to Width Ratio (1/x)" controls bulge (tutorial sets 3) [book].
- Break Line: a loaded detail component (Break Line.rfa, Div 01-General). Place, rotate to be perpendicular to the element it interrupts; its ends must cross the component edges for the break to take effect; grips extend it. Instance parameters in the tutorial: Right, Left, Jag Depth, Jag Width, Masking depth [book].
- Legend Component (Annotate > Detail > Component drop-down): options bar Family, View, Host Length [book].

### 3.6 Drafting View
- Where: View > Create > Drafting View; New Drafting View dialog: Name, Scale (Custom allowed) [book]. Appears under Drafting Views in the Project Browser. No model is shown; tools like Detail Line, Detail Component, Insulation, Filled Region, Text, Dimension, Break Line are used. Not linked to the model and does not update with it [book].

### 3.7 Line Styles, Line Weights, Line Patterns
- Line Styles (Manage > Settings > Additional Settings > Line Styles [book says Settings panel]): table of styles with line weight, colour, pattern; New under Modify Subcategories creates a style as a subcategory of Lines; Delete/Rename; Select All/None/Invert [book]. New style stored in that project only.
- Line Weights (Manage > Settings > Additional Settings > Line Weights) [book]: three tabs: Model Line Weights (16 weights x several view scales; Add/Delete scale), Perspective Line Weights (16 weights), Annotation Line Weights (16 weights, independent of scale).
- Line Patterns [book]: dialog listing patterns; Edit/New open Line Pattern Properties: Name and rows of Type (dash, dot, space) and Value. Pattern = series of dashes, dots, spaces.

### 3.8 Text note
- Where: Annotate > Text > Text (Modify | Place Text) [book].
- Panels: Alignment (Align Left default, Center, Right), Leader (No Leader default, One Segment, Two Segments, Curved) [book].
- Interaction [book]: no leader: click once to open the text box (Edit Text tab appears), type, then click outside or Close on the Edit Text panel; Modify to exit. One Segment: click leader tip, click text start. Two Segments: tip, elbow, text start (three clicks). Curved: start and end of arc (two clicks).
- Edit Text tab [book]: Bold, Italic, Underline, Subscript, Superscript, All Caps; list (bullets, numbers, letters), Increase/Decrease Indent.
- Selected text: stretch grips on the sides, rotation control at top right corner (leader tip stays; tail adapts), leader tip and elbow drag handles, move control. Modify | Text Notes tab: alignment, Add Right/Left Side Straight Leader, Add Left/Right Side Arc Leader (multiple leaders allowed), Remove Last Leader, Copy/Mirror/Rotate/Array, Copy to Clipboard / Paste from Clipboard (Aligned to selected levels) [book].
- Instance: Horizontal Align, Arc Leaders check box [book]. Type: Text Font, Text Size, Tab Size, Bold, Italic, Underline, width factor, colour, line weight, Leader Arrowhead [book].
- Rules: a text note can be moved without a leader following (the leader adjusts automatically) [book].

### 3.9 Model Text
- Where: Architecture > Model > Model Text [book]. Dialog Edit Text for content; preview attached to cursor; click to place on a work plane/level.
- Edit later: Modify | Generic Models > Text panel > Edit Text. Instance: Text, Horizontal Align, Depth (extrusion), Material. Type: Text Font, Text Size, Bold, Italic [book].
- Rules: 3D element visible in all views and rendered; when a view cuts it, it shows as cut unless the view range is adjusted [book]. Category Generic Models [book]. [knowledge] IFC export as IfcBuildingElementProxy with extruded text geometry.

### 3.10 Revision cloud and revision tag
- Where: Annotate > Detail > Revision Cloud [book]. Sketch mode Modify | Create Revision Cloud Sketch; any Draw tool; Spacebar flips cloud direction; Finish Edit Mode.
- Not valid in 3D views [book]. Hide: select cloud > right-click > Hide in view > Category; hiding does not affect revision tables/schedules; hides its tag too [book].
- Tag: Tag by Category on the cloud; revision tag family loaded from Annotations; Leader on by default in the options bar [book].
- Visibility rule [book]: Manage > Settings > Additional Settings > Sheet Issues/Revisions; column Show: None / Tag / Cloud and Tag (default). With Tag only, clouds are hidden but can still be hover-selected.

### 3.11 Schedule/Quantities
- Where: View > Create > Schedules drop-down > Schedule/Quantities (Ch 12, "Generating a Schedule") [book].
- New Schedule dialog [book]: category list (Filter List for disciplines), Name box defaults from category, radio "Schedule building components" (other schedule kinds exist in the Schedules drop-down: Material Takeoff, Sheet List, Note Block, View List etc.). Area schemes appear as categories too (Ch 16).
- Schedule Properties tabs [book]:
  - Fields: Available fields, Add/Remove parameter, Move up/down, New parameter (custom field), Add calculated parameter (e.g. percentage or formula), Combined Parameters, Include elements in links check box (linked-model elements).
  - Filter: up to four filter rules (parameter, condition, value).
  - Sorting/Grouping: Sort by (+ further levels), ascending/descending, Header/Footer/Blank line options, Grand totals with custom title, "Itemize every instance" (clear it to collapse identical rows into counted type rows).
  - Formatting: per-field heading text, heading orientation, alignment, Field Format (numeric format), Hidden field (field used for sort/filter but not displayed), Calculate totals / Show conditional format on sheets.
  - Appearance: Graphics (grid lines, outline, stripe rows with two colours, new in 2026), Text (title/header/body fonts and sizes) [book].
- Post-creation edits [book]: select schedule name in Project Browser; Properties has Edit buttons for Fields, Filter, Sorting/Grouping, Formatting, Appearance. Ctrl+wheel or Ctrl +/- zooms, Ctrl+0 resets. Freeze Header in the Titles and Header panel. Split & Place (Split panel) splits a schedule across sheets into parent/children in the Project Browser [book].
- Bidirectional rule [book]: editing a cell edits the element parameter for all instances of that type/instance and updates every view; model changes update schedules.
- Wall schedule fields noted [book]: Base Constraint, Base Offset, Top Constraint, Top Offset, Unconnected Height appear in Wall schedules and Material Takeoff.
- Cost example [book]: add a Cost field, a calculated field such as Cost*Area (unit-suppressed), and totals via Formatting > Calculate totals.
- Key schedules [book]: schedules grouping rooms by shared finishes (mentioned conceptually).
- Export [book]: File > Export > Reports > Schedule: Export Schedule dialog; options Export title, Export column header (and grouped headers), Export group headers/footers/blank lines, Field delimiter (tab, space, comma, semicolon), Text qualifier. Output .txt, openable in Excel.
- IFC/BIM [knowledge]: schedules are derived views of property sets/quantities; an open-source equivalent would be queries over IFC property sets (Qto_*BaseQuantities).

### 3.12 Legend
- Where: View > Create > Legend; New Legend View dialog: Name, Scale (Custom 1:n). Appears under Legends [book]. Content tools: Legend Component (Family, View, Host Length options), Text, Detail Line, Symbol [book].

---------------------------------------------------------------------------

## 4. Chapter 13: sheets, viewports, dependent views, printing

### 4.1 Sheet
- Where: View > Sheet Composition > Sheet [book]. Dialog "New Sheet": Select titleblocks list (with Load button to bring in titleblock families from Library > Titleblocks), Select placeholder sheets.
- Sheets are views bounded by a titleblock. Sheet number auto-increments for each new sheet [book]. Sheets are listed under "Sheets (all)" [book].
- Project Information (Manage > Settings > Project Information) feeds the titleblock: Author, Project Issue Date, Project Status, Client Name, project name, number, address [book]. Per-sheet values (Sheet Name, Sheet Number, Drawn By, ...) are instance parameters of the titleblock edited in Properties or directly by clicking the label in the sheet [book]. Values typed into project-wide fields propagate to all sheets [book].
- [knowledge] Recommended practice from the book: build views and sheets in the .rte template before modelling (Ch 13 intro) [book].

### 4.2 Place View / viewport
- Where: View > Sheet Composition > Place View, or drag a view from the Project Browser onto the sheet [book].
- Interaction [book]: Place View opens a Views dialog; select a view, "Add View to Sheet"; a rectangle attached to the cursor; click to drop. Viewport label (view name, scale, view number) appears bottom-left.
- Viewport properties [book]: View Scale, Detail Level (Coarse/Medium/Fine), Visibility/Graphics Overrides Edit, View Name, Rotation on Sheet (also in options bar), Edit Type. Viewport type parameters control visibility of title and extension line plus line weight, colour, pattern of the label line.
- A view can sit on one sheet only; use Duplicate as Dependent or Duplicate to reuse [book note on dependent views for placing on multiple sheets; [knowledge] one view/one sheet].
- Saved positions (2026) [book]: Modify | Viewports > Positioning & View panel: Save Position (name dialog), Edit Position, Manage Position; saved position applies to other views so they land at the same sheet spot.
- Replace view in viewport [book]: select viewport; Viewport Positioning drop-down (View Origin or Viewport Center) then View drop-down to swap view.
- Pan active view [book]: right-click viewport > Activate View; right-click > Pan Active View; drag; Deactivate View. 
- Activate View [book]: select viewport > Activate View (Viewport panel). All other items grey. Edits to model and Visibility/Graphics apply to the real view and propagate everywhere. Create new views while active if wanted. Deactivate via right-click or View > Sheet Composition > Viewports drop-down > Deactivate View.

### 4.3 Schedules on sheets
- Drag from Project Browser to sheet, click to place [book]. Blue triangles at column tops drag column width (applies to all parts); break line on right edge is the split control; each part has a move control and a dot to resize the part, last part cannot be resized, rows redistribute automatically. Parts can be rejoined by dragging one over its neighbour (only adjacent parts). Parts cannot be deleted individually or moved to another sheet separately. Right-click > Edit Schedule reaches the definition [book].

### 4.4 Guide Grid
- Where: View > Sheet Composition > Guide Grid; Assign Guide Grid dialog (Create new with Name, or pick existing); resizable with drag handles; instance Guide Spacing and Name; sheet instance parameter Guide Grid to reuse in other sheets [book].

### 4.5 Duplicate as Dependent
- Where: View > Create > Duplicate View > Duplicate as Dependent, or Project Browser right-click > Duplicate View > Duplicate as Dependent [book]. Valid on plan, elevation, section, callout views.
- Behaviour [book]: dependent view sits under its primary view; shares model and annotation settings; changes in the primary appear in dependents. Primary shows crop boundaries (Show Crop Region); resize each dependent's crop to cover its portion. Default name `<view>-Copy1`; rename in Project Browser. Navigation: dependent crop right-click > Go to Primary View; primary crop right-click > Go to View.

### 4.6 Matchline
- Where: View > Sheet Composition > Matchline [book]. Sketch mode Modify | Create Matchline Sketch; Finish Edit Mode. Edit via Edit Sketch on selection. Graphics via right-click > Override Graphics in View > By Element (weight, pattern, colour). Property Top Constraint can end the matchline at a chosen level (example: stop at level 30 in a 35-level building) [book].

### 4.7 View Reference
- Where: Annotate > Tag > View Reference [book]. Preconditions: dependent views placed on sheets. Interaction [book]: activate the primary viewport on the sheet; choose Target View in the View Reference panel; click in the view to place; Deactivate. Double-click the reference to open the target. The reference is not shown on the view it refers to.

### 4.8 Print
- Where: File > Print (Print dialog) [book]. Name (printer), Properties (driver options), Print Range: Current window, Visible portion of current window, Selected views/sheets (Select opens View/Sheet Set dialog with Save As / Rename / Delete sets, Check All / Check None, Show filters). Option to Print to File. Setup button opens Print Setup [book].
- Print Setup [book]: saved setting Name; Paper Size and Source; Orientation Portrait/Landscape; Paper Placement: Center, or Offset from corner with No Margin / Printer limit / User defined (x, y); Hidden Line Views: Vector Processing (smaller files, faster) vs Raster Processing; Zoom: Fit to page or percentage; Appearance: Raster quality, Colors (Black Lines, Grayscale, Color); Options: view links in blue, hide ref/work planes, hide scope boxes, hide crop boundaries, etc. Elements hidden temporarily (Temporary Hide/Isolate) STILL print [book].
- Preview button gives Print Preview with zoom [book].
- Publishing (Ch 13 intro and Ch 16): PDF/DWF export of views and sheets [book]; DWF set default zoom Fit to Page; rooms/areas can be published [book].

---------------------------------------------------------------------------

## 5. Chapter 14: 3D views, navigation, camera, section box

### 5.1 Default 3D View
- Where: View > Create > 3D View drop-down > Default 3D View [book]. Orthographic, first instance placed from the south-east; the view named {3D} appears under 3D Views only after first use [book].

### 5.2 ViewCube and SteeringWheels
- SteeringWheels toggle: F8 or Navigation Bar; Shift+W also toggles [book]. Nine tools: Center, Forward, Look, Orbit, Pan, Rewind, Up/Down, Walk, Zoom (availability varies by wheel type) [book].
- Center [book]: drag sphere cursor to a model point, release; view pans to centre it; right-click Center > Restore Original Center.
- Forward [book]: Tour Building wheel; pivot defined by wedge; Drag Distance Indicator slider with START and SURFACE; single click moves 50% of the distance to the pivot.
- Look [book]: rotate about a fixed point at view centre.
- Orbit [book]: pivot choices: via Center tool; Ctrl + Orbit then drag to a model point; select elements first (pivot at selection extent); default is target of current view.
- Pan [book]: hold and drag.
- Rewind [book]: thumbnail history of previous views, view-specific.
- Up/Down [book]: vertical slider with TOP and BOTTOM marks; bright orange current, dim orange previous.
- Walk [book]: perspective views only; drag from the blue circle; default movement constrained parallel to the ground plane (Options > SteeringWheels tab > clear "Move parallel to ground plane"); Speed Factor slider range 0.1-10 and period/comma keys accelerate/decelerate; Shift temporarily switches to Up/Down; context menu Increase/Decrease Walk Speed.
- Zoom [book]: click zooms in by 25 percent (needs "Zoom in one increment" setting for full wheel), Shift+click out 25 percent, Ctrl+click in 25 percent, drag for dynamic zoom.
- Orient (right-click ViewCube or wheel) [book]: Orient to View (a plan/elevation), Orient to a Direction (North, South, ..., Top, Northeast Isometric...), Orient to a Plane (Select Orientation Plane dialog). Default plan orientation has north at top; Manage > Project Location > Location can change it [book].
- ViewCube Home view can be saved from the current view [book self-test].

### 5.3 Camera
- Where: View > Create > 3D View drop-down > Camera [book]. Start from a plan (floor or ceiling) view.
- Options bar [book]: Perspective check box (clear for orthographic with a Scale drop-down), From (level of eye) and Offset (height above that level).
- Interaction [book]: click eye point; move away (three rays show field of view); set From/Offset for the target; click target point. Exterior: eye outside, target inside or beyond. Interior: both inside. If eye and target levels differ, four lines show the field. View is added as "3D View 1, 2..." under 3D Views; rename via Project Browser.
- After creation [book]: crop region with four blue grips adjusts field of view/extent; right-click zoom out 2x helps.
- View properties [book]: View Name, Detail Level, Visibility/Graphics Overrides, Eye Elevation and Target Elevation (default 5 ft 6 in above level), Section Box check box (also available in camera views).
- Perspective toggle on the default 3D view via the Projection Mode property [book].

### 5.4 Fly (2026)
- Navigation Bar Fly tool in a perspective view [book]: hold right button and drag to look; arrow keys or W/S/A/D move; E/Q (or Page Up/Down) up/down; Shift + wheel changes speed factor up to 50, shown in the status bar.

### 5.5 Lock / Unlock 3D view
- View Control Bar "Unlocked 3D View" flyout (only in 3D and camera views) [book]: Save Orientation and Lock View (names the view via dialog "Rename Default 3D View To Lock"; camera views skip the dialog), Restore Orientation and Lock View, Unlock View. While locked you can add annotations and tags; unlocking removes those annotations from view display (the book says the added tags disappear) and allows re-orienting; restore returns to the saved orientation.

### 5.6 Section Box and Selection Box
- Section Box [book]: 3D (and camera) view only; Properties > Section Box check box; drag blue arrow controls on faces; extent can also be adjusted from plan/elevation views; hide with Hide in view > Elements or clear the Properties check box; duplicate the view to keep a copy. Crop regions do not affect the box [book]. Purpose in the book: limit geometry for export.
- Selection Box [book]: with elements selected, ribbon View panel > Selection Box makes a section box fitted to the selection.

---------------------------------------------------------------------------

## 6. Chapter 15: materials, lighting, decals, rendering, walkthrough

### 6.1 Material Browser and Editor
- Where: Manage > Settings > Materials [book]. Dialog Material Browser with Project Materials list (context: rename, duplicate, add to library), view-options flyout (Document Materials: Show In Use; View Type; Sort; Thumbnail Size), Library panel (Favorites, AEC Materials; locked library items cannot be edited), toolbar: create/duplicate material, open user libraries, Asset Browser toggle, material editor toggle [book].
- Material Editor tabs [book]:
  - Identity: Name; Descriptive Information (Description, Comments, Keywords, Class); Product Information (Manufacturer, Model, Cost, URL); Revit Annotation Information (Keynote, Mark).
  - Graphics: Shading (Use Render Appearance, Color, Transparency 0-100), Surface Pattern (Foreground/Background pattern and colour, Texture Alignment), Cut Pattern (Foreground/Background pattern and colour).
  - Appearance: Information, Generic (Color, Image/texture, Image Fade, Glossiness, Highlights Metallic/Non-metallic), Reflectivity (Direct, Oblique), Transparency, Cutouts, Self Illumination, Bump, Tint. Replace asset from Asset Browser > Appearance Library.
  - Physical: Information (Name, Description, Keywords, Type read-only, Sub-class), Mechanical (Density, and in the book's structural intro Young's modulus, shear modulus, Poisson ratio).
  - Thermal: conductivity, specific heat, etc. [book]
- Assigning materials [book]: (a) Manage > Settings > Object Styles: per category/subcategory Material column (Model Objects, Imported Objects tabs); (b) Family Editor: Material instance parameter in Materials and Finishes group, Associate Family Parameter, new parameter as instance or type; (c) directly: instance parameter via browse button, type parameter in Type Properties, or structural layer in Edit Assembly; (d) Paint tool: Modify > Geometry > Paint, choose material, click face (face-level material without changing type); Remove Paint to undo. Textures are visible only in Realistic or Textures style [book].
- [knowledge] IFC4: IfcMaterial / IfcMaterialLayerSet (wall, floor), IfcSurfaceStyle for paint overrides; Pset_MaterialCommon for density.

### 6.2 Sun and lights
- Sun Settings: Manage > Settings > Additional Settings > Sun Settings, shortcut SU [book]. Areas: Solar Study (Still, Single Day, Multi-Day, Lighting), Presets (Duplicate, New), Settings (location/date/time or azimuth/altitude), Ground Plane at Level check box + level; unticking leaves shadows only on toposurface [book].
- Lighting fixtures [book]: Architecture > Build > Component > Place a Component; Load Family from Lighting > Architectural (Internal/External). Fixtures place only on appropriate plan: floor plan for floor/exterior mounted, ceiling plan for ceiling mounted. Type parameter Initial Intensity (dialog with Wattage, Luminous Flux, Luminous Intensity, Illuminance), lamp and initial colour; instance Level, Phase Created, Offset.
- Artificial Lights dialog (from Rendering dialog) [book]: tree of Grouped and Ungrouped Lights; per light/group Dimming (0 to 1) and On/Off; light groups: New Light Group; Edit shows a Light Group panel (Add, Remove, Finish; members green, other groups halftone); Move to Group / Remove from Group buttons.

### 6.3 Decals and entourage
- Decals [book]: Insert > Link > Decal drop-down > Decal Types (Create new decal, Name, Source image: jpeg, targa, tiff, bmp; linked externally, not embedded) and Place Decal. Attributes: Brightness, Reflectivity (0-99), Transparency (0 opaque to 1), Finish (Matte, Semi-gloss, Gloss, High Gloss, Custom 0-1), Luminance (cd/m2), Bump Pattern and Amount (0-100 percent), Cutouts image. Place: Type Selector, options bar Width/Height with Lock Proportions; hover face (flat or cylindrical only), click. Appears as rectangle with diagonals except in rendered views. Edit: size via options bar or corner grips; Reset restores original size; Decal Attributes Edit in Type Properties.
- Entourage [book]: families loaded from Entourage and Planting folders; show as simple placeholders in views and become photorealistic only in renders; RPC people cannot be placed in elevation/section views; plan/3D only; options bar Rotate after placement and level selector.

### 6.4 Rendering dialog and render
- Where: View > Presentation > Render, or View Control Bar Show Rendering Dialog, or shortcut RR [book].
- Settings [book]: View area methods: Render region (red rectangle with grips, Region check box), Crop region, Section box, Camera clipping plane. Output Settings: Screen or Printer with DPI 75/150/300/600. Quality setting (Draft, Low, ..., Best). Lighting: Scheme (six: Exterior Sun only; Exterior Sun and Artificial; Exterior Artificial only; Interior Sun only; Interior Sun and Artificial; Interior Artificial only), Sun Setting, Artificial Lights button (enabled only when the scheme includes artificial light). Background: Style (Sky/Color/Image), Haze slider (not for Color/Image). Image area: Adjust Exposure, Export, Save to Project; saved images appear under Views (all) > Renderings.
- Exposure Control [book]: Exposure Value (-6 to 16; higher = darker), Highlights 0-1 (default 0.25), Shadows 0.1-4 (default 0.2), Saturation 0-5 (default 1), White Point (6500 for daylight). Apply to preview.
- Engine in the book: mental ray, block-by-block progress dialog with Close-when-done option [book]. Flag: engine details are product-specific.
- Cloud rendering [book]: Autodesk Rendering service via View > Presentation > Cloud Rendering (Render in Cloud, Render Gallery); options Output Type still/panorama, Render Quality standard/final, Image Size, Exposure; project must be saved. Out of scope for a local Rust app.
- Enscape [book]: third-party real-time plugin (views, batch render, asset library, Veras AI, standalone EXE export, video editor with keyframes). Out of scope.

### 6.5 Walkthrough
- Create: View > Create > 3D View > Walkthrough [book]. Options bar: Perspective, Scale (orthographic), Offset and From (camera height relative to level, per key frame). Click points to define key frames of a spline; three lines show camera direction; Finish Walkthrough (Walkthrough panel). Can be started from plan, section, elevation or 3D views; path created first, edited afterwards [book]. Listed under Walkthroughs as "Walkthrough 1".
- Edit: Modify | Cameras > Edit Walkthrough; options bar Controls: Active Camera (drag camera along path; at key frames drag target and far clip, elsewhere only far clip), Path (key frames become draggable), Add Key Frame, Remove Key Frame; Frame edit box; Frame Settings opens Walkthrough Frames: Total Frames, Uniform Speed, Frames per second (default 15), table with Accelerator column (0.1-10, editable when Uniform Speed is off), Key Frame column, Indicators check box and Frame Increment [book]. Default frame count in the tutorial was 300 until changed [book]. Crop: grips or Size Crop (Crop Region Size dialog: Width/Height, Field of view vs Scale radio) [book]. Open Walkthrough and Play buttons; Visual Style from View Control Bar [book].
- Record: File > Export > Images and Animations > Walkthrough; Length/Format: All frames or Frame range Start/End, Frames/second, Visual Style (Rendering allowed), Dimensions or Zoom to; then Export Walkthrough (AVI) and Video Compression dialog [book]. A walkthrough lives in the project, not as a file, until recorded [book].

---------------------------------------------------------------------------

## 7. Chapter 16: structural elements (full detail)

### 7.1 Concept [book]
- Structural components categories: Structural Walls (foundation, retaining, shear), Structural Columns, Structural Beams and Braces, Structural Foundations (listed as supporting superstructure; the chapter text does not give a foundation placement workflow beyond foundation walls).
- Structural tab > Structure panel hosts Wall: Structural, Column, Beam, Brace. Structural elements can be added before or after the architectural model [book].
- Wall structure properties [book]: instance "Structural Usage" with values Non-bearing, Bearing, Shear, Structural combined; type "Function" with Interior, Exterior, Retaining, Foundation (also Soffit/Core-shaft exist in Revit [knowledge]). Example: Generic 8 in masonry defaults to Non-bearing; change to Bearing/Shear/Structural combined for load-bearing. Beams can join a wall only if its usage is Bearing or Structural combined [book].

### 7.2 Structural Wall, retaining wall, foundation wall
- Where: Structure > Structure > Wall drop-down > Wall: Structural (Modify | Place Structural Wall) [book].
- Retaining: select "Basic Wall: Retaining - 12 in Concrete" (metric 300 mm) in Type Selector; sketch in plan or 3D. Convert existing wall: Edit Type > Function = Retaining. Retaining walls are automatically non-room-bounding [book].
- Foundation: "Basic Wall: Foundation - 12 in Concrete" default. In plan it is created with the current level as TOP constraint, so it extends downward and a warning says it is invisible in the view. Fix by editing View Range (Primary Range: Bottom and View Depth set to Level Below). Structural Usage is set to Bearing automatically. Height is changed through Base Offset [book].
- Gotcha: walls hang downward, so a foundation wall is not visible until the view range reaches below the level [book].

### 7.3 Structural Column
- Where: Structure > Structure > Column (Modify | Place Structural Column); also under Architecture > Build > Column drop-down (Structural Column) [book].
- Preconditions: [book] set up structural grid first for grid placement; Load Family button in Mode panel brings in families from Structural Columns subfolders Concrete, Precast Concrete, Steel, Wood. Type chosen in Type Selector (example Concrete-Square-Column with square sizes).
- Single placement [book]: cursor snaps to a grid intersection; click to place. Spacebar toggles orientation relative to a highlighted (including inclined) grid (parallel vs perpendicular); press repeatedly to cycle. Options bar "Rotate after placement" starts rotation after the click. First drop-down: Height or Depth; second drop-down: level constraint, or Unconnected for explicit length (depth/height value).
- Multiple placement panel [book]:
  - At Grids: tab Modify | Place Structural Column > At Grid Intersection; select grid lines (all selected lines highlight, columns preview at crossings); Finish.
  - At Columns: select architectural columns (individually or several); structural columns snap to their centres; Finish on Modify | Place Structural Column > At Architectural Columns.
- Instance parameters used in the tutorial [book]: Base Level, Top Level, Base Offset, Top Offset (example: base at First Floor, top at Roof, top offset -150 mm / -6 in). Copy with the Multiple option to repeat along a direction. Temporary dimension while placing shows distance to nearest wall centerline.
- Rules [book]: columns are placed from top level downward by default (like foundation walls), so in a plan view they may be hidden until the View Range bottom clip is lowered. 
- [knowledge] IFC4: IfcColumn (PredefinedType COLUMN), profile IfcRectangleProfileDef/IfcIShapeProfileDef, Pset_ColumnCommon (LoadBearing, FireRating), Qto_ColumnBaseQuantities (Length, CrossSectionArea, GrossVolume).

### 7.4 Structural Beam
- Where: Structure > Structure > Beam (Modify | Place Beam) [book].
- Load Family from Structural Framing > Concrete / Steel / Wood families [book]. Beam type parameters (concrete rectangular family) include b and h; duplicate a type and set both to change the section (tutorial: 18 x 18 type from a 16 x 32 default) [book].
- Options bar [book]: Placement Plane drop-down (level on which the beam is drawn); Structural Usage drop-down (default Automatic: Revit infers from what the beam connects: girder, joist, purlin, etc. [knowledge for list]); Chain check box; 3D Snapping check box (snap to points in 3D); Multiple panel > On Grids to place beams along selected grid lines.
- Interaction [book]: click start point, click end point (with Chain, click successive points); Esc to exit. Beams drawn between columns may raise a warning (tutorial says ignore it) [book].
- Editing [book]: copy beams to clipboard; Paste > Aligned to Selected Levels (Select Levels dialog, Ctrl-multi-select levels) replicates them on other floors; drag the end grips in elevation views to extend.
- Rule [book]: beams join columns or bearing walls; wall usage must be bearing or combined to join.
- [knowledge] IFC4: IfcBeam, Pset_BeamCommon, Qto_BeamBaseQuantities (Length, CrossSectionArea, NetVolume).

### 7.5 Framing elevation and Brace
- Framing Elevation: View > Create > Elevation drop-down > Framing Elevation; pick a structural grid; creates an elevation view listed under Elevations (Interior Elevations) in the Project Browser; shows structural members as general-arrangement line work [book].
- Brace: Structure > Structure > Brace (Modify | Place Brace); Load Family from Structural Framing; in the framing elevation, snap to members (columns, beams) and sketch; displayed as lines in the elevation, visible as 3D in 3D views [book].
- [knowledge] IFC4: IfcMember with PredefinedType BRACE.

### 7.6 Openings in beams, braces, columns
- Where: Architecture > Opening panel > By Face [book].
- Interaction [book]: choose By Face; hover the member; two orthogonal planes (horizontal and vertical) of the member highlight green; click the plane; sketch mode Modify | Create Opening Boundary; draw closed profile with any Draw tool; Rectangle tool supports a Radius check box in the options bar with a radius edit box for a filleted rectangle; Finish Edit Mode. Example shapes: circular and filleted rectangular openings in beams for ducts and pipes.
- [knowledge] IFC4: IfcOpeningElement with IfcRelVoidsElement to the member.

### 7.7 Structural design option facade (tutorial) [book]
Option 1: 450 mm (18 in) square columns plus equal-section beams; Option 2: 300 mm (12 in) columns plus 300 x 600 (12 x 24 in) beams; each option also holds floor slabs and railings. Replacing the section is a Type Selector swap on the selected elements; beams and slabs are then stretched to the new columns.

---------------------------------------------------------------------------

## 8. Chapter 16: advanced features

### 8.1 Design Options
- Where: Manage > Design Options panel > Design Options (dialog). Panel also has Active Design Option drop-down and Pick to Edit [book].
- Dialog structure [book]: Option Set (New, Rename; Delete [knowledge]), Option (New, Rename; Accept Primary), Edit (Edit Selected, Finish Editing), "Now Editing" box. New set creates "Option 1 (primary)"; add more options with the Option New button.
- Workflow [book]: select an option > Edit Selected > Close dialog; everything modelled until Finish Editing belongs to that option; cut/paste existing items into the option is allowed. Finish Editing returns to Main Model (shown in Active Design Option drop-down).
- Presentation [book]: duplicate the view per option (Duplicate View > Duplicate), rename, set Active Design Option per view (view property) so each view shows its option; place views on a sheet. Visibility/Graphics controls option visibility per view. Area schemes can have options too.
- Copy between options [book]: copy to clipboard in Option A, switch Active Design Option to B, Paste > Aligned to Same Place.
- Gotchas (tutorial) [book]: mirroring option content requires switching to that option first and mirroring separately; ignore overlapping-warning dialogs for transient states.

### 8.2 Area analysis
- Where: Architecture > Room & Area. Tools: Area and Volume Computations (dialog with Area Schemes tab), Area Plan, Area Boundary, Area, Tag Area [book].
- Area Schemes [book]: default schemes Gross Building (not editable) and Rentable; add New, edit Description, Delete.
- Area Plan [book]: New Area Plan dialog: Type (scheme), Area Plan views list of levels (multi-select creates one plan per level, grouped by scheme under Area Plans), "Do not duplicate existing views". After OK, prompt: Yes = auto-create boundaries along exterior walls (Gross Building: exterior face; Rentable: interior face); No = sketch manually.
- Area Boundary [book]: sketch with Draw panel or Pick Lines; options bar "Apply Area Rules" (default on; wall boundary position follows area rules). When off: Offset and Lock become available.
- Area [book]: Modify | Place Area; Tag on Placement toggle; options bar Horizontal/Vertical/Model orientation, Leader, Area drop-down (existing or New); click inside boundary. Tag Area places a tag later. Area instance parameter Number auto-assigns; duplicate numbers produce a warning [book].
- Area schedules [book]: Schedule/Quantities; area schemes appear as categories.

### 8.3 Color schemes
- Where: Architecture > Room & Area > Color Schemes (dialog Edit Color Scheme), or the view property Color Scheme (button) [book].
- Dialog [book]: Category (Rooms/Areas), Schemes list with Duplicate; Title (legend title); Color parameter (list varies with category); radio By value (default) or By range (only for Area or Perimeter; Edit Format button with "Use project settings" check box and rounding); table columns: At Least, Less Than (read-only), Caption, Visible, Color, Fill Pattern, Preview, In Use; Move Rows Up/Down (by value only); Add Value dialog. Changing the Color parameter prompts to make a new scheme. Option "Include elements from linked files".
- Apply [book]: view property Color Scheme; Color Scheme Location = Background (floor only) or Foreground (all elements except columns).
- Legend: Annotate > Color Fill > Color Fill Legend (Modify | Place Color Fill Legend); choose scheme in Type Selector; click to place; drag circle control to split into columns; type property Show Title; annotation crop region can exclude it [book]. Modify scheme: select legend > Scheme > Edit Scheme [book].
- Linked rooms/areas colouring [book]: in Revit Links tab of Visibility/Graphics set Display Settings (custom, choose linked view, Color Fill = By host view).

### 8.4 Displace Elements
- Select elements in a 3D view > Modify | Multi-Select > View panel > Displace Elements [book]. Modify | Displacement set tab with xyz triad on-screen; drag arrows along X, Y, Z or type values in Properties X/Y/Z. Path tool (Displacement Sets panel) draws dashed paths from displaced elements to origin; Reset restores [book]. Typical use: explode roof/upper floors away to see inside.

### 8.5 Phasing
- Concepts [book]: every element has Phase Created and Phase Demolished instance parameters; each view has Phase and Phase Filter parameters. New project starts with phases Existing and New Construction.
- Phasing dialog: Manage > Phasing > Phases [book]: tabs Project Phases (rename; Insert Before/After a selected phase; Combine with Previous/Next; phases cannot be reordered afterwards), Phase Filters, Graphic Overrides (by status: New, Existing, Demolished, Temporary).
- Element phase statuses relative to a view's phase [book]: New (created in this phase), Existing (earlier, still present), Demolished (earlier, demolished this phase), Temporary (created and demolished this phase).
- Preloaded filters [book]: Show All, Show Complete, Show Demo + New, Show New, Show Previous + Demo, Show Previous + New, Show Previous Phase. New filters via New button.
- Use [book]: duplicate a view per phase, set its Phase and Phase Filter. Demolish tool (Manage > Phasing > Demolish) click elements; they display dashed in demolition-phase views; Phase Demolished property shows.
- [knowledge] IFC4 has no direct phase object; export Phase Created/Demolished as a property set or use IfcConstructionResource/IfcTask in 4D.

### 8.6 Linking and coordinates
- Link Revit: Insert > Link > Link Revit; dialog Import/Link RVT with preview and Positioning drop-down: Auto - Center to Center, Auto - Origin to Origin, Manual - Origin, Manual - Center, Manual - Base point; Place at level; Orient to View [book]. Linked models list under Revit Links in the Project Browser; right-click New Link / Manage Links; drag from browser into a view [book].
- Link CAD / Import CAD: Insert > Link/Import panels. Dialog options [book]: Current view only; Layers/Levels (All, Visible, Specify); Colors (Black and white, Preserve, Invert); Import units (Auto-Detect or specified); Positioning (Auto Center to Center, Origin to Origin, Manual Origin, Manual Center); Place at; Orient to View. Link keeps updating from the source; Import is a one-time copy. To see layers not chosen at link time delete and relink [book].
- Nested links [book]: Reference Type in Manage Links: Overlay (default: nested link not loaded when the host is linked elsewhere; a "Nested Links Invisible" message appears) vs Attachment (nested link follows). Change in the parent's file, save, reload. The example: A in B, B in C; A only appears in C when B treats A as Attachment.
- Bind Link [book]: select link > Modify | RVT Links > Bind Link; Bind Link Options (Attached Details, Levels, Grids); converts to a group; duplicate-name prompt, optional Remove Link.
- Visibility of links [book]: Visibility/Graphics > Revit Links tab: per link Visibility, Halftone, Underlay, Display Settings (RVT Link Display Settings dialog: Basics tab radio By host view / By linked view (choose Linked view) / Custom; Nested links drop-down By parent link / By linked view); plus Phases etc.
- Manage Links columns [book]: Linked File, Status (loaded or not), Reference Type, Positions Not Saved, Saved Path, Path Type (Relative recommended), Reload/Unload/Remove/Add.
- Coordinates [book]: Manage > Project Location > Coordinates > Acquire Coordinates and pick a link so the link's shared origin becomes the host's shared origin. True North: set view Orientation to True North; Manage > Project Location > Position > Rotate True North; options bar "Angle from Project to True North" [book].
- Schedules with links [book]: Schedule Properties > Include elements in links check box; project information fields from links selectable.
- Copy from link [book]: hover a linked element (Tab cycles nested), click, Copy to Clipboard, switch to host, Paste from Clipboard.
- Linked model is read-only in the host [book self-test: linked elements cannot be edited in the host].

### 8.7 Worksharing
- Concept [book]: single-user projects are not workshared. A central file stores ownership; each user works in a local copy; one user edits a workset at a time; element-level borrowing exists.
- Enable: Collaborate > Manage Collaboration > Worksets (dialog) [book]. Disable via expanded Workset panel Disable Worksharing.
- Worksets dialog columns [book]: Name, Editable, Owner, Borrowers, Opened, Visible in all views. Active workset drop-down; Gray Inactive Workset Graphics. Buttons: New, Delete (elements deleted or moved to another workset), Rename, Open/Close, Editable/Non Editable. Default worksets: Shared Levels and Grids, Workset1. Kinds: User-Created, View, Family, Project Standards (materials, line styles...). Show check boxes for Views, Families, Project Standards.
- Saving [book]: first save after enabling offers "Save File as Central"; local files via Save As on central; Save = local save; Synchronize with Central: Synchronize Now or Synchronize and Modify Settings (dialog: central path, relinquish options, comment); Reload Latest (shortcut RL).
- Ownership [book]: owned elements highlight blue; not owned elements show a blue icon with red highlight; trying to edit claims ownership if free; Make Elements Editable context menu; Borrowing: click icon, Place Request, Check Editability Grants dialog (Check Now, Continue); Worksharing Display options in View Control Bar (borrowed, owners, updates, checkout status).
- Workset visibility: closing a workset hides its elements; view workset ownership via Make Workset Editable [book].

### 8.8 Purge, Transfer, Browser Organization
- Purge Unused: Manage > Settings > Purge Unused; dialog lists categories of unused families, types, groups, annotation types, materials; Check All/None [book]; worksets must be open [book].
- Transfer Project Standards: Manage > Settings > Transfer Project Standards (run in the target); Copy from drop-down; item list (family types, materials, view templates, object styles...); duplicate-type prompt (New Only / Overwrite) [book].
- Browser Organization: View > Windows > User Interface > Browser Organization; Views tab (grouping/filter/sort parameters), Sheets tab (e.g. Sheet Prefix grouping) [book].

### 8.9 Shadows, sun, solar study
- Graphic Display Options (View > Graphics inclined arrow) [book]: Model Display (Style: Wireframe, Hidden Line, Shaded, Consistent Colors, Realistic; Show Edges; Transparency; Silhouettes), Shadows (Cast Shadows, Show Ambient Shadows), Lighting (Sun Setting, Sun, Ambient Light, Shadows sliders), Realistic Exposure (Automatic or Manual value; Color Correction: Highlights, Shadow Intensity, Color Saturation, White Point), Background (Sky with Ground Color, Gradient with Sky/Horizon/Ground, Image, None). Save as View Template button.
- Still, Single Day, Multi-Day modes in Sun Settings; location via Location and Site dialog (Define Location by Default City List; City, Latitude, Longitude); preview with Shadows On + Sun Path > Solar Study, Play Preview, frame stepping; export File > Export > Images and Animations > Solar Study (Frame range, Frames/sec, Visual Style, size) as AVI etc. [book].
- Rotate True North needed for correct sun angles [book].

### 8.10 Point Cloud, interoperability, publishing, route analysis
- Point Cloud: Insert > Link > Point Cloud; .rcp/.rcs index files linked by reference; raw .pcg/.las prompt indexing (Start Indexing). Visibility via Visibility/Graphics Point Cloud tab, colour modes via RGB button [book].
- DWG export: File > Export > CAD Formats > DWG; DWG Export dialog, Modify Export Setup (Layers tab: Export layer options, Load layers from standards) [book]; section box and detail level reduce size before export to 3ds Max or SketchUp. SKP link supported [book]. DWF publishing summarised [book].
- Route analysis: Analyze > Route Analysis > Path of Travel: Modify | Place Path of Travel; Line Style, Tag on Placement; click start and end in a plan; shortest path avoiding obstacles; single level; read-only instance params Speed, View Name, Level, Time, Length; editable Mark and Line Style; Route Analysis Settings dialog chooses which categories never act as obstacles; Update tool refreshes. People Flow Toolkit tools: Multiple Paths, One Way Indicator, People Content, Spatial Grids [book].

---------------------------------------------------------------------------

## 9. Acceptance checklist (testable statements)

Ch 11
- [11 Tag by Category] Placing a tag for an element whose category has no tag family loaded triggers a load prompt, not a silent failure.
- [11 Tag by Category] Tag with Leader Type Attached End keeps the leader end on the host; Free End allows moving it off; Leader Length applies only to Attached End.
- [11 Tag by Category] Editing the "?" label of a tag type warns that a type parameter will change and affects all instances of that tag type.
- [11 Tag All] Only one category can be tagged per Apply; "Only selected objects" is disabled unless a selection existed before opening.
- [11 Tag All] Tagging hidden categories enables their visibility first, with a confirmation.
- [11 Tread Number] Works only on component stairs, not in stair sketch mode; has five anchor positions.
- [11 Room] Room with Tag on Placement off creates no tag; tag can be added later; rooms in unenclosed regions can be tagged but get no area/perimeter.
- [11 Room Bounding] Clearing Room Bounding on a wall removes it from room computation and the room expands to the next boundaries.
- [11 Room Separator] Separation lines split rooms without walls; hiding the Room Separation subcategory hides lines but not the split.
- [11 Room Tag] Types Room Tag / With Area / With Volume exist; numbering increments sequentially.
- [11 Room Tag] Moving a room out from under its tag warns; Move to Room repositions, OK adds a leader.
- [11 Keynotes] Three keynote kinds (Element, Material, User) behave as described; User always opens the picker and does not follow later type/material changes.
- [11 Keynote Legend] Lists only used keynotes with columns Key Value and Keynote Text.
- [11 Symbol] Number of Leaders and Rotate after placement work; symbols are view-specific.
- [11 Temporary dimensions] Reference settings (Centerlines/Center of Core/Faces/Faces of Core; Centerlines/Openings) change what the temporary dimension measures.
- [11 Temporary dimensions] The conversion control turns a temporary dimension into a permanent view-specific one.
- [11 Aligned] Three clicks (ref, ref, placement) create the dimension; Prefer changes snap target.
- [11 Entire Walls] One wall click plus placement click yields chained segments including openings (centres or widths) and intersecting walls per Auto Dimension Options.
- [11 Radial/Diameter] Text prefix is R and the diameter symbol respectively; Arc Length needs a radial point plus two refs.
- [11 Baseline] Baseline Offset sets spacing of stacked dimensions; Ordinate measures from the first datum click; both only on Linear/Aligned.
- [11 Dimension text] The dimension value cannot be typed over, but prefix/suffix/alternate units can be.
- [11 Lock] A locked dimension prevents changing the referenced distance until unlocked, but the pair can be moved together.
- [11 Flip arrows] If a segment is too short, arrows flip outside and interior ticks are used (needs Tick Mark = Arrow type).
- [11 Spot] Spot Elevation/Coordinate/Slope work in plan, elevation and 3D; Elevation Origin may be project origin or relative.

Ch 12
- [12 Callout] A callout view lives under the same browser heading as its parent and is deleted when the parent is deleted.
- [12 Callout] Detail Level Fine reveals composite layer lines that Coarse hides.
- [12 Callout] Display Model Halftone / Do not display controls the model underlay in a detail view.
- [12 Crop] An annotation touched partly by the annotation crop boundary disappears entirely.
- [12 Detail Line] Detail lines exist only in the view where drawn.
- [12 Filled Region] Cut surfaces accept only drafting fill patterns; drafting patterns keep fixed density on paper while model patterns scale with the model.
- [12 Filled Region] Area instance parameter reports the region area.
- [12 Masking Region] Hides elements behind it; in families, Draw in Foreground changes the plane used.
- [12 Insulation] Width, Chain, offset reference (center/near/far) and bulge ratio parameter apply.
- [12 Break Line] Only breaks the element when both ends cross its edges.
- [12 Drafting View] Contains no model and does not update with the building model.
- [12 Line Weights] Model weights depend on view scale; annotation weights do not.
- [12 Text] Leader options (none, one segment, two segments, curved) require 0/1-2/2-3/2 clicks respectively; leaders can be added later in either side and straight or arc forms.
- [12 Model Text] Is a 3D Generic Model with depth, material, shows as cut in cutting views.
- [12 Revision Cloud] Not placeable in 3D views; Spacebar flips; hiding the cloud hides its tag; Show column of Sheet Issues/Revisions switches None/Tag/Cloud and Tag.
- [12 Schedule] Up to four filters; clearing Itemize every instance collapses rows; Hidden field lets a field sort/filter without displaying.
- [12 Schedule] Editing a schedule cell changes the element parameter in all views and vice versa.
- [12 Schedule] Grand totals can have a custom title; Calculate totals per field.
- [12 Schedule export] Delimited export with title/header/group-header toggles and a selectable delimiter.
- [12 Legend] A legend is a view with its own scale and components.

Ch 13
- [13 Sheet] New sheet numbers auto-increment; titleblock reads Project Information and sheet-level parameters.
- [13 Viewport] View Scale, Detail Level, V/G overrides, Rotation on Sheet are viewport-level, and changing them changes the view itself.
- [13 Viewport] Saved positions recorded by Save Position can be reapplied to other views.
- [13 Activate View] Edits in an activated viewport change the model and show in all views; Deactivate returns to the sheet.
- [13 Schedule on sheet] Split sections cannot be deleted individually or moved to another sheet; only adjacent parts rejoin; column width change applies to all parts.
- [13 Dependent view] Changes in the primary appear in dependents; each dependent has its own crop; Go to Primary / Go to View navigation works.
- [13 View Reference] The reference tag does not show in the view it points to.
- [13 Print] Print ranges: current window, visible portion, selected sheets/views; temporarily hidden elements still print.
- [13 Print Setup] Vector vs Raster hidden-line processing; Fit to page vs Zoom percent; Black Lines prints raster images in grayscale.

Ch 14
- [14 Default 3D] {3D} appears in the browser only after the tool is first used; view is orthographic from the south-east.
- [14 Camera] Perspective ON gives perspective; OFF gives an orthographic view with a Scale; default eye and target elevation 5 ft 6 in above their levels.
- [14 Camera] Eye and target levels may differ; cropping handles widen the field of view.
- [14 Walk] Only in perspective views; movement constrained parallel to ground by default; speed factor 0.1 to 10.
- [14 Fly] Speed factor max 50; WASD/QE keys.
- [14 Lock 3D] Only in 3D/camera views; lock allows annotations; unlock drops them; restore re-applies saved orientation.
- [14 Section Box] 3D views only; adjustable from plan/elevation; independent of crop regions; Selection Box fits to selection.

Ch 15
- [15 Material] Materials carry Identity, Graphics (shading, surface pattern, cut pattern), Appearance and Physical/Thermal assets; Use Render Appearance disables shading colour/transparency.
- [15 Paint] Paint assigns a material to a face without changing the type; Remove Paint reverts.
- [15 Sun] Ground Plane at Level off confines shadows to toposurface only.
- [15 Light] Ceiling-mounted fixtures can be placed only on ceiling plans; floor-mounted on floor plans.
- [15 Decal] Only flat or cylindrical faces; linked externally; types have the listed attributes.
- [15 Rendering] Six lighting schemes; Artificial Lights button enabled only for schemes including artificial light; Dimming range 0-1.
- [15 Exposure] Ranges exactly as listed (Exposure -6 to 16, Highlights 0-1, Shadows 0.1-4, Saturation 0-5).
- [15 Walkthrough] Key frames from click points; Total Frames re-distributes; Accelerator editable only when Uniform Speed is off; default 15 fps.

Ch 16
- [16 Wall] Retaining walls are non-room-bounding automatically; Foundation walls are Bearing automatically and invisible in plan until View Range reaches below.
- [16 Wall] Beams connect only to walls with Bearing or Structural combined usage.
- [16 Column] Spacebar cycles orientation relative to a highlighted grid; At Grids creates columns at all selected grid intersections; At Columns snaps to architectural column centres.
- [16 Column] Height vs Depth and level/Unconnected constraint in the options bar; Base/Top Level and Base/Top Offset parameters.
- [16 Column] Columns hang from the level downward by default and may vanish from the plan until View Range bottom changes.
- [16 Beam] Placement Plane, Structural Usage Automatic, Chain, 3D Snapping, On Grids behave as listed.
- [16 Beam] Paste > Aligned to Selected Levels replicates beams onto multiple chosen levels.
- [16 Brace] Placed in a framing elevation created from a grid; appears as lines there.
- [16 Opening] By Face works on beams, braces and columns; plane is picked from two orthogonal planes; Rectangle with Radius gives filleted rectangle.
- [16 Design Options] One primary per option set; edits go to the option being edited; Finish Editing returns to Main Model; each view has an Active Design Option.
- [16 Area] Gross Building scheme cannot be edited; Rentable boundaries sit on interior face of exterior walls; Gross Building on exterior face.
- [16 Color Scheme] By range exists only for Area and Perimeter; foreground colours all but columns, background colours floors only.
- [16 Phasing] New projects have two phases; phases cannot be reordered; seven preloaded filters.
- [16 Links] Overlay nested links do not display; Attachment makes them follow; Bind Link converts to a group.
- [16 Links] Acquire Coordinates adopts the link's shared origin for the host.
- [16 Worksets] Default worksets Shared Levels and Grids plus Workset1; only one user edits a workset at a time; borrowing uses requests.
- [16 Purge] Worksets must be open for Purge Unused.
- [16 Solar] Single day animates time; multi-day animates dates at fixed time; True North rotation is a prerequisite for correct orientation.
- [16 Route] Path of Travel is single level and avoids obstacles chosen in settings.

---------------------------------------------------------------------------

## 10. Open questions

1. Foundations: the chapter lists Structural Foundations as a category, but only foundation walls are described. Isolated footing, wall footing, foundation slab, pile, and rebar workflows are not covered; need another source (structural book).
2. Beam "Structural Usage" automatic mapping and the full list of usage values (girder, horizontal brace, joist, purlin, other) are not enumerated in this text [knowledge candidates].
3. Beam/column joins, cutbacks, justification and Z-offset parameters (Start/End Level Offset, y/z justification) are not described here.
4. Column Base/Top attachment ("Attach Top/Base"), "Moves with Grids", and Structural Material instance parameter are not covered.
5. Unclear whether column placement "from top to bottom" in the book applies to all families or only some; the tutorial explicitly sets Base Level and Top Level manually.
6. The text says Linear dimension measures the shortest distance, which contradicts usual Revit behaviour (orthogonal); needs verification against the Revit docs.
7. Whether Tag All "one category at a time" is a hard limit or UI selection behaviour is unclear (text vs actual product, which allows multi-select).
8. View-to-sheet cardinality (one sheet per view) is not stated; confirm.
9. Exact semantics of the book's statement that unlocking a 3D view removes tags (in Revit they are kept but hidden). Treat as view-state bound annotations.
10. Phase graphic override defaults and the phase filter table (which statuses each filter shows) are summarised only by name; the full matrix is not in this slice.
11. Worksets: exact behaviour for relinquishing and for the "Editable" column semantic needs a primary source; also how a Rust app should model central/local files (suggestion: out of scope for v1, use CRDT or file locks).
12. Walkthrough frame default (300) vs key-frame density rules are only inferred from the tutorial.
13. IFC4 mapping suggestions marked [knowledge] need review: rooms (IfcSpace), openings (IfcOpeningElement), bracing (IfcMember), phase information, and keynotes (IfcClassificationReference).
14. Several dialog/path names reflect Revit 2026 UI (Settings panel vs Additional Settings drop-down); the book is inconsistent about Line Styles location.
