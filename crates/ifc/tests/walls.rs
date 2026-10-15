use openbim_arch::{Level, Levels};
use openbim_geom::WallChain;
use openbim_ifc::walls::*;
use openbim_ifc::*;

fn chain(pts: &[[f64; 2]], t: f64) -> WallChain {
    WallChain::new(pts.to_vec(), false, t, 3.0)
}
fn layer(m: &str, t: f64) -> Layer {
    Layer { material: m.into(), thickness: t }
}
fn scene(walls: Vec<(WallChain, Option<usize>)>, types: Vec<WallType>, joins: Vec<WallJoin>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: walls.into_iter().enumerate().map(|(i, (c, t))| WallItem { id: 10 + i as u64, name: format!("W{i}"), chain: c, wall_type: t }).collect(),
        wall_penetrations: vec![],
        slabs: vec![],
        shafts: vec![],
        floor_penetrations: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        wall_types: types,
        wall_joins: joins,
        pset_map: Default::default(),
    }
}
fn brick() -> WallType {
    WallType { name: "Cavity".into(), layers: vec![layer("Brick", 0.1), layer("Air", 0.05), layer("Block", 0.05)] }
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
    assert!(e(WallType { name: "x".into(), layers: vec![] }, 0.2).unwrap().contains("at least one layer"));
    assert!(e(WallType { name: "x".into(), layers: vec![layer("A", 0.0), layer("B", 0.2)] }, 0.2).unwrap().contains("above 0"));
    assert!(e(WallType { name: "x".into(), layers: vec![layer(" ", 0.2)] }, 0.2).unwrap().contains("no material"));
    assert!(e(WallType { name: "x".into(), layers: (0..65).map(|_| layer("A", 0.01)).collect() }, 0.65).unwrap().contains("64"));
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
