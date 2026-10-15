use openbim_family::mep::{Domain, System};
use openbim_place::penetrations::{Run, RunSize};
use openbim_route::catalogue::*;
use openbim_route::snaps::*;
use openbim_route::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn round(d: f64) -> RunSize { RunSize::Round { diameter: d } }
fn off() -> Snaps { Snaps::new(SnapSettings::off()) }

/// A drainage pipe: elbows of 90, 45 and 22.5 with 2 degrees of slack for the slope.
fn drain(slack: f64) -> RoutingPrefs {
    use FittingClass::*;
    let d = Domain::Pipe;
    RoutingPrefs {
        domain: d,
        system: System::Sanitary,
        label: "SAN".into(),
        elbow_angles: AngleSet::Any,
        bend_radius_factor: 1.0,
        min_straight: 0.0,
        junction: Junction::Tee,
        tee_angles: AngleSet::Any,
        tee_half_factor: 1.0,
        branch_stub_factor: 1.0,
        pick_tolerance: 0.02,
        transition_angle_deg: 30.0,
        min_transition_len: 0.1,
        connector_stub: 0.3,
        margin: 0.0,
        catalogue: Some(Catalogue::new(vec![
            FittingType::new("Bend", Elbow, d).angles(AngleSet::List(vec![90.0, 45.0, 22.5])).radius(1.0).slack(slack),
            FittingType::new("Wye", Tee, d).angles(AngleSet::List(vec![45.0, 90.0])).tee_lengths(1.0, 1.0).slack(slack),
            FittingType::new("Reducer", Transition, d).transition(30.0, 0.1),
        ])),
    }
}

#[test]
fn elevations_follow_the_gradient_leg_by_leg() {
    // 10 m at 1 %, then 6 m level, then 4 m rising at 2 %
    let plan = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [14.0, 6.0]];
    let r = route_graded(&plan, 3.0, &[0.01, 0.0, -0.02], &[round(0.1); 3], &drain(2.0), &off()).unwrap();
    assert!(close(r.points[0][2], 3.0) && close(r.points[1][2], 2.9) && close(r.points[2][2], 2.9) && close(r.points[3][2], 2.98));
    // the first run falls exactly 1 % over its own horizontal length
    let a = &r.runs[0];
    let h = ((a.b[0] - a.a[0]).powi(2) + (a.b[1] - a.a[1]).powi(2)).sqrt();
    assert!(close((a.a[2] - a.b[2]) / h, 0.01));
    // a sloping leg followed by a level one turns exactly 90 degrees in space, so the trim is R tan 45
    match r.fittings[0].kind {
        FittingKind::Elbow { angle } => assert!(close(angle, 90.0), "{angle}"),
        ref k => panic!("{k:?}"),
    }
    assert!(close(r.fittings[0].trim[0], 0.1));
    // two sloping legs turn a little less than 90 degrees in space: cos(theta) = g^2 / (1 + g^2) for equal gradients
    let r2 = route_graded(&[[0.0, 0.0], [10.0, 0.0], [10.0, 6.0]], 3.0, &[0.02, 0.02], &[round(0.1); 2], &drain(2.0), &off()).unwrap();
    let theta = (0.02f64 * 0.02 / (1.0 + 0.02 * 0.02)).acos();
    match r2.fittings[0].kind {
        FittingKind::Elbow { angle } => assert!(close(angle, theta.to_degrees()), "{angle}"),
        ref k => panic!("{k:?}"),
    }
    assert!(close(r2.fittings[0].trim[0], 0.1 * (theta / 2.0).tan()));
}

#[test]
fn a_steep_slope_needs_slack_because_the_bend_in_space_is_less_than_the_bend_in_plan() {
    // equal gradient g on both legs: cos(theta) = g^2 / (1 + g^2). At 1 % that is 89.994 degrees (inside the default
    // tolerance); at 10 % it is 89.43 degrees
    let plan = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0]];
    let go = |g: f64, slack: f64| route_graded(&plan, 3.0, &[g, g], &[round(0.1); 2], &drain(slack), &off());
    assert!(go(0.01, 0.01).is_ok());
    match go(0.1, 0.01) {
        Err(RouteError::BendNotAvailable { vertex: 1, angle }) => assert!((angle - (0.01f64 / 1.01).acos().to_degrees()).abs() < 1e-9 && (angle - 89.43).abs() < 0.01, "{angle}"),
        o => panic!("{o:?}"),
    }
    assert!(go(0.1, 0.5).is_err()); // needs 0.57
    assert!(go(0.1, 0.6).is_ok());
    assert!(go(0.1, 1.0).is_ok());
    // the trim uses the angle in space, not 90
    let r = go(0.1, 1.0).unwrap();
    let theta = (0.01f64 / 1.01).acos();
    assert!(close(r.fittings[0].trim[0], 0.1 * (theta / 2.0).tan()));
}

#[test]
fn a_fall_between_two_elevations_is_even() {
    let plan = [[0.0, 0.0], [12.0, 0.0], [12.0, 8.0]]; // 20 m
    let r = route_fall_between(&plan, 3.0, 2.8, &[round(0.1); 2], &drain(2.0), &off()).unwrap();
    assert!(close(r.points[0][2], 3.0) && close(r.points[1][2], 3.0 - 0.01 * 12.0) && close(r.points[2][2], 2.8));
    // rising when the end is higher
    let r = route_fall_between(&plan, 3.0, 3.4, &[round(0.1); 2], &drain(2.0), &off()).unwrap();
    assert!(close(r.points[2][2], 3.4));
    assert_eq!(route_fall_between(&plan[..1], 3.0, 2.8, &[], &drain(2.0), &off()).err().map(|e| matches!(e, RouteError::BadInput(_))), Some(true));
    assert!(matches!(route_fall_between(&[[0.0, 0.0], [0.0, 0.0]], 3.0, 2.8, &[round(0.1)], &drain(2.0), &off()), Err(RouteError::BadInput(_))));
}

#[test]
fn the_fall_check_names_the_first_run_that_does_not_fall_enough() {
    let plan = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [16.0, 6.0]];
    let r = route_graded(&plan, 3.0, &[0.01, 0.01, 0.01], &[round(0.1); 3], &drain(2.0), &off()).unwrap();
    assert_eq!(r.check_fall(0.01), Ok(()));
    assert_eq!(r.check_fall(0.0100001), Err(0));
    // a level leg in the middle, and a rising one
    let r = route_graded(&plan, 3.0, &[0.01, 0.0, 0.01], &[round(0.1); 3], &drain(2.0), &off()).unwrap();
    assert_eq!(r.check_fall(0.005), Err(1));
    let r = route_graded(&plan, 3.0, &[0.01, 0.01, -0.01], &[round(0.1); 3], &drain(2.0), &off()).unwrap();
    assert_eq!(r.check_fall(0.005), Err(2));
    // a vertical drop falls, a vertical rise does not
    let mk = |a: f64, b: f64| Route { points: vec![], fittings: vec![], runs: vec![Run::round("v", Domain::Pipe, System::Sanitary, [0.0, 0.0, a], [0.0, 0.0, b], 0.1)] };
    assert_eq!(mk(3.0, 2.0).check_fall(0.01), Ok(()));
    assert_eq!(mk(2.0, 3.0).check_fall(0.01), Err(0));
    assert_eq!(mk(3.0, 3.0).check_fall(0.01), Err(0));
}

#[test]
fn the_plan_snaps_apply_but_the_slope_is_kept() {
    // the plan angle 8 degrees snaps to 0; the vertical snap (45 degree steps) must not flatten the 1 % slope
    let snaps = Snaps::new(SnapSettings { plan: SnapMode::Increment(45.0), vertical: SnapMode::Increment(45.0) });
    let plan = [[0.0, 0.0], [10.0, 1.4]];
    let h = (10.0f64 * 10.0 + 1.4 * 1.4).sqrt();
    let r = route_graded(&plan, 3.0, &[0.01], &[round(0.1)], &drain(2.0), &snaps).unwrap();
    assert!(close(r.points[1][1], 0.0) && close(r.points[1][0], h));
    assert!(close((r.points[0][2] - r.points[1][2]) / h, 0.01));
    // the vertical snap is not used here, so even a bad vertical setting changes nothing; a bad plan one is refused
    let vbad = Snaps::new(SnapSettings { plan: SnapMode::Off, vertical: SnapMode::Increment(0.0) });
    assert!(route_graded(&plan, 3.0, &[0.01], &[round(0.1)], &drain(2.0), &vbad).is_ok());
    let pbad = Snaps::new(SnapSettings { plan: SnapMode::Increment(0.0), vertical: SnapMode::Off });
    assert_eq!(route_graded(&plan, 3.0, &[0.01], &[round(0.1)], &drain(2.0), &pbad).err(), Some(RouteError::Snap(SnapError::Increment)));
}

#[test]
fn bad_gradients_are_refused() {
    let plan = [[0.0, 0.0], [10.0, 0.0]];
    let go = |g: f64| route_graded(&plan, 3.0, &[g], &[round(0.1)], &drain(2.0), &off()).err();
    assert_eq!(go(f64::NAN), Some(RouteError::BadGrade));
    assert_eq!(go(f64::INFINITY), Some(RouteError::BadGrade));
    assert_eq!(go(1.0001), Some(RouteError::BadGrade));
    assert_eq!(go(-1.0001), Some(RouteError::BadGrade));
    assert_eq!(go(1.0), None);
    assert_eq!(go(-1.0), None);
    assert!(matches!(route_graded(&plan, 3.0, &[], &[round(0.1)], &drain(2.0), &off()), Err(RouteError::BadInput(_))));
    assert!(matches!(route_graded(&plan, f64::NAN, &[0.01], &[round(0.1)], &drain(2.0), &off()), Err(RouteError::BadInput(_))));
    assert!(matches!(route_graded(&plan[..1], 3.0, &[], &[], &drain(2.0), &off()), Err(RouteError::BadInput(_))));
    assert!(matches!(route_graded(&[[0.0, 0.0], [f64::NAN, 0.0]], 3.0, &[0.01], &[round(0.1)], &drain(2.0), &off()), Err(RouteError::BadInput(_))));
}

#[test]
fn a_branch_joins_a_sloped_run_and_slack_covers_the_slope() {
    // a 10 % main and a 10 % branch: the branch leaves at 89.43 degrees to the main, not 90
    let main = Run::round("main", Domain::Pipe, System::Sanitary, [0.0, 0.0, 3.0], [10.0, 0.0, 2.0], 0.1);
    let pts = [[5.0, 0.0, 2.5], [5.0, 3.0, 2.2]];
    let ok = branch_from_run(&main, &pts, &[round(0.1)], &drain(2.0), &off()).unwrap();
    match ok.fitting.kind {
        FittingKind::Tee { branch_angle } => assert!((branch_angle - 89.43).abs() < 0.01, "{branch_angle}"),
        ref k => panic!("{k:?}"),
    }
    assert!(matches!(branch_from_run(&main, &pts, &[round(0.1)], &drain(0.01), &off()), Err(RouteError::TeeAngleNotAvailable { .. })));
    // a level branch on a sloping main is exactly square
    let level = [[5.0, 0.0, 2.5], [5.0, 3.0, 2.5]];
    let b = branch_from_run(&main, &level, &[round(0.1)], &drain(0.01), &off()).unwrap();
    assert!(matches!(b.fitting.kind, FittingKind::Tee { branch_angle } if close(branch_angle, 90.0)));
}

#[test]
fn slack_is_checked() {
    for bad in [-0.1, 45.1, f64::NAN, f64::INFINITY] {
        let mut p = drain(2.0);
        p.catalogue.as_mut().unwrap().types[0].angle_slack_deg = bad;
        assert_eq!(p.check(), Err(RouteError::BadCatalogue(CatalogueError::BadDimension)), "{bad}");
    }
    for ok in [0.0, 45.0, 2.0] {
        let mut p = drain(2.0);
        p.catalogue.as_mut().unwrap().types[0].angle_slack_deg = ok;
        assert_eq!(p.check(), Ok(()));
    }
}

proptest! {
    #[test]
    fn every_straight_run_keeps_its_gradient_and_the_invert_follows_the_legs(
        legs in proptest::collection::vec((0usize..4, 3.0f64..9.0, -0.05f64..0.05), 1..7),
        z0 in 0.0f64..10.0,
    ) {
        // plan legs along the four axes, so every bend is a right angle in plan; slack covers the slope
        let mut plan = vec![[0.0, 0.0]];
        let mut grades = Vec::new();
        let mut last_dir = 9usize;
        for (dir, l, g) in &legs {
            // always turn a right angle in plan: not straight on, not back
            let dir = if last_dir != 9 && (*dir % 2 == last_dir % 2) { (last_dir + 1) % 4 } else { *dir };
            last_dir = dir;
            let p = *plan.last().unwrap();
            let (dx, dy) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][dir];
            plan.push([p[0] + dx * l, p[1] + dy * l]);
            grades.push(*g);
        }
        let n = plan.len() - 1;
        let r = match route_graded(&plan, z0, &grades, &vec![round(0.1); n], &drain(8.0), &off()) {
            Ok(r) => r,
            Err(e) => return Err(TestCaseError::fail(format!("{e:?}"))),
        };
        // the elevation at each vertex is the start less the sum of gradient x horizontal length
        let mut z = z0;
        prop_assert!((r.points[0][2] - z).abs() < 1e-9);
        for i in 0..n {
            let h = ((plan[i + 1][0] - plan[i][0]).powi(2) + (plan[i + 1][1] - plan[i][1]).powi(2)).sqrt();
            z -= grades[i] * h;
            prop_assert!((r.points[i + 1][2] - z).abs() < 1e-9, "vertex {} {} vs {}", i + 1, r.points[i + 1][2], z);
        }
        // and each run falls with its leg's gradient
        for (i, run) in r.runs.iter().enumerate() {
            let h = ((run.b[0] - run.a[0]).powi(2) + (run.b[1] - run.a[1]).powi(2)).sqrt();
            prop_assert!(h > 0.0);
            prop_assert!(((run.a[2] - run.b[2]) / h - grades[i]).abs() < 1e-9);
        }
    }
}
