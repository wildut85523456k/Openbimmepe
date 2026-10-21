# Revit MEP reference, part 2: Chapters 4-6

Source: "Exploring Autodesk Revit 2024 for MEP" (CADCIM, Tickoo). Text extract `mep.txt`, lines 7102-12820.

| Chapter | Title in book | Lines |
|---|---|---|
| 4 | Creating Spaces and Zones and Performing System Analysis | 7102-9295 |
| 5 | Creating an HVAC System | 9296-10727 |
| 6 | Creating an Electrical System | 10728-12819 |

Tags: **[book]** = stated in the book (paraphrased). **[knowledge]** = general Revit/MEP/IFC knowledge added by the author of this document; verify before treating as normative.

## 0. Scope warning (read first)

The assigned slice was expected to hold detailed duct content. It does not. What the book actually gives:

- Ch 4: spaces, zones, colour schemes, analytical systems (EnergyPlus). Detailed, tutorial-backed.
- Ch 5: HVAC system creation is a conceptual overview plus three short tutorials. Duct placement is covered at the options-bar / Properties level only. **Duct fittings, routing preferences, segments/sizes, insulation/lining, friction methods and the Duct Sizing dialog fields are NOT described in detail** (Ch 5 only names the four sizing methods and shows a dialog in a figure). Fabrication parts are covered briefly.
- Ch 6 is **electrical** (equipment, devices, lighting, circuits, wires, electrical settings, lighting analysis), not ducts. Duct/pipe routing preferences, insulation and pipe chapters live elsewhere in the book (outside this slice).

Where an implementer needs duct detail beyond the book, sections below add [knowledge] material clearly marked. Chapter 6 is covered in a compact form because it matters for IFC4 electrical classes and for the system/circuit model.

---

## 1. Tutorial spine (build order from scratch and dependencies)

### 1.1 HVAC chain (Ch 4 then Ch 5) [book]

1. **Project from MEP template** (Systems-Default; imperial or metric). Mechanical views: Floor Plans "1-Mech", Ceiling Plans "1 - Ceiling Mech" (Ch 4 Tut 1; Ch 5 Tut 1).
2. **Link architectural model** into the MEP project (Insert > Link Revit), in the 1-Mech plan. Set link type property **Room Bounding = on** so walls/floors of the link bound spaces. Roof should be room-bounding in the link; ceilings should not be (Ch 4, "Creating Spaces"; Tut 1 note). Path type Relative recommended.
3. **Plenum level** (optional but recommended): add a level type "Level Plenum" ~8'6" above Level 1 (via Level tool, type the offset, uncheck structural/ceiling plan view types, keep floor plan), rename view "1-Mech-Plenum", remove view template, set View Range so top = level above, bottom = associated level, cut plane offset 1' (Ch 4 Tut 1). Spaces must exist in occupied AND unoccupied volumes (plenum, chases) for correct loads.
4. **Place spaces** (Analyze > Spaces & Zones > Space) with Upper Limit / Offset in options bar; add **Space Separators** where there is no wall; place spaces in separated areas and chases (chase may need Upper Limit = Level 2 with offset).
5. **Name/number spaces** (edit in tag, or Space Naming tool to copy from linked rooms).
6. **Enable space visibility** (VG > Model Categories > Spaces > Interior/Reference/Color Fill), optionally colour schemes + legend.
7. **Area and Volume Computations**: Areas and Volumes on, "At wall finish" for room area (Ch 4 Tut 3).
8. **Zones**: Zone tool, click spaces; all spaces start in the single "Default" zone; assign to named zones. Check in System Browser (F9, View: Zones).
9. **Analysis set-up (optional chain)**: Energy Settings (Mode: Use Rooms or Spaces; Building Type) and Location; Analytical Systems (water loops, air systems, zone equipment); System-Zones sketched; Create Energy Model; Systems Analysis workflow run; reports in Project Browser (Ch 4 Tut 3-4).
10. **Load air terminal families** (Insert > Load Family > Mechanical > MEP > Air Side Components > Air Terminals); adjust the ceiling-plan **View Range** so hosts are visible (Ch 5 Tut 1: top = level + 8'4", cut plane offset 0, view depth same).
11. **Place air terminals** (Systems > HVAC > Air Terminal), hosted types with Place on Face on ceiling; set **Flow** per terminal; copy-with-multiple to repeat; return and exhaust types are different families with different system classification.
12. **Check airflow vs space** with a schedule (Air Terminals schedule with Space fields and a calculated field "actual minus calculated supply airflow", conditional format outside +-20 CFM) and fix flows in the schedule (Ch 5 Tut 2).
13. **Place mechanical equipment** (Systems > Mechanical > Mechanical Equipment), e.g. VAV box, at an elevation, rotate with Space.
14. **Create duct system**: multi-select terminals + equipment, Create Systems > Duct, choose system type and name, pick the equipment connector to be the system source when prompted (Ch 5 Tut 3).
15. **Generate Layout** (Network / Perimeter / Intersection solutions), settings for main/branch duct types and offsets, Next Solution, Finish Layout; or Generate Placeholder; or draw ducts by hand.
16. **Convert placeholders** to real ducts/fittings (Convert Placeholder; named in Ch 5 only as a Layout-panel companion tool) [knowledge on mechanics].
17. **Size ducts** (Duct/Pipe Sizing on a selected duct, one of Friction / Equal Friction / Velocity / Static Regain).
18. **Verify**: System Inspector (flow arrows + flags), Check Duct Systems (disconnect warnings), Duct Legend colour fills.

Dependency summary: link (room-bounding) -> levels (+plenum) -> spaces (+separators) -> zones -> (loads/energy model) -> families loaded -> terminals (need host or level+offset) -> equipment -> **system** (needs terminals and a source equipment connector) -> layout (needs a system and visible plan or a 3D/section selection) -> ducts/fittings -> sizing (needs flow on terminals) -> inspect/check.

### 1.2 Electrical chain (Ch 6) [book]

Electrical Settings (wiring types, voltage definitions, distribution systems, load classifications/demand factors) -> load families -> required-illuminance project parameter on Spaces + key schedule -> assign keys to spaces -> lighting analysis schedule (calculated delta) -> place lighting fixtures (hosted on ceiling) -> switches and receptacles (hosted on wall faces) -> enable "run calculations for loads in spaces" -> consumption schedule. Power distribution: transformer -> panels with distribution system -> circuits (Power) -> Select Panel -> wires.

---

## 2. Spaces (Analyze > Spaces & Zones)

### 2.1 Space tool

- **Where**: Analyze tab > Spaces & Zones panel > Space (contextual tab "Modify | Place Space"). [book]
- **Preconditions**: A bounded region exists (walls/floors/roofs/ceilings in the host or linked model with Room Bounding on, or space separation lines). With the link's Room Bounding off, placing a space gives a warning that the space is not enclosed and loads cannot be computed. [book]
- **Interaction**: Activate a plan; choose Space; hover a region (red boundary with cross lines highlights); click to place; Esc/Modify to exit. A tag follows placement by default (Tag on Placement toggle). Space name default "Space", number sequential (1, 2...). [book]
- **Contextual tools**: Tag on Placement; **Highlight Boundaries** (highlights bounding elements in the view and pops a message listing space warnings; Expand shows details; Close restores). [book]
- **Options bar**: **Upper Limit** (level the space extends up to); **Offset** (distance beyond that limit); orientation of tag (Horizontal/Vertical); Leader check box; **Space** selector (New / existing unplaced space). [book] Tutorial uses Upper Limit = plenum level for ordinary rooms; chase with Upper Limit = next level plus offset. [book]
- **Type selector**: space tag types (e.g. tag with volume). Space tags only show what the tag type's parameters allow (volume, number, area). [book]
- **Hidden by default**: spaces do not draw in a view until VG > Model Categories > Spaces is enabled; sub-items Interior, Reference, Color Fill. [book]

### 2.2 Space instance properties [book]

Constraints: Level, Upper Limit, Limit Offset, Base Offset (depth below Level; used to bound spaces with no floor).

Electrical-Lighting: Average Estimated Illumination (read-only, computed from fixtures), Room Cavity Ratio (formula given: 2.5 x cavity height x perimeter / area), Lighting Calculation Workplane (default 2'6"/762 mm), Lighting Calculation Luminaire Plane (read-only, from fixtures), Ceiling/Wall/Floor Reflectance (defaults 75/50/20 percent).

Electrical Loads: Design HVAC Load per area, Design Other Load per area.

Mechanical-Flow: Specified Supply Airflow, Calculated Supply Airflow, **Actual Supply Airflow** (read-only; sum of flows of supply terminals in the space), Specified Exhaust Airflow, Return Airflow (method: Specified / Specified Supply Airflow / Calculated Supply Airflow / Actual Supply Airflow), Specified Return Airflow, **Actual Return Airflow** (sum of return terminals), Outdoor Airflow (read-only).

Dimensions: Area (from bounding elements at finish face), Perimeter, Unbounded Height (from Level, Upper Limit, Limit Offset, Base Offset), Volume (shown only when volume computation is enabled).

Identity: Number (duplicates warn), Name, Room Number/Name (of associated room), Comments, Occupant.

Energy Analysis: Occupiable (clear for plenums, shafts, chases), Plenum (check box), Zone (read-only name), Condition Type (Heated, Cooled, Heated and cooled, Unconditioned, Naturally vented, Vented), Space Type (links to Space Type Settings), Construction Type (dialog), People (dialog: occupancy and heat gain), Electrical Loads (dialog: lighting/power), Outdoor Air Information (from space type or zone), Outdoor Air per Person, Outdoor Air per Area, Air Changes per Hour, Outdoor Air Method (by People and by Area; by ACH; Max(People, Area); Max(ACH, People, Area); Max(ACH, Area, People)), Calculated Heating/Cooling Load ("Not Computed" before analysis), Design Heating/Cooling Load.

IFC Parameters: Export to IFC, Export to IFC As, IFC Predefined Type.

### 2.3 Space modification

- **Space Separator** (Analyze > Spaces & Zones > Space Separator; tab "Modify | Place Space Separation"): sketch lines with the Draw panel (Line, Chain option on, Offset 0). Regions must be closed. Drawing inside an existing space reshapes it. Separation lines are MEP-specific; the Architecture side of Revit does not recognise them. Entrance zones that are only semi-bounded need separators before a space can be placed. Tutorial: start at a boundary endpoint (Endpoint snap), move vertically, type the length, Enter, then Modify to exit. [book]
- **Move**: hover to highlight, press-and-drag. [book]
- **Unplace**: select the space in plan and Delete (or Ctrl+X). Space is removed from views but stays in the project; schedules show "Not Placed"; re-placement possible with the options-bar Space selector. [book]
- **Delete permanently**: open the space schedule, select rows, right-click > Delete Row; tags are removed with them. [book]
- **Area and Volume Computations** (Spaces & Zones panel): choose Areas and Volumes to enable Volume parameter; room area computed at wall finish. Volume uses the finish faces and the vertical limits (Base Offset limits spaces without floors). A tag type "Space Tag With Volume" shows it. [book]
- **Space Naming** (Spaces & Zones panel): dialog with Names and Numbers / Names Only / Numbers Only; All Levels or Specific Levels; copies from linked architectural rooms to MEP spaces. [book]

### 2.4 Colour schemes [book]

- Analyze > Spaces & Zones > Color Schemes (or the Color Scheme parameter on the view). Edit Color Scheme dialog: Category (Spaces/Rooms/Areas), Schemes list (Duplicate then name), Title, Color (parameter), By value / By range, Edit Format for range units (Use project settings off), table columns At Least, Less Than, Caption, Visible, Color, Fill Pattern, Preview, In Use; Add Value, Move Rows Up/Down (by value only). Option to apply to rooms/areas of linked files.
- Apply: view Properties > Color Scheme; **Color Scheme Location** Background (floor only) or Foreground (all but columns).
- Legend: Annotate > Color Fill > Color Fill Legend; choose scheme in the type selector; click to place; drag handle to reflow columns; Edit Type for text/swatches; Show Title. Legend is an annotation, affected by annotation crop. Edit through Modify Color Fill Legends > Scheme > Edit Scheme.

---

## 3. Zones (Analyze > Spaces & Zones > Zone)

- **Where**: Analyze > Spaces & Zones > Zone; contextual "Edit Zone" tab; view fades; cursor shows a plus icon. [book]
- **Model rules**: Every project has one **Default** zone and every space starts in it. Zones group spaces with the same environmental needs. A zone can include occupied and unoccupied spaces and spaces from several levels (zone itself has a Level property). An **unbounded zone** (no spaces) can be created and dragged; once a space is assigned the zone becomes bounded and cannot be moved. Zone does not snap to bounded areas. [book]
- **Interaction**: Zone tool > Add Space tool (default in Mode panel) > click each space (it fades when added) > Remove Space tool (minus cursor) to take one out > Finish Editing Zone. Re-edit by selecting the zone in the System Browser (Zones view) then Modify | HVAC Zones > Edit Zone. Rename via the zone Name property. [book]
- **System Browser**: F9 or View > Windows > User Interface > System Browser; View drop-down Zones shows tree Zone > spaces. [book]
- **Zone properties [book]**: Level; Mechanical Flow: Calculated Supply Airflow (read-only), Calculated Supply Airflow per area; Dimensions (all read-only sums): Occupied Area, Gross Area, Occupied Volume, Gross Volume, Perimeter (shared edges between adjacent spaces counted once; disabled for Default zone); Identity: Comments, Name; Phasing: Phase; Energy Analysis: Service Type, Coil Bypass, Cooling Information / Heating Information / Outdoor Air Information (dialogs), Calculated Heating Load, Calculated Heating Load per area, Calculated Area per Heating Load, and the cooling equivalents.
- **Visibility**: VG > Model Categories > HVAC Zones > Interior Fill / Reference Lines. [book]

---

## 4. Analytical system analysis (Analyze > Energy Optimization) [book]

Purpose: define the thermal and HVAC topology without modelling it physically, then run EnergyPlus.

1. **Model basis**: building elements, massing, or both.
2. **Energy Settings**: Mode (Use Rooms or Spaces), Other Options > Advanced Energy Settings (Building Type etc.). **Location**: Define Location by (Default City List etc.) + weather station.
3. **System Browser > Analytical Systems** (view selector). Buttons: Add Water Loop, Add Air System, Add Zone Equipment.
   - Water loop: Name; Loop Type = Chilled Water (variable-speed pump, chiller air or water cooled), Condenser Water (cooling tower serving a water-cooled chiller), Hot Water (boiler + variable-speed pump), Variable Refrigerant Flow (VRF condenser). Tutorial also sets chiller type.
   - Air system: Name; Heat Exchanger, Preheat Coil, Cooling Coil, Heating Coil, Fan options; water loops assigned to it appear as children.
   - Zone equipment: Name, Equipment Type (e.g. Chilled Beam Active), Behavior (e.g. Group Space on Equipment).
4. **System-Zone** tool (Analyze > Energy Optimization): contextual "Create System-Zone"; Properties: Level, Level Offset, Name, Zone Equipment; sketch closed boundary with Draw tools; Finish Edit Mode; Esc to end the tool. System zones tie building areas to analytical systems and zone equipment.
5. **Create Energy Model** (Energy Optimization panel): choose "Create the Energy Analytical Model"; Analytical Spaces tab; auto-assigns analytical spaces to zone equipment; generates Analytical Spaces and Analytical Surfaces schedules.
6. **Systems Analysis** (Analysis tool): choose workflow **HVAC Systems Loads and Sizing** (default; heating/cooling loads) or **Annual Building Energy Simulation**; Report name; Reports Folder Path in Energy Settings; Run Analysis; reports under Reports > Analysis Reports. Components are auto-sized by EnergyPlus; default efficiencies are high-performance typicals, overridable.

Outputs: calculated heating/cooling load per space/zone, calculated supply airflow, load summary and psychrometric report. These feed Space/Zone parameters above.

---

## 5. Air terminals (Systems > HVAC > Air Terminal)

- **Where**: Systems > HVAC panel > Air Terminal; contextual "Modify | Place Air Terminal". [book]
- **Families**: Insert > Load Family > Mechanical > MEP > Air Side Components > Air Terminals. Classification by function is in the family (supply, return, exhaust); book names supply diffuser, return diffuser, exhaust diffuser hosted families. Default type is a 24x24 supply diffuser with a 12x12 connection (metric 600x600/300x300). [book]
- **Taxonomy** [book]: grille (static, intake or exhaust, no adjustable direction), register (grille with adjustable damper, supply only), diffuser (multi-direction pattern, typical in ceilings, square/round, eyeball types).
- **Hosting**: hosted types need a ceiling plan (Views > Mechanical > HVAC > Ceiling Plans); non-hosted use Level + Offset. Equipment and non-hosted items go in floor plans. [book]
- **Interaction (hosted)**: activate ceiling plan; Air Terminal; pick a hosted type; choose **Place on Face** in the Placement panel; hover the ceiling grid for preview; click. Then set Flow. Level/offset come from the host. Drag the centre grip to nudge into a tile. [book]
- **Options bar**: **Flow** value (after placement/selection). Tag on Placement in Tag panel. [book]
- **Instance properties [book]**: Constraints Level, Offset (non-hosted); Mechanical: UpArrow, RightArrow, LeftArrow, DownArrow check boxes (airflow direction arrows for 2-way/3-way/4-way blow); Mechanical-Flow: Pressure Drop, Flow; System Type/System Name/Classification come from connection to a system. Tutorial flows: 300 CFM (150 L/s) supply and return, 250 CFM (120 L/s) exhaust, lowered to 150 CFM (70.8 L/s) in a restroom after schedule check.
- **Copy practice**: copy existing terminals so they inherit type, elevation, flow (Ch 5 "Recommended Practices"). Use Copy with the Multiple option and snap to midpoint, entering distance (12'). [book]
- **Tags**: Tag All (Annotate) > Air Terminal Tags shows Flow. [book]
- **Schedule use**: Air Terminals schedule fields Flow, Mark, System Type, plus Space fields (Number, Name, Calculated Supply Airflow, Actual Supply Airflow) and a calculated field (HVAC discipline, Air Flow type) = actual minus calculated; Conditional Format "Not Between" -20 and +20 CFM (-9.5/+9.5 L/s) with yellow background; sort by space number; itemize every instance; editing Flow directly in the schedule updates the model (WT tiles plan + schedule). [book]

## 6. Mechanical equipment and control devices

- **Where**: Systems > Mechanical > Mechanical Equipment (VAV, FCU, AHU) and Mechanical Control Device (thermostats, humidistats, sensors; families in Information and Communication > Building Controls). Load Family button on the contextual tab. [book]
- **Family folders**: Air Side Components > Air Handling Units and Terminal Units (VAV Unit - Single Duct etc.). [book]
- **Interaction**: floor plan; tool; pick type; set properties; click to place; **Space bar rotates** (tutorial pressed it twice) or tick Rotate after Placement. Default placement is at finished floor elevation. [book]
- **Properties [book]**: Constraints Level, Offset (tutorial uses "Elevation from Level" 1'6"); Mechanical Pressure Drop (default 0.04 in-wg / 10 Pa on VAV); Mechanical-Flow Supply Air Flow (default 500 CFM / 236 L/s). Properties vary with the family.
- **Connectors**: equipment families carry several connectors (e.g. VAV has supply and return) defined in the family. When making a system from a multi-connector unit, Revit asks which connector to use ("Select Connector" list shows connector number, system classification, shape/size, and **flow direction In/Out**). Tutorial picks the Supply Air, rectangular, Flow Direction Out connector for a supply system. [book]
- **Practices [book]**: copy equipment to inherit properties; schedule space info for design even if not issued; conditional formatting to expose design problems; edit equipment properties in schedules.

---

## 7. HVAC systems (Create Systems > Duct)

- **Where**: select terminals and equipment in plan (Modify | Multi-Select) or select terminals only (Modify | Air Terminals) > **Create Systems panel > Duct**. Dialog "Create Duct System": **System type** (Supply Air, Return Air, Exhaust Air) and **System name**. OK, then the connector-selection dialog if the equipment has several connectors. A blue dotted rectangle encloses the system; contextual tab "Modify | Duct Systems". [book]
- **Edit System** (System Tools panel > Edit System; tab "Edit Duct System"): add/remove components; options bar **System Equipment** drop-down selects the source equipment; Finish Editing System in Mode panel. [book]
- **System Browser**: Mechanical > Systems discipline lists the systems. [book]
- **Rules [book]**: Return terminals get a separate return system, created the same way; the system connects terminals plus equipment; layout later considers only members in the visible plan (see section 8).
- **Rules [knowledge]**: Each connector carries a System Classification (SupplyAir, ReturnAir, ExhaustAir, OtherAir, Fitting, etc.) that restricts which system type it can join; a duct system is a connected set of connectors sharing a classification and a single base (source) equipment; Revit also generates a physical "system" for ducts that are merely drawn from connectors. Classification propagates through fittings. Flow direction (In/Out/Bidirectional) is defined per connector; a supply terminal connector is In, a supply-side equipment connector is Out. Flow is accumulated from terminals toward the base; a duct flow value is the sum of downstream terminal flows.
- **Inspect and verify [book]**: select a duct, Analysis panel > System Inspector > Inspect: arrows in the ducts turn red and hovering shows flow info for that section. Analyze > Check Systems panel > **Check Duct Systems** reports disconnects as warnings. Analyze > Color Fill > **Duct Legend**: click in view, choose Color Scheme (e.g. by flow) in Choose Color Scheme dialog; ducts get colour fills and a legend.
- **Outputs**: system name, type, member list, per-duct flow, velocity, pressure drop, total system pressure loss [knowledge; read-only duct parameters: Flow, Velocity, Friction, Loss Coefficient, Pressure Drop].

---

## 8. Generate Layout and Generate Placeholder

- **Where**: select any terminal (or select a system member) > Modify tab **Layout panel > Generate Layout** (also Generate Placeholder in the same panel; Modify | Duct Systems also offers it). Tab "Generate Layout". [book]
- **Preconditions [book]**: the components must belong to a duct system (Create Systems > Duct first; pick a System Equipment in Edit System). Only components of the system **visible in the current plan view** are routed; selecting in a 3D, section or elevation view includes all system components even across levels.
- **Options bar [book]**: **Solution Type** (Network, Perimeter, Intersection); **Next/Previous Solution** arrows (a "solution n of m" counter; tutorial pressed the arrow four times to reach 5 of 5); **Settings** button opens **Duct Conversion Settings**; Finish Layout (Layout panel) commits; Esc/Cancel discards.
- **Solution types [book]**: Network = up to 6 solutions, a main with 90-degree branches. Perimeter = 5 solutions wrapping the main around the perimeter of the components. Intersection = up to 8 solutions, main run across component connectors with perpendicular drops to each. Preview colours: **blue = main**, **green = branches**.
- **Duct Conversion Settings dialog [book]**: left pane Main / Branch. Main: Duct Type, Offset. Branch: Duct Type, Offset, **Flex Duct Type**, **Maximum Flex Duct Length**. Tutorial used "Rectangular Duct: Mitered Elbows/Taps" for both, offset 2'6" for main and branch.
- **Finish**: Finish Layout creates actual duct (or placeholder) with fittings per the routing preferences of the chosen duct types. Layout lines can be edited manually before finishing (book says paths can be manually modified).
- **Generate Placeholder [book/knowledge]**: produces lightweight duct placeholders (centre-line objects) that are later converted. Book names the tool and the Duct Placeholder tool (HVAC panel) but does not detail Convert Placeholder; see [knowledge] below.
- **[knowledge]** Layout engine inputs: connector positions and directions, system base, ceilings/obstacles are ignored (no clash avoidance). After Finish, branch connections to terminals shorter than the flex-length limit become flex duct; longer runs become rigid duct plus a flex stub. Placeholders convert via Modify > Convert Placeholder (Convert panel, appears with placeholder selection); conversion applies the type's routing preferences, inserts fittings and may resize to the connector.

---

## 9. Duct placement (Systems > HVAC > Duct, Duct Placeholder)

- **Where**: Systems > HVAC panel > Duct (contextual "Modify | Place Duct"); shortcut DT [knowledge]. Duct Placeholder in the same panel. [book]
- **Placement Tools panel [book]**: **Automatically Connect** (on by default: start/end snap to connectors and connect); **Inherit Elevation** (take elevation from the snapped component); **Inherit Size** (take size from the snapped component); Tag on Placement (Tag panel). *(The book's text names Inherit Elevation twice; the second clearly means Inherit Size.)*
- **Options bar [book]**: **Width**, **Height** (defaults 12"/300 mm; rectangular), **Offset** (default 9'/2750 mm), **Lock/Unlock** specified elevation next to Offset, **Apply**. (Round ducts show Diameter instead [knowledge].)
- **Properties [book]**: Type Selector (duct type); Constraints: **Horizontal Justification** (Center default, Left, Right), **Vertical Justification**, **Reference Level**, **Middle Elevation** (offset of duct centre from reference level); Mechanical: **System Type** (Supply Air default; Exhaust Air; Return Air), **Size Lock** check box.
- **Interaction [book]**: pick type; set width/height/offset; click start in plan or snap to an equipment connector (snap marker appears); move; preview shows cross-section and justification; click for end; keep clicking for further segments; Modify to exit. Segments are connected with fittings automatically (no join or trim step).
- **[knowledge] Placement behaviours**: Change of direction inserts elbow per routing preference; picking a start on a duct inserts a tee/tap; ends at a perpendicular duct create tee/tap; typing a number during drawing sets length; Space bar flips justification during drawing; Enter/Esc finish; changing offset between segments inserts transitions/vertical elbows with a slope option (Slope drop-down on options bar when slope placement tools are active); Justify tool (Modify > Justify) re-aligns after the fact; Size Lock prevents later resizing from sizing tools.
- **Type Properties [knowledge]**: Routing Preferences (Elbow, Junction, Cross, Transition, Union, Multi Shape Transition Oval to Round, Multi Shape Transition Rectangular to Round, Tap Adjustment, Flex duct); each lists preferred fitting families, optional minimum/maximum size conditions and priority order; Segment/material (e.g. Galvanized); Roughness; Fitting loss method per fitting; Insulation and Lining are instance-level add-ons (Add Insulation / Add Lining on the duct contextual tab; insulation type + thickness; lining type + thickness); Duct size lists in Mechanical Settings.
- **Mechanical Settings [knowledge]**: Duct Settings: Duct Sizing (method, velocity limit, friction limit), Rectangular/Oval/Round size tables (sizes list with Use for Sizing / Use in Size Lists), Angles (fitting angle snaps), Conversion (Duct Conversion Settings defaults), Calculation (fluid density, viscosity, friction method).

## 10. Duct sizing (Analysis panel > Duct/Pipe Sizing)

- **Where**: select a duct in a system > Modify | Ducts > Analysis panel > **Duct/Pipe Sizing**; dialog "Duct Sizing". [book]
- **Methods named [book]**: **Friction**, **Equal Friction**, **Velocity**, **Static Regain**. Book notes that constant friction loss is the most common method, velocity reduction and static regain being alternatives.
- **Behaviour [book]**: choose the method, OK; the selected duct (and its system run) is resized to the airflow carried. Sizing relies on terminal flows already being set.
- **Dialog fields [knowledge]**: Sizing Method list; Constraints (Lock Height, Lock Width, Limit Velocity, Limit Friction) combined with the chosen method; Duct Size dropdown in options; Apply to Selected Duct / Main / Branch (Selected, Selected+Downstream etc.). Friction = target pressure drop per length (default about 0.1 in-wg/100 ft); Equal Friction = sizes the whole branch at a constant friction for the main; Velocity = size to keep a velocity; Static Regain = pressure recovery at junctions (set by velocity/regain coefficient).
- **Size Lock [book]**: duct instance Size Lock prevents the sizing tool from changing that duct.
- **Calculated duct parameters [knowledge]**: Flow, Velocity, Velocity Pressure, Friction, Loss Coefficient, Pressure Drop, Equivalent Diameter, Hydraulic Diameter, Area, Perimeter, Length, Section.

## 11. Fabrication parts (Systems > Fabrication)

[book] condensed.
- Concepts: Fabrication Configuration (content + data; network or local; per model; e.g. "Revit MEP Imperial Content V2.2"), Fabrication Profile (subset of services; **Global** = all), Services (loaded vs unloaded), additional Parts.
- **Fabrication Settings** (Fabrication panel button, shortcut FS): tabs "Fabrication configuration" (Configuration list, Profile list, Services tab with Unloaded/Loaded + Add; Parts tab) and "Connection Indicators" (colours for Plan, Towards, Away arrows, used by Route and Fill).
- **Fabrication Part** (shortcut PB): MEP Fabrication Parts palette: Service list (loaded only), Group list (e.g. Rect, Hangers), Services tab part list (Straight, Radius Elbow, Straight Cut 2xL, Full Strap hanger...). Click part, click to place; to attach a new part, hover the free end of the placed part until the **magenta** connection control shows, then click. Hangers placed on a duct by hovering and clicking.
- Workflows: place a part then detail, or convert generic Revit duct/pipe/containment to fabrication parts.
- Ductwork service example: "Ductwork+2in WG".

---

## 12. Chapter 6 electrical tools (compact)

### 12.1 Electrical equipment
- **Where**: Systems > Electrical > Electrical Equipment (contextual "Modify | Place Equipment"); transformers, switchboards, panels. [book]
- Families: Electrical > MEP > Electric Power > Generation and Transformation (dry type NEMA 2, 3R hosted, wet type) and Distribution (switchboards: Circuit Breaker, Metering, Utility, Transformer; panelboards e.g. Lighting and Appliance 480V MCB). [book]
- **Placement tools**: Place on Vertical Face (default for panels), Place on Face, Place on Work Plane; Space bar flips side or rotates; Rotate after placement; Tag on Placement (tag family loaded on prompt); Leader. [book]
- **Instance props [book]**: Level, Host, Offset from Host, Elevation from Level, Secondary Distribution, Mounting, Panel Name, Max #1 Pole Breakers, Feed, Short Circuit Rating, Distribution System, plus schedule notes, Mains, Circuit Naming.
- **Type props [book]**: Primary Voltage, Primary Number of Poles, Load Classifications, Voltage, Wattage, dimensions; panels: Default Elevation, Load Classification (dialog), voltage, wattage, size.
- **Distribution example [book]**: Transformer: distribution system 480/277 on the options bar, Secondary Distribution 120/208 Wye, Panel Name T1. Panels H1 (480 V MLO, 480/277 Wye) and L1 (208 V MCB, 120/208 Wye). Select L1 > Create Systems > Power > Select Panel tool > pick T1 from Panel list; right-click transformer connector > Create Power Circuit > Select Panel > pick H1. The panel list only offers panels whose distribution system matches the connector. Electrical Analysis panel (Systems) adds Area Based Load Boundary, Area Based Load, Equipment Load for preliminary load takeoffs.

### 12.2 Devices and fixtures
- Tools under Systems > Electrical > Device drop-down: Electrical Fixture (receptacles), Lighting (switches, sensors), Communication, plus others (telephone, data, nurse call, fire alarm, security). Mostly **hosted**; default placement mode Place on Vertical Face; Space bar flips. Tag on Placement; Tags button opens Loaded Tags. [book]
- Instance: Host (read-only, "<not associated>" until placed), Elevation, Offset, Schedule Level; lighting devices: Switch Voltage, Switch ID. Type: Default Elevation, Load, Switch Voltage, Load Classification, Apparent Load. [book]
- **Lighting Fixture** (Electrical panel): placed from ceiling plans (Electrical > Lighting > Ceiling Plans) using Place on Face on ceiling; families under Lighting > MEP > Internal/External. Type props: Default Elevation, Light Box Material, Diffuser Material, Load Classification, Lamp (T8, A-19, A-21), Ballast Voltage, Ballast Number of Poles, Apparent Load (VA = apparent current x voltage), Tilt Angle, Light Loss Factor, Initial Intensity, Photometric Web File (IES), Color Filter. [book]

### 12.3 Electrical Settings (Systems > Electrical > Electrical Settings)
[book] Nodes: Wiring (Ambient Temperature default 86 F/30 C so correction factor = 1; Gap of Wiring Crossing; Hot/Ground/Neutral Wire Tick Mark styles; Slanted Line across Tick Marks; Show Tick Marks; Max Voltage Drop for Branch and Feeder sizing), Wire Sizes > Correction Factor (per material and temperature), Wire Sizes > Ground Conductors (ampacity vs size), Wiring Types (Name, Material, Temperature Rating, Insulation, Max Size, Neutral Multiplier, Neutral Required, Conduit Type, Neutral Size), Voltage Definitions (Name, Value, Min, Max), Distribution Systems (Name, Phase, Configuration, Wires, L-L Voltage, L-G Voltage), cable tray and conduit settings, Load Calculations (Load Classifications dialog with Demand Factor and Demand Factors dialog: calculation methods such as By load with incremental ranges; "load class for use with spaces"; option **Run calculations for loads in space**), Panel Schedules (Spare Label, Include Spares in Panel Totals, Merge multi-poled circuits into a single cell).

### 12.4 Circuits and wires
- **Circuit**: select devices/fixtures > Create Systems > Power (or other type) > "Modify | Electrical Circuits" > Edit Circuit (Add to Circuit) > Finish Editing Circuit; circuit is unnamed until a panel is chosen with **Select Panel** (options bar Panel list); then it takes a circuit number. Circuit types follow connectors: Power-Balanced, Power-Unbalanced, Communication, Controls, Fire Alarm, Telephone, Security. **Circuits are the real connection; wires are only their symbolic drawing.** [book]
- **Wire tools**: Systems > Electrical > Wire drop-down: Arc Wire, Spline Wire, Chamfered Wire. Interaction: start point, middle (arc) point, end point; wire type in Type Selector; Tag on Placement. [book]

### 12.5 Lighting analysis workflow [book]
Project Parameters: new project parameter "Required Illumination Level" (Spaces; Electrical discipline; Illuminance type) -> Key schedule on Spaces with key name "Illumination Levels (fc)" and rows per space type (examples: private office 30 fc, open office 45, conference 35, lounge 25, circulation 20) -> assign key per space via the key parameter -> Spaces schedule "Lighting Analysis" with Average Estimated Illumination, Required Illumination, Ceiling/Wall/Floor reflectance, Room Cavity Ratio and a calculated delta, conditional-formatted (cell highlighted when deficit exceeds 6 fc) -> add fixtures until no highlighted cells. Consumption report: enable Run calculations for loads in space, then a Spaces schedule with Actual Lighting Load, Actual Power Load and per-area versions.

---

## 13. IFC4 mapping notes

Mostly [knowledge]; book only states that spaces carry Export to IFC / As / Predefined Type parameters.

| Revit concept | IFC4 target | Notes |
|---|---|---|
| Space | IfcSpace (PredefinedType e.g. INTERNAL), `Pset_SpaceCommon`, `Pset_SpaceThermalRequirements` | Space heights from Upper Limit/Base Offset; Net/Gross area in Qto_SpaceBaseQuantities |
| Zone | IfcZone (member of IfcSystem-like grouping), `Pset_ZoneCommon` | Group of IfcSpace via IfcRelAssignsToGroup |
| Duct segment | IfcDuctSegment (RIGIDSEGMENT / FLEXIBLESEGMENT) | Flex ducts to FLEXIBLESEGMENT |
| Duct fitting | IfcDuctFitting (BEND, TEE, TRANSITION, JUNCTION...) | Fitting classification from routing preference |
| Air terminal | IfcAirTerminal (DIFFUSER, GRILLE, REGISTER...) | Flow in Pset_AirTerminalTypeCommon |
| VAV / AHU / FCU | IfcAirTerminalBox (VAV), IfcUnitaryEquipment (AHU packaged), IfcFan, IfcCoil | Choice by family category |
| Ports | IfcDistributionPort with FlowDirection SOURCE/SINK/SOURCEANDSINK, SystemType | Revit connector In/Out maps to SINK/SOURCE |
| Duct system | IfcDistributionSystem (PredefinedType VENTILATION / EXHAUST / AIRCONDITIONING) | Membership via IfcRelAssignsToGroup; connectivity via IfcRelConnectsPorts |
| Insulation / lining | Pset_DuctSegmentTypeCommon thermal properties or IfcCovering (INSULATION) | |
| Electrical: panel, transformer, devices, lights | IfcElectricDistributionBoard, IfcTransformer, IfcOutlet/IfcSwitchingDevice, IfcLightFixture | Circuit = IfcDistributionCircuit |
| Wire | symbolic only | not exported as geometry |

---

## 14. Acceptance checklist

Prefix = chapter and section.

### Spaces and zones
- [ ] Ch4 "Creating Spaces": a space cannot compute loads if no enclosing room-bounding elements exist; warn on placement.
- [ ] Ch4 "Creating Spaces": link Room Bounding flag controls whether linked elements bound MEP spaces.
- [ ] Ch4 "Placing Spaces": Upper Limit and Offset define space height; default is invisible until category visibility is enabled.
- [ ] Ch4 "Placing Spaces": Highlight Boundaries lists bounding elements and warnings.
- [ ] Ch4 "Splitting the Space": separation lines may be sketched in empty areas or inside spaces; the space re-fits to the new boundary.
- [ ] Ch4 "Removing the Spaces": delete unplaces (data kept, schedule shows Not Placed); deleting from schedule removes permanently with its tags.
- [ ] Ch4 "Computing the Volume": Volume parameter exists only when volume computation is on; uses finish faces; Base Offset bounds floorless spaces.
- [ ] Ch4 "Renaming and Renumbering": Space Naming copies names/numbers (both, names only, numbers only) for all levels or selected levels.
- [ ] Ch4 "Mechanical-Flow": Actual Supply Airflow equals sum of supply terminal flows in the space; Actual Return Airflow likewise; Return Airflow method selects which value is used.
- [ ] Ch4 "Creating Zones": new spaces enter the Default zone; adding to a zone removes from Default; an unbounded zone is movable, a bounded zone is not.
- [ ] Ch4 "Adding and Modifying HVAC Zones": zone area/volume are sums of member spaces; shared edges count once in Perimeter.
- [ ] Ch4 "Color Schemes": scheme by value or by range; background vs foreground location; legend is an annotation.

### Analysis
- [ ] Ch4 "Performing System Analysis": water loop types are Chilled, Condenser, Hot, VRF; air system has heat exchanger, preheat, cooling, heating, fan options.
- [ ] Ch4 "Adding System-Zones": system zone is a sketched closed boundary with level, offset, name, zone equipment.
- [ ] Ch4 "Creating the Analytical Model": creating the energy model generates analytical spaces and surfaces schedules and auto-assigns zone equipment.
- [ ] Ch4 "Running the System Analysis": two workflows (loads and sizing; annual energy) write reports to a configurable folder.

### Air side
- [ ] Ch5 "Placing Air Terminals": hosted terminals take level/offset from the ceiling host; non-hosted use level+offset.
- [ ] Ch5 "Placing Air Terminals": Flow and Pressure Drop are instance parameters; four arrow flags control blow-direction symbols.
- [ ] Ch5 "Placing Mechanical Equipment": default elevation is floor level; rotation by Space bar; VAV default pressure drop 0.04 in-wg (10 Pa) and supply 500 CFM (236 L/s) as shown.
- [ ] Ch5 "Creating Air Supply System": a system needs a type (Supply/Return/Exhaust) and a name; multi-connector equipment prompts for the connector (with flow direction shown).
- [ ] Ch5 "Generating HVAC System Layouts": only plan-visible members are routed from a plan; 3D/section selection includes all levels.
- [ ] Ch5 "Generating HVAC System Layouts": Network gives up to 6 solutions, Perimeter 5, Intersection up to 8; main drawn blue, branches green.
- [ ] Ch5 "Generating HVAC System Layouts": Conversion Settings carry main duct type + offset and branch duct type + offset + flex type + max flex length.
- [ ] Ch5 "Creating Ducts": Auto Connect, Inherit Elevation, Inherit Size; width/height default 12"/300 mm and offset default 9'/2750 mm in the book's template.
- [ ] Ch5 "Creating Ducts": horizontal justification Left/Center/Right; vertical justification; reference level; middle elevation; System Type default Supply Air; Size Lock.
- [ ] Ch5 "Creating Ducts": fittings insert automatically on connection.
- [ ] Ch5 "Sizing Ductwork": four methods selectable; locked duct not resized [knowledge].
- [ ] Ch5 "Inspecting the Duct System": inspector shows flow of the hovered section.
- [ ] Ch5 "Checking the Duct System": disconnected ducts raise warnings.
- [ ] Ch5 "Creating Duct Legend": colour fills by scheme with legend.
- [ ] Ch5 Tut 2: schedule edit of terminal Flow updates model and conditional format clears when actual vs calculated is within +-20 CFM.
- [ ] Ch5 "Fabrication Details": parts attach at free connection ends; services must be loaded before they appear.

### Electrical
- [ ] Ch6 "Creating Circuits": circuit exists without panel, named only after a panel is assigned; panel list filters by distribution system.
- [ ] Ch6 "Adding Wires": wires are symbols; circuits are the logical connection.
- [ ] Ch6 "Specifying Electrical Settings": ambient 86 F gives correction factor 1; wiring types, voltage and distribution definitions are editable tables.
- [ ] Ch6 "Performing Lighting Analysis": delta between required and estimated illuminance; flag when deficit exceeds 6 fc.

---

## 15. Open questions

1. Duct fitting selection rules (routing preferences, min/max size conditions, preferred junction type tee vs tap) are absent from this slice; locate in Ch 7 or other chapters, otherwise rely on [knowledge].
2. Exact Duct Sizing dialog fields (constraints, which runs are affected) not described in the text; the figure is not in the extract.
3. Convert Placeholder: the book mentions Generate Placeholder and Duct Placeholder but not the conversion command; semantics here are [knowledge].
4. Flow-direction semantics for bidirectional and "OtherAir" connectors and transfer-air terminals are not covered.
5. Whether placeholder ducts carry flow/pressure-loss data before conversion is unknown from the book.
6. The Layout solution count formulas (6/5/8) are upper bounds in the book; ordering rules between solutions are unspecified.
7. Space bounding at plenum/ceiling interplay (the architectural ceiling should not bound) needs a decision for an IFC-based import where IfcCovering(CEILING) bounding behaviour is ambiguous.
8. Behaviour of Default zone assignment when a space is removed from its zone (back to Default?) is only implied.
9. Unit defaults (imperial vs metric) in the template differ; the app needs unit-system-neutral storage (SI internal).
10. Fabrication parts depend on proprietary content (MEP Content V2.2 database); out of scope for an open implementation unless an equivalent is defined.

---

## Appendix A. Tutorial ledgers (exact order of operations, paraphrased)

### A.1 Ch 4 Tut 1: placing spaces (office)
1. New project from Systems-Default template (metric: Systems-Default_Metric).
2. Select 1-Mech plan; Insert > Link Revit; choose architectural file; select the link; Type Properties; tick Room Bounding; OK; Modify.
3. Open East elevation; Architecture > Datum > Level; type selector "Level Plenum"; options bar: Make Plan View on, Plan View Types: keep Floor Plan only; pick Line; hover near end of Level 1, move up, type 8'6" (2591 mm), Enter; move right and click when the extension snap/alignment shows over Level 1's end. New level appears (name "Mechanical 3"). Modify.
4. Open the new plan; rename view to "1-Mech-Plenum" (confirm dialog Yes); remove the view template (Apply View Template: none); View Range: Top = Level Above, Bottom = Associated Level (plenum), cut plane offset 1'; View Depth level = associated level. Return to 1-Mech.
5. Space tool; Tag on Placement on; Upper Limit = plenum level; offset 0; click the CEO office region, ZF (zoom fit), click the central open area. Modify.
6. Click tag text to rename (CEO-Office), double-click number to renumber (101); repeat for Central Area (110).
7. VG: Spaces > Color Fill on, Interior and Reference on.
8. ZR to zoom; Space Separator; Line, Chain on, Offset 0; click an endpoint snap on the space boundary, move straight down, type 26'5" (8052 mm), Enter; Modify. The central space shrinks to the new boundary.
9. Space tool; click the newly bounded area; Modify; select it; in Properties set Number 115 and Name Lounge.
10. Chase: zoom; Space tool; Upper Limit = Level 2, Offset 1'6" (457 mm); click chase; set Name Chases, Number 121.
11. Save As; Close.

### A.2 Ch 4 Tut 2: zones
VG: clear Spaces > Reference; F9 System Browser; View = Zones; expand Default (all spaces listed). Zone tool; Add Space; click eight named spaces; Finish Editing Zone. Zone appears as "1" with its spaces. VG: HVAC Zones > Interior Fill + Reference Lines. Select zone "1" in browser; Edit Zone; open Properties; Name = Office-North-Zone; Finish Editing Zone.

### A.3 Ch 4 Tut 3: system zone analysis
Energy Settings (Mode = Use Rooms or Spaces; Building Type = Office) -> Location (default city list; city; weather station) -> Area and Volume Computations (Areas and Volumes; room area at wall finish) -> F9, Analytical Systems: Add Water Loop "CW1" (Chilled Water, air-cooled chiller), Add Water Loop "HW1" (Hot Water) -> Add Air System "AS1" (Heat Exchanger = Enthalpy; other options per figure) -> Add Zone Equipment "ZE-1" (Chilled Beam Active; Group Space on Equipment) -> System-Zone tool, Line, sketch a closed boundary, Finish Edit Mode -> Create Energy Model -> move spaces to the system zone -> Systems Analysis (HVAC Systems Loads and Sizing) -> read load summary + psychrometric report. Tutorial 4 repeats with Annual Building Energy Simulation.

### A.4 Ch 5 Tut 1: air terminals
Ceiling plan "1 - Ceiling Mech" view range (top: level + 8'4" offset; cut offset 0; depth: level + 8'4"). Air Terminal; Load Family: Supply Diffuser - Hosted; type "Workplane-based Supply Diffuser"; Place on Face; click in ceiling tile; Flow 300 CFM (150 L/s); Modify; drag centre grip to fit a tile; Copy with Multiple: pick midpoint, move down 12' (3658 mm) at 90 degrees, repeat. Load Exhaust Diffuser - Hosted and Return Diffuser - Hosted (multi-select with Ctrl); return type Flow 300 CFM; place two; exhaust Flow 250 CFM; place in the toilet. Adjust arrow parameters for 2-/3-way blow.

### A.5 Ch 5 Tut 2: air terminal schedule
View > Schedules/Quantities; filter Mechanical; category Air Terminals; name; fields: Flow, Mark, System Type; switch field source to Space: Actual Supply Airflow, Calculated Supply Airflow, Name, Number; reorder so Space Number is first; add calculated parameter (HVAC discipline, Air Flow type): actual minus calculated; Formatting > Conditional Format on that field: Not Between -20 and 20 CFM (-9.5 and 9.5 L/s), yellow background; Sorting: Space Number ascending, header/footer count and totals, blank line, no grand totals, itemize every instance; no filter. Then WT to tile plan and schedule; edit a Flow cell (200 to 150 CFM); the cell turns white; Tag All air terminal tags to show Flow.

### A.6 Ch 5 Tut 3: duct system
Plan "1 - Mech-Plenum" view range (top unlimited; cut offset 1'; depth = plenum level, offset 0). Load VAV Unit - Single Duct (Terminal Units folder). Mechanical Equipment; choose a dual-duct 12x12 type; Elevation from Level 1'6"; Space bar twice; click in conference room. Select four terminals + VAV; Create Systems > Duct; name "Mechanical Air Supply System 1"; OK; in Select Connector pick the Supply Air rectangular connector with flow direction Out; Generate Layout; Solution Network, press next 4 times to solution 5 of 5; Settings: both Main and Branch duct type "Rectangular Duct: Mitered Elbows/Taps" with offset 2'6"; OK; Finish Layout; open Default 3D view to confirm.

### A.7 Ch 6 tutorials (summary)
T1: Electrical Settings (new wiring type AL-THHN, aluminium, 75 degrees, THHN, max size 500, neutral multiplier 1.00, neutral required, steel conduit, neutral size = hot conductor size; default voltage definitions and distribution systems; load classification Other with demand factor By load, incremental per range with two additional rows) and load panelboard/receptacle/switch/transformer/troffer families. T2: lighting analysis set-up (parameter, key schedule, key assignment, colour fills, comparison schedule). T3: place fixtures, switches (wall hosted), receptacles (power plan), consumption schedule.

---

## Appendix B. [knowledge] Duct engineering details to implement

These are not in the slice; use as design input, verify against Revit behaviour.

1. **Connector model**: Each MEP element exposes connectors with: position, direction (outward normal), shape (round, rect, oval), size, domain (HVAC), system classification, flow direction (In/Out/Bidirectional), flow configuration (Calculated/Preset/System), flow value, loss method and coefficient. Two connectors join only if domain compatible, classifications compatible, positions coincident and directions opposed.
2. **Duct system** types: SupplyAir, ReturnAir, ExhaustAir (plus OtherAir). A mechanical system is a graph rooted at the base equipment; flow is solved downstream from terminals; each duct section carries the sum of downstream flows. Terminal-less segments inherit zero flow and are flagged by Check Systems.
3. **Fitting auto-insertion**: when two segments meet, choose the elbow family (smooth, mitered, with vanes) from Routing Preferences by angle; T intersections choose tee or tap based on size ratio rule; transitions inserted when sizes differ; elbow angles snap to angle sets (e.g. 15, 22.5, 30, 45, 60, 90 degrees) from Mechanical Settings.
4. **Rectangular sizing**: aspect ratio limit commonly 4:1; Lock Height/Width during sizing; round alternatives from the Round Size list.
5. **Friction (Darcy-Weisbach / Colebrook)** per length; fitting loss = coefficient C x velocity pressure; ASHRAE fitting database coefficients; total system loss = sum of the longest (critical) path.
6. **Insulation/lining**: add-ons to duct; they change outside dimension for clash and the thermal Pset; typical thickness lists in the type.
7. **Slope**: ducts can be drawn with slope (rise/run) for drainage; elbows convert to vertical shapes.
8. **Air terminal behaviour**: flow can be set manually or pulled from the space's specified/calculated supply airflow divided among terminals; pressure-drop parameter per terminal contributes to the system total.
9. **Placeholders**: centre-line only, no fittings; carry system and size; convert to ducts with fittings via routing preferences; fast to move.
10. **Generate Layout search**: Network: candidate main axes aligned to orthogonal directions through connector projections; branches perpendicular to main (tap). Perimeter: main offset around the convex bounding rectangle. Intersection: main along rows/columns where terminal connectors align. Solutions ranked by total length.

## Appendix C. Data-model implications for the Rust app

- `Space` (placed/unplaced flag, level, upper limit level + offset, base offset, derived area/perimeter/volume, airflow fields, energy fields, number/name/room links, zone ref); `SpaceSeparator` (line, level).
- `Zone` (name, level, member spaces, bounded flag derived from membership, energy info blocks); a singleton `Default` zone created with the project; invariant: every space in exactly one zone.
- `AnalyticalSystem` hierarchy: WaterLoop(type), AirSystem(coils, fan, heat exchanger, loops), ZoneEquipment(type, behaviour), SystemZone(sketch, level, offset, equipment ref).
- `AirTerminal` (family type, host face or level+offset, flow, pressure drop, arrows, classification, system ref); `MechanicalEquipment` (connectors, flow, pressure drop).
- `DuctSystem` (type, name, base equipment connector, members) with `LayoutProposal` (solution type, index, main/branch polylines, conversion settings) as a transient edit session cancelled by Esc and committed by Finish.
- `Duct` (type, size, justification h/v, reference level, middle elevation, system type, size lock, insulation, lining, slope) with auto-generated `DuctFitting` records.
- Views: plan range (top/cut/bottom/depth), category/subcategory visibility (spaces, zones), colour schemes, schedules with calculated fields and conditional formats, System Browser views (Zones, Systems, Analytical Systems).
- Warnings registry: unenclosed space, duplicate space number, disconnected duct, duplicate system name, flow mismatch.
