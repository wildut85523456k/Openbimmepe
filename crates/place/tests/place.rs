use openbim_family::library::{self, door};
use openbim_geom::*;
use openbim_place::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn pl(family: usize, chain: usize, segment: usize, centre: f64) -> Placement {
    Placement { family, type_name: None, overrides: vec![], texts: vec![], chain, segment, centre }
}
fn model(chains: Vec<WallChain>, placements: Vec<Placement>) -> Model {
    Model { chains, families: library_families(), placements }
}
fn straight(len: f64, t: f64, h: f64) -> WallChain { WallChain::new(vec![[0.0, 0.0], [len, 0.0]], false, t, h) }
fn room(w: f64, d: f64, t: f64, h: f64) -> WallChain { WallChain::new(vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], true, t, h) }

#[test]
fn a_door_cuts_exactly_its_opening() {
    let m = model(vec![straight(10.0, 0.2, 3.0)], vec![pl(0, 0, 0, 3.0)]);
    let b = m.build().unwrap();
    assert!(close(b.volume, 10.0 * 0.2 * 3.0 - 0.9 * 2.1 * 0.2), "{}", b.volume);
    let o = b.chains[0].openings[0][0];
    assert!(close(o.offset, 3.0 - 0.45) && close(o.width, 0.9) && close(o.height, 2.1) && o.sill == 0.0);
    let mesh = b.chains[0].chain_mesh().unwrap();
    assert!(mesh.is_closed_manifold() && close(mesh.volume(), b.volume));
}

#[test]
fn a_window_cuts_at_its_sill_and_the_family_takes_the_wall_thickness() {
    let mut w = pl(1, 0, 0, 5.0);
    w.overrides = vec![("sill".into(), 1.0), ("mullions".into(), 2.0)];
    let m = model(vec![straight(10.0, 0.3, 3.0)], vec![w]);
    let b = m.build().unwrap();
    assert!(close(b.volume, 10.0 * 0.3 * 3.0 - 1.2 * 1.2 * 0.3));
    let o = b.chains[0].openings[0][0];
    assert!(close(o.sill, 1.0));
    // the frame in the family spans the host thickness
    let jamb = b.instances[0].solids.iter().find(|(s, _)| m.families[1].solid_name(*s) == "jamb_left").unwrap().1;
    assert!(close(jamb.min[1], -0.15) && close(jamb.max[1], 0.15));
}

#[test]
fn placed_parts_sit_exactly_in_the_opening_on_a_diagonal_wall() {
    // a 3-4-5 wall: length 10 at an angle, with a window whose parts are mapped into world space
    let c = WallChain::new(vec![[2.0, 1.0], [8.0, 9.0]], false, 0.25, 3.0);
    let mut w = pl(1, 0, 0, 4.0);
    w.overrides = vec![("sill".into(), 0.8), ("mullions".into(), 3.0)];
    let m = model(vec![c], vec![w]);
    let b = m.build().unwrap();
    let (u, nl) = ([0.6, 0.8], [-0.8, 0.6]);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for bx in m.world_boxes(&b, 0) {
        for corner in 0..8u32 {
            let (dx, dy, dz) = ((corner & 1) as f64 * bx.size[0], ((corner >> 1) & 1) as f64 * bx.size[1], ((corner >> 2) & 1) as f64 * bx.size[2]);
            let p = [bx.origin[0] + bx.ax[0] * dx + bx.ay[0] * dy, bx.origin[1] + bx.ax[1] * dx + bx.ay[1] * dy];
            // back into the wall frame
            let rel = [p[0] - 2.0, p[1] - 1.0];
            let f = [rel[0] * u[0] + rel[1] * u[1], rel[0] * nl[0] + rel[1] * nl[1], bx.origin[2] + dz];
            for k in 0..3 { lo[k] = lo[k].min(f[k]); hi[k] = hi[k].max(f[k]); }
        }
    }
    let o = b.chains[0].openings[0][0];
    assert!(close(lo[0], o.offset) && close(hi[0], o.offset + o.width), "{lo:?} {hi:?} {o:?}");
    assert!(close(lo[2], o.sill) && close(hi[2], o.sill + o.height));
    assert!(close(lo[1], -0.125) && close(hi[1], 0.125));
}

#[test]
fn placement_order_does_not_change_the_wall() {
    let ps = vec![pl(0, 0, 0, 1.5), pl(1, 0, 0, 3.5), pl(0, 0, 2, 2.0), pl(1, 0, 1, 2.0)];
    let a = model(vec![room(6.0, 4.0, 0.2, 3.0)], ps.clone()).build().unwrap();
    let mut rev = ps; rev.reverse();
    let b = model(vec![room(6.0, 4.0, 0.2, 3.0)], rev).build().unwrap();
    assert!(close(a.volume, b.volume));
    assert!(close(a.volume, 20.0 * 0.2 * 3.0 - 2.0 * 0.9 * 2.1 * 0.2 - 2.0 * 1.2 * 1.2 * 0.2));
}

#[test]
fn resizing_a_door_updates_the_wall_and_a_bad_edit_changes_nothing() {
    let mut m = model(vec![straight(6.0, 0.2, 3.0)], vec![pl(0, 0, 0, 3.0)]);
    let before = m.build().unwrap().volume;
    // Type B: 1.2 x 2.4
    let mut p = m.placements[0].clone(); p.type_name = Some("Type B".into());
    let after = m.try_update(0, p).unwrap().volume;
    assert!(close(after - before, 0.9 * 2.1 * 0.2 - 1.2 * 2.4 * 0.2));
    // moving it into the wall end is refused and the model stays as it was
    let snapshot = m.placements.clone();
    let mut bad = m.placements[0].clone(); bad.centre = 0.3;
    assert!(matches!(m.try_update(0, bad), Err(PlaceError::Wall { placement: 0, .. })));
    assert_eq!(m.placements, snapshot);
    assert!(close(m.build().unwrap().volume, after));
    // a type that does not exist is refused the same way
    let mut bad = m.placements[0].clone(); bad.type_name = Some("Nope".into());
    assert!(matches!(m.try_update(0, bad), Err(PlaceError::Family { placement: 0, .. })));
    assert_eq!(m.placements, snapshot);
}

#[test]
fn bad_placements_are_refused_with_the_culprit_named() {
    let walls = || vec![room(6.0, 4.0, 0.2, 3.0)];
    // too close to the mitred end (usable range starts at t/2 = 0.1 m)
    assert!(matches!(model(walls(), vec![pl(0, 0, 0, 0.5)]).build(), Err(PlaceError::Wall { placement: 0, err: GeomError::OpeningOutside { .. } })));
    // two doors on top of each other: the later one is blamed
    assert!(matches!(model(walls(), vec![pl(0, 0, 0, 2.0), pl(0, 0, 0, 2.3)]).build(), Err(PlaceError::Wall { placement: 1, err: GeomError::OpeningsOverlap { .. } })));
    // a window that would stick out of the top of the wall
    let mut w = pl(1, 0, 0, 3.0); w.overrides = vec![("sill".into(), 2.0)];
    assert!(matches!(model(walls(), vec![w]).build(), Err(PlaceError::Wall { placement: 0, .. })));
    // walls the family cannot be built in
    assert!(matches!(model(vec![straight(6.0, 1.5, 3.0)], vec![pl(0, 0, 0, 3.0)]).build(), Err(PlaceError::Family { placement: 0, .. })));
    assert!(matches!(model(vec![straight(6.0, 0.02, 3.0)], vec![pl(0, 0, 0, 3.0)]).build(), Err(PlaceError::Family { placement: 0, .. })));
    // references that do not exist, and numbers that are not numbers
    assert!(matches!(model(walls(), vec![pl(5, 0, 0, 3.0)]).build(), Err(PlaceError::BadReference { .. })));
    assert!(matches!(model(walls(), vec![pl(0, 9, 0, 3.0)]).build(), Err(PlaceError::BadReference { .. })));
    assert!(matches!(model(walls(), vec![pl(0, 0, 9, 3.0)]).build(), Err(PlaceError::BadReference { .. })));
    assert!(matches!(model(walls(), vec![pl(0, 0, 0, f64::NAN)]).build(), Err(PlaceError::NotFinite { .. })));
    assert!(matches!(model(walls(), vec![pl(0, 0, 0, f64::INFINITY)]).build(), Err(PlaceError::NotFinite { .. })));
    // a broken wall chain is reported, not cut
    let bad = WallChain::new(vec![[0.0, 0.0], [0.0, 0.0]], false, 0.2, 3.0);
    assert!(matches!(model(vec![bad], vec![]).build(), Err(PlaceError::Chain { chain: 0, .. })));
    // an empty model builds
    assert_eq!(model(vec![], vec![]).build().unwrap().volume, 0.0);
}

#[test]
fn a_family_without_an_opening_does_not_cut() {
    let mut f = door();
    f.opening = None;
    let m = Model { chains: vec![straight(6.0, 0.2, 3.0)], families: openbim_family::compile_library(&[library::handle(), f]).unwrap().split_off(1), placements: vec![pl(0, 0, 0, 3.0)] };
    assert!(close(m.build().unwrap().volume, 6.0 * 0.2 * 3.0));
}

#[test]
fn existing_wall_openings_are_kept() {
    let mut c = straight(10.0, 0.2, 3.0);
    c.openings = vec![vec![Opening { offset: 7.0, width: 1.0, sill: 0.0, height: 2.0 }]];
    let b = model(vec![c], vec![pl(0, 0, 0, 3.0)]).build().unwrap();
    assert!(close(b.volume, 6.0 - 1.0 * 2.0 * 0.2 - 0.9 * 2.1 * 0.2));
    // and a family opening that lands on a hand-made one is refused
    let mut c = straight(10.0, 0.2, 3.0);
    c.openings = vec![vec![Opening { offset: 2.8, width: 1.0, sill: 0.0, height: 2.0 }]];
    assert!(matches!(model(vec![c], vec![pl(0, 0, 0, 3.0)]).build(), Err(PlaceError::Wall { placement: 0, .. })));
}

#[test]
fn many_rooms_build_identically_on_any_thread_count() {
    let mut chains = Vec::new(); let mut ps = Vec::new();
    for k in 0..2000usize {
        chains.push(WallChain { base_z: 0.0, ..room(5.0 + (k % 7) as f64 * 0.3, 4.0, 0.2, 3.0) });
        ps.push(pl(0, k, 0, 1.2 + (k % 5) as f64 * 0.1));
        ps.push(Placement { overrides: vec![("mullions".into(), (k % 4) as f64)], ..pl(1, k, 2, 2.0) });
        ps.push(Placement { type_name: Some("Type B".into()), ..pl(0, k, 1, 2.0) });
    }
    let m = model(chains, ps);
    let reference = m.build().unwrap();
    for threads in [1usize, 2, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let b = pool.install(|| m.build()).unwrap();
        assert_eq!(b.volume.to_bits(), reference.volume.to_bits(), "threads {threads}");
        assert_eq!(b.instances, reference.instances);
    }
    // the mesh of every wall is closed and has the same volume
    for c in reference.chains.iter().take(50) { assert!(c.chain_mesh().unwrap().is_closed_manifold()); }
}

proptest! {
    #[test]
    fn random_placements_in_a_room_follow_the_rules(
        t in 0.1f64..0.4, h in 2.6f64..3.6,
        specs in proptest::collection::vec((0usize..2, 0usize..4, -0.5f64..6.5, 0.0f64..2.0, 0u32..5, any::<bool>()), 0..7)) {
        let (w, d) = (6.0, 4.0);
        let seg_len = [w, d, w, d];
        let ps: Vec<Placement> = specs.iter().map(|&(fam, seg, c, sill, mull, typeb)| {
            let mut p = pl(fam, 0, seg, c);
            if fam == 1 { p.overrides = vec![("sill".into(), sill), ("mullions".into(), mull as f64)]; }
            if fam == 0 && typeb { p.type_name = Some("Type B".into()); }
            p
        }).collect();
        let m = model(vec![room(w, d, t, h)], ps);
        let r = m.build();
        // the openings the families produce
        let ops: Vec<Option<[f64; 3]>> = (0..m.placements.len()).map(|k| {
            let p = &m.placements[k];
            let mut ov: Vec<(&str, f64)> = p.overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect();
            ov.push(("wall_t", t));
            m.families[p.family].solve(&openbim_family::Request { type_name: p.type_name.as_deref(), overrides: ov, ..Default::default() }).ok().and_then(|i| i.opening)
        }).collect();
        let mut definitely_bad = false; let mut s_overlap = false;
        for k in 0..ops.len() {
            let Some([ow, oh, sill]) = ops[k] else { definitely_bad = true; continue };
            let p = &m.placements[k];
            let (s0, s1) = (p.centre - ow / 2.0, p.centre + ow / 2.0);
            if s0 < t / 2.0 + 1e-6 || s1 > seg_len[p.segment] - t / 2.0 - 1e-6 || sill + oh > h - 1e-6 { definitely_bad = true; }
            for (j, oj) in ops.iter().enumerate().take(k) {
                if let Some([jw, _, _]) = *oj {
                    let q = &m.placements[j];
                    if q.segment == p.segment && (q.centre - p.centre).abs() < (jw + ow) / 2.0 + 1e-9 { s_overlap = true; }
                }
            }
        }
        if definitely_bad { prop_assert!(r.is_err(), "{:?}", r.map(|b| b.volume)); }
        else if !s_overlap { prop_assert!(r.is_ok(), "{:?}", r.map(|b| b.volume)); }
        if let Ok(b) = r {
            let cut: f64 = ops.iter().flatten().map(|[ow, oh, _]| ow * oh * t).sum();
            prop_assert!(close(b.volume, 2.0 * (w + d) * t * h - cut), "{} vs {}", b.volume, 2.0 * (w + d) * t * h - cut);
            let mesh = b.chains[0].chain_mesh().unwrap();
            prop_assert!(mesh.is_closed_manifold() && close(mesh.volume(), b.volume));
        }
    }
}

#[test]
fn walls_on_an_upper_level_place_their_parts_at_that_level() {
    let mut c = straight(8.0, 0.2, 3.0);
    c.base_z = 3.0;
    let mut w = pl(1, 0, 0, 4.0);
    w.overrides = vec![("sill".into(), 0.8)];
    let m = model(vec![c], vec![w]);
    let b = m.build().unwrap();
    let boxes = m.world_boxes(&b, 0);
    let lo = boxes.iter().map(|x| x.origin[2]).fold(f64::MAX, f64::min);
    let hi = boxes.iter().map(|x| x.origin[2] + x.size[2]).fold(f64::MIN, f64::max);
    assert!(close(lo, 3.8) && close(hi, 3.8 + 1.2), "{lo} {hi}");
    assert!(close(b.chains[0].openings[0][0].sill, 0.8), "the cut is measured from the wall base");
}

#[test]
fn updating_a_placement_that_does_not_exist_is_refused() {
    let mut m = model(vec![straight(6.0, 0.2, 3.0)], vec![pl(0, 0, 0, 3.0)]);
    let snapshot = m.placements.clone();
    assert!(matches!(m.try_update(1, pl(0, 0, 0, 2.0)), Err(PlaceError::BadReference { .. })));
    assert!(matches!(m.try_update(usize::MAX, pl(0, 0, 0, 2.0)), Err(PlaceError::BadReference { .. })));
    assert_eq!(m.placements, snapshot);
}

/// door and window, compiled together with the families they contain (the handle)
fn library_families() -> Vec<openbim_family::Compiled> {
    let mut v = library::all().unwrap();
    v.remove(0);
    v
}

#[test]
fn texts_reach_the_family_and_the_handle_follows_the_hinge_side() {
    let mut m = model(vec![straight(6.0, 0.2, 3.0)], vec![pl(0, 0, 0, 3.0)]);
    let handle_x = |m: &Model, b: &Built| {
        let w = m.world_boxes(b, 0).into_iter().find(|w| m.families[0].solid_name(w.solid) == "handle/plate_front").unwrap();
        w.origin[0] + w.size[0] / 2.0          // wall runs along +x from the origin
    };
    let b = m.build().unwrap();
    assert!(close(handle_x(&m, &b), 3.0 - 0.33), "{}", handle_x(&m, &b));     // right-hinged: handle at the low-x edge of the leaf
    m.placements[0].texts = vec![("swing".into(), "left".into()), ("mark".into(), "D-07".into())];
    let b = m.build().unwrap();
    assert!(close(handle_x(&m, &b), 3.0 + 0.33), "{}", handle_x(&m, &b));
    assert_eq!(m.families[0].text(&b.instances[0], "mark"), Some("D-07"));
    // a bad text names the placement, and try_update leaves the model as it was
    let bad = Placement { texts: vec![("swing".into(), "up".into())], ..pl(0, 0, 0, 3.0) };
    let before = m.placements.clone();
    match m.try_update(0, bad) { Err(PlaceError::Family { placement: 0, err: openbim_family::FamilyError::BadChoice { .. } }) => {}, o => panic!("{:?}", o.map(|b| b.volume)) }
    assert_eq!(m.placements, before);
}

fn world_corners(w: &WorldBox) -> Vec<[f64; 2]> {
    let c = |i: f64, j: f64| [w.origin[0] + w.ax[0] * w.size[0] * i + w.ay[0] * w.size[1] * j, w.origin[1] + w.ax[1] * w.size[0] * i + w.ay[1] * w.size[1] * j];
    vec![c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0), c(0.0, 1.0)]
}

#[test]
fn an_open_door_leaf_is_placed_turned_in_the_world() {
    // leaf of the default door, hinged right, open 90 degrees, door centre at 3.0 on the segment:
    // in the door frame it fills x 0.38..0.42, y 0..0.8 (hand calculation in the family tests)
    let open = |chain: WallChain| {
        let mut m = model(vec![chain], vec![pl(0, 0, 0, 3.0)]);
        m.placements[0].overrides = vec![("open_angle".into(), 90.0)];
        let b = m.build().unwrap();
        let leaf = m.world_boxes(&b, 0).into_iter().find(|w| m.families[0].solid_name(w.solid) == "leaf").unwrap();
        world_corners(&leaf)
    };
    let has = |cs: &[[f64; 2]], x: f64, y: f64| cs.iter().any(|c| close(c[0], x) && close(c[1], y));
    // wall along +x from the origin: world = (3 + x, y)
    let cs = open(straight(6.0, 0.2, 3.0));
    for (x, y) in [(3.38, 0.0), (3.42, 0.0), (3.38, 0.8), (3.42, 0.8)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    // wall along +y: u = (0,1), left normal = (-1,0): world = (-y, 3 + x)
    let cs = open(WallChain::new(vec![[0.0, 0.0], [0.0, 6.0]], false, 0.2, 3.0));
    for (x, y) in [(0.0, 3.38), (0.0, 3.42), (-0.8, 3.38), (-0.8, 3.42)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    // the opening cut does not depend on the leaf being open
    let mut m = model(vec![straight(6.0, 0.2, 3.0)], vec![pl(0, 0, 0, 3.0)]);
    let closed = m.build().unwrap().volume;
    m.placements[0].overrides = vec![("open_angle".into(), 90.0)];
    assert_eq!(m.build().unwrap().volume, closed);
}
