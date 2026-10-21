# MEP engine and content verification

> Scope: the ten claims M1–M10 about MEP content, systems, Generate Layout, compute and electrical in `revit-alike`.
> SHA: `6ac8addc9550fcbed3d2b5cce868a540fe0a0780` (worktree `C:/Users/wilfr/hermes-first-task/revit-book-refs`), code claims pinned to `dcf838297246d1adb0093143ad86a393af8e0303`.
> Method: code reading only. No `cargo`, no edits, no checkout. `main` (`5560203`) differs from `6ac8addc` by 62 doc files only, so every code line number below is identical at `dcf8382`. Verdicts: CONFIRMED / WRONG / APPROXIMATE.

## M1 — five families only, fan coil has no types and Type-scope MEP values

**Verdict: CONFIRMED**

**Real lines:** `crates/family/src/library.rs:309-311` (all), `:322` (fan coil name), `:327-334` (Type-scope MEP values), `:441-442` (no `types:`); `apps/desktop/src/export.rs:63` (Instance-scope filter).

**Quoted code:**
```rust
// library.rs:309-311
pub fn all() -> Result<Vec<Compiled>, FamilyError> {
    compile_library(&[handle(), door(), window(), arched_door(), fan_coil()])
}
// library.rs:322
        name: "Fan coil unit, ducted".into(),
// library.rs:327-334  (all Scope::Type)
            V::number("air_flow", 0.1, 0.01, 5.0, Type),
            V::number("water_flow", 0.0003, 0.00001, 0.05, Type),
            V::input("water_pipe", 0.022, 0.01, 0.1, Type),
            V::input("condensate_pipe", 0.02, 0.01, 0.05, Type),
            V::input("duct_w", 0.4, 0.1, 1.5, Type),
            V::input("duct_h", 0.2, 0.1, 1.0, Type),
            V::number("voltage", 230.0, 12.0, 1000.0, Type),
            V::number("power", 80.0, 1.0, 5000.0, Type),
// library.rs:441-442  (no `types:` field at all; ..Default::default() leaves it empty)
        category: Category::UnitaryEquipment,
        ..Default::default()
// export.rs:63 — only Instance-scope inputs reach Properties
        .filter(|(n, _)| *n != "wall_t" && f.input_scope(n) == Some(openbim_family::Scope::Instance))
```
`library.rs` defines exactly five `Family` values: `handle` (:26), `door` (:95), `arched_door` (:163), `window` (:223), `fan_coil` (:316) — no other `Family { … }` literal exists in `crates/family/src` outside `library.rs`. `library::all()` is the only family source the app uses (`apps/desktop/src/export.rs:47`, `crates/document/src/schedule.rs:336`). `Category::ALL` (23) and `System::ALL` (21) are enums in `crates/family/src/mep.rs:89,195`; no family instantiates AirTerminal/Fan/Pump/Boiler/Chiller/Valve/SanitaryTerminal/Sprinkler/LightFixture/Outlet/SwitchingDevice.

**What it means:** no AHU, air terminal, VAV, pump, chiller, boiler, valve, plumbing fixture, sprinkler head, luminaire, socket, switch or panel-board family exists anywhere in the shipped content. The fan coil's `air_flow`/`water_flow`/`duct_w`/`duct_h`/pipe sizes/`voltage`/`power` are Type-scope, so `instance_values_with` filters them out and the Properties panel ("Instance Parameters") never offers them; the model-edit test asserts exactly that (`apps/desktop/src/model_edit.rs:1913`: `"type values and the wall's are not instance values"`).

**Confidence:** High.

## M2 — the AHU button and the Component button place the fan coil

**Verdict: CONFIRMED**

**Real lines:** `apps/desktop/src/main.rs:464` (arm); `apps/desktop/src/ribbon.rs:68` (`ahu`/"AHU") and `:109` (`component`/"Component").

**Quoted code:**
```rust
// main.rs:464
            "ahu" | "component" => { self.family_pick = "Fan coil unit, ducted".into(); self.family_type.clear(); self.family_kind = Some(openbim_family::Category::UnitaryEquipment); self.choose_tool(Tool::Family) }
// ribbon.rs:68  (Systems > HVAC > Mechanical Equipment)
        c("ahu", "AHU"), c("fan", "Fan"), c("vav_box", "VAV Box"),
// ribbon.rs:109  (Architecture > Build)
        c("wall", "Wall"), c("door", "Door"), c("window", "Window"), c("component", "Component"),
```
`component` is also used by the Systems "All" subtab icon (ribbon.rs:43) and Massing & Site "Site Component" (ribbon.rs:153), so all of them land on the same arm.

**What it means:** both buttons dispatch one command that selects the fan-coil family with `Tool::Family`. The label "AHU" is cosmetic.

**Confidence:** High.

## M3 — ribbon wiring counts (HVAC / Plumbing / Electrical vs `run_command`)

**Verdict: CONFIRMED**

**Real lines:** placeholder arm `apps/desktop/src/main.rs:495`; `run_command` body `main.rs:405-497`; group definitions `ribbon.rs:56-105`.

**Quoted code:**
```rust
// main.rs:495
            _ => self.message = format!("{label} is not implemented in this UI prototype"),
```

Exact counts (id counted per group occurrence; "wired" = appears as an arm in `run_command`):

| Group | Buttons | Wired | Placeholder |
|---|---|---|---|
| HVAC (ribbon.rs:56-71) | 21 | 8 | 13 |
| Plumbing & Piping (ribbon.rs:73-88) | 22 | 7 | 15 |
| Electrical (ribbon.rs:90-105) | 20 | 3 | 17 |
| **Sum (with duplicates)** | **63** | **18** | **45** |
| **Unique ids** | **59** | **14** | **45** |

Wired ids — HVAC: `duct`, `duct_insulation`, `end_point`, `compute`, `create_mech_system`, `generate_layout`, `system_browser`, `ahu`. Plumbing: `pipe`, `pipe_insulation`, `end_point`, `compute`, `create_pipe_system`, `generate_layout`, `system_browser`. Electrical: `elec_node`, `elec_cable`, `elec_compute`. `end_point`/`compute`/`generate_layout`/`system_browser` are duplicated across groups, which is why the unique total (59) is below the per-group sum (63).

Placeholder ids fall to `main.rs:495`: HVAC — `duct_fitting`, `duct_elbow`, `duct_transition`, `flex_duct`, `damper`, `air_terminal`, `diffuser`, `grille`, `fan`, `vav_box`, `mechanical_equipment`, `chiller`, `cooling_tower`. Plumbing — `pipe_fitting`, `pipe_elbow`, `pipe_tee`, `pipe_transition`, `valve_gate`, `valve_ball`, `valve_butterfly`, `backflow_preventer`, `hanger`, `pipe_support`, `plumbing_fixture`, `sprinkler`, `drain`, `cleanout`, `pump`. Electrical — all 17 Containment / Lighting & Devices / Electrical Equipment buttons.

**What it means:** 45 of 59 unique Systems-tab buttons are stubs; only the drawing, end-point, compute, system-browser and electrical-node/cable/compute commands do anything.

**Confidence:** High.

## M4 — Generate Layout: Network X/Y + Routed for two, axis/height constraint, flow ignored

**Verdict: APPROXIMATE** — substance fully confirmed, but the file path in the claim is wrong.

**Real lines:** the file is **`crates/document/src/layout.rs`** (398 lines); `apps/desktop/src/layout.rs` **does not exist** (`git cat-file -e dcf8382:apps/desktop/src/layout.rs` → MISSING; no `layout.rs` anywhere under `apps/desktop/src`). Relevant lines: `layout.rs:26-33` (kinds), `:121-124` (axis + height), `:138-140` (Routed for exactly two), `:157-176` (`routed_layout`), `:229-308` (`network_layout`).

**Quoted code:**
```rust
// layout.rs:26-33  — only three kinds; no Perimeter, Intersections or Custom
pub enum SolutionKind {
    NetworkAlongX,
    NetworkAlongY,
    Routed,
}
// layout.rs:121-124 — must face along an axis (no up/down) and sit at the base connector's height
            let d = unit(c.conn.dir);
            let sideways = d[2].abs() < 1e-6 && (d[0].abs() > 1.0 - 1e-6 || d[1].abs() > 1.0 - 1e-6);
            if !sideways { unconnected.push((c.element, format!("the connector '{}' does not face along an axis, or points up or down (vertical runs are not supported yet)", c.conn.name))); }
            else if (c.conn.pos[2] - z).abs() > 0.002 { unconnected.push((c.element, format!("the connector '{}' is at a different height from the base's ({:.0} mm against {:.0} mm)", ...))); }
// layout.rs:138-140 — the routed solution is offered only when exactly two connectors are usable
        if usable.len() == 2 {
            match self.routed_layout(level, conns, &usable, trunk_size, prefs) { Ok(l) => offer(l, &mut layouts), Err(e) => why.push(format!("Routed: {e}")) }
        }
```
Flow direction is never read: `grep -n "flow\|Flow" crates/document/src/layout.rs` returns only the `#[cfg(test)]` imports/fields at :314 and :325 — no `flow_dir` consult in the engine. The module header states it outright (`layout.rs:6-8`): *"only the Network type (and the routed one for two connectors) exists, not Perimeter or Intersections; all connectors must face along an axis, point sideways (not up or down) and be at the height of the base's connector"*.

**What it means:** the layout engine is real but minimal; the claim's behaviour is exactly as described, only the path is wrong.

**Confidence:** High on substance; the path correction is exact.

## M5 — MepSystemDef shape; Compute filters by System, so two CHW loops collide

**Verdict: CONFIRMED** (line range in the claim is off; real lines below)

**Real lines:** `crates/document/src/mep.rs:105-111` (MepSystemDef), `:328-337` (members must be Family elements); `crates/document/src/compute.rs:156-157` (System filter), `:201` (MoreThanOneSource).

**Quoted code:**
```rust
// mep.rs:105-111
pub struct MepSystemDef {
    pub name: String,
    pub system: System,
    pub members: Vec<ElementId>,
    pub base: Option<ElementId>,
}
// mep.rs:332 — members are family instances only, so ducts/pipes cannot be members
                && d.members.iter().all(|m| elements.get(m).is_some_and(|x| x.category == Category::Family))
// compute.rs:156-157 — the run set is chosen by System, not by MepSystemDef
    let runs: Vec<ElementId> = doc.elements().filter(|e| matches!(e.category, Category::Duct | Category::Pipe) && run_z(doc, e.id).is_some())
        .map(|e| e.id).filter(|id| system_of_run(doc, *id) == s.system).collect();
// compute.rs:201
    let root = match sources.len() { 0 => return Err(ComputeError::NoSource), 1 => sources[0], _ => return Err(ComputeError::MoreThanOneSource) };
```
`system_of_run` (compute.rs:122-132) resolves a run's system from `run_systems`, else from the joined fitting, else `System::Other`; `MepData.systems` is never consulted by `compute_system`.

**What it means:** `MepSystemDef` is a named group of family instances (equipment) plus an optional base. Because Compute works per `System` enum across the whole drawing, two independent CHW loops of the same enum in one model are one "system" and trip `MoreThanOneSource` (test: compute.rs:550). To split them you must use different enums or keep one source per enum.

**Confidence:** High.

## M6 — `end_point_click` drops an equipment join on the clicked port

**Verdict: CONFIRMED**

**Real line:** `apps/desktop/src/compute_ui.rs:132` (inside `end_point_click`, which starts at `:115`).

**Quoted code:**
```rust
// compute_ui.rs:129-133
        let mut data = self.document.mep().clone();
        let had = data.endpoints.len();
        data.endpoints.retain(|e| e.port != port);
        data.equip_links.retain(|q| q.port != port);   // <-- line 132
        let removed = data.endpoints.len() != had;
```
The `retain` runs before any mark is added, so it fires for `Mark::Source`, `Mark::Terminal` and `Mark::Remove` alike. For `Mark::Terminal` with `kind == 4` the join is rebuilt at `:144` (`data.equip_links.push(openbim_document::EquipLink { port, element: el, connector: c.name.clone() });`), but for a Source or a typed Terminal the existing `EquipLink` is discarded with no message.

**What it means:** marking a Source (or a typed Terminal) on a run end that was joined to a fan coil/pump silently removes that equipment join from the model; only undo restores it.

**Confidence:** High.

## M7 — penetrations engine exists but the UI never uses it; export passes empty vectors

**Verdict: CONFIRMED**

**Real lines:** `crates/place/src/penetrations.rs:279` (`find`), `:381` (`register_csv`), `:107` (`Penetration`), `:148` (`Firestop`); `apps/desktop/src/export.rs:290-291`.

**Quoted code:**
```rust
// export.rs:290-291
        walls, wall_penetrations: vec![], firestops: Default::default(), slabs, roofs, railings, stairs, columns: columns.clone(), shafts: vec![],
        floor_penetrations: vec![], slab_voids, networks: if mep.segments.is_empty() { vec![] } else { vec![mep] }, families, items, rooms: vec![], wall_types: wall_types.into_iter().map(|(_, t)| t).collect(),
// penetrations.rs:279
pub fn find(chains: &[WallChain], runs: &[Run], merge_gap: f64) -> (Vec<Penetration>, Vec<RunIssue>) {
```
`grep` for `penetrations::find|register_csv` finds callers only inside `crates/place` (`crates/place/src/lib.rs:171,283` — the `Builder`) and `crates/place/tests/*`. **No file under `apps/` calls the engine**; `apps/` only uses `openbim_place::penetrations::{Run, RunSize}` as data types (`mep_tool.rs:8`, `export.rs:205-207`). The exported `Scene` goes straight to `openbim_ifc::export` (`export.rs:429`), not through the `Builder`, so no penetration is ever computed on the UI path.

**What it means:** openings/firestops/register are a complete library that the desktop app never invokes; exported IFC reports zero wall/floor penetrations and no firestops.

**Confidence:** High.

## M8 — Compute needs a typed/CSV catalogue, not the MEP Settings lists; Air preset leaves 1.5 m/s

**Verdict: CONFIRMED** (with one edge case noted)

**Real lines:** `apps/desktop/src/compute_ui.rs:69` (default 1.5), `:70` (catalogue empty), `:179-187` (settings from the dialog text), `:239-240` (Air/Water presets); `crates/calc/src/tree.rs:228` (`NoSize`), `:227` (`BadSettings`); `crates/document/src/mep.rs:172-202` (MepData has no catalogue).

**Quoted code:**
```rust
// compute_ui.rs:69-70
            max_velocity: 1.5, max_friction: 0.0, hazen_c: 0.0, count_static: false, keep_sizes: false, k: [0.3, 0.1, 0.5, 0.5],
            catalogue: String::new(), loading: String::new(), report: None, error: String::new(),
// compute_ui.rs:179-187 — the catalogue comes from the dialog text only
        let catalogue = parse_catalogue(&d.catalogue, is_duct(d.system))?;
        ...
                criteria: calc::Criteria { max_velocity: (d.max_velocity > 0.0).then_some(d.max_velocity), max_friction: (d.max_friction > 0.0).then_some(d.max_friction) },
                catalogue, count_static: d.count_static, loading, hazen_williams: (d.hazen_c > 0.0).then_some(d.hazen_c),
// compute_ui.rs:239-240 — Air/Water change fluid only, never max_velocity
                if ui.button("Water").on_hover_text("density 998, viscosity 1.0 mPa.s, specific heat 4186").clicked() { (d.density, d.viscosity_mpas, d.specific_heat, d.roughness_mm) = (998.0, 1.0, 4186.0, 0.045); }
                if ui.button("Air").on_hover_text("density 1.2, viscosity 0.0181 mPa.s, specific heat 1006").clicked() { (d.density, d.viscosity_mpas, d.specific_heat, d.roughness_mm) = (1.2, 0.0181, 1006.0, 0.09); }
// tree.rs:227-228
                if s.criteria.max_velocity.is_none() && s.criteria.max_friction.is_none() { return Err(TreeError::BadSettings("no limit to size by")); }
                (catalogue.iter().find(|z| meets(z)).ok_or(TreeError::NoSize(i))?.clone(), false)
```
`ComputeDialog.catalogue` is plain app state; `MepData` (`mep.rs:172-202`) has no catalogue field, so nothing is persisted with the model. The MEP Settings lists (`duct_widths_mm`, `pipe_diameters_mm`, `mep.rs:124-125`) are consumed only by the drawing options bar (`apps/desktop/src/mep_tool.rs:319`) — `grep` finds no reference in `compute_ui.rs` or `compute.rs`.

**What it means:** sizing requires a hand-typed or CSV-loaded catalogue each session; the Mechanical Settings size lists are decorative for Compute. With the default `max_velocity = 1.5` an air duct computed without pressing Air is sized with water properties (998 kg/m³, 1.0 mPa·s) against a 1.5 m/s limit. Edge case: if both `max_velocity` and `max_friction` are set to 0 the engine fails earlier with `BadSettings("no limit to size by")` rather than `NoSize`.

**Confidence:** High.

## M9 — electrical nodes/cables are abstract records, not elements

**Verdict: APPROXIMATE** — the substance is confirmed; the "grep `circuit` finds only `grouped_circuits`" sub-claim is WRONG.

**Real lines:** `crates/document/src/elec.rs:20-38` (`ElecNode`), `:47-63` (`ElecCable`), `:82-91` (one-feeder rule); `crates/document/src/mep.rs:194-196` (stored in `MepData`); `apps/desktop/src/model_view.rs:482-487` (plan-only).

**Quoted code:**
```rust
// elec.rs:21-26  — a point on a level, not an Element
pub struct ElecNode {
    pub id: u64,
    pub name: String,
    pub kind: ElecKind,
    pub at: [f64; 2],
    pub level: ElementId,
// elec.rs:57  — a circuit count field, not a circuit object
    pub grouped_circuits: u32,
// elec.rs:86-87 — every destination is fed once; a load cannot feed, a source cannot be fed
        let ok = src.is_some_and(|n| n.kind != ElecKind::Load) && dst.is_some_and(|n| n.kind != ElecKind::Source) && c.from != c.to
            && fed.insert(c.to) && c.via.iter().all(|p| p.iter().all(|v| v.is_finite())) && pos(c.extra_m) && c.grouped_circuits >= 1
// model_view.rs:482 — plan views only; there is no 3D element
        if matches!(app.tool, crate::Tool::ElecNode | crate::Tool::Cable) && !three_d {
```
They live in `MepData.elec_nodes` / `elec_cables` (`mep.rs:194-196`) with no `ElementId` of their own, so there is no Select, no Properties, no 3D, no `Element`-based schedule row. `crates/ifc/src/lib.rs` has cable-carrier segment/fitting entities (`:813-827`) but nothing for a node, board or load. No panel-schedule or cable-tray/conduit tool exists (those ribbon ids are M3 placeholders).

**What it means:** correct — the electrical model is a pure data structure drawn as plan symbols. The grep sub-claim fails: `grep -rn circuit apps/ crates/` also matches `apps/desktop/src/elec_ui.rs:392` ("Check short circuit and fault protection"), `apps/desktop/src/icons.rs:38` (`icon!("circuit")`), and many lines in `crates/calc/src/electrical.rs`. The absence of a circuit *object* is nonetheless true.

**Confidence:** High on substance; the grep claim is demonstrably false.

## M10 — schematic engine exists on main but has no caller in `apps/`; doc 06 is stale

**Verdict: CONFIRMED**

**Real lines:** `crates/document/src/schematic.rs:13` and `crates/document/src/schematic_view.rs:18-19,191` (engine callers inside the document crate); doc claim `docs/revit-style-ui/06-mep-schematic-automation.md:3,5`.

**Quoted code:**
```rust
// schematic_view.rs:188-191 — the document crate drives the engine
    pub fn generate_schematic_commands(&self, discipline: Discipline, mode: SpacingMode) -> Result<Vec<Command>, EditError> {
        let draft = crate::schematic::extract_schematic(self, discipline);
        let input = draft.layout_input(mode);
        let mut view = draft.render(&openbim_schematic::layout::layout(&input));
// schematic.rs:13
use openbim_schematic::layout::Connection;
```
`git ls-tree dcf8382` confirms `crates/document/src/schematic.rs`, `crates/document/src/schematic_view.rs`, `crates/schematic/Cargo.toml`, `crates/schematic/src/layout.rs`, `crates/schematic/src/lib.rs` all exist at the pinned SHA (added by commit `9f5a59e` "feat(schematic): the pure grid layout engine (06, SCH-002)"). `grep -rn "schematic|Schematic|GenerateSchematic|zones" --include=*.rs apps/` returns **nothing**.

**What it means:** the doc's own header is stale. `06-mep-schematic-automation.md:3` still says *"not implemented"* and `:5` says the engine *"is not part of this candidate"*, yet the SCH-001 zone model and SCH-002 grid layout are in the tree and reachable through `Document::generate_schematic_commands` — just with no desktop UI surface.

**Confidence:** High.

## Summary table

| Claim | Verdict | Real line(s) |
|---|---|---|
| M1 five families; fan coil Type-scope | CONFIRMED | library.rs:309-311, 327-334, 441-442 |
| M2 AHU + Component → fan coil | CONFIRMED | main.rs:464; ribbon.rs:68, 109 |
| M3 ribbon wiring counts | CONFIRMED | main.rs:495; ribbon.rs:56-105 |
| M4 Generate Layout constraints | APPROXIMATE (path wrong) | crates/document/src/layout.rs:26-33, 121-124, 138-140 |
| M5 MepSystemDef / Compute by System | CONFIRMED | mep.rs:105-111, 332; compute.rs:156-157, 201 |
| M6 end_point_click drops equip join | CONFIRMED | compute_ui.rs:132 |
| M7 penetrations unused by UI | CONFIRMED | export.rs:290-291; penetrations.rs:279, 381 |
| M8 compute catalogue / presets | CONFIRMED | compute_ui.rs:69-70, 179-187, 239-240; tree.rs:228 |
| M9 electrical abstract records | APPROXIMATE (grep sub-claim wrong) | elec.rs:21-26, 47-63, 82-91; mep.rs:194-196 |
| M10 schematic engine, no apps caller | CONFIRMED | schematic_view.rs:188-191; doc 06:3,5 |
