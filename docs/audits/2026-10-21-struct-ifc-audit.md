> Evidence file for `docs/gap-analysis-2026-10-21.md`. Written by a review agent by reading the code at `main` @ `dcf8382`; the agent did not run the app. The consolidated report lists which findings were re-checked independently (section 12). Line numbers refer to that commit.

# Structure-for-MEP + cross-cutting audit (IFC, save, collaboration, undo, phasing, units, families, types)

Repo: /home/claude/revit-alike, branch main, HEAD dcf8382 (Merge PR #57). Read-only review. No cargo run, no network git.
Scope per owner message: structure is deferred. Only (1) structure that MEP needs (trusses, supports/hangers, equipment pads,
openings/sleeves) and (2) the cross-cutting layer are traced. All "test evidence" below is from READING test source; tests were NOT executed here.
Paths are relative to /home/claude/revit-alike. Status words: WORKS (wired ribbon->tool->Command->result, with a test that I read),
PARTIAL, PLACEHOLDER (ribbon button/icon that only prints "not implemented"), ABSENT.

---------------------------------------------------------------------------------------------------
## 0. How a ribbon click reaches the model (applies to every row below)

* Ribbon groups are static tables: apps/desktop/src/ribbon.rs:109-157 (Architecture, STRUCTURE at :120-131, Steel/Precast tabs at :133-138 just reuse the same ids).
* Dispatch is one `match` on the command id: apps/desktop/src/main.rs:404-496 (`run_command`). Its fallback arm (main.rs:495) is
  `_ => self.message = format!("{label} is not implemented in this UI prototype")`.
* A tool then draws/picks in apps/desktop/src/model_view.rs (per-tool branches, e.g. Truss :951, Beam :983, Footing :898, WallOpening :863),
  calls an `app.*_click` handler (apps/desktop/src/model_edit.rs), which calls `Document::place_*` (crates/document/src/lib.rs), which calls
  `Document::transact` (lib.rs:1078) = validated batch of `Command`s, one undo step.
* Plan clicks are snapped to a 50 mm grid only (model_view.rs:942-956 `(v/0.05).round()*0.05`), NOT to grid lines/intersections or element snaps.
  docs/revit-ux-parity-audit.md:120 itself says "Not every tool uses it yet: Column, Beam, Family, Duct and Pipe keep their own picking" - confirmed in code.

### Test-evidence caveat (discrepancy D1 in section 7)
docs/revit-ux-parity-audit.md:3 says tools are "Verified by automated event tests (simulated mouse and keys)". In code, almost every structural/opening test
calls the handler directly (e.g. `app.truss_click(...)` model_edit.rs:1745, `app.footing_click(...)` :1615, `app.column_click`), NOT through egui pointer events.
Pointer-event injection (`egui::Event::PointerButton`) exists in only 4 places of model_view.rs (:1250, :1305 helper, :2030-2031 helper, plus the beam test :1250);
the Beam tool test (model_view.rs:1238-1262) is the only structural tool that I found driven by real events. Everything else = "handler + document" evidence, not "mouse" evidence.

---------------------------------------------------------------------------------------------------
## 1. Structure that MEP needs (the owner's reduced scope)

| Revit step (what MEP uses) | What the user does in the app | Status | Evidence (file:line) | Gaps / wrong vs Revit |
|---|---|---|---|---|
| Truss (Structure > Truss) | Ribbon Structure > Truss -> two clicks in a plan; options bar: panel length, member size, depth, elevation, material | PARTIAL (works, but only a flat Warren truss; no truss type, no host, no top/bottom chord attach to beams/roof) | ribbon.rs:123; main.rs:461; model_view.rs:951-956; Document::place_truss lib.rs:925; test model_edit.rs:1740-1760 (handler-driven: truss_click x2, 10 members, edit spacing, undo; IFC contains 10 IFCBEAM/IFCMEMBER) | Truss = a Category::Truss parent + child Category::Beam members (lib.rs:925-946). The truss itself is NOT exported (export.rs:198 skips it); members leave as plain IfcBeam/IfcMember with no IfcRelAggregates to a parent (no IfcElementAssembly/IFCTRUSS... none written). Truss drawn in a vertical plane along the 2 clicks only; no sloped roof truss, no family-based truss (Revit trusses are families with a Truss Type). Members snapped to 50 mm only |
| Beam System (joists for MEP supports) | Ribbon Structure > Beam System -> click outline corners, Enter/click first | WORKS (document+handler) | main.rs:466; model_edit.rs:1715-1738; lib.rs:1002 | Children exported as IfcBeam JOIST (lib.rs:108-112), parent dropped on export (export.rs:198) |
| Brace / diagonal support | Ribbon Structure > Brace = Beam tool with Usage=Brace (sloped member) | PARTIAL | main.rs:469; model_edit.rs:1250 (`run_command("brace")` asserts tool=Beam, usage Brace); export.rs test (export.rs:471) asserts IFCMEMBER .BRACE. | Brace is a Beam with `slope_ratio`; the "Brace Frame" ribbon button (ribbon.rs:123) = PLACEHOLDER (no arm in run_command) |
| Hangers / pipe supports / duct supports | Plumbing tab has buttons "Hanger", "Support" (ribbon.rs:81) | PLACEHOLDER | ribbon.rs:81 ("hanger","pipe_support"); no arm in main.rs:404-496 -> message "not implemented in this UI prototype" | No hanger/support element, no family in the library (crates/family/src/library.rs has only: Lever handle, Door single flush, Door arched, Window fixed w/ mullions, Fan coil ducted; library.rs:26,95,163,223,316), no "host on beam/slab/truss", no auto-hangers along runs, no support category in schedules/IFC (IfcPipeSupport/IfcDiscreteAccessory/IfcMechanicalFastener never written). docs/revit-book-review.md:M12 lists "hangers" as P3 missing - consistent |
| Equipment pad / housekeeping pad | Structure > Foundation > Isolated: click in plan, options bar width/length/thickness/top offset/material | PARTIAL | main.rs:467; model_view.rs:898; Document::place_footing lib.rs:844; test model_edit.rs:1610-1641 (handler `footing_click`, volume 1.5 m3, edit thickness, IFC contains IFCFOOTING + `.PAD_FOOTING.`, undo) | A pad is `Category::Footing` with `Geometry::Pad`, rectangular only, and is NOT hosted by/associated with the equipment (moving the equipment does not move the pad; no "pad under equipment" tool). Exported as IfcFooting PAD_FOOTING (lib.rs:1285-1290) - IFC-wise an equipment base should be IfcSlab/IfcBuildingElementProxy or IfcFooting; acceptable but unusual. Floor-standing top offset must be typed (+value) as docs say "negative: below" (lib.rs:204). Foundation slab ("Slab" under Foundation, ribbon.rs:126; main.rs:468) = Slab kind Foundation, polygon, exported IfcSlab BASESLAB (test model_edit.rs:1599 reads `.BASESLAB.`) |
| Structural Floor / Wall as host of MEP | Ribbon Structure > Floor (`structural_floor`) / Wall (`structural_wall`) | Floor: PARTIAL (identical tool to Architecture Floor: main.rs:450 maps both ids to Tool::Slab/Floor). Structural Wall: PLACEHOLDER | main.rs:450; no "structural_wall" arm (main.rs:404-496) | No "Structural Usage"/"Is Structural"/Load Bearing field on Element (crates/document/src/lib.rs:Element, no such field) nor in IFC (no Pset_WallCommon.LoadBearing/IsExternal; Pset_WallCommon only written when `fire_rating_min` is Some - lib.rs:1070 - and the app always passes None: export.rs:164,171,180,192 for slabs, WallChain default for walls). Revit has Structural Usage on walls (Bearing/Shear/Structural combination) |
| Openings/sleeves in walls | Architecture > Opening > Wall: click wall at one end then other end (sill/head in options bar) | WORKS | ribbon.rs:117; main.rs:456; model_view.rs:863; model_edit.rs:444 `wall_opening_click`; Document::place_wall_opening opening.rs:35; tests model_edit.rs:2079 (opening exported as IFCOPENINGELEMENT + IFCRELVOIDSELEMENT), model_view.rs:2337 (elevation sketch), :2390 (drag in elevation), :2708 (tool states) | Rectangle only (opening.rs:154 `[offset,width,sill,height]`), no round sleeve, no opening in a curved/layered wall layer, no host = beam/column ("openings in beams and columns missing" docs/revit-book-review.md A13, confirmed: Opening geometry variants are InWall/Shaft/Area only, native.rs:40-61). Rule: an MEP-driven sleeve does not exist: the crate `openbim_place::penetrations` (firestop register, round sleeves) is NOT used by the app - `wall_penetrations: vec![]`, `firestops: Default::default()`, `floor_penetrations: vec![]` (export.rs:290-291). |
| Openings in floors (shaft, vertical opening) | Architecture > Opening > Shaft (outline + base/top level) or Vertical (click slab, Tab, outline) | WORKS | ribbon.rs:117; main.rs:454-455; opening.rs:47,91; tests model_view.rs:2651,2708; model_edit.rs:2079; opening.rs:185-333 (shaft cuts slab at its top level, notch, roof cut) | Exported as IfcOpeningElement + IfcRelVoidsElement for slabs (lib.rs slab_voids path; test import.rs:810-870 asserts 5 IFCOPENINGELEMENT). A ceiling's hole is not in the IFC (docs/revit-ux-parity-audit.md:180). IFC import of IfcOpeningElement is skipped (import.rs:869 expected map). No MEP-clash driven opening (no "Opening by MEP crossing wall" tool, ribbon "clash"/"interference_check" = PLACEHOLDER, ribbon.rs:151) |
| Equipment/support hosting levels | Level + Grid tools | WORKS (Level: model_view.rs:845, Grid :824; tests model_view.rs:1451) | -- | Grid is not exported to IFC (no IfcGrid, export.rs:198 silent skip) |
| Structural members as MEP hosts (beam "Cut with" etc.) | -- | ABSENT | -- | Beams are cut back to columns (join_ends, lib.rs Element.join_ends; test model_edit.rs:1684) but no MEP-vs-structure join/cut, no coping |

Mouse-only verdict for the MEP-relevant structure: a user can (a) draw a truss, (b) draw a beam system and braces, (c) put a pad footing under equipment by typing an offset, (d) cut rectangular wall openings and floor/roof shafts, all with mouse + options bar. A user can NOT place hangers/supports, structural walls, pipe sleeves, or openings in beams/columns. The owner's MEP-support story is the biggest hole (hangers/supports PLACEHOLDER).

### 1a. Deferred structure (status only, not traced deeply)
| Revit capability | Status | Note |
|---|---|---|
| Rebar (beam only) | PARTIAL | Category::Rebar hosted only on a rectangular beam (Geometry::OnMember, lib.rs:985; crates/document/src/rebar.rs), 2 top bars + `count` bottom bars + stirrups; exported as IfcReinforcingBar sweep (lib.rs:1205); not importable (import skips IFCREINFORCINGBAR, reader.rs:908). No rebar on columns/slabs/walls/footings, no hooks, no area/path reinforcement (cover = `elevation_m`) |
| Structural columns at grid intersections | PARTIAL | Column tool = one click, 50 mm snap only (model_view.rs:946-950), no snap to grid intersections, no "At Grids" / multi-select-grids mode |
| Pile / pile cap | ABSENT | no IfcPile anywhere; no Category |
| Analytical model, loads, load cases/combinations, boundary conditions | ABSENT | only a ribbon icon `analytical_model` (icons.rs:24) and a keymap string (crates/keymap/tests/keymap.rs:299); no IfcStructural* in any crate (grep 0) |
| Steel connections / anchors / shear wall / Framing / Brace Frame / Structural Section | PLACEHOLDER | ribbon.rs:123-134; no arms in run_command |
| Structural schedules / material takeoff | PARTIAL | Schedules exist for Columns, Framing, Foundations (docs/revit-ux-parity-audit.md:186); "Material Takeoff" not built (:199) |
| IFC Qto for members | ABSENT | only Wall/Slab/Door/Window/Space get Qto_*BaseQuantities (lib.rs:1067,1323,quantities.rs); no Qto_BeamBaseQuantities/Qto_ColumnBaseQuantities/Qto_FootingBaseQuantities |

---------------------------------------------------------------------------------------------------
## 2. IFC coverage matrix (writer = crates/ifc/src/lib.rs fed by apps/desktop/src/export.rs; reader = crates/ifc/src/reader.rs + apps/desktop/src/import.rs)

Schemas: writer emits IFC4X3 (default, export.rs:~395 `Schema::ALL`), IFC4, IFC2X3 (lib.rs:839-856; header via step.rs). Reader accepts IFC2X3/IFC4/IFC4X3 headers (reader.rs; tests crates/ifc/tests/reader.rs:130,275,309,418; import test import.rs:810 loops all 3 schemas).
IFC2X3 writer drops: type objects (lib.rs:1249), service networks (lib.rs:967), `IfcRelConnectsPathElements`, walls/doors as plain boxes (crates/ifc/tests/ifc.rs:336).

### 2a. Entities the writer emits, as fed by the APP (not what the crate could do)
| Discipline | Entity | Predefined type | Properties / psets | Quantities | Relationships | Type object | Fed by app? |
|---|---|---|---|---|---|---|---|
| Spatial | IfcProject, IfcSite ('Site'), IfcBuilding ('Building'), IfcBuildingStorey (per Level) | ELEMENT | project carries `OpenBIM_Document` pset = the whole native file (lib.rs:903-908) | -- | IfcRelAggregates (lib.rs:~925-944), IfcRelContainedInSpatialStructure per storey (lib.rs:~975) | -- | yes. Site name/lat-long NOT written: `georef: None` (export.rs:292), doc.site() only yields a warning "not exported" (export.rs:432). No IfcMapConversion/ProjectedCRS in app output |
| Arch | IfcWall (one per drawn chain, one extruded body per mitred segment) | STANDARD in type | `OpenBIM_Element`, `Pset_WallCommon{FireRating}` only if fire rating set (never from app) | Qto_WallBaseQuantities (lib.rs:1067) | IfcRelVoidsElement to IfcOpeningElement (lib.rs:~1100-1125), IfcRelAssociatesMaterial -> IfcMaterialLayerSetUsage for layered types (lib.rs:1060-1063) | IfcWallType + IfcMaterialLayerSet (walls.rs; test crates/ifc/tests/walls.rs:86,278) | yes; `IfcRelConnectsPathElements` NOT written by app (`wall_joins: vec![]`, export.rs:292) |
| Arch | IfcDoor, IfcWindow (family instance) | operation type from `swing` (families.rs:325-329), else NOTDEFINED | `OpenBIM_Family`, `OpenBIM_Instance` (custom sets; mapping table empty: `pset_map: Default::default()`, export.rs:294) | Qto_DoorBaseQuantities / Qto_WindowBaseQuantities (families.rs:346-347) | IfcRelFillsElement (families.rs:359), IfcRelVoidsElement | IfcDoorType/IfcWindowType via IfcRelDefinesByType + IfcRepresentationMap/IfcMappedItem (families.rs:305,376) | yes. No Pset_DoorCommon/Pset_WindowCommon, no IsExternal/FireRating |
| Arch | IfcSlab | FLOOR / BASESLAB / LANDING (lib.rs:1304) | OpenBIM_Element | Qto_SlabBaseQuantities (lib.rs:1323) | voids from shafts/vertical openings | IfcSlabType (FLOOR only, export.rs:379 `TypeClass::Slab("FLOOR")`) | yes; IfcSlab for a Roof kind is IfcRoof |
| Arch | IfcRoof | FLAT/SHED/GABLE/HIP_ROOF | -- | none | cut-in-shape (no opening element) | none | yes; faceted brep |
| Arch | IfcCovering | CEILING (also FLOORING in crate) | -- | none | -- | none | ceiling yes |
| Arch | IfcStair (+LANDING IfcSlab), IfcRailing | STRAIGHT_RUN_STAIR; GUARDRAIL | Pset_StairCommon (lib.rs:1488) | none | -- | none | yes. Only Pset_StairCommon from the Pset_*Common family is written by the app |
| Arch | IfcSpace | -- | -- | Qto_SpaceBaseQuantities in crate (lib.rs:1531) | IfcRelSpaceBoundary 2nd level in crate (lib.rs:1576 write_boundaries) | -- | NOT FED: `rooms: vec![]` (export.rs:291); Category::Room skipped silently, no warning (export.rs:198). Rooms exist only in the native records |
| Arch | IfcGrid | -- | -- | -- | -- | -- | ABSENT (writer has none; Grid skipped silently, export.rs:198) |
| Struct | IfcColumn | COLUMN | OpenBIM_Element | none | IfcRelAssociatesMaterial -> IfcMaterial('Concrete'/'Steel'/'Timber') | IfcColumnType | yes |
| Struct | IfcBeam / IfcMember | BEAM, JOIST, NOTDEFINED / PURLIN, BRACE (lib.rs:106-112) | OpenBIM_Element | none | material as above; truss/beam-system parent not represented | IfcBeamType (BEAM/JOIST/NOTDEFINED) | yes |
| Struct | IfcFooting | PAD_FOOTING, STRIP_FOOTING | OpenBIM_Element | none | material | IfcFootingType | yes |
| Struct | IfcReinforcingBar | MAIN / SHEAR | -- | none | contained in storey, no IfcRelAggregates to host | none | yes (beam rebar only) |
| Struct | IfcPile, IfcStructural*, IfcElementAssembly | -- | -- | -- | -- | -- | ABSENT |
| MEP | IfcDuctSegment RIGIDSEGMENT, IfcPipeSegment RIGIDSEGMENT, IfcCableCarrierSegment CABLETRAYSEGMENT/CONDUITSEGMENT, IfcCableSegment CABLESEGMENT (lib.rs:809-816) | per domain | `OpenBIM_Mep` (size, `System` label) (lib.rs:1735) | none (no Qto_DuctSegmentBaseQuantities etc.) | IfcDistributionPort nested via IfcRelNests (lib.rs:1629 `nest`), IfcRelConnectsPorts for every link (lib.rs:1828), IfcDistributionSystem + IfcRelAssignsToGroup (lib.rs:1823-1838 (IFCDISTRIBUTIONSYSTEM at :1835)) | none | Ducts/pipes yes. Cable tray / conduit / cable: crate yes, but app has no doc category for them in export.rs (only Duct/Pipe elements: export.rs:200-209); electrical nodes/cables only in the native records (no IFC entity: grep of export.rs finds no ElecNode) |
| MEP | IfcDuctFitting / IfcPipeFitting / IfcCableCarrierFitting / IfcCableFitting | BEND/REDUCER(TRANSITION)/TEE/JUNCTION (lib.rs:819-828) | OpenBIM_Mep {PartCode, System, BendRadius, Angle, Legs} | none | ports as above | none | yes. Fitting bodies are straight stubs per leg - no swept bend (lib.rs header note) |
| MEP | Equipment: placed families -> IfcUnitaryEquipment / IfcFan / IfcPump / IfcChiller / IfcAirTerminal ... (crates/family/src/mep.rs:50-70 `ifc_class`) with predefined NOTDEFINED (families.rs:331-337) | NOTDEFINED only | OpenBIM_Family, OpenBIM_Instance, lighting psets for light fixtures (families.rs:255-283) | none | connector ports (families.rs / item_ports) | Ifc*Type + shared representation map | yes for family instances (library has only fan-coil as MEP equipment). A run connected to a non-family "Equipment" gets a bare `IFCFLOWTERMINAL` placeholder with `OpenBIM_Equipment{Placeholder}` (lib.rs:1798-1801) |
| MEP | IfcDistributionSystem | system.ifc_system() VENTILATION etc. (family/mep.rs:235-254) | -- | -- | IfcRelAssignsToGroup of segments+fittings ONLY (lib.rs:1823-1838 (IFCDISTRIBUTIONSYSTEM at :1835)): equipment is not assigned to its system | -- | PARTIAL, see wrong-behaviours W1/W2 |
| MEP | IfcSystem / named "Mechanical System" objects the user creates in Systems browser | -- | -- | -- | -- | -- | NOT exported (docs say so, docs/revit-ux-parity-audit.md:34; `MepData.systems` is not read in export.rs) |
| MEP | Penetrations (IfcOpeningElement with service/firestop data) | -- | OpenBIM_Penetration (lib.rs:1120,1380) | -- | IfcRelVoidsElement | -- | NOT FED (vectors empty, export.rs:290-291) |
| Any | IfcMaterial | -- | -- | -- | IfcRelAssociatesMaterial (lib.rs:1221-1235) | -- | Only for beam/column/slab/roof/railing/footing, with the name of the enum (Concrete/Steel/Timber) - no material library, no properties, no colour |
| Any | GlobalId | -- | -- | -- | -- | -- | see section 3 |

### 2b. What the reader accepts (crates/ifc/src/reader.rs:139-156, apps/desktop/src/import.rs:206-233)
| Class | Accepted? | How |
|---|---|---|
| IFCWALL / IFCWALLSTANDARDCASE | yes | extruded profile only; layered wall type from IfcMaterialLayerSet (test import.rs:890) |
| IFCBEAM, IFCMEMBER | yes -> Category::Beam | extruded rect/circle/I profile; predefined -> Usage |
| IFCCOLUMN | yes | vertical prism, section rect/circle/I, top must coincide with a level (import.rs:279-282, else skipped) |
| IFCSLAB (not LANDING), IFCCOVERING CEILING | yes -> Slab | |
| IFCFOOTING | yes | STRIP -> WallFooting needs a wall above it (import.rs:383) |
| IFCPIPESEGMENT, IFCDUCTSEGMENT | yes | round pipe, rect/round duct only (import.rs:257-260) |
| IFCCABLECARRIERSEGMENT | read by the reader (reader.rs:152) but import.rs:206-233 has no arm -> reported "no such element in the app" |
| IFCROOF, IFCSTAIR, IFCRAILING | NOT read from foreign files (import.rs:222-227; rails/others counted in skipped) |
| IFCDOOR, IFCWINDOW, IFCOPENINGELEMENT, IFCRELVOIDSELEMENT, IFCRELFILLSELEMENT | NOT imported: counted as skipped (import.rs:866-870 expected map lists IFCDOOR 1, IFCWINDOW 1, IFCOPENINGELEMENT 5, IFCRAILING 3) |
| Fittings, equipment (IfcFan, IfcUnitaryEquipment, IfcAirTerminal ...), terminals, IfcDistributionPort, IfcRelConnectsPorts, IfcDistributionSystem | NOT imported by IFC semantics. Fittings/equipment/ports are only restored when the file carries the app's own `OpenBIM_Document` records (import.rs:810-870: `report.restored == want.len()`; tests import.rs:731,753) |
| IfcSpace, IfcZone | NOT imported |
| Geometry other than IfcExtrudedAreaSolid (Brep, MappedItem, SweptDisk, BooleanResult) | NOT imported (reader.rs:473 BooleanResult -> None; test reader.rs:337 "boolean results are not imported as uncut operands") |
| Units | only metre or millimetre SI length (reader.rs:563-596; tests reader.rs:275,309). Centimetre, foot/inch (IfcConversionBasedUnit) -> hard error "missing, ambiguous or unsupported project length unit" |
| Property sets | read generically (IfcPropertySingleValue only: reader.rs:539); IfcElementQuantity not read; IfcPropertyEnumerated/Table/List not read |
| GlobalId | read (`Member.guid`, reader.rs:85) but NEVER used by import.rs (grep guid import.rs: 0 hits) |

### 2c. What Revit's IFC export normally has and this app does not write (from the app, as fed today)
* Pset_*Common (Pset_WallCommon IsExternal/LoadBearing/FireRating/ThermalTransmittance, Pset_SlabCommon, Pset_BeamCommon, Pset_ColumnCommon, Pset_DoorCommon, Pset_WindowCommon, Pset_DuctSegmentTypeCommon, Pset_PipeSegmentTypeCommon, Pset_FanTypeCommon ...). Written: Pset_StairCommon, Pset_LightFixtureTypeCommon, Pset_ManufacturerTypeInformation, Pset_SpaceLightingRequirements (crate only). Mapping table `PsetMap` exists (crates/ifc/src/pset_map.rs) but the app supplies an empty one.
* Base quantities for beams, columns, footings, roofs, stairs, ducts, pipes, fittings, equipment (Qto_*). Only wall/slab/door/window/space.
* IfcRelConnectsPathElements for walls (crate has it; app passes none), IfcRelSpaceBoundary / IfcSpace (not fed), IfcGrid, IfcRelAggregates for trusses/beam systems/stairs parts, IfcRelAssociatesMaterial to real named materials, IfcSystem/IfcRelAssignsToGroup for equipment, IfcZone, classification references, IfcRelDefinesByType for ducts/pipes/fittings (no types for MEP runs at all), type objects for equipment predefined types (all NOTDEFINED), IfcMapConversion (`georef: None`), IfcRelServicesBuildings, IfcOwnerHistory in IFC4 (not needed), IfcDistributionPort with SystemType/FlowDirection pset attributes (flow direction is written on the port, lib.rs:1620 `write_port`).
* Revit-style Pset_ "Reference" / "Mark" / "Level" instance identifiers: not written; Name is the element name only.

---------------------------------------------------------------------------------------------------
## 3. GlobalId stability on re-export

* Products: stable. `Out::gid(element_number, part)` = 48-bit project key | 16-bit part | 64-bit element number (crates/ifc/src/guid.rs:1-47; lib.rs:638-644). Tests: crates/ifc/tests/ifc.rs:68 (plain wall exports identically), :101 (edited wall keeps id, new gets own), :8,:17 (encoder/collision), crates/ifc/tests/reader.rs:178 (reader sees same wall id across exports); app tests assert byte-identical output twice (export.rs:486,510 `assert_eq!(out.text, export_ifc(...).text)`), and the exported text contains no clock (timestamp is caller supplied).
* NOT stable (code reading, no test):
  - Everything that uses `gen_gid()` (lib.rs:645-648: IfcProject, IfcSite, IfcBuilding, every IfcBuildingStorey, every IfcRelAggregates / IfcRelContained.../IfcRelDefinesBy*/IfcRelConnectsPorts/IfcRelAssignsToGroup, every IfcPropertySet, IfcElementQuantity, IfcDistributionSystem) takes the next value of a running counter `aux`. Adding or deleting ONE level, one MEP system, or a pset-bearing element shifts the ids of all later items. Concretely: Project is aux=1; if "Include this app's own data" is ticked the OpenBIM_Document pset+rel consume aux 2-3, so Site/Building/Storeys get different GlobalIds with the checkbox on vs off, and adding a level shifts every following storey's GlobalId. Storeys, IfcDistributionSystem and relationships are the objects BCF/IDS/coordination tools reference most. Revit keeps storey ids tied to the level's UniqueId.
  - Foreign GlobalIds are discarded on import (import.rs never reads `Member.guid`); re-export writes ids derived from the document element number. A model round-tripped Revit -> app -> IFC does not keep Revit's GlobalIds.
  - Element numbers = `max id + 1` (`Document::next_id`, lib.rs:566): deleting the newest element and placing another re-uses the same element number and thus the same GlobalId (Revit never re-uses a UniqueId).
  - `gen_gid` ids use element value (0xFFFFFFFF<<32 | aux) in the element-number namespace: collision only if an element author field is 0xFFFFFFFF (theoretical).

---------------------------------------------------------------------------------------------------
## 4. Round-trip evidence (tests I read, not run)

| Claim | Evidence | Real scope |
|---|---|---|
| Export then import returns "nothing lost" in IFC2X3/IFC4/IFC4.3 | apps/desktop/src/import.rs:810-870 (`a_file_the_app_wrote_comes_back_with_nothing_lost_in_every_schema`) | Works because the whole model is embedded as text records in the project pset `OpenBIM_Document` (lib.rs:903-908; native::records) and re-applied (`report.restored == want.len()`, :858). It is a private-format round trip carried inside IFC, not IFC semantic round trip. Doors/windows/openings/rails/fittings/equipment are all counted as skipped from the IFC part (:866-870) |
| Without the app's own data only geometry + types return | import.rs:875-887 | even then a wall end that stops at another wall's face changes length (:884) |
| Edited elsewhere is detected (fingerprint) | import.rs:919 (`changes_made_in_another_program_are_taken_and_the_rest_is_kept`), export.rs `fingerprint` (:~351) | only for beams/columns/slabs/footings/walls: `OpenBIM_Element` is written for member_entities only (lib.rs:1237-1245) |
| Import is one undo | import.rs:948 | |
| IFC checked by an independent reader | README.md:15 + spikes/* (Python ifcopenshell scripts) | I did not run them; CI only (".github/workflows/ci.yml" referenced by README) |
| Fittings/ports/connectivity survive | import.rs:731,753,788 | again via native records; from a foreign IFC the reader drops IFCDISTRIBUTIONPORT (import.rs:602-604 `others` filter) |

---------------------------------------------------------------------------------------------------
## 5. Behaviours that look WRONG versus Revit

* W1. A duct/pipe's system chosen in the UI is lost on IFC export unless it touches a fitting: export.rs:205,207 create every run with `System::Other`; the document's per-run system (`MepData.run_systems`) is never read in export.rs (grep: 0 hits). `System::Other.ifc_system()` = "NOTDEFINED" (family/mep.rs:254) and the IFC gets a system literally named "Other 1". Revit exports the run's System Type/Classification.
* W2. Equipment is never assigned to its IfcDistributionSystem (by_system gets segments and fittings only, lib.rs:1823-1838 (IFCDISTRIBUTIONSYSTEM at :1835)), and the Systems-browser "Mechanical System" objects are not written as IfcSystem/IfcDistributionSystem (docs/revit-ux-parity-audit.md:34 admits it).
* W3. `Category::Room`, `Grid`, `Dimension`, `Tag`, `Section`, `Elevation`, `Truss`, `BeamSystem` are dropped silently - no warning is added (export.rs:198 returns `{}`), whereas other skipped things do warn (export.rs:213-214). A user exporting a room-based model gets no IfcSpace and no message.
* W4. Wall joins: `Wall Joins` allow/disallow are persisted (native records) but IfcRelConnectsPathElements is never produced from the app (export.rs:292 `wall_joins: vec![]`), contradicting docs/revit-ux-parity-audit.md:71 ("round-trips through IFC with OpenBIM data" is only true through the private records).
* W5. Project location: Site (lat/long/north) can be set in the document but never written (export.rs:292 `georef: None`, :432 warning), so the file has no true north / map conversion although the crate supports IfcMapConversion (lib.rs:~884-896).
* W6. Material: structural `Material` is a 4-value enum (lib.rs:Material) exported as IfcMaterial('Steel') etc.; wall layers carry a free-text material name. No material library, no thermal/structural properties, no appearance - Revit materials have class, identity, appearance, physical/thermal assets.
* W7. Structural Floor and Architectural Floor are the same tool and the same data (main.rs:450); neither carries a structural flag. Foundation slab is a SlabKind and is exported BASESLAB; structural walls have no tool.
* W8. IFC import of a foreign file rejects the whole file when the unit is centimetre or foot/inch (reader.rs:565-596), and silently ignores doors/windows/equipment/openings - a Revit-exported MEP IFC would import only straight pipe/duct and no connectivity.
* W9. A whole-state clone is kept for every undo step (lib.rs:1267, 1292), unbounded (no cap found, grep for a limit: none). Large models will cost O(model x edits) memory and time per edit (see section 6).
* W10. Status bar prints `Workset: Mechanical (sample)` as a constant string (main.rs:597) although no worksets exist.

---------------------------------------------------------------------------------------------------
## 6. Cross-cutting layer verdicts

| Area | Verdict | Evidence | Detail |
|---|---|---|---|
| Working file format | WORKS (single-user, snapshot only) | apps/desktop/src/file.rs:7-47; crates/document/src/native.rs:7-136 | `.obim` = UTF-8 text, one `E|...` line per element plus `T|/L|/P|/Z|/X|/N|...` MEP records (native.rs:130-200). Header `F|obim-records-1` (native.rs:9, check :384). Not an operation log. Saved atomically via `<file>.saving` + rename (file.rs:60-68). Test: file.rs:141 (`a_saved_model_opens_equal_with_no_undo...`) |
| Op-log / snapshot / crash recovery | ABSENT in the app | crates/oplog is a standalone crate (binary records + CRC-32, torn-tail detection, snapshot, merge: oplog/src/lib.rs:1-4,319-470,540-595). grep for `openbim_oplog`/`openbim-oplog` outside the crate: 0 hits (apps/desktop and crates/document do not depend on it). No autosave, no backup (.bak), no lock file, no recovery prompt (grep autosave/recover/backup in apps+crates: 0). A crash loses everything since the last manual Save | The task statement "the working file is the app's own operation-log + snapshot format" is NOT what the code does: the working file is a full text snapshot written on Save; the op-log engine exists but is unused by the app |
| Versioning/migration | PARTIAL | native.rs:9,384 | one version string, refused if different ("not in a format this version reads"); no migration path; unknown fields? `fields()` map lookup (native.rs:377) - missing key behaviour per field not audited |
| What the .obim holds | PARTIAL | native.rs:111-136, 173-240 | all elements, site, MEP fittings/links/taps/run_systems/zones/endpoints/settings/systems/schedules/view settings/sheets/elec nodes+cables are in `MepData` records. Not saved: undo history (by design), UI layout, selection |
| Undo/redo | WORKS but heavy | crates/document/src/lib.rs:555-562 (history/future), :1267 (push on transact), :1275-1296 (undo/redo), tests lib.rs:1331 (redo), :1356 (create/resize/undo atomic), :1400 (ids stable after undo); UI main.rs:578-580 Ctrl+Z / Ctrl+Y | One step per `transact` batch; `merge_steps_since`/`roll_back_to` for composite tools (lib.rs:1060-1072). Snapshot-per-step with no cap; no named steps (Revit's Undo list shows the command name); file open clears the history (file.rs:30). Redo after new edit cleared (as Revit) |
| Multi-user / central model / worksets / borrowing | ABSENT (planned only) | no code: grep borrow|ownership|heartbeat|shared folder|change request in crates+apps: 0 relevant hits; `Document::transact` only does an optimistic revision check (lib.rs:1078-1084) with the comment "centralized edits will need additional per-element semantic checks"; ribbon `workset` (ribbon.rs:155) -> "not implemented"; status text fake (main.rs:597). Plans: docs/revit-style-ui/03-implementation-architecture.md:35, docs/revit-style-ui/05-backlog.md:17-19 (COLLAB-001..003 P0/P1), docs/revit-style-ui/02-workflows-and-acceptance.md:19-21 | Only primitive that could be reused: `openbim_oplog::merge` (oplog/src/lib.rs:570, per-property key conflict detection, whole-transaction drop) - spike, unit-tested in-crate, not connected |
| Links (Revit/IFC link, linked-model visibility, copy/monitor) | PLACEHOLDER | ribbon.rs:140,156 `link_model` -> no arm in main.rs; docs/revit-book-review.md:M13 "Link Model button only; no linking yet" | "Import IFC" is an import into the host model (merges elements), not a link: no read-only linked instance, no reload, no per-view link visibility |
| Phasing | ABSENT | ribbon.rs:155 `phase` -> "not implemented"; no Phase field on Element (lib.rs Element struct), none in native records, none in IFC (no IfcRelAssignsToControl/phase pset), grep `phase` hits in crates are unrelated (MCP handshake, 3-phase electrical). docs/revit-style-ui README lists "phases" only as ribbon text |
| Design options | ABSENT | grep "design option" in code: 0 relevant hits; docs/revit-book-review.md:A17 |
| Units | PARTIAL (fixed) | Model unit = metres internally, UI is hard-coded mm (e.g. main.rs:459 etc., panels.rs fields `*_mm`); no Project Units dialog (docs/revit-book-review.md:A14 "Fixed metric"); IFC writer always SI metre/m2/m3/radian (lib.rs:874-881); reader only m/mm (see 2b). Family expression engine has dimensions (crates/family/src/expr.rs: LENGTH, AREA, FLOW, PRESSURE...; tests crates/family/tests/units.rs) but the app does not expose unit/format settings |
| Family editor | ABSENT (engine only) | no desktop module (apps/desktop/src list has no family editor); grep "family editor|load family|rfa" in apps/crates: 0 hits; engine crates/family (solver, forms, meshing, MEP connectors, shared params, catalogue TSV loader crates/family/src/catalogue.rs). Library is 5 hard-coded families (library.rs:26,95,163,223,316), the only MEP equipment is "Fan coil unit, ducted". Catalogue (type table) loader is not wired to the desktop (grep `catalogue` in apps/desktop: only the calc-size catalogue in compute_ui.rs:59) | FAMILY-001 "graphical editor ... in the first interface" (docs/agent-team/gap-review.md table) is still open. Owner cannot add hangers, valves, pumps, AHUs, sprinklers except in Rust code |
| Types / Edit Type | PARTIAL | apps/desktop/src/type_editor.rs; crates/document/src/types.rs; `Element::can_have_type` = Beam, Column, WallFooting, Slab, Rebar, Wall, Footing only (lib.rs:Element::can_have_type) | No types for Duct, Pipe, fittings, family instances' own types are solved by name from the library (FamilySpec.type_name), no duplicate/rename family types in UI; docs/revit-book-review.md:M1 lists duct/pipe Type Selector as not done |
| Materials | PARTIAL | see W6 | wall layer materials are strings; no library/Materials dialog |
| Schedules / quantities | PARTIAL | crates/document/src/schedule.rs (27 categories, docs/revit-ux-parity-audit.md:186-199); schedule definitions are saved in `MepData.schedules` (mep.rs:179); no Material Takeoff, no calculated values, no hangers category (none exist) |
| Compare to docs | see section 7 | | |

---------------------------------------------------------------------------------------------------
## 7. Discrepancies between self-reported docs and code

| # | Doc statement | Code reality |
|---|---|---|
| D1 | docs/revit-ux-parity-audit.md:3 "Verified by automated event tests (simulated mouse and keys)" | Structural tools are tested through handler calls (`truss_click`, `footing_click`, `column_click`, `beam_system` via `slab_click`: model_edit.rs:1203,1610,1715,1740). Only the Beam tool has an egui pointer-event test (model_view.rs:1238-1262); plan tools use a different 50 mm snap than the document/test path |
| D2 | docs/revit-ux-parity-audit.md:6 "Working tools: ... Column, Beam, Brace, Floor/Slab, Foundation, Wall Footing, Beam System, Truss, Rebar ..." | True for those; but the ribbon also shows Structural Wall, Brace Frame, Framing, Shear Wall, Connection, Anchor, Cast-in Anchor, Section (Steel tab), Hanger, Support, Curtain Wall, Link Model, Worksets, Phases, Clash Detection, Interference Check as normal buttons that only say "not implemented in this UI prototype" (ribbon.rs:81,111,123-134,151-156 vs main.rs:495). The doc's closing line (:258) names some of them but not Structural Wall, Brace Frame, Framing, Hanger, Support |
| D3 | docs/revit-ux-parity-audit.md:71 Wall joins "round-trips through IFC with OpenBIM data" | Only through the private `OpenBIM_Document` pset; IfcRelConnectsPathElements not written from the app (export.rs:292) |
| D4 | docs/revit-ux-parity-audit.md:95,131,142,146,177,180 "IFC: ... the app's own file brings it back unchanged" | true only because of the embedded native records (import.rs:810-870); a foreign copy of the same IFC loses them (reader skips IFCROOF/IFCSTAIR/IFCRAILING/IFCOPENINGELEMENT, import.rs:222-227,866-870) |
| D5 | docs/revit-ux-parity-audit.md:34 and docs/revit-book-review.md:M5 "Create Mechanical/Piping System ... in the IFC file's own records" | accurate wording, but a reader of "IFC" should note no IfcSystem is written; run systems are lost on export (W1) |
| D6 | Task brief: "working file is the app's own operation-log + snapshot format" | The app saves a text snapshot only (file.rs); crates/oplog is not a dependency of apps/desktop or crates/document (grep 0). docs/agent-team/gap-review.md:BIM-004 ("PARTIAL / UNVERIFIED: oplog ... do not establish native save/open ... crash recovery") is still accurate for recovery; its baseline text is old (it says the desktop app is not on main; it is now) |
| D7 | docs/agent-team/gap-review.md: "main does not include the desktop app", "GUI-001 PROTOTYPE ON BRANCH" | Stale: apps/desktop is in the workspace on main (Cargo.toml:3). README.md:5-15 does not list apps/desktop, crates/document, crates/ifc reader, route, view, sheets, calc, automation either (README crate table lists 7 crates of 19) |
| D8 | crates/ifc/src/lib.rs header (:1-20) "Equipment is written as a bare IfcFlowTerminal ... until families are exported as types" | Families ARE exported as typed occurrences (families.rs:211-376); the bare IfcFlowTerminal is only the fallback for run-connected non-family equipment (lib.rs:1798) |
| D9 | apps/desktop/src/export.rs:1 "levels, walls and beams. The document's other elements are named in the warnings" | Out of date: export handles ~20 element kinds, and some (Room, Grid, Truss, BeamSystem, Tag...) are dropped without any warning (export.rs:198) |
| D10 | crates/document/src/lib.rs:1-4 "intentionally does NOT claim persistence, IFC import, ... or central-model concurrency" | Persistence and IFC import now exist in the app (file.rs, import.rs); the module doc is stale but its "no central model" statement is still true |

---------------------------------------------------------------------------------------------------
## 8. Mouse-only verdict summary (MEP-relevant)

| Step | Mouse-only possible? |
|---|---|
| Levels, grids | Yes (Level tool is a click + typed elevation in options bar; Grid two clicks) |
| Truss, beam system, brace as MEP support frames | Yes (two clicks / outline), geometry snaps 50 mm only |
| Equipment pad | Yes with typed thickness/offset; not tied to equipment |
| Hangers, supports, sleeves | No - not implemented |
| Wall openings, floor/roof shafts | Yes |
| Structural wall / framing / connections / analytical | No |
| IFC export | Yes (Export IFC dialog with schema + "own data" checkbox, model_edit.rs:233-265), IFC4X3 default |
| IFC import of a Revit MEP IFC | Technically yes for straight pipe/duct/beam/column/slab/wall; most MEP content skipped |
| Save/recover | Save/Open yes; no recovery |

## 9. Items worth saving to project memory (for the main session)
* Working file = text `.obim` snapshot (native.rs), NOT the oplog; crates/oplog unused by the app. No autosave/recovery.
* IFC round trip depends on embedded `OpenBIM_Document` pset; foreign IFC import is geometry-only for wall/beam/column/slab/footing/pipe/duct, units m/mm only, doors/windows/equipment/fittings/ports/openings skipped; foreign GlobalIds discarded.
* Hangers/supports, structural wall, links, worksets, phases, design options, family editor, project units: all absent or ribbon placeholders. Status bar "Workset: Mechanical (sample)" is a constant.
* GlobalIds of products stable; storeys/relationships/psets use a running counter (unstable when levels/own-data checkbox change).
