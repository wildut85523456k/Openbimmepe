use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_place::penetrations::{Run, RunSize};
use openbim_place::Model;
use openbim_route::snaps::*;
use openbim_route::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn close3(a: [f64; 3], b: [f64; 3]) -> bool { (0..3).all(|k| close(a[k], b[k])) }
fn round(d: f64) -> RunSize { RunSize::Round { diameter: d } }

fn prefs() -> RoutingPrefs {
    RoutingPrefs {
        domain: Domain::Pipe,
        system: System::ChilledWaterSupply,
        label: "CHWS".into(),
        elbow_angles: AngleSet::List(vec![90.0, 45.0, 22.5]),
        bend_radius_factor: 1.5,
        min_straight: 0.05,
        junction: Junction::Tee,
        tee_angles: AngleSet::List(vec![90.0, 45.0]),
        tee_half_factor: 1.0,
        branch_stub_factor: 1.0,
        pick_tolerance: 0.02,
        transition_angle_deg: 30.0,
        min_transition_len: 0.1,
        connector_stub: 0.3,
        margin: 0.0,
        catalogue: None,
    }
}
fn off() -> Snaps { Snaps::new(SnapSettings::off()) }
fn plan(m: SnapMode) -> Snaps { Snaps::new(SnapSettings { plan: m, vertical: SnapMode::Off }) }
fn sizes(n: usize, d: f64) -> Vec<RunSize> { vec![round(d); n] }

// ---------- snaps ----------

#[test]
fn increment_snaps_to_the_nearest_multiple() {
    let m = SnapMode::Increment(15.0);
    assert_eq!(m.snap_plan(37.0), 30.0);
    assert_eq!(m.snap_plan(38.0), 45.0);
    assert_eq!(m.snap_plan(22.5), 30.0); // half way goes away from zero
    assert_eq!(m.snap_plan(-22.5), -30.0);
    assert_eq!(m.snap_plan(181.0), 180.0);
    assert_eq!(m.snap_plan(-179.0), 180.0);
    assert_eq!(m.snap_plan(730.0), 15.0);
    assert_eq!(SnapMode::Increment(90.0).snap_plan(50.0), 90.0);
    assert_eq!(m.snap_vertical(22.5), 30.0);
    assert_eq!(m.snap_vertical(89.0), 90.0);
    assert_eq!(m.snap_vertical(-89.0), -90.0);
    assert_eq!(m.snap_vertical(120.0), 90.0);
    assert_eq!(SnapMode::Off.snap_plan(33.3), 33.3);
    assert_eq!(SnapMode::Off.snap_vertical(33.3), 33.3);
}

#[test]
fn a_list_of_angles_is_mirrored_into_all_quadrants() {
    let m = SnapMode::Angles(vec![0.0, 45.0, 90.0]);
    for (a, want) in [(10.0, 0.0), (30.0, 45.0), (100.0, 90.0), (130.0, 135.0), (170.0, 180.0), (-170.0, 180.0), (-130.0, -135.0), (-80.0, -90.0), (-20.0, 0.0), (200.0, 180.0), (215.0, -135.0)] {
        assert_eq!(m.snap_plan(a), want, "{a}");
    }
    // ties go to the angle nearer the X axis, then to the positive one
    let t = SnapMode::Angles(vec![0.0, 45.0]);
    assert_eq!(t.snap_plan(22.5), 0.0);
    assert_eq!(t.snap_plan(-22.5), 0.0);
    // 30 is equally near +30 and -30: positive wins
    assert_eq!(SnapMode::Angles(vec![30.0]).snap_plan(0.0), 30.0);
    // vertical: mirrored up and down
    let v = SnapMode::Angles(vec![0.0, 45.0, 90.0]);
    assert_eq!(v.snap_vertical(20.0), 0.0);
    assert_eq!(v.snap_vertical(30.0), 45.0);
    assert_eq!(v.snap_vertical(-30.0), -45.0);
    assert_eq!(v.snap_vertical(-80.0), -90.0);
    assert_eq!(SnapMode::Angles(vec![0.0, 90.0]).snap_vertical(45.0), 0.0);
}

#[test]
fn snap_settings_are_checked() {
    use SnapMode::*;
    for bad in [Increment(0.0), Increment(-5.0), Increment(90.1), Increment(f64::NAN), Increment(f64::INFINITY), Increment(7.0), Increment(37.0), Increment(100.0)] {
        assert_eq!(bad.check(false), Err(SnapError::Increment), "{bad:?}");
    }
    // 40 divides 360 but not 90: fine in plan, not for the vertical angle
    assert_eq!(Increment(40.0).check(false), Ok(()));
    assert_eq!(Increment(40.0).check(true), Err(SnapError::Increment));
    for ok in [1.0, 5.0, 10.0, 15.0, 22.5, 30.0, 45.0, 90.0] {
        assert_eq!(Increment(ok).check(false), Ok(()), "{ok}");
        assert_eq!(Increment(ok).check(true), Ok(()), "{ok}");
    }
    for bad in [Angles(vec![]), Angles(vec![f64::NAN]), Angles(vec![-1.0]), Angles(vec![360.1]), Angles(vec![0.0; 65])] {
        assert_eq!(bad.check(false), Err(SnapError::List));
    }
    assert_eq!(Angles(vec![91.0]).check(true), Err(SnapError::List));
    assert_eq!(Angles(vec![91.0]).check(false), Ok(()));
    assert_eq!(Angles(vec![360.0, 0.0]).check(false), Ok(()));
    assert_eq!(Angles(vec![0.0; 64]).check(false), Ok(()));
    assert_eq!(Increment(90.0).check(true), Ok(()));
    assert_eq!(Increment(0.001).check(true), Ok(()));
    // a bad setting stops a route
    let s = Snaps { project: SnapSettings { plan: Increment(0.0), vertical: Off }, session: None };
    assert_eq!(route_path(&[[0.0; 3], [1.0, 0.0, 0.0]], &sizes(1, 0.1), &prefs(), &s).err(), Some(RouteError::Snap(SnapError::Increment)));
}

#[test]
fn a_session_can_override_the_project_snaps_without_changing_them() {
    let proj = SnapSettings { plan: SnapMode::Increment(15.0), vertical: SnapMode::Increment(45.0) };
    let mut s = Snaps::new(proj.clone());
    assert_eq!(s.active(), &proj);
    let ses = SnapSettings { plan: SnapMode::Angles(vec![0.0, 90.0]), vertical: SnapMode::Off };
    s.set_session(ses.clone()).unwrap();
    assert_eq!(s.active(), &ses);
    assert_eq!(s.project, proj);
    // the same drawn point snaps differently under each
    let pts = [[0.0, 0.0, 0.0], [2.0, 1.0, 0.0]]; // 26.6 degrees
    let r1 = route_path(&pts, &sizes(1, 0.1), &prefs(), &s).unwrap();
    assert!(close(r1.points[1][1], 0.0)); // 0 degrees
    s.clear_session();
    let r2 = route_path(&pts, &sizes(1, 0.1), &prefs(), &s).unwrap();
    assert!(r2.points[1][1] > 0.5); // 30 degrees
    // a bad override is refused and leaves things as they were
    s.set_session(ses.clone()).unwrap();
    assert!(s.set_session(SnapSettings { plan: SnapMode::Increment(0.0), vertical: SnapMode::Off }).is_err());
    assert_eq!(s.active(), &ses);
    assert!(s.set_project(SnapSettings { plan: SnapMode::Angles(vec![]), vertical: SnapMode::Off }).is_err());
    assert_eq!(s.project, proj);
    s.set_project(SnapSettings::off()).unwrap();
    assert_eq!(s.project, SnapSettings::off());
}

proptest! {
    #[test]
    fn snapping_lands_on_an_allowed_angle_no_further_than_half_a_step(a in -720.0f64..720.0, step in proptest::sample::select(vec![1.0f64, 2.0, 5.0, 10.0, 15.0, 22.5, 30.0, 45.0, 60.0, 90.0])) {
        let m = SnapMode::Increment(step);
        let s = m.snap_plan(a);
        prop_assert!(s > -180.0 - 1e-9 && s <= 180.0 + 1e-9);
        let off = (s / step - (s / step).round()).abs();
        prop_assert!(off < 1e-9 || (s.abs() - 180.0).abs() < 1e-9);
        let d = (a - s).rem_euclid(360.0);
        prop_assert!(d.min(360.0 - d) <= step / 2.0 + 1e-9);
        // idempotent
        prop_assert!((m.snap_plan(s) - s).abs() < 1e-9 || (m.snap_plan(s).abs() - 180.0).abs() < 1e-9);
    }
    #[test]
    fn a_list_snap_is_the_nearest_of_the_mirrored_angles(a in -180.0f64..180.0, l in proptest::collection::vec(0.0f64..90.0, 1..6)) {
        let m = SnapMode::Angles(l.clone());
        let s = m.snap_plan(a);
        let circ = |x: f64, y: f64| { let d = (x - y).rem_euclid(360.0); d.min(360.0 - d) };
        let allowed: Vec<f64> = l.iter().flat_map(|&q| [q, 180.0 - q, 180.0 + q, -q]).collect();
        prop_assert!(allowed.iter().any(|&c| circ(c, s) < 1e-9));
        let best = allowed.iter().map(|&c| circ(c, a)).fold(f64::MAX, f64::min);
        prop_assert!((circ(s, a) - best).abs() < 1e-9);
    }
}

// ---------- elbows ----------

#[test]
fn a_right_angle_gets_an_elbow_and_trimmed_runs_by_hand_calculation() {
    let r = route_path(&[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0]], &sizes(2, 0.1), &prefs(), &off()).unwrap();
    // R = 1.5 x 0.1 = 0.15, tangent = R tan 45 = 0.15
    assert_eq!(r.runs.len(), 2);
    assert!(close3(r.runs[0].a, [0.0, 0.0, 0.0]) && close3(r.runs[0].b, [1.85, 0.0, 0.0]));
    assert!(close3(r.runs[1].a, [2.0, 0.15, 0.0]) && close3(r.runs[1].b, [2.0, 2.0, 0.0]));
    assert_eq!(r.fittings.len(), 1);
    let f = &r.fittings[0];
    assert!(matches!(f.kind, FittingKind::Elbow { angle } if close(angle, 90.0)));
    assert!(close3(f.at, [2.0, 0.0, 0.0]) && close(f.radius, 0.15) && f.trim == vec![0.15, 0.15]);
    assert!(close3(f.ports[0], [-1.0, 0.0, 0.0]) && close3(f.ports[1], [0.0, 1.0, 0.0]));
    // path = 1.85 + 1.85 + a quarter circle of radius 0.15
    assert!(close(r.centreline_length(), 3.7 + 0.15 * std::f64::consts::FRAC_PI_2));
    assert_eq!(r.runs[0].label, "CHWS-1");
    assert_eq!((r.runs[0].domain, r.runs[0].system), (Domain::Pipe, System::ChilledWaterSupply));
}

#[test]
fn a_45_degree_bend_uses_the_half_angle_tangent() {
    let r = route_path(&[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 1.0, 0.0]], &sizes(2, 0.1), &prefs(), &off()).unwrap();
    let t = 0.15 * (22.5f64).to_radians().tan();
    assert!(close(r.runs[0].b[0], 2.0 - t));
    let d = std::f64::consts::FRAC_1_SQRT_2;
    assert!(close3(r.runs[1].a, [2.0 + t * d, t * d, 0.0]));
    assert!(matches!(r.fittings[0].kind, FittingKind::Elbow { angle } if close(angle, 45.0)));
}

#[test]
fn a_bend_with_no_elbow_in_the_preferences_is_refused() {
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 1.7320508075688772, 0.0]]; // 60 degrees
    match route_path(&pts, &sizes(2, 0.1), &prefs(), &off()) {
        Err(RouteError::BendNotAvailable { vertex: 1, angle }) => assert!((angle - 60.0).abs() < 1e-6),
        o => panic!("{o:?}"),
    }
    let mut p = prefs();
    p.elbow_angles = AngleSet::Any;
    assert!(route_path(&pts, &sizes(2, 0.1), &p, &off()).is_ok());
    // within 0.01 degrees counts as the same angle
    let ok = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0 + 1e-4, 1.0, 0.0]];
    assert!(route_path(&ok, &sizes(2, 0.1), &prefs(), &off()).is_ok());
    // 0.5 degrees off is a different angle
    let off_by_half = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0 + 0.5f64.to_radians().tan(), 1.0, 0.0]];
    assert!(matches!(route_path(&off_by_half, &sizes(2, 0.1), &prefs(), &off()), Err(RouteError::BendNotAvailable { .. })));
}

#[test]
fn short_legs_reversals_and_collinear_points() {
    // two elbows on a 0.2 leg need 0.3 of tangent
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 0.2, 0.0], [4.0, 0.2, 0.0]];
    assert_eq!(route_path(&pts, &sizes(3, 0.1), &prefs(), &off()).err(), Some(RouteError::SegmentTooShort { segment: 1 }));
    // 0.3 leaves nothing for a straight; 0.31 leaves 0.01 which is under min_straight 0.05
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 0.31, 0.0], [4.0, 0.31, 0.0]];
    assert!(route_path(&pts, &sizes(3, 0.1), &prefs(), &off()).is_err());
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 0.35, 0.0], [4.0, 0.35, 0.0]];
    assert!(route_path(&pts, &sizes(3, 0.1), &prefs(), &off()).is_ok());
    // out and back
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    assert_eq!(route_path(&pts, &sizes(2, 0.1), &prefs(), &off()).err(), Some(RouteError::Reversal { vertex: 1 }));
    // points on a line of one size become one run
    let r = route_path(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]], &sizes(3, 0.1), &prefs(), &off()).unwrap();
    assert_eq!((r.runs.len(), r.fittings.len(), r.points.len()), (1, 0, 2));
    assert!(close3(r.runs[0].b, [3.0, 0.0, 0.0]));
}

#[test]
fn bad_input_is_refused() {
    let p = prefs();
    let ok = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    let e = |pts: &[[f64; 3]], sz: &[RunSize]| route_path(pts, sz, &p, &off()).err();
    assert!(matches!(e(&ok[..1], &[]), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&ok, &[]), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&ok, &sizes(2, 0.1)), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&[[0.0; 3], [f64::NAN, 0.0, 0.0]], &sizes(1, 0.1)), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&[[0.0; 3], [0.0, 0.0, 0.0]], &sizes(1, 0.1)), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&ok, &[round(0.0)]), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&ok, &[round(-1.0)]), Some(RouteError::BadInput(_))));
    assert!(matches!(e(&ok, &[RunSize::Rect { width: 0.2, height: 0.0, roll: 0.0 }]), Some(RouteError::BadInput(_))));
    let many: Vec<[f64; 3]> = (0..=MAX_POINTS).map(|i| [i as f64, 0.0, 0.0]).collect();
    assert!(matches!(e(&many, &sizes(MAX_POINTS, 0.1)), Some(RouteError::BadInput(_))));
    // bad preferences
    let mut b = prefs();
    b.bend_radius_factor = 0.0;
    assert!(matches!(route_path(&ok, &sizes(1, 0.1), &b, &off()), Err(RouteError::BadPrefs(_))));
    for f in [
        |p: &mut RoutingPrefs| p.domain = Domain::Electrical,
        |p: &mut RoutingPrefs| p.system = System::SupplyAir,
        |p: &mut RoutingPrefs| p.elbow_angles = AngleSet::List(vec![]),
        |p: &mut RoutingPrefs| p.elbow_angles = AngleSet::List(vec![180.0]),
        |p: &mut RoutingPrefs| p.tee_angles = AngleSet::List(vec![0.0]),
        |p: &mut RoutingPrefs| p.min_straight = -1.0,
        |p: &mut RoutingPrefs| p.transition_angle_deg = 0.0,
        |p: &mut RoutingPrefs| p.transition_angle_deg = 180.0,
        |p: &mut RoutingPrefs| p.margin = 11.0,
        |p: &mut RoutingPrefs| p.connector_stub = f64::NAN,
    ] {
        let mut q = prefs();
        f(&mut q);
        assert!(matches!(q.check(), Err(RouteError::BadPrefs(_))));
    }
}

// ---------- snapping while drawing ----------

#[test]
fn points_are_snapped_one_after_the_other_keeping_the_distance() {
    // 8.5 degrees snaps to 0, keeping the length of the drawn leg
    let s = plan(SnapMode::Increment(45.0));
    let r = route_path(&[[0.0, 0.0, 0.0], [2.0, 0.3, 0.0]], &sizes(1, 0.1), &prefs(), &s).unwrap();
    assert!(close3(r.points[1], [(4.0f64 + 0.09).sqrt(), 0.0, 0.0]));
    // the next point snaps from the snapped one: a leg drawn up and slightly right becomes a right angle
    let r = route_path(&[[0.0, 0.0, 0.0], [2.0, 0.3, 0.0], [2.2, 3.0, 0.0]], &sizes(2, 0.1), &prefs(), &s).unwrap();
    let p1 = r.points[1];
    assert!(close(r.points[2][0], p1[0]) && r.points[2][1] > 2.5);
    assert!(matches!(r.fittings[0].kind, FittingKind::Elbow { angle } if close(angle, 90.0)));
    // vertical snaps: 24.2 degrees up becomes 45 with an increment of 45, or 0 with the list 0 and 90
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.9]];
    let v45 = Snaps::new(SnapSettings { plan: SnapMode::Off, vertical: SnapMode::Increment(45.0) });
    let l = (4.0f64 + 0.81).sqrt();
    let r = route_path(&pts, &sizes(1, 0.1), &prefs(), &v45).unwrap();
    assert!(close(r.points[1][2], l * std::f64::consts::FRAC_1_SQRT_2));
    let v0 = Snaps::new(SnapSettings { plan: SnapMode::Off, vertical: SnapMode::Angles(vec![0.0, 90.0]) });
    let r = route_path(&pts, &sizes(1, 0.1), &prefs(), &v0).unwrap();
    assert!(close(r.points[1][2], 0.0) && close(r.points[1][0], l));
    // a riser: straight up stays straight up, whatever the plan snap
    let r = route_path(&[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.01, 0.0, 3.0]], &sizes(2, 0.1), &prefs(), &Snaps::new(SnapSettings { plan: SnapMode::Increment(45.0), vertical: SnapMode::Increment(45.0) })).unwrap();
    assert_eq!(r.fittings.len(), 1);
    assert!(close(r.runs[1].a[0], r.runs[1].b[0]) && close(r.runs[1].a[1], 0.0));
    assert!(r.runs[1].b[2] > 2.9);
}

// ---------- transitions ----------

#[test]
fn a_size_change_in_a_straight_line_gets_a_transition_of_the_preferred_length() {
    let pts = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
    let r = route_path(&pts, &[round(0.1), round(0.2)], &prefs(), &off()).unwrap();
    // delta 0.1, included angle 30 degrees: L = 0.1 / (2 tan 15) = 0.18660...
    let l = 0.1 / (2.0 * 15f64.to_radians().tan());
    assert!(l > 0.1);
    assert_eq!(r.fittings.len(), 1);
    assert_eq!(r.fittings[0].kind, FittingKind::Transition);
    assert!(close3(r.fittings[0].at, [1.0, 0.0, 0.0]));
    assert_eq!(r.fittings[0].port_sizes, vec![round(0.1), round(0.2)]);
    assert!(close3(r.runs[0].b, [1.0, 0.0, 0.0]) && close3(r.runs[1].a, [1.0 + l, 0.0, 0.0]));
    assert!(close(r.centreline_length(), 3.0));
    // a small change takes the minimum length
    let r = route_path(&pts, &[round(0.1), round(0.11)], &prefs(), &off()).unwrap();
    assert!(close(r.runs[1].a[0], 1.1));
    // rectangular: the larger change of width or height decides
    let rc = |w: f64, h: f64| RunSize::Rect { width: w, height: h, roll: 0.0 };
    let r = route_path(&pts, &[rc(0.4, 0.2), rc(0.3, 0.2)], &prefs_duct(), &off()).unwrap();
    let l = 0.1 / (2.0 * 15f64.to_radians().tan());
    assert!(close(r.runs[1].a[0], 1.0 + l));
    // round to rectangular is not made here
    assert_eq!(route_path(&pts, &[rc(0.4, 0.2), round(0.3)], &prefs_duct(), &off()).err(), Some(RouteError::MixedTransition { vertex: 1 }));
    // a size change on a bend is refused
    let bend = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0]];
    assert_eq!(route_path(&bend, &[round(0.1), round(0.2)], &prefs(), &off()).err(), Some(RouteError::SizeChangeAtBend { vertex: 1 }));
    // a transition that does not fit the leg after it
    let short = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.15, 0.0, 0.0]];
    assert_eq!(route_path(&short, &[round(0.1), round(0.2)], &prefs(), &off()).err(), Some(RouteError::SegmentTooShort { segment: 1 }));
}

fn prefs_duct() -> RoutingPrefs {
    RoutingPrefs { domain: Domain::Duct, system: System::SupplyAir, label: "SA".into(), ..prefs() }
}

// ---------- branches ----------

fn main_run() -> Run { Run::round("main", Domain::Pipe, System::ChilledWaterSupply, [0.0, 0.0, 1.0], [4.0, 0.0, 1.0], 0.1) }

#[test]
fn a_branch_dropped_on_a_run_gets_a_tee_and_splits_it() {
    let b = branch_from_run(&main_run(), &[[2.0, 0.01, 1.0], [2.0, 2.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    // the start is dropped onto the run; half tee = 1.0 x 0.1; branch stub = 0.1
    assert_eq!(b.main.len(), 2);
    assert!(close3(b.main[0].a, [0.0, 0.0, 1.0]) && close3(b.main[0].b, [1.9, 0.0, 1.0]));
    assert!(close3(b.main[1].a, [2.1, 0.0, 1.0]) && close3(b.main[1].b, [4.0, 0.0, 1.0]));
    assert_eq!((b.main[0].label.as_str(), b.main[1].label.as_str()), ("main.1", "main.2"));
    assert!(matches!(b.fitting.kind, FittingKind::Tee { branch_angle } if close(branch_angle, 90.0)));
    assert!(close3(b.fitting.at, [2.0, 0.0, 1.0]));
    assert_eq!(b.fitting.port_sizes, vec![round(0.1), round(0.1), round(0.05)]);
    assert!(close3(b.fitting.ports[2], [0.0, 1.0, 0.0]));
    assert_eq!(b.route.runs.len(), 1);
    assert!(close3(b.route.runs[0].a, [2.0, 0.1, 1.0]) && close3(b.route.runs[0].b, [2.0, 2.0, 1.0]));
    // a 45 degree branch, and the supplement of an allowed angle
    let b = branch_from_run(&main_run(), &[[2.0, 0.0, 1.0], [3.0, 1.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    assert!(matches!(b.fitting.kind, FittingKind::Tee { branch_angle } if close(branch_angle, 45.0)));
    let b = branch_from_run(&main_run(), &[[2.0, 0.0, 1.0], [1.0, 1.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    assert!(matches!(b.fitting.kind, FittingKind::Tee { branch_angle } if close(branch_angle, 135.0)));
    // a drop straight down from the run
    let b = branch_from_run(&main_run(), &[[1.0, 0.0, 1.0], [1.0, 0.0, 0.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    assert!(close3(b.route.runs[0].b, [1.0, 0.0, 0.0]) && close3(b.route.runs[0].a, [1.0, 0.0, 0.9]));
}

#[test]
fn branch_errors() {
    let m = main_run();
    let pts = |p: [f64; 3], q: [f64; 3]| vec![p, q];
    let go = |p: Vec<[f64; 3]>| branch_from_run(&m, &p, &[round(0.05)], &prefs(), &off()).err();
    assert_eq!(go(pts([2.0, 0.05, 1.0], [2.0, 2.0, 1.0])), Some(RouteError::NotOnRun)); // too far from the run
    assert_eq!(go(pts([-0.5, 0.0, 1.0], [-0.5, 2.0, 1.0])), Some(RouteError::NotOnRun)); // beyond the end
    assert_eq!(go(pts([4.5, 0.0, 1.0], [4.5, 2.0, 1.0])), Some(RouteError::NotOnRun));
    assert!(matches!(go(pts([2.0, 0.0, 1.0], [3.0, 1.7320508075688772, 1.0])), Some(RouteError::TeeAngleNotAvailable { .. }))); // 60 degrees
    assert_eq!(go(pts([0.1, 0.0, 1.0], [0.1, 2.0, 1.0])), Some(RouteError::TeeTooCloseToEnd));
    assert_eq!(go(pts([3.95, 0.0, 1.0], [3.95, 2.0, 1.0])), Some(RouteError::TeeTooCloseToEnd));
    assert!(matches!(go(pts([2.0, 0.0, 1.0], [3.0, 0.0, 1.0])), Some(RouteError::BadInput(_)))); // along the run
    assert!(matches!(go(vec![[2.0, 0.0, 1.0]]), Some(RouteError::BadInput(_))));
    assert!(matches!(branch_from_run(&m, &pts([2.0, 0.0, 1.0], [2.0, 2.0, 1.0]), &[], &prefs(), &off()), Err(RouteError::BadInput(_))));
    // a branch whose first leg is shorter than the stub
    assert_eq!(go(pts([2.0, 0.0, 1.0], [2.0, 0.12, 1.0])), Some(RouteError::SegmentTooShort { segment: 0 }));
    // the pick tolerance is the author's
    let mut p = prefs();
    p.pick_tolerance = 0.1;
    assert!(branch_from_run(&m, &pts([2.0, 0.05, 1.0], [2.0, 2.0, 1.0]), &[round(0.05)], &p, &off()).is_ok());
}

#[test]
fn a_tap_leaves_the_run_whole() {
    let mut p = prefs();
    p.junction = Junction::Tap;
    let b = branch_from_run(&main_run(), &[[0.1, 0.0, 1.0], [0.1, 2.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    assert_eq!(b.main.len(), 1);
    assert_eq!(b.main[0], main_run());
    assert!(matches!(b.fitting.kind, FittingKind::Tap { branch_angle } if close(branch_angle, 90.0)));
    assert_eq!(b.fitting.ports.len(), 1);
}

// ---------- equipment ----------

fn conn(pos: [f64; 3], dir: [f64; 3], up: [f64; 3], size: Size) -> Connector {
    Connector { name: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos, dir, up, size, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
}

#[test]
fn a_route_leaves_a_connector_straight_and_is_resized_after_the_stub() {
    let c = conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], Size::Round { diameter: 0.05 });
    let r = route_from_connector(&c, &[[3.0, 0.0, 1.0]], round(0.1), &prefs(), &off()).unwrap();
    // stub 0.3 at the connector's own size, then a transition (0.05 change: min length 0.1), then the run
    assert_eq!(r.runs.len(), 2);
    assert!(close3(r.runs[0].a, [0.0, 0.0, 1.0]) && close3(r.runs[0].b, [0.3, 0.0, 1.0]) && r.runs[0].size == round(0.05));
    assert_eq!(r.fittings[0].kind, FittingKind::Transition);
    assert!(close3(r.runs[1].a, [0.4, 0.0, 1.0]) && r.runs[1].size == round(0.1));
    // same size: just one run
    let r = route_from_connector(&c, &[[3.0, 0.0, 1.0]], round(0.05), &prefs(), &off()).unwrap();
    assert_eq!((r.runs.len(), r.fittings.len()), (1, 0));
    // the first leg is exactly the connector's direction even with coarse snaps, and a drawn point off to the side bends
    let s = plan(SnapMode::Increment(90.0));
    let r = route_from_connector(&c, &[[0.6, 0.0, 1.0], [0.6, 2.0, 1.0]], round(0.05), &prefs(), &s).unwrap();
    assert!(close3(r.runs[0].a, [0.0, 0.0, 1.0]) && r.runs[0].b[1].abs() < 1e-9);
    assert_eq!(r.fittings.len(), 1);
    // a connector that faces diagonally keeps its direction
    let dd = std::f64::consts::FRAC_1_SQRT_2;
    let c2 = conn([0.0, 0.0, 1.0], [dd, dd, 0.0], [0.0, 0.0, 1.0], Size::Round { diameter: 0.05 });
    let r = route_from_connector(&c2, &[[3.0, 3.0, 1.0]], round(0.05), &prefs(), &s).unwrap();
    assert!(close(r.runs[0].b[0], r.runs[0].b[1]));
}

#[test]
fn a_connector_must_belong_to_the_system_and_be_routable() {
    let ok = conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], Size::Round { diameter: 0.05 });
    let go = |c: &Connector| route_from_connector(c, &[[3.0, 0.0, 1.0]], round(0.05), &prefs(), &off()).err();
    assert_eq!(go(&ok), None);
    let mut c = ok.clone();
    c.domain = Domain::Duct;
    assert_eq!(go(&c), Some(RouteError::ConnectorMismatch));
    let mut c = ok.clone();
    c.system = System::DomesticCold;
    assert_eq!(go(&c), Some(RouteError::ConnectorMismatch));
    let mut c = ok.clone();
    c.system = System::Other;
    assert_eq!(go(&c), None);
    let mut c = ok.clone();
    c.domain = Domain::Electrical;
    c.size = Size::Electrical { voltage: 230.0, poles: 1 };
    assert_eq!(go(&c), Some(RouteError::ConnectorMismatch));
    assert!(matches!(route_from_connector(&ok, &[], round(0.05), &prefs(), &off()), Err(RouteError::BadInput(_))));
}

#[test]
fn a_rectangular_connector_sets_the_roll_of_the_stub() {
    // faces +Y with its height along +X (a duct on its side): up_for gives +Z by default, so the roll is 90 degrees
    let c = Connector { domain: Domain::Duct, system: System::SupplyAir, ..conn([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], Size::Rect { width: 0.4, height: 0.2 }) };
    let r = route_from_connector(&c, &[[0.0, 3.0, 1.0]], RunSize::Rect { width: 0.4, height: 0.2, roll: 90.0 }, &prefs_duct(), &off()).unwrap();
    match r.runs[0].size {
        RunSize::Rect { roll, width, height } => assert!(close(roll, 90.0) && close(width, 0.4) && close(height, 0.2)),
        s => panic!("{s:?}"),
    }
    // the default up gives roll 0
    let c = Connector { domain: Domain::Duct, system: System::SupplyAir, ..conn([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], Size::Rect { width: 0.4, height: 0.2 }) };
    let r = route_from_connector(&c, &[[0.0, 3.0, 1.0]], RunSize::Rect { width: 0.4, height: 0.2, roll: 0.0 }, &prefs_duct(), &off()).unwrap();
    assert!(matches!(r.runs[0].size, RunSize::Rect { roll, .. } if roll.abs() < 1e-9));
}

#[test]
fn a_reversed_route_is_the_same_path() {
    let r = route_path(&[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [2.0, 2.0, 2.0]], &sizes(3, 0.1), &prefs(), &off()).unwrap();
    let b = r.reversed();
    assert_eq!(b.runs.len(), 3);
    assert!(close3(b.runs[0].a, r.runs[2].b) && close3(b.runs[0].b, r.runs[2].a));
    assert!(close(b.centreline_length(), r.centreline_length()));
    assert_eq!(b.reversed(), r);
}

// ---------- straight into the wall register ----------

#[test]
fn a_routed_pipe_through_a_wall_gets_its_opening_in_the_register() {
    use openbim_geom::WallChain;
    let mut p = prefs();
    p.margin = 0.025;
    let mut wall = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0]], false, 0.2, 3.0);
    wall.fire_rating_min = Some(60);
    // goes through the wall at y = 0, turns after it
    let r = route_path(&[[3.0, -2.0, 1.5], [3.0, 2.0, 1.5], [6.0, 2.0, 1.5]], &sizes(2, 0.1), &p, &off()).unwrap();
    let m = Model { chains: vec![wall], runs: r.runs.clone(), ..Default::default() };
    let b = m.build().unwrap();
    assert_eq!(b.penetrations.len(), 1);
    let pen = &b.penetrations[0];
    assert!(pen.in_fire_partition() && pen.runs == vec![0]);
    assert!(close(pen.width, 0.15) && close(pen.height, 0.15) && close(pen.offset, 2.925) && close(pen.sill, 1.425));
}

// ---------- the path is the polyline ----------

proptest! {
    #[test]
    fn the_routed_path_is_the_drawn_polyline_with_its_corners_rounded(
        legs in proptest::collection::vec((0usize..8, 0usize..3, 1.0f64..3.0), 1..8),
        d in 0.02f64..0.3,
    ) {
        // directions come from the snap sets so every bend angle is a multiple of 45 degrees
        let snaps = Snaps::new(SnapSettings { plan: SnapMode::Increment(45.0), vertical: SnapMode::Angles(vec![0.0, 90.0]) });
        let mut p = prefs();
        p.elbow_angles = AngleSet::Any;
        p.min_straight = 0.0;
        let mut pts = vec![[0.0, 0.0, 0.0]];
        for (az, el, l) in &legs {
            let last = *pts.last().unwrap();
            let a = (*az as f64) * 45.0f64.to_radians();
            let (dx, dy, dz) = match el { 0 => (a.cos(), a.sin(), 0.0), 1 => (0.0, 0.0, 1.0), _ => (0.0, 0.0, -1.0) };
            pts.push([last[0] + dx * l, last[1] + dy * l, last[2] + dz * l]);
        }
        let r = match route_path(&pts, &sizes(pts.len() - 1, d), &p, &snaps) {
            Ok(r) => r,
            Err(RouteError::Reversal { .. }) | Err(RouteError::SegmentTooShort { .. }) => return Ok(()),
            Err(e) => return Err(TestCaseError::fail(format!("{e:?}"))),
        };
        // 1. snapping a snapped path changes nothing
        let again = route_path(&r.points, &sizes(r.points.len() - 1, d), &p, &snaps).unwrap();
        prop_assert_eq!(again.points.len(), r.points.len());
        for (x, y) in again.points.iter().zip(&r.points) { prop_assert!(dist3(*x, *y) < 1e-9); }
        // 2. independent: length of the polyline minus (2t - R theta) at each corner equals the centreline
        let n = r.points.len();
        let poly: f64 = (0..n - 1).map(|i| dist3(r.points[i], r.points[i + 1])).sum();
        let mut expect = poly;
        let mut bends = 0;
        for v in 1..n - 1 {
            let a = unit3(sub3(r.points[v], r.points[v - 1]));
            let b = unit3(sub3(r.points[v + 1], r.points[v]));
            let theta = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0).acos();
            let rr = 1.5 * d;
            expect -= 2.0 * rr * (theta / 2.0).tan() - rr * theta;
            bends += 1;
        }
        prop_assert!((r.centreline_length() - expect).abs() < 1e-9, "{} vs {}", r.centreline_length(), expect);
        // 3. one elbow per bend, and each run lies on its leg
        prop_assert_eq!(r.fittings.len(), bends);
        prop_assert_eq!(r.runs.len(), n - 1);
        for (i, run) in r.runs.iter().enumerate() {
            let (a, b) = (r.points[i], r.points[i + 1]);
            let dirl = unit3(sub3(b, a));
            for q in [run.a, run.b] {
                let w = sub3(q, a);
                let along = w[0] * dirl[0] + w[1] * dirl[1] + w[2] * dirl[2];
                let perp = [w[0] - along * dirl[0], w[1] - along * dirl[1], w[2] - along * dirl[2]];
                prop_assert!(perp.iter().all(|v| v.abs() < 1e-9));
                prop_assert!(along > -1e-9 && along < dist3(a, b) + 1e-9);
            }
            // the run points the way the leg does
            let rd = unit3(sub3(run.b, run.a));
            prop_assert!(rd[0] * dirl[0] + rd[1] * dirl[1] + rd[2] * dirl[2] > 1.0 - 1e-9);
        }
    }
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 { let d = sub3(a, b); (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() }
fn unit3(a: [f64; 3]) -> [f64; 3] { let n = dist3(a, [0.0; 3]); [a[0] / n, a[1] / n, a[2] / n] }

#[test]
fn a_kink_too_small_for_a_fitting_keeps_both_runs_and_gets_no_elbow() {
    // 0.00001 degrees off straight: not merged (that would move a point), and no fitting is made for it
    let k = 1e-5f64.to_radians().tan();
    let pts = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [6.0, 3.0 * k, 0.0]];
    let r = route_path(&pts, &sizes(2, 0.1), &prefs(), &off()).unwrap();
    assert_eq!((r.runs.len(), r.fittings.len()), (2, 0));
    assert!(close3(r.runs[0].b, [3.0, 0.0, 0.0]) && close3(r.runs[1].a, [3.0, 0.0, 0.0]));
    // exactly straight is merged, as before
    let r = route_path(&[[0.0; 3], [3.0, 0.0, 0.0], [6.0, 0.0, 0.0]], &sizes(2, 0.1), &prefs(), &off()).unwrap();
    assert_eq!(r.runs.len(), 1);
    // nearly straight but a size change: a transition, as for exactly straight
    let r = route_path(&pts, &[round(0.1), round(0.2)], &prefs(), &off()).unwrap();
    assert_eq!(r.fittings.len(), 1);
    assert_eq!(r.fittings[0].kind, FittingKind::Transition);
    // doubling back on the same line is a reversal, never a merge
    let back = [[0.0; 3], [3.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    assert_eq!(route_path(&back, &sizes(2, 0.1), &prefs(), &off()).err(), Some(RouteError::Reversal { vertex: 1 }));
}
