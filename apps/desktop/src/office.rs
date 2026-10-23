//! TEST-001: the office building the end-to-end tests build, save, reopen, export and read back (decided 14 Oct 2026:
//! the office for architecture; the data hall for the MEP chain is MEP-004). Two storeys, 20 x 12 m: a core, a store, a
//! meeting room and an open office on the ground floor, a plant room above, doors and windows, a stair, slabs, a
//! ceiling, rooms with numbers, a fan coil unit, a duct, and a small electrical network on the fan coil unit.
use openbim_document::{
    Category, Command, Document, ElecCable, ElecKind, ElecNode, ElementId, FamilySpec, Geometry, Material, MepData, SlabKind, SlabSpec, StairSpec,
};

pub(crate) struct Office {
    pub doc: Document,
    pub rooms: Vec<ElementId>,
    pub fcu: ElementId,
}

const W: f64 = 20.0;
const D: f64 = 12.0;

fn box_lines(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<([f64; 2], [f64; 2])> {
    vec![([x0, y0], [x1, y0]), ([x1, y0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, y0])]
}

pub(crate) fn office() -> Office {
    let mut d = Document::new();
    let ground = d.place_level("Level 1", 0.0).unwrap();
    let upper = d.place_level("Level 2", 3.6).unwrap();
    for (i, x) in [0.0, 10.0, 20.0].into_iter().enumerate() {
        d.place_grid([x, -1.0], [x, D + 1.0], ["A", "B", "C"][i]).unwrap();
    }
    for (i, y) in [0.0, 6.0, 12.0].into_iter().enumerate() {
        d.place_grid([-1.0, y], [W + 1.0, y], ["1", "2", "3"][i]).unwrap();
    }
    // the envelope of both storeys, and the partitions of the ground floor
    let outer = d.place_walls(ground, &box_lines(0.0, 0.0, W, D), 0.3, 3.6, 0.0, None).unwrap();
    d.place_walls(upper, &box_lines(0.0, 0.0, W, D), 0.3, 3.6, 0.0, None).unwrap();
    // a store (0..5 x 0..4), a meeting room (5..10 x 0..6) and the rest open
    d.place_wall(ground, [0.0, 4.0], [5.0, 4.0], 0.15, 3.6, 0.0).unwrap();
    let part = d.place_walls(ground, &[([5.0, 0.0], [5.0, 6.0]), ([5.0, 6.0], [10.0, 6.0]), ([10.0, 6.0], [10.0, 0.0])], 0.15, 3.6, 0.0, None).unwrap();
    // a plant room above (14..20 x 6..12)
    d.place_walls(upper, &[([14.0, 6.0], [14.0, 12.0]), ([14.0, 6.0], [20.0, 6.0])], 0.15, 3.6, 0.0, None).unwrap();
    // floors, a roof slab and a ceiling over the open office
    let slab = |outline: Vec<[f64; 2]>, kind| SlabSpec { outline, thickness_m: 0.25, top_offset_m: 0.0, kind, material: Material::Concrete };
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    d.place_slab(ground, &slab(rect(0.0, 0.0, W, D), SlabKind::Floor)).unwrap();
    d.place_slab(upper, &slab(rect(0.0, 0.0, W, D), SlabKind::Floor)).unwrap();
    d.place_ceiling(ground, rect(10.0, 0.0, W, D), 2.8, 0.05, Material::Unspecified).unwrap();
    // a door in the meeting room's partition and windows in the south wall
    let door = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
    d.place_family_in_wall(part[0], 3.0, &door).unwrap();
    let window = FamilySpec { family: "Window, fixed with mullions".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.9 };
    d.place_family_in_wall(outer[0], 3.0, &window).unwrap();
    d.place_family_in_wall(outer[0], 14.0, &window).unwrap();
    // a stair from the open office up to the plant room's storey
    d.place_stair(ground, &StairSpec { start: [11.0, 10.0], toward: [11.0, 2.0], width_m: 1.2, rise_m: 3.6, risers: 20, tread_m: 0.28, base_offset_m: 0.0 }).unwrap();
    // rooms: the store and the meeting room are closed spaces
    let mut rooms = Vec::new();
    for (at, number, name, dept) in [([2.5, 2.0], "G01", "Store", "Services"), ([7.5, 3.0], "G02", "Meeting room", "Office")] {
        let r = d.place_room(ground, at).unwrap();
        d.set_room_data(r, number, name, dept).unwrap();
        rooms.push(r);
    }
    // a fan coil unit in the meeting room, and a duct above it
    let fcu = d.place_family(ground, [7.5, 3.0], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 2.4 }).unwrap();
    d.transact(d.revision(), &[
        Command::Create { id: d.next_id(), category: Category::Duct, name: "Supply duct".into(), width_m: 0.3, height_m: 0.2, elevation_m: 2.6 },
    ]).unwrap();
    let duct = d.elements().find(|e| e.category == Category::Duct).unwrap().id;
    d.transact(d.revision(), &[Command::SetGeometry { id: duct, geometry: Geometry::Linear { start: [6.0, 3.0], end: [9.0, 3.0], level: ground } }]).unwrap();
    // the electrical network: utility, main board, a distribution board and the fan coil unit
    let base = openbim_document::elec::NODE_BASE;
    let mut fan = ElecNode::new(base + 3, "FCU-1", ElecKind::Load, [7.5, 3.0], ground);
    fan.watts = 450.0;
    fan.power_factor = 0.85;
    fan.element = Some(fcu);
    fan.connector = "Power".into();
    d.transact(d.revision(), &[Command::SetMep(Box::new(MepData {
        elec_nodes: vec![
            ElecNode::new(base, "Utility", ElecKind::Source, [0.5, 0.5], ground),
            ElecNode::new(base + 1, "MSB", ElecKind::Board, [1.5, 0.5], ground),
            ElecNode::new(base + 2, "DB-G", ElecKind::Board, [5.5, 6.5], ground),
            fan,
        ],
        elec_cables: vec![ElecCable::new(base, base + 1, true), ElecCable::new(base + 1, base + 2, true), ElecCable::new(base + 2, base + 3, false)],
        ..Default::default()
    }))]).unwrap();
    Office { doc: d, rooms, fcu }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(d: &Document, c: Category) -> usize { d.elements().filter(|e| e.category == c).count() }

    #[test]
    fn the_office_builds_and_its_rooms_are_closed_spaces() {
        let o = office();
        assert_eq!(count(&o.doc, Category::Level), 2);
        assert_eq!(count(&o.doc, Category::Wall), 4 + 4 + 1 + 3 + 2);
        assert_eq!(o.rooms.len(), 2);
        for r in &o.rooms { assert!(o.doc.room_shape(*r).is_ok(), "{:?}", o.doc.room_shape(*r)); }
    }

    fn class_counts(text: &str) -> std::collections::BTreeMap<String, usize> {
        let mut m = std::collections::BTreeMap::new();
        for l in text.lines() {
            if let Some(eq) = l.find("=IFC") {
                if let Some(par) = l[eq..].find('(') { *m.entry(l[eq + 1..eq + par].to_string()).or_default() += 1; }
            }
        }
        m
    }

    #[test]
    fn the_office_is_saved_reopened_exported_read_back_and_handed_to_the_electrical_design_unchanged() {
        let o = office();
        let (ground_rooms, fcu) = (o.rooms.clone(), o.fcu);
        let mut app = crate::Desktop::sample();
        app.document = o.doc;
        // save and reopen: the records are the same, line for line
        let path = std::env::temp_dir().join(format!("office-e2e-{}.obim", std::process::id())).to_str().unwrap().to_string();
        app.write_model(&path);
        let mut again = crate::Desktop::sample();
        again.load_model(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(openbim_document::native::records(&app.document), openbim_document::native::records(&again.document), "{}", again.message);
        // export: what the building holds is in the file
        let key = app.document.ensure_project_key();
        let out = crate::export::export_ifc(&app.document, "Office", key, "2026-10-23T00:00:00").unwrap();
        let c = class_counts(&out.text);
        eprintln!("{c:?}");
        assert_eq!(c.get("IFCSPACE"), Some(&2), "{c:?}");
        assert!(c.get("IFCSTAIR").copied().unwrap_or(0) + c.get("IFCSTAIRFLIGHT").copied().unwrap_or(0) > 0, "{c:?}");
        assert_eq!(c.get("IFCDOOR"), Some(&1), "{c:?}");
        assert_eq!(c.get("IFCWINDOW"), Some(&2), "{c:?}");
        assert!(c.get("IFCBUILDINGSTOREY") == Some(&2), "{c:?}");
        // read back from the geometry alone (as a file from another program): the rooms keep their numbers and names
        let foreign = crate::export::export_ifc_with(&app.document, "Office", key, "2026-10-23T00:00:00", crate::export::Schema::Ifc4x3, false).unwrap();
        let mut back = Document::new();
        let report = crate::import::import_ifc(&mut back, &foreign.text).unwrap();
        let mut rooms: Vec<(String, String)> = back.elements().filter(|e| e.category == Category::Room).map(|e| (back.room_text(e.id, "Number"), e.name.clone())).collect();
        rooms.sort();
        assert_eq!(rooms, vec![("G01".to_string(), "Store".to_string()), ("G02".to_string(), "Meeting room".to_string())], "{}", report.text());
        assert_eq!(back.elements().filter(|e| e.category == Category::Wall).count(), app.document.elements().filter(|e| e.category == Category::Wall).count(), "{}", report.text());
        // the electrical design gets the network, and the fan coil unit carries the id it has in the IFC file
        let json = app.exchange_text();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["readiness"]["can_compute"], true, "{}", v["readiness"]);
        assert_eq!(v["nodes"].as_array().unwrap().len(), 4);
        let gid = v["nodes"][3]["element"]["ifc_global_id"].as_str().unwrap().to_string();
        assert_eq!(gid, openbim_ifc::guid::global_id(key, 0, fcu.0));
        assert!(out.text.contains(&format!("'{gid}'")), "the fan coil unit's id is in the IFC file");
        assert_eq!(v["rooms"].as_array().unwrap().len(), ground_rooms.len());
        // the cable to the fan coil unit is the drawn route: board (5.5, 6.5) to the unit (7.5, 3.0), on one level
        let len = v["cables"][2]["length_m"].as_f64().unwrap();
        assert!((len - (2.0f64.powi(2) + 3.5f64.powi(2)).sqrt()).abs() < 1e-9, "{len}");
    }
}
