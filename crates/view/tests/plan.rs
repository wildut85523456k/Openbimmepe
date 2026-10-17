use openbim_arch::{Level, Levels};
use openbim_family::library;
use openbim_family::Request;
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{FamilyItem, FamilyRef, Frame, Scene, WallItem};
use openbim_view::*;

fn scene(walls: Vec<WallChain>, items: Vec<FamilyItem>, families: Vec<FamilyRef>) -> Scene {
    Scene {
        project_name: "P".into(), project_key: 1, timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }]).unwrap(),
        walls: walls.into_iter().enumerate().map(|(i, chain)| WallItem { id: 100 + i as u64, name: format!("W{i}"), chain, wall_type: None, room_bounding: true }).collect(),
        wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], networks: vec![],
        families, items, rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(),
    }
}

fn range() -> ViewRange { ViewRange::new(2.3, 1.2, 0.0, -1.0).unwrap() }

/// a 4 m wall along x, 0.2 thick, 3 m high
fn straight() -> WallChain {
    let mut c = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0);
    c.openings = vec![vec![]];
    c
}

fn area(poly: &[[f64; 2]]) -> f64 {
    (0..poly.len()).map(|k| { let (p, q) = (poly[k], poly[(k + 1) % poly.len()]); p[0] * q[1] - q[0] * p[1] }).sum::<f64>().abs() / 2.0
}
fn fills(v: &PlanView) -> Vec<Vec<[f64; 2]>> {
    v.items.iter().filter_map(|i| if let Shape::Fill(p) = &i.shape { Some(p.clone()) } else { None }).collect()
}
fn lines(v: &PlanView, c: Class) -> usize { v.items.iter().filter(|i| i.class == c && matches!(i.shape, Shape::Line(..))).count() }
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

#[test]
fn a_door_in_a_wall_is_a_gap_in_the_cut_below_its_head_and_no_gap_above_it() {
    let mut c = straight();
    c.openings[0].push(Opening { offset: 1.5, width: 1.0, sill: 0.0, height: 2.1 });
    let s = scene(vec![c], vec![], vec![]);
    // the cut at 1.2 m passes through the door: two pieces of 1.5 m x 0.2 m, each with four edges
    let v = plan_view(&s, 0, range()).unwrap();
    let f = fills(&v);
    assert_eq!(f.len(), 2);
    assert!(f.iter().all(|p| close(area(p), 0.3)));
    assert_eq!(lines(&v, Class::Cut), 8);
    assert!(v.items.iter().all(|i| i.class == Class::Cut && i.element == Element { kind: Kind::Wall, id: 100 }));
    // the cut at 2.2 m is above the head of the door: one piece, 4 m x 0.2 m
    let high = plan_view(&s, 0, ViewRange::new(2.5, 2.2, 0.0, -1.0).unwrap()).unwrap();
    let f = fills(&high);
    assert_eq!((f.len(), lines(&high, Class::Cut)), (1, 4));
    assert!(close(area(&f[0]), 0.8));
}

#[test]
fn a_window_is_a_gap_only_while_the_cut_is_between_its_sill_and_head() {
    let mut c = straight();
    c.openings[0].push(Opening { offset: 1.5, width: 1.0, sill: 0.9, height: 1.2 });
    let s = scene(vec![c], vec![], vec![]);
    let at = |cut: f64| fills(&plan_view(&s, 0, ViewRange::new(2.5, cut, 0.0, -1.0).unwrap()).unwrap()).len();
    assert_eq!((at(0.5), at(0.9), at(1.2), at(2.0), at(2.1), at(2.4)), (1, 2, 2, 2, 1, 1));
}

#[test]
fn a_closed_wall_chain_has_no_lines_across_its_corners() {
    let mut c = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]], true, 0.2, 3.0);
    c.openings = vec![vec![]; 4];
    let s = scene(vec![c], vec![], vec![]);
    let v = plan_view(&s, 0, range()).unwrap();
    // four pieces; only the outer and inner edge of each: 8 lines
    assert_eq!((fills(&v).len(), lines(&v, Class::Cut)), (4, 8));
    // together they fill the ring between the 4.2 x 3.2 outside and the 3.8 x 2.8 inside
    let total: f64 = fills(&v).iter().map(|p| area(p)).sum();
    assert!(close(total, 4.2 * 3.2 - 3.8 * 2.8), "{total}");
}

#[test]
fn walls_below_above_and_beyond_the_cut_are_classed_by_the_range() {
    let wall = |base: f64, h: f64| { let mut c = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, h); c.base_z = base; c.openings = vec![vec![]]; c };
    // range: top 2.3, cut 1.2, bottom 0.0, depth -1.0 above level 0
    let class_of = |base: f64, h: f64| { let v = plan_view(&scene(vec![wall(base, h)], vec![], vec![]), 0, range()).unwrap(); v.items.first().map(|i| i.class) };
    assert_eq!(class_of(0.0, 3.0), Some(Class::Cut));
    assert_eq!(class_of(0.0, 1.0), Some(Class::Below));
    assert_eq!(class_of(1.5, 1.0), Some(Class::Overhead));
    assert_eq!(class_of(-0.8, 0.5), Some(Class::Beyond)); // top at -0.3, below the view bottom 0
    assert_eq!(class_of(-3.0, 1.0), None); // top at -2, deeper than the depth
    assert_eq!(class_of(2.4, 1.0), None); // starts above the view top
    // a wall seen from above, not cut, is its outline: two long edges and two ends
    let v = plan_view(&scene(vec![wall(0.0, 1.0)], vec![], vec![]), 0, range()).unwrap();
    assert_eq!((v.items.len(), lines(&v, Class::Below)), (4, 4));
    // the cut passes exactly through the bottom of a wall: cut; exactly through its top: not cut
    assert_eq!(class_of(1.2, 1.0), Some(Class::Cut));
    assert_eq!(class_of(0.2, 1.0), Some(Class::Below));
}

#[test]
fn a_sloped_top_is_cut_only_where_the_wall_is_higher_than_the_cut() {
    let mut c = straight();
    // the top rises from 0 m at one end to 4 m at the other: at a cut of 1.2 m the wall is there for s > 1.2
    c.tops = vec![Some(vec![[0.0, 0.0], [4.0, 4.0]])];
    c.height = 4.0;
    let s = scene(vec![c], vec![], vec![]);
    let f = fills(&plan_view(&s, 0, ViewRange::new(3.0, 1.2, 0.0, -1.0).unwrap()).unwrap());
    assert_eq!(f.len(), 1);
    assert!(close(area(&f[0]), (4.0 - 1.2) * 0.2), "{}", area(&f[0]));
    let xs: Vec<f64> = f[0].iter().map(|p| p[0]).collect();
    assert!(close(xs.iter().cloned().fold(f64::MAX, f64::min), 1.2) && close(xs.iter().cloned().fold(f64::MIN, f64::max), 4.0));
}

#[test]
fn a_polygonal_opening_is_a_gap_as_wide_as_the_polygon_at_the_cut_height() {
    let mut c = straight();
    // a triangle in (s, z): base from 1 to 3 on the floor, apex at (2, 2): at 1 m high it spans 1.5 to 2.5
    c.poly_openings = vec![vec![vec![[1.0, 0.0], [3.0, 0.0], [2.0, 2.0]]]];
    let s = scene(vec![c], vec![], vec![]);
    let f = fills(&plan_view(&s, 0, ViewRange::new(2.5, 1.0, 0.0, -1.0).unwrap()).unwrap());
    let widths: Vec<f64> = f.iter().map(|p| area(p) / 0.2).collect();
    assert_eq!(widths.len(), 2);
    assert!(close(widths[0], 1.5) && close(widths[1], 1.5), "{widths:?}");
    // above the apex the wall is whole
    assert_eq!(fills(&plan_view(&s, 0, ViewRange::new(2.9, 2.5, 0.0, -1.0).unwrap()).unwrap()).len(), 1);
}

#[test]
fn a_door_symbol_follows_the_wall_it_is_in_and_turns_with_it() {
    let fams = library::all().unwrap();
    let k = fams.iter().position(|c| c.name() == "Door, single flush").unwrap();
    let inst = fams[k].solve(&Request::default()).unwrap();
    // a wall along +y, so the family frame's x axis points along +y and its y axis along -x
    let mut c = WallChain::new(vec![[5.0, 0.0], [5.0, 4.0]], false, 0.2, 3.0);
    c.openings = vec![vec![]];
    let frame = Frame::on_wall(&c, 0, 2.0);
    let item = FamilyItem { id: 7, name: "D".into(), family: 0, type_name: None, frame, instance: inst, host: None };
    let s = scene(vec![c], vec![item], vec![FamilyRef { id: 1, family: fams[k].clone() }]);
    let v = plan_view(&s, 0, range()).unwrap();
    let syms: Vec<&Item> = v.items.iter().filter(|i| i.class == Class::Symbol).collect();
    assert_eq!(syms.len(), 2);
    // the right-hinged leaf of 0.8 m: hinge at family x = 0.4, i.e. (5, 2.4); its free end at family y = 0.8, i.e. (4.2, 2.4)
    let Shape::Line(a, b) = syms[0].shape else { panic!("line") };
    assert!(close(a[0], 5.0) && close(a[1], 2.4) && close(b[0], 4.2) && close(b[1], 2.4), "{a:?} {b:?}");
    // the arc starts at family angle 90 and the frame is turned by 90 degrees
    let Shape::Arc { c, r, a0, a1 } = syms[1].shape else { panic!("arc") };
    assert!(close(c[0], 5.0) && close(c[1], 2.4) && close(r, 0.8) && close(a0, 180.0) && close(a1, 270.0), "{c:?} {r} {a0} {a1}");
    assert_eq!(syms[0].element, Element { kind: Kind::Family, id: 7 });
    // a symbol above the view top is not drawn
    let mut s2 = scene(vec![], vec![], vec![]);
    s2.families = vec![FamilyRef { id: 1, family: fams[k].clone() }];
    s2.items = vec![FamilyItem { id: 8, name: "D".into(), family: 0, type_name: None, frame: Frame { origin: [0.0, 0.0, 2.5], x: [1.0, 0.0] }, instance: fams[k].solve(&Request::default()).unwrap(), host: None }];
    assert!(plan_view(&s2, 0, range()).unwrap().items.is_empty());
}

#[test]
fn the_level_decides_the_heights_and_bad_input_is_an_error() {
    // a wall on the second level (base 3 m): level 1 cuts it, level 0 sees nothing of it
    let mut c = straight();
    c.base_z = 3.0;
    let s = scene(vec![c], vec![], vec![]);
    assert!(plan_view(&s, 0, range()).unwrap().items.is_empty());
    let v = plan_view(&s, 1, range()).unwrap();
    assert_eq!((v.level_name.as_str(), v.elevation, fills(&v).len()), ("L1", 3.0, 1));
    assert_eq!(plan_view(&s, 2, range()), Err(ViewError::NoSuchLevel(2)));
    assert!(ViewRange::new(1.0, 2.0, 0.0, -1.0).is_err() && ViewRange::new(2.0, 1.0, 1.5, -1.0).is_err() && ViewRange::new(f64::NAN, 1.0, 0.0, -1.0).is_err());
    // a wall that cannot be built is named
    let bad = WallChain::new(vec![[0.0, 0.0], [0.0, 0.0]], false, 0.2, 3.0);
    let s = scene(vec![bad], vec![], vec![]);
    assert!(matches!(plan_view(&s, 0, range()), Err(ViewError::Wall { id: 100, .. })));
}

#[test]
fn the_view_does_not_depend_on_the_number_of_threads_and_draws_as_svg() {
    let mut walls = Vec::new();
    for k in 0..200 {
        let mut c = WallChain::new(vec![[0.0, k as f64 * 5.0], [4.0, k as f64 * 5.0], [4.0, k as f64 * 5.0 + 3.0]], false, 0.2, 3.0);
        c.openings = vec![vec![Opening { offset: 1.5, width: 1.0, sill: 0.0, height: 2.1 }], vec![]];
        walls.push(c);
    }
    let s = scene(walls, vec![], vec![]);
    let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| plan_view(&s, 0, range())).unwrap();
    let four = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(|| plan_view(&s, 0, range())).unwrap();
    assert_eq!(one, four);
    let svg1 = svg(&one);
    assert_eq!(svg1, svg(&four));
    assert!(svg1.starts_with("<svg ") && svg1.ends_with("</svg>") && svg1.contains("<polygon"));
    assert_eq!(plan_view(&scene(vec![], vec![], vec![]), 0, range()).unwrap().bounds(), None);
}
