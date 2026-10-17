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

// ------------------------------------------------------------------ slabs and rooms

use openbim_arch::Room;
use openbim_ifc::boundaries::SeparationLine;
use openbim_ifc::{RoomItem, SlabItem, SlabRole};
use openbim_place::floors::{FloorOpeningKind, FloorPenId, FloorPenetration, ShaftVoid, Slab};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> { vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]] }

fn slab_at(top: f64, thickness: f64) -> SlabItem {
    SlabItem { id: 50, slab: Slab { name: "S".into(), top, thickness, outline: rect(0.0, 0.0, 6.0, 4.0), fire_rating_min: None }, role: SlabRole::Structure, room_bounding: true }
}

fn pen(slab: usize, kind: FloorOpeningKind, fits: bool) -> FloorPenetration {
    FloorPenetration { id: FloorPenId { slab, seq: 0 }, runs: vec![], systems: vec![], domains: vec![], x0: 1.0, x1: 1.5, y0: 1.0, y1: 1.5, centre: [1.25, 1.25, 0.0], fire_rating_min: None, kind, fits }
}

type Ring = Vec<[f64; 2]>;

fn regions(v: &PlanView, kind: Kind) -> Vec<(Ring, Vec<Ring>)> {
    v.items.iter().filter(|i| i.element.kind == kind).filter_map(|i| if let Shape::Region { outer, holes } = &i.shape { Some((outer.clone(), holes.clone())) } else { None }).collect()
}

#[test]
fn a_slab_follows_the_view_range_and_its_voids_are_holes() {
    // the floor at the level itself is below the view bottom (0): beyond, outline only
    let mut s = scene(vec![], vec![], vec![]);
    s.slabs = vec![slab_at(0.0, 0.2)];
    let v = plan_view(&s, 0, range()).unwrap();
    assert!(v.items.iter().all(|i| i.class == Class::Beyond && i.element == Element { kind: Kind::Slab, id: 50 }));
    assert_eq!((v.items.len(), regions(&v, Kind::Slab).len()), (4, 0));
    // a slab the cut plane passes through (1.1 to 1.3 m): filled, outline too
    s.slabs = vec![slab_at(1.3, 0.2)];
    let v = plan_view(&s, 0, range()).unwrap();
    let r = regions(&v, Kind::Slab);
    assert_eq!((r.len(), r[0].1.len(), lines(&v, Class::Cut)), (1, 0, 4));
    // a shaft through it, and an opening of its own: two holes, in the fill and as lines
    s.shafts = vec![ShaftVoid { name: "Sh".into(), outline: rect(4.0, 1.0, 5.0, 2.0), z_bottom: 0.0, z_top: 3.0 }];
    s.floor_penetrations = vec![pen(0, FloorOpeningKind::Cut, true)];
    let v = plan_view(&s, 0, range()).unwrap();
    let r = regions(&v, Kind::Slab);
    assert_eq!((r[0].1.len(), lines(&v, Class::Cut)), (2, 12));
    // an opening that is only inside a shaft, one that does not fit, a shaft that ends below the slab, and an
    // opening in another slab make no holes
    s.floor_penetrations = vec![pen(0, FloorOpeningKind::InShaft(0), true), pen(0, FloorOpeningKind::Cut, false), pen(1, FloorOpeningKind::Cut, true)];
    s.shafts[0].z_top = 1.3;
    let v = plan_view(&s, 0, range()).unwrap();
    assert_eq!(regions(&v, Kind::Slab)[0].1.len(), 0);
}

#[test]
fn rooms_of_the_level_are_regions_with_a_tag_inside_them_and_the_area_without_holes() {
    let room = |level: usize, outline: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>| RoomItem { id: 9, room: Room { name: "Office <1>".into(), usage: "office".into(), occupancy: 4.0, level, outline, holes, height: 2.7 } };
    let mut s = scene(vec![], vec![], vec![]);
    s.rooms = vec![room(0, rect(0.0, 0.0, 6.0, 4.0), vec![])];
    let v = plan_view(&s, 0, range()).unwrap();
    assert_eq!(v.tags.len(), 1);
    let t = &v.tags[0];
    assert!(close(t.at[0], 3.0) && close(t.at[1], 2.0) && close(t.area, 24.0) && t.name == "Office <1>" && t.usage == "office" && t.id == 9);
    assert!(v.items.iter().all(|i| i.class == Class::Room && i.element.kind == Kind::Room));
    // a core in the middle takes its area off and moves the tag out of it
    s.rooms = vec![room(0, rect(0.0, 0.0, 6.0, 4.0), vec![rect(2.5, 1.5, 3.5, 2.5)])];
    let v = plan_view(&s, 0, range()).unwrap();
    let t = &v.tags[0];
    assert!(close(t.area, 23.0));
    assert!(!(t.at[0] > 2.5 && t.at[0] < 3.5 && t.at[1] > 1.5 && t.at[1] < 2.5), "{:?}", t.at);
    assert!(t.at[0] > 0.0 && t.at[0] < 6.0 && t.at[1] > 0.0 && t.at[1] < 4.0);
    assert_eq!(regions(&v, Kind::Room)[0].1.len(), 1);
    // a U shape: the centre of mass is in the notch, the tag is not
    let u = vec![[0.0, 0.0], [9.0, 0.0], [9.0, 6.0], [6.0, 6.0], [6.0, 2.0], [3.0, 2.0], [3.0, 6.0], [0.0, 6.0]];
    s.rooms = vec![room(0, u.clone(), vec![])];
    let t = plan_view(&s, 0, range()).unwrap().tags[0].clone();
    assert!(close(t.area, 54.0 - 12.0));
    let in_notch = t.at[0] > 3.0 && t.at[0] < 6.0 && t.at[1] > 2.0;
    assert!(!in_notch && t.at[0] > 0.0 && t.at[0] < 9.0 && t.at[1] > 0.0 && t.at[1] < 6.0, "{:?}", t.at);
    // a room of another level is not on this plan
    s.rooms = vec![room(1, rect(0.0, 0.0, 6.0, 4.0), vec![])];
    let v = plan_view(&s, 0, range()).unwrap();
    assert!(v.items.is_empty() && v.tags.is_empty());
    assert_eq!(plan_view(&s, 1, range()).unwrap().tags.len(), 1);
}

#[test]
fn separation_lines_of_the_level_are_drawn_and_rooms_and_slabs_go_under_the_walls() {
    let mut s = scene(vec![straight()], vec![], vec![]);
    s.separation_lines = vec![SeparationLine { id: 70, level: 0, a: [0.0, 1.0], b: [4.0, 1.0] }, SeparationLine { id: 71, level: 1, a: [0.0, 2.0], b: [4.0, 2.0] }];
    s.slabs = vec![slab_at(0.0, 0.2)];
    s.rooms = vec![RoomItem { id: 9, room: Room { name: "R".into(), usage: "".into(), occupancy: 0.0, level: 0, outline: rect(0.0, 1.0, 4.0, 3.0), holes: vec![], height: 2.7 } }];
    let v = plan_view(&s, 0, range()).unwrap();
    let kinds: Vec<Kind> = v.items.iter().map(|i| i.element.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == Kind::SeparationLine).count(), 1);
    let last_under = kinds.iter().rposition(|k| matches!(k, Kind::Room | Kind::SeparationLine | Kind::Slab)).unwrap();
    let first_wall = kinds.iter().position(|k| *k == Kind::Wall).unwrap();
    assert!(last_under < first_wall, "{kinds:?}");
    // the drawing: the region has evenodd fill, the tag text is escaped and upright, the separation line is a line
    let d = svg(&v);
    assert!(d.contains("fill-rule=\"evenodd\"") && d.contains(">R</text>") && d.contains("8.0 m²"));
    let mut s2 = scene(vec![], vec![], vec![]);
    s2.rooms = vec![RoomItem { id: 9, room: Room { name: "A & B <c>".into(), usage: "".into(), occupancy: 0.0, level: 0, outline: rect(0.0, 0.0, 2.0, 2.0), holes: vec![], height: 2.7 } }];
    assert!(svg(&plan_view(&s2, 0, range()).unwrap()).contains("A &amp; B &lt;c&gt;"));
    // bounds include a region
    assert_eq!(plan_view(&s2, 0, range()).unwrap().bounds(), Some([0.0, 0.0, 2.0, 2.0]));
}
