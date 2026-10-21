# Revit MEP 2024 reference, part 1 (Chapters 1-3)

Source: "Exploring Autodesk Revit 2024 for MEP" (CADCIM, Tickoo). Extracted text file `books/mep.txt`, this slice = lines 711-7102.

| Chapter | Title | Text lines |
|---|---|---|
| 1 | Introduction to Autodesk Revit 2024 for MEP | 711-1614 |
| 2 | Getting Started with an MEP Project | 1615-3653 |
| 3 | Creating Building Envelopes | 3654-7101 |

Citation form: (Ch N, "section heading"). Tags: **[book]** = stated in the text; **[knowledge]** = added from general Revit/IFC knowledge, not in this slice.

IMPORTANT SCOPE NOTE. Despite the expected scope, this slice contains NO spaces, zones, mechanical settings, HVAC load calculation, copy/monitor of levels and grids as a worked workflow, or system classification. Those start in Chapter 4 (line 7102 on). What this slice does contain:
- the UI/data-model vocabulary (project, level, category, family, type, instance);
- project startup (templates, units, project information, location, browser organization, transfer standards);
- linking Revit models, shared coordinates, link visibility, bind link, Copy/Monitor (as a short overview only);
- snaps;
- the architectural "building envelope" tools an MEP user needs: levels, grids, reference planes, work planes, views, scope boxes, walls, doors, windows, openings, floors, ceilings, rooms, room volume.
It also contains a note on IFC export mapping (Ch 1) which is directly relevant for this project. Wherever an MEP-specific item is only mentioned, it is flagged. Chapter 3 treats the architectural model as something the MEP user may build in the same file; the "link an architectural model" path is described only in Ch 2 and the Ch 3 intro.

Book errata worth knowing when using it as a reference (do not copy these into the app): the Mass Density default is described inconsistently ("pound per cubic meter"); the Time unit list repeats "Hours"; the View Control Bar is called "lower right" in a quiz while the body says lower left; "Autodesk App Store in status bar" quiz item is false; the Options dialog description says "twelve tabs" and lists twelve names though some sections described are not in the listing; the Snaps dialog "three areas" are Dimension Snaps, Object Snaps, Temporary Overrides; the Location dialog is said to have "three tabs" but only Location and Site are named (the real dialog has Location, Weather, Site, [knowledge]); Imperial vs Metric default numbers are mixed in places.

---

## 1. Tutorial spine (ordered build sequence)

The book has three tutorials/exercises in this slice. They define the order in which a project is brought up. Reading them together gives the "from scratch" spine; the MEP-specific spine for linking an architectural model is reconstructed from the Ch 2 linking text plus Ch 1 workflow text.

### 1.1 General MEP model workflow (Ch 1, "Creating an MEP Model Using Parametric Elements") [book]
1. Pick a discipline template: Mechanical-Default, Electrical-Default, Plumbing-Default, or Systems-Default (all three disciplines). Metric variants carry a `_Metric` suffix.
2. Get levels: either copy the levels from the architectural model, or create levels manually.
3. Activate a plan view from the Project Browser, under Views (Discipline) > the discipline node.
4. Per discipline: analysis, then design, then documentation. For mechanical: analyse spaces, then place air terminals and equipment, then route ducts. (Spaces are Ch 4, not here.)
5. Documentation: views on sheets, schedules.

Dependency summary: template -> units -> project information/location -> levels (copied/monitored from link, or own) -> grids (same) -> views per level and per discipline -> architectural content (walls, floors, ceilings, rooms) either linked or built in-file -> only then MEP content (spaces, systems).

### 1.2 Chapter 2 project, "Office-Space" project setup (Ch 2, "Tutorial 1") [book]
Order the book uses (it is a pure setup project, no geometry):
1. Start Revit, File > New > Project.
2. Choose template by Browse: Systems-Default.rte (Imperial) or Systems-Default_Metric.rte (Metric). Radio "Project" (not "Project template").
3. Project Units (Manage > Settings > Project Units, or type `UN`): set HVAC, Electrical, Piping, Energy discipline entries (density, pressure, illuminance, apparent power, heat transfer coefficient) to named formats.
4. Project Information (Manage > Settings): fill fields.
5. Create a shared parameter file and parameter (Manage > Settings > Shared Parameters): new file, new group, new parameter with a discipline/type.
6. Add that shared parameter as a project parameter bound to the Project Information category (Manage > Project Parameters > New), instance, group under Energy Analysis; then give it a value in Project Information.
7. Project Location (Manage > Project Location > Location): address search via mapping service.
8. Browser Organization (View > Windows > User Interface > Browser Organization): new views organization grouped by Discipline then Family and Type, sorted by View Name ascending; tick it and Apply.
9. Save As > Project with Options: max backups, thumbnail source (3D view), regenerate thumbnail option.
10. Close.
Dependency note: the shared parameter is created before it can be selected for a project parameter; the project parameter must exist before a value can be entered in Project Information.

### 1.3 Chapter 3 Tutorial 1, "Office Building I" (envelope shell) [book]
Starts from Systems-Default (so Level 1, Level 2, mechanical/electrical/plumbing views already exist).
1. Because the template has only MEP-discipline views, duplicate the mechanical floor plans and elevations and set each copy's Discipline view property to Architectural so an Architectural node appears in the browser; rename the copies (Level 1, Level 2, East - Archi, etc.). (Ch 3, "Opening a New Project and Using the Template File".)
2. Open the architectural Level 1 plan, start Wall: Architectural. Duplicate a type (Generic - 8" -> Generic - 9" / 230 mm), edit its Structure so the single core layer has the new thickness.
3. Set instance values: Location Line = Wall Centerline; Top Constraint = Up to Level: Level 2.
4. Sketch the exterior with the Rectangle tool, then type exact lengths into the temporary dimensions; then additional exterior segments with Line and Chain.
5. Switch to an interior wall type (Generic - 5" / 90 mm), sketch interior walls using snaps to intersections and typed lengths; SHIFT locks orthogonal.
6. Rename levels in an elevation or the browser (Level 1 -> Ground Floor, Level 2 -> Roof) and accept the prompt to rename the associated views.
7. Add grids in the plan with Grid tool: Line for the first, Pick Lines with offset equal to half interior wall thickness for interior-wall grids; grids auto-number.
8. Save, close.

### 1.4 Chapter 3 Tutorial 2, "Office Building II" (openings, floor, ceiling) [book]
1. Reopen the Tutorial 1 file; hide Grids in the plan via Visibility/Graphics (Annotation Categories).
2. Door tool: the template has no door families, so the first use prompts to load; load from the library Doors folder, pick specific types in the Specify Types dialog. Place with Tag on Placement off; move to exact position via temporary dimension; flip swing with the flip control.
3. Window tool: same load prompt; place; set position through temporary dimension; to get a new size, duplicate the type and change Width.
4. Default 3D View to check.
5. Floor: Floor: Architectural, Extend into wall (to core) on, Pick Walls on the perimeter; use Split Element and delete segments where picked boundary lines cross an interior wall; Finish Edit Mode; floor type Generic 12" / 300 mm.
6. Ceilings need a ceiling plan view: duplicate a mechanical ceiling plan to the Architectural discipline, rename, open it; Ceiling tool with automatic ceiling; set Height Offset From Level to 8'6" / 2590 mm; click once inside each enclosed region; check in 3D view (3D view must have the Architecture or Coordination discipline or it shows halftone).
7. Save.

### 1.5 Dependency graph (what an MEP user must have from architecture) [book + knowledge]
- Levels with correct elevations and names (everything level-hosted: ducts, pipes, equipment, spaces, floors, ceilings) [book: Ch 3, "Levels"].
- Grids (reference for placing equipment; copy/monitor candidates) [book].
- Walls with Room Bounding on, floors, ceilings, roofs, curtain systems, columns, room separation lines [book: Ch 3, "Room Bounding Elements"]. Used to compute room area/volume and, in Ch 4, spaces.
- Doors/windows are wall-hosted and cut openings [book]; their rough/opening sizes feed schedules [book] and heat load [knowledge: used for envelope load].
- Rooms from architecture (name, number) are the basis for creating spaces [book: Ch 3, "Creating Rooms": "Revit forms the basis for creating spaces"].
- Shared coordinates between host and link [book: Ch 2].
- Thermal properties of wall/roof/floor/ceiling type (Thermal Mass, Thermal Resistance, Heat Transfer Coefficient read-only; Absorptance and Roughness editable) [book: Ch 3, "Specifying Type Properties"], needed by load calcs.
- Building location (latitude/longitude, weather) [book: Ch 2, "Setting the Project Location"].
- Project computation height of levels and the Areas and Volumes computation switch [book: Ch 3].

---

## 2. Data model vocabulary (Ch 1)

### 2.1 Core concepts [book]
(Ch 1, "Terms Used in Autodesk Revit for MEP")
- Project: one file holding model, views, sheets, schedules, settings; extension `.rvt`. Template `.rte`, family `.rfa`.
- Level: infinite horizontal plane hosting level-based elements. Every created element belongs to a level.
- Category -> Family -> Family Type -> Instance. A type change updates all instances of that type; an instance change affects only that instance. Example given: Air Terminal category, "Rectangular Diffuser - Round Connection" family, a sized type, then instances.
- Element classes: host, component, annotation, view, datum. Datum = grids, levels, reference planes.
- Categories groups: model, annotation, and (when present) imported, workset, filter, Revit-link categories.
- Parametric change engine: edit in one view, all views update; "a change anywhere is a change everywhere". Associativity is bidirectional.
- System families (walls, floors, ceilings, levels, grids, ducts, pipes) live inside the project/template; loadable families (doors, windows, MEP components) are `.rfa` loaded on demand.
- Scale -> detail level: choosing a view scale auto-sets Coarse/Medium/Fine; detail level can be overridden per view.
- Large projects: link multiple Revit files; teams use worksets.

### 2.2 Disciplines [book]
Mechanical (HVAC, ducting, fire suppression), Electrical (lighting, devices, panels, circuits, load analysis), Plumbing (fixtures, piping, firefighting). A view has a Discipline property (Architectural, Structural, Mechanical, Electrical, Plumbing, Coordination); the browser groups by it. Default view discipline in Options is Coordination [book: Ch 2, "General Tab"].

### 2.3 Analytical vs physical [book]
(Ch 1, "Building Information Modeling and Autodesk Revit 2024") The analytical and physical representations of an MEP model are created at the same time. Data is passed to third-party analysis through the API and results can come back.

### 2.4 Export to IFC mapping (Ch 1, "Export IFC Element Mapping") [book]
- Every element has an **Export to IFC As** instance parameter under "IFC Parameters" in Properties.
- Clicking its browse button opens the Select Export As IFC Entity dialog.
- Dialog: IFC Schema Version drop-down (default IFC4); a search box; left list = IFC entity (e.g. IfcPipeSegment); right list = predefined type (e.g. RIGIDSEGMENT).
- The book's example: type "pipe", choose IfcPipeSegment, choose predefined type RIGIDSEGMENT, OK; the chosen values then appear as IFC parameters in the element's instance properties.
- Door type properties also show an "IFC Parameters" group (Ch 3, "Understanding Door Properties").
- [knowledge] The mapping is per instance and per type; unset means the category default mapping from the exporter mapping file; IFC export also carries the Pset_*Common property sets.
IFC4 implication for this app: store an optional per-element `ifc_class` and `ifc_predefined_type` override that defaults from category.

### 2.5 Interoperability list [book]
(Ch 1) DWG/DXF/DGN, DWF, IFC, SKP, SAT, plus image, ODBC, HTML, TXT, XML, XLS, MDB. Link or import of ACIS solids from AutoCAD MEP. Not essential for MVP.

---

## 3. Tool and command reference

Template per tool. "Pre" = preconditions. "Outputs" mixes [book] and [knowledge] tags.

### 3.1 New Project (File > New > Project)

- Where: File menu > New > Project. Also "New" on the start screen, MODELS section. [book: Ch 1, "Starting Autodesk Revit 2024"; Ch 2, "Starting a New MEP Project"]
- Pre: none.
- Sequence: choose File > New > Project; dialog "New Project". Template file area: drop-down with Construction Template (default), Architectural Template, Structural Template, Mechanical Template, others; Browse opens Choose Template (file list; select; Open) and the choice appears in the drop-down. "Create new" radio: Project vs Project template. OK.
- Options: radio Project / Project template; template drop-down; Browse.
- Parameters inherited from template: units, view definitions and scales, families, line styles, line weights, fill patterns, materials, view templates, visibility/graphics, print settings, geometry included in the template. [book: Ch 2, "Project Templates"]
- Rules: new project adopts every setting and family from the template. A template with geometry (e.g. a hospital core) will put that geometry into every new project. [book]
- Template choices for MEP: Mechanical-Default.rte, Electrical-Default.rte, Plumbing-Default.rte, Systems-Default.rte, with `_Metric` names for metric. [book]
- Template "None": produces an "Undefined System of Measurement" box asking Imperial or Metric; the chosen option loads default units for that system, and the file is then saved as a template via Save As > Template (`.rte`). [book]
- Gotchas: confusion between `.rte` (template) and `.rvt`; a quiz in the book wrongly states `.rft`. [book]
- Outputs for BIM: nothing IFC-specific; app equivalent = project template format (store units, level set, view set, types).

### 3.2 Project Units (Manage > Settings > Project Units; type `UN`)

- Where: Manage tab > Settings panel > Project Units. Shortcut `UN`. [book: Ch 2, "Project Units"; Tutorial 1]
- Pre: project open.
- Sequence: open dialog; choose Discipline from drop-down; click a cell in the Format column for a unit row; "Format" dialog opens; pick Units, Rounding, optional custom Rounding increment, Unit symbol, and flags such as Suppress spaces; OK; OK. Decimal symbol/digit grouping drop-down sits at bottom-left of the dialog.
- Disciplines in the dialog (seven): Common, Structural, HVAC, Electrical, Piping, Infrastructure, Energy. [book]
- Format dialog fields: Units, Rounding (including Custom, which activates Rounding increment, except for Feet and fractional inches, Fractional inches, Meters and centimeters), Unit symbol (inactive for Feet and fractional inches and for Meters and centimeters), Suppress spaces (feet/inches strings). [book]
- Unit types and their selectable values mentioned: [book]
  - Length: decimal feet, feet and fractional inches, decimal inches, fractional inches, meters, decimeters, centimeters, millimeters, meters and centimeters.
  - Area: square feet, square meters, acres, etc. Volume: cubic yards, cubic feet, cubic meters, liters.
  - Angle (units list), Slope: ratio:12, ratio:10, rise/12", rise/1'-0", rise/1000mm, decimal degrees, percentage (Imperial default rise/12").
  - Currency (symbol, rounding). Mass density: kg/m3, lb/ft3. Rotation angle: degrees, gradians, multiples of n, radians (default degrees). Cost per area: per sq ft, per sq m. Speed: mph, ft/min, ft/s, km/h, m/s, cm/min. Time: seconds default, plus ms, minutes, hours.
  - HVAC: Power (W, kW, BTU/s, BTU/h; default BTU/h imperial, W metric); Pressure (in. water at 60F, Pa, kPa, MPa, bar; default in. water imperial, Pa metric); Air Flow (CFM, L/s, m3/s; default CFM imperial, L/s metric); Heating Load (W, kW, BTU/s, BTU/h; default BTU/h imperial, W metric). Also Density, Velocity, Duct Size, Heat Gain, Factor (named in Tutorial and Exercise).
  - Electrical: Illuminance (footcandles / lux; defaults fc imperial, lux metric); Electrical Potential (V, kV, mV; default V); Demand Factor (percentage or fixed; default percentage); Current, Frequency, Apparent Power.
  - Piping: Flow (US gal/min, US gal/h, m3/h, L/s; default GPM imperial, L/s metric); Velocity (ft/s or m/s); Pipe Size (fractional inches, decimal inches, decimal feet; default fractional inches imperial, millimeters metric); Density.
  - Energy: Coefficient of Heat Transfer (BTU/h*ft2*F or W/m2*K or other), set in the tutorial.
- Rules: units only change display and input parsing, not the stored value (a wall rounded to the nearest 1' may still be a fractional length). [book: Ch 2 note] During install the user picks Imperial/Metric as the default system. [book]
- Gotchas: stored values must stay in an internal unit; format is presentation only. [book + knowledge: Revit stores feet, internally]
- Outputs: IFC4 uses SI (IfcSIUnit). [knowledge] Project unit system must map to IfcUnitAssignment on export.

### 3.3 Project Information (Manage > Settings > Project Information)

- Where: Manage > Settings > Project Information. [book: Ch 2, "Setting the Project Information"]
- Sequence: open dialog (a parameter table with Value column); click value cells to edit; for Project Address a browse button opens an "Edit Text" dialog; OK.
- Fields named: Organization Name, Organization Description, Building Name, Author, Project Issue Date, Project Status, Project Address, Project Name, Project Number, Client Name. (Exercise adds Commencement Date, Documented Contract Completion Date, Provisional Period; these are custom parameters.) [book]
- Extending: Project Parameters (Manage > Settings > Project Parameters > New Parameter): Parameter Type radio Project parameter or Shared parameter (Select button); Categories area (Filter list discipline e.g. Architecture; tick Project Information); Parameter Data: Name, Discipline, Type of Parameter, Group parameter under, Instance vs Type, "Values are aligned per group type" vs "Values can vary by group instance". OK twice. The new parameter appears in the Project Information dialog. [book]
- Shared parameters: Manage > Settings > Shared Parameters opens "Edit Shared Parameters"; Create (file dialog, new .txt file), Groups: New (Name), Parameters: New (Name, Discipline, Type of Parameter). Shared parameter file is external and referenced. [book]
- Rules: project information is itself a model category ("Project Information") that parameters can bind to; schedules can include project information from linked files (see 3.9). [book]
- Gotchas: shared parameters require a file path; project-local ones do not. [book + knowledge]
- Outputs: IfcProject / IfcSite / IfcBuilding name, long name, description fields. [knowledge] Custom parameters map to property sets.

### 3.4 Location / Site (Manage > Project Location > Location)

- Where: Manage > Project Location panel > Location. [book: Ch 2, "Setting the Project Location"]
- Sequence: dialog "Location and Site"; tab Location: Define Location by drop-down (Internet Mapping Service default, or Default City List); with mapping: type address in Project Address edit box, press Search, the map shows the result; with city list: pick City, latitude/longitude fill, or type custom latitude/longitude. Tab Site: list "Sites defined in this project" with Internal (current) default; Duplicate (Name dialog), Delete, Make Current (current location is suffixed "(current)"). OK.
- Purpose: geographic location used for solar/weather in energy analysis. The text also says it defines the "coordinate system of the host and linked model (if present)". [book]
- Gotchas: needs internet for mapping; the book also contradictorily says Google/Bing. Weather tab exists in the real UI but is not discussed. [book]
- Outputs: IfcSite RefLatitude / RefLongitude / RefElevation; IfcSite.Address from Project Address. [knowledge]

### 3.5 Transfer Project Standards (Manage > Settings)

- Where: Manage > Settings > Transfer Project Standards. [book: Ch 2, "Transferring Project Standards"]
- Pre: at least one other project open in the session; otherwise a "no open projects" message appears and tool does nothing.
- Sequence: open source project, make target current; tool -> "Select Items To Copy" dialog; "Copy from" drop-down lists open projects; check boxes for standards (all on by default); Check All / Check None; OK. If any exist already in the target, a "Duplicate Types" box: Overwrite or New Only.
- What transfers: system family types only (not loaded families), line weights, line styles, line patterns, materials, view templates, and so on. [book]
- Gotchas: loaded families are not transferred (they must be loaded). [book]

### 3.6 Browser Organization (View > Windows > User Interface > Browser Organization)

- Where: View tab > Windows panel > User Interface drop-down. [book: Ch 2, "Setting the Browser Organization"; Tutorial 1]
- Dialog tabs: Views, Sheets, Schedules, each a list of organizations with check boxes (one active); Edit, New, Apply, OK.
- Properties dialog tabs: Filtering (Filter by up to several rows: view property, operator, value) and Grouping and Sorting (Group by, Then by, Sort by, ascending/descending, "All characters" vs first N characters for grouping).
- Tutorial config: Group by Discipline, then Family and Type, sort by View Name ascending. Enabling the new organization (check) and Apply regroups the browser. [book]
- Default browser behaviours: nodes expand/contract with +/-, current view in bold, search bar at the top of the browser searches views, schedules, sheets, families, groups and links as you type; double-click or drag a view to open it. [book: Ch 1, "Project Browser"]
- Rules: grouping is driven by view properties (Discipline, family and type, view type, level, etc.); duplicate view + change Discipline to relocate it. [book: Ch 3 Tutorial 1]

### 3.7 Insert > Link Revit (and link management)

- Where: Insert tab > Link panel > Link Revit. Right-click Revit Links node in browser > New Link. Manage Links: Insert tab > Link panel > Manage Links. [book: Ch 2, "Linking Revit Models and Sharing Coordinates"]
- Pre: the host MEP project should be open; the linked architectural/structural file should be accessible.
- Sequence (link):
  1. Insert > Link Revit; dialog "Import/Link RVT"; browse, select the file; a Preview pane shows it.
  2. Positioning drop-down area: choose auto/manual positioning (the book names the area "Positioning" but not the exact options; [knowledge] typical options: Auto - Origin to Origin, Auto - Center to Center, Auto - By Shared Coordinates, Auto - Project Base Point to Project Base Point, Manual - Origin, Manual - Base point, Manual - Center).
  3. Open. The model appears and the link is listed under the Revit Links node in the Project Browser, with nested links under their host link. [book]
- Manage Links dialog columns: Status (loaded or not), Reference Type (Overlay is the default; overlay stops nested links being loaded/displayed in the host; the other option is Attachment, [knowledge]), Position Not Saved (whether the link location is stored in shared coordinates), Saved Path, Path Type (relative or absolute; relative recommended because Revit can re-resolve after folders move). [book]
- Shared coordinates: the host and linked model must share coordinates so the link keeps position. Use Manage > Project Location > Coordinates drop-down > Acquire Coordinates and then click the linked model: the link's shared origin becomes the host's shared origin. [book]
- Link visibility control: View > Graphics > Visibility/Graphics, tab "Revit Links": expand link; Halftone and Display Settings columns; click Display Settings to open "RVT Link Display Settings" with tab Basics: By host view / By linked view (choose Linked view) / Custom; Nested links: By parent link / By linked view; Apply, OK. Other tabs exist for filters and phases. The tab only exists if links exist. [book: Ch 2, "Controlling the Visibility of Linked Models"]
- Bind Link: select the link, contextual tab "Modify | RVT Links" > Link panel > Bind Link; dialog "Bind Link Options" with checkboxes Attached Details, Levels, Grids (and the real dialog also has others [knowledge]); OK. The link becomes a group in the host. If a group with the same name exists, prompt to replace (Yes) or rename (No); after Yes there is a warning to remove the link now ("Remove Link") or later (OK). The result appears under Groups. [book]
- Copying link elements: hover the link, press TAB to cycle to the desired element (important with nested links), click; "Copy to Clipboard" in the Clipboard panel; open the host (or other file), Paste from Clipboard, click to place. [book]
- Schedules including linked elements: select a schedule, in Properties > Fields > Edit, tick "Include elements in Linked files"; to include project information of a link, pick Project Information in the field category drop-down. [book]
- Gotchas: [book] Revit links behave like AutoCAD Xrefs. Host changes do not modify the link. Nested links hide by default (Overlay).
- Outputs: IFC4 equivalent would be separate IfcProject files referenced by the app; position via IfcMapConversion/IfcProjectedCRS or IfcLocalPlacement. [knowledge]

### 3.8 Copy/Monitor (Collaborate > Coordinate > Copy/Monitor)

Book content here is a single short paragraph; details below from the paragraph, with [knowledge] flagged.
- Where: Collaborate tab > Coordinate panel > Copy/Monitor drop-down > Select Link. [book: Ch 2, "Copying and Monitoring Linked Model Elements"]
- Pre: a loaded linked architectural project. Purpose given: keep MEP aligned with changes in the architecture, avoid conflicts.
- Sequence:
  1. Choose Select Link; click the linked model in the drawing.
  2. The contextual "Copy/Monitor" tab appears with a Tools panel: Copy (copy an element from the link into the host and monitor it) and Monitor (relate an existing host element to the corresponding linked element).
  3. Use the tools.
  4. Click Finish in the Copy/Monitor panel.
- Rules: monitored elements show an eye-like symbol marker. [book]
- [knowledge] Typical subjects: levels, grids, walls, floors, columns, openings. Options for Copy include Multiple, Separate; Options button sets parameter mapping (offset limits, type mapping). Monitoring creates a relationship; on reload of link, Coordination Review flags changes (moved, type changed, deleted). Copied levels create no plan views automatically unless requested; Copy of levels from an architect gives the MEP file the same elevations; the reverse is not allowed to drift without a warning.
- Gotchas: Copy/Monitor, not just Copy, needed to detect later changes; reapplying the tool is required after each link reload. [knowledge]
- Outputs: none directly for IFC; for the app: a relationship record (host element <-> source link element GUID, monitored parameters, last-seen values).

### 3.9 Snaps (Manage > Settings > Snaps)

- Where: Manage > Settings > Snaps. Toggle all with `SO`. [book: Ch 2, "Snaps Tool"]
- Scope: not saved with the project; applies to all projects in the session. [book]
- Dimension Snaps area: "Length dimension snap increments" with default list of four values, semicolon separated (4', 0'6", 0'1", 0'0 1/4" imperial; 1000, 100, 20, 5 mm). Angular dimension snap increments similar. Multiple values because the active increment depends on zoom: zoom in uses smaller, zoom out uses larger. Example custom values in the book: 5'; 2'; 4". [book]
- Object Snaps area: Endpoints, Midpoints, Nearest, Work Plane Grid, Quadrants, Intersections, Centers, Perpendicular, Tangents, Points, Snap to Point Clouds, Snap to Remote Objects. All enabled snaps work at once. "Check All"/"Check None" except Snap to Remote Objects. Only snaps to visible objects. A marker and tooltip name the snap; shapes: endpoint = square, midpoint = triangle, nearest = cross. [book]
- Snaps Off check box at top of the dialog; `SO` keyboard toggle while in a tool. [book]
- Temporary overrides (single-use): type two-letter shortcut (e.g. `SE` for endpoint) before picking; it turns off after one use. TAB cycles snap candidates at the same spot. SHIFT constrains to orthogonal. [book]
- Gotchas: object snap needs visible geometry; the work plane grid snap needs the grid to be shown. [book]

### 3.10 Save / Save As / Open / Options / Close

- Save As: File > Save As > Project (or Template). Dialog has Options button -> "File Save Options": Maximum backups (defaults: 3 for non-workshared, 20 for workshared), Worksharing area (compact central, default workset, inactive when not workshared), Thumbnail Preview Source (default Active view/sheet; can pick e.g. "3D View: {3D}"), Regenerate if view/sheet is not up-to-date (costly). [book: Ch 2, "Saving an MEP Project"]
- Save: first save behaves as Save As. QAT button. [book]
- Open: File > Open > Project, QAT, or `CTRL+O`; dialog with Preview pane; Places list customizable via Options > File Locations > Places. Opening an older-version file triggers "Program Upgrade" one-time upgrade. Opening an already-open modified file asks to close first. Drag and drop of multiple files from Explorer opens each. [book: Ch 2, "Opening an Existing MEP Project"]
- Close: File > Close prompts Yes/No/Cancel when unsaved. Exit Revit: File > Exit Revit. [book]
- Options dialog (File > Options), tabs: General (save reminder interval, default 30 minutes; Synchronize with Central reminder; Username; Journal file cleanup: delete when count > N and older than D days; worksharing update frequency; Default view discipline = Coordination), User Interface (tab switching behaviour: stay on Modify tab or return to previous tab; Ctrl+Tab order; tooltip assistance None/Minimal/Normal/High; keyboard shortcuts customize; theme Light/Dark), Graphics (simplify during navigation, allow navigation during redraw, smooth lines/anti-aliasing), Hardware (acceleration), File Locations (templates default path, user files path, family templates path, point clouds, Places, system analysis workflow `.osw` file), Rendering (Render Appearance Library read-only; additional appearance paths), Check Spelling (dictionaries; `F7` runs spelling), SteeringWheels, ViewCube, Colors, Cloud Model, Macros. [book: Ch 2, "The Options Dialog Box"]
- Journal files record every step for debugging and recovery; saved at each session close. [book]
- Username in Options is used for multi-user editing permissions. [book]
- Note on partial stuff: these are UI/app-level conveniences; for the app only: autosave reminder, backup count, thumbnail source, user name, default template path, keyboard shortcut editor.

### 3.11 View Control Bar, Temporary Hide/Isolate, Reveal Hidden, Visibility/Graphics (VG)

- View Control Bar (bottom left): Scale, Detail Level (Coarse/Medium/Fine), Visual Style (Wireframe, Hidden Line, Shaded, Consistent Colors, Realistic [book also mentions Raytrace]), plus Temporary Hide/Isolate (sunglasses icon) and Reveal Hidden Elements (bulb icon). [book: Ch 1, "View Control Bar"; Ch 3, "Using the Temporary Hide/Isolate Tool"]
- Temporary Hide/Isolate: only available after selection; menu with six items: Hide Element, Isolate Element, Hide Category, Isolate Category, Reset Temporary Hide/Isolate, Apply Hide/Isolate to View. A cyan border shows temporary mode. Apply makes the state permanent for the view. [book]
- Reveal Hidden Elements: bulb toggles a magenta-bordered mode in which hidden elements are shown in magenta; select and use Unhide Element or Unhide Category; click bulb again to exit. [book]
- Visibility/Graphics (`VG`): View > Graphics > Visibility/Graphics; tabs Model Categories, Annotation Categories, Imported Categories, (Filters, Revit Links, etc.). Per category: Visible check, Projection/Surface, Cut, Halftone, Transparency, Detail level overrides. Dialog title shows the view name. [book: Ch 2, Ch 3]
- Per-element override: select, right-click > Override Graphics in View > By Element; dialog "View Specific Element Graphics": Visible, Halftone, Projection Lines (weight, color, pattern), Surface Patterns (visible, color, pattern), Surface Transparency (slider/edit; higher = more transparent), Cut Lines, Cut Patterns. Apply/OK. Transparency leaves edges and patterns visible. [book]
- By Category: right-click > Override Graphics in View > By Category; opens "View-Specific Category Graphics"; its button leads to full VG for the view. [book]
- Hide in View: right-click > Hide in View > Category or Elements. [book]
- Properties-level view parameters: View Scale, Display Model, Detail Level, View Name, View Range, Crop Region Visible, Discipline. [book: Ch 3, "Modifying the Plan View Properties"]
- Gotchas: Level and grid visibility are controlled by category (Annotation Categories > Grids). Hiding category vs hiding element differ. If a 3D view is halftone grey, change its Discipline to Architecture or Coordination. [book]

### 3.12 Level (Architecture > Datum > Level; `LL`)

- Where: Architecture tab > Datum panel > Level (also Structure tab, [knowledge]). Shortcut `LL`. [book: Ch 3, "Adding Levels"]
- Pre: must be in an elevation or section view; the Level tool is disabled in plan views. [book]
- Interaction sequence:
  1. Open an elevation/section (e.g. North). Start Level.
  2. Contextual tab "Modify | Place Level". Draw panel: Line (default) or Pick Lines.
  3. Type Selector: in Imperial Level: 1/4" Head, Level: No Head, Plenum; in Metric Level: 8mm Head and Level: Plenum. [book]
  4. Options Bar: Make Plan View check box (default on); Plan View Types button opens "Plan View Types" dialog listing Ceiling Plan and Floor Plan (multi-select) to decide which views are created; Offset edit box (distance from a picked point/element).
  5. Move the cursor near an existing level: a temporary dimension shows the distance to the nearest level; type a value to set elevation by distance; alternatively type the elevation number before the first click. Click the start point, move left/right, the level line plus name and elevation follow the cursor; alignment dashed line appears when aligned with existing level ends; click for end point.
  6. Esc / Modify to leave.
- Naming: each new level takes the next default name automatically; the line length is just display. [book]
- Level behaviour: a level is an infinite plane even though the line is finite. [book]
- Selected-level controls: [book]
  - Bubble visibility check boxes at each end.
  - Drag circles at ends to resize line.
  - Padlock = length alignment lock (locked: changing one line changes all aligned level lines; unlock to adjust individually).
  - Break control ("add elbow") offsets bubble and name with extension line; blue dots to position it.
  - 2D/3D extents toggle: 3D extents propagate to all views; 2D extents make the change view-specific.
  - Click the name to rename; Revit asks whether to also rename associated views (Yes renames plans in browser).
  - Click the temporary dimension to type a new elevation; or drag (SHIFT locks vertical).
  - Right-click: Go to Floor Plan, Find Referring Views (opens Go To View dialog + Open View).
- Hiding: right-click > Hide in View > Category or Elements; or Override by Category with Visible cleared (Modify | Levels > View > Override Graphics in View). Scope Box assignment also limits visibility. [book]
- Type properties (change applies to all levels of the type): [book]
  - Elevation Base (relative to project origin or shared origin), Line Weight, Color, Line Pattern, Symbol (head family or None), Symbol at End 1 (Default), Symbol at End 2 (Default).
- Instance properties: [book]
  - Elevation; Name; Computation Height (used for area, perimeter, volume of rooms); Structural (off by default; e.g. top of foundation); Building Story (on by default; marks a functional story); Story Above; Scope Box.
- Defaults: the default template provides Level 1 and Level 2. [book]
- Gotchas:
  - Plan views are created only if Make Plan View was on; otherwise create later via View > Plan Views. [book]
  - Renaming a level prompts to rename views; a "No" leaves mismatched names.
  - A view icon is blue if the view is on a sheet (2024 UI detail). [book]
  - Level elevation base: project vs shared origin matters for linked models. [book]
- Outputs: IfcBuildingStorey (Name, Elevation; Building Story flag decides whether a Level becomes a storey [knowledge]); non-story levels (plenum, sill) become reference data only. [knowledge]
- Copy/Monitor levels: this is the way an MEP file adopts architectural levels (see 3.8). [book: Ch 1 workflow; detail knowledge]

### 3.13 Grid (Architecture > Datum > Grid)

- Where: Architecture > Datum > Grid. Contextual tab "Modify | Place Grid". [book: Ch 3, "Working with Grids"]
- Pre: a plan, elevation or section view (the book's tutorial uses plan). Grids appear in all plan, elevation, section views they intersect. [book]
- Draw panel tools: Line (default), Start-End-Radius Arc, Center-ends Arc, Pick Lines, Multi-Segment. Options Bar: Offset; Radius edit box (enabled by checking the box before it) for arcs. [book]
- Line grid: click start, move; a temporary angular dimension shows the angle; type the angle after sketching by clicking the angle dimension; hold SHIFT for orthogonal. The new grid shows a bubble toggle at each end and two drag circles. [book]
- Second grid near an existing one: a temporary dimension shows the distance; type the spacing or just move the cursor; alignment lines appear. Name auto-increments (1, 2, 3...). [book]
- Pick Lines: pick existing lines/walls/etc. to create grids; combined with Offset in the Options Bar to offset from element face (Tutorial: offset 2 1/2" / 64 mm for 5"/90 mm interior walls; 3'/900 mm from the first wall centerline for the first grid). [book: Tutorial 1]
- Multi-Segment: choose from Draw panel; moves to "Modify | Edit Sketch"; sketch segments (Line, arc, etc.) clicking points; Finish Edit Mode (green check) in Mode panel. Result is a polyline-style grid. [book]
- Modify: select; rename by clicking the name (or Properties Name); temporary dimension to re-space; drag; Ctrl-select multiple; SHIFT constrains; right-click menu items Select Previous, Select All Instances, Create Similar, View Properties, etc.; bubble offset via the break control; 2D/3D extents toggles as for levels. [book]
- Type properties: Symbol; Center Segment (Continuous / None / Custom); Center Segment Weight, Color, Pattern (if Custom); End Segment Weight, Color, Pattern; End Segment Length (paper size) when Center segment is None or Custom; Plan View Symbols End 1 (Default) (default off), End 2 (Default) (default on); Non-Plan View Symbols (Default): none/top/bottom/both. [book]
- Instance properties: Name, Scope Box. [book]
- Gotchas: if grids "disappear", check VG Annotation Categories > Grids. [book] Grid and level visibility use scope boxes; an unassociated datum has infinite scope. [book]
- Outputs: IfcGrid with UAxes/VAxes (IfcGridAxis) [knowledge]; arcs and multi-segment need IfcGridAxis curves.

### 3.14 Reference Plane (Architecture > Work Plane > Ref Plane; `RP`)

- Where: Architecture > Work Plane > Ref Plane. `RP`. Contextual "Modify | Place Reference Plane". [book: Ch 3, "Reference Planes"]
- Sequence: choose Draw tools (Line, etc.); click start point, then end point; reference plane is a line in plan that represents a vertical plane. Name it via Properties > Name.
- Use: datum guideline for placing elements and for families. Can be assigned to a Scope Box. Appears in the Work Plane "Name" drop-down. [book]

### 3.15 Work Plane (Architecture > Work Plane > Set / Show / Viewer)

- Concept: elements can be created/edited only on the active work plane; horizontal, vertical or inclined. Plan views have an automatic plane; sections/elevations/3D need one set. [book: Ch 3, "Work Planes"]
- Set: Architecture > Work Plane > Set -> "Work Plane" dialog. Specify New Work Plane radio options: Name (drop-down listing levels, grids, reference planes), Pick a plane (then click a plane/face such as a wall face, floor, roof), or Pick a line and use the work plane it was sketched in. [book]
- Show: toggles the visible work-plane grid; select the displayed plane and set Spacing in the Options Bar; snaps can target the work plane grid. [book]
- Viewer: opens the "Workplane Viewer" for editing work-plane-dependent elements. [book]
- Gotchas: sketching a floor in 3D requires the work plane to be a plan plane (see floors). [book]

### 3.16 Scope Box (View > Create > Scope Box)

- Where: View > Create panel > Scope Box. [book: Ch 3, "Using the Scope Box Tool"]
- Sequence: tool in a plan view; Options Bar shows Name and Height; cross cursor; click the upper-left corner and the opposite corner; drag controls on the box to resize.
- Apply: select a datum (grid/level/reference plane), Properties > Scope Box parameter > pick the box > Apply. The datum is then drawn only in views whose cut plane intersects the scope box. [book]
- View visibility: select the scope box; Properties > "Views Visible" Edit button opens "Scope Box Views Visible": table of view types and names with column Automatic Visibility and column Override (None/Visible/Invisible). [book]
- Rule: datum has infinite extent by nature; the scope box limits where it is displayed per view. [book]

### 3.17 Plan / Elevation / Section Views

- Floor Plan: View > Create > Plan Views > Floor Plan -> "New Floor Plan" dialog: Type drop-down with Edit Type (Type Properties, Duplicate), level list (SHIFT for multiple), "Do not duplicate existing views" check box (clear it to create duplicates; duplicates get suffix (1), (2)...). Ceiling plans similarly. [book: Ch 3, "Adding a Plan View"]
- Default Systems template views: floor plans (Mechanical, Electrical, Plumbing), ceiling plans for Electrical and Mechanical; opens with "1 - Mech". Four elevation markers appear (N,E,S,W); double-click one to open the elevation. [book]
- View properties (Properties palette): View Scale, Display Model, Detail Level, View Name, View Range (Extents group), Crop Region Visible, Discipline. View Range defines the horizontal cut slab that determines what plan shows. [book]
- Elevation: View > Create > Elevation. Type Selector in Properties selects elevation type; the arrow flips to be perpendicular to the nearest wall as the cursor moves; click to place; numbering automatic; the elevation lands in the browser under the discipline of the host plan view (e.g. Mechanical > Elevations (Building Elevation)). Clip plane control (blue line with drag dots) sets the width; for interior elevations the extents auto-extend to the room. Rename via right-click > Rename (Rename View dialog). Open via double-click on name or arrow, right-click symbol > Find Referring Views or Go to Elevation View. [book: Ch 3, "Creating an Elevation View"]
- Gotcha: if the model grows past the clip plane of the four default elevations, the elevations show a cut-through; drag the clip control beyond the building. [book]
- Section: View > Create > Section; "Modify | Section"; Type Selector chooses Detail View: Detail, Section: Building Section, Section: Wall Section (listed under corresponding browser subheads Sections (Building Sections), Sections (Wall Sections), Detail Views (Detail)); Options Bar "Reference Other View" makes a reference section (not added as its own browser view). Click start, click end (horizontal/vertical or any angle). [book]
- Section controls: flip arrows (twin arrows) to flip the viewing side; head/tail cycle controls to show or hide at each end; break line to split the line into two ranges then rejoin; view depth shown as dashed line with blue arrows; Properties: View Name, View Scale, Crop Region Visible; Type Properties: Callout Tag, Section Tag, Reference Label. Hide section line in view: right-click > Hide in View > Element. Section lines are shown in other views if the view range intersects the crop region. In-place elements are not shown in section cuts. [book]
- Segmented section: Split Segments (Section panel in "Modify | Views"); click on the line at the split point, move perpendicular, click again for the offset segment; segments stay orthogonal to the view direction. [book]
- Level tool needs an elevation/section. [book]

### 3.18 Wall: Architectural (Architecture > Build > Wall > Wall: Architectural)

- Where: Architecture > Build > Wall drop-down. Contextual "Modify | Place Wall". [book: Ch 3, "Creating Architectural Walls"]
- Wall kinds: architectural walls have no analytical (structural) properties; structural walls do. Curtain walls consist of panels and mullions. Exterior predefined examples: Brick on CMU, Brick on Mtl. Stud, CMU Insulated. [book]
- Pre: a plan view (or 3D); levels exist.
- Interaction sequence (sketch):
  1. Start the tool; choose wall type in Type Selector (Properties).
  2. Options Bar: Level/height (constraint) controls, Location Line, Offset, Chain, and, for arc/circle, Radius check box + edit box.
  3. Pick a Draw tool: Line, Rectangle, Polygon, Circle, Arc variants, Pick Lines/Faces. Book names Line and Rectangle, Circle (as the "sketching tools"), and the answer key mentions Fillet Arc among draw tools.
  4. Line: click start, move; temporary dimension shows length and angle; alignment dashed line and tooltip tell horizontal/vertical; the cursor snaps to the Dimension Snap increments. Click end, or type a length then ENTER before clicking the end. Chain on: end becomes next start.
  5. After sketching, select the wall: click the temporary dimension to type the exact length, ENTER. To set angle, click the angular dimension and type. The "convert temporary dimension to permanent" icon appears.
  6. Esc twice (or Modify) to leave the tool.
  7. Circle: click center, move/type radius (distance center to location line) + ENTER; or tick the Radius check box and type the number in Options Bar, then click the center.
- Selected wall controls: blue flip arrows on the exterior side (default: walls drawn left to right have exterior on top; top to bottom exterior on right); click arrows or press SPACEBAR while hovering the control to flip; blue dots at ends for stretch. [book]
- Location Line choices: Wall Centerline, Core Centerline, Finish Face: Exterior, Finish Face: Interior, Core Face: Exterior, Core Face: Interior. The dashed line during sketch shows the active reference. The location line stays fixed when the type thickness changes (so editing composition does not shift the wall). It is an instance property (the book's quiz says true). [book]
- Instance parameters (named): Location Line, Base Constraint, Base is Attached, Base Offset, Base Extension Distance, Top Constraint (level or Unconnected), Unconnected Height (default 20'0" imperial or 8000 mm), Top Offset, Top Extension Distance, Room Bounding, Related to Mass, Cross-Section (vertical or slanted), Structural Usage, Length, Area, Volume, Comments, Mark, Phase Created, Phase Demolished, Image. Groups: Constraints, Structural, Dimensions, Identity Data, Phasing. [book]
- Type parameters: Structure (Edit -> Edit Assembly), Function, Coarse Scale Fill Pattern, plus analytical: Thermal Mass, Thermal Resistance, Heat Transfer Coefficient (read-only, computed), Absorptance and Roughness (editable). [book]
- Edit Assembly dialog: [book]
  - Layers table; first row = exterior side, last = interior side. Columns: Function, Material, Thickness, Wraps, Variable (check to make a layer taper), and layer rows insertable/deletable with Insert/Delete/Up/Down. Total thickness at top.
  - Layer functions: Structure [1], Substrate [2], Thermal/Air Layer, Membrane Layer (zero thickness vapour barrier), Finish 1 [4] (exterior), Finish 2 [5] (interior). The bracket number is the priority; higher priority wins at joins at corners and intersections.
  - Default Wrapping: At Inserts (Exterior, Interior, Both, Do not wrap) and At Ends (Exterior, Interior, None).
  - Preview button shows a section; View drop-down "Section: Modify type attributes".
  - The tutorial wall type "Brick on Mtl. Stud" has ten layers and total thickness 1'1 7/8" (350 mm).
- Typical tutorial moves: duplicate a type (Edit Type > Duplicate > Name) then set the Structure layer thickness; name types with the thickness to be informative (but the name does not drive the value, see doors). [book]
- Rules:
  - Walls are Room Bounding by default checkbox; this feeds rooms/spaces. [book]
  - Walls are the host for doors, windows and wall openings.
  - Height may be level-to-level (Top Constraint "Up to Level") or Unconnected Height.
- Gotchas: [book] Wall height defaults vary; "Unconnected Height" default is 20'/8000 mm; the Location Line is drawn along the sketch line, not the visual centre unless centerline. Interior grid offsets use half thickness. Walls sketched lower-to-upper level: browser level name turns bold to show the working level.
- Outputs: IfcWall / IfcWallStandardCase (type IfcWallType) with IfcMaterialLayerSetUsage; IsExternal and LoadBearing from Function/Structural Usage; IfcRelSpaceBoundary for room bounding walls (needs spaces, Ch 4). [knowledge] Thermal properties map to Pset_WallCommon (ThermalTransmittance). [knowledge]

### 3.19 Door (Architecture > Build > Door; `DR`)

- Where: Architecture > Build > Door. `DR`. Contextual "Modify | Place Door". [book: Ch 3, "Using Doors in a Building Model"]
- Pre: a host wall must exist; the family must be loaded: default templates carry none. First use pops a prompt that no door family is loaded -> Yes opens Load Family dialog (library folders English-Imperial > Doors, or English > Doors for Metric) -> select family -> Open -> "Specify Types" dialog (CTRL multi-select) -> OK. Alternatively Insert > Load from Library > Load Family. [book]
- Placement sequence: choose type in Type Selector; hover the wall (a "+" cursor and a door preview appear); the preview flips to the side of the wall nearest the cursor; temporary dimensions show distance to the adjacent walls/edges; click to place; opening is cut automatically.
- Options Bar: tag orientation (Horizontal or Vertical); Tag on Placement button on the Tag panel toggles automatic tagging.
- After place: select; twin-arrow along the wall edge flips swing side (hand); the perpendicular twin arrow moves the door to the other side of the wall (facing); click a temporary dimension and type exact distance to the center. [book]
- Contextual tab tools: Model In-Place (Mode panel; opens Name dialog then Family editor "In-Place Editor"; Finish Model), Load Family; Tag panel. [book]
- Replace type: select the door and choose another type in the Type Selector. [book]
- Tags: door mark increments automatically per placement/copy; each door instance gets its own mark; visibility of Door Tags controlled in VG Annotation Categories. [book]
- Type parameters (named): Function (Interior / Exterior; used in scheduling), Wall Closure, Construction Type, Door Material, Frame Material, Height, Trim Projection Ext, Trim Projection Int, Trim Width, Width, Rough Width, Rough Height, Assembly Code, Keynote, Model, Manufacturer, Type Comments, URL, Description, Assembly Description, Type Mark, Fire Rating, Cost; groups: Construction, Materials and Finishes, Dimensions, Identity Data, IFC Parameters. [book]
- Instance parameters: Level, Sill Height, Frame Type, Swing Angle, Frame Material, Finish, Image, Comments, Mark, Phase Created, Phase Demolished, Head Height. [book]
- Rules: [book] A door can be placed in any wall regardless of height, with alerts if misplaced; Duplicate a type before editing a library one; renaming a type does not change its width (type name is just a label); Load button in Type Properties adds more types of the family.
- Gotchas: tag numbering is per placement; "Rough Width/Height" are for schedule/export only.
- Outputs: IfcDoor + IfcOpeningElement via IfcRelVoidsElement and IfcRelFillsElement; OverallWidth/Height; FireRating in Pset_DoorCommon; IsExternal from Function. [knowledge]

### 3.20 Window (Architecture > Build > Window)

- Same flow as the door. [book: Ch 3, "Adding Windows in a Building Model"]
- Pre: wall host; family loaded (Windows folder; same prompt/Specify Types). [book]
- Placement: hover; the preview appears on whichever face the cursor is nearer; temporary dimension to adjacent wall edges; click; edit via temporary dimension; flip arrow flips orientation (arrow shown on interior side).
- Marks: windows of the same type share the same mark (differs from doors). The Type Mark can be set in Type Properties. Window tag visibility in VG Annotation Categories (hiding does not delete). [book]
- Type parameters: Wall Closure, Construction Type, Glass Pane Material, Sash Material, Trim Exterior Material, Trim Interior Material, Height, Default Sill Height (3'0" / 915 mm), Trim Projection Ext/Int, Trim Width Exterior/Interior, Width, Window Inset, Rough Width, Rough Height, Keynote, Model, Manufacturer, Type Comments, URL, Description, Assembly Description, Type Mark, Cost. [book]
- Instance parameters: Level, Sill Height, Head Height, Image, Comments, Mark, Phase Created, Phase Demolished. [book]
- To make a larger window: select the window type -> Edit Type -> Duplicate -> change Width. [book Tutorial 2]
- In-Place and Load Family: same as doors.
- Outputs: IfcWindow, IfcOpeningElement, OverallWidth/Height; glazing for load calcs (heat gain through glazing) [knowledge: Ch 4+ uses glazing area].

### 3.21 Wall openings (Door/Window as openings; opening tools)

- Pre-made families "Passage Opening - Cased" / "Elliptical Arch" (door-like) and "Opening with Trim" / "Opening" (window-like) are in the Openings library folder; place with Architecture > Build > Component > Place a Component after loading. Width/height are editable. [book: Ch 3, "Doors and Windows as Wall Openings"]
- Opening panel (Architecture > Opening): By Face, Shaft, Wall, Vertical, Dormer. The view Discipline must be Architecture for the openings to work. [book: Ch 3, "Cutting Openings in a Wall, Floor, and Ceiling"]
- Wall opening (rectangular, curved or straight walls): best in elevation/section. Choose Wall tool in Opening panel, click the host wall, then click two diagonal corners; adjust via temporary dimensions; Modify to exit; properties: height, top offset, base offset. [book: "Openings in the Wall"]
- By Face: select a planar face of a floor/ceiling/beam/column; sketch boundary in the contextual "Modify | Create Opening Boundary" tab; Finish Edit Mode; the cut is perpendicular to the face. [book]
- Vertical: select ceiling or floor; sketch; Finish; cut is vertical regardless of face slope. [book]
- Shaft: sketch in plan, Finish; cuts through the full height; restrict extents with Base Constraint and Top Constraint (levels) in Properties after selecting it ("Modify | Shaft Openings"). Required: set the work plane and the view first. [book] (This is the standard way MEP risers get architecture voids; the book does not state that explicitly.)
- Dormer: select the roof, then Pick Roof/Wall Edges to define a closed loop; Finish. [book]
- Outputs: IfcOpeningElement with IfcRelVoidsElement to wall/slab. [knowledge]

### 3.22 Floor: Architectural (Architecture > Build > Floor)

- Where: Architecture > Build > Floor drop-down > Floor: Architectural. Tab "Modify | Create Floor Boundary". [book: Ch 3, "Creating Architectural Floors"]
- Pre: plan view of the target level (or 3D if work plane is the plan).
- Sequence: enter sketch mode (drawing area fades); Pick Walls (default) with Options Bar: Offset, "Extend into wall (to core)" check box; hover the wall: face choice follows cursor (dashed line); click the wall; magenta boundary line appears with a flip control (switch between faces) and drag controls. Repeat for walls; the boundary must be a closed loop with no overlap. Alternatively sketch using Line, Rectangle, Polygon, Arc (Chain and Offset options). Use sketch Modify tools (Split Element, Trim, Delete) to clean overlapping lines. Finish Edit Mode (green check, Mode panel). Floor will not be visible in plan at Hidden Line unless the view range sees it; use Default 3D View.
- Slope: Slope Arrow tool (Draw panel; Line or Pick Lines); set via Properties of the arrow: how slope is specified, the level of tail, height offset at tail, etc.
- Type properties: Structure (Edit Assembly layers like walls), plus usual identity; Type Selector includes Floor: Generic 12" (300 mm). Floor instance: Level, Height Offset From Level (inferred) etc. [book: the instance parameters named: the level and height offset of the floor from the level]
- After creation: "Modify | Floors" tab with nine panels: Properties, Clipboard, Geometry, View, Dimension, Create, Modify, Mode, Shape Editing. [book]
- Gotcha: if picked boundary crosses interior walls, split and delete segments. [book Tutorial 2]
- Outputs: IfcSlab (PredefinedType FLOOR) with material layer set; [knowledge]. For MEP: floors define room/space lower boundary [book: Room Bounding Elements].

### 3.23 Ceiling (Architecture > Build > Ceiling)

- Where: Architecture > Build > Ceiling; "Modify | Place Ceiling". Panel "Ceiling": Automatic Ceiling (default), Sketch Ceiling. [book: Ch 3, "Placing Ceilings"]
- Pre: ceilings are placed in ceiling plans; in a floor plan the ceiling is created but Revit warns that it is not visible there. [book]
- Automatic Ceiling: choose type in Type Selector; set Height Offset From Level in Properties; hover inside a closed region: the boundary highlights in red; click. One click per enclosed room. [book + Tutorial 2]
- Sketch Ceiling: "Modify | Create Ceiling Boundary": Boundary Line tool and draw tools (Line default), Pick Walls (Options Bar Offset and "Extend into wall (to core)"), Finish Edit Mode. Pick Walls is the usual route. [book]
- Modify existing: select; copy/move/rotate/mirror; "Edit Boundary" in Mode panel for sketch re-edit; selected boundary edges show their own instance properties. From a section view, Edit shows a "Go To View" dialog to choose where to edit the sketch. [book]
- Instance: Level, Height Offset From Level (e.g. 10' = 3000 mm above level; tutorial 8'6" = 2590 mm). Type: Structure (Edit Assembly). [book]
- Rules: ceiling is a level-hosted element; it is a room bounding element. For MEP plenum/air terminal hosting, ceiling is the host of ceiling-based air terminals and lights [knowledge]. The Level type "Plenum" in 3.12 is for plenum-type levels [book list item without explanation].
- Outputs: IfcCovering with PredefinedType CEILING; relation to IfcSpace through IfcRelCoversSpaces. [knowledge]

### 3.24 Room (Architecture > Room & Area > Room)

- Where: Architecture > Room & Area panel > Room. "Modify | Place Room". [book: Ch 3, "Creating Rooms"]
- Concept: rooms are architectural elements; placed only in plan views; created independently of room tags; Revit forms the basis for spaces (Ch 4). Rooms and areas look alike. [book]
- Sequence:
  1. Start Room; pick tag type in Type Selector: Room Tag, Room Tag With Area, Room Tag With Volume.
  2. Optional: Highlight Boundaries tool (Room panel): a message box says room-bounding elements are highlighted; Close.
  3. Tag on Placement (Tag panel) controls whether a tag is created.
  4. Options Bar: Upper Limit (level) default current; Offset (default 10'/3000 mm; positive up, negative down from Upper Limit level); tag orientation (Horizontal, Vertical, Model); Leader check box; Room drop-down listing unplaced rooms from a schedule.
  5. Move the cursor inside a closed boundary: a cross-hair shape scaled to the region area; click to create.
  6. Esc to end.
- Unenclosed placement: allowed in free space; Revit warns that the room is not in a properly enclosed region; the room resizes once walls close the region later. [book]
- From schedule: add rows in a room schedule (unplaced rooms), then in Room tool use the Room drop-down to choose one, click to place; if none selected, new numbering continues after the last schedule room number. [book]
- Room-bounding elements: walls, roofs, ceilings, curtain systems, floors, columns, room separation lines. Area is bounded by these and the height. [book]
- Instance parameters: Level, Upper Limit, Limit Offset (can be negative), Area, Perimeter, Unbounded Height (sum of base level, upper limit and limit offset), Volume (only with computation on), Number, Name (default "Room"), Comments, Occupancy, Base/Ceiling Finish, Wall/Floor Finish, Occupant, Phase. [book] Also "Base Offset" for the lower limit that prevents leaking through floor openings. [book]
- Volume computation: Architecture > Room & Area > Area and Volume Computations: radio Areas only (default) vs Areas and Volumes; Volume is blank until the latter. Volumes use finish faces of bounding elements; selecting the room shows the exact computed outline and fill. Volume engine looks up and down from a measurement height for the vertical limits of bounding elements. [book]
- Level Computation Height ties into room area/perimeter/volume. [book: Level instance params]
- Gotchas: tag type affects only the text; room geometry is separate; room needs bounding elements to have Room Bounding set; the ceiling and floor limit the height. [book]
- Outputs: IfcSpace (rooms are the architect's "spaces"; the book keeps Space and Room distinct—Ch 4). Parameters Name/LongName/Number map to IfcSpace Name/LongName. Area & volume to Qto_SpaceBaseQuantities. [knowledge]

### 3.25 Properties palette, Type Selector, Edit Type, Options Bar [book: Ch 1]

- Properties palette: dockable; shows instance properties of selection or of the active view if nothing is selected; Type Selector at top; Edit Type opens "Type Properties" (Duplicate, Rename, Load, Preview). Apply button commits. Properties button on Modify tab toggles palette.
- Type Selector: changes type of selection or sets the type before placement.
- Options Bar: contextual controls under the ribbon.
- Contextual tab: appears on selecting an element or invoking a tool, e.g. Modify | Place Duct has panels Select, Properties, View, Measure, Geometry, Clipboard, Create, Modify, Tag, Placement Tools (this list is from the book's description; it lists "Mode" for load families).
- Status Bar: prompts, and family/type name under cursor.
- Keyboard accelerators: tooltips show them; seen: `UN`, `LL`, `RP`, `DR`, `VG`, `SO`, `SE`, `F7`, `CTRL+O`, `CTRL+C/V`. [book]
- Multiple documents: several projects open at once; copy/paste between; Switch Windows and Tile Views in View > Windows. [book]

---

## 4. Cross-cutting rules (consolidated)

1. Levels are first-class planes. All of ducts, pipes, equipment, spaces, ceilings, floors, rooms, doors are level-attached (via Level plus offset/sill). [book]
2. Datum scope: levels/grids/reference planes have infinite extent; scope boxes only restrict where they display. [book]
3. Type vs instance: type edits cascade; instance edits do not. Duplicate before editing library types. Type name never constrains values. [book]
4. Wall layer priority drives joins; location line fixes the reference plane. [book]
5. Hosted objects (doors, windows) require a host wall and cut openings automatically. [book]
6. Rooms need a closed boundary of room-bounding elements, can be placed beforehand, are computed from finish faces, with optional volumes. [book]
7. Discipline of a view filters what is shown in full vs halftone; set to Architecture or Coordination to see architectural content in full. [book]
8. A linked model is read-only reference; use Copy/Monitor and shared coordinates to follow it; Bind Link converts it to a group and severs the live link. [book]
9. Units are display only. [book]
10. Browser organization is derived from view properties; changing a view's Discipline moves it. [book]
11. Everything lives in one database; editing in any view propagates. [book]

---

## 5. IFC4 mapping summary for this slice

| Revit item | IFC4 target | Source |
|---|---|---|
| Project Information (name, number, address, client) | IfcProject, IfcSite.Address, IfcOwnerHistory / IfcActorRole | [knowledge] |
| Location lat/long | IfcSite.RefLatitude/RefLongitude | [knowledge] |
| Level with Building Story | IfcBuildingStorey (Elevation) | [knowledge] |
| Grid | IfcGrid, IfcGridAxis | [knowledge] |
| Wall | IfcWall (IfcWallStandardCase), IfcWallType, IfcMaterialLayerSet | [knowledge] |
| Door / Window | IfcDoor / IfcWindow, IfcOpeningElement, IfcRelVoidsElement, IfcRelFillsElement | [knowledge] |
| Floor | IfcSlab FLOOR | [knowledge] |
| Ceiling | IfcCovering CEILING | [knowledge] |
| Room | IfcSpace (architectural) | [knowledge]; the book only says rooms underlie spaces |
| Shaft opening | IfcOpeningElement | [knowledge] |
| Export to IFC As, IFC schema version default IFC4, entity + predefined type, e.g. IfcPipeSegment/RIGIDSEGMENT | per-element override | [book] |
| Door IFC Parameters group | IFC parameters on type | [book] |
| HVAC Zone, IfcDistributionSystem, IfcZone | Chapter 4+ | not in this slice |
| Linked model | separate IFC file / reference | [knowledge] |

---

## 6. Acceptance checklist

Prefix = chapter / section. Each line is testable.

### Ch 1
- Ch1/Data model: An element has category, family, type, instance; editing a type parameter updates every instance of that type; editing an instance parameter updates only that instance.
- Ch1/Data model: Creating any element requires a level (datum) assignment; levels are infinite horizontal planes.
- Ch1/Data model: Datum elements (grids, levels, reference planes) are a separate class from host, component, annotation, view.
- Ch1/Parametric: Moving a wall in a plan view updates the section, elevation, 3D view and schedules without user action.
- Ch1/Scale: Changing a view's scale changes its default detail level (Coarse/Medium/Fine), and detail level can then be overridden manually.
- Ch1/Templates: A new project can start from a discipline template (Mechanical, Electrical, Plumbing, Systems, each with metric variant) that loads units, views and families.
- Ch1/IFC: An element has an "Export to IFC As" property that opens a chooser defaulting to IFC4, with search, an entity list and a predefined type list; the selected values are stored on the element.
- Ch1/IFC: An IfcPipeSegment with predefined type RIGIDSEGMENT can be assigned to a pipe through that chooser.
- Ch1/UI: Project Browser opens a view on double-click, supports search-as-you-type over views, schedules, sheets, families, groups, links; the active view is shown in bold.
- Ch1/UI: Properties palette shows view properties when nothing is selected and instance properties otherwise; Edit Type opens type properties.
- Ch1/UI: Options Bar contents change per tool.
- Ch1/Multi-doc: Two projects can be open; cut/copy/paste of elements works between them.

### Ch 2
- Ch2/New project: New Project dialog lets the user choose a template (default list plus Browse) and Project vs Project template.
- Ch2/New project: Choosing "None" as template asks for Imperial or Metric and produces default units for that system.
- Ch2/Template: Saving as template stores levels, views, families, types, units and any geometry present; opening it gives a new project with all of that.
- Ch2/Units: Project Units dialog groups units by Common, Structural, HVAC, Electrical, Piping, Infrastructure, Energy.
- Ch2/Units: Format dialog offers Units, Rounding (with Custom increment when allowed), Unit symbol and Suppress spaces; Unit symbol disabled for feet-and-fractional-inches and meters-and-centimeters.
- Ch2/Units: Defaults imperial/metric: air flow CFM/L-s; pressure inH2O/Pa; power BTU/h / W; heating load BTU/h / W; piping flow GPM / L-s; pipe size fractional inches / mm; illuminance fc / lux; electrical potential V; demand factor percentage; piping velocity ft/s / m/s.
- Ch2/Units: Changing a unit format changes displayed and typed values but not stored geometry; a wall rounded to the nearest 1' still stores its true length.
- Ch2/Units: Keyboard `UN` opens the dialog.
- Ch2/Project info: Project Information holds organization name/description, building name, author, issue date, status, address, project name, number, client; custom project parameters (local or shared) can be added and appear there.
- Ch2/Project info: A shared parameter needs a shared parameter file, a group and a parameter definition (name, discipline, type) before it can be bound to the Project Information category.
- Ch2/Location: Location accepts an address (map search) or city list or manual latitude/longitude; Site tab supports Duplicate, Delete, Make Current (current marked).
- Ch2/Transfer: Transfer Project Standards copies system family types, line styles/weights/patterns, materials, view templates but not loaded families; with name clashes the user chooses Overwrite or New Only; with no other project open it shows a message.
- Ch2/Browser org: Browser organization can be created with grouping (Group by, Then by) and sorting (Sort by, asc/desc) on view properties; applying it regroups the browser; changing a view's Discipline moves it to the other group.
- Ch2/Link: Link Revit lists the link under Revit Links; nested links show beneath their host link; default reference type is Overlay (nested links not shown in the host).
- Ch2/Link: Manage Links shows status, reference type, whether position is saved, saved path and path type (relative/absolute).
- Ch2/Link: Acquire Coordinates on a link makes the link's shared origin the host's shared origin.
- Ch2/Link visibility: Visibility/Graphics has a Revit Links tab only when links exist; halftone and display settings can be overridden per link; By host view, By linked view and Custom exist.
- Ch2/Bind: Bind Link converts a link into a group with options for attached details, levels, grids; same-name group triggers a replace prompt; the link is removed.
- Ch2/Link copy: TAB cycles the hovered element through nested links; copy-to-clipboard from a link and paste into the host works.
- Ch2/Link schedules: a schedule can include elements from linked files via a checkbox.
- Ch2/Copy-Monitor: Select Link then pick the link opens a Copy/Monitor tab with Copy and Monitor tools; a Finish button ends it; monitored elements show a marker.
- Ch2/Snaps: Dimension snap increments are a list of values; smaller increments are used when zoomed in, larger when zoomed out.
- Ch2/Snaps: Object snaps (endpoint, midpoint, nearest, work plane grid, quadrants, intersections, centers, perpendicular, tangents, points, point clouds, remote objects) can all be active at once; marker and tooltip show; only visible geometry snaps.
- Ch2/Snaps: `SO` toggles snaps off/on; typing a two-letter override (e.g. `SE`) enables one snap for one pick only; TAB cycles candidates; SHIFT restricts to orthogonal.
- Ch2/Snaps: Snaps settings are not saved in the project.
- Ch2/Save: Maximum backups defaults to 3 for local files and 20 for workshared; thumbnail source selectable; regenerate option exists.
- Ch2/Save: First Save on an unsaved project shows Save As.
- Ch2/Open: Opening an older-format file shows an upgrade notice and upgrades once.
- Ch2/Options: Save reminder default 30 minutes; username used for editing ownership; default view discipline Coordination; journal cleanup by count and age.

### Ch 3
- Ch3/Level: Level tool is disabled in plan views and enabled in elevation/section views.
- Ch3/Level: New level shows alignment line to existing level ends; typing an elevation before the first click sets it; with a nearby level, temporary dimension accepts a distance.
- Ch3/Level: Make Plan View on by default; Plan View Types lets the user create floor plan, ceiling plan, or both; clearing it creates no views.
- Ch3/Level: Level type choices (Imperial) 1/4" Head, No Head, Plenum; (Metric) 8 mm Head, Plenum.
- Ch3/Level: Renaming a level asks whether to rename related views.
- Ch3/Level: Level instance parameters: Elevation, Name, Computation Height, Structural, Building Story, Story Above, Scope Box; type parameters: Elevation Base, Line Weight, Color, Line Pattern, Symbol, Symbol at End 1 and 2.
- Ch3/Level: Dragging a level with SHIFT held moves it vertically; typing a new temporary dimension moves it.
- Ch3/Level: Length alignment lock moves all aligned level ends together; unlocked, only one.
- Ch3/Level: 2D extents change only the current view; 3D extents change all views.
- Ch3/Level: Right-click Go to Floor Plan and Find Referring Views exist.
- Ch3/Level: Hide in View > Category hides all levels in the view; Elements hides only the selected level.
- Ch3/Grid: Grid tool offers Line, two arc modes, Pick Lines, Multi-Segment; names auto-increment; Radius edit box requires its check box.
- Ch3/Grid: Pick Lines with Offset places a grid parallel to a picked element at the given distance.
- Ch3/Grid: Multi-Segment ends with Finish Edit Mode.
- Ch3/Grid: Type parameters include Center Segment (Continuous/None/Custom), End Segment Length when center is not Continuous, plan symbol defaults end 1 off and end 2 on, non-plan symbol display.
- Ch3/Grid: Scope Box instance parameter limits the views in which the grid is drawn to views whose cut plane intersects the scope box.
- Ch3/Reference plane: `RP` starts a reference plane; plane is nameable via Properties and appears in the work plane list.
- Ch3/Work plane: Work plane can be set by name (level/grid/reference plane), by picking a plane, or by picking a line's sketch plane; plan views have an automatic plane; the grid of a work plane can be shown with editable spacing and snapped to.
- Ch3/Views: Elevation tool creates an elevation whose arrow follows nearest wall; auto-numbers; lives under the discipline of its host view; clip plane width adjustable; interior elevation extents default to the room.
- Ch3/Views: Section types Building Section, Wall Section, Detail; Reference Other View creates a reference only.
- Ch3/Views: Section flip, head/tail visibility, break line, adjustable view depth, split segments exist.
- Ch3/Views: New Floor Plan dialog respects "Do not duplicate existing views"; duplicates are named with (1), (2).
- Ch3/Scope box: created by two corner clicks in plan with Name and Height in the Options Bar; Views Visible dialog gives Automatic Visibility and Override None/Visible/Invisible per view.
- Ch3/Visibility: Per-element override includes visible, halftone, projection lines, surface patterns, transparency (higher is more transparent), cut lines, cut patterns.
- Ch3/Visibility: Temporary Hide/Isolate has six actions; Apply to View makes it permanent; Reveal Hidden Elements shows hidden items in magenta and allows Unhide Element/Category.
- Ch3/Wall: Location Line options are the six listed; the line stays fixed when the type thickness changes.
- Ch3/Wall: Walls sketched left-to-right have exterior on the top side; SPACEBAR over the flip control flips.
- Ch3/Wall: Typing a number during sketch sets length; selecting then clicking the temporary dimension sets exact length; clicking the angle sets angle.
- Ch3/Wall: Chain option makes the end of one wall the start of the next; Offset in Options Bar offsets from the picked point.
- Ch3/Wall: Circle wall radius is measured to the location line; a Radius check box in the Options Bar presets it.
- Ch3/Wall: Unconnected Height default is 20 ft / 8000 mm; Top Constraint "Up to Level" overrides it.
- Ch3/Wall: Edit Assembly first layer is exterior, last is interior; layer priorities decide joins; Variable layers enable tapered walls; Default Wrapping at inserts/ends options exist.
- Ch3/Wall: Wall type analytical values Thermal Mass, Thermal Resistance, Heat Transfer Coefficient are computed and read-only; Absorptance and Roughness are editable.
- Ch3/Wall: Room Bounding instance flag exists and is on by default (affects rooms).
- Ch3/Door: No door/window families are present in the default templates; first use prompts to load and shows Specify Types.
- Ch3/Door: A door cannot exist without a host wall; placement cuts an opening and completes edges; flip controls change swing side and facing side.
- Ch3/Door: Renaming a door type leaves its width unchanged.
- Ch3/Door: Door marks increment on each placement; window marks are shared by type.
- Ch3/Door: Door type has Function (Interior/Exterior), Fire Rating, Width, Height, Rough Width/Height, Wall Closure, materials; instance has Level, Sill Height, Head Height, Mark, phases.
- Ch3/Window: Default Sill Height 3 ft / 915 mm; window type has Window Inset; duplicating a window type and changing Width yields a new size type.
- Ch3/Opening: Wall opening tool needs a rectangle (two corners) in an elevation/section/plan; Shaft cuts all levels between Base and Top Constraint; By Face cuts perpendicular to the face; Vertical cuts straight through; Dormer needs a closed loop of roof/wall edges.
- Ch3/Floor: Pick Walls with Extend into wall (to core) builds the boundary; closed non-overlapping loop required; slope arrow tail level and height offset define slope; default type Generic 12 in / 300 mm.
- Ch3/Ceiling: Ceiling created in a floor plan triggers a not-visible warning; automatic ceiling highlights the room boundary; Height Offset From Level sets the elevation (e.g. 8'6" = 2590 mm in the tutorial).
- Ch3/Room: Room placed only in plan; tag types Room Tag / With Area / With Volume; Upper Limit and Offset in Options Bar (default 10 ft / 3000 mm); a room in an unclosed area warns; closing the area later updates it.
- Ch3/Room: Volume is empty unless Areas and Volumes computation is turned on; volume uses finish faces.
- Ch3/Room: Room instance parameters: Level, Upper Limit, Limit Offset, Area, Perimeter, Unbounded Height, Volume, Number, Name, Comments, Occupancy, Base/Ceiling Finish, Wall/Floor Finish, Occupant, Phase.
- Ch3/Room: A new room not chosen from the schedule list is numbered after the last number in the schedule.
- Ch3/Room-bounding: Walls, roofs, ceilings, curtain systems, floors, columns and room separation lines bound rooms.
- Ch3/Tutorial: Starting from Systems-Default with Level 1 and 2, a rectangular 80 ft x 40 ft exterior (24000 x 12000 mm) can be sketched by typing the dimensions into temporary dimensions; walls span Level 1 to Level 2 using Wall Centerline.
- Ch3/Tutorial: Renaming Level 1 to "Ground Floor" updates the plan view name when the user accepts the prompt.

---

## 7. Open questions

1. Positioning options for Link Revit: the book names a "Positioning" area but not its options. Needs the [knowledge] list (origin-to-origin, center-to-center, shared coordinates, base point) confirmed against Revit docs.
2. Copy/Monitor: the book's coverage is one paragraph. Detail needed (supported categories, coordination review, change notification, parameter mapping) is [knowledge]; chapter 4 or later may extend it (not in this slice).
3. Where are spaces, zones, HVAC loads, mechanical settings and system classification described? Not in Ch 1-3; the first lines of Ch 4 (7102+) begin "Creating Spaces and Zones and Performing System Analysis". The next slice owner should cover those; this document only gives the room-side prerequisites.
4. Room vs Space relationship in the app: do we model Room (architect, from linked model) and Space (MEP) as separate entities with the IfcSpace export only on spaces? The book says rooms form the basis of spaces but does not describe the exact conversion in this slice.
5. Computation Height of levels: book says it computes area/perimeter/volume of rooms; not whether spaces also use it. [knowledge: spaces use it as the measurement height].
6. Level "Plenum" type: listed without definition in the book (an architectural level type that marks plenum height). Behaviour with spaces/ceiling plenums is Chapter 4+ material.
7. "Related to Mass", "Phase Created/Demolished": phasing and massing are not covered in this slice; decide whether to defer.
8. The Wall Opening tool is described under both "Opening panel of the Architecture tab" and "Opening panel in the MEP tab"; confirm it is only on the Architecture tab (the MEP tab is a book slip).
9. Door "swing angle" default and hand/facing conventions are not specified numerically; the flip behaviour is described only visually.
10. Unit tables: the dialog also contains rows (Structural, Infrastructure) whose full contents are not enumerated here.
11. The exact options of the Floor "Slope Arrow" Specify setting ("Height at tail" vs "Slope") are not listed; the book says only "specification method".
12. Whether an MVP needs the Revit-style Options dialog at all (hardware, SteeringWheels, ViewCube, rendering paths): likely no; the app's equivalents are preferences.
13. Shared parameter file format ([knowledge]: text file with GROUP and PARAM rows with GUIDs) is not in the book; decide on a representation for custom properties (IFC property sets are the closest).
14. Confirm the equivalence between "Building Story" level flag and IfcBuildingStorey emission, and how non-story levels should serialise.
