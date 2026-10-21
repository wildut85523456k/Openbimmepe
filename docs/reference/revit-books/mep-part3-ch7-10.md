# Revit MEP reference, part 3: Chapters 7-10 (plumbing, fire protection, electrical, documentation)

Source: "Exploring Autodesk Revit 2024 for MEP" (CADCIM, Tickoo), extracted text `mep.txt`.
Slice: lines 12820-18694.

| Chapter | Title (book) | Text lines |
|---|---|---|
| 7 | Creating Plumbing Systems | 12820-14267 |
| 8 | Creating Fire Protection System | 14268-15136 |
| 9 | Creating Construction Documents (dimensions, text, tags, callouts, drafting views, duplicate views, sheets) | 15137-16812 |
| 10 | Creating Families and Worksharing (massing, family editor, worksets, Worksharing Monitor) plus 3 case studies | 16813-18694 |

Conventions: `[book]` = stated in the book; `[knowledge]` = general Revit/IFC/engineering knowledge not in the book (flagged so the team can verify). Citations are `(Ch N, "section heading")`. All text is paraphrased. Tutorial file names, figure numbers and review questions are omitted. Imperial/metric: the book gives imperial values with metric in parentheses; where only one is given, assume the other is a unit conversion.

Intended consumer: implementers of a Rust, IFC4-based, Revit-style BIM authoring app. "Implementer note" lines are suggestions, not book content.

IMPORTANT SCOPE FINDING: the assigned slice does NOT contain electrical content (panels, circuits, cable tray, conduit, lighting, panel schedules, load classification, demand factors) nor a clash-detection chapter. Chapters 7-10 are plumbing, fire protection, construction documents, and families/worksharing. Electrical is presumably in an earlier or later text range (the book's electrical chapter precedes or follows this slice). The only coordination content here is the case-study outline (Navisworks) and the worksharing chapter. This file therefore covers what is present and flags the electrical gap in "Open questions".

---

# Tutorial spine (build order from scratch, per system)

All orders below are derived from the Ch 7-10 tutorials; dependencies are explicit. Electrical has no tutorial in this slice.

## S1. Piping / plumbing (sanitary) [book: Ch 7, tutorials 1-2]

1. Model with architecture (walls, floors, rooms/spaces) present, usually as a link; open the Plumbing floor plan for a level.
2. Load families BEFORE use: fixtures (water closet, urinal, sink, floor drain), pipe fittings (tee, bend, vent tee) and, if needed, accessories/equipment.
3. Configure pipe types: duplicate a pipe type (e.g. PVC DWV -> Sanitary-PVC; Sanitary Vent-PVC) and edit Routing Preferences (segment material; fittings per role such as junction = vent tee for the vent type). Fitting types may carry a K loss method.
4. Mechanical Settings > Pipe Settings: segments/sizes (material table), fluids (temperature/viscosity/density), slopes (list of allowed slope values), calculations (pressure-drop method, flow method), Conversions per System Classification (Sanitary: Main and Branch pipe type and middle elevation e.g. -1 ft / -305 mm).
5. Set plan View Range deep enough (bottom and view depth -5 ft / -1500 mm) so below-floor pipes and drains are visible.
6. Place fixtures: face-hosted (WC, sinks, drains) via Place on Face; wall-hosted (urinals) via Place on Vertical Face with an elevation (1'6" / 450 mm); WC with Rotate after placement and Angle.
7. Optionally Analyze > Show Disconnects (Pipe) to see open connectors; set Wireframe visual style for visibility.
8. Select fixtures that share a system -> Piping (Create Systems panel) -> choose Sanitary system type and name (e.g. "Drainage System - Toilet 1").
9. Generate Layout: Network solution; step solutions (n of m); Settings (main/branch type and offset); optionally Place Base, Edit Layout; Finish Layout (pipes + fittings created).
10. Extend: Edit System > Add to System for more fixtures; Generate Layout again; Finish Layout.
11. Verify in 3D (unload architectural link for clarity; Default 3D View); optionally System Inspector on placeholders.
12. Optional: Design to Fabrication (load a Fabrication configuration and service, select the whole run with Tab, convert, then adjust middle elevation).

Dependency summary: families -> pipe types/routing preferences -> mechanical settings (conversion, slopes, calcs) -> view range -> fixtures -> system membership -> layout -> real pipes -> checks. Domestic hot/cold water and vent follow the same pattern with different system types and pipe types (Ch 7 mentions both a sanitary system and a distribution system in the Create Piping System type list; only sanitary is built in the tutorial).

## S2. Fire protection (wet and dry) [book: Ch 8, tutorials 1-2]

1. Architecture/spaces linked; duplicate a mechanical plan view as "1-Fire Protection" and a ceiling plan for heads; create separate wet and dry 3D views [tutorial implies].
2. Create two pipe types (wet, dry), each with Routing Preferences segment (carbon steel Sch 40) and Assembly Code D40 (Fire Protection).
3. Mechanical Settings > Pipe Settings > Conversion: System Classification Fire Protection Wet and Fire Protection Dry each with Main and Branch pipe type and Middle Elevation (default 9'0" / 2750; tutorial 9'3" / 2820 = near ceiling plenum).
4. Space schedule (Analyze > Schedules/Quantities, category Spaces): Number, Name, Level, Area plus calculated "min sprinklers" = Area / 130 (imperial) or Area / 12 (metric); round up in practice.
5. Load sprinkler families (hosted pendent; non-hosted upright/dry).
6. Place heads (Systems > Sprinkler): hosted on the ceiling face in a ceiling plan; non-hosted with an explicit offset.
7. Select heads -> Piping -> system type (wet or dry), name -> Generate Layout (Network/Perimeter/Intersections) -> Settings (main/branch offset, e.g. 9'6" / 2895.6 mm) -> Finish Layout.
8. Verify in 3D; tag and size pipe (sizing/tagging are mentioned but not procedurally covered).

Constraint: the main/branch elevation must be above pendent heads and below upright heads, else no layout solution.

## S3. Electrical [not in slice]

No book content in lines 12820-18694. See Open questions. [knowledge] Typical build order: link architecture -> place electrical equipment (panels, transformers, switchboards) -> place devices/fixtures (receptacles, lighting fixtures, switches) -> create power/lighting/switch systems by selecting devices and Power/Lighting/Switch System tools -> assign to panel -> generate cable tray/conduit routing -> panel schedules/load calc. Not verified against the book.

## S4. Documentation [book: Ch 9]

Model views -> Visibility/Graphics (annotation categories on) -> dimension styles and dimensions -> tag loading and Tag All/Tag by Category -> callouts and detail lines/drafting views -> schedules -> title block load -> sheets -> add views (set view scales first) and schedules -> activate/pan viewports -> print/publish.

## S5. Family authoring [book: Ch 10]

Template -> reference planes + parameters (dimensions labelled) -> work planes -> forms (extrusion/revolve/sweep/blend, voids) bound to parameters -> connectors (duct/pipe) with flow direction and system classification, size bound to parameters -> Family Types (type vs instance) -> Save As Family -> Load into Project.

---

# PART A. Chapter 7 tools: plumbing and piping

## A1. Plumbing Fixture (place)

- Where: Systems > Plumbing & Piping > Plumbing Fixture. Contextual tab "Modify | Place Plumbing Fixture". Shortcut: none in book ([knowledge]: PX). (Ch 7, "Adding Plumbing Fixtures")
- Preconditions: a plumbing floor plan view is active (Project Browser > Plumbing > Floor Plans > e.g. 1-Plumbing); the fixture family must be loaded (Mode panel > Load Family, or Insert > Load Family). Library path pattern: Plumbing > MEP > Fixtures > (Urinals, Water Closets, Sinks, Drains, bath tubs...). (Ch 7)
- Interaction sequence:
  1. Click tool. Optionally Load Family (Mode panel).
  2. Pick a type in the Properties-palette Type Selector. Edit instance parameters (Elevation, Schedule Level, etc.) BEFORE placing.
  3. Optional: Tag panel > Tag on Placement toggle (tag appears with the fixture). Options Bar > "Rotate after placement" check box: after the click, type an angle in the Options Bar Angle box and press Enter (tutorial used -90). (Ch 7, Tutorial 1 "Adding the Plumbing Fixtures")
  4. Choose a placement mode in the Placement panel: Place on Vertical Face (wall-hung items such as urinals), Place on Face (face-hosted items such as sinks, floor drains: click the slab or ceiling face), Place on Work Plane. Click in the view to place. Preview follows the cursor.
  5. Repeat clicks to place more of the same type. Exit with Modify (Select panel) or Esc.
- Hosting: fixture families are wall-hosted (e.g. wall-hung urinal) or face-hosted (e.g. island sink, floor-mounted water closet) [book]; hosted fixtures need the host (wall/floor) present, possibly in a link.
- Type parameters (examples from bath tub, Ch 7 table): Materials and Finishes (Faucet Material, Bath Tub Material); Mechanical: WFU (water fixture units, total domestic supply load), HWFU (hot water FU), CWFU (cold water FU); Dimensions: Sanitary Radius (drain connector radius), Hot Water Radius, Cold Water Radius, width, length; Identity: Keynote, Model, Cost. [book]
- Instance parameters: Constraints (Level, Host, Offset from host), Plumbing > Flow Pressure (pressure required at the supply outlet), Mechanical > System Classification (read-only), System Type, System Name, System Abbreviation; Comments; Phasing (Phase Created/Demolished). In the tutorial, Elevation and Schedule Level also appear for face/wall hosted fixtures (urinal Elevation 1'6" = 450 mm). [book]
- Fixture anatomy [book]: one or more water outlets plus a drain; flood rim lets the fixture overflow; every fixture drain has a trap (water seal blocks sewer gas). Hot and cold supplies can both be present (one fixture, two supply connectors). Each fixture has several pipe connectors (cold, hot, sanitary).
- Rules & behaviours: WFU values are per-type parameters feeding the fixture-unit-to-flow conversion (see A9). Fixture connectors carry system classification (Domestic Cold Water, Domestic Hot Water, Sanitary) set in the family; the placed instance's System Type/Name fill in only after it is assigned to a piping system.
- Gotchas: view range matters. The tutorial lowers the plan View Range bottom and view depth to -5' (-1500 mm) relative to Level 1 so floor-level items (floor drain, pipes at -1') are visible; otherwise items below the cut plane vanish from the plan. (Ch 7, "Setting the View Range")
- Outputs: IFC4 [knowledge]: IfcSanitaryTerminal (WC, urinal, lavatory, bath, sink via PredefinedType: WATERCLOSET, URINAL, WASHHANDBASIN, BATH, SINK, SHOWER...), IfcWasteTerminal (floor drain, FLOORTRAP, GULLYSUMP). Ports: IfcDistributionPort with FlowDirection SINK/SOURCE, SystemType (DOMESTICCOLDWATER, DOMESTICHOTWATER, SEWAGE/WASTEWATER) via Pset_DistributionPortTypePipe or port attributes. FU as a property set [knowledge, custom].

## A2. Pipe Type (create/duplicate) and Routing Preferences

- Where: Project Browser > Families > Pipes > Pipe Types > right-click Standard > Duplicate, then Rename or double-click (Type Properties). (Ch 7, "Creating the Pipe Type"; Ch 8, "Creating New Pipe Types")
- Interaction: duplicate the type (default name "Standard 2"); open Type Properties; Duplicate button to name (e.g. PVC-Sanitary); click Edit in the Routing Preferences value; set rows; OK; optionally Rename. Type Properties can also hold Assembly Code (Uniformat, via Choose Assembly Code dialog, e.g. D40 Fire Protection; sets Assembly Description), Keynote, Model, Manufacturer, Cost. [book]
- Routing Preferences dialog: a table of fitting roles, each with Segment/Size range and a Content cell choosing a family type. Roles named in the book: Pipe Segment (material, e.g. Steel Carbon Schedule 40), Elbow, Junction (tee), Cross, Transition, Union, Cap, plus size min/max per row. Load Family button in the dialog loads missing fittings (e.g. Tee Vent PVC Sch 40 DWV, then set Junction = that type). (Ch 7, tutorial "Configuring the Piping Systems"; Ch 7, "Creating the Pipe Type")
- Rules & behaviours: the pipe type's routing preferences decide which fitting is auto-inserted at every bend/branch/size change when the user draws or converts a layout. A different pipe type per service (e.g. PVC sanitary vs PVC vent) with a different junction (vent tee vs sanitary tee). [book]
- Outputs: IfcPipeSegmentType + IfcPipeFittingType catalog [knowledge].

## A3. Pipe Fitting types (loss method)

- Where: Project Browser > Families > Pipe Fittings > (Elbow-Generic, Tee, etc.) > Standard > Duplicate > Type Properties. (Ch 7, "Creating Fitting Types for Pipes")
- Fitting kinds named: bends/elbows, cap, cross, coupling, tee, transition, reducer, plug, union, wye. [book]
- Loss Method type parameter: None / K Coefficient / K Coefficient from Table (book also says "Not Defined" to ignore loss). Choosing K Coefficient enables the K Coefficient value; choosing the table option enables K Coefficient Table (pick lookup table). Other type fields: Keynote, Model, Manufacturer, Cost, Lookup Table Name. [book]
- Rules: K is a fluid-resistance coefficient used in pressure-drop analysis: dP = K * rho * v^2 / 2 [knowledge].

## A4. Mechanical Settings > Pipe Settings

- Where: Systems > Plumbing & Piping panel > Mechanical Settings (dialog launcher), left tree: Pipe Settings. Tabs/nodes named: Segments and Sizes, Fluids, Slopes, Calculations, Conversions. (Ch 7, "Specifying the Pipe Settings"; Ch 7 tutorial)
- Segments and Sizes: choose a Segment (material) from the dropdown; the Size Catalog table lists sizes with Nominal, Inside and Outside diameter columns (editable). New Size button opens Add Pipe Size (Inside Diameter, Nominal Diameter, Outside Diameter). Delete Size removes the selected row. (Ch 7, "Setting the Pipe Material and Pipe Size")
- Fluids: Fluid Name dropdown; table of Temperature / Viscosity / Density rows; New Temperature (temperature, viscosity, density), Delete Temperature; Add Fluid button opens New Fluid (New Fluid Name, "New Fluid Based on" an existing fluid whose properties it inherits). (Ch 7, "Specifying the Fluid in the Pipe")
- Slopes: table of slope values; New Slope (Slope Value box), Delete Slope. These feed the Slope Value dropdown on the Modify | Place Pipe tab. (Ch 7, "Setting the Slope of the Pipe")
- Calculations: three tabs. Hydronic Networks: check box "Enable analysis for closed loop hydronic piping networks" (analyzes flow and pressure drop in supply/return loops; uses Colebrook). Pressure Drop: Calculation Method = Colebrook Equation (default) or Haaland Equation. Flow: Calculation Method default Plumbing Fixture Flow (description shown below); converts fixture units into volumetric flow. (Ch 7, "Specifying the Calculation Method for the Pressure of the Pipe")
- Conversions: System Classification dropdown (Sanitary, Fire Protection Wet/Dry, etc.); Main area and Branch area, each with Pipe Type and Middle Elevation (offset). Applied when a layout path is converted to real pipe (see A8). (Ch 7 tutorial "Creating the Sanitary System"; Ch 8, "Configuring the Pipe Conversion Settings")
- Implementer note: model fluid as a table-interpolated (temperature -> viscosity, density). Colebrook is implicit (iterate); Haaland is explicit [knowledge].

## A5. Pipe (manual route)

- Where: Systems > Plumbing & Piping > Pipe. Contextual tab "Modify | Place Pipe". Shortcut [knowledge]: PI. Works in plan or section view. (Ch 7, "Manually Routing Pipes")
- Placement Tools panel: Automatically Connect (on by default; snaps to components at different elevations and connects); Inherit Elevation (start/end takes elevation from the element snapped to); Inherit Size (takes diameter of the element snapped to); Justification (dialog: Horizontal Justification, Horizontal Offset, Vertical Justification).
- Sloped Piping panel: Slope Off, Slope Up, Slope Down; Slope Value dropdown (from Mechanical Settings > Slopes); Show Slope Tooltip (shows start offset and slope while drawing); Ignore Slope to Connect (ignores the current slope when joining a component). 
- Tag panel: Tag on Placement.
- Options Bar: Diameter (dropdown/typed), Offset (vertical offset above current level), Lock/Unlock Specified Elevation button. Properties palette: Type Selector = pipe type; instance props such as System Type, Reference Level, Offset, Diameter, Pipe Segment, Horizontal/Vertical Justification.
- Interaction sequence: choose Pipe; set type; set Diameter/Offset; click start point (a segment rubber-bands from it); click end point; keep clicking to chain segments (fittings auto-inserted per routing preferences); Modify (Select panel) or Esc to finish. Changing Offset in the Options Bar mid-run adds a vertical riser with elbows [knowledge]. (Ch 7)
- Gotchas: if Automatically Connect is off, ends will not join to a nearby pipe/fitting [book]. Slope is applied relative to the start point offset; the tooltip shows the resulting end elevation [book/knowledge].
- Outputs: IfcPipeSegment (PredefinedType RIGIDSEGMENT), fittings IfcPipeFitting, ports IfcDistributionPort, grouping into IfcDistributionSystem (PredefinedType e.g. DOMESTICCOLDWATER, SEWAGE, FIREPROTECTION) via IfcRelAssignsToGroup [knowledge]. Slope as inclination of the segment axis (no dedicated IFC attribute; use property).

## A6. Modify Pipe (selected segment)

- Select a pipe: Modify | Pipes contextual tab; end connectors show endpoint offsets. Edit panel tools: justification, slope (constraints), cap open end. Pipe Insulation panel: Add Insulation / Edit Insulation / Remove Insulation. Properties palette: Horizontal Justification, Vertical Justification, Reference Level, Offset, System Type, Diameter, Pipe Segment. (Ch 7, "Modifying a Pipe Segment")
- Add Insulation dialog: Insulation Type dropdown, Thickness box, Edit Type button (Type Properties: Material via Material Browser, Keynote, Model, Manufacturer, Cost). [book]
- Outputs: insulation as IfcCovering (PredefinedType INSULATION) related to pipe via IfcRelCoversElements [knowledge].

## A7. Pipe Fitting and Pipe Accessory (place)

- Pipe Fitting: Systems > Plumbing & Piping > Pipe Fitting; tab "Modify | Place Pipe Fitting". Options Bar: Level, "Rotate after placement". Type Selector picks fitting type; instance: Offset, Insulation Thickness, Nominal Radius. Click at a pipe end (fitting snaps to the end and resizes to the pipe diameter) or at a point along a pipe (e.g. tee). Esc/Modify to finish. Later, select fitting: Layout panel (connect to other pipes), Edit panel (slope/justification/cap), Pipe Insulation panel; Options Bar Diameter and Offset. (Ch 7, "Placing Fittings")
- Pipe Accessory: Systems > Plumbing & Piping > Pipe Accessory (valves, strainers, gauges, air separators, venturi flow meter; families under Pipe > Accessories and Pipe > Valves). Options Bar: Level; Type Selector (example double check valve); instance props: Pressure Drop, Flow, Loss Method. Hover over a pipe (accessory snaps in-line) and click. On-canvas controls: rotate, flip, add pipe to inlet/outlet connector. (Ch 7, "Placing Pipe Accessories")
- Rules: in-line accessory/fitting splits the pipe at insertion point [knowledge]. Accessory pressure drop may be fixed or K-based via Loss Method [book].
- Outputs: IfcPipeFitting; IfcValve (PredefinedType per kind), IfcFlowMeter, IfcFlowInstrument [knowledge].

## A8. Plumbing Equipment (place)

- Where: Systems > Plumbing & Piping > Plumbing Equipment. Families (water heaters, sewage/waste/drainage pumps) from Plumbing > MEP > Equipment. Options Bar option; Type Selector (e.g. Water Heater 40 Gallon / 152 L); instance Level, Offset from Host. Click to place. (Ch 7, "Adding Plumbing Equipments")
- Outputs: IfcPump / IfcBoiler or IfcWaterHeater [knowledge: IfcTank/IfcHeatExchanger variants], IfcFlowStorageDevice.

## A9. Create Piping System / System assignment

- Where: select fixtures (or sprinklers, equipment) in a view; contextual tab "Modify | Plumbing Fixtures"; Create Systems panel > Piping. Dialog "Create Piping System": System type dropdown (Sanitary, domestic water systems, etc.), System name text box. OK. (Ch 7, "Auto Routing Pipes"; Ch 7 tutorial)
- Result: selected elements are boxed with a dotted rectangle; tab "Modify | Piping Systems" appears. System Tools panel has System Selector and Edit System. Edit System opens "Edit Piping System" tab: Add to System (default; non-members fade, cursor shows +), Remove from System; Finish Editing System (Mode panel) commits. (Ch 7)
- Rules & behaviours [book]: sanitary and water distribution systems are separate system types; a fixture with hot and cold connectors can belong to both a cold and hot system. Piping system holds the members; routing is generated per system.
- Flow derivation [book + knowledge]: Calculation Method "Plumbing Fixture Flow" converts the sum of fixture units in each pipe section to a design flow via a fixture-unit table [knowledge: Hunter-curve style lookup, tables for flush tank vs flush valve].
- Outputs: IfcDistributionSystem (PredefinedType DOMESTICCOLDWATER / DOMESTICHOTWATER / SEWAGE / etc.) with IfcRelAssignsToGroup to members [knowledge].

## A10. Generate Layout (auto-route)

- Where: select a system member (fixture) > Modify | Plumbing Fixtures (or Modify | Piping Systems) > Layout panel > Generate Layout (real routing preview) or Generate Placeholder. Opens contextual tab "Generate Layout". (Ch 7, "Auto Routing Pipes"; Ch 8, "Process of Creating a Fire Protection System")
- Preconditions: pipe settings (types, conversion main/branch type + offset) set; fixtures placed; members assigned to a system; for sprinklers, a pipe offset must lie on the correct side of the heads (see C3).
- Interaction sequence:
  1. Generate Layout opens with a default solution previewed in the view (layout lines, not real pipe).
  2. Options Bar > Solution Type: Network (default; up to 5 solutions for fixtures in the book's example; "n of m" counter with Previous/Next Solution arrows), Perimeter, Intersections, Custom (Ch 7 lists Network, Perimeter, Intersections, Custom; Ch 8 lists the first three). The number of Network solutions depends on network complexity.
  3. Options Bar > Settings opens "Pipe Conversion Settings": left pane Main / Branch; each has Pipe Type and Offset. (Tutorial entered -1' (-305 mm) for sanitary main and branch; fire tutorial 9'6".)
  4. Generate Layout tab > Modify Layout panel: Place Base (adds an outlet/base point where the flow must exit, e.g. a chase or open area; Options Bar then shows base Offset and Diameter); Modify Base (default), Remove Base; Solutions; Edit Layout (select a layout segment, a move icon appears; drag to relocate; click the displayed offset number, type a new value, Enter).
  5. Finish Layout (Generate Layout panel) commits: with "Generate Layout" real pipes are created; with "Generate Placeholder" placeholder segments are created.
- Placeholders: select placeholders; "Modify | Pipe Placeholders" tab; Analysis panel > System Inspector (floating panel: Inspect tool shows flow/slope direction arrows; hover on a segment shows a tag with section number and fixture units; Finish exits). To make real pipe: select placeholders, set Diameter in the Options Bar, then Edit panel > Convert Placeholder. (Ch 7)
- Adding fixtures after layout: select a member, Piping Systems tab > Edit System > Add to System > pick fixtures > Finish Editing System; select an added fixture > Generate Layout again (pipes extended to the new fixtures; pick a Network solution; Finish Layout). Tutorial: network solution "2 of 5" first, then "4 of 4" after adding more fixtures. (Ch 7 tutorial)
- Gotchas: layout lines can be rerouted but only conversion settings determine real elevation/type. Conversion settings must be correct BEFORE Finish Layout; they are applied at conversion. (Ch 8, Note)
- Outputs: same as A5; the layout is an intermediate (non-IFC) state. Fixture units per section [book] (System Inspector tag).

## A11. Show Disconnects

- Where: Analyze > Check Systems > Show Disconnects. Dialog "Show Disconnects Options": check Pipe (also Duct, Electrical). Result: warning symbols at every open connector. (Ch 7 tutorial, "Creating the Sanitary System")
- Use: verify all fixture connectors are connected after routing. Implementer: a validation pass listing unconnected IfcDistributionPort.

## A12. Design to Fabrication

- Where: Systems > Fabrication > Fabrication Settings (dialog: Fabrication configuration tab, choose a configuration e.g. "Revit MEP Imperial/Metric Content V2.2"; Services tab: Unloaded services list, Add moves to Loaded services; e.g. Piping DWV PVC40). (Ch 7, Tutorial 2)
- Convert: select the design pipes/fittings (hover a pipe, press Tab (three times in the tutorial) to highlight the whole connected run, then click); contextual tab "Modify | Multi-Select" > Fabrication panel > Design to Fabrication; "MEP Fabrication Parts" palette plus "Change Service Editor" tab appear; green tick Finish converts; Close palette. (Ch 7)
- Modify fabrication pipework: select a fabrication pipe; Properties palette; type Middle Elevation (e.g. -6"); Apply; the parameter propagates to all horizontally connected pipework. [book]
- Outputs: fabrication parts keep connectivity; IFC [knowledge]: IfcPipeSegment / IfcPipeFitting with manufacturer/product data.

## A13. View range for plumbing plans

- Properties palette > View Range Edit. Primary Range: Bottom = Associated Level with offset; View Depth: Level = Associated Level, offset. Tutorial sets both Bottom and View Depth offsets to -5' (-1500 mm). Apply then OK. (Ch 7, "Setting the View Range")

## A14. 3D view and links

- Manage > Manage Project > Manage Links: select the linked architectural model, Unload (to see MEP only). View > Create > 3D View > Default 3D View. (Ch 7 tutorial "Viewing the Plumbing System in 3D")

---

# PART B. Chapter 8 tools: fire protection (sprinklers)

## B1. Sprinkler (place)

- Where: Systems > Plumbing & Piping > Sprinkler. Tab "Modify | Place Sprinkler" (Mode panel > Load Family; Placement panel). Library: Fire Protection > Sprinklers. (Ch 8, "Placing Sprinkler Heads")
- Preconditions: for hosted heads, open a CEILING plan (recommended) and have a ceiling/wall/slab host (can be in a linked architectural model, or an "envelope" modeled in the project); non-hosted heads can be placed in a floor plan. Load sprinkler families first. A space schedule to count heads is recommended first (see B3). (Ch 8)
- Interaction sequence:
  1. Activate ceiling plan (hosted) or floor plan (non-hosted).
  2. Click Sprinkler; optionally Load Family.
  3. Type Selector: hosted example "Sprinkler-Pendent-Hosted 1/2in (15 mm)" or non-hosted "Sprinkler-Dry-Upright 1/2in Upright (15 mm)". Instance parameters differ between hosted and non-hosted types.
  4. Hosted: Placement panel > Place on Face (ceiling), Place on Vertical Face (wall) or Place on Work Plane; hover over a ceiling tile (preview), click. Repeat. Non-hosted: click anywhere; set the Offset (elevation) so the head sits at a proper height.
  5. Modify (Select panel) to exit.
- Rules & behaviours [book]:
  - Hosted heads are face-based families (host: wall, ceiling, slab, soffit); non-hosted are independent of surfaces.
  - A head created for a dry system can only join dry systems; a wet head only wet systems. (Ch 8, "Wet and Dry Fire Protection Systems")
  - Sprinkler flow data on individual heads is for scheduling/coordination/analysis only; do not size pipe from per-head flow. (Ch 8, "Guidelines")
- Wet system [book]: automatic heads that discharge immediately when heat opens them; risers feed heads; includes automatic alarm check valve. Dry system [book]: pipes hold pressurized air or nitrogen; opening of a head releases pressure so a dry-pipe valve opens and water flows. Used where freezing is possible (unheated buildings, parking garages, canopies).
- Outputs: IfcFireSuppressionTerminal (PredefinedType SPRINKLER) [knowledge]; pipes IfcPipeSegment in IfcDistributionSystem PredefinedType FIREPROTECTION [knowledge]; valves IfcValve (ALARMCHECK? not standard; use ISOLATING/CHECK) [knowledge].

## B2. Fire protection pipe types and conversion settings

- Create pipe types: Project Browser > Families > Pipes > Pipe Types > Standard > Duplicate; Type Properties > Duplicate with name (book names: Fire Protection-Wet / Fire Protection-Dry; tutorial: Wet Protection-Fire / Dry Protection-Fire). Routing Preferences > Pipe Segment (e.g. Steel, Carbon - Schedule 40). Optional Assembly Code D40 (Uniformat Fire Protection) so schedules can sort. (Ch 8, "Creating New Pipe Types"; tutorial 1)
- Pipe Conversion settings: Systems > Mechanical Settings > Pipe Settings > Conversion; System Classification = Fire Protection Wet or Fire Protection Dry (separate entries); Main: Pipe Type, Middle Elevation; Branch: same fields. Default middle elevation 9'-0" (2750 mm); the tutorial sets 9'-3" (2820 mm). The reference level for the height is the plenum: choose elevation relative to the ceiling plenum. (Ch 8, "Configuring the Pipe Conversion Settings")
- Rules: conversion settings apply when a layout is converted to physical pipes; configure/verify before Finish Layout. Branch offset lets branches run above or below the main/obstacles for clash avoidance. (Ch 8, Note)

## B3. Space schedule for sprinkler count (Analyze > Schedules/Quantities)

- Where: Analyze > Reports & Schedules > Schedule/Quantities. (Ch 8, "Creating the Space Schedule"; tutorial)
- Sequence (tutorial): New Schedule dialog: Filter list = Mechanical; Category = Spaces; Name; "Schedule building components"; Phase = New Construction. Fields tab: add Number, Name, Level, Area (reorder with Move parameter up/down); Add calculated parameter: name "Sprinkler Quantity - Min. Required", Formula, Discipline Common, Type Number, formula Area/130 (imperial, one head per 130 sq ft) or Area/12 (metric, per 12 m2). Formula field names are case sensitive. Filter tab: Level equals Level 1. Sorting/Grouping: sort by Number ascending, Grand totals on (title, count, totals). Formatting tab: field format with fixed units and 2 decimals; "Calculate totals" for the calculated column. (Ch 8)
- Rule: result should be rounded UP to next whole number when placing heads (the schedule shows decimals). Schedules are live views of the model database; editing a cell edits the element. (Ch 8, Note)
- Note: 130 sq ft / 12 m2 are the book's example code coverage; actual coverage depends on hazard class [knowledge: NFPA 13 light hazard max ~225 sq ft per head, ordinary ~130 sq ft; the book value matches ordinary hazard].

## B4. Connect sprinklers: system + auto-layout

- Sequence (Ch 8, "Connecting the Sprinklers"; tutorial 2): (1) in ceiling plan, window-select the heads; tab "Modify | Sprinklers"; (2) Create Systems panel > Piping; dialog: System type (Fire Protection Wet/Dry), System name (e.g. "Wet Fire Protection System-Central Area"); OK; (3) tab "Modify | Piping Systems" > Layout panel > Generate Layout; (4) Options Bar Solution Type: Network (default) / Perimeter / Intersections; Previous/Next Solution arrows with "n of m" (tutorial had 1 of 2); (5) Settings: Pipe Conversion Settings with Main and Branch (Pipe Type and Offset; tutorial 9'6" = 2895.6 mm both); (6) Finish Layout; (7) check in a 3D view (3D View-Fire Protection-Wet).
- Alternative: draw pipe manually with the Pipe tool (see A5) and connect to head connectors.
- Equivalent manual system creation sequence summary in book (Steps 1-4): place heads, create system, generate layout path, finish layout. (Ch 8, "Process of Creating a Fire Protection System")
- Guidelines (important for solvers) [book]: place the pipe ABOVE pendent heads and BELOW upright heads (a run on the wrong side of a head will not connect as intended). Always set a pipe elevation when using layout tools, because layout tools find no solution when the elevation is above upright heads or below pendent heads. (Ch 8, "Guidelines for Creating a Fire Protection System")
- Outputs: see B1.

## B5. Project view setup for fire protection

- Duplicate a plan view: Project Browser > Views (Discipline) > Mechanical > HVAC > Floor Plans > right-click > Duplicate View > Duplicate; rename (e.g. "1-Fire Protection") by right-click > Rename. Ceiling plans for hosted heads; a dedicated 3D view for the wet system. (Ch 8, "Creating the Project View")
- Pipe sizing for fire protection is left to the designer [book: sizing via Pipe Sizing / tags in the chapter text; see Ch 8 intro lists "size and tag the pipes" but no procedure given]. [knowledge] Revit's Pipe Sizing tool applies to fire protection only by the user; no hydraulic sprinkler calculation (e.g. density/area, Hazen-Williams) is performed by the book's workflow.

---

# PART C. Chapter 9 tools: construction documents (dimensions, text, tags, callouts, drafting, views, sheets)

Note: this slice contains NO electrical content (panels, circuits, cable tray, conduit, lighting, panel schedules). Those are not in lines 12820-18694; Chapter 9 is documentation and Chapter 10 is families and worksharing. See "Open questions".

## C1. Temporary dimensions and their settings

- Behaviour: appear (a) while drawing an element, (b) when an element is selected, (c) while placing/selecting a component. Not view-specific; not part of printed views. Show distance to the nearest perpendicular element with a predefined snap increment; clicking the value and typing a number moves/resizes the element. A conversion control (symbol) next to a temporary dimension turns it into a permanent dimension. (Ch 9, "Using Temporary Dimensions"; "Converting Temporary Dimensions into Permanent Dimensions")
- Settings: Manage > Settings > Additional Settings > Annotations > Temporary Dimensions: Walls reference = Centerlines | Center of Core | Faces | Faces of Core; Doors and Windows reference = Centerlines | Openings. (Ch 9)

## C2. Permanent dimensions (Annotate > Dimension panel)

- Common: tab "Modify | Place Dimensions"; Type Selector (Properties) chooses dimension style; permanent dimensions are VIEW-SPECIFIC (absent in other views); units follow project units. Esc (twice in the tutorial) or another tool ends it. (Ch 9, "Types of Dimensions")
- Aligned: Options Bar first dropdown = snap target (e.g. Wall centerlines); Pick dropdown = Individual References (or entire wall); click ref 1, ref 2, (more refs to chain), then click away to place the dimension line. Tutorial: aligned dimension across consecutive grid lines (click grid after grid, then click above the bubbles). Aligned measures two parallel/orthogonal references. (Ch 9, "Aligned Tool")
- Linear: shortest distance between two picked points; click point 1, point 2, then place. (Ch 9)
- Angular: pick two non-parallel references, then place the arc. (Ch 9)
- Radial / Diameter: click on arc/circle; a center mark is generated; place. (Ch 9)
- Arc Length: click the arc, then start and end references, then place; for curved duct or cable tray length. (Ch 9)
- Spot Elevation: Options Bar: Leader and Shoulder check boxes (default on), Display Elevations (e.g. Top&Bottom Elevation used in tutorial); types: Crosshair (Project/Relative), No Symbol (Project/Relative), Target (Project/Relative). Works in plan, elevation, 3D; on non-horizontal/non-planar faces; targets floors, ramps, ducts, equipment, topography, stairs. (Ch 9)
- Spot Coordinate: Northing/Easting; Leader/Shoulder options; types e.g. Horizontal. (Ch 9)
- Spot Slope: Options Bar Slope Representation = Arrow (default) | Triangle; Offset from Reference (default 1/16 in, metric 1.5875 mm); click on sloped edge/face, click again to fix vertical alignment; horizontal surface gives "No Slope". (Ch 9)
- Dimension anatomy [book]: dimension line (arc for angular/radial), text (value cannot be edited; prefix/suffix allowed; updates automatically), tick marks/arrows (selectable symbols), witness lines (drag control; right-click Move Witness Line; Edit Witness Line adds a reference to an existing dimension).
- Locking: a lock control on a permanent dimension. Locked = the constrained element cannot be resized by changing the number; element can still move together with it; unlock to change the value. [book]
- Label parameter: Modify | Dimensions > Label dropdown assigns a (global) parameter; Create Parameter (Label Dimension panel) opens Global Parameter Properties (Name); a pencil icon appears next to the text. [book]
- Dimension type parameters mentioned [book]: Leader Type (Arc/Line), Leader Tick Mark, Show Leader When Text Moves, Tick Mark, Line Weight (1-16), Tick Mark Line Weight, Dimension Line Extension, Equality Text (replaces "EQ"), Text Size, Text Offset, Text Font. Tutorial: Tick Mark = Arrow Filled 15 Degree; Centerline Pattern = Center; type duplicated and renamed.
- Dimensionable MEP items [book]: lengths of ducts, pipes, cable trays, conduits; distances between them; arc lengths; angles; spot elevations.
- Outputs for BIM: none in IFC (annotations); IFC4 has IfcAnnotation with IfcDimension... [knowledge, rarely used]. Treat dimensions as view-level drawing data.

## C3. Text notes

- Where: Annotate > Text > Text. Tab "Modify | Place Text". Panels: Leader (No Leader default; One Segment; Two Segments; Curved), leader attachment (Top/Middle/Bottom Left, Top/Middle/Bottom Right; defaults Top Left and Bottom Right), Paragraph (Align Left default/Center/Right), Tools (Check Spelling). (Ch 9, "Adding Text Notes")
- Sequence: choose leader style and alignment; click to place the note (one or two extra clicks if leader segments are used); a text box opens with "Edit Text" tab (Bold, Italic, Underline; Subscript, Superscript, All Caps; list bullets/numbers/letters; Increase/Decrease Indent; Text Editor Options: show border, opaque background while editing); type; Close in Edit Text panel; Modify to exit.
- Edit existing: stretch dots on box sides, rotation control at top right, leader drag controls; Add Left/Right Side Straight Leader, Remove Last Leader; Copy/Mirror/Rotate/Array; Copy to Clipboard / Paste from Clipboard to copy across levels. Instance props: Horizontal Align, Arc Leaders (off). Type props: Text Font, Text Size, Tab Size, Bold, Italic, Underline. (Ch 9, "Editing Text Notes")
- Text notes are view-specific. Check Spelling dialog: Context, Not in Dictionary, Change to, Change.

## C4. Tags

- Tags are view-specific annotations bound to an element category; each category needs a loaded tag family (Annotate > Tag panel expanded > Loaded Tags And Symbols; Load Family from English-Imperial/English > Annotations). Visibility per category: View > Graphics > Visibility/Graphics > Annotation Categories tab (Air Terminal Tags, Assembly Tags, Cable Tray Tags, Conduit Tags, Duct Tags, Electrical Equipment Tags, Mechanical Equipment Tags, Pipe Tags, Space Tags, Sprinkler Tags...). (Ch 9, "Adding Tags")
- Tag by Category: Options Bar: Horizontal/Vertical orientation; Leader check box; Attached End (leader end fixed on element; length box enabled) vs Free End. Hover element; preview; click. If no tag loaded: "No Tag Loaded" message, Yes opens Load Family. Selected tag shows move control, leader elbow control, "?" label control (editing a label edits a TYPE parameter of the tag and warns it affects many elements). (Ch 9)
- Tag All Not Tagged (Tag All): dialog with radio buttons "All objects in current view" and "Only selected objects in current view" (the latter enabled only with a prior selection); category table with check box, loaded tag type dropdown per category, Leader option, orientation, leader length; Apply, OK. If a category's visibility is off a message prompts to enable it (tutorial: room tags). Tag on Placement toggle exists on many place tools. (Ch 9; tutorial 1)
- Tag content [knowledge]: a tag family reads a parameter of the tagged element (e.g. Mark, size, system abbreviation, flow); many tags double as keys for schedules.
- Outputs: none in IFC; the data comes from element properties (Mark -> IfcElement.Tag).

## C5. Callout views

- Where: View > Create > Callout (drop-down): Rectangle, Sketch. Tabs "Modify | Callout" / "Modify | Edit Profile". Parent views: plan, section, elevation. (Ch 9, "Callout Views")
- Rectangle sequence: choose Rectangle; Type Selector (Detail View: Detail, or Floor Plan: Floor Plan (for current view)); set Scale in View Control Bar; optional Options Bar "Reference Other View" + view dropdown (creates a reference callout to an existing view, no new view); drag from top-left to bottom-right (or two clicks); a new view appears in the Project Browser under the parent category; open by double-click or right-click callout > Go To View. (Ch 9)
- Sketch: draw any closed profile with Line, Rectangle, Pick Lines etc. (Ch 9)
- Parts of a callout tag: bubble (outline), head (shows detail and sheet number when on a sheet), leader, reference labels. Parent deleted => callout view deleted. Controls: bubble extents, rotation, leader elbow. (Ch 9)
- View properties (Properties palette): View Name, Title on Sheet, Display Model (Normal | Halftone | Do not display), Underlay (None or a level, shown dimmed), Detail Level (Coarse | Medium default | Fine), Visibility/Graphics Overrides (Edit), View Scale. Tutorial: Display Model Halftone + Detail Level Fine for tracing, then switch Display Model back to Normal. (Ch 9)
- Detail lines: Annotate > Detail > Detail Line (tab Modify | Place Detail Lines; Line Style dropdown; Draw panel). View-specific. Filled Region via Annotate > Detail > Region. Detail Component, Insulation (graphical symbol), Region, break lines also in Detail panel. (Ch 9)

## C6. Drafting views and details

- Where: View > Create > Drafting View. Dialog "New Drafting View": Name, Scale dropdown (Custom with Scale value 1:n). Appears under Drafting Views in the Browser. Independent of the model (no model geometry visible; does not update with the model). Tools as in C5 detail panel plus dimension and text. (Ch 9, "Drafting Details")

## C7. Duplicating views

- Duplicate View: model geometry only. Duplicate with Detailing: model plus view-specific elements (annotations, detail lines). Duplicate as Dependent: child tied to parent, shows a crop region; edits to the parent view's model/annotation propagate. Reached via View > Create > Duplicate View dropdown or Project Browser right-click > Duplicate View. (Ch 9, "Duplicating Views")
- Dependent view workflow: create; crop region drag controls to crop; rename (Rename View dialog); Show Crop Region in View Control Bar; right-click crop region > Go to Primary View (from dependent) or Go to View (from primary). Use: split a large plan across sheets. Dependent views can come from plan, elevation, section, callout. (Ch 9)
- Note (book calls all duplicates "dependent on the original"; strictly only Duplicate as Dependent is linked) [knowledge].

## C8. Sheets

- Title blocks are families: Insert > Load Family > Titleblocks (ANSI sizes, portrait and landscape; tutorial used "C 17x22 Horizontal" imperial, "A2 metric"). Project data for title blocks: Manage > Settings > Project Information (client, project name, number, issue date...). (Ch 9, "Adding a Drawing Sheet to a Project")
- New sheet: View > Sheet Composition > Sheet; dialog "New Sheet": Select titleblocks list (Load button), Select placeholder sheets; OK creates sheet in Browser > Sheets with a number and name.
- Add view: View > Sheet Composition > View (dialog Views list > Add View to Sheet) or drag a view from the Project Browser onto the sheet; the viewport rectangle follows the cursor; click to place. The view's scale is a property of the VIEW (View Scale in Properties); one view can be placed on only one sheet [knowledge] (use duplicates for several sheets).
- Viewport properties: View Name (renaming the viewport title renames the view), View Scale, Detail Level, Visibility/Graphics Overrides (Edit), Rotation on Sheet, Crop Region Visible, label (title) type: Title, Show Title, Show Extension Line, line weight/colour/pattern. (Ch 9, "Modifying View Properties")
- Activate View (right-click viewport or Modify | Viewports > Activate View): edit the model through the viewport; changes propagate to all views (parametric change engine); Pan Active View moves the content within the viewport; Deactivate View (right-click or View > Sheet Composition > Viewports dropdown). (Ch 9)
- Schedules on sheets: drag schedule from Project Browser; column-width blue triangles; split control on the right edge makes sections; sections movable/resizable, rejoin by dragging a section onto its neighbour (only adjacent sections). (Ch 9)
- Guide grids: View > Sheet Composition > Guide Grid; dialog Assign Guide Grid (Name); properties Guide Spacing, Name; assigned via sheet's Guide Grid instance parameter. (Ch 9)
- Outputs for BIM: sheets are drawing-layer data; in IFC4 not modelled (documents via IfcDocumentInformation if needed) [knowledge].

## C9. Tutorial spine for Ch 9 (from-scratch order)

1. Open model with levels and grids; 2. set Visibility/Graphics so Dimensions annotation category is on; 3. create/duplicate a dimension style (tick mark, centerline pattern), dimension grids; 4. Tag All for duct, equipment, room/space tags; Tag by Category for stragglers (diffusers); 5. callout (Detail View) from a plan with halftone model and fine detail; tag ducts, spot elevation (Top & Bottom); 6. load title block, New Sheet, set view scales then drag views, hide crop region and elevation marks, pan; 7. save.

---

# PART D. Chapter 10 tools: massing, family authoring, worksharing

## D1. Massing environments and concepts

- Three environments [book]: Family Editor (File > New > Family, template e.g. Generic Model.rft), Conceptual Design Environment (File > New > Conceptual Mass, Mass template from Conceptual Mass folder), Project environment (Massing & Site tab > Conceptual Mass panel: In-Place Mass, Show Mass, Place Mass). Model by Face panel ("Building Maker" tools) converts mass faces to walls, floors, roofs, curtain systems; inclined planes and curved surfaces convert to in-place roofs. (Ch 10, "Creating the Massing Geometry")
- Limits [book]: for conceptual studies only; not for detailed geometry (columns, cavity walls, footings). The book's MEP framing: composed solids/voids are building blocks for equipment/fixture families (bath tub, AHU, WC volumes). (Ch 10, "Understanding Massing Concepts")
- Project-environment limits: no 3D reference planes, no 3D levels. Conceptual environment allows levels in 3D/elevation views. (Ch 10)

## D2. Family Editor form tools (Create > Forms panel)

Solid: Extrusion, Blend, Revolve, Sweep, Swept Blend. Void: Void Extrusion, Void Blend, Void Revolve, Void Sweep, Void Swept Blend (Void Forms dropdown). A void cuts every intersecting solid when finished (click away/Esc). (Ch 10)

- Extrusion: Work Plane panel > Set (choose plane/named reference plane/pick a face, or via Work Plane dialog Name or Pick a plane), draw closed-loop profile (Draw panel; Chain option in Options Bar), Options Bar Depth, Properties: Extrusion Start and Extrusion End (depth = End - Start; measured from the work plane), then Finish Edit Mode (green check). Edit later: select > Edit Extrusion (Mode panel) re-enters sketch; parameters Start/End, material, visibility. Associate Family Parameter button next to Extrusion End binds it to a family parameter (tutorial: VAV box Height). (Ch 10, "Creating an Extrusion"; tutorial)
- Revolve: set work plane perpendicular to the ground plane; Boundary Lines profile (single closed loop, or multiple non-intersecting loops); Axis Line (Line or Pick Lines); Finish. Instance props Start Angle (0) and End Angle (360). (Ch 10, "Creating a Revolved Geometry")
- Sweep: Sketch Path (open or closed) > Finish; then profile: Edit Profile (Go To View dialog picks the view where the profile is drawn), or pick a built-in profile in the Profile dropdown, or Load Profile (Profiles folder). Finish twice. Edit Sweep > Properties: Part Type (Mechanical group), Shared check box (Other group). (Ch 10, "Creating a Sweep")
- Blend: base boundary sketch; Edit Top for the top boundary; Edit Vertices (available once both profiles exist) sets vertex connectivity; First End / Second End instance values or Depth in Options Bar set heights. (Ch 10, "Creating a Blend")
- Swept Blend: path is a single open segment (Sketch Path or Pick Path); Select Profile 1 > Edit Profile (Go To View) draw at the path's red dot end; Select Profile 2 likewise; Finish Edit Mode. (Ch 10, "Creating a Swept Blend")
- Editing: arrow drag controls on faces resize the solid directly; Mirror, Copy, Group, Array work on forms. (Ch 10, "Editing a Massing Geometry...")
- Load into project: Modify > Family Editor panel > Load into Project (dialog lists open projects when several); the loaded family attaches to the cursor in Place Component. (Ch 10, "Loading Massing Geometry into a Project")
- Outputs [knowledge]: family geometry becomes IfcExtrudedAreaSolid / IfcRevolvedAreaSolid / IfcSweptDiskSolid / IfcBooleanResult(DIFFERENCE) in an IfcRepresentationMap for the type; void forms map to boolean differences or openings.

## D3. Place Mass and In-Place Mass (project)

- Place Mass: Massing & Site > Conceptual Mass > Place Mass. If Show Mass is off, a "Show Mass Enabled" window and a message box appear (Close, then Yes) leading to Load Family (Mass subfolder). Placement panel: Place on Face (align to a clicked face) or Place on Work Plane (Options Bar Placement Plane = level). "Rotate after placement" option. Dimension instance parameters editable in Properties > Dimensions. (Ch 10, "Placing the Massing Geometry in a Project")
- In-Place Mass: Massing & Site > Conceptual Mass > In-Place Mass; Name dialog; Create tab: Draw panel > Model (Modify | Place Lines) + Reference Plane; select sketch; Form panel > Create Form > Solid Form | Void Form; tab "Modify | Form": X-Ray, Add Edge, Add Profile, Divide Path/Surface, Dissolve. (Ch 10, "Creating the In-Place Mass in a Project")
- Divide Path: select form > Edit In-Place > select an edge > Divide Path (Divide panel); default 6 nodes shown; click the number to edit count; alternatives: set distance between divisions or intersect with levels, reference planes or divided paths. (Ch 10, "Dividing Edges or Paths")

## D4. Conceptual Design Environment

- Ribbon tabs: Create, Insert, View, Manage, Modify. Create tab panels: Work Plane (Set, Show, Viewer), Datum (Level, plane), Draw (Model, Reference, Plane; with Draw on Face / Draw on Work Plane), Family Editor (Load into Project), Dimension. References: Line, Rectangle, Circle, Pick Lines, Point Elements. Edit tools Offset/Move/Copy in Modify panel of Modify | Place Lines. (Ch 10)
- Forms: select sketch lines > Form panel > Create Form > Solid Form (line extrudes with temporary dimensions) | Void Form. Select an extruded face > Edit Profile (Mode panel) to re-sketch it; Finish Edit Mode. (Ch 10)
- X-Ray (Form Element panel): shows geometry skeleton (vertices, edges, profiles); only one form can be in X-Ray at a time; toggle again to restore. Add Profile: cross-section profile inserted at the clicked position on the surface (shows blue highlighted preview); visible only in X-Ray. Add Edge: adds an edge at click. Dissolve: deletes the surfaces but keeps profiles for re-forming. Divide Surface also present. (Ch 10)

## D5. Family authoring mechanics shown in the VAV tutorial (Ch 10 Tutorial 1)

Useful because it is the only from-scratch parametric-family walkthrough in the slice; it encodes the dependency order for MEP equipment families.

Build order (Mechanical Equipment.rft template; Ref. Level plan and Front/Right/Left elevations):
1. New Family from Mechanical Equipment template (template provides reference planes and a Ref. Level).
2. Reference Planes (Datum panel > Reference Plane > Pick Lines with Options Bar Offset, Enter) at fixed offsets either side of the centre planes: 2' (610) each side in X, 1' (305) each side in Y; then Aligned dimensions across the outer planes; click the blue EQ symbol on the dimension to make the planes permanently equally spaced; select the overall dimension > Create Parameter (Label Dimension panel) to label it. Dimension labels become family parameters.
3. Parameter kinds: Shared parameter (Parameter Properties > Shared parameter > Select; first time creates a Shared Parameter File via Edit Shared Parameters: Create file, Groups > New (e.g. "Mechanical VAV box"), Parameters > New with Name, Discipline (Common, HVAC), Type of Parameter (Length, Air Flow)). Tutorial parameters: VAV box Width/Length/Height, VAV Outlet Width/Height, VAV Inlet Diameter (Length), VAV Max CFM (HVAC, Air Flow). Non-shared family parameters made in Family Types dialog > New Parameter (Name, Type of parameter, Group parameter under = Other / Mechanical - Flow, Type vs Instance radio).
4. Named reference plane: select a plane, Properties > Name (e.g. "VAV box bottom"); Work Plane > Set > Name radio to choose it; then Go To View dialog picks the plan view.
5. Body: Extrusion with rectangle snapped (locked) to the reference planes; Extrusion Start 0; Associate Family Parameter on Extrusion End to the height parameter.
6. Outlet sleeve: Work Plane > Set > Pick a plane then click the body's right face; sketch an 8x8 in (200x200 mm) rectangle locked to reference planes; Extrusion Start = 2' (610 mm), End = 2'2" (660 mm); labelled with Outlet Width/Height parameters.
7. Inlet sleeves: Family Types dialog; add shared parameter (Inlet Diameter 4"/100 mm); add a calculated family parameter "Inlet Radius" with formula "VAV Inlet Diameter / 2" (Apply, OK); circles via Extrusion/Circle tool at reference-plane intersections; Radial dimension then Label dropdown = Inlet Radius; Extrusion Start 0, End -4" (-100 mm). Hide helper planes via Temporary Hide/Isolate. Visual Style Hidden Line.
8. Control boxes: Pick a plane on the body face; reference planes by offset 2" (51 mm) and 1' (300 mm); rectangle 1'x10" (300 x 254 mm), Extrusion Start 1' End 1'4"; a mirror on the back with negative values.
9. Connectors: Create > Connectors panel > Duct Connector with Placement panel Face tool; click the sleeve face. Select connector: "+" next to width/height dimension opens Associate Family Parameter (bind to VAV Outlet Width/Height). Properties: Flow Direction (Out for outlet, In for inlet), System Classification (Supply Air), Shape (Round for inlets), Diameter bound to "Duct Inlet Radius" (a calculated parameter = Inlet Radius + 1/16" (2 mm)), Flow via Associate Family Parameter > New Parameter (instance, group Mechanical - Flow): VAV Outflow, Inlet Flow.
10. Save As > Family (.rfa); Load into Project to use.

Rules & behaviours [book]:
- A connector is a face-hosted element on the geometry with: size parameters, shape, flow direction (In/Out/Bidirectional [knowledge: Bidirectional]), system classification, flow (instance parameter). Piping connectors work the same way [knowledge]; the plumbing fixture connectors seen in Ch 7 (hot, cold, sanitary radii) are produced this way.
- Formula parameters: expressions referring to other parameters by name with units arithmetic (e.g. diameter / 2; radius + small clearance). Calculated duct connector size is slightly larger than the geometry radius so a duct slips over the sleeve [book].
- Shared parameters (file-based, GUID identity) can be scheduled and tagged across families [knowledge]; type vs instance determines whether a value is per-family-type or per-placement.
- Outputs [knowledge]: connectors export as IfcDistributionPort (FlowDirection SOURCE/SINK/SOURCEANDSINK; PredefinedType DUCT/PIPE; SystemType SUPPLYAIR etc.) nested in the IfcDistributionElement via IfcRelNests; shared parameters as IfcPropertySet properties.

## D6. Worksharing (Collaborate tab)

Concepts [book]: Single integrated model file; worksets = ownership groups of elements; only one user can have a workset editable at a time; central file (master, stores ownership of all worksets) vs local files (each user's working copy; renamed copy of central). Worksharing is off by default for a new project. Do not work directly in the central file with one user. (Ch 10, "Worksharing Concepts")

- Enable: Collaborate > Manage Collaboration > Collaborate; dialog "Collaborate" radio "Within your network" (default) or "In the cloud" (Autodesk Construction Cloud). If the model is unsaved a dialog offers "Save the model and continue". Then Worksets tool. Disable Worksharing is under the expanded Workset panel (the review questions ask whether enabling can be undone without giving an answer; the text itself offers Disable Worksharing). (Ch 10, "Using Worksets Tool")
- Worksets dialog columns: Name, Editable (Yes/No), Owner, Borrowers, Opened (Yes/No; No hides contents), Visible in all views (check). Top: Active workset dropdown (only user-defined, opened worksets), "Gray Inactive Workset Graphics" check box. Buttons: New (Enter new workset name), Delete (Delete Workset dialog: delete the elements, or "Moved to" another workset), Rename, Open/Close (sets Opened), Editable/Non Editable. Show filter check boxes: User-Created, Families, Project Standards, Views. Default worksets: "Shared Levels and Grids" (levels/grids) and "Workset1" (everything else). Username comes from File > Options > General > Username (or the signed-in Autodesk ID). (Ch 10)
- Four workset kinds [book]: User-Created; Views (view-specific elements such as dimensions, tags, text; can't be moved to other worksets; own a view via Project Browser right-click > Make Workset Editable); Families (one automatically per loaded family); Project Standards (materials, line styles, callout tags).
- Central/local flow [book]: Save As the project to a network location = central; each user opens it and Save As a local copy (rename). Save = local only. Synchronize with Central (Collaborate > Synchronize > Synchronize Now) publishes your changes and also pulls others' published changes. Synchronize and Modify Settings opens "Synchronize with Central" dialog: Central Model Location (Browse), check boxes to relinquish borrowed elements/worksets, Comment text. Reload Latest (shortcut RL) pulls others' changes without publishing. (Ch 10, "Synchronizing with Central File"; "Saving Methodology"; "Reloading Files")
- Ownership of elements [book]: owned elements are editable (shown blue); not-owned elements show a blue icon and a red highlight and cannot be modified. Ways to own: just edit or move the element (granted if nobody owns it), click the blue icon, or right-click > Make Elements Editable. If another user owns it: warning dialog, Place Request button; Check Editability Grants dialog (Check Now, Continue). The owner grants or denies; granted elements are borrowed, edited, synced, then relinquished. Display: View Control Bar > Worksharing Display options colour borrowed elements, owners, model updates, checkout status.
- Worksharing Monitor (separate program, subscription installs): panes Central File Access (Current User, Action, Progress, Step/Time Remaining; actions: Working in local, Opening central file, Working in central, Synchronizing with central, Reloading latest), Editing Requests, Notifications; Options dialog (General, Central File Access, Editing Requests, Notifications tabs); System Performance window (memory, CPU, disk; transparency 0-80%). Grey user name = not running Monitor. (Ch 10, "Monitoring the Worksharing Process")
- Implementer relevance: the app's multi-user model could map worksets to ownership groups with element-level borrowing and request/grant, central vs local with sync (merge on sync rather than lock for CRDT-like designs; book describes lock-based checkout) [knowledge].

## D7. Case studies (end of Ch 10; non-procedural)

- Case Study 1 (London residential, ~9000 m2) [book]: workflow Revit model (architecture, structure, MEP: basement structure first, then architecture, then MEP with pipes/ducts/fixtures/equipment), clash detection in Navisworks (MEP model appended, interferences reviewed), resolve clashes until overlap-free, coordination/presentation review, then construction documents (sheets) from Revit. Case Studies 2 (Kuala Lumpur mass rapid transit station) and 3 (multi-speciality hospital): Revit modelling/documentation plus Navisworks clash detection and coordination; each about 6 months. No parameters, tolerances or clash rules are given. (Case Studies)
- Implementer note: the only clash/coordination content in the slice is this workflow outline. Clash detection is performed in an external tool (Navisworks) on appended models; the book gives no in-Revit Interference Check procedure here.
---

# PART E. Cross-cutting rules, data model hints and IFC4 mapping

## E1. System types and classifications seen in the slice [book unless marked]

| Area | Names in book | Notes |
|---|---|---|
| Plumbing | Sanitary (waste); a water-distribution system type; vent (pipe type "Sanitary Vent-PVC" under Sanitary classification as a pipe type, not a classification) | Domestic hot/cold, storm, and vent as separate Revit classifications are [knowledge]; the book only shows Sanitary in tutorial |
| Fire protection | Fire Protection Wet; Fire Protection Dry | Heads are typed wet/dry and can only be assigned to the matching system |
| Hydronic | Closed-loop hydronic analysis (supply/return) via Calculations tab | Chilled/hot water supply/return are [knowledge]; only mentioned generically |
| Duct connectors (family) | Supply Air | Others [knowledge] |

## E2. Calculation settings

- Pressure drop: Colebrook (default) or Haaland. Flow: "Plumbing Fixture Flow" (fixture units -> flow). Hydronic closed-loop analysis toggle. Fitting loss via K coefficient (value or table) or none. Accessories: Pressure Drop, Flow, Loss Method parameters. [book]
- Fluid properties table keyed by temperature (viscosity, density). [book]
- Fixture unit parameters per fixture type: WFU, HWFU, CWFU. Per-section FU shown by System Inspector tag. [book]
- Not in slice: demand factors, load classification, diversity [absent].

## E3. Routing logic summary

- Conversion settings: per system classification: Main and Branch each {pipe type, middle elevation}. Branches can sit above/below main (obstacle avoidance).
- Layout solver inputs: system members, optional base(s) (outlets), solution type (Network/Perimeter/Intersections/Custom), conversion offsets. Output: ordered candidate layouts ("n of m"). Sprinkler special rule: elevation vs head orientation.
- Pipe slope: Slope Off/Up/Down with a value from a user-editable list; start offset + slope define the end elevation; Ignore Slope to Connect overrides at connections.

## E4. IFC4 class mapping [knowledge; the book has no IFC content]

| Revit concept | IFC4 |
|---|---|
| Pipe | IfcPipeSegment (RIGIDSEGMENT) |
| Pipe fitting | IfcPipeFitting (BEND, JUNCTION, TRANSITION, CONNECTOR, ENTRY, EXIT) |
| Pipe accessory (valve) | IfcValve; meters IfcFlowMeter; gauges IfcFlowInstrument |
| Plumbing fixture | IfcSanitaryTerminal (WATERCLOSET, URINAL, WASHHANDBASIN, BATH, SHOWER, SINK); floor drain IfcWasteTerminal (FLOORTRAP) |
| Sprinkler | IfcFireSuppressionTerminal (SPRINKLER) |
| Plumbing equipment | IfcPump, IfcTank, IfcBoiler/IfcWaterHeater-like (IFC4 has IfcBoiler; water heater as IfcBoiler/IfcTank by use) |
| Piping system | IfcDistributionSystem (DOMESTICCOLDWATER, DOMESTICHOTWATER, SEWAGE, FIREPROTECTION, VENT...) via IfcRelAssignsToGroup |
| Connector | IfcDistributionPort via IfcRelNests; links by IfcRelConnectsPorts |
| Pipe insulation | IfcCovering (INSULATION) via IfcRelCoversElements |
| Space | IfcSpace (area for sprinkler count from Qto_SpaceBaseQuantities) |
| Schedule | not IFC; derive from property sets / quantities |
| Tags/dimensions/text/callouts/sheets | drawing layer; outside IFC4 core |
| Worksets | no direct IFC; use IfcOwnerHistory/organisation per element or app-level metadata |
| Electrical (absent in slice) | IfcElectricDistributionBoard, IfcCableCarrierSegment, IfcCableSegment, IfcLightFixture, IfcOutlet, IfcDistributionCircuit (IFC4.3: IfcDistributionCircuit; IFC4: IfcDistributionSystem ELECTRICAL/CIRCUIT) - [knowledge] |

# PART F. Acceptance checklist (testable)

Prefix = chapter/section.

## Ch 7 Plumbing

- 7/Fixtures: Placing a face-hosted fixture requires a face under the cursor; with no host the click does nothing.
- 7/Fixtures: A wall-hosted fixture attaches to a vertical face and its Elevation parameter sets its height above the level (1'6" = 450 mm accepted in tutorial).
- 7/Fixtures: "Rotate after placement" prompts for an angle after the click; typing -90 then Enter rotates the placed WC.
- 7/Fixtures: Every plumbing fixture type exposes WFU, HWFU, CWFU; a fixture with hot and cold connectors reports both.
- 7/Fixtures: Instance System Classification is read-only; System Type/Name remain empty until the fixture is placed into a piping system.
- 7/Pipe type: Duplicating "Standard" creates "Standard 2"; renaming and routing preferences change only the new type.
- 7/Pipe type: A routing preference "Junction" change (e.g. vent tee) changes the fitting inserted at branches of pipes of that type.
- 7/Fitting type: Loss Method accepts None, K Coefficient, K Coefficient from Table; K Coefficient field enabled only for the first, table field only for the second.
- 7/Mechanical Settings: Adding a pipe size adds a row with nominal/inside/outside diameters; deleting removes it.
- 7/Mechanical Settings: A new fluid copies properties from its "based on" fluid; temperatures add/delete table rows.
- 7/Slopes: New Slope appends to the list that populates the Slope Value dropdown; Delete Slope removes it.
- 7/Calculations: Default pressure-drop method is Colebrook; Haaland selectable; default flow method is Plumbing Fixture Flow.
- 7/Pipe tool: Auto-connect ON joins to a snapped element at its elevation; OFF leaves ends unconnected.
- 7/Pipe tool: Slope Up/Down with a chosen value changes the end elevation by length x slope; tooltip shows start offset and slope.
- 7/Pipe tool: Inherit Elevation and Inherit Size copy elevation/diameter from the snapped element.
- 7/Pipe tool: Diameter and Offset in the Options Bar apply to the next segment; Lock keeps the specified elevation.
- 7/Insulation: Add Insulation requires type and thickness; Edit and Remove exist; fitting insulation is separate.
- 7/Accessory: Snap onto a pipe splits it and sizes the accessory to the pipe.
- 7/Create System: Selecting fixtures then Piping with system type Sanitary creates a named system; members appear in a dotted box.
- 7/Edit System: Add to System adds only selected fixtures; Finish Editing System commits; Escape does not.
- 7/Generate Layout: First default solution is shown immediately; Network option gives "n of m" counter; Next/Previous cycle layouts without changing model until Finish Layout.
- 7/Generate Layout: Pipe Conversion Settings Main/Branch offset (-1' / -305 mm) is the elevation of resulting pipes; type is the chosen pipe type.
- 7/Generate Layout: Finish Layout creates pipes and fittings per routing preferences; Generate Placeholder creates placeholder segments needing Convert Placeholder with a diameter.
- 7/Generate Layout: Adding fixtures to a system and generating again extends the network and recomputes "n of m".
- 7/System Inspector: Inspect shows flow/slope arrows on placeholders; hover tag shows section number and fixture units.
- 7/Show Disconnects: All unconnected pipe connectors flagged after routing; none after full connection.
- 7/Fabrication: Convert Design to Fabrication converts the whole selected connected run; changing Middle Elevation of one fabrication pipe applies to all connected horizontal pipework.
- 7/View range: Pipes below the cut plane are invisible unless view depth/bottom lowered.

## Ch 8 Fire protection

- 8/Sprinkler: Hosted sprinklers need a host face (ceiling/wall/slab/soffit, possibly linked); non-hosted use Offset for elevation.
- 8/Sprinkler: A dry sprinkler cannot join a wet system and vice versa.
- 8/Conversion: Wet and dry classifications have independent Main/Branch type and middle elevation; defaults 9'0" / 2750.
- 8/Layout: Pipe elevation must lie above pendent heads and below upright heads (book guideline); on the wrong side the layout tool finds no solution and the pipe will not connect as intended.
- 8/Layout: Solution types offered: Network (default), Perimeter, Intersections.
- 8/Schedule: Spaces schedule with calculated field Area/130 (or Area/12 metric) updates when a space area changes; grand totals shown; output not auto-rounded to whole heads.
- 8/Schedule: Formula identifiers are case sensitive.
- 8/Pipe type: Assembly Code D40 sets Assembly Description "Fire Protection".
- 8/Guideline: Per-head flow data must not be used to size pipe (analysis/scheduling only).

## Ch 9 Documentation

- 9/Temporary dimensions: Appear on draw, select, and place; not stored per view; clicking the conversion control creates a view-specific permanent dimension.
- 9/Temporary dimension settings: Walls: centerlines/center of core/faces/faces of core; Doors/Windows: centerlines/openings.
- 9/Permanent dimensions: View-specific (not shown in other views); updated when elements move; text value not directly editable; prefix/suffix allowed.
- 9/Lock: Locked dimension prevents changing its value; element may still move as a whole.
- 9/Dimension types: aligned, linear, angular, radial, diameter, arc length, spot elevation, spot coordinate, spot slope all exist; spot slope on flat shows "No Slope".
- 9/Spot slope: Representation Arrow (default) or Triangle; offset default 1/16 in (1.5875 mm).
- 9/Dimension style: Duplicating a dimension type allows tick mark and centerline pattern changes without altering the original.
- 9/Text: Text notes are view-specific; leader options none/one/two/curved; alignment left/center/right; spell check dialog.
- 9/Tags: Tag by Category requires a loaded tag family for the category; otherwise a prompt to load; tags are view-specific; visibility controlled per annotation category.
- 9/Tag All: "Only selected objects" option enabled only when a selection exists; per-category tag type and leader settings; categories with hidden visibility prompt a warning.
- 9/Callouts: Callout creates a child view under the parent's category; deleting the parent deletes the callout view; types Detail and Floor Plan (for current view); reference callout option links an existing view.
- 9/Callouts: Display Model options Normal/Halftone/Do not display; Detail Level Coarse/Medium (default)/Fine; Underlay level option.
- 9/Detail lines: View-specific.
- 9/Drafting view: Independent of model; scale selectable incl. custom.
- 9/Duplicate: Three modes (plain, with detailing, as dependent); dependent views follow the parent and have crop regions; navigation Go to Primary View.
- 9/Sheets: Title block family required; New Sheet lists title blocks; views added via dialog or drag; viewport shows title; view scale is a view property; activating a viewport allows model edits that update all views; pan active view changes only crop contents.
- 9/Sheets: Schedules on sheets can be split into sections and rejoined only with adjacent sections.
- 9/Guide grids: Named; spacing property; assigned per sheet.

## Ch 10 Families and worksharing

- 10/Forms: Extrusion depth = End - Start; Revolve angles default 0-360; void cuts any intersecting solid on finish; Swept Blend needs path plus two profiles; Blend needs two profiles and optional vertex editing.
- 10/Parameters: Dimension labelled with a parameter drives geometry; EQ constraint keeps reference planes symmetric; formulas such as Diameter / 2 recalc.
- 10/Connectors: Duct connector requires face, flow direction, system classification; size parameters can be associated to family parameters; flow associates to an instance parameter.
- 10/Load into project: loads family and starts placement.
- 10/Mass: Show Mass must be on to see masses; Place Mass can use face or work plane.
- 10/X-Ray: Only one form in X-Ray at a time; added profiles visible only in X-Ray; Dissolve deletes surfaces keeping profiles.
- 10/Worksharing: Default worksets: Shared Levels and Grids, Workset1; one user owns a workset editable at a time; Opened = No hides content; Delete workset asks delete-or-move elements.
- 10/Worksharing: Save is local; Synchronize with Central publishes and receives; Reload Latest receives only.
- 10/Worksharing: Editing a not-owned element is blocked with request/grant; elements checked out show owner colour overlays.
- 10/Worksharing: View-specific elements belong to Views worksets and cannot be moved to others.

# PART G. Open questions

1. Electrical content (panels, circuits, cable tray, conduit, lighting, panel schedules, load classification, demand factors) is NOT in lines 12820-18694. Which text range holds it? Likely before line 12820 (earlier chapters) given Ch 9 mentions cable tray and conduit tags and "Electrical Equipment Tags" as already known. The team should assign another slice.
2. Clash detection / interference check: no in-Revit procedure in this slice; only the Navisworks workflow outline in Case Study 1. Need a different source (Collaborate > Interference Check) or [knowledge].
3. Hydraulic / sprinkler calculations are not described; pipe sizing in plumbing (Pipe Sizing tool, sizing methods) is not described despite intro mentions. Which chapter covers it? (Possibly Ch 6 HVAC/piping in an earlier slice.)
4. The book lists Solution Type options inconsistently: Ch 7 lists Network/Perimeter/Intersections/Custom, Ch 8 lists three (no Custom). Implement all four with Custom as free-hand editing.
5. "Five solutions" for Network is described as dependent on complexity; the exact solver heuristic is not given. Tutorial counts: 5, 4 (2 of 5 then 4 of 4).
6. The book says all duplicate views are dependent on the original, which contradicts the three-mode description (only Duplicate as Dependent). Treat as: only dependent views stay linked.
7. Worksharing: review question says "once enabled, can be undone" without answer; text offers Disable Worksharing. Confirm behaviour (all worksets collapse?).
8. Domestic hot/cold vs vent classification and fixture-unit-to-flow tables are not shown; default tables in Revit are not described.
9. Exact default values for Colebrook roughness, fluid defaults, and pipe slope defaults are not given.
10. Shortcuts for most commands are not listed in the slice except RL (Reload Latest); other shortcuts here are [knowledge] and unverified.
