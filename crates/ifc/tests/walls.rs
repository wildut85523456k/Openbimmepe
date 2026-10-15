use openbim_arch::{Level, Levels};
use openbim_geom::WallChain;
use openbim_ifc::walls::*;
use openbim_ifc::*;

fn chain(pts: &[[f64; 2]], t: f64) -> WallChain {
    WallChain::new(pts.to_vec(), false, t, 3.0)
}
fn layer(m: &str, t: f64) -> Layer {
    Layer::new(m, t)
}
fn scene(walls: Vec<(WallChain, Option<usize>)>, types: Vec<WallType>, joins: Vec<WallJoin>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: walls.into_iter().enumerate().map(|(i, (c, t))| WallItem { id: 10 + i as u64, name: format!("W{i}"), chain: c, wall_type: t, room_bounding: true }).collect(),
        wall_penetrations: vec![],
        firestops: Default::default(),
        slabs: vec![],
        columns: vec![],
        shafts: vec![],
        floor_penetrations: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        wall_types: types,
        wall_joins: joins,
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        pset_map: Default::default(),
    }
}
fn brick() -> WallType {
    WallType { name: "Cavity".into(), layers: vec![layer("Brick", 0.1), layer("Air", 0.05), layer("Block", 0.05)], core: None }
}

#[test]
fn joins_are_found_for_ends_that_meet_and_ends_that_land_on_a_side() {
    let a = chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2);
    let b = chain(&[[4.0, 0.0], [4.0, 3.0]], 0.2); // an L: a's end is b's start
    let c = chain(&[[2.0, 0.1], [2.0, 5.0]], 0.2); // its start is on a's side
    let far = chain(&[[20.0, 0.0], [24.0, 0.0]], 0.2);
    let j = find_joins(&[&a, &b, &c, &far], 0.01);
    assert!(j.contains(&WallJoin { a: 0, a_end: JoinEnd::End, b: 1, b_end: JoinEnd::Start }), "{j:?}");
    assert!(j.contains(&WallJoin { a: 2, a_end: JoinEnd::Start, b: 0, b_end: JoinEnd::Path }), "{j:?}");
    assert_eq!(j.len(), 2, "{j:?}");
    // an end-to-end pair is written once
    assert_eq!(j.iter().filter(|x| (x.a, x.b) == (1, 0) || (x.a, x.b) == (0, 1)).count(), 1);
}

#[test]
fn walls_on_other_storeys_a_closed_wall_and_a_wall_itself_are_not_joined() {
    let a = chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2);
    let mut up = chain(&[[4.0, 0.0], [4.0, 3.0]], 0.2);
    up.base_z = 3.0;
    assert!(find_joins(&[&a, &up], 0.01).is_empty());
    // a wall whose end touches its own start is not joined to itself
    let ring = chain(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 0.0]], 0.2);
    assert!(find_joins(&[&ring], 0.01).is_empty());
    // a wall whose end lands on its own side is not joined to itself either
    let hook = chain(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 0.0]], 0.2);
    assert!(find_joins(&[&hook], 0.01).is_empty());
    // a closed wall has no ends, even where its first point lies on another wall
    let room0 = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]], true, 0.2, 3.0);
    let along = chain(&[[-3.0, 0.0], [1.0, 0.0]], 0.2);
    let j0 = find_joins(&[&room0, &along], 0.01);
    assert!(!j0.is_empty() && j0.iter().all(|x| x.a == 1 && x.b == 0), "{j0:?}");
    // a closed wall has no ends; an open wall landing on it is joined to its path
    let room = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]], true, 0.2, 3.0);
    let t = chain(&[[2.0, 0.1], [2.0, 3.9]], 0.1);
    let j = find_joins(&[&room, &t], 0.01);
    assert_eq!(j.len(), 2);
    assert!(j.iter().all(|x| x.a == 1 && x.b == 0 && x.b_end == JoinEnd::Path));
    // a wall that only passes near the side but beyond half the thickness is not joined
    let near = chain(&[[2.0, 0.5], [2.0, 3.0]], 0.2);
    assert!(find_joins(&[&room, &near], 0.01).is_empty());
}

#[test]
fn a_typed_wall_is_one_element_with_its_type_a_layer_set_and_a_usage() {
    let w = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], false, 0.2, 3.0); // two segments, one wall
    let out = export(&scene(vec![(w, Some(0))], vec![brick()], vec![])).unwrap();
    assert_eq!(out.text.matches("IFCWALL(").count(), 1);
    assert_eq!(out.text.matches("IFCWALLTYPE(").count(), 1);
    assert_eq!(out.text.matches("IFCMATERIALLAYERSET((").count(), 1);
    assert_eq!(out.text.matches("IFCMATERIALLAYER(").count(), 3);
    assert_eq!(out.text.matches("IFCMATERIALLAYERSETUSAGE(").count(), 1);
    assert!(out.text.contains(",.AXIS2.,.POSITIVE.,-0.1,$)"));
    assert_eq!(out.text.matches("IFCRELDEFINESBYTYPE(").count(), 1);
    assert_eq!(out.text.matches("IFCRELASSOCIATESMATERIAL(").count(), 2, "the type and the wall");
    assert_eq!((out.report.wall_types, out.report.walls), (1, 1));
    // the same input, the same file
    let w2 = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], false, 0.2, 3.0);
    assert_eq!(out.text, export(&scene(vec![(w2, Some(0))], vec![brick()], vec![])).unwrap().text);
}

#[test]
fn materials_are_shared_and_walls_of_one_type_share_it() {
    let mut t2 = brick();
    t2.name = "Cavity 2".into();
    let walls = vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2), Some(0)), (chain(&[[0.0, 5.0], [4.0, 5.0]], 0.2), Some(0)), (chain(&[[0.0, 9.0], [4.0, 9.0]], 0.2), Some(1)), (chain(&[[0.0, 12.0], [4.0, 12.0]], 0.2), None)];
    let out = export(&scene(walls, vec![brick(), t2], vec![])).unwrap();
    assert_eq!(out.text.matches("IFCMATERIAL('").count(), 3, "Brick, Air, Block once each");
    assert_eq!(out.text.matches("IFCWALLTYPE(").count(), 2);
    assert_eq!(out.text.matches("IFCMATERIALLAYERSETUSAGE(").count(), 3);
    assert_eq!(out.text.matches("IFCRELDEFINESBYTYPE(").count(), 2, "one per type, listing its walls");
    assert!(out.text.contains("IFCRELDEFINESBYTYPE(") && out.text.matches("IFCWALL(").count() == 4);
}

#[test]
fn a_type_that_does_not_fit_the_wall_is_refused() {
    let e = |t: WallType, thick: f64| export(&scene(vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], thick), Some(0))], vec![t], vec![])).err().map(|e| format!("{e}"));
    assert!(e(brick(), 0.25).unwrap().contains("add up to"));
    assert!(e(WallType { name: "x".into(), layers: vec![], core: None }, 0.2).unwrap().contains("at least one layer"));
    assert!(e(WallType { name: "x".into(), layers: vec![layer("A", 0.0), layer("B", 0.2)], core: None }, 0.2).unwrap().contains("above 0"));
    assert!(e(WallType { name: "x".into(), layers: vec![layer(" ", 0.2)], core: None }, 0.2).unwrap().contains("no material"));
    assert!(e(WallType { name: "x".into(), layers: (0..65).map(|_| layer("A", 0.01)).collect(), core: None }, 0.65).unwrap().contains("64"));
    // a wall naming a type that does not exist
    let r = export(&scene(vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2), Some(3))], vec![brick()], vec![]));
    assert!(format!("{}", r.err().unwrap()).contains("no wall type"));
    assert!(e(brick(), 0.2).is_none());
}

#[test]
fn joins_are_written_between_two_elements_with_their_connection_types() {
    let walls = vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2), None), (chain(&[[4.0, 0.0], [4.0, 3.0]], 0.2), None), (chain(&[[2.0, 0.1], [2.0, 5.0]], 0.2), None)];
    let refs: Vec<&WallChain> = walls.iter().map(|w| &w.0).collect();
    let joins = find_joins(&refs, 0.01);
    assert_eq!(joins.len(), 2);
    let out = export(&scene(walls, vec![], joins)).unwrap();
    assert_eq!(out.report.wall_joins, 2);
    assert_eq!(out.text.matches("IFCRELCONNECTSPATHELEMENTS(").count(), 2);
    // the L: a's end meets b's start; the T: c's start meets a's path (related is the wall that is met)
    assert!(out.text.contains(",(),(),.ATSTART.,.ATEND.);"), "{}", out.text.lines().filter(|l| l.contains("PATHELEM")).collect::<Vec<_>>().join("\n"));
    assert!(out.text.contains(",(),(),.ATPATH.,.ATSTART.);"));
}

#[test]
fn bad_joins_are_refused() {
    let w = || vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2), None), (chain(&[[4.0, 0.0], [4.0, 3.0]], 0.2), None)];
    let j = |a, b| WallJoin { a, a_end: JoinEnd::End, b, b_end: JoinEnd::Start };
    assert!(export(&scene(w(), vec![], vec![j(0, 5)])).is_err());
    assert!(export(&scene(w(), vec![], vec![j(1, 1)])).is_err());
    assert!(export(&scene(w(), vec![], vec![j(0, 1)])).is_ok());
}

fn revit_type() -> WallType {
    // exterior to interior: brick facing 0.1 (finish 2), cavity 0.05 (thermal), block core 0.2 (structure), plaster 0.015 (finish 1)
    WallType {
        name: "Cavity".into(),
        layers: vec![Layer::with("Brick", 0.1, Function::Finish2), Layer::with("Cavity", 0.05, Function::ThermalAir), Layer::with("Block", 0.2, Function::Structure), Layer::with("Plaster", 0.015, Function::Finish1)],
        core: Some((2, 2)),
    }
}
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn the_layer_functions_have_revits_priorities() {
    let all = [Function::Structure, Function::Substrate, Function::ThermalAir, Function::Membrane, Function::Finish1, Function::Finish2];
    assert_eq!(all.map(|f| f.priority()), [1, 2, 3, 4, 5, 6]);
    assert_eq!(all.map(|f| f.name()), ["Structure", "Substrate", "Thermal/Air Layer", "Membrane Layer", "Finish 1", "Finish 2"]);
    assert!(all.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn the_core_is_between_the_boundaries_and_needs_a_structure_layer() {
    let t = revit_type();
    assert!(t.check().is_ok());
    assert!(near(t.thickness(), 0.365));
    assert_eq!(t.core_range(), (2, 2));
    // without boundaries the whole wall is the core
    let whole = WallType { core: None, ..t.clone() };
    assert_eq!(whole.core_range(), (0, 3));
    assert!(whole.check().is_ok());
    let e = |t: WallType| t.check().unwrap_err();
    assert!(e(WallType { core: Some((3, 3)), ..t.clone() }).contains("structure layer"));
    assert!(e(WallType { core: Some((2, 4)), ..t.clone() }).contains("core boundaries"));
    assert!(e(WallType { core: Some((3, 2)), ..t.clone() }).contains("core boundaries"));
    let mut m = t.clone();
    m.layers[1].function = Function::Membrane;
    assert!(e(WallType { core: Some((1, 2)), ..m.clone() }).contains("membrane"));
    // a membrane has no thickness, but nothing else may lack it
    m.layers[1].thickness = 0.0;
    assert!(m.check().is_ok());
    let mut z = t.clone();
    z.layers[1].thickness = 0.0;
    assert!(e(z).contains("above 0"));
    let mut n = t.clone();
    n.layers[1].thickness = -0.01;
    assert!(e(n).contains("above 0"));
    let only_membrane = WallType { name: "m".into(), layers: vec![Layer::with("M", 0.0, Function::Membrane)], core: None };
    assert!(only_membrane.check().is_err());
}

#[test]
fn each_location_line_puts_the_drawn_line_where_revit_does() {
    let t = revit_type();
    // positions from the exterior (left) face: centreline 0.1825, core centreline 0.25, core faces 0.15 and 0.35, finish faces 0 and 0.365
    let want = [
        (LocationLine::WallCentreline, 0.0),
        (LocationLine::CoreCentreline, 0.25 - 0.1825),
        (LocationLine::FinishFaceExterior, -0.1825),
        (LocationLine::FinishFaceInterior, 0.1825),
        (LocationLine::CoreFaceExterior, 0.15 - 0.1825),
        (LocationLine::CoreFaceInterior, 0.35 - 0.1825),
    ];
    for (loc, shift) in want {
        assert!(near(t.centreline_shift(loc), shift), "{loc:?} {} {shift}", t.centreline_shift(loc));
    }
    // a symmetrical wall has the same centre line and core centre line
    let sym = WallType { name: "s".into(), layers: vec![Layer::with("A", 0.05, Function::Finish1), Layer::new("B", 0.1), Layer::with("C", 0.05, Function::Finish1)], core: Some((1, 1)) };
    assert!(near(sym.centreline_shift(LocationLine::CoreCentreline), 0.0));
    assert!(near(sym.centreline_shift(LocationLine::CoreFaceExterior), -0.05));
}

#[test]
fn a_path_is_offset_to_the_left_with_mitred_corners() {
    // east along y = 0: left is north
    let p = offset_path(&[[0.0, 0.0], [4.0, 0.0]], false, 0.5).unwrap();
    assert_eq!(p, vec![[0.0, 0.5], [4.0, 0.5]]);
    assert_eq!(offset_path(&[[0.0, 0.0], [4.0, 0.0]], false, -0.5).unwrap(), vec![[0.0, -0.5], [4.0, -0.5]]);
    // a corner turning left (east then north): the offset lines meet inside the turn
    let c = offset_path(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], false, 0.5).unwrap();
    assert!(near(c[1][0], 3.5) && near(c[1][1], 0.5), "{c:?}");
    // turning right: they meet outside
    let c = offset_path(&[[0.0, 0.0], [4.0, 0.0], [4.0, -3.0]], false, 0.5).unwrap();
    assert!(near(c[1][0], 4.5) && near(c[1][1], 0.5), "{c:?}");
    // a closed clockwise rectangle moved left (outwards) grows by the offset on every side
    let r = offset_path(&[[0.0, 0.0], [0.0, 3.0], [4.0, 3.0], [4.0, 0.0]], true, 0.5).unwrap();
    assert_eq!(r.len(), 4);
    for (got, want) in r.iter().zip([[-0.5, -0.5], [-0.5, 3.5], [4.5, 3.5], [4.5, -0.5]]) {
        assert!(near(got[0], want[0]) && near(got[1], want[1]), "{r:?}");
    }
    // a straight continuation is just moved
    let s = offset_path(&[[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]], false, 0.25).unwrap();
    assert_eq!(s, vec![[0.0, 0.25], [2.0, 0.25], [5.0, 0.25]]);
    // an angled corner keeps the distance to both segments equal to the offset
    let a = offset_path(&[[0.0, 0.0], [4.0, 0.0], [6.0, 3.0]], false, 0.4).unwrap();
    let (u, v) = ([1.0, 0.0], [2.0 / 13f64.sqrt(), 3.0 / 13f64.sqrt()]);
    let dist = |d: [f64; 2], p0: [f64; 2], q: [f64; 2]| ((q[0] - p0[0]) * -d[1] + (q[1] - p0[1]) * d[0]).abs();
    assert!(near(dist(u, [0.0, 0.0], a[1]), 0.4) && near(dist(v, [4.0, 0.0], a[1]), 0.4), "{a:?}");
}

#[test]
fn bad_paths_are_refused() {
    assert!(offset_path(&[[0.0, 0.0]], false, 0.1).is_err());
    assert!(offset_path(&[[0.0, 0.0], [1.0, 0.0]], true, 0.1).is_err());
    assert!(offset_path(&[[0.0, 0.0], [0.0, 0.0], [1.0, 0.0]], false, 0.1).unwrap_err().contains("no length"));
    assert!(offset_path(&[[0.0, 0.0], [2.0, 0.0], [0.0, 0.0]], false, 0.1).unwrap_err().contains("doubles back"));
    assert!(offset_path(&[[0.0, 0.0], [4.0, 0.0], [0.0, 0.01]], false, 0.1).unwrap_err().contains("too sharp"));
    assert!(offset_path(&[[0.0, 0.0], [1.0, f64::NAN]], false, 0.1).is_err());
    assert!(offset_path(&[[0.0, 0.0], [1.0, 0.0]], false, f64::INFINITY).is_err());
}

#[test]
fn a_wall_drawn_on_a_location_line_has_its_centre_where_the_type_says() {
    let t = revit_type();
    let w = wall_on_location_line(&[[0.0, 0.0], [5.0, 0.0]], false, &t, LocationLine::FinishFaceExterior, 3.0).unwrap();
    assert!(near(w.thickness, 0.365) && near(w.height, 3.0));
    // drawn along the exterior face, which is on the left (north): the centre is 0.1825 to the right (south)
    assert!(near(w.points[0][1], -0.1825) && near(w.points[1][1], -0.1825));
    let plans = w.plans().unwrap();
    // the body spans y from -0.365 to 0, so its left face is at the drawn line
    assert!(near(plans[0].l0[1], 0.0) && near(plans[0].r0[1], -0.365), "{:?} {:?}", plans[0].l0, plans[0].r0);
    let bad = WallType { core: Some((0, 0)), ..t };
    assert!(wall_on_location_line(&[[0.0, 0.0], [5.0, 0.0]], false, &bad, LocationLine::WallCentreline, 3.0).is_err());
}

#[test]
fn the_export_lists_the_layers_from_the_right_and_says_what_each_is() {
    let t = revit_type();
    let mut s = scene(vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], 0.365), Some(0))], vec![t], vec![]);
    s.georef = None;
    let out = export(&s).unwrap();
    let layers: Vec<&str> = out.text.lines().filter(|l| l.contains("IFCMATERIALLAYER(")).collect();
    assert_eq!(layers.len(), 4);
    // plaster (interior, on the right) first, brick (exterior, on the left) last
    assert!(layers[0].contains("'Plaster'") && layers[0].contains("'Finish 1',5);"), "{}", layers[0]);
    assert!(layers[1].contains("'Block'") && layers[1].contains("'Core','Structure',1);"), "{}", layers[1]);
    assert!(layers[2].contains("'Cavity'") && layers[2].contains("$,'Thermal/Air Layer',3);"), "{}", layers[2]);
    assert!(layers[3].contains("'Brick'") && layers[3].contains("$,'Finish 2',6);"), "{}", layers[3]);
    assert!(out.text.contains("IFCPROPERTYSINGLEVALUE('CoreFirstLayer',$,IFCINTEGER(3),$)") && out.text.contains("IFCPROPERTYSINGLEVALUE('CoreLastLayer',$,IFCINTEGER(3),$)"));
    assert!(out.text.contains("IFCWALLTYPE(") && out.text.lines().any(|l| l.contains("IFCWALLTYPE(") && l.contains(",(#")));
}

#[test]
fn a_wall_end_that_may_not_join_joins_nothing() {
    let a = chain(&[[0.0, 0.0], [4.0, 0.0]], 0.2);
    let b = chain(&[[4.0, 0.0], [4.0, 3.0]], 0.2); // an L with a
    let c = chain(&[[2.0, 0.1], [2.0, 5.0]], 0.2); // a T on a's side
    let all = find_joins(&[&a, &b, &c], 0.01);
    assert_eq!(all.len(), 2);
    // a's end may not join: the L goes, the T stays
    let j = find_joins_except(&[&a, &b, &c], 0.01, &[(0, JoinEnd::End)]);
    assert_eq!(j, vec![WallJoin { a: 2, a_end: JoinEnd::Start, b: 0, b_end: JoinEnd::Path }]);
    // b's start may not join: the same
    let j = find_joins_except(&[&a, &b, &c], 0.01, &[(1, JoinEnd::Start)]);
    assert_eq!(j, vec![WallJoin { a: 2, a_end: JoinEnd::Start, b: 0, b_end: JoinEnd::Path }]);
    // c's start may not join: only the L stays
    let j = find_joins_except(&[&a, &b, &c], 0.01, &[(2, JoinEnd::Start)]);
    assert_eq!(j, vec![WallJoin { a: 0, a_end: JoinEnd::End, b: 1, b_end: JoinEnd::Start }]);
    // the other end of a wall is not affected, and an empty list changes nothing
    assert_eq!(find_joins_except(&[&a, &b, &c], 0.01, &[(0, JoinEnd::Start)]), all);
    assert_eq!(find_joins_except(&[&a, &b, &c], 0.01, &[]), all);
}

#[test]
fn a_join_between_typed_walls_carries_the_layer_priorities() {
    let t = revit_type(); // from the exterior: finish 2, thermal, structure, finish 1 -> in the file from the right: 5, 1, 3, 6
    let s = scene(
        vec![(chain(&[[0.0, 0.0], [4.0, 0.0]], 0.365), Some(0)), (chain(&[[4.0, 0.0], [4.0, 3.0]], 0.365), Some(0)), (chain(&[[9.0, 0.0], [9.0, 3.0]], 0.365), None), (chain(&[[9.0, 3.0], [12.0, 3.0]], 0.365), Some(0))],
        vec![t],
        vec![WallJoin { a: 0, a_end: JoinEnd::End, b: 1, b_end: JoinEnd::Start }, WallJoin { a: 2, a_end: JoinEnd::End, b: 3, b_end: JoinEnd::Start }],
    );
    let out = export(&s).unwrap();
    let rels: Vec<&str> = out.text.lines().filter(|l| l.contains("IFCRELCONNECTSPATHELEMENTS(")).collect();
    assert_eq!(rels.len(), 2);
    assert!(rels[0].contains(",(5,1,3,6),(5,1,3,6),"), "{}", rels[0]);
    // one of the two has no layers: no priorities
    assert!(rels[1].contains(",(),(),"), "{}", rels[1]);
}
