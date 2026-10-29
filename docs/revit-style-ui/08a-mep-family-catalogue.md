# MEP family catalogue (proposed 2026-10-24)

Status: **proposed, untracked** — research deliverable for the C3 / MEP-003 family library. Not committed,
not pushed. Companion to `08-gap-closure-plan-2026-10-21.md`; the class 3.1 rows it serves are C3, C4, C15,
C16, C17, C18 of `docs/gap-remediation-plan-2026-10-21.md`.

Measured against `origin/main` = `b1528496dc137268abcfe449e129ac378e921909`
(`git show origin/main:crates/family/src/library.rs` — `families()` at line 311, 7 families).

## 0. Measuring convention

- **"Family" = one entry in `library::families()`**, i.e. one named `Family` value with its own connector set and
  category. A *type* (size) inside a family is not counted. Revit's own `.rfa` is the same unit.
- **"Built"** = the family function exists on `origin/main` **and** is reachable through the app (a ribbon `Cmd`
  with a `run_command` arm, per `ribbon::tests::every_ribbon_command_is_either_handled_or_disabled`). "Missing"
  means the family does not exist in the library.
- **Counts are this document's own catalogue rows**, not a claim about Revit's shipped content, which is
  Autodesk's and varies by library version.

Current state on `main`: 7 families — `handle`, `door`, `window`, `arched_door` (architecture) and
`fan_coil`, `ahu`, `ceiling_diffuser` (MEP). So **3 MEP families**, 4 architecture.

## 1. The P0 set — what closes MEP-003 / C3

The register's order is authoritative (`gap-remediation-plan-2026-10-21.md`, C3 row; backlog row **MEP-003** in
`docs/revit-style-ui/05-backlog.md`). Acceptance: *each family places, hosts correctly, carries editable type
parameters, exports its IFC class and appears in a schedule.*

| # | Family (register order) | Revit library folder | Revit category | App `Category` | Domain | `System` | `Host` | IFC class | Status |
|---|---|---|---|---|---|---|---|---|---|
| 1 | Air handling unit | Mechanical > MEP > Air Side Components > Air Handling Units and Terminal Units | Mechanical Equipment | `UnitaryEquipment` | Duct + Pipe | `SupplyAir` + `ChilledWaterSupply`/`Return` | Free | `IfcUnitaryEquipment` | **Built** (`ahu()`) |
| 2 | Ceiling diffuser | Mechanical > MEP > Air Side Components > Air Terminals | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | Ceiling | `IfcAirTerminal` | **Built** (`ceiling_diffuser()`) |
| 3 | VAV box, single duct | Mechanical > MEP > Air Side Components > Air Handling Units and Terminal Units | Mechanical Equipment | `UnitaryEquipment` | Duct | `SupplyAir` + `ReturnAir` | Ceiling | `IfcUnitaryEquipment` | Missing |
| 4 | Pump | Plumbing > MEP > Equipment / Pipe | Mechanical Equipment | `Pump` | Pipe | `ChilledWater*`, `HeatingWater*`, `CondenserWater*` | Free | `IfcPump` | Missing |
| 5 | Valve | Pipe > Valves | Pipe Accessories | `Valve` | Pipe | any water | Free | `IfcValve` | Missing |
| 6 | WC / basin | Plumbing > MEP > Fixtures > Water Closets, Sinks | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold` + `DomesticHot` + `Sanitary` | Floor (WC) / Face (basin) | `IfcSanitaryTerminal` | Missing |
| 7 | Sprinkler head | Fire Protection | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | Ceiling (pendent) / Free (upright) | `IfcFireSuppressionTerminal` | Missing |
| 8 | Luminaire | Lighting | Lighting Fixtures | `LightFixture` | Electrical | `Power` | Ceiling / Wall | `IfcLightFixture` | Missing |
| 9 | Panel board | Electrical > MEP > Electric Power > Distribution | Electrical Equipment | `DistributionBoard` | Electrical | `Power` | Wall | `IfcElectricDistributionBoard` | Missing |
| 10 | Hanger / support | (no MEP folder; Revit puts hangers under Structural Connections and MEP Fabrication Parts) | Specialty Equipment / Generic Models | `Generic` or a new `Hanger` | — | `Other` | Face (beam / slab / truss) | `IfcDiscreteAccessory` / `IfcBuildingElementProxy` | Missing |

**8 of the 10 are missing.** Rows 3-10 are the work.

Two prerequisites gate the whole set, and neither is a family:

- **C5 (host resolution) is a prerequisite, not a detail.** `Category::default_host` picks a host, but
  `model_edit.rs` `family_click` only branches on "starts with Door/Window" and everything else goes to
  `Document::place_family(level, at, spec)`. Rows 2, 6, 7, 8 and 10 therefore *cannot* satisfy "hosts correctly"
  until C5 lands — see `references/mep-family-authoring.md` ("Hosting: the enum is not the resolution").
- **C1 (Load Family) decides whether these are Rust code or user data.** C1 is closed for *types* (a catalogue
  file becomes types in the project). A whole-family file format does not exist; every new family here is still a
  Rust function in `library.rs`, which contradicts the register's own wording ("Families must be user data, not
  Rust code, so that the owner can add content without a rebuild"). The class-3.1 pattern is to author in Rust
  *and* keep the loader able to carry the same data. That decision needs recording.

## 2. The full catalogue, by discipline

Scope follows the standing rule: **full discipline scope** — electrical, ELV, access control, BMS, plumbing,
HVAC water and air, sprinkler/fire fighting. Column "App" names the `Category` the family would take today;
`(new)` means the `Category` does not exist in `crates/family/src/mep.rs` yet.

### 2.1 HVAC air side — Mechanical > MEP > Air Side Components

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Air handling unit | Mechanical Equipment | `UnitaryEquipment` | Duct + Pipe | `SupplyAir`, `ChilledWater*` | **yes** (`ahu`) |
| Fan coil unit, ducted | Mechanical Equipment | `UnitaryEquipment` | Duct + Pipe | `SupplyAir`, `ChilledWater*` | **yes** (`fan_coil`) |
| VAV box, single duct | Mechanical Equipment | `UnitaryEquipment` | Duct | `SupplyAir` (+`ReturnAir`) | no |
| VAV box, dual duct | Mechanical Equipment | `UnitaryEquipment` | Duct | `SupplyAir` | no |
| VAV box, fan-powered | Mechanical Equipment | `UnitaryEquipment` | Duct | `SupplyAir` | no |
| Heat recovery ventilator (HRV/ERV) | Mechanical Equipment | `UnitaryEquipment` | Duct ×4 | `SupplyAir`, `ExhaustAir` | no |
| Fan filter unit (FFU) | Mechanical Equipment | `UnitaryEquipment` | Duct | `SupplyAir` | no |
| Unitary / split condenser (VRF) | Mechanical Equipment | `UnitaryEquipment` | Pipe ×2 | `CondenserWater*` | no |
| Packaged rooftop unit | Mechanical Equipment | `UnitaryEquipment` | Duct + Pipe | `SupplyAir`, `HeatingWater*` | no |
| Air curtain | Mechanical Equipment | `UnitaryEquipment` | Duct | `Other` | no |
| Ceiling diffuser, square | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | **yes** (`ceiling_diffuser`) |
| Ceiling diffuser, round | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | no |
| Linear slot diffuser | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | no |
| Swirl diffuser | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | no |
| Jet nozzle | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | no |
| Grille, supply (static) | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | no |
| Grille, return | Air Terminals | `AirTerminal` | Duct | `ReturnAir` | no |
| Grille, exhaust | Air Terminals | `AirTerminal` | Duct | `ExhaustAir` | no |
| Register, supply (grille + damper) | Air Terminals | `AirTerminal` | Duct | `SupplyAir` | no |
| Register, return | Air Terminals | `AirTerminal` | Duct | `ReturnAir` | no |
| Louvre / intake grille | Air Terminals | `AirTerminal` | Duct | `OutsideAir` | no |
| Transfer / door grille | Air Terminals | `AirTerminal` | Duct | `Other` | no |
| Silencer / attenuator | Duct Accessories | `(new) DuctAccessory` | Duct | follows the run | no |
| Filter box / filter section | Mechanical Equipment | `UnitaryEquipment` | Duct | follows the run | no |
| Humidifier | Mechanical Equipment | `UnitaryEquipment` | Duct + Pipe | `SupplyAir`, `HeatingWater*` | no |

C4 asks for the terminal families to carry **a `Flow` instance parameter and the four airflow-direction arrow
flags** (`gap-remediation-plan-2026-10-21.md` C4). Only the ceiling diffuser exists and it has `air_flow` as a
**Type** parameter, not an instance `Flow` — so C4 is partial, not closed.

### 2.2 Mechanical equipment — Mechanical > MEP > Equipment, Pipe

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Chiller, air-cooled / water-cooled | Mechanical Equipment | `Chiller` | Pipe ×2 | `CondenserWater*`, `ChilledWater*` | no |
| Boiler, hot water / steam | Mechanical Equipment | `Boiler` | Pipe | `HeatingWater*` | no |
| Cooling tower | Mechanical Equipment | `UnitaryEquipment` | Pipe | `CondenserWater*` | no |
| Heat exchanger, plate / shell-and-tube | Mechanical Equipment | `Coil` | Pipe ×4 | `ChilledWater*`, `HeatingWater*` | no |
| Coil: preheat / cooling / heating / runaround | Mechanical Equipment | `Coil` | Pipe | `ChilledWater*`, `HeatingWater*` | no |
| Pump, end-suction | Mechanical Equipment | `Pump` | Pipe | `ChilledWater*`, `HeatingWater*` | no |
| Pump, vertical in-line | Mechanical Equipment | `Pump` | Pipe | `ChilledWater*`, `HeatingWater*` | no |
| Pump, variable-speed (VSD) | Mechanical Equipment | `Pump` | Pipe | `ChilledWater*` | no |
| Pump, condenser water | Mechanical Equipment | `Pump` | Pipe | `CondenserWater*` | no |
| Pump, drainage / sewage ejector | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe | `Sanitary` | no |
| Tank, expansion | Mechanical Equipment | `Tank` | Pipe | `HeatingWater*` | no |
| Tank, buffer / storage | Mechanical Equipment | `Tank` | Pipe | `ChilledWater*`, `HeatingWater*` | no |
| Tank, cistern / water storage | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe | `DomesticCold` | no |
| Fan, extract / axial / centrifugal | Mechanical Equipment | `Fan` | Duct | `ExhaustAir` | no |
| Fan, roof-mounted | Mechanical Equipment | `Fan` | Duct | `ExhaustAir` | no |
| Heat pump | Mechanical Equipment | `UnitaryEquipment` | Pipe ×2 | `HeatingWater*`, `ChilledWater*` | no |
| Gas heater / furnace | Mechanical Equipment | `UnitaryEquipment` | Duct + Gas | `SupplyAir` | no |
| Electric heater | Mechanical Equipment | `UnitaryEquipment` | Duct + Electrical | `SupplyAir` | no |
| Radiator / convector | Mechanical Equipment | `UnitaryEquipment` | Pipe | `HeatingWater*` | no |
| Underfloor heating manifold | Mechanical Equipment | `UnitaryEquipment` | Pipe ×2 | `HeatingWater*` | no |
| Air & dirt separator / dosing pot | Pipe Accessories | `(new) PipeAccessory` | Pipe | follows the run | no |
| Expansion vessel | Mechanical Equipment | `Tank` | Pipe | `HeatingWater*` | no |
| Pressure-reducing valve station | Pipe Accessories | `(new) PipeAccessory` | Pipe | `DomesticCold` | no |

### 2.3 Duct fittings and accessories — Duct > Fittings, Duct > Accessories

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Duct fitting: elbow / tee / cross / transition / reducer / offset / lateral / wye | Duct Fittings | `DuctFitting` | Duct | follows the run | no |
| Duct accessory: volume control damper (VCD) | Duct Accessories | `Damper` | Duct | follows the run | no |
| Duct accessory: fire damper | Duct Accessories | `Damper` | Duct | follows the run | no |
| Duct accessory: smoke damper | Duct Accessories | `Damper` | Duct | follows the run | no |
| Duct accessory: access door / inspection hatch | Duct Accessories | `(new) DuctAccessory` | Duct | follows the run | no |
| Duct accessory: flexible connector | Duct Accessories | `(new) DuctAccessory` | Duct | follows the run | no |

### 2.4 Pipe fittings, valves and accessories — Pipe > Fittings, Pipe > Valves, Pipe > Accessories

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Pipe fitting: elbow / tee / cross / transition / union / cap / reducer / vent tee / Y / lateral | Pipe Fittings | `PipeFitting` | Pipe | follows the run | no |
| Valve: gate | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: globe | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: check (swing / lift) | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: ball | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: butterfly | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: balancing / commissioning set | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: double-check / backflow preventer | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Valve: drain cock / air vent | Pipe Accessories | `Valve` | Pipe | follows the run | no |
| Accessory: Y-strainer / strainer | Pipe Accessories | `(new) PipeAccessory` | Pipe | follows the run | no |
| Accessory: pressure gauge / thermometer | Pipe Accessories | `(new) PipeAccessory` | Pipe | follows the run | no |
| Accessory: venturi flow meter / flow meter | Pipe Accessories | `(new) PipeAccessory` | Pipe | follows the run | no |
| Accessory: expansion joint | Pipe Accessories | `(new) PipeAccessory` | Pipe | follows the run | no |
| Accessory: flexible connector | Pipe Accessories | `(new) PipeAccessory` | Pipe | follows the run | no |

### 2.5 Plumbing fixtures — Plumbing > MEP > Fixtures

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Water closet (WC), close-coupled | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Water closet, wall-hung | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Urinal, wall-hung | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×2 | `DomesticCold`, `Sanitary` | no |
| Lavatory / basin, wall-hung | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Lavatory / basin, countertop or island | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Sink, kitchen / laboratory | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Bath tub | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Shower, valve and head | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×2 | `DomesticCold`, `DomesticHot` | no |
| Bidet | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×2 | `DomesticCold`, `DomesticHot` | no |
| Drinking fountain | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×2 | `DomesticCold`, `Sanitary` | no |
| Floor drain | Plumbing Fixtures | `SanitaryTerminal` | Pipe | `Sanitary` | no |
| Floor sink / gully | Plumbing Fixtures | `SanitaryTerminal` | Pipe | `Sanitary`, `StormWater` | no |
| Cleanout | Plumbing Fixtures | `SanitaryTerminal` | Pipe | `Sanitary` | no |
| Trap, P-trap / bottle trap | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×2 | `Sanitary` | no |
| Grease trap / interceptors | Specialty Equipment | `(new) SpecialtyEquipment` | Pipe ×2 | `Sanitary` | no |
| Wash fountain / scrub sink | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×3 | `DomesticCold`, `DomesticHot`, `Sanitary` | no |
| Emergency shower / eyewash | Plumbing Fixtures | `SanitaryTerminal` | Pipe ×2 | `DomesticCold`, `DomesticHot` | no |
| Cistern / flush tank | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe ×2 | `DomesticCold`, `Sanitary` | no |

C15 asks for **WFU / HWFU / CWFU type parameters** and connectors on every fixture, and for the fixture-unit
conversion to be driven from them (`crates/calc/src/plumbing.rs`). Only a fixture family with those three
parameters can close it.

### 2.6 Plumbing equipment — Plumbing > MEP > Equipment

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Water heater, storage | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe ×2 | `DomesticCold`, `DomesticHot` | no |
| Water heater, instantaneous / heat pump | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe ×2 | `DomesticCold`, `DomesticHot` | no |
| Sewage / waste pump | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe ×2 | `Sanitary` | no |
| Sump pump | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe | `Sanitary` | no |
| Booster pump set | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe ×2 | `DomesticCold` | no |
| Softener / water treatment | Plumbing Equipment | `(new) PlumbingEquipment` | Pipe ×2 | `DomesticCold` | no |

### 2.7 Fire protection — Fire Protection

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Sprinkler head, pendent | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler head, upright | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler head, sidewall / horizontal | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler head, concealed | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler head, dry pendent | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler head, ESFR | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler head, water mist | Sprinklers | `Sprinkler` | Pipe | `FireProtection` | no |
| Sprinkler valve: wet alarm / alarm test | Sprinklers | `Valve` | Pipe | `FireProtection` | no |
| Sprinkler valve: deluge | Sprinklers | `Valve` | Pipe | `FireProtection` | no |
| Sprinkler valve: pre-action | Sprinklers | `Valve` | Pipe | `FireProtection` | no |
| Sprinkler valve: check / stop | Sprinklers | `Valve` | Pipe | `FireProtection` | no |
| Flow switch / pressure switch / tamper switch | Specialty Equipment | `(new) SpecialtyEquipment` | Pipe | `FireProtection` | no |
| Fire hose reel | Specialty Equipment | `(new) SpecialtyEquipment` | Pipe | `FireProtection` | no |
| Fire extinguisher | Specialty Equipment | `(new) SpecialtyEquipment` | — | `Other` | no |
| Fire pump / jockey pump | Mechanical Equipment | `Pump` | Pipe | `FireProtection` | no |
| Fire water tank | Specialty Equipment | `(new) SpecialtyEquipment` | Pipe | `FireProtection` | no |
| Hydrant / standpipe | Site | `(new) SiteElement` | Pipe | `FireProtection` | no |

C16 asks for **pendent and upright head families, a design-area concept, and the wet/dry system attribute**,
keeping the book's rule that the pipe runs above pendent heads and below upright heads. Nothing of that exists.

### 2.8 Electrical equipment — Electrical > MEP > Electric Power

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Transformer, dry type | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Transformer, wet / cast resin | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Switchboard | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Panelboard / distribution board | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Motor control centre (MCC) | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Generator / genset | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| UPS | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Automatic transfer switch (ATS) | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Busbar / busduct | Electrical Equipment | `(new) Busbar` | Electrical | `Power` | no |
| Meter / meter panel | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| Power factor correction / capacitor bank | Electrical Equipment | `DistributionBoard` | Electrical | `Power` | no |
| Solar inverter | Electrical Equipment | `DistributionBoard` | Electrical ×2 | `Power` | no |
| EV charger | Electrical Equipment | `DistributionBoard` | Electrical | `Power` | no |

The book's own build order is `transformer -> panels with distribution system -> circuits (Power) ->
Select Panel -> wires` (`mep-part2` §12). C17 asks for panels, devices and luminaires to become
**elements with families**, plus circuits and panel schedules; today they are records in `MepData`, not elements.

### 2.9 Electrical fixtures and devices — Electrical > MEP > Devices

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Outlet / receptacle, 1-phase | Electrical Fixtures | `Outlet` | Electrical | `Power` | no |
| Outlet / receptacle, 3-phase | Electrical Fixtures | `Outlet` | Electrical | `Power` | no |
| Isolator | Electrical Fixtures | `Outlet` | Electrical | `Power` | no |
| Cooker / appliance outlet | Electrical Fixtures | `Outlet` | Electrical | `Power` | no |
| Fused connection unit (FCU) | Electrical Fixtures | `Outlet` | Electrical | `Power` | no |
| Switch, 1/2/3-gang | Electrical Fixtures | `SwitchingDevice` | Electrical | `Power` | no |
| Dimmer | Electrical Fixtures | `SwitchingDevice` | Electrical | `Power` | no |
| Time switch | Electrical Fixtures | `SwitchingDevice` | Electrical | `Power` | no |
| Pull-cord switch | Electrical Fixtures | `SwitchingDevice` | Electrical | `Power` | no |
| Socket outlet with switch | Electrical Fixtures | `Outlet` | Electrical | `Power` | no |
| Data outlet / RJ45 | Data Devices | `(new) DataDevice` | Electrical | `Data` | no |
| Telephone outlet | Telephone Devices | `(new) TelephoneDevice` | Electrical | `Data` | no |
| TV / antenna outlet | Communication Devices | `(new) CommunicationDevice` | Electrical | `Data` | no |
| Speaker outlet | Communication Devices | `(new) CommunicationDevice` | Electrical | `Data` | no |

### 2.10 Lighting — Lighting

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Luminaire, recessed downlight | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Luminaire, surface / batten | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Luminaire, suspended | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Luminaire, wall sconce | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Luminaire, high bay / low bay | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Luminaire, linear / strip | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Luminaire, emergency | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Exit sign | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Floodlight | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Street / area light | Lighting Fixtures | `LightFixture` | Electrical | `Power` | no |
| Occupancy sensor | Lighting Devices | `(new) LightingDevice` | Electrical | `Controls` | no |
| Daylight sensor | Lighting Devices | `(new) LightingDevice` | Electrical | `Controls` | no |
| Lighting control module / relay | Lighting Devices | `(new) LightingDevice` | Electrical | `Controls` | no |

### 2.11 ELV, access control and BMS — the Revit device categories

Revit carries seven device categories for this scope; the app carries **none**. They are listed separately in
Autodesk's own MEP category list and each has its own copy/monitor behaviour.

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Network switch / patch panel | Data Devices | `(new) DataDevice` | Electrical | `Data` | no |
| Wireless access point | Data Devices | `(new) DataDevice` | Electrical | `Data` | no |
| Media converter / gateway | Data Devices | `(new) DataDevice` | Electrical | `Data` | no |
| Intercom door station / master station | Communication Devices | `(new) CommunicationDevice` | Electrical | `Data` | no |
| Public address speaker | Communication Devices | `(new) CommunicationDevice` | Electrical | `Data` | no |
| Antenna / repeater | Communication Devices | `(new) CommunicationDevice` | Electrical | `Data` | no |
| Telephone point / PBX | Telephone Devices | `(new) TelephoneDevice` | Electrical | `Data` | no |
| Smoke detector | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Heat detector | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Multi-sensor detector | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Manual call point | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Sounder / beacon / strobe | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Beam detector | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Aspirating detector (VESDA) | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Fire alarm control panel / repeater panel | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Fire alarm module / I/O module | Fire Alarm Devices | `(new) FireAlarmDevice` | Electrical | `Controls` | no |
| Card reader | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Keypad | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Biometric reader | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Door contact / reed switch | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Magnetic lock | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Electric strike | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Electrified door closer | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Request-to-exit sensor | Security Devices | `(new) SecurityDevice` | Electrical | `Controls` | no |
| Dome camera | Security Devices | `(new) SecurityDevice` | Electrical | `Data` | no |
| Bullet camera | Security Devices | `(new) SecurityDevice` | Electrical | `Data` | no |
| PTZ camera | Security Devices | `(new) SecurityDevice` | Electrical | `Data` | no |
| NVR / recorder | Security Devices | `(new) SecurityDevice` | Electrical | `Data` | no |
| Camera housing / bracket | Security Devices | `(new) SecurityDevice` | — | `Other` | no |
| DDC controller | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| BMS I/O module | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| Sensor: temperature / humidity / CO2 / pressure / flow / level | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| Actuator: damper / motorised valve | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| Thermostat / humidistat | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| VAV controller | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| Gateway / router / protocol converter | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| BMS power supply / PSU | Mechanical Control Devices | `(new) MechanicalControlDevice` | Electrical | `Controls` | no |
| Nurse call bedhead unit / call point / corridor light | Nurse Call Devices | `(new) NurseCallDevice` | Electrical | `Controls` | no |

### 2.12 Cable tray, conduit and wiring — Cable Tray, Conduit

| Family | Revit category | App | Domain | `System` | Built |
|---|---|---|---|---|---|
| Cable tray, ladder | Cable Trays | `(new) CableTray` | CableTray | `Data`, `Power` | no |
| Cable tray, perforated | Cable Trays | `(new) CableTray` | CableTray | `Data`, `Power` | no |
| Cable tray, trunking / wireway | Cable Trays | `(new) CableTray` | CableTray | `Data`, `Power` | no |
| Cable tray, solid bottom | Cable Trays | `(new) CableTray` | CableTray | `Data`, `Power` | no |
| Cable tray fitting: elbow / tee / cross / reducer / riser / coupling | Cable Tray Fittings | `CableFitting` | CableTray | follows the run | no |
| Conduit, rigid metal | Conduits | `(new) Conduit` | Conduit | `Data`, `Power` | no |
| Conduit, flexible | Conduits | `(new) Conduit` | Conduit | `Data`, `Power` | no |
| Conduit, PVC | Conduits | `(new) Conduit` | Conduit | `Data`, `Power` | no |
| Conduit fitting: coupling / connector / bushing / nipple / elbow / offset | Conduit Fittings | `(new) ConduitFitting` | Conduit | follows the run | no |
| Wire / cable | Wires | `(new) Wire` | Electrical | `Power`, `Data` | no |

C17 names **cable tray and conduit tools** as part of making electrical an element model; the tool side is C17,
the families are this table.

### 2.13 Hangers, supports and fixings — C18

Revit has no MEP "hanger" category. Hangers are MEP Fabrication Parts (group "Rect, Hangers"), Revit
Structural Connections, or Specialty Equipment — which is why the register says *treat them as MEP families (C3),
hosted on beams, slabs or trusses, with auto-placement along a run*.

| Family | Revit category | App | Host | Built |
|---|---|---|---|---|
| Rod hanger / clevis hanger | Specialty Equipment | `(new) Hanger` or `Generic` | Face (beam / slab) | no |
| Trapeze hanger | Specialty Equipment | `(new) Hanger` | Face (beam / slab) | no |
| Riser clamp | Specialty Equipment | `(new) Hanger` | Face (wall / column) | no |
| Beam clamp | Specialty Equipment | `(new) Hanger` | Face (beam) | no |
| Spring hanger / variable spring | Specialty Equipment | `(new) Hanger` | Face (beam / slab) | no |
| Constant-support hanger | Specialty Equipment | `(new) Hanger` | Face (beam / slab) | no |
| Duct hanger / duct support | Specialty Equipment | `(new) Hanger` | Face (beam / slab) | no |
| Pipe support / pipe shoe / saddle | Specialty Equipment | `(new) Hanger` | Face (beam / slab) | no |
| Equipment pad / equipment curb | Specialty Equipment | `(new) Hanger` | Floor | no |
| Sleeve / penetration | Specialty Equipment | `(new) Hanger` | Wall / Floor | no |
| Threaded rod | Specialty Equipment | `(new) Hanger` | — | no |
| Anchor / shot fix | Specialty Equipment | `(new) Hanger` | — | no |
| Unistrut / channel | Specialty Equipment | `(new) Hanger` | — | no |
| Anti-vibration mount / AV pad | Specialty Equipment | `(new) Hanger` | Floor / Face | no |
| Rooftop curb | Specialty Equipment | `(new) Hanger` | Roof | no |
| Equipment rail / frame | Specialty Equipment | `(new) Hanger` | Floor | no |

### 2.14 Architecture and other categories

| Family | Revit category | App | Built |
|---|---|---|---|
| Door, single flush | Doors | `Door` | **yes** (`door`) |
| Window, fixed with mullions | Windows | `Window` | **yes** (`window`) |
| Door, arched | Doors | `Door` | **yes** (`arched_door`) |
| Handle | (annotation) | `Generic` | **yes** (`handle`) |
| Opening, passage (cased) | Openings / Generic Models | `Generic` | no |
| Opening, window | Openings / Generic Models | `Generic` | no |
| Casework / cabinet | Casework | `Furniture` | no |
| Desk / table / chair | Furniture | `Furniture` | no |
| Laboratory / medical equipment | Specialty Equipment | `(new) SpecialtyEquipment` | no |
| Site: hydrant / manhole / bollard | Site | `(new) SiteElement` | no |
| Planting | Planting | `(new) Planting` | no |

## 3. What the catalogue implies for the `Category` enum

`crates/family/src/mep.rs` `Category::ALL` has 23 variants. The catalogue above needs, at minimum:

| New `Category` | Revit category it mirrors | Families that need it |
|---|---|---|
| `PipeAccessory` | Pipe Accessories | strainers, gauges, separators, flow meters, expansion joints |
| `DuctAccessory` | Duct Accessories | access doors, flexible connectors (dampers already use `Damper`) |
| `PlumbingEquipment` | Plumbing Equipment | water heaters, sewage pumps, booster sets |
| `MechanicalControlDevice` | Mechanical Control Devices | BMS controllers, sensors, actuators, thermostats, VAV controllers |
| `DataDevice` | Data Devices | network switch, patch panel, access point, media converter |
| `CommunicationDevice` | Communication Devices | intercom, PA speaker, antenna, TV outlet |
| `TelephoneDevice` | Telephone Devices | telephone point, PBX |
| `FireAlarmDevice` | Fire Alarm Devices | detectors, call points, sounders, panels, modules |
| `SecurityDevice` | Security Devices | readers, contacts, locks, strikes, cameras, NVR |
| `NurseCallDevice` | Nurse Call Devices | bedhead units, call points |
| `LightingDevice` | Lighting Devices | occupancy/daylight sensors, control modules |
| `CableTray` / `Conduit` / `Wire` | Cable Trays / Conduits / Wires | the tray and conduit runs and their wiring |
| `ConduitFitting` | Conduit Fittings | conduit couplings, connectors, bushings, elbows |
| `Hanger` | Specialty Equipment / Structural Connections | C18's whole group |
| `SpecialtyEquipment` | Specialty Equipment | grease traps, fire hose reels, extinguishers, lab equipment |
| `SiteElement` / `Planting` | Site / Planting | hydrants, manholes, trees |

`Category::ifc_class` must gain the matching entity for each (Revit's own default IFC mapping is the reference
for which entity each category carries — see §5). `Category::default_host` and `schedule::family_kind` must gain
an arm for each, and `ScheduleKind` needs the matching schedule kinds (`PipeAccessories` and
`DuctAccessories` already exist; `CableTrayFittings`, `ConduitFittings`, `CableTrays`, `Conduits`,
`LightingDevices`, `SpecialtyEquipment` do not).

Adding a `Category` is a one-to-one mapping to a Revit category, not a new `NodeKind` — the
`mep-family-authoring.md` rule about distinct engines does not apply here.

## 4. Findings that block the family work

1. **`System::Controls` is defined and used but absent from `System::ALL`.**
   `crates/family/src/mep.rs`: `Controls` is a variant (line 190), `domain_ok` accepts it (line 230),
   `ifc_system` names it (line 253) and a connector filter uses it (line 591) — but `ALL`
   (`mep.rs:195`) is `[System; 21]` and does not list it. `code()` therefore returns `unwrap_or(0)` = `SupplyAir` for a
   Controls system and `from_code` can never return `Controls`. Every ELV / access-control / BMS device in
   §2.11 uses `System::Controls`, so this must be fixed before that group can round-trip through save/reopen.
2. **`family_kind` cannot reach `ScheduleKind::DuctFittings` or `PipeFittings`.** `schedule.rs:340` maps
   `Fam::DuctFitting | Fam::PipeFitting | Fam::CableFitting => ScheduleKind::GenericModels`, while
   `ScheduleKind::DuctFittings` and `ScheduleKind::PipeFittings` exist and are unreachable. A duct-fitting
   family would not appear in a "Duct Fittings" schedule, so the MEP-003 acceptance clause *appears in a
   schedule* fails for §2.3 and §2.4 fittings.
3. **Hosting is the blocker for six of the ten P0 families.** `Category::default_host` returns `Ceiling` for
   `AirTerminal`/`LightFixture`/`Sprinkler` and `Face` is never resolved; `family_click` puts everything
   non-Door/Window on the level (C5). Rows 2, 6, 7, 8 and 10 of §1 cannot satisfy "hosts correctly" until C5
   lands, and §2.13 (hangers) needs a host that is a *structural element*, which the enum cannot express.
4. **C1's "families must be user data" is not satisfied by adding Rust functions.** The register's own wording
   is that families must not be Rust code so the owner can add content without a rebuild. Authoring each new
   family as a function in `library.rs` meets the class-3.1 pattern but not that sentence. Either the loader is
   extended to whole families, or the sentence is amended — a user decision, and it should be taken before
   §2.1-2.13 are built in bulk.
5. **No ELV / BMS category exists at all.** §2.11 is 50 families across seven Revit device categories that the
   app cannot represent without new `Category` variants. This is a scope decision the owner has not taken.

## 5. Sources

Reference product (Revit), primary sources, cited inline; anything not sourced is marked `[unverified]`.

- **Revit MEP Fundamentals (SDC Publications, the same imprint as the team's book)** — the library folder set
  (`Cable Tray, Conduit, Duct, Electrical, Fire Protection, Lighting, Mechanical, Pipe, Plumbing`), the
  component tool list (Air terminal, Electrical Equipment, Mechanical Equipment, Devices, Plumbing Fixture,
  Lighting Fixtures, Sprinkler) and the hosted-family kinds. <https://static.sdcpublications.com/pdfsample/978-1-63057-446-8-2-bxawsm8nct.pdf>
- **Autodesk help, "About Categories for MEP Families"** — the category × family-parameter table that decides
  which parameters each MEP category activates (Air Terminals, Electrical Equipment, Electrical Fixtures,
  Lighting Fixtures, Mechanical Equipment, Pipe Accessories, Pipe Fittings, Plumbing Fixtures, Specialty
  Equipment, Sprinklers, and the 2023 additions Mechanical Control Devices and Plumbing Equipment).
  <https://help.autodesk.com/cloudhelp/2023/ENU/Revit-Customize/files/GUID-48B14613-5BFC-4E4D-B1DF-80B85EC8DFF3.htm>
- **Autodesk help, "MEP Components"** (2027) — the same category list for the current release.
  <https://help.autodesk.com/cloudhelp/2027/ENU/Revit-Customize/files/GUID-F9FCBE97-55AE-4263-8AF7-6D6F8EA68B6A.htm>
- **Autodesk help, "Revit Categories for Inventor"** — the flat Revit category list including Mechanical
  Control Devices and Plumbing Equipment. <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/REVIT-CATEGORIES-IN-INVENTOR.htm>
- **AUGI, "Navigating the Digital Blueprint"** — the MEP component category set as used in practice (Air
  Terminals, Communication/Data/Electrical Equipment/Electrical Fixtures/Fire Alarm/Lighting Devices/Lighting
  Fixtures/Mechanical Equipment/Nurse Call/Plumbing Fixtures/Security/Sprinklers/Telephone).
  <https://www.augi.com/articles/detail/navigating-the-digital-blueprint-mepf-revit-tips-and-tricks-for-daily-use>
- **GitHub: jeremytammik/tbc, `a/zip/367_categories.txt`** — the real Revit built-in category list with ids
  (219 document categories, 747 built-ins). <https://github.com/jeremytammik/tbc/blob/master/a/zip/367_categories.txt>
- **GitHub: OpeningDesign FreeMVD mirror, `revit ifc mapping - default mapping.txt`** — Revit's own IFC export
  mapping, class + type → Revit category/subcategory. Used to choose the IFC entity for each new category
  (e.g. `IfcValve -> Pipe Accessories`, `IfcLamp -> Lighting Devices`, `IfcFireSuppressionTerminal/SPRINKLER
  -> Sprinklers`, `IfcTank -> Specialty Equipment`, `IfcController/IfcSensor -> Specialty Equipment`).
  <https://hub.openingdesign.com/OpeningDesign/FreeMVD_Mirror/src/branch/master/Starter_Files/Revit/revit%20ifc%20mapping%20-%20default%20mapping.txt>
- **GitHub: Autodesk official Revit family/template library installers** (link index, Autodesk domains) —
  confirms the library is shipped as a separate per-region package, i.e. the folder tree is Autodesk's content
  and not something this project can copy. <https://github.com/iamlovedit/Revit>
- **In-repo references**: `docs/reference/revit-books/mep-part1-ch1-3.md` (§2-3: categories, families, load
  family, browser organisation), `mep-part2-ch4-6.md` (§5 air terminals, §6 mechanical equipment, §12 electrical
  equipment), `mep-part3-ch7-10.md` (§A1 plumbing fixtures, §A9 pipe settings, §B1-B4 sprinklers), and
  `docs/gap-remediation-plan-2026-10-21.md` rows C1-C18.

Not verified this pass: the exact shipped `.rfa` filenames inside each Revit library folder (Autodesk's
content, version-specific, and not readable without a Revit install). The family *names* above are the ones
the team's book references and Revit's categories require, not a copy of Autodesk's shipped files.
