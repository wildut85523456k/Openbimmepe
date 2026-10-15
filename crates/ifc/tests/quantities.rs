use openbim_arch::Room;
use openbim_family::shared::SharedSet;
use openbim_geom::{WallChain, Opening};
use openbim_ifc::pset_map::{MapError, MapRow, PropKind, PsetMap};
use openbim_ifc::quantities::{self, Kind, Qty};
use openbim_place::floors::Slab;

fn get(q: &[Qty], n: &str) -> f64 {
    q.iter().find(|x| x.name == n).unwrap_or_else(|| panic!("no {n}")).value
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

#[test]
fn wall_quantities_by_hand() {
    // 4 m long, 0.2 thick, 3 high, one 0.9 x 2.1 opening
    let mut w = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0);
    w.openings = vec![vec![Opening { offset: 1.0, width: 0.9, sill: 0.0, height: 2.1 }]];
    let q = quantities::wall(&w).unwrap();
    near(get(&q, "Length"), 4.0);
    near(get(&q, "Width"), 0.2);
    near(get(&q, "Height"), 3.0);
    near(get(&q, "GrossFootprintArea"), 0.8);
    near(get(&q, "GrossSideArea"), 12.0);
    near(get(&q, "NetSideArea"), 12.0 - 0.9 * 2.1);
    near(get(&q, "GrossVolume"), 2.4);
    near(get(&q, "NetVolume"), 2.4 - 0.9 * 2.1 * 0.2);
    assert_eq!(q.iter().find(|x| x.name == "NetVolume").unwrap().kind, Kind::Volume);
    assert_eq!(q.iter().find(|x| x.name == "Length").unwrap().kind, Kind::Length);
}

#[test]
fn slab_and_opening_and_space_quantities_by_hand() {
    let s = Slab { name: "S".into(), top: 3.0, thickness: 0.25, outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]], fire_rating_min: None };
    let q = quantities::slab(&s, 2.25);
    near(get(&q, "Perimeter"), 36.0);
    near(get(&q, "GrossArea"), 80.0);
    near(get(&q, "NetArea"), 77.75);
    near(get(&q, "NetVolume"), 77.75 * 0.25);
    let d = quantities::opening_item(0.9, 2.1);
    near(get(&d, "Area"), 1.89);
    let r = Room { name: "R".into(), usage: "office".into(), occupancy: 2.0, level: 0, outline: vec![[0.0, 0.0], [6.0, 0.0], [6.0, 5.0], [0.0, 5.0]], holes: vec![vec![[1.0, 1.0], [1.0, 2.0], [2.0, 2.0], [2.0, 1.0]]], height: 2.5 };
    let q = quantities::space(&r);
    near(get(&q, "GrossFloorArea"), 30.0);
    near(get(&q, "NetFloorArea"), 29.0);
    near(get(&q, "GrossPerimeter"), 22.0);
    near(get(&q, "NetPerimeter"), 26.0);
    near(get(&q, "NetVolume"), 72.5);
    near(get(&q, "GrossVolume"), 75.0);
}

#[test]
fn overlap_area_cases() {
    let sq = |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    let big = sq(0.0, 0.0, 10.0, 8.0);
    near(quantities::overlap_area(&big, &sq(1.0, 1.0, 3.0, 2.0)).unwrap(), 2.0); // inside
    near(quantities::overlap_area(&big, &sq(9.0, 7.0, 12.0, 10.0)).unwrap(), 1.0); // straddling a corner
    near(quantities::overlap_area(&big, &sq(20.0, 20.0, 21.0, 21.0)).unwrap(), 0.0); // outside
    near(quantities::overlap_area(&sq(1.0, 1.0, 3.0, 2.0), &big).unwrap(), 2.0); // order does not matter
    let cw: Vec<[f64; 2]> = sq(1.0, 1.0, 3.0, 2.0).into_iter().rev().collect();
    near(quantities::overlap_area(&big, &cw).unwrap(), 2.0); // clockwise clip
    // an L and a concave star: neither convex
    let l = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 4.0], [0.0, 4.0]];
    assert!(quantities::overlap_area(&l, &l).is_none());
    near(quantities::overlap_area(&l, &sq(0.0, 0.0, 2.0, 2.0)).unwrap(), 3.0); // L meets a 2 x 2 box: 2 + 1
}

#[test]
fn map_text_round_trip_and_lookup() {
    let text = "# c\n\nIfcDoor\tmark\tPset_DoorCommon\tReference\tidentifier\n*\tmark\tPset_Any\tMark\ttext\n";
    let m = PsetMap::from_text(text).unwrap();
    assert_eq!(m.rows.len(), 2);
    assert_eq!(m.find("IfcDoor", "mark").unwrap().pset, "Pset_DoorCommon"); // the class row wins
    assert_eq!(m.find("ifcdoor", "mark").unwrap().pset, "Pset_DoorCommon");
    assert_eq!(m.find("IfcWindow", "mark").unwrap().pset, "Pset_Any"); // the * row
    assert!(m.find("IfcWindow", "other").is_none());
    assert_eq!(PsetMap::from_text(&m.to_text()).unwrap(), m);
    for k in PropKind::ALL {
        assert_eq!(PropKind::from_code(k.code()), Some(k));
    }
}

#[test]
fn map_errors() {
    assert!(matches!(PsetMap::from_text("IfcDoor\ta\tP\tQ\n"), Err(MapError::Bad(1, _))));
    assert!(matches!(PsetMap::from_text("\nIfcDoor\ta\tP\tQ\tnope\n"), Err(MapError::Bad(2, _))));
    let row = |c: &str, s: &str, p: &str, q: &str| MapRow { class: c.into(), source: s.into(), pset: p.into(), property: q.into(), kind: PropKind::Real };
    assert!(PsetMap { rows: vec![row("Door", "a", "P", "Q")] }.check().is_err()); // not an IFC class
    assert!(PsetMap { rows: vec![row("IfcDoor", "a", "P", "Q"), row("ifcdoor", "a", "P", "R")] }.check().is_err()); // same parameter twice
    assert!(PsetMap { rows: vec![row("IfcDoor", "a", "P", "Q"), row("IfcDoor", "b", "P", "Q")] }.check().is_err()); // same target
    assert!(PsetMap { rows: vec![row("*", "a", "P'x", "Q")] }.check().is_err()); // a quote
    assert!(PsetMap { rows: vec![row("*", "a", "", "Q")] }.check().is_err());
    assert!(PsetMap { rows: vec![row("*", "a", "P", "Q")] }.check().is_ok());
}

#[test]
fn map_from_shared_parameters_and_overlay() {
    let id = |n: u128| format!("{n:032x}");
    let text = format!(
        "{}\tfire_load\tnumber\ttype\t1\t0\t9\t\tPset_Fire\tload\n{}\tzones\tcount\tinstance\t1\t0\t9\t\tPset_Fire\tn\n{}\tnote\ttext\ttype\tx\t\t\t\t\tfree\n{}\tflag\tflag\ttype\t1\t\t\t\tPset_Fire\tf\n{}\tslope\tangle\ttype\t5\t0\t90\t\tPset_Fire\ta\n{}\tarea_x\tarea\ttype\t1\t0\t9\t\tPset_Fire\ta\n{}\tvol_x\tvolume\ttype\t1\t0\t9\t\tPset_Fire\ta\n{}\tratio_x\tratio\ttype\t1\t0\t9\t\tPset_Fire\ta\n",
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        id(7),
        id(8)
    );
    let set = SharedSet::from_text(&text).unwrap();
    let m = PsetMap::from_shared(&set);
    assert_eq!(m.rows.len(), 7, "a parameter without a group has no row");
    for (n, k) in [("slope", PropKind::Angle), ("area_x", PropKind::Area), ("vol_x", PropKind::Volume), ("ratio_x", PropKind::Real)] {
        assert_eq!(m.find("IfcWall", n).unwrap().kind, k, "{n}");
    }
    assert_eq!(m.find("IfcDoor", "fire_load").unwrap().kind, PropKind::Length); // number is the older word for length
    assert_eq!(m.find("IfcDoor", "zones").unwrap().kind, PropKind::Integer);
    assert_eq!(m.find("IfcDoor", "flag").unwrap().kind, PropKind::Boolean);
    assert!(m.find("IfcDoor", "note").is_none());
    m.check().unwrap();
    // an explicit row replaces the one made from the group
    let over = PsetMap::from_text("IfcDoor\tfire_load\tPset_DoorCommon\tFireLoad\tarea\n").unwrap();
    let both = m.overlaid(&over);
    assert_eq!(both.find("IfcDoor", "fire_load").unwrap().property, "FireLoad");
    assert_eq!(both.find("IfcWall", "fire_load").unwrap().property, "fire_load");
    assert_eq!(both.rows.len(), 8);
}
