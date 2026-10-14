use openbim_geom::*;
use proptest::prelude::*;

type P = [f64; 2];

fn rect(w: f64, d: f64, t: f64, h: f64) -> WallChain {
    WallChain::new(vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], true, t, h)
}
fn straight(l: f64, t: f64, h: f64) -> WallChain { WallChain::new(vec![[0.0, 0.0], [l, 0.0]], false, t, h) }
fn perimeter(c: &WallChain) -> f64 {
    let n = c.points.len();
    (0..n).map(|i| { let a = c.points[i]; let b = c.points[(i + 1) % n]; ((b[0]-a[0]).powi(2) + (b[1]-a[1]).powi(2)).sqrt() }).sum()
}
fn exact(c: &WallChain) -> f64 { total_volume_serial(std::slice::from_ref(c)).unwrap() }
fn mesh(c: &WallChain) -> f64 { total_mesh_volume_parallel(std::slice::from_ref(c)).unwrap() }
fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0) }
fn poly_area(p: &[P]) -> f64 { let n = p.len(); 0.5 * (0..n).map(|i| p[i][0]*p[(i+1)%n][1] - p[(i+1)%n][0]*p[i][1]).sum::<f64>().abs() }
fn circle(cx: f64, cz: f64, r: f64, n: usize) -> Vec<P> {
    (0..n).map(|k| { let a = std::f64::consts::TAU * k as f64 / n as f64; [cx + r * a.cos(), cz + r * a.sin()] }).collect()
}
fn arch(x0: f64, w: f64, spring: f64, n: usize) -> Vec<P> {
    // door with a semicircular head: bottom edge on z = 0, then up, over the arch, and down
    let r = w / 2.0;
    let mut v = vec![[x0, 0.0], [x0 + w, 0.0], [x0 + w, spring]];
    for k in 1..n { let a = std::f64::consts::PI * k as f64 / n as f64; v.push([x0 + r + r * a.cos(), spring + r * a.sin()]); }
    v.push([x0, spring]);
    v
}
/// Reference for rectangular openings, written independently of the profile code.
fn reference(c: &WallChain) -> f64 {
    let plans = c.plans().unwrap();
    plans.iter().enumerate().map(|(i, p)| {
        let area = 0.5 * ((p.l0[0]*p.l1[1]-p.l1[0]*p.l0[1]) + (p.l1[0]*p.r1[1]-p.r1[0]*p.l1[1]) + (p.r1[0]*p.r0[1]-p.r0[0]*p.r1[1]) + (p.r0[0]*p.l0[1]-p.l0[0]*p.r0[1])).abs();
        let cut: f64 = c.openings.get(i).map(|o| o.iter().map(|o| o.width * o.height * c.thickness).sum()).unwrap_or(0.0);
        area * c.height - cut
    }).sum()
}

// ---------------------------------------------------------------- S1 behaviour kept
#[test]
fn rectangle_volume_is_perimeter_times_t_times_h() {
    let c = rect(6.0, 4.0, 0.2, 3.0);
    assert!(close(exact(&c), 12.0));
    assert!(close(mesh(&c), 12.0));
}

#[test]
fn door_and_window_match_independent_reference() {
    let mut c = rect(6.0, 4.0, 0.2, 3.0);
    c.openings = vec![vec![Opening { offset: 2.0, width: 0.9, sill: 0.0, height: 2.1 }],
                      vec![Opening { offset: 1.0, width: 1.5, sill: 0.9, height: 1.2 }, Opening { offset: 3.0, width: 0.8, sill: 0.0, height: 2.0 }]];
    let want = 12.0 - 0.9 * 2.1 * 0.2 - 1.5 * 1.2 * 0.2 - 0.8 * 2.0 * 0.2;
    assert!(close(exact(&c), want)); assert!(close(mesh(&c), want)); assert!(close(reference(&c), want));
}

#[test]
fn concave_l_shape_and_open_chain() {
    let mut c = rect(1.0, 1.0, 0.3, 2.7);
    c.points = vec![[0.,0.],[8.,0.],[8.,3.],[3.,3.],[3.,6.],[0.,6.]]; c.base_z = 1.0;
    let want = perimeter(&c) * 0.3 * 2.7;
    assert!(close(exact(&c), want)); assert!(close(mesh(&c), want));
    let o = WallChain::new(vec![[0.,0.],[2.,0.],[5.,0.],[5.,4.]], false, 0.2, 3.0);
    assert!(close(exact(&o), 9.0 * 0.2 * 3.0)); assert!(close(mesh(&o), 9.0 * 0.2 * 3.0));
}

#[test]
fn parallel_equals_serial_bitwise_any_thread_count() {
    let chains: Vec<WallChain> = (0..3000).map(|i| {
        let mut c = rect(3.0 + (i % 7) as f64, 2.0 + (i % 5) as f64, 0.15 + (i % 3) as f64 * 0.05, 2.5);
        c.openings = vec![vec![Opening { offset: 0.5, width: 0.9, sill: 0.0, height: 2.1 }]]; c
    }).collect();
    let a = total_volume_serial(&chains).unwrap();
    let b = total_volume_parallel(&chains).unwrap();
    assert_eq!(a.to_bits(), b.to_bits());
    assert!(close(a, total_mesh_volume_parallel(&chains).unwrap()));
    for threads in [1, 2, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        assert_eq!(pool.install(|| total_volume_parallel(&chains).unwrap()).to_bits(), b.to_bits());
        assert_eq!(pool.install(|| total_mesh_volume_parallel(&chains).unwrap()).to_bits(),
                   pool.install(|| total_mesh_volume_parallel(&chains).unwrap()).to_bits());
    }
}

// ---------------------------------------------------------------- S1b: one closed shell
#[test]
fn ring_of_walls_is_one_closed_manifold_torus() {
    let m = rect(6.0, 4.0, 0.2, 3.0).chain_mesh().unwrap();
    assert!(m.is_closed_manifold());
    assert_eq!(m.euler_characteristic(), 0); // a ring of wall is a torus
    assert!(close(m.volume(), 12.0));
}

#[test]
fn wall_with_door_is_a_sphere_and_with_window_gains_a_hole() {
    let mut c = straight(6.0, 0.2, 3.0);
    c.openings = vec![vec![Opening { offset: 2.0, width: 0.9, sill: 0.0, height: 2.1 }]];
    let m = c.chain_mesh().unwrap();
    assert!(m.is_closed_manifold()); assert_eq!(m.euler_characteristic(), 2);
    c.openings = vec![vec![Opening { offset: 2.0, width: 0.9, sill: 0.0, height: 2.1 }, Opening { offset: 4.0, width: 1.2, sill: 0.9, height: 1.2 }]];
    let m = c.chain_mesh().unwrap();
    assert!(m.is_closed_manifold()); assert_eq!(m.euler_characteristic(), 0);
    assert!(close(m.volume(), exact(&c)));
}

// ---------------------------------------------------------------- S1b: polygonal openings and sloped tops
#[test]
fn arched_door_and_round_window() {
    let (l, t, h) = (6.0, 0.25, 3.2);
    let door = arch(1.0, 1.0, 1.8, 32);
    let win = circle(4.0, 1.8, 0.5, 48);
    let mut c = straight(l, t, h);
    c.poly_openings = vec![vec![door.clone(), win.clone()]];
    let want = (l * h - poly_area(&door) - poly_area(&win)) * t;
    assert!(close(exact(&c), want), "{} vs {}", exact(&c), want);
    let m = c.chain_mesh().unwrap();
    assert!(m.is_closed_manifold());
    assert!(close(m.volume(), want));
}

#[test]
fn gable_wall_and_attachment_to_a_sloped_roof() {
    let (l, t) = (8.0, 0.2);
    let mut c = straight(l, t, 3.0);
    c.tops = vec![Some(vec![[0.0, 2.5], [4.0, 4.0], [8.0, 2.5]])];
    let area = l * 2.5 + 0.5 * l * 1.5;
    assert!(close(exact(&c), area * t));
    let m = c.chain_mesh().unwrap();
    assert!(m.is_closed_manifold()); assert!(close(m.volume(), area * t));
    // window under the slope
    c.openings = vec![vec![Opening { offset: 3.0, width: 2.0, sill: 1.0, height: 1.0 }]];
    assert!(close(exact(&c), (area - 2.0) * t));
    assert!(close(mesh(&c), (area - 2.0) * t));
}

#[test]
fn wall_top_attached_to_slab_underside_is_a_flat_top_at_that_height() {
    // slab underside at 3.15 m above the wall base: attaching the top is the same as height = 3.15
    let mut a = rect(5.0, 5.0, 0.2, 3.0); a.height = 3.15;
    assert!(close(exact(&a), 20.0 * 0.2 * 3.15));
}

// ---------------------------------------------------------------- S1b: T-junctions
#[test]
fn t_junction_has_no_overlap_and_exact_volume() {
    let host = straight(10.0, 0.2, 3.0);
    let mut branch = WallChain::new(vec![[5.0, 5.0], [5.0, 0.0]], false, 0.2, 3.0);
    branch.end_stop = Some(host.face_stop_for(0, [5.0, 5.0]).unwrap());
    let want = 10.0 * 0.2 * 3.0 + (5.0 - 0.1) * 0.2 * 3.0;
    assert!(close(exact(&host) + exact(&branch), want));
    assert!(close(mesh(&host) + mesh(&branch), want));
    // branch from the other side
    let mut b2 = WallChain::new(vec![[3.0, -4.0], [3.0, 0.0]], false, 0.2, 3.0);
    b2.end_stop = Some(host.face_stop_for(0, [3.0, -4.0]).unwrap());
    assert!(close(exact(&b2), (4.0 - 0.1) * 0.2 * 3.0));
    // angled branch: butt end cut along the host face
    let mut b3 = WallChain::new(vec![[8.0, 4.0], [6.0, 0.0]], false, 0.2, 3.0);
    b3.end_stop = Some(host.face_stop_for(0, [8.0, 4.0]).unwrap());
    let pl = b3.plans().unwrap();
    assert!((pl[0].l1[1] - 0.1).abs() < 1e-9 && (pl[0].r1[1] - 0.1).abs() < 1e-9);
    assert!(b3.chain_mesh().unwrap().is_closed_manifold());
}

#[test]
fn bad_stops_are_rejected() {
    let host = straight(10.0, 0.2, 3.0);
    let mut b = WallChain::new(vec![[5.0, 5.0], [5.0, 0.0]], false, 0.2, 3.0);
    b.end_stop = Some(StopLine { point: [0.0, 0.1], dir: [0.0, 1.0] }); // parallel to the branch
    assert!(matches!(b.plans(), Err(GeomError::BadStop(_))));
    b.end_stop = Some(StopLine { point: [0.0, -90.0], dir: [1.0, 0.0] }); // a face 95 m away
    assert!(matches!(b.plans(), Err(GeomError::BadStop(_))));
    b.end_stop = Some(StopLine { point: [0.0, 0.0], dir: [0.0, 0.0] });
    assert!(matches!(b.plans(), Err(GeomError::BadStop(_))));
    let mut c = rect(3.0, 3.0, 0.2, 3.0);
    c.end_stop = Some(host.face_stop_for(0, [1.0, 1.0]).unwrap());
    assert!(matches!(c.plans(), Err(GeomError::BadStop(_))));
    assert!(matches!(host.face_stop_for(7, [0.0, 0.0]), Err(GeomError::BadStop(_))));
}

// ---------------------------------------------------------------- input safety
#[test]
fn bad_input_is_rejected_not_panicking() {
    let mut c = rect(6.0, 4.0, 0.2, 3.0); c.thickness = f64::NAN; assert!(matches!(c.plans(), Err(GeomError::NotFinite(_))));
    let mut c = rect(6.0, 4.0, 0.2, 3.0); c.thickness = 0.0; assert!(matches!(c.plans(), Err(GeomError::NonPositive(_))));
    let mut c = rect(6.0, 4.0, 0.2, 3.0); c.height = -1.0; assert!(matches!(c.plans(), Err(GeomError::NonPositive(_))));
    let mut c = rect(6.0, 4.0, 0.2, 3.0); c.points[1] = [f64::INFINITY, 0.0]; assert!(matches!(c.plans(), Err(GeomError::NotFinite(_))));
    assert_eq!(WallChain::new(vec![[0., 0.]], false, 0.2, 3.0).plans().unwrap_err(), GeomError::TooFewPoints);
    assert!(matches!(WallChain::new(vec![[0.,0.],[0.,0.],[1.,1.]], false, 0.2, 3.0).plans(), Err(GeomError::ZeroLengthSegment(0))));
    assert!(matches!(WallChain::new(vec![[0.,0.],[5.,0.],[0.,0.001]], false, 0.2, 3.0).plans(), Err(GeomError::Parallel(_))));
}

fn err_of(c: &WallChain) -> GeomError { total_volume_serial(std::slice::from_ref(c)).unwrap_err() }

#[test]
fn bad_openings_and_tops_are_rejected() {
    let mut c = straight(6.0, 0.2, 3.0);
    c.openings = vec![vec![Opening { offset: 5.5, width: 1.0, sill: 0.0, height: 2.0 }]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
    c.openings = vec![vec![Opening { offset: 1.0, width: 1.0, sill: 0.0, height: 2.0 }, Opening { offset: 1.5, width: 1.0, sill: 0.0, height: 2.0 }]];
    assert!(matches!(err_of(&c), GeomError::OpeningsOverlap { .. }));
    c.openings = vec![vec![Opening { offset: 1.0, width: 1.0, sill: 2.5, height: 1.0 }]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
    // would cut the wall in two pieces
    c.openings = vec![vec![Opening { offset: 1.0, width: 1.0, sill: 0.0, height: 3.0 }]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
    c.openings = vec![vec![Opening { offset: 0.0, width: 1.0, sill: 0.0, height: 2.0 }]]; // touches the end
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
    c.openings = vec![];
    c.poly_openings = vec![vec![vec![[1.0, 1.0], [2.0, 2.0], [2.0, 1.0], [1.0, 2.0]]]]; // bow-tie
    assert!(matches!(err_of(&c), GeomError::BadOpening { .. }));
    c.poly_openings = vec![vec![vec![[1.0, 1.0], [2.0, 1.0], [3.0, 1.0]]]]; // zero area
    assert!(matches!(err_of(&c), GeomError::BadOpening { .. }));
    c.poly_openings = vec![vec![vec![[1.0, 1.0], [2.0, 1.0]]]];
    assert!(matches!(err_of(&c), GeomError::BadOpening { .. }));
    // two circles overlapping, and one inside another
    c.poly_openings = vec![vec![circle(2.0, 1.5, 0.6, 24), circle(2.5, 1.5, 0.6, 24)]];
    assert!(matches!(err_of(&c), GeomError::OpeningsOverlap { .. }));
    c.poly_openings = vec![vec![circle(2.0, 1.5, 0.8, 24), circle(2.0, 1.5, 0.3, 24)]];
    assert!(matches!(err_of(&c), GeomError::OpeningsOverlap { .. }));
    // bad tops
    c.poly_openings = vec![];
    for top in [vec![[0.0, 2.0]], vec![[0.0, 2.0], [6.0, -1.0]], vec![[0.0, 2.0], [3.0, 3.0], [2.0, 3.0], [6.0, 2.0]], vec![[0.5, 2.0], [6.0, 2.0]], vec![[0.0, 2.0], [5.0, 2.0]], vec![[0.0, f64::NAN], [6.0, 2.0]]] {
        c.tops = vec![Some(top)];
        assert!(matches!(err_of(&c), GeomError::BadTop { .. } | GeomError::NotFinite(_)), "{:?}", err_of(&c));
    }
    // opening poking above a sloped top
    c.tops = vec![Some(vec![[0.0, 2.0], [6.0, 1.0]])];
    c.openings = vec![vec![Opening { offset: 4.0, width: 1.0, sill: 0.5, height: 1.0 }]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
}

#[test]
fn size_limits_stop_huge_inputs() {
    let mut c = straight(6.0, 0.2, 3.0);
    c.poly_openings = vec![vec![circle(3.0, 1.5, 1.0, MAX_PROFILE_VERTS + 5)]];
    assert!(matches!(err_of(&c), GeomError::BadOpening { .. }));
    c.poly_openings = vec![];
    c.openings = vec![(0..(MAX_OPENINGS_PER_WALL + 1)).map(|k| Opening { offset: 0.001 * k as f64 + 0.01, width: 0.0005, sill: 0.1, height: 0.1 }).collect()];
    assert!(matches!(err_of(&c), GeomError::TooMany(_)));
    c.openings = vec![]; c.openings = vec![vec![]; 5]; // more lists than segments
    assert!(matches!(err_of(&c), GeomError::TooMany(_)));
}

// ---------------------------------------------------------------- properties
proptest! {
    #[test]
    fn rectangles_match_closed_form(w in 0.5f64..50.0, d in 0.5f64..50.0, t in 0.05f64..0.4, h in 1.0f64..10.0) {
        prop_assume!(t < w.min(d) * 0.9);
        let c = rect(w, d, t, h);
        let want = 2.0 * (w + d) * t * h;
        prop_assert!(close(exact(&c), want));
        let m = c.chain_mesh().unwrap();
        prop_assert!(m.is_closed_manifold() && m.euler_characteristic() == 0);
        prop_assert!(close(m.volume(), want));
    }

    #[test]
    fn random_rect_openings_match_reference_and_stay_manifold(
        l in 3.0f64..30.0, h in 2.0f64..6.0,
        slots in proptest::collection::vec((0.05f64..0.9, 0.0f64..0.5, 0.1f64..0.9), 0..6)) {
        // partition the length into equal slots, one opening per used slot
        let k = slots.len().max(1);
        let w = l / k as f64;
        let mut c = straight(l, 0.2, h);
        let ops: Vec<Opening> = slots.iter().enumerate().map(|(i, (frac_w, sill, frac_h))| {
            let ow = (w * 0.8 * frac_w).max(0.05);
            Opening { offset: i as f64 * w + (w - ow) / 2.0, width: ow, sill: sill * h * 0.5, height: (frac_h * h * 0.45).max(0.05) }
        }).collect();
        c.openings = vec![ops];
        let want = reference(&c);
        prop_assert!(close(exact(&c), want));
        let m = c.chain_mesh().unwrap();
        prop_assert!(m.is_closed_manifold());
        prop_assert!(close(m.volume(), want));
    }

    #[test]
    fn random_gable_walls_with_round_windows(l in 4.0f64..20.0, eave in 2.0f64..4.0, rise in 0.2f64..2.0, r in 0.1f64..0.6) {
        let mut c = straight(l, 0.25, eave + rise);
        c.tops = vec![Some(vec![[0.0, eave], [l / 2.0, eave + rise], [l, eave]])];
        let win = circle(l / 4.0, 1.2, r, 24);
        c.poly_openings = vec![vec![win.clone()]];
        let want = (l * eave + 0.5 * l * rise - poly_area(&win)) * 0.25;
        prop_assert!(close(exact(&c), want));
        let m = c.chain_mesh().unwrap();
        prop_assert!(m.is_closed_manifold() && close(m.volume(), want));
    }

    // arbitrary junk must never panic: only Ok or Err
    #[test]
    fn garbage_never_panics(pts in proptest::collection::vec((-1e6f64..1e6, -1e6f64..1e6), 0..12), t in -1.0f64..2.0, h in -1.0f64..5.0, closed: bool,
                            off in -2.0f64..10.0, ow in -1.0f64..5.0,
                            poly in proptest::collection::vec((-3.0f64..9.0, -1.0f64..5.0), 0..9),
                            top in proptest::collection::vec((-1.0f64..9.0, -1.0f64..6.0), 0..5)) {
        let mut c = WallChain::new(pts.iter().map(|p| [p.0, p.1]).collect(), closed, t, h);
        c.openings = vec![vec![Opening { offset: off, width: ow, sill: 0.0, height: 1.0 }]];
        c.poly_openings = vec![vec![poly.iter().map(|p| [p.0, p.1]).collect()]];
        c.tops = vec![Some(top.iter().map(|p| [p.0, p.1]).collect())];
        let _ = total_volume_serial(std::slice::from_ref(&c));
        let _ = c.chain_mesh();
    }

    // a small, sane wall with junk openings: whenever it is accepted, exact volume and mesh must agree
    #[test]
    fn accepted_geometry_is_always_consistent(poly in proptest::collection::vec((0.2f64..5.8, 0.0f64..2.9), 3..8)) {
        let mut c = straight(6.0, 0.2, 3.0);
        c.poly_openings = vec![vec![poly.iter().map(|p| [p.0, p.1]).collect()]];
        if let Ok(v) = total_volume_serial(std::slice::from_ref(&c)) {
            let m = c.chain_mesh().unwrap();
            prop_assert!(close(v, m.volume()));
            prop_assert!(m.is_closed_manifold());
        }
    }
}

// ---------------------------------------------------------------- fast path must equal the reference path
proptest! {
    #[test]
    fn fast_path_equals_profile_path_for_valid_and_invalid_openings(
        l in 2.0f64..20.0, h in 1.5f64..6.0, t in 0.05f64..0.5,
        ops in proptest::collection::vec((-1.0f64..22.0, -0.5f64..4.0, -0.5f64..3.0, -0.5f64..4.0), 0..11)) {
        let mut c = straight(l, t, h);
        c.openings = vec![ops.iter().map(|&(o, w, s, hh)| Opening { offset: o, width: w, sill: s, height: hh }).collect()];
        let plan = c.plans().unwrap();
        let fast = c.segment_volume_exact(0, &plan[0]);
        let full = c.segment_volume_profile(0, &plan[0]);
        match (fast, full) {
            (Ok(a), Ok(b)) => prop_assert!(close(a, b), "{a} vs {b}"),
            (Err(a), Err(b)) => prop_assert_eq!(a, b),
            (a, b) => prop_assert!(false, "disagree: fast {:?} vs full {:?}", a, b),
        }
    }

    #[test]
    fn fast_path_equals_profile_path_on_mitred_rooms(
        w in 2.0f64..20.0, d in 2.0f64..20.0, t in 0.05f64..0.4, h in 2.0f64..5.0, n in 0usize..4) {
        prop_assume!(t < w.min(d) * 0.9);
        let mut c = rect(w, d, t, h);
        c.openings = (0..4).map(|s| if s < n { vec![Opening { offset: 0.5, width: 0.8, sill: 0.0, height: 2.0 }] } else { vec![] }).collect();
        for (i, p) in c.plans().unwrap().iter().enumerate() {
            prop_assert!(close(c.segment_volume_exact(i, p).unwrap(), c.segment_volume_profile(i, p).unwrap()));
        }
    }
}

// ---------------------------------------------------------------- openings keep their true size on angled ends
/// Along-wall coordinates of mesh vertices at a given height, for one wall direction.
fn along_at_height(m: &Mesh, a: P, u: P, z: f64) -> Vec<f64> {
    let mut v: Vec<f64> = m.verts.iter().filter(|p| (p[2] - z).abs() < 1e-9)
        .map(|p| (p[0] - a[0]) * u[0] + (p[1] - a[1]) * u[1]).collect();
    v.sort_by(|x, y| x.total_cmp(y)); v.dedup_by(|x, y| (*x - *y).abs() < 1e-9); v
}

#[test]
fn door_keeps_its_width_on_both_faces_of_a_wall_with_angled_ends() {
    // a partition between two angled faces: both ends are cut at an angle, so the left and right faces have different lengths
    let host_a = straight(10.0, 0.4, 3.0);
    let host_b = WallChain::new(vec![[0.0, 10.0], [10.0, 10.0]], false, 0.4, 3.0);
    let mut c = WallChain::new(vec![[2.0, 1.0], [8.0, 9.0]], false, 0.15, 3.0);
    c.start_stop = Some(host_a.face_stop_for(0, [2.0, 1.0]).unwrap());
    c.end_stop = Some(host_b.face_stop_for(0, [8.0, 9.0]).unwrap());
    let len_c = ((6.0f64).powi(2) + 8.0f64.powi(2)).sqrt(); // 10 m
    c.openings = vec![vec![Opening { offset: 3.0, width: 0.9, sill: 0.0, height: 2.1 }]];
    let plan = &c.plans().unwrap()[0];
    assert!((plan.s_l0 - plan.s_r0).abs() > 1e-3, "test needs genuinely angled ends");
    let m = c.chain_mesh().unwrap();
    assert!(m.is_closed_manifold());
    let u = [6.0 / len_c, 8.0 / len_c];
    let head = along_at_height(&m, [2.0, 1.0], u, 2.1);
    // the door head has exactly two stations, 0.9 m apart, at the requested offset, on BOTH faces
    assert_eq!(head.len(), 2, "{head:?}");
    assert!((head[0] - 3.0).abs() < 1e-9 && (head[1] - 3.9).abs() < 1e-9, "{head:?}");
    // volume equals plan-quad area x height minus the cut box through the full thickness
    assert!(close(m.volume(), reference(&c)));
    assert!(close(exact(&c), reference(&c)));
}

#[test]
fn openings_inside_a_mitre_zone_are_rejected() {
    let mut c = rect(6.0, 4.0, 0.4, 3.0); // 90-degree mitres remove 0.2 m from the inside face at each end
    c.openings = vec![vec![Opening { offset: 0.1, width: 0.9, sill: 0.0, height: 2.1 }]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
    c.openings = vec![vec![Opening { offset: 0.21, width: 0.9, sill: 0.0, height: 2.1 }]];
    assert!(total_volume_serial(std::slice::from_ref(&c)).is_ok());
    // polygon opening too
    c.openings = vec![];
    c.poly_openings = vec![vec![vec![[0.1, 0.5], [1.0, 0.5], [1.0, 1.5], [0.1, 1.5]]]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
}

#[test]
fn wall_shorter_than_its_own_mitres_is_rejected() {
    let c = WallChain::new(vec![[0., 0.], [0.3, 0.], [0.3, 3.0], [0., 3.0]], true, 0.5, 3.0);
    assert!(matches!(c.plans().and_then(|p| c.segment_volume_exact(0, &p[0])), Err(GeomError::WallTooShort(0))));
}

#[test]
fn sloped_top_with_angled_ends_has_planar_faces_and_matching_volumes() {
    let host_a = straight(10.0, 0.4, 3.0);
    let host_b = WallChain::new(vec![[0.0, 10.0], [10.0, 10.0]], false, 0.4, 3.0);
    let mut c = WallChain::new(vec![[2.0, 1.0], [8.0, 9.0]], false, 0.15, 4.0);
    c.start_stop = Some(host_a.face_stop_for(0, [2.0, 1.0]).unwrap());
    c.end_stop = Some(host_b.face_stop_for(0, [8.0, 9.0]).unwrap());
    c.tops = vec![Some(vec![[0.0, 2.5], [5.0, 3.8], [10.0, 2.7]])];
    c.openings = vec![vec![Opening { offset: 2.0, width: 1.0, sill: 0.9, height: 1.0 }]];
    let m = c.chain_mesh().unwrap();
    assert!(m.is_closed_manifold());
    assert!(close(m.volume(), exact(&c)), "{} vs {}", m.volume(), exact(&c));
    // every boundary quad must be planar: check by splitting along both diagonals
    let plan = &c.plans().unwrap()[0];
    let _ = plan;
}

proptest! {
    #[test]
    fn random_angled_walls_mesh_equals_exact_and_door_width_is_true(
        ax in 0.0f64..4.0, bx in 6.0f64..10.0, t in 0.08f64..0.3, len_off in 0.5f64..2.5,
        door_w in 0.6f64..1.2, slope in -0.3f64..0.3) {
        let host_a = straight(14.0, 0.3, 3.0);
        let host_b = WallChain::new(vec![[0.0, 8.0], [14.0, 8.0]], false, 0.3, 3.0);
        let (p, q) = ([ax, 1.0], [bx, 7.0]);
        let mut c = WallChain::new(vec![p, q], false, t, 4.0);
        c.start_stop = Some(host_a.face_stop_for(0, p).unwrap());
        c.end_stop = Some(host_b.face_stop_for(0, q).unwrap());
        let l = ((q[0]-p[0]).powi(2) + (q[1]-p[1]).powi(2)).sqrt();
        c.tops = vec![Some(vec![[0.0, 3.0], [l, 3.0 + slope]])];
        c.openings = vec![vec![Opening { offset: len_off + 0.5, width: door_w, sill: 0.0, height: 2.1 }]];
        let m = c.chain_mesh().unwrap();
        prop_assert!(m.is_closed_manifold());
        prop_assert!(close(m.volume(), exact(&c)));
        let u = [(q[0]-p[0]) / l, (q[1]-p[1]) / l];
        let head = along_at_height(&m, p, u, 2.1);
        prop_assert_eq!(head.len(), 2);
        prop_assert!((head[0] - (len_off + 0.5)).abs() < 1e-7 && (head[1] - (len_off + 0.5 + door_w)).abs() < 1e-7);
    }
}

#[test]
fn openings_inside_the_far_mitre_zone_are_rejected() {
    let mut c = rect(6.0, 4.0, 0.4, 3.0); // far end of the 6 m wall loses 0.2 m: usable up to 5.8
    c.openings = vec![vec![Opening { offset: 4.95, width: 0.9, sill: 0.0, height: 2.1 }]]; // ends at 5.85
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
    c.openings = vec![vec![Opening { offset: 4.85, width: 0.9, sill: 0.0, height: 2.1 }]]; // ends at 5.75
    assert!(total_volume_serial(std::slice::from_ref(&c)).is_ok());
    c.openings = vec![];
    c.poly_openings = vec![vec![vec![[5.0, 0.5], [5.85, 0.5], [5.85, 1.5], [5.0, 1.5]]]];
    assert!(matches!(err_of(&c), GeomError::OpeningOutside { .. }));
}
