use openbim_arch::{Level, Levels, Room};
use openbim_family::Instance;
use openbim_geom::{Opening, WallChain};
use openbim_ifc::boundaries::*;
use openbim_ifc::*;
use openbim_place::floors::Slab;

fn scene(walls: Vec<WallChain>, slabs: Vec<Slab>, room: Room) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        beams: vec![], materials: Default::default(),
        timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }]).unwrap(),
        walls: walls.into_iter().enumerate().map(|(i, c)| WallItem { id: 10 + i as u64, name: format!("W{i}"), chain: c, wall_type: None, room_bounding: true }).collect(),
        wall_penetrations: vec![],
        firestops: Default::default(),
        slabs: slabs.into_iter().enumerate().map(|(i, slab)| SlabItem { id: 50 + i as u64, slab, room_bounding: true, role: Default::default() }).collect(),
        columns: vec![],
        shafts: vec![],
        floor_penetrations: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![RoomItem { id: 90, room }],
        wall_types: vec![],
        wall_joins: vec![],
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        room_lighting: Default::default(),
        pset_map: Default::default(), types: vec![], member_info: Default::default(),
    }
}
fn room(outline: &[[f64; 2]], level: usize) -> Room {
    Room { name: "R".into(), usage: "u".into(), occupancy: 0.0, level, outline: outline.to_vec(), holes: vec![], height: 2.8 }
}
fn slab(top: f64, thickness: f64, x1: f64, y1: f64) -> Slab {
    Slab { name: format!("S{top}"), top, thickness, outline: vec![[-1.0, -1.0], [x1, -1.0], [x1, y1], [-1.0, y1]], fire_rating_min: None }
}
fn enclosure() -> WallChain {
    // centre line 0..4 by 0..3, thickness 0.2: the faces on the inside are at 0.1 and 3.9 by 0.1 and 2.9
    WallChain::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]], true, 0.2, 3.0)
}
const INSIDE: [[f64; 2]; 4] = [[0.1, 0.1], [3.9, 0.1], [3.9, 2.9], [0.1, 2.9]];

fn count(b: &[Boundary], f: impl Fn(&Target) -> bool) -> usize {
    b.iter().filter(|x| f(&x.target)).count()
}

#[test]
fn a_room_inside_a_closed_wall_has_one_boundary_for_each_face() {
    let s = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(count(&b, |t| matches!(t, Target::Wall(0))), 4);
    // the floor slab (top at the floor), the ceiling slab (bottom 2.7 is within 0.3 m of the room top 2.8)
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(0))), 1);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(1))), 1);
    assert_eq!(b.len(), 6);
    let walls: f64 = b.iter().filter(|x| matches!(x.target, Target::Wall(_))).map(|x| x.surface.area()).sum();
    assert!((walls - 2.0 * (3.8 + 2.8) * 2.8).abs() < 1e-9, "{walls}");
    for x in &b {
        if let Target::Slab(_) = x.target {
            assert!((x.surface.area() - 3.8 * 2.8).abs() < 1e-9);
        }
    }
}

#[test]
fn the_normal_of_each_face_points_out_of_the_room() {
    let s = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(3.0, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    for x in &b {
        let centre = {
            let p: Vec<[f64; 3]> = x.surface.outer.iter().map(|q| x.surface.point(*q)).collect();
            let n = p.len() as f64;
            [p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n, p.iter().map(|q| q[2]).sum::<f64>() / n]
        };
        let step = |sign: f64| [centre[0] + sign * 0.05 * x.surface.axis[0], centre[1] + sign * 0.05 * x.surface.axis[1], centre[2] + sign * 0.05 * x.surface.axis[2]];
        let inside = |p: [f64; 3]| p[0] > 0.1 && p[0] < 3.9 && p[1] > 0.1 && p[1] < 2.9 && p[2] > 0.0 && p[2] < 2.8;
        assert!(!inside(step(1.0)) && inside(step(-1.0)), "{:?}", x.target);
    }
}

#[test]
fn an_opening_in_the_face_is_a_hole_and_the_hole_is_clipped_to_the_room() {
    let mut w = enclosure();
    // 0.9 wide door in the south wall at 1.0..1.9 along the segment, and a window across the corner of segment 1's start
    w.openings = vec![vec![Opening { offset: 1.0, width: 0.9, sill: 0.0, height: 2.1 }], vec![], vec![], vec![]];
    let s = scene(vec![w], vec![], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    let south = b.iter().find(|x| matches!(x.target, Target::Wall(0)) && x.surface.axis == [0.0, -1.0, 0.0]).expect("south face");
    assert_eq!(south.surface.inner.len(), 1);
    let area = south.surface.area();
    assert!((area - (3.8 * 2.8 - 0.9 * 2.1)).abs() < 1e-9, "{area}");
    // the hole is where the door is: u runs from x = 0.1
    let h = &south.surface.inner[0];
    let (u0, u1) = (h.iter().map(|p| p[0]).fold(f64::MAX, f64::min), h.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
    assert!((u0 - 0.9).abs() < 1e-9 && (u1 - 1.8).abs() < 1e-9, "{u0} {u1}");
}

#[test]
fn walls_that_do_not_touch_the_room_do_not_bound_it() {
    // a wall across the middle, 0.5 from the room's edge, one on another storey, and the enclosure with the room pulled in
    let mid = WallChain::new(vec![[2.0, 0.1], [2.0, 2.9]], false, 0.1, 3.0);
    let mut up = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0);
    up.base_z = 3.0;
    let s = scene(vec![mid, up], vec![], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(b.is_empty(), "{:?}", b.iter().map(|x| x.target).collect::<Vec<_>>());
    let s2 = scene(vec![enclosure()], vec![], room(&[[0.5, 0.5], [3.5, 0.5], [3.5, 2.5], [0.5, 2.5]], 0));
    assert!(boundaries::room_boundaries(&s2, &s2.rooms[0]).0.is_empty());
}

#[test]
fn a_wall_that_covers_part_of_a_side_bounds_that_part() {
    // a partition 0.1 thick at x = 2, from y 0.1 to 1.0: its west face bounds a room on its west side along 0.9 m
    let part = WallChain::new(vec![[2.0, 0.1], [2.0, 1.0]], false, 0.1, 3.0);
    let s = scene(vec![part], vec![], room(&[[0.1, 0.1], [1.95, 0.1], [1.95, 2.9], [0.1, 2.9]], 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 1);
    assert!((b[0].surface.area() - 0.9 * 2.8).abs() < 1e-9, "{}", b[0].surface.area());
    assert_eq!(b[0].surface.axis, [1.0, 0.0, 0.0]);
    // it starts at the end of the partition (y = 0.1) and runs 0.9 up
    assert!((b[0].surface.origin[1] - 0.1).abs() < 1e-9);
}

#[test]
fn a_wall_lower_than_the_room_gives_a_lower_face() {
    let mut w = enclosure();
    w.height = 2.0;
    let s = scene(vec![w], vec![], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 4);
    assert!(b.iter().all(|x| (x.surface.outer.iter().map(|p| p[1]).fold(f64::MIN, f64::max) - 2.0).abs() < 1e-9));
}

#[test]
fn the_floor_and_ceiling_need_a_slab_that_covers_the_room_and_sits_at_the_right_height() {
    // the slab ends short of the room: no floor
    let short = slab(0.0, 0.2, 2.0, 2.0);
    let s = scene(vec![], vec![short], room(&INSIDE, 0));
    assert!(boundaries::room_boundaries(&s, &s.rooms[0]).0.is_empty());
    // a floor 0.2 below the room's floor level is not the floor
    let s = scene(vec![], vec![slab(-0.2, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    assert!(boundaries::room_boundaries(&s, &s.rooms[0]).0.is_empty());
    // a slab whose bottom is 1.0 m above the room top: no ceiling, and the reason is given
    let s = scene(vec![], vec![slab(4.0, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(b.is_empty());
    assert_eq!(w.iter().filter(|m| m.contains("slab")).count(), 1, "{w:?}");
    // of two slabs above, the lower is the ceiling
    let s = scene(vec![], vec![slab(3.5, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].target, Target::Slab(1));
    assert_eq!(b[0].surface.axis, [0.0, 0.0, 1.0]);
    assert!((b[0].surface.origin[2] - 2.7).abs() < 1e-9);
}

#[test]
fn a_room_with_a_hole_has_the_hole_in_its_floor() {
    let mut r = room(&INSIDE, 0);
    r.holes = vec![vec![[1.0, 1.0], [1.0, 2.0], [2.0, 2.0], [2.0, 1.0]]];
    let s = scene(vec![], vec![slab(0.0, 0.2, 5.0, 4.0)], r);
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].surface.inner.len(), 1);
    assert!((b[0].surface.area() - (3.8 * 2.8 - 1.0)).abs() < 1e-9);
    assert_eq!(b[0].surface.axis, [0.0, 0.0, -1.0]);
}

#[test]
fn a_room_on_the_upper_storey_is_in_the_frame_of_its_storey() {
    let mut w = enclosure();
    w.base_z = 3.0;
    let s = scene(vec![w], vec![slab(3.0, 0.2, 5.0, 4.0)], room(&INSIDE, 1));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 5);
    // heights are from the storey's elevation
    for x in &b {
        for p in &x.surface.outer {
            let q = x.surface.point(*p);
            assert!(q[2] > -1e-9 && q[2] < 2.8 + 1e-9, "{q:?}");
        }
    }
}

#[test]
fn the_boundaries_are_written_with_their_geometry() {
    let s = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let out = export(&s).unwrap();
    assert_eq!(out.report.space_boundaries, 6);
    assert_eq!(out.text.matches("IFCRELSPACEBOUNDARY2NDLEVEL(").count(), 6);
    assert_eq!(out.text.matches("IFCCURVEBOUNDEDPLANE(").count(), 6);
}

fn thin(points: &[[f64; 2]], t: f64, base: f64, h: f64) -> WallChain {
    let mut w = WallChain::new(points.to_vec(), false, t, h);
    w.base_z = base;
    w
}
fn instance(opening: Option<[f64; 3]>) -> Instance {
    Instance { values: vec![], solids: vec![], materials: vec![], xf: vec![], strings: vec![], texts: vec![], opening, opening_rise: 0.0, connectors: vec![], plan: vec![], forms: vec![], form_materials: vec![] }
}
fn item(id: u64, chain: &WallChain, wall: usize, segment: usize, centre: f64, opening: [f64; 3]) -> FamilyItem {
    FamilyItem { id, name: format!("I{id}"), family: 0, type_name: None, frame: Frame::on_wall(chain, segment, centre), instance: instance(Some(opening)), host: Some((wall, segment, centre)) }
}

#[test]
fn a_face_longer_than_the_side_is_cut_to_the_side_and_to_the_room_height() {
    // the wall runs from y = -2 to 5 and from z = -1 to 4, past the room on every side
    let mut w = thin(&[[0.0, -2.0], [0.0, 5.0]], 0.2, -1.0, 5.0);
    // an opening across the end of the side (y 2.5..3.3) and below the floor (z -0.5..1.5)
    w.openings = vec![vec![Opening { offset: 4.5, width: 0.8, sill: 0.5, height: 2.0 }]];
    let s = scene(vec![w], vec![], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 1);
    let f = &b[0].surface;
    assert_eq!(f.origin, [0.1, 2.9, 0.0]);
    assert_eq!(f.outer, vec![[0.0, 0.0], [2.8, 0.0], [2.8, 2.8], [0.0, 2.8]]);
    let h = &f.inner[0];
    let (u0, u1) = (h.iter().map(|p| p[0]).fold(f64::MAX, f64::min), h.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
    let (v0, v1) = (h.iter().map(|p| p[1]).fold(f64::MAX, f64::min), h.iter().map(|p| p[1]).fold(f64::MIN, f64::max));
    assert!(near(u0, 0.0) && near(u1, 0.4) && near(v0, 0.0) && near(v1, 1.5), "{h:?}");
    assert!(near(f.area(), 2.8 * 2.8 - 0.4 * 1.5));
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_hole_is_measured_from_where_the_face_begins() {
    // the face covers the side from y = 1.5 down to 0.1 only (the wall ends at y = 1.5)
    let mut w = thin(&[[0.0, -2.0], [0.0, 1.5]], 0.2, 0.0, 3.0);
    // an opening at y 0.5..1.2, which is 1.7..2.4 along the side from its start at y = 2.9
    w.openings = vec![vec![Opening { offset: 2.5, width: 0.7, sill: 0.0, height: 2.0 }]];
    let s = scene(vec![w], vec![], room(&INSIDE, 0));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 1);
    let f = &b[0].surface;
    // the face starts 1.4 m along the side, and is 1.4 m long
    assert!(near(f.origin[1], 1.5) && near(f.outer[1][0], 1.4), "{:?} {:?}", f.origin, f.outer);
    let h = &f.inner[0];
    let (u0, u1) = (h.iter().map(|p| p[0]).fold(f64::MAX, f64::min), h.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
    assert!(near(u0, 0.3) && near(u1, 1.0), "{u0} {u1}");
}

#[test]
fn a_wall_that_touches_the_side_at_a_point_or_slants_away_or_stands_inside_the_room_bounds_nothing() {
    // ends exactly at the corner of the room, running away from it
    let away = thin(&[[0.0, 2.9], [0.0, 5.0]], 0.2, 0.0, 3.0);
    // starts on the side's line but leaves it at an angle
    let slant = thin(&[[0.1, 1.0], [-1.0, 2.0]], 0.001, 0.0, 3.0);
    // its face lies on the side's line, but the wall is on the room's side of it
    let inside = thin(&[[0.2, 0.0], [0.2, 5.0]], 0.2, 0.0, 3.0);
    for w in [away, slant, inside] {
        let s = scene(vec![w.clone()], vec![], room(&INSIDE, 0));
        let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
        assert!(b.is_empty(), "{:?} {:?}", w.points, b.iter().map(|x| x.surface.area()).collect::<Vec<_>>());
    }
}

#[test]
fn a_wall_with_a_sloped_top_is_left_out_and_a_wall_below_the_storey_does_not_bound_it() {
    let mut w = enclosure();
    w.tops = vec![Some(vec![[0.0, 0.0]]), None, None, None];
    let s = scene(vec![w], vec![], room(&INSIDE, 0));
    assert!(boundaries::room_boundaries(&s, &s.rooms[0]).0.is_empty());
    // the enclosure of the storey below, 3.0 high, under a room whose floor is at 3.0
    let s = scene(vec![enclosure()], vec![], room(&INSIDE, 1));
    assert!(boundaries::room_boundaries(&s, &s.rooms[0]).0.is_empty());
}

#[test]
fn a_door_that_fills_an_opening_has_a_face_of_its_own_inside_the_wall_face() {
    let mut w = enclosure();
    // south wall: two openings, 1.0..1.9 (a door, 2.1 high) and 2.5..3.5 (a window, sill 0.9, 1.2 high)
    w.openings = vec![vec![Opening { offset: 1.0, width: 0.9, sill: 0.0, height: 2.1 }, Opening { offset: 2.5, width: 1.0, sill: 0.9, height: 1.2 }], vec![], vec![], vec![]];
    let items = vec![
        item(70, &w, 0, 0, 1.45, [0.9, 2.1, 0.0]),
        item(71, &w, 0, 0, 3.0, [1.0, 1.2, 0.9]),
        // the right place but not the right size, on another segment, on another wall, not in a wall at all
        item(72, &w, 0, 0, 1.45, [0.8, 2.1, 0.0]),
        item(73, &w, 0, 1, 1.45, [0.9, 2.1, 0.0]),
        item(74, &w, 1, 0, 1.45, [0.9, 2.1, 0.0]),
        item(75, &w, 0, 0, 1.45, [0.9, 2.0, 0.0]),
        item(76, &w, 0, 0, 3.0, [1.0, 1.2, 1.0]),
        item(77, &w, 0, 0, 1.55, [0.9, 2.1, 0.0]),
        // starts where the door's opening starts but is narrower
        item(78, &w, 0, 0, 1.4, [0.8, 2.1, 0.0]),
    ];
    let mut s = scene(vec![w.clone()], vec![], room(&INSIDE, 0));
    s.items = items;
    s.items[7].host = Some((0, 0, 1.55));
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    let kids: Vec<&Boundary> = b.iter().filter(|x| matches!(x.target, Target::Item(_))).collect();
    assert_eq!(kids.iter().map(|k| k.target).collect::<Vec<_>>(), vec![Target::Item(0), Target::Item(1)]);
    let south = b.iter().position(|x| matches!(x.target, Target::Wall(0)) && x.surface.axis == [0.0, -1.0, 0.0]).unwrap();
    for k in &kids {
        assert_eq!(k.parent, Some(south));
        // the same plane as the wall's face, and its outline is the hole
        let (kf, wf) = (&k.surface, &b[south].surface);
        assert_eq!((kf.origin, kf.axis, kf.refdir), (wf.origin, wf.axis, wf.refdir));
        assert!(wf.inner.contains(&kf.outer));
    }
    assert!(near(kids[0].surface.area(), 0.9 * 2.1) && near(kids[1].surface.area(), 1.0 * 1.2));
    assert!(b.iter().filter(|x| !matches!(x.target, Target::Item(_))).all(|x| x.parent.is_none()));
}

#[test]
fn a_slab_inside_the_height_of_the_room_is_neither_floor_nor_ceiling() {
    // a mezzanine deck 1.0 m up across the whole room
    let s = scene(vec![], vec![slab(1.0, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(b.is_empty() && !w.iter().any(|m| m.contains("slab")), "{w:?}");
}

#[test]
fn the_ceiling_of_a_room_with_a_hole_has_the_hole_too() {
    let mut r = room(&INSIDE, 0);
    r.holes = vec![vec![[1.0, 1.0], [1.0, 2.0], [2.0, 2.0], [2.0, 1.0]]];
    let s = scene(vec![], vec![slab(3.0, 0.2, 5.0, 4.0)], r);
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].surface.axis, [0.0, 0.0, 1.0]);
    assert_eq!(b[0].surface.inner.len(), 1);
    assert!(near(b[0].surface.area(), 3.8 * 2.8 - 1.0));
    // the hole is where it was in the plan: plan x of the corners 1..2, y 1..2
    for p in &b[0].surface.inner[0] {
        let q = b[0].surface.point(*p);
        assert!((1.0 - 1e-9..=2.0 + 1e-9).contains(&q[0]) && (1.0 - 1e-9..=2.0 + 1e-9).contains(&q[1]) && near(q[2], 2.8), "{q:?}");
    }
}

#[test]
fn a_wall_that_does_not_bound_rooms_makes_no_boundary_and_the_side_is_reported() {
    let mut s = scene(vec![enclosure()], vec![], room(&INSIDE, 0));
    s.walls[0].room_bounding = false;
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Wall(_))), 0);
    assert_eq!(w.iter().filter(|m| m.contains("no wall and no separation line")).count(), 4, "{w:?}");
}

#[test]
fn a_separation_line_bounds_a_side_without_a_wall_and_is_virtual() {
    let one_wall = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0);
    let mut s = scene(vec![one_wall], vec![], room(&INSIDE, 0));
    // the other three sides are lines on the room's edges
    s.separation_lines = vec![
        SeparationLine { id: 1, level: 0, a: [3.9, 0.1], b: [3.9, 2.9] },
        SeparationLine { id: 2, level: 0, a: [3.9, 2.9], b: [0.1, 2.9] },
        SeparationLine { id: 3, level: 0, a: [0.1, 2.9], b: [0.1, 0.1] },
    ];
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(count(&b, |t| matches!(t, Target::Wall(0))), 1);
    assert_eq!(count(&b, |t| matches!(t, Target::Virtual(_))), 3);
    for x in b.iter().filter(|x| matches!(x.target, Target::Virtual(_))) {
        assert!((x.surface.area() - x.surface.outer[2][0] * 2.8).abs() < 1e-9);
        assert_eq!(x.surface.axis[2], 0.0);
    }
    // a line on another level does nothing
    s.separation_lines[0].level = 1;
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Virtual(_))), 2);
    assert_eq!(w.len(), 1, "{w:?}");
}

#[test]
fn the_boundary_location_moves_the_boundary_to_the_centre_line() {
    let mut s = scene(vec![enclosure()], vec![], room(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]], 0));
    // the finish faces are not on the room's edges, so nothing bounds it
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Wall(_))), 0);
    s.room_settings.boundary_location = BoundaryLocation::WallCentre;
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Wall(_))), 4, "{w:?}");
    let walls: f64 = b.iter().map(|x| x.surface.area()).sum();
    assert!((walls - 2.0 * (4.0 + 3.0) * 2.8).abs() < 1e-6, "{walls}");
}

#[test]
fn without_volumes_the_floor_and_ceiling_are_not_boundaries() {
    let mut s = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    s.room_settings.volumes = false;
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(_))), 0);
    assert_eq!(b.len(), 4);
}

fn finished_scene() -> Scene {
    // 0: floor slab, 1: slab above (bottom 2.7), 2: floor finish 50 mm, 3: false ceiling at 2.48
    let mut s = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0), slab(0.05, 0.05, 5.0, 4.0), slab(2.5, 0.02, 5.0, 4.0)], room(&INSIDE, 0));
    s.slabs[2].role = SlabRole::FloorFinish;
    s.slabs[3].role = SlabRole::Ceiling;
    s
}
fn z_of(b: &Boundary) -> f64 {
    b.surface.origin[2]
}

#[test]
fn a_floor_finish_and_a_false_ceiling_become_the_floor_and_ceiling_of_the_room() {
    let s = finished_scene();
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(w.is_empty(), "{w:?}");
    let floor: Vec<_> = b.iter().filter(|x| x.surface.axis[2] < -0.5).collect();
    let ceil: Vec<_> = b.iter().filter(|x| x.surface.axis[2] > 0.5).collect();
    assert_eq!((floor.len(), ceil.len()), (1, 1));
    assert_eq!(floor[0].target, Target::Slab(2));
    assert!((z_of(floor[0]) - 0.05).abs() < 1e-9);
    assert_eq!(ceil[0].target, Target::Slab(3));
    assert!((z_of(ceil[0]) - 2.48).abs() < 1e-9);
    // the walls run between them
    let walls: f64 = b.iter().filter(|x| matches!(x.target, Target::Wall(_))).map(|x| x.surface.area()).sum();
    assert!((walls - 2.0 * (3.8 + 2.8) * 2.43).abs() < 1e-9, "{walls}");
}

#[test]
fn slab_to_slab_is_the_basis_that_ignores_finishes() {
    let mut s = finished_scene();
    s.room_settings.volume_basis = VolumeBasis::SlabToSlab;
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(0))), 1);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(1))), 1);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(2) | Target::Slab(3))), 0);
    let v = boundaries::room_volumes(&s, &s.rooms[0]);
    assert!((v.volume - 10.64 * 2.8).abs() < 1e-9 && v.volume == v.slab_to_slab);
}

#[test]
fn volumes_split_into_the_room_the_ceiling_void_and_the_floor_build_up() {
    let s = finished_scene();
    let v = boundaries::room_volumes(&s, &s.rooms[0]);
    let a = 3.8 * 2.8;
    assert!((v.volume - a * (2.48 - 0.05)).abs() < 1e-9, "{v:?}");
    assert!((v.slab_to_slab - a * 2.8).abs() < 1e-9);
    assert!((v.ceiling_void - a * (2.8 - 2.48)).abs() < 1e-9);
    assert!((v.floor_build_up - a * 0.05).abs() < 1e-9);
    assert!((v.volume + v.ceiling_void + v.floor_build_up - v.slab_to_slab).abs() < 1e-9);
    // without finishes the room is slab to slab
    let plain = scene(vec![enclosure()], vec![slab(0.0, 0.2, 5.0, 4.0), slab(2.9, 0.2, 5.0, 4.0)], room(&INSIDE, 0));
    let v = boundaries::room_volumes(&plain, &plain.rooms[0]);
    assert!(v.ceiling_void == 0.0 && v.floor_build_up == 0.0 && (v.volume - a * 2.8).abs() < 1e-9);
}

#[test]
fn air_changes_use_the_room_or_the_room_with_its_ceiling_void() {
    let s = finished_scene();
    let v = boundaries::room_volumes(&s, &s.rooms[0]);
    let room_only = boundaries::air_changes_per_hour(0.5, &v, false).unwrap();
    let with_void = boundaries::air_changes_per_hour(0.5, &v, true).unwrap();
    assert!((room_only - 0.5 * 3600.0 / v.volume).abs() < 1e-9);
    assert!((with_void - 0.5 * 3600.0 / (v.volume + v.ceiling_void)).abs() < 1e-9);
    assert!(with_void < room_only);
    assert!(boundaries::air_changes_per_hour(f64::NAN, &v, false).is_none());
    let none = RoomVolumes { volume: 0.0, slab_to_slab: 0.0, ceiling_void: 0.0, floor_build_up: 0.0 };
    assert!(boundaries::air_changes_per_hour(1.0, &none, true).is_none());
}

#[test]
fn a_finish_that_does_not_stand_on_the_level_or_hangs_too_low_is_not_a_boundary() {
    let mut s = finished_scene();
    // a finish floating 0.3 above the level
    s.slabs[2].slab.top = 0.35;
    // a ceiling below the middle of the room
    s.slabs[3].slab.top = 1.0;
    let (b, _) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(2) | Target::Slab(3))), 0);
    assert_eq!(count(&b, |t| matches!(t, Target::Slab(0) | Target::Slab(1))), 2);
}

fn column(id: u64, x0: f64, y0: f64, x1: f64, y1: f64) -> ColumnItem {
    ColumnItem { id, name: format!("C{id}"), footprint: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], base_z: 0.0, height: 3.0, room_bounding: true }
}
/// the enclosure room with a notch round a column sticking 0.2 into it from the south wall (finish face at y = 0.1)
fn notched_room() -> Room {
    room(&[[0.1, 0.1], [1.7, 0.1], [1.7, 0.3], [2.3, 0.3], [2.3, 0.1], [3.9, 0.1], [3.9, 2.9], [0.1, 2.9]], 0)
}
// the column stands across the south wall: x 1.7..2.3, y -0.3..0.3
fn with_column() -> Scene {
    let mut s = scene(vec![enclosure()], vec![], notched_room());
    s.columns = vec![column(70, 1.7, -0.3, 2.3, 0.3)];
    s
}

#[test]
fn the_faces_of_a_column_with_a_wall_attached_that_look_into_the_room_are_boundaries() {
    let s = with_column();
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert!(w.is_empty(), "{w:?}");
    let faces: Vec<_> = b.iter().filter(|x| matches!(x.target, Target::Column(0))).collect();
    // the three faces of the part that sticks out: 0.2 + 0.6 + 0.2 wide, 2.8 high
    assert_eq!(faces.len(), 3);
    let mut widths: Vec<f64> = faces.iter().map(|f| f.surface.area() / 2.8).collect();
    widths.sort_by(f64::total_cmp);
    assert!((widths[0] - 0.2).abs() < 1e-9 && (widths[1] - 0.2).abs() < 1e-9 && (widths[2] - 0.6).abs() < 1e-9, "{widths:?}");
    // the south wall bounds the rest of that side: 1.6 + 1.6
    let south: f64 = b.iter().filter(|x| matches!(x.target, Target::Wall(0)) && x.surface.axis[1] < -0.5).map(|x| x.surface.area() / 2.8).sum();
    assert!((south - 3.2).abs() < 1e-9, "{south}");
}

#[test]
fn a_column_with_no_wall_at_it_or_not_room_bounding_is_not_a_boundary() {
    // the same column, away from every wall
    let mut s = with_column();
    s.columns[0] = column(70, 1.7, -2.3, 2.3, -1.7);
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Column(_))), 0);
    assert!(!w.is_empty(), "the notch is bounded by nothing, and that is said");
    // attached, but switched off
    let mut s = with_column();
    s.columns[0].room_bounding = false;
    let (b, w) = boundaries::room_boundaries(&s, &s.rooms[0]);
    assert_eq!(count(&b, |t| matches!(t, Target::Column(_))), 0);
    assert!(!w.is_empty());
}

#[test]
fn a_column_clockwise_or_a_storey_apart_behaves_as_stated() {
    let mut s = with_column();
    s.columns[0].footprint.reverse();
    assert_eq!(count(&boundaries::room_boundaries(&s, &s.rooms[0]).0, |t| matches!(t, Target::Column(_))), 3);
    // a column on the floor above the room's top does not bound it
    let mut s = with_column();
    s.columns[0].base_z = 3.0;
    assert_eq!(count(&boundaries::room_boundaries(&s, &s.rooms[0]).0, |t| matches!(t, Target::Column(_))), 0);
}
