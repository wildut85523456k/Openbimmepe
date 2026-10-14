use openbim_arch::*;
use openbim_geom::WallChain;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn rect(w: f64, d: f64, t: f64, h: f64) -> WallChain { WallChain::new(vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], true, t, h) }
fn at(c: &WallChain, dx: f64, dy: f64) -> WallChain {
    let mut c = c.clone();
    for p in c.points.iter_mut() { p[0] += dx; p[1] += dy; }
    c
}

#[test]
fn a_rectangular_room_has_the_hand_calculated_figures() {
    let r = Room::from_chain("Office 1", "Office", 6.0, 0, &rect(8.0, 5.0, 0.2, 3.0), &[], None).unwrap();
    assert!(close(r.area(), 7.8 * 4.8));
    assert!(close(r.perimeter(), 2.0 * (7.8 + 4.8)));
    assert!(close(r.volume(), 7.8 * 4.8 * 3.0));
    assert!(close(r.area_per_person().unwrap(), 7.8 * 4.8 / 6.0));
    assert!(r.contains([4.0, 2.5]) && !r.contains([0.05, 2.5]) && !r.contains([-1.0, 2.5]));
}

#[test]
fn drawing_direction_does_not_matter() {
    let mut cw = rect(8.0, 5.0, 0.2, 3.0);
    cw.points.reverse();
    let a = Room::from_chain("a", "", 0.0, 0, &cw, &[], None).unwrap();
    assert!(close(a.area(), 7.8 * 4.8));
    assert!(a.outline.iter().all(|q| q[0] > 0.0 && q[0] < 8.0 && q[1] > 0.0 && q[1] < 5.0));
}

#[test]
fn an_l_shaped_room_and_a_core_inside() {
    // L: 6 x 4 with a 3 x 2 corner removed; wall thickness 0.4 -> faces 0.2 inside the centreline
    let l = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [3.0, 2.0], [3.0, 4.0], [0.0, 4.0]], true, 0.4, 3.0);
    let r = Room::from_chain("L", "Lobby", 10.0, 0, &l, &[], None).unwrap();
    // hand: centreline area 6*4 - 3*2 = 18; area inside faces = 18 - 0.2 * perimeter(20) + 4 * 0.2^2 * (convex 5 - reflex 1 = 4 net corners)... computed from the offset polygon directly:
    // outline x 0.2..5.8, y 0.2..1.8 (bottom strip) plus x 0.2..2.8, y 1.8..3.8 -> 5.6*1.6 + 2.6*2.0
    assert!(close(r.area(), 5.6 * 1.6 + 2.6 * 2.0), "{}", r.area());
    // a 1 x 1 core (walls 0.2) standing in the wide part
    let core = at(&rect(1.0, 1.0, 0.2, 3.0), 3.5, 0.6);
    let r2 = Room::from_chain("L", "Lobby", 10.0, 0, &l, &[&core], None).unwrap();
    assert!(close(r2.area(), 5.6 * 1.6 + 2.6 * 2.0 - 1.2 * 1.2));
    assert!(!r2.contains([4.0, 1.1]) && r2.contains([5.0, 1.0]));
}

#[test]
fn bad_rooms_are_refused() {
    let w = rect(8.0, 5.0, 0.2, 3.0);
    let open = WallChain::new(vec![[0.0, 0.0], [8.0, 0.0], [8.0, 5.0]], false, 0.2, 3.0);
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &open, &[], None).unwrap_err(), ArchError::NeedClosedChain);
    assert_eq!(Room::from_chain("", "", 0.0, 0, &w, &[], None).unwrap_err(), ArchError::BadName(String::new()));
    assert_eq!(Room::from_chain("x", "", -1.0, 0, &w, &[], None).unwrap_err(), ArchError::BadOccupancy);
    assert_eq!(Room::from_chain("x", "", f64::NAN, 0, &w, &[], None).unwrap_err(), ArchError::NotFinite("occupancy"));
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[], Some(3.5)).unwrap_err(), ArchError::BadHeight);
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[], Some(0.0)).unwrap_err(), ArchError::BadHeight);
    // walls so thick that nothing is left inside
    let fat = rect(2.0, 2.0, 2.0, 3.0);
    assert!(Room::from_chain("x", "", 0.0, 0, &fat, &[], None).is_err());
    // a core poking through the wall, outside the room, touching the wall, and two cores overlapping
    let c = rect(1.0, 1.0, 0.2, 3.0);
    for (i, core) in [at(&c, 7.8, 2.0), at(&c, 20.0, 20.0), at(&c, 7.5, 2.0)].iter().enumerate() {
        assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[core], None).unwrap_err(), ArchError::BadHole(0), "case {i}");
    }
    let (a, b) = (at(&c, 2.0, 2.0), at(&c, 2.5, 2.0));
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[&a, &b], None).unwrap_err(), ArchError::BadHole(1));
    // a core inside another core's footprint
    let big = at(&rect(3.0, 3.0, 0.2, 3.0), 2.0, 1.0);
    let small = at(&c, 3.0, 2.0);
    assert!(Room::from_chain("x", "", 0.0, 0, &w, &[&big, &small], None).is_err());
}

#[test]
fn levels_are_sorted_and_checked() {
    let lv = Levels::new(vec![Level { name: "L2".into(), elevation: 7.0 }, Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.5 }]).unwrap();
    assert_eq!(lv.all().iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["L0", "L1", "L2"]);
    assert_eq!(lv.index_of("L1"), Some(1));
    assert_eq!(lv.floor_to_floor(0).unwrap(), Some(3.5));
    assert_eq!(lv.floor_to_floor(2).unwrap(), None);
    assert_eq!(lv.floor_to_floor(3).unwrap_err(), ArchError::UnknownLevel(3));
    let e = Levels::new(vec![Level { name: "A".into(), elevation: 0.0 }, Level { name: "B".into(), elevation: 0.05 }]).unwrap_err();
    assert!(matches!(e, ArchError::LevelsTooClose { .. }));
    let e = Levels::new(vec![Level { name: "A".into(), elevation: 0.0 }, Level { name: "A".into(), elevation: 3.0 }]).unwrap_err();
    assert_eq!(e, ArchError::DuplicateName("A".into()));
    assert!(Levels::new(vec![Level { name: "A".into(), elevation: f64::INFINITY }]).is_err());
    assert!(Levels::new(vec![Level { name: "bad\nname".into(), elevation: 0.0 }]).is_err());
    // exactly the minimum gap is allowed
    assert!(Levels::new(vec![Level { name: "A".into(), elevation: 0.0 }, Level { name: "B".into(), elevation: MIN_LEVEL_GAP }]).is_ok());
}

#[test]
fn grids_cross_where_the_hand_calculation_says() {
    let g = |n: &str, a: [f64; 2], b: [f64; 2]| Grid { name: n.into(), a, b };
    let (g1, g2, g3) = (g("1", [0.0, 0.0], [0.0, 10.0]), g("A", [-5.0, 3.0], [20.0, 3.0]), g("B", [2.0, 0.0], [2.0, 10.0]));
    check_grids(&[g1.clone(), g2.clone(), g3.clone()]).unwrap();
    assert_eq!(grid_crossing(&g1, &g2), Some([0.0, 3.0]));
    assert_eq!(grid_crossing(&g1, &g3), None);
    // diagonal: y = x and y = 4 - x meet at (2, 2)
    let d1 = g("D1", [0.0, 0.0], [1.0, 1.0]); let d2 = g("D2", [4.0, 0.0], [3.0, 1.0]);
    let p = grid_crossing(&d1, &d2).unwrap();
    assert!(close(p[0], 2.0) && close(p[1], 2.0));
    assert_eq!(check_grids(&[g1.clone(), g1.clone()]).unwrap_err(), ArchError::DuplicateName("1".into()));
    assert_eq!(check_grids(&[g("Z", [1.0, 1.0], [1.0, 1.0])]).unwrap_err(), ArchError::ZeroLengthGrid("Z".into()));
    assert!(check_grids(&[g("N", [f64::NAN, 0.0], [1.0, 1.0])]).is_err());
}

proptest! {
    #[test]
    fn a_rectangular_room_shrinks_by_the_wall_thickness_whatever_the_position_and_turn(w in 1.0f64..30.0, d in 1.0f64..30.0, t in 0.05f64..0.6, dx in -100.0f64..100.0, dy in -100.0f64..100.0, rev in any::<bool>(), h in 2.0f64..6.0) {
        prop_assume!(t < w.min(d) * 0.9);
        let mut c = at(&rect(w, d, t, h), dx, dy);
        if rev { c.points.reverse(); }
        let r = Room::from_chain("r", "", 0.0, 0, &c, &[], None).unwrap();
        let want = (w - t) * (d - t);
        prop_assert!((r.area() - want).abs() < 1e-8 * (1.0 + want));
        prop_assert!((r.volume() - want * h).abs() < 1e-7 * (1.0 + want * h));
        prop_assert!(r.contains([dx + w / 2.0, dy + d / 2.0]));
    }
}

#[test]
fn limits_and_edges() {
    let w = rect(8.0, 5.0, 0.2, 3.0);
    // names: 128 characters are fine, 129 are not
    let long = "n".repeat(128);
    assert!(Room::from_chain(&long, "", 0.0, 0, &w, &[], None).is_ok());
    assert!(Room::from_chain(&format!("{long}n"), "", 0.0, 0, &w, &[], None).is_err());
    assert!(Room::from_chain("x", &"u".repeat(128), 0.0, 0, &w, &[], None).is_ok());
    assert!(Room::from_chain("x", &"u".repeat(129), 0.0, 0, &w, &[], None).is_err());
    assert!(Room::from_chain("x", "bad\tusage", 0.0, 0, &w, &[], None).is_err());
    // occupancy: both ends allowed, nothing beyond
    assert!(Room::from_chain("x", "", MAX_OCCUPANCY, 0, &w, &[], None).is_ok());
    assert!(Room::from_chain("x", "", MAX_OCCUPANCY + 1.0, 0, &w, &[], None).is_err());
    // an empty room has no area per person
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[], None).unwrap().area_per_person(), None);
    // floor to floor works above the lowest level too
    let lv = Levels::new(vec![Level { name: "a".into(), elevation: 1.0 }, Level { name: "b".into(), elevation: 4.0 }, Level { name: "c".into(), elevation: 9.0 }]).unwrap();
    assert_eq!(lv.floor_to_floor(0).unwrap(), Some(3.0));
    assert_eq!(lv.floor_to_floor(1).unwrap(), Some(5.0));
}

#[test]
fn perimeter_counts_the_inner_walls_too() {
    let w = rect(8.0, 5.0, 0.2, 3.0);
    let core = at(&rect(1.0, 1.0, 0.2, 3.0), 3.0, 2.0);
    let r = Room::from_chain("x", "", 0.0, 0, &w, &[&core], None).unwrap();
    assert!(close(r.perimeter(), 2.0 * (7.8 + 4.8) + 4.0 * 1.2));
}

#[test]
fn cores_touching_the_wall_or_each_other_from_inside_are_refused() {
    let w = rect(8.0, 5.0, 0.25, 3.0);
    let c = rect(1.0, 1.0, 0.25, 3.0); // outer faces 1.25 square (all numbers exact in binary)
    // outer face lands exactly on the room face x = 7.875
    let touching = at(&c, 6.75, 2.0);
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[&touching], None).unwrap_err(), ArchError::BadHole(0));
    // two cores whose faces touch exactly (1.25 apart centre to centre)
    let (a, b) = (at(&c, 2.0, 2.0), at(&c, 3.25, 2.0));
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[&a, &b], None).unwrap_err(), ArchError::BadHole(1));
    // small core listed before the big one that contains it
    let big = at(&rect(3.0, 3.0, 0.25, 3.0), 2.0, 1.0);
    let small = at(&c, 3.0, 2.0);
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[&small, &big], None).unwrap_err(), ArchError::BadHole(1));
    // a gap of 1 cm is fine
    let b2 = at(&c, 3.26, 2.0);
    assert!(Room::from_chain("x", "", 0.0, 0, &w, &[&a, &b2], None).is_ok(), "{:?}", Room::from_chain("x", "", 0.0, 0, &w, &[&a, &b2], None));
}

#[test]
fn grid_thresholds() {
    let g = |n: &str, a: [f64; 2], b: [f64; 2]| Grid { name: n.into(), a, b };
    assert_eq!(check_grids(&[g("s", [0.0, 0.0], [1e-7, 0.0])]).unwrap_err(), ArchError::ZeroLengthGrid("s".into()));
    assert!(check_grids(&[g("s", [0.0, 0.0], [1e-5, 0.0])]).is_ok());
    // practically parallel lines do not cross; a clear angle does
    let h = g("h", [0.0, 0.0], [1.0, 0.0]);
    assert_eq!(grid_crossing(&h, &g("p", [0.0, 1.0], [1.0, 1.0 + 1e-14])), None);
    assert!(grid_crossing(&h, &g("q", [0.5, -1.0], [0.5, 1.0])).is_some());
}

#[test]
fn cores_meeting_corner_to_corner_are_refused() {
    let w = rect(8.0, 5.0, 0.25, 3.0);
    let c = rect(1.0, 1.0, 0.25, 3.0);
    // outer faces 1.875..3.125 and 3.125..4.375 in both directions: they share only the corner point,
    // and their edges lie on the same lines (end to end)
    let (a, b) = (at(&c, 2.0, 2.0), at(&c, 3.25, 3.25));
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &w, &[&a, &b], None).unwrap_err(), ArchError::BadHole(1));
}

#[test]
fn a_thin_inner_chain_may_not_cut_across_a_corner_of_the_room() {
    // L-shaped room: bottom strip x 0.2..5.8, y 0.2..1.8; upper block x 0.2..2.8, y 1.8..3.8
    let l = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [3.0, 2.0], [3.0, 4.0], [0.0, 4.0]], true, 0.4, 3.0);
    // a thin diagonal chain from the upper block to the strip: all its corners are inside the room, but it runs
    // through the missing corner (x > 2.8, y > 1.8) on the way
    let thin = WallChain::new(vec![[2.5, 3.5], [2.6, 3.5], [5.1, 1.0], [5.0, 1.0]], true, 0.02, 3.0);
    assert_eq!(Room::from_chain("x", "", 0.0, 0, &l, &[&thin], None).unwrap_err(), ArchError::BadHole(0));
}
