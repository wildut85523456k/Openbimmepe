use openbim_arch::envelope::*;
use openbim_arch::layers::*;
use openbim_arch::*;
use openbim_geom::{Opening, WallChain};
use proptest::prelude::*;

// Illustrative round numbers for testing only; they are not taken from any standard or product.
fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn lib() -> Library {
    let m = |n: &str, l: f64| Material::new(n, l);
    Library::new(vec![m("Plaster", 0.5), m("Block", 1.0), m("Insulation", 0.04), m("Concrete", 2.0)]).unwrap()
}
fn solid(m: &str, t: f64) -> Layer { Layer::Solid { material: m.into(), thickness: t } }
fn wall_type() -> Assembly {
    Assembly {
        name: "External wall".into(), kind: AssemblyKind::Wall,
        layers: vec![solid("Plaster", 0.02), solid("Block", 0.2), solid("Insulation", 0.05), solid("Plaster", 0.015), Layer::Resistance { name: "Cavity".into(), thickness: 0.03, r: 0.18 }],
        r_se: 0.04, r_si: 0.13,
    }
}

#[test]
fn u_value_and_thickness_match_the_hand_calculation() {
    let a = wall_type();
    // R = 0.04 + 0.13 + 0.02/0.5 + 0.2/1.0 + 0.05/0.04 + 0.015/0.5 + 0.18 = 0.04+0.13+0.04+0.2+1.25+0.03+0.18 = 1.87
    assert!(close(a.r_total(&lib()).unwrap(), 1.87));
    assert!(close(a.u_value(&lib()).unwrap(), 1.0 / 1.87));
    assert!(close(a.thickness(), 0.02 + 0.2 + 0.05 + 0.015 + 0.03));
    assert!(close(a.layer_r(2, &lib()).unwrap(), 1.25));
    assert!(close(a.layer_r(4, &lib()).unwrap(), 0.18));
    assert!(a.layer_r(5, &lib()).is_err());
    let slab = Assembly { name: "Slab".into(), kind: AssemblyKind::Floor, layers: vec![solid("Concrete", 0.15), solid("Insulation", 0.1)], r_se: 0.17, r_si: 0.17 };
    // R = 0.17 + 0.17 + 0.15/2 + 0.1/0.04 = 0.34 + 0.075 + 2.5 = 2.915
    assert!(close(slab.u_value(&lib()).unwrap(), 1.0 / 2.915));
}

#[test]
fn the_wall_chain_takes_the_assembly_thickness() {
    let t = wall_type().thickness();
    let c = WallChain::new(vec![[0.0, 0.0], [10.0, 0.0]], false, t, 3.0);
    assert!(close(c.thickness, 0.315));
}

#[test]
fn bad_assemblies_and_libraries_are_refused() {
    let l = lib();
    let mut a = wall_type();
    a.layers.push(solid("Gold", 0.01));
    assert_eq!(a.check(&l).unwrap_err(), ArchError::UnknownMaterial("Gold".into()));
    for (what, f) in [
        ("zero thickness", Box::new(|a: &mut Assembly| a.layers[0] = solid("Plaster", 0.0)) as Box<dyn Fn(&mut Assembly)>),
        ("negative", Box::new(|a: &mut Assembly| a.layers[0] = solid("Plaster", -0.1))),
        ("nan", Box::new(|a: &mut Assembly| a.layers[0] = solid("Plaster", f64::NAN))),
        ("empty", Box::new(|a: &mut Assembly| a.layers.clear())),
        ("too thick", Box::new(|a: &mut Assembly| a.layers[1] = solid("Block", 5.0))),
        ("r_se negative", Box::new(|a: &mut Assembly| a.r_se = -0.1)),
        ("r_si huge", Box::new(|a: &mut Assembly| a.r_si = 101.0)),
        ("resistance zero", Box::new(|a: &mut Assembly| a.layers[4] = Layer::Resistance { name: "Cavity".into(), thickness: 0.03, r: 0.0 })),
        ("resistance name", Box::new(|a: &mut Assembly| a.layers[4] = Layer::Resistance { name: "".into(), thickness: 0.03, r: 0.2 })),
        ("bad name", Box::new(|a: &mut Assembly| a.name = "".into())),
    ] {
        let mut a = wall_type(); f(&mut a);
        assert!(a.check(&l).is_err(), "{what}");
        assert!(a.u_value(&l).is_err(), "{what}");
    }
    // limits on the edge are fine
    let mut a = wall_type(); a.layers = vec![solid("Block", 5.0)]; assert!(a.check(&l).is_ok());
    a.layers = vec![Layer::Resistance { name: "x".into(), thickness: 0.1, r: MAX_RESISTANCE }]; a.r_si = MAX_RESISTANCE; assert!(a.check(&l).is_ok());
    a.layers = (0..MAX_LAYERS).map(|_| solid("Block", 0.01)).collect(); assert!(a.check(&l).is_ok());
    a.layers.push(solid("Block", 0.01)); assert!(a.check(&l).is_err());
    // zero surface resistances are allowed when the layers have resistance
    let mut a = wall_type(); a.r_se = 0.0; a.r_si = 0.0; assert!(a.u_value(&l).is_ok());
    // library
    let bad = |m: Material| Library::new(vec![m]).unwrap_err();
    assert!(matches!(bad(Material::new("a", 0.0)), ArchError::BadValue { .. }));
    assert!(matches!(bad(Material::new("a", MAX_CONDUCTIVITY + 1.0)), ArchError::BadValue { .. }));
    assert!(Library::new(vec![Material::new("a", MAX_CONDUCTIVITY)]).is_ok());
    assert!(matches!(bad(Material::new("a", f64::INFINITY)), ArchError::NotFinite(_)));
    assert!(Library::new(vec![Material::new("a", 1.0), Material::new("a", 2.0)]).is_err());
}

fn office(h: f64) -> WallChain {
    let mut c = WallChain::new(vec![[0.0, 0.0], [12.0, 0.0], [12.0, 8.0], [0.0, 8.0]], true, 0.315, h);
    c.openings = vec![
        (0..3).map(|k| Opening { offset: 1.5 + 3.5 * k as f64, width: 1.8, sill: 0.9, height: 1.5 }).collect(),
        vec![Opening { offset: 3.0, width: 1.0, sill: 0.0, height: 2.1 }],
        vec![], vec![],
    ];
    c
}

#[test]
fn the_office_envelope_has_the_hand_calculated_areas_and_directions() {
    let segs = wall_envelope(&office(3.2), Side::Outside, 0.0).unwrap();
    // outside faces are 0.1575 beyond the centreline: south and north 12.315 long, east and west 8.315
    let (south, east, north, west) = (12.315 * 3.2, 8.315 * 3.2, 12.315 * 3.2, 8.315 * 3.2);
    let want = [(180.0, 4, south, 3.0 * 1.8 * 1.5), (90.0, 2, east, 2.1), (0.0, 0, north, 0.0), (270.0, 6, west, 0.0)];
    for (s, (az, sec, gross, open)) in segs.iter().zip(want) {
        assert!(close(s.azimuth, az) && s.sector == sec, "segment {}: {} {}", s.segment, s.azimuth, s.sector);
        assert!(close(s.gross_area, gross) && close(s.opening_area, open) && close(s.net_area, gross - open));
    }
    let t = by_sector(&segs);
    assert!(close(t[4].0, south - 8.1) && close(t[4].1, 8.1) && close(t[2].1, 2.1) && close(t[0].0, north));
    // true north 30 degrees counter-clockwise of the plan's +y: the south wall faces south-west, the east wall south-east
    let r = wall_envelope(&office(3.2), Side::Outside, 30.0).unwrap();
    assert!(close(r[0].azimuth, 210.0) && r[0].sector == 5 && close(r[1].azimuth, 120.0) && r[1].sector == 3);
    assert!(close(r[2].azimuth, 30.0) && r[2].sector == 1 && close(r[3].azimuth, 300.0) && r[3].sector == 7);
}

#[test]
fn drawing_direction_and_side_choices() {
    let mut cw = office(3.2);
    cw.points.reverse(); cw.openings = vec![vec![]; 4];
    let segs = wall_envelope(&cw, Side::Outside, 0.0).unwrap();
    // reversed: segment 0 now runs east along the north side; the outside is on its left
    let mut az: Vec<f64> = segs.iter().map(|s| s.azimuth.round()).collect();
    az.sort_by(f64::total_cmp);
    assert_eq!(az, vec![0.0, 90.0, 180.0, 270.0]);
    assert!(close(segs[0].gross_area, 12.315 * 3.2) && close(segs[1].gross_area, 8.315 * 3.2)); // (0,8)->(12,8), then (12,8)->(12,0)
    // an open wall: the caller says which side is outside
    let open = WallChain::new(vec![[0.0, 0.0], [10.0, 0.0]], false, 0.3, 3.0);
    let r = wall_envelope(&open, Side::Right, 0.0).unwrap(); // right of +x is south
    assert!(close(r[0].azimuth, 180.0) && close(r[0].gross_area, 30.0)); // open chain: face = centreline length
    let l = wall_envelope(&open, Side::Left, 0.0).unwrap();
    assert!(close(l[0].azimuth, 0.0));
    assert_eq!(wall_envelope(&open, Side::Outside, 0.0).unwrap_err(), ArchError::NeedClosedChain);
    assert!(wall_envelope(&open, Side::Left, f64::NAN).is_err());
}

#[test]
fn openings_that_do_not_fit_and_sloped_tops_are_refused() {
    let mut c = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.3, 3.0);
    c.openings = vec![vec![Opening { offset: 0.25, width: 3.5, sill: 0.0, height: 4.0 }]];
    assert!(matches!(wall_envelope(&c, Side::Left, 0.0).unwrap_err(), ArchError::BadValue { .. }));
    let mut t = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.3, 3.0);
    t.tops = vec![Some(vec![[0.0, 2.5], [4.0, 3.0]])];
    assert_eq!(wall_envelope(&t, Side::Left, 0.0).unwrap_err(), ArchError::Unsupported("sloped wall tops in an envelope"));
}

#[test]
fn an_arched_opening_counts_with_its_polygon_area() {
    // a polygon opening: 1 x 2 rectangle written as a polygon, plus a triangle of area 0.5
    let mut c = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0]], false, 0.3, 3.0);
    c.poly_openings = vec![vec![vec![[1.0, 0.0], [2.0, 0.0], [2.0, 2.0], [1.0, 2.0]], vec![[3.0, 1.0], [4.0, 1.0], [3.0, 2.0]]]];
    let r = wall_envelope(&c, Side::Left, 0.0).unwrap();
    assert!(close(r[0].opening_area, 2.0 + 0.5) && close(r[0].net_area, 18.0 - 2.5));
}

#[test]
fn sectors_cover_the_compass() {
    // centre of each sector and the edges between them
    for (az, want) in [(0.0, 0), (22.49, 0), (22.5, 1), (45.0, 1), (67.5, 2), (180.0, 4), (337.4, 7), (337.5, 0), (359.9, 0)] {
        assert_eq!(sector_of(az), want, "{az}");
    }
    assert_eq!(SECTOR_NAMES[sector_of(225.0)], "SW");
}

proptest! {
    #[test]
    fn splitting_a_layer_in_two_does_not_change_u(t1 in 0.005f64..0.3, t2 in 0.005f64..0.3, lam in 0.02f64..3.0, r_se in 0.0f64..0.3, r_si in 0.0f64..0.3, frac in 0.05f64..0.95) {
        let l = Library::new(vec![Material::new("m", lam), Material::new("n", 0.7)]).unwrap();
        let a = Assembly { name: "a".into(), kind: AssemblyKind::Wall, layers: vec![solid("m", t1), solid("n", t2)], r_se, r_si };
        let b = Assembly { name: "b".into(), kind: AssemblyKind::Wall, layers: vec![solid("m", t1 * frac), solid("m", t1 * (1.0 - frac)), solid("n", t2)], r_se, r_si };
        let (ua, ub) = (a.u_value(&l).unwrap(), b.u_value(&l).unwrap());
        prop_assert!((ua - ub).abs() < 1e-9 * ua);
        // adding any layer lowers U
        let mut c = a.clone(); c.layers.push(solid("n", 0.01));
        prop_assert!(c.u_value(&l).unwrap() < ua);
        // thickness adds up
        prop_assert!((a.thickness() - (t1 + t2)).abs() < 1e-12);
    }

    #[test]
    fn the_four_sides_of_a_rectangle_face_four_directions_for_any_north(w in 2.0f64..40.0, d in 2.0f64..40.0, north in -720.0f64..720.0, rev in any::<bool>()) {
        let mut c = WallChain::new(vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], true, 0.3, 3.0);
        if rev { c.points.reverse(); }
        let segs = wall_envelope(&c, Side::Outside, north).unwrap();
        let mut az: Vec<f64> = segs.iter().map(|s| (s.azimuth - north).rem_euclid(360.0)).map(|a| if a > 359.999 { 0.0 } else { a.round() }).collect();
        az.sort_by(f64::total_cmp);
        prop_assert_eq!(az, vec![0.0, 90.0, 180.0, 270.0]);
        let total: f64 = segs.iter().map(|s| s.gross_area).sum();
        prop_assert!((total - (2.0 * (w + 0.3) + 2.0 * (d + 0.3)) * 3.0).abs() < 1e-7 * total);
        prop_assert!(segs.iter().all(|s| s.azimuth >= 0.0 && s.azimuth < 360.0));
    }
}

#[test]
fn counts_limits_and_orientation_of_polygon_openings() {
    let mk = |n: usize| (0..n).map(|i| Material::new(&format!("m{i}"), 1.0)).collect::<Vec<_>>();
    assert!(Library::new(mk(MAX_MATERIALS)).is_ok());
    assert_eq!(Library::new(mk(MAX_MATERIALS + 1)).unwrap_err(), ArchError::TooMany("materials"));
    // a clockwise polygon counts with its positive area
    let mut c = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0]], false, 0.3, 3.0);
    c.poly_openings = vec![vec![vec![[1.0, 0.0], [1.0, 2.0], [2.0, 2.0], [2.0, 0.0]]]];
    let r = wall_envelope(&c, Side::Left, 0.0).unwrap();
    assert!(close(r[0].opening_area, 2.0));
    // an opening that is exactly the whole face leaves nothing, without an error
    let mut f = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.3, 3.0);
    f.openings = vec![vec![Opening { offset: 0.0, width: 4.0, sill: 0.0, height: 3.0 }]];
    let r = wall_envelope(&f, Side::Left, 0.0).unwrap();
    assert!(close(r[0].net_area, 0.0) && close(r[0].gross_area, 12.0));
}
