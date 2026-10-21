# IFC export/import verification

> Scope: `apps/desktop/src/{export.rs,import.rs,file.rs,model_edit.rs}`, `crates/ifc/src/{lib.rs,reader.rs,guid.rs,pset_map.rs,quantities.rs,families.rs}`, `crates/document/src/{lib.rs,native.rs,mep.rs}`, `crates/oplog`.
> Worktree `C:/Users/wilfr/hermes-first-task/revit-book-refs` at SHA `6ac8addc9550fcbed3d2b5cce868a540fe0a0780` (main `dcf8382`).
> Method: code reading only — no cargo, no builds, no edits. All line numbers below are from the file as read at this SHA. Claim line numbers in the task are approximate; the real ones are given per item.

---

## I1 — Every run exported as `System::Other`; only `add_connections` copies a system; `run_systems` never read

**Verdict: CONFIRMED** (all four parts).

**Real lines** — `apps/desktop/src/export.rs:203` (stale comment), `:205`, `:207`, `:217`, `:302`, `:322`.

**Quoted code**

`export.rs:203`
```rust
                // the document does not say which system a run belongs to yet
```
`export.rs:204-208`
```rust
                let run = if el.category == Category::Pipe {
                    openbim_place::penetrations::Run::round(&el.name, Domain::Pipe, System::Other, a, b, el.width_m)
                } else {
                    openbim_place::penetrations::Run::rect(&el.name, Domain::Duct, System::Other, a, b, el.width_m, el.height_m)
                };
```
`export.rs:322` (the only place a system is copied from the model)
```rust
                if let (Some(&si), Some(&fi)) = (seg.get(&id.0), fit.get(&fid)) { net.segments[si].run.system = data.fittings[fi].system; }
```
`export.rs:299-301` (doc comment on `add_connections`, same stale wording)
```rust
/// stays joined to the very ducts and pipes it was joined to. A run takes the system of the fittings it is joined to
/// (the model does not say it otherwise yet). A link whose ends no longer meet (a run was moved after) is left out and
```
`export.rs:304` — `let data = doc.mep();` reads `fittings`, `links`, `taps`, `insulations` — never `run_systems`.

**What it means**: the exporter ignores `MepData::run_systems` entirely; `grep -n run_system apps/desktop/src/export.rs` → **0 hits** (only `run_systems` in `import.rs:490`, `main.rs`, `mep_tool.rs`, `systems_ui.rs`, and `crates/document/*`). A run keeps `System::Other` unless a *joined fitting* happens to carry a system, and `System::Other` maps to `.NOTDEFINED.` in the file (`crates/family/src/mep.rs:235-253`). The comment at `:203` is stale because the document *does* say which system a run belongs to (`crates/document/src/mep.rs:189`, `run_systems`, `mep.rs:208 run_system()`), and the exporter also has `doc` in scope (`scene_of` takes `&Document`).

**Confidence: high** — direct reads plus a whole-tree grep for `run_system` in the exporter.

---

## I2 — Rooms exported as `rooms: vec![]`; Grid/Room/Level/Opening skipped; `georef: None` — while the crate *can* write `IfcSpace`

**Verdict: CONFIRMED.**

**Real lines** — skip list `apps/desktop/src/export.rs:198`; empty rooms `export.rs:291`; georef `export.rs:292`; site warning `export.rs:432`. Crate capability: `crates/ifc/src/lib.rs:1497-1556` (`write_spaces`), called at `lib.rs:964`; boundary writer `lib.rs:1576-1620` (`write_boundaries`, `IFCRELSPACEBOUNDARY2NDLEVEL`), called only when not 2x3 at `lib.rs:1553-1555`; quantities `lib.rs:1531`; tests `crates/ifc/tests/ifc.rs:258-277`, `crates/ifc/tests/boundaries.rs:191-196`.

**Quoted code**

`export.rs:198`
```rust
            (Category::Level | Category::Grid | Category::Tag | Category::Dimension | Category::Room | Category::Opening | Category::Section | Category::Elevation | Category::BeamSystem | Category::Truss | Category::Rebar, _) => {}
```
`export.rs:291-292`
```rust
        floor_penetrations: vec![], slab_voids, networks: if mep.segments.is_empty() { vec![] } else { vec![mep] }, families, items, rooms: vec![], wall_types: wall_types.into_iter().map(|(_, t)| t).collect(),
        wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![],
```
`export.rs:432`
```rust
    if doc.site().is_some() { warnings.push("the project site is not exported: it needs a coordinate reference system".into()); }
```
`lib.rs:1499-1506` (rooms are iterated — but the vector is always empty from the app)
```rust
    for (ri, r) in scene.rooms.iter().enumerate() {
        let room = &r.room;
        if room.level >= scene.levels.all().len() {
            return Err(ExportError::Unsupported(format!("room {}: no level {}", room.name, room.level)));
        }
```
`lib.rs:1522-1531`
```rust
        let sp = o.s.add(format!("IFCSPACE({g},$,{},$,$,#{pl},#{shape},$,.ELEMENT.,.INTERNAL.,$)", string(&room.name)));
        ...
        o.qto(sp, "Qto_SpaceBaseQuantities", &quantities::space(&clear));
```
`lib.rs:1553-1555`
```rust
        if !o.v2 {
            write_boundaries(o, r, sp);
        }
```
`lib.rs:964` — `write_spaces(&mut o)?;` (unconditional).

`crates/ifc/tests/ifc.rs:258-271`
```rust
fn walls_slabs_and_rooms_get_base_quantities() {
    ...
    assert_eq!((out.report.walls, out.report.slabs, out.report.spaces, out.report.quantity_sets), (1, 1, 1, 3));
    for n in ["Qto_WallBaseQuantities", "Qto_SlabBaseQuantities", "Qto_SpaceBaseQuantities"] {
    ...
    assert!(out.text.contains("IFCSPACE("));
```
`crates/ifc/tests/boundaries.rs:191-196`
```rust
fn the_boundaries_are_written_with_their_geometry() {
    let s = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let out = export(&s).unwrap();
    assert_eq!(out.report.space_boundaries, 6);
    assert_eq!(out.text.matches("IFCRELSPACEBOUNDARY2NDLEVEL(").count(), 6);
```
Note also: `Scene::rooms` is a real field (`crates/ifc/src/lib.rs:389`) and `write_spaces` runs even for IFC2X3 (`lib.rs:964` is outside the `if v2` branch) — so 2x3 files *would* get `IFCSPACE` too; only the app's `rooms: vec![]` prevents it. Opening categories are written by the crate (`lib.rs:1100,1338,1352,1371` `IFCOPENINGELEMENT`) but the app drops `Category::Opening` at `:198`.

**What it means**: the app exports no rooms, no grids, no level-adjacent geometry, no openings, and no georeferencing; the underlying crate has working, tested `IfcSpace` + quantities + second-level boundary support that is unreachable from the desktop app.

**Confidence: high.**

---

## I3 — User `MepSystemDef` systems are never written; one `IfcDistributionSystem` per `System` enum code; `IfcRelAssignsToGroup` is used (not …ByFactor)

**Verdict: CONFIRMED** in substance. `IfcRelAssignsToGroupByFactor` does **not** exist in the tree; `IfcRelAssignsToGroup` is used, and only for MEP pieces.

**Real lines** — `crates/ifc/src/lib.rs:1706` (`by_system`), `:1739` (segments), `:1781` (fittings), `:1831-1838` (the systems loop); `crates/document/src/mep.rs:177-178`, `:105-111` (`MepSystemDef`).

**Quoted code**

`lib.rs:1831-1838`
```rust
        // systems
        for (code, elems) in by_system {
            let sys = System::from_code(code).unwrap_or(System::Other);
            let g = o.gen_gid();
            let grp = o.s.add(format!("IFCDISTRIBUTIONSYSTEM({g},$,{},$,$,$,.{}.)", string(&format!("{} {}", sys.name(), ni + 1)), sys.ifc_system()));
            let g = o.gen_gid();
            o.s.add(format!("IFCRELASSIGNSTOGROUP({g},$,$,$,({}),$,#{grp})", elems.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
        }
```
`lib.rs:1706`
```rust
        let mut by_system: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
```
`lib.rs:1739` and `:1781`
```rust
            by_system.entry(r.system.code()).or_default().push(e);
```
```rust
            by_system.entry(nf.system.code()).or_default().push(e);
```
The grouping key is `System::code()` — a u8 of the *enum* (`crates/family/src/mep.rs:219-221`), not the user's system name or `MepSystemDef`.

Whole-tree grep: `grep -rn "IFCRELASSIGNSTOGROUP\|IFCRELASSIGNSTOGROUPBYFACTOR" crates/ apps/ --include=*.rs` → **one hit, `crates/ifc/src/lib.rs:1837`**. `IFCRELASSIGNSTOGROUPBYFACTOR` has **0 hits**.

**What it means**: (a) `MepData::systems: Vec<MepSystemDef>` (`mep.rs:178`) is never read by the exporter — `grep -n "\.systems" apps/desktop/src/export.rs crates/ifc/src/lib.rs` returns only the unrelated penetration `p.systems` at `lib.rs:1106,1375`. A user-created "Mechanical System" named e.g. "AHU-1 Loop" is therefore not written as an `IfcSystem`; only runs/fittings already carrying a `System` get one. (b) Exactly one `IFCDISTRIBUTIONSYSTEM` per `System` code per network, named `"<System name> N"` with `System::name()` (`crates/family/src/mep.rs:258`). (c) Membership is `IFCRELASSIGNSTOGROUP` (no factor attribute — correct: …ByFactor takes a `MeasureWithUnit`). (d) Equipment written as a *bare* `IFCFLOWTERMINAL` placeholder (`lib.rs:1798-1801`) is never assigned to its distribution system — it is not added to `by_system` (only segments and fittings are, at `:1739`/`:1781`), and `write_networks` has no `IFCRELASSIGNSTOGROUP` for it.

**Confidence: high** — grep-verified for `ByFactor`, direct reads for the rest.

---

## I4 — Product ids stable; storeys/relationships/psets/systems use a running counter; project key fixed at 1, name `Project1`

**Verdict: CONFIRMED** (with one nuance: the counter is *stable across unchanged exports*, so it only shifts when the set of non-element entities changes).

**Real lines** — stable: `crates/ifc/src/lib.rs:638-643` (`gid`), `crates/ifc/src/guid.rs:14-15`; counter: `lib.rs:602` (`aux`), `:645-648` (`gen_gid`), callers `:751,756,772,774,923,933,935,937,938,942,948`; project key/name: `apps/desktop/src/model_edit.rs:305`.

**Quoted code**

`lib.rs:638-643`
```rust
    fn gid(&mut self, element: u64, part: u16) -> String {
        if !self.used.insert((element, part)) {
            self.dup.get_or_insert(element);
        }
        string(&guid::global_id(self.key, part, element))
    }
```
`guid.rs:14-15`
```rust
pub fn bits(project_key: u64, part: u16, element: u64) -> u128 {
    (u128::from(project_key & MAX_PROJECT_KEY) << 80) | (u128::from(part) << 64) | u128::from(element)
}
```
`lib.rs:644-648`
```rust
    /// ids for things that are not model elements (the site, relationships, property sets)
    fn gen_gid(&mut self) -> String {
        self.aux += 1;
        string(&guid::global_id(self.key, 0, (0xFFFF_FFFFu64 << 32) | self.aux))
    }
```
`lib.rs:942-946` (storeys)
```rust
        let g = o.gen_gid();
        let e = o.r(l.elevation, "level elevation");
        let st = o.s.add(format!("IFCBUILDINGSTOREY({g},$,{},$,$,#{pl},$,$,.ELEMENT.,{e})", string(&l.name)));
```
`lib.rs:751-757` (psets + the relationship)
```rust
        let g = self.gen_gid();
        self.s.add(format!("IFCPROPERTYSET({g},$,{},$,({}))", string(name), ids.join(",")))
    }
    fn pset(&mut self, owner: u32, name: &str, props: Vec<(&str, Val)>) {
        let ps = self.pset_entity(name, props);
        let g = self.gen_gid();
        self.s.add(format!("IFCRELDEFINESBYPROPERTIES({g},$,$,$,(#{owner}),#{ps})"));
```
`model_edit.rs:305`
```rust
        match crate::export::export_ifc_with(&self.document, "Project1", 1, &crate::export::iso_time(secs), schema, own_data) {
```

**What it means**: product ids are a pure function of `(project_key, part, element number)` — stable. Everything else (site, building, storeys, relationships, property sets, element quantity sets, distribution systems, boundaries) is `0xFFFF_FFFF_0000_0000 | aux`, i.e. a 1-based counter over non-element entities; because the counter is bumped in the same traversal order, an edit that adds a storey, toggles `own_data` (adds `OpenBIM_Document`, `lib.rs:907`), or changes an element count shifts every later aux id. `next_id` is `elements.keys().next_back() + 1` (`crates/document/src/lib.rs:571`), so deleting the newest element does reuse its number. With `project_key = 1` and name `"Project1"` hard-coded in the only export call site, two different projects with equal element ids produce byte-identical GlobalIds.

**Confidence: high.**

---

## I5 — Import `MEMBERS` list; doors/windows/openings/spaces/equipment/ports/fittings skipped; foreign GlobalIds discarded; m/mm only

**Verdict: CONFIRMED.**

**Real lines** — `crates/ifc/src/reader.rs:1-3`, `:139-153` (`MEMBERS`), `:156` (`NOT_PRODUCTS`), `:920-924` (skip counter); units `:576-598`; guid capture `:888-893`; discarded at use `:903`; reader test `crates/ifc/tests/reader.rs:370-383`.

**Quoted code**

`reader.rs:1-3`
```rust
//! Reads the building out of an IFC file (IFC2X3, IFC4 or IFC4.3): the storeys, and the walls, beams, columns, slabs,
//! footings, ducts and pipes that are an extruded profile, with their property sets, materials and types.
//! Everything else is counted by class in `skipped`, so that an import can say what it left out.
```
`reader.rs:139-153`
```rust
const MEMBERS: [&str; 13] = [
    "IFCBEAM",
    "IFCMEMBER",
    "IFCCOLUMN",
    "IFCSLAB",
    "IFCROOF",
    "IFCCOVERING",
    "IFCSTAIR",
    "IFCFOOTING",
    "IFCWALL",
    "IFCWALLSTANDARDCASE",
    "IFCPIPESEGMENT",
    "IFCDUCTSEGMENT",
    "IFCCABLECARRIERSEGMENT",
];
```
`reader.rs:920`
```rust
            *out.skipped.entry(e.class.clone()).or_default() += 1;
```
`reader.rs:888-893`
```rust
                guid: e
                    .args
                    .first()
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
```
`reader.rs:903` — `storey: contained.get(&id).copied(),` (no guid use).
`apps/desktop/src/import.rs:175` — identity comes from the app's own property set instead: `imp.members.iter().filter_map(|m| m.psets.get("OpenBIM_Element")...("ElementNumber")...)`. A foreign file with no `OpenBIM_Element` set gets no preserved id → new numbers.
`reader.rs:576-598` (units)
```rust
        for value in assignment.args.first().ok_or_else(invalid)?.as_list() {
            let unit = f.deref(value).ok_or_else(invalid)?;
            if unit.args.get(1).and_then(Value::as_enum) != Some("LENGTHUNIT") {
                continue;
            }
            if unit.class != "IFCSIUNIT"
                || unit.args.get(3).and_then(Value::as_enum) != Some("METRE")
            {
                return Err(invalid());
            }
            scales.push(match unit.args.get(2) {
                Some(Value::Null) => 1.0,
                Some(Value::Enum(p)) if p == "MILLI" => 0.001,
                _ => return Err(invalid()),
            });
```
`crates/ifc/tests/reader.rs:370-376`
```rust
fn null_placement_unsupported_products_are_counted() {
    let got = reader::read(&step("#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);#11=IFCUNITASSIGNMENT((#10));#12=IFCPROJECT('p',$,'P',$,$,$,$,(),#11);#1=IFCOPENINGELEMENT('o',$,'O',$,$,$,$,$,$);#2=IFCDOOR('d',$,'D',$,$,$,$,$,$);#3=IFCSPACE('s',$,'S',$,$,$,$,$,$);")).unwrap();
    for class in ["IFCOPENINGELEMENT", "IFCDOOR", "IFCSPACE"] {
        assert_eq!(got.skipped.get(class), Some(&1));
    }
```
`reader.rs:444-445` — geometry is `IFCEXTRUDEDAREASOLID` only (`fn body`); booleans are refused (`reader.rs:608-614`).

**What it means**: exactly the 13 member classes above become members (extruded profile only). `IFCDOOR`, `IFCWINDOW`, `IFCOPENINGELEMENT`, `IFCSPACE`, `IFCCURTAINWALL`, `IFCEQUIPMENT`, `IFCDISTRIBUTIONPORT` and fittings are either in `NOT_PRODUCTS` or counted in `ImportReport.skipped` (also `IFCREINFORCINGBAR`, explicitly, `:908-911`). `Member.guid`/`TypeObj.guid` are parsed but never carried into the document; element identity is re-derived from `OpenBIM_Element/ElementNumber`, so a foreign GlobalId is lost. Units must be a single `IFCSIUNIT` LENGTHUNIT `.METRE.` with prefix none or `MILLI` — anything else is a hard `ReadError::Invalid`.

**Confidence: high.**

---

## I6 — `.obim` text snapshot, atomic Save, no autosave/backup/recovery; `crates/oplog` exists with zero dependants

**Verdict: CONFIRMED.**

**Real lines** — `apps/desktop/src/file.rs:1-4`, `:9-25`, `:59-66`, `:475` (save binding); `crates/document/src/native.rs:130-136`; `crates/oplog/Cargo.toml:2`; `crates/document/src/lib.rs:1072` (`clear_undo_history`).

**Quoted code**

`file.rs:1-4`
```rust
//! The app's own file: the document as the lines of `openbim_document::native`, one per element, in a `.obim` text
//! file. Saving writes a temporary file beside the target and renames it, so a failed write never damages the file
//! that was there.
```
`file.rs:59-66`
```rust
    pub(crate) fn write_model(&mut self, path: &str) {
        let tmp = format!("{path}.saving");
        let result = std::fs::write(&tmp, file_text(&self.document)).and_then(|()| std::fs::rename(&tmp, path));
        self.message = match result {
            Ok(()) => { self.file.path = Some(path.into()); self.file.saved_revision = self.document.revision(); format!("Saved {path}") }
            Err(e) => { let _ = std::fs::remove_file(&tmp); format!("Could not save {path}: {e}") }
        };
    }
```
`native.rs:130-136`
```rust
pub fn records(doc: &Document) -> Vec<String> {
    let mut out = vec![format!("F|{FORMAT}")];
    if let Some(s) = doc.site() { out.push(format!("S|lat={:?}|lon={:?}|ang={:?}|utc={:?}", s.latitude, s.longitude, s.angle_to_true_north_deg, s.utc_offset_hours)); }
    out.extend(doc.elements().map(element_record));
    out.extend(mep_records(doc.mep()));
    out
}
```
`crates/oplog/Cargo.toml:2`
```toml
name = "openbim-oplog"
```

Dependency grep — `grep -rn "openbim-oplog\|openbim_oplog" --include=*.toml --include=*.rs --include=*.lock .` returns **only** `crates/oplog/Cargo.toml`, `Cargo.lock:2594`, and `crates/oplog/{examples,tests}` — **no `[dependencies]` entry anywhere**. It is listed as a workspace member (`Cargo.toml:3`) but nothing depends on it. `grep -rn "autosave\|backup\|recovery\|crash"` over `apps/` and `crates/document/` → 0 hits. The only "Save" trigger is the user action `main.rs:475` → `self.save_file(false)`. Loading clears undo history: `file.rs:25` → `doc.clear_undo_history();` (`document/src/lib.rs:1072`).

**What it means**: the working file is a human-readable text snapshot (`obim-records-1`, `native.rs:9`) written atomically via a `.saving` temp + rename; there is no autosave, no backup copy, no crash recovery, and `crates/oplog` (binary log with checksums, snapshots, merge) is dead code as far as the shipped crates are concerned.

**Confidence: high.**

---

## I7 — Every undo step clones the whole element map, uncapped; every transact revalidates all elements

**Verdict: CONFIRMED.**

**Real lines** — `crates/document/src/lib.rs:558` (history field), `:1084` (clone), `:1262-1263` (validate), `:1267` (push), `:1292` (redo push).

**Quoted code**

`lib.rs:558`
```rust
    history: Vec<(BTreeMap<ElementId, Element>, Option<Site>, MepData)>,
```
`lib.rs:1084`
```rust
        let mut trial = self.elements.clone();
```
`lib.rs:1262-1263`
```rust
        if trial.values().any(|e| !e.valid()) { return Err(EditError::InvalidElement); }
        spatial::validate(&trial)?;
```
`lib.rs:1267`
```rust
        self.history.push((std::mem::replace(&mut self.elements, trial), std::mem::replace(&mut self.site, trial_site), std::mem::replace(&mut self.mep, trial_mep)));
```
`lib.rs:1292`
```rust
        self.history.push((std::mem::replace(&mut self.elements, after.0), std::mem::replace(&mut self.site, after.1), std::mem::replace(&mut self.mep, after.2)));
```
`lib.rs:1064-1065` — the only trimming that exists is caller-driven:
```rust
    pub fn merge_steps_since(&mut self, mark: usize) { if self.history.len() > mark + 1 { self.history.truncate(mark + 1); } }
```

**What it means**: `transact` clones the entire `BTreeMap<ElementId, Element>` for a trial, validates **every** element (`trial.values().any(...)`) plus a whole-model `spatial::validate`, then swaps and pushes the *previous full map* onto `history`; `redo` does the same onto `future`. There is no cap, no ring buffer, no delta/structural sharing: memory grows linearly with the number of transactions, and cost per transaction is O(all elements + whole spatial validation). `merge_steps_since`/`roll_back_to` exist but nothing in the app calls them (grep shows only the definition).

**Confidence: high.**

---

## I8 — Only `Pset_StairCommon` plus wall/slab/door/window/space quantities; no `Pset_*Common` (IsExternal/LoadBearing), no `Qto_` for ducts/pipes/fittings/equipment/beams

**Verdict: CONFIRMED**, with one correction: `Pset_WallCommon` and `Pset_SlabCommon` **are** written — but each carries only `FireRating`. `IsExternal` and `LoadBearing` appear **nowhere** in the IFC crate.

**Real lines** — `crates/ifc/src/lib.rs:1067,1071` (wall), `:1323,1325` (slab), `:1488` (stair), `:1531` (space), `:1801` (equipment), `:1735,1774` (runs/fittings); `crates/ifc/src/families.rs:346-347,411-412` (door/window); `crates/ifc/src/pset_map.rs:1-3`; `crates/ifc/src/quantities.rs:88,116,130,135`.

**Quoted code**

`lib.rs:1067-1071`
```rust
            Ok(q) => o.qto(wall, "Qto_WallBaseQuantities", &q),
...
            o.pset(wall, "Pset_WallCommon", vec![("FireRating", Val::Label(fire_label(m)))]);
```
`lib.rs:1323-1325`
```rust
        o.qto(slab, "Qto_SlabBaseQuantities", &quantities::slab(s, void_area));
...
            o.pset(slab, "Pset_SlabCommon", vec![("FireRating", Val::Label(fire_label(m)))]);
```
`lib.rs:1488-1491`
```rust
        o.pset(stair, "Pset_StairCommon", vec![
            ("NumberOfRiser", Val::Int(i64::from(s.risers))), ("NumberOfTreads", Val::Int(i64::from(s.risers - 1))),
            ("RiserHeight", Val::Len(s.riser_height)), ("TreadLength", Val::Len(s.tread_length)),
        ]);
```
`lib.rs:1801`
```rust
                    o.pset(e, "OpenBIM_Equipment", vec![("Placeholder", Val::Bool(true))]);
```
`lib.rs:1766-1774` (fittings, no quantity set)
```rust
            let mut props = vec![("PartCode", Val::Label(f.part_code.clone())), ("System", Val::Label(nf.system.ifc_system().into()))];
            ...
            o.pset(e, "OpenBIM_Mep", props);
```
`families.rs:346-347`
```rust
                Category::Door => o.qto(occ, "Qto_DoorBaseQuantities", &crate::quantities::opening_item(ow, oh)),
                Category::Window => o.qto(occ, "Qto_WindowBaseQuantities", &crate::quantities::opening_item(ow, oh)),
```
`pset_map.rs:1-3`
```rust
//! Where family and shared parameters go in the IFC file. A value with a row in the table is written in the
//! property set and under the property name the row names, with the value type the row names; everything else
//! stays in the custom sets (`OpenBIM_Family` on the type, `OpenBIM_Instance` on the occurrence).
```
`quantities.rs` public functions: `wall` (`:88`), `slab` (`:116`), `opening_item` (`:130`), `space` (`:135`) — nothing for duct/pipe/fitting/equipment/beam/column/roof.

Whole-tree grep for `IsExternal|LoadBearing` over `crates/ifc/src/ crates/ifc/tests/` → **0 hits**. Every `o.pset(` / `o.qto(` call site is enumerated above (14 `pset` + 6 `qto`), so the claim is exhaustive.

**What it means**: the only standard `Pset_*Common` written are `Pset_WallCommon`, `Pset_SlabCommon`, `Pset_StairCommon` — and the wall/slab ones carry only `FireRating` (both also lack `IsExternal` and `LoadBearing`, the two properties Revit-family parameters normally map to). Ducts, pipes, fittings, equipment, beams and columns get **no** `Qto_*BaseQuantities` at all (only custom `OpenBIM_Mep` / `OpenBIM_Equipment` sets). Duct/pipe/equipment/beam geometry is written (`lib.rs:1700-1841`, `:1178`) but unmeasured. The `PsetMap` table (`pset_map.rs`) can route family/shared parameters into arbitrary psets, but the app passes `pset_map: Default::default()` (`export.rs:293`) so the table is always empty.

**Confidence: high.**

---

## I9 — IFC2X3 export drops non-door/window families (equipment)

**Verdict: CONFIRMED** (and the wording "equipment" is right — the warning names luminaires, furniture, equipment).

**Real lines** — `crates/ifc/src/families.rs:382-394` (skip), `:425-427` (warning); dispatch `crates/ifc/src/lib.rs:952-956`; test `crates/ifc/tests/ifc.rs:335-363`.

**Quoted code**

`families.rs:382-394`
```rust
/// IFC2X3: the doors and windows only, as plain boxes in their wall openings. Luminaires and other families have no
/// place in a 2x3 shell file (a 2x3 light fixture is a flow terminal, which DIALux ignores); they are counted in the warnings.
pub(crate) fn write_openings_2x3(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    let mut skipped = 0usize;
    for it in &scene.items {
        let Some(fam) = scene.families.get(it.family).map(|f| &f.family) else {
            return Err(ExportError::Unsupported(format!("{}: no family {}", it.name, it.family)));
        };
        let cat = fam.category();
        let (Category::Door | Category::Window, Some([ow, oh_, sill])) = (cat, it.instance.opening) else {
            skipped += 1;
            continue;
        };
```
`families.rs:425-427`
```rust
    if skipped > 0 {
        o.report.warnings.push(format!("IFC2X3: {skipped} placed families (luminaires, furniture, equipment) are not written; use the luminaire schedule"));
    }
```
`lib.rs:952-956`
```rust
    if v2 {
        families::write_openings_2x3(&mut o)?;
    } else {
        families::write_families(&mut o)?;
    }
```
`crates/ifc/tests/ifc.rs:340-347,360-362`
```rust
    let other = fams.iter().position(|f| !matches!(f.category(), Category::Door | Category::Window)).unwrap();
    ...
    assert_eq!(out.report.items, 2);
    assert_eq!(out.text.matches("IFCDOOR(").count(), 2);
```
```rust
    let w: Vec<&String> = out.report.warnings.iter().filter(|w| w.contains("placed families")).collect();
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("2 placed families"), "{}", w[0]);
```

**What it means**: in the 2x3 schema path, only `Category::Door`/`Category::Window` items with an `opening` are written; every other placed family (equipment, furniture, luminaires, etc.) is dropped and reported once. Doors/windows become plain extruded boxes with `IFCDOOR`/`IFCWINDOW` and no IFC4 types/maps/ports (`lib.rs:963-965`). Note the *crate* supports the rest in IFC4 (`families.rs:write_families`, `lib.rs:955`) and even supports equipment as `IFCFLOWTERMINAL` in the network path (`lib.rs:1798-1801`) — but `lib.rs:966-968` warns "IFC2X3: service networks are not written", so in a 2x3 export equipment is dropped twice over.

**Confidence: high.**

---

## Summary table

| Claim | Verdict | Real line(s) |
|---|---|---|
| I1 runs always `System::Other`; only `add_connections` copies a system; `run_systems` unread; stale comment | **CONFIRMED** | `export.rs:203,205,207,217,302,322` |
| I2 `rooms: vec![]`; Grid/Room/Level/Opening skipped; `georef: None`; crate can write `IfcSpace` | **CONFIRMED** | `export.rs:198,291,292,432`; `ifc/lib.rs:964,1497-1556,1576`; `tests/ifc.rs:258-277`; `tests/boundaries.rs:191-196` |
| I3 no `MepSystemDef`→`IfcSystem`; one `IFCDISTRIBUTIONSYSTEM` per code; `IFCRELASSIGNSTOGROUP` used, no `…ByFactor` | **CONFIRMED** | `ifc/lib.rs:1706,1739,1781,1831-1838`; `document/mep.rs:178,105` |
| I4 stable product ids vs running counter; key=1, name "Project1" | **CONFIRMED** | `ifc/lib.rs:638-648,942`; `guid.rs:14-15`; `model_edit.rs:305` |
| I5 13-class `MEMBERS`, doors/windows/spaces/equipment/ports skipped, guids discarded, m/mm | **CONFIRMED** | `ifc/reader.rs:139-153,576-598,888-893,920`; `tests/reader.rs:370-376` |
| I6 `.obim` atomic text snapshot, no autosave/backup/recovery, `oplog` orphaned | **CONFIRMED** | `file.rs:1-4,59-66`; `document/native.rs:130-136`; `oplog/Cargo.toml:2` (0 dependants) |
| I7 whole-map clone per undo step, uncapped; full revalidation per transact | **CONFIRMED** | `document/lib.rs:558,1084,1262-1263,1267,1292` |
| I8 only `Pset_StairCommon` (+ wall/slab with `FireRating` only); no `IsExternal`/`LoadBearing`; no `Qto_` for ducts/pipes/fittings/equipment/beams | **CONFIRMED** (see note) | `ifc/lib.rs:1067,1071,1323,1325,1488,1735,1774,1801`; `families.rs:346-347,411-412`; `quantities.rs:88,116,130,135` |
| I9 IFC2X3 drops non-door/window families | **CONFIRMED** | `ifc/families.rs:382-394,425-427`; `ifc/lib.rs:952-956`; `tests/ifc.rs:335-363` |

**Notes on the two claims that needed precision**

- I3's `…ByFactor` detail is exactly right: the code writes `IFCRELASSIGNSTOGROUP` (`ifc/lib.rs:1837`), the only hit for either name in the whole tree; `IFCRELASSIGNSTOGROUPBYFACTOR` has 0 hits.
- I8: `Pset_WallCommon` and `Pset_SlabCommon` *are* written — but each contains only `FireRating` (`ifc/lib.rs:1071,1325`), so the substance (no `IsExternal`, no `LoadBearing`, no `Qto_` for MEP/equipment/beams) holds; both `IsExternal` and `LoadBearing` have 0 hits anywhere under `crates/`.
