//! Grid lines leave as one IfcGrid whose axes are the document's lines (gap B8, grids).
use openbim_arch::{Level, Levels};
use openbim_ifc::{export, export_ifc2x3, export_ifc4x3, GridItem, Scene};

fn grid(id: u64, name: &str, start: [f64; 2], end: [f64; 2]) -> GridItem {
    GridItem { id, name: name.into(), start, end }
}

fn scene(grids: Vec<GridItem>) -> Scene {
    let mut s = empty();
    s.grids = grids;
    s
}

fn empty() -> Scene {
    Scene {
        project_name: "P".into(), project_key: 1, timestamp: "2026-10-23T00:00:00".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], roofs: vec![], railings: vec![],
        stairs: vec![], columns: vec![], beams: vec![], materials: Default::default(), shafts: vec![], floor_penetrations: vec![],
        slab_voids: vec![], networks: vec![], families: vec![], items: vec![], rooms: vec![], grids: vec![],
        room_lighting: Default::default(), wall_types: vec![], types: vec![], member_info: Default::default(), native: vec![],
        wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], pset_map: Default::default(),
    }
}

fn count(text: &str, entity: &str) -> usize {
    text.lines().filter(|l| l.contains(&format!("={entity}("))).count()
}

#[test]
fn a_model_without_grids_has_no_grid() {
    let e = export(&scene(vec![])).unwrap();
    assert_eq!(count(&e.text, "IFCGRID"), 0);
    assert_eq!(e.report.grid_axes, 0);
}

#[test]
fn grid_lines_become_one_grid_with_a_named_axis_each() {
    let g = vec![grid(1, "A", [0.0, 0.0], [0.0, 12.0]), grid(2, "B", [6.0, 0.0], [6.0, 12.0]), grid(3, "1", [-1.0, 0.0], [8.0, 0.0])];
    for (name, e) in [("4", export(&scene(g.clone()))), ("4x3", export_ifc4x3(&scene(g.clone()))), ("2x3", export_ifc2x3(&scene(g.clone())))] {
        let e = e.unwrap();
        assert_eq!(count(&e.text, "IFCGRID"), 1, "{name}");
        assert_eq!(count(&e.text, "IFCGRIDAXIS"), 3, "{name}");
        assert_eq!(e.report.grid_axes, 3, "{name}");
        for tag in ["'A'", "'B'", "'1'"] {
            assert!(e.text.contains(&format!("IFCGRIDAXIS({tag},")), "{name}: axis {tag}");
        }
    }
}

#[test]
fn vertical_lines_are_u_axes_and_horizontal_lines_are_v_axes() {
    let e = export(&scene(vec![grid(1, "A", [0.0, 0.0], [0.0, 12.0]), grid(3, "1", [-1.0, 0.0], [8.0, 0.0])])).unwrap();
    let id_of = |tag: &str| e.text.lines().find(|l| l.contains(&format!("IFCGRIDAXIS('{tag}'"))).unwrap().split('=').next().unwrap().to_string();
    let grid_line = e.text.lines().find(|l| l.contains("=IFCGRID(")).unwrap();
    assert!(grid_line.ends_with(&format!(",({}),({}),$);", id_of("A"), id_of("1"))), "U holds A, V holds 1: {grid_line}");
}

#[test]
fn one_direction_only_is_written_as_u_axes_with_no_v_list() {
    let e = export(&scene(vec![grid(1, "1", [0.0, 0.0], [8.0, 0.0]), grid(2, "2", [0.0, 5.0], [8.0, 5.0])])).unwrap();
    let line = e.text.lines().find(|l| l.contains("=IFCGRID(")).unwrap();
    assert!(line.ends_with(",$,$);") || line.ends_with(",$,$)"), "V and W lists are empty: {line}");
}

#[test]
fn the_grid_belongs_to_the_lowest_storey_and_exports_repeatably() {
    let g = vec![grid(1, "A", [0.0, 0.0], [0.0, 12.0])];
    let (a, b) = (export(&scene(g.clone())).unwrap(), export(&scene(g)).unwrap());
    assert_eq!(a.text, b.text);
    let grid_id = a.text.lines().find(|l| l.contains("=IFCGRID(")).unwrap().split('=').next().unwrap().to_string();
    assert!(a.text.lines().any(|l| l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE") && l.contains(&grid_id)), "{grid_id} is contained in a storey");
}
