//! The ids of relationships and property sets come from what they are, not from the order they were written in (B11).
use openbim_arch::{Level, Levels, Room};
use openbim_ifc::{export, export_ifc2x3, export_ifc4x3, RoomItem, Scene};
use std::collections::BTreeSet;

fn room(id: u64, name: &str, x: f64) -> RoomItem {
    RoomItem { id, number: String::new(), room: Room { name: name.into(), usage: "Office".into(), occupancy: 2.0, level: 0, outline: vec![[x, 0.0], [x + 4.0, 0.0], [x + 4.0, 3.0], [x, 3.0]], holes: vec![], height: 2.7 } }
}

fn scene(rooms: Vec<RoomItem>) -> Scene {
    Scene {
        project_name: "P".into(), project_key: 7, timestamp: "2026-10-23T00:00:00".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], roofs: vec![], railings: vec![],
        stairs: vec![], columns: vec![], beams: vec![], materials: Default::default(), shafts: vec![], floor_penetrations: vec![],
        slab_voids: vec![], networks: vec![], families: vec![], items: vec![], rooms, grids: vec![], system_defs: vec![], assemblies: vec![],
        room_lighting: Default::default(), wall_types: vec![], types: vec![], member_info: Default::default(), native: vec![],
        wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], pset_map: Default::default(),
    }
}

fn first_quoted(line: &str) -> String { line.split('\'').nth(1).unwrap_or("").to_string() }

/// The ids of the entities that belong to the room named `name`: its space and everything that refers to the space or
/// to a property set of it.
fn ids_of_room(text: &str, name: &str) -> BTreeSet<String> {
    let space = text.lines().find(|l| l.contains("=IFCSPACE(") && l.contains(&format!(",'{name}',"))).expect("the space");
    let space_no = space.split('=').next().unwrap().to_string();
    let mut out = BTreeSet::new();
    for l in text.lines().filter(|l| l.contains(&format!("{space_no})")) || l.contains(&format!("{space_no},"))) {
        if l.starts_with(&format!("{space_no}=")) || !l.contains("=IFCREL") { if l.starts_with(&format!("{space_no}=")) { out.insert(first_quoted(l)); } continue; }
        out.insert(first_quoted(l));
    }
    out
}

fn all_ids(text: &str) -> Vec<String> {
    text.lines().filter(|l| l.contains("=IFC") && l.contains("('")).map(first_quoted).filter(|g| g.len() == 22).collect()
}

#[test]
fn adding_a_room_before_another_leaves_the_ids_of_the_other_alone() {
    for (name, f) in [("4", export as fn(&Scene) -> _), ("4x3", export_ifc4x3), ("2x3", export_ifc2x3)] {
        let one = f(&scene(vec![room(2, "B", 10.0)])).unwrap().text;
        let two = f(&scene(vec![room(1, "A", 0.0), room(2, "B", 10.0)])).unwrap().text;
        let (b1, b2) = (ids_of_room(&one, "B"), ids_of_room(&two, "B"));
        assert!(b1.len() >= 2, "{name}: the space and at least the relationship that puts it in the storey: {b1:?}");
        assert_eq!(b1, b2, "{name}: room B has the same ids with room A in the file");
    }
}

#[test]
fn every_id_in_a_file_is_different_and_exports_repeat_exactly() {
    let s = scene(vec![room(1, "A", 0.0), room(2, "B", 10.0), room(3, "C", 20.0)]);
    for f in [export as fn(&Scene) -> _, export_ifc4x3, export_ifc2x3] {
        let a = f(&s).unwrap().text;
        let ids = all_ids(&a);
        assert!(ids.len() > 10);
        let unique: BTreeSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "no GlobalId is used twice");
        assert!(!a.contains("@@AUX@@"));
        assert_eq!(a, f(&s).unwrap().text);
    }
}

#[test]
fn two_property_sets_that_are_the_same_in_everything_are_still_told_apart() {
    // two rooms with the same use and occupancy have property sets that read alike
    let a = export(&scene(vec![room(1, "A", 0.0), room(2, "B", 0.0)])).unwrap().text;
    let ids = all_ids(&a);
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}
