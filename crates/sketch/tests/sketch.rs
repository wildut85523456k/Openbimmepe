use openbim_sketch::*;
use proptest::prelude::*;

fn near(a: [f64; 2], x: f64, y: f64) -> bool { (a[0] - x).abs() < 1e-8 && (a[1] - y).abs() < 1e-8 }

fn rectangle(w: f64, h: f64) -> Sketch {
    let mut s = Sketch::default();
    let p: Vec<usize> = [(0.1, -0.1), (3.8, 0.2), (4.1, 3.1), (-0.2, 2.9)].iter().map(|q| s.point(q.0, q.1)).collect();
    for k in 0..4 { s.line(p[k], p[(k + 1) % 4]); }
    s.constrain(Constraint::Fixed { p: 0, x: 0.0, y: 0.0 })
        .constrain(Constraint::Horizontal { a: 0, b: 1 })
        .constrain(Constraint::Vertical { a: 1, b: 2 })
        .constrain(Constraint::Horizontal { a: 2, b: 3 })
        .constrain(Constraint::Vertical { a: 3, b: 0 })
        .constrain(Constraint::Distance { a: 0, b: 1, d: w })
        .constrain(Constraint::Distance { a: 1, b: 2, d: h });
    s
}

#[test]
fn a_locked_rectangle_with_two_dimensions_is_fully_fixed() {
    let sol = rectangle(4.0, 3.0).solve().unwrap();
    assert_eq!(sol.status, Status::Solved);
    assert!(near(sol.points[0], 0.0, 0.0) && near(sol.points[1], 4.0, 0.0) && near(sol.points[2], 4.0, 3.0) && near(sol.points[3], 0.0, 3.0), "{:?}", sol.points);
    assert_eq!((sol.dof, sol.redundant), (0, 0));
    assert_eq!(sol.fixed, vec![true; 4]);
}

#[test]
fn a_missing_dimension_leaves_one_movement_and_unfixed_points() {
    let mut s = rectangle(4.0, 3.0);
    s.constraints.pop();
    let sol = s.solve().unwrap();
    assert_eq!(sol.status, Status::Solved);
    assert_eq!((sol.dof, sol.redundant), (1, 0));
    assert_eq!(sol.fixed, vec![true, true, false, false]);
    assert!(near(sol.points[1], 4.0, 0.0));
    // the unconstrained height stays where it was drawn (nearest solution)
    assert!((sol.points[2][1] - sol.points[3][1]).abs() < 1e-9 && sol.points[2][0].abs() - 4.0 < 1e-9);
}

#[test]
fn a_repeated_constraint_is_redundant_not_wrong() {
    let mut s = rectangle(4.0, 3.0);
    s.constrain(Constraint::Horizontal { a: 0, b: 1 });
    s.constrain(Constraint::Distance { a: 0, b: 1, d: 4.0 });
    let sol = s.solve().unwrap();
    assert_eq!(sol.status, Status::Solved);
    assert_eq!((sol.dof, sol.redundant), (0, 2));
    // a rectangle's opposite sides being parallel is already implied
    let mut s = rectangle(4.0, 3.0);
    s.constrain(Constraint::Parallel { l1: 0, l2: 2 });
    assert_eq!(s.solve().unwrap().redundant, 1);
}

#[test]
fn conflicting_dimensions_are_reported_not_hidden() {
    let mut s = rectangle(4.0, 3.0);
    s.constrain(Constraint::Distance { a: 0, b: 1, d: 5.0 });
    let sol = s.solve().unwrap();
    assert!(matches!(sol.status, Status::Unsolved { residual } if residual > 0.1), "{:?}", sol.status);
    // fixed point forced to two places
    let mut s = Sketch::default();
    let p = s.point(0.0, 0.0);
    s.constrain(Constraint::Fixed { p, x: 1.0, y: 1.0 }).constrain(Constraint::Fixed { p, x: 2.0, y: 1.0 });
    assert!(matches!(s.solve().unwrap().status, Status::Unsolved { .. }));
}

#[test]
fn a_single_dimension_moves_both_points_the_least() {
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    let b = s.point(3.0, 0.0);
    s.constrain(Constraint::Distance { a, b, d: 5.0 });
    let sol = s.solve().unwrap();
    assert!(near(sol.points[0], -1.0, 0.0) && near(sol.points[1], 4.0, 0.0), "{:?}", sol.points);
    assert_eq!(sol.dof, 3);
    assert_eq!(sol.fixed, vec![false, false]);
}

#[test]
fn a_3_4_5_triangle_picks_the_side_it_was_drawn_on() {
    for (start, sign) in [((0.2, 2.5), 1.0), ((0.2, -2.5), -1.0)] {
        let mut s = Sketch::default();
        let p0 = s.point(0.0, 0.0);
        let p1 = s.point(4.2, 0.1);
        let p2 = s.point(start.0, start.1);
        s.constrain(Constraint::Fixed { p: p0, x: 0.0, y: 0.0 })
            .constrain(Constraint::Horizontal { a: p0, b: p1 })
            .constrain(Constraint::Distance { a: p0, b: p1, d: 4.0 })
            .constrain(Constraint::Distance { a: p0, b: p2, d: 3.0 })
            .constrain(Constraint::Distance { a: p1, b: p2, d: 5.0 });
        let sol = s.solve().unwrap();
        assert_eq!(sol.status, Status::Solved);
        assert!(near(sol.points[1], 4.0, 0.0) && near(sol.points[2], 0.0, 3.0 * sign), "{:?}", sol.points);
        assert_eq!((sol.dof, sol.redundant), (0, 0));
    }
}

#[test]
fn relations_one_by_one() {
    // perpendicular + equal length make a square corner
    let mut s = Sketch::default();
    let o = s.point(0.0, 0.0);
    let a = s.point(2.0, 0.1);
    let b = s.point(0.2, 1.0);
    let l1 = s.line(o, a);
    let l2 = s.line(o, b);
    s.constrain(Constraint::Fixed { p: o, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: a, x: 2.0, y: 0.0 })
        .constrain(Constraint::Perpendicular { l1, l2 }).constrain(Constraint::EqualLength { l1, l2 });
    let sol = s.solve().unwrap();
    assert!(near(sol.points[2], 0.0, 2.0), "{:?}", sol.points);
    assert_eq!((sol.dof, sol.redundant), (0, 0));

    // midpoint and point-on-line
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    let b = s.point(6.0, 2.0);
    let m = s.point(2.0, 2.0);
    let q = s.point(1.0, 5.0);
    let l = s.line(a, b);
    s.constrain(Constraint::Fixed { p: a, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: b, x: 6.0, y: 2.0 })
        .constrain(Constraint::Midpoint { p: m, l }).constrain(Constraint::PointOnLine { p: q, l }).constrain(Constraint::DistanceX { a, b: q, d: 3.0 });
    let sol = s.solve().unwrap();
    assert!(near(sol.points[2], 3.0, 1.0), "{:?}", sol.points);
    assert!(near(sol.points[3], 3.0, 1.0), "{:?}", sol.points);
    assert_eq!(sol.dof, 0);
    // DistanceY sign
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    let b = s.point(1.0, 1.0);
    s.constrain(Constraint::Fixed { p: a, x: 0.0, y: 0.0 }).constrain(Constraint::DistanceY { a, b, d: -2.0 });
    assert!((s.solve().unwrap().points[1][1] + 2.0).abs() < 1e-9);
    // coincident
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    let b = s.point(1.0, 1.0);
    s.constrain(Constraint::Fixed { p: a, x: 5.0, y: 6.0 }).constrain(Constraint::Coincident { a, b });
    assert!(near(s.solve().unwrap().points[1], 5.0, 6.0));
    // vertical and parallel
    let mut s = Sketch::default();
    let (a, b, c, d) = (s.point(0.0, 0.0), s.point(0.2, 3.0), s.point(2.0, 0.0), s.point(2.5, 3.0));
    let (l1, l2) = (s.line(a, b), s.line(c, d));
    s.constrain(Constraint::Fixed { p: a, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: b, x: 1.0, y: 2.0 }).constrain(Constraint::Fixed { p: c, x: 2.0, y: 0.0 })
        .constrain(Constraint::Parallel { l1, l2 }).constrain(Constraint::Vertical { a: d, b: d });
    let sol = s.solve().unwrap();
    // the parallel line points the same way as (1, 2)
    let (dx, dy) = (sol.points[3][0] - 2.0, sol.points[3][1]);
    assert!((dx * 2.0 - dy * 1.0).abs() < 1e-8);
}

#[test]
fn angles_are_counter_clockwise_between_lines() {
    let mut s = Sketch::default();
    let o = s.point(0.0, 0.0);
    let a = s.point(2.0, 0.0);
    let b = s.point(1.0, 1.5);
    let (l1, l2) = (s.line(o, a), s.line(o, b));
    s.constrain(Constraint::Fixed { p: o, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: a, x: 2.0, y: 0.0 })
        .constrain(Constraint::Angle { l1, l2, degrees: 60.0 }).constrain(Constraint::Distance { a: o, b, d: 2.0 });
    let sol = s.solve().unwrap();
    assert!(near(sol.points[2], 1.0, 3f64.sqrt()), "{:?}", sol.points);
    // drawn below the axis the other way round
    let mut s = Sketch::default();
    let o = s.point(0.0, 0.0);
    let a = s.point(2.0, 0.0);
    let b = s.point(1.0, -1.5);
    let (l1, l2) = (s.line(o, a), s.line(o, b));
    s.constrain(Constraint::Fixed { p: o, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: a, x: 2.0, y: 0.0 })
        .constrain(Constraint::Angle { l1, l2, degrees: -60.0 }).constrain(Constraint::Distance { a: o, b, d: 2.0 });
    assert!(near(s.solve().unwrap().points[2], 1.0, -(3f64.sqrt())));
}

#[test]
fn circles_radius_tangent_and_points_on_them() {
    // a line fixed at height 1.4 over a circle at the origin: tangency decides the radius
    let mut s = Sketch::default();
    let (c, a, b) = (s.point(0.0, 0.0), s.point(-2.0, 1.4), s.point(2.0, 1.3));
    let l = s.line(a, b);
    let ci = s.circle(c, 1.0);
    s.constrain(Constraint::Fixed { p: c, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: a, x: -2.0, y: 1.4 }).constrain(Constraint::Horizontal { a, b })
        .constrain(Constraint::Tangent { l, c: ci });
    let sol = s.solve().unwrap();
    assert_eq!(sol.status, Status::Solved);
    assert!((sol.radii[0] - 1.4).abs() < 1e-8, "{:?}", sol.radii);
    // radius dimension, point on circle, equal radius
    let mut s = Sketch::default();
    let (c1, c2, p) = (s.point(0.0, 0.0), s.point(5.0, 0.0), s.point(0.0, 2.2));
    let (k1, k2) = (s.circle(c1, 1.0), s.circle(c2, 3.0));
    s.constrain(Constraint::Fixed { p: c1, x: 0.0, y: 0.0 }).constrain(Constraint::Fixed { p: c2, x: 5.0, y: 0.0 })
        .constrain(Constraint::Radius { c: k1, r: 2.0 }).constrain(Constraint::EqualRadius { c1: k1, c2: k2 })
        .constrain(Constraint::PointOnCircle { p, c: k1 }).constrain(Constraint::Vertical { a: c1, b: p });
    let sol = s.solve().unwrap();
    assert_eq!(sol.radii.len(), 2);
    assert!((sol.radii[0] - 2.0).abs() < 1e-9 && (sol.radii[1] - 2.0).abs() < 1e-9);
    assert!(near(sol.points[2], 0.0, 2.0), "{:?}", sol.points);
    assert_eq!(sol.dof, 0);
}

#[test]
fn nothing_to_solve_changes_nothing() {
    let mut s = Sketch::default();
    s.point(1.5, -2.5);
    let sol = s.solve().unwrap();
    assert_eq!((sol.status.clone(), sol.dof, sol.redundant, sol.iterations), (Status::Solved, 2, 0, 0));
    assert_eq!(sol.points, vec![[1.5, -2.5]]);
    assert_eq!(sol.fixed, vec![false]);
    assert_eq!(Sketch::default().solve().unwrap().dof, 0);
    // solving a solved sketch again moves nothing
    let sol = rectangle(4.0, 3.0).solve().unwrap();
    let mut again = rectangle(4.0, 3.0);
    again.points = sol.points.clone();
    let sol2 = again.solve().unwrap();
    assert_eq!(sol2.iterations, 0);
    assert_eq!(sol2.points, sol.points);
}

#[test]
fn bad_input_is_refused_with_a_reason() {
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    let b = s.point(1.0, 0.0);
    s.line(a, b);
    let bad = |c: Constraint| { let mut t = s.clone(); t.constrain(c); t.solve().unwrap_err() };
    assert!(matches!(bad(Constraint::Horizontal { a: 0, b: 9 }), SketchError::BadIndex { constraint: 0, .. }));
    assert!(matches!(bad(Constraint::Parallel { l1: 0, l2: 1 }), SketchError::BadIndex { .. }));
    assert!(matches!(bad(Constraint::Radius { c: 0, r: 1.0 }), SketchError::BadIndex { .. }));
    assert!(matches!(bad(Constraint::Midpoint { p: 0, l: 3 }), SketchError::BadIndex { .. }));
    assert!(matches!(bad(Constraint::PointOnCircle { p: 0, c: 0 }), SketchError::BadIndex { .. }));
    assert!(matches!(bad(Constraint::Tangent { l: 0, c: 0 }), SketchError::BadIndex { .. }));
    assert!(matches!(bad(Constraint::EqualRadius { c1: 0, c2: 0 }), SketchError::BadIndex { .. }));
    assert!(matches!(bad(Constraint::Distance { a, b, d: 0.0 }), SketchError::BadValue { .. }));
    assert!(matches!(bad(Constraint::Distance { a, b, d: -1.0 }), SketchError::BadValue { .. }));
    assert!(matches!(bad(Constraint::Distance { a, b, d: f64::NAN }), SketchError::BadValue { .. }));
    assert!(matches!(bad(Constraint::Distance { a, b, d: 1.0e6 + 1.0 }), SketchError::BadValue { .. }));
    assert!(matches!(bad(Constraint::DistanceX { a, b, d: f64::INFINITY }), SketchError::BadValue { .. }));
    assert!(matches!(bad(Constraint::Fixed { p: a, x: 0.0, y: f64::NAN }), SketchError::BadValue { .. }));
    assert!(matches!(bad(Constraint::Angle { l1: 0, l2: 0, degrees: f64::NAN }), SketchError::BadValue { .. }));
    // exactly the largest allowed value is fine
    let mut t = s.clone();
    t.constrain(Constraint::Distance { a, b, d: 1.0e6 });
    assert!(t.solve().is_ok());
    assert!(s.clone().solve().is_ok());
    // geometry
    let mut t = s.clone();
    t.lines.push((a, a));
    assert_eq!(t.solve().unwrap_err(), SketchError::DegenerateLine(1));
    let mut t = s.clone();
    t.lines.push((a, 7));
    assert!(matches!(t.solve().unwrap_err(), SketchError::BadIndex { .. }));
    let mut t = s.clone();
    t.points[0] = [f64::NAN, 0.0];
    assert!(matches!(t.solve().unwrap_err(), SketchError::NotFinite(_)));
    let mut t = s.clone();
    t.points[1] = [0.0, 2.0e6];
    assert!(matches!(t.solve().unwrap_err(), SketchError::NotFinite(_)));
    let mut t = s.clone();
    t.circles.push(Circle { centre: 5, radius: 1.0 });
    assert!(matches!(t.solve().unwrap_err(), SketchError::BadIndex { .. }));
    for r in [0.0, -1.0, f64::NAN, 2.0e6] {
        let mut t = s.clone();
        t.circles.push(Circle { centre: 0, radius: r });
        assert!(matches!(t.solve().unwrap_err(), SketchError::NotFinite(_)));
    }
    // residuals needs the right number of coordinates
    assert!(s.residuals(&[0.0]).is_err());
    assert!(s.residuals(&[0.0; 4]).is_ok());
}

#[test]
fn limits() {
    let mut s = Sketch::default();
    for i in 0..MAX_POINTS { s.point(i as f64, 0.0); }
    assert!(s.solve().is_ok());
    s.point(0.0, 0.0);
    assert_eq!(s.solve().unwrap_err(), SketchError::TooMany("points"));
    let mut s = Sketch::default();
    let (a, b) = (s.point(0.0, 0.0), s.point(1.0, 0.0));
    for _ in 0..MAX_LINES { s.line(a, b); }
    assert!(s.solve().is_ok());
    s.line(a, b);
    assert_eq!(s.solve().unwrap_err(), SketchError::TooMany("lines"));
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    for _ in 0..MAX_CIRCLES { s.circle(a, 1.0); }
    assert!(s.solve().is_ok());
    s.circle(a, 1.0);
    assert_eq!(s.solve().unwrap_err(), SketchError::TooMany("circles"));
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    for _ in 0..MAX_CONSTRAINTS { s.constrain(Constraint::Fixed { p: a, x: 0.0, y: 0.0 }); }
    assert!(s.solve().is_ok());
    s.constrain(Constraint::Fixed { p: a, x: 0.0, y: 0.0 });
    assert_eq!(s.solve().unwrap_err(), SketchError::TooMany("constraints"));
}

// ---- property tests: exact derivatives, and consistent sketches always solve ----

fn arb_sketch() -> impl Strategy<Value = (Sketch, Vec<f64>)> {
    (prop::collection::vec((-10.0f64..10.0, -10.0f64..10.0), 5..9), prop::collection::vec((0usize..16, 0usize..6, 0usize..6, 0usize..6, 0usize..6), 1..14), prop::collection::vec((-0.05f64..0.05, -0.05f64..0.05), 9), 0.5f64..3.0)
        .prop_map(|(pts, cons, jitter, rad)| {
            let mut truth = Sketch::default();
            for p in &pts { truth.point(p.0, p.1); }
            let n = pts.len();
            for k in 0..n - 1 { truth.line(k, k + 1); }
            truth.line(0, n - 1);
            truth.circle(0, rad);
            truth.circle(1, rad * 1.3);
            let mut s = truth.clone();
            let tp = |i: usize| truth.points[i % n];
            for (kind, a, b, c, d) in cons {
                let (a, b, c) = (a % n, b % n, c % n);
                let _ = d;
                let nl = truth.lines.len();
                let (l1, l2) = (a % nl, c % nl);
                let dist = |p: usize, q: usize| ((tp(p)[0] - tp(q)[0]).powi(2) + (tp(p)[1] - tp(q)[1]).powi(2)).sqrt();
                let con = match kind {
                    0 => Constraint::Fixed { p: a, x: tp(a)[0], y: tp(a)[1] },
                    1 if a != b => Constraint::Distance { a, b, d: dist(a, b) },
                    2 => Constraint::DistanceX { a, b, d: tp(b)[0] - tp(a)[0] },
                    3 => Constraint::DistanceY { a, b, d: tp(b)[1] - tp(a)[1] },
                    4 => Constraint::Coincident { a: a.min(b), b: a.min(b) },
                    5 if l1 != l2 => {
                        let (p, q) = truth.lines[l1]; let (r, t) = truth.lines[l2];
                        let ang = (tp(t)[1] - tp(r)[1]).atan2(tp(t)[0] - tp(r)[0]) - (tp(q)[1] - tp(p)[1]).atan2(tp(q)[0] - tp(p)[0]);
                        Constraint::Angle { l1, l2, degrees: ang.to_degrees() }
                    }
                    6 if l1 != l2 => Constraint::EqualLength { l1, l2 },
                    7 => Constraint::Radius { c: a % 2, r: truth.circles[a % 2].radius },
                    8 => Constraint::EqualRadius { c1: 0, c2: 0 },
                    _ => Constraint::Horizontal { a, b: a },
                };
                // only keep constraints that are true in the truth drawing
                let ok = match con {
                    Constraint::EqualLength { l1, l2 } => { let (p, q) = truth.lines[l1]; let (r, t) = truth.lines[l2]; (dist(p, q) - dist(r, t)).abs() < 1e-12 }
                    _ => true,
                };
                if ok { s.constrain(con); }
            }
            let mut x: Vec<f64> = s.points.iter().enumerate().flat_map(|(i, p)| [p[0] + jitter[i % 9].0, p[1] + jitter[i % 9].1]).collect();
            x.extend(s.circles.iter().map(|c| c.radius * 1.01));
            (s, x)
        })
}

proptest! {
    #[test]
    fn derivatives_match_finite_differences((s, x) in arb_sketch()) {
        let (r0, j) = s.residuals(&x).unwrap();
        for col in 0..x.len() {
            let h = 1e-6;
            let (mut xp, mut xm) = (x.clone(), x.clone());
            xp[col] += h; xm[col] -= h;
            let (rp, _) = s.residuals(&xp).unwrap();
            let (rm, _) = s.residuals(&xm).unwrap();
            for row in 0..r0.len() {
                let num = (rp[row] - rm[row]) / (2.0 * h);
                prop_assert!((num - j[row][col]).abs() < 1e-4 * (1.0 + num.abs()), "row {} col {}: {} vs {}", row, col, num, j[row][col]);
            }
        }
    }

    #[test]
    fn a_sketch_made_from_a_real_drawing_solves_from_a_nearby_one((mut s, x) in arb_sketch()) {
        let np = s.points.len();
        for p in 0..np { s.points[p] = [x[2 * p], x[2 * p + 1]]; }
        for (i, c) in s.circles.iter_mut().enumerate() { c.radius = x[2 * np + i]; }
        let sol = s.solve().unwrap();
        prop_assert_eq!(sol.status.clone(), Status::Solved, "{:?}", sol.status);
        // every constraint really holds at the answer
        let mut y: Vec<f64> = sol.points.iter().flat_map(|p| [p[0], p[1]]).collect();
        y.extend(&sol.radii);
        let (r, _) = s.residuals(&y).unwrap();
        prop_assert!(r.iter().all(|v| v.abs() < 1e-8));
        prop_assert!(sol.dof + sol.redundant <= y.len() + r.len());
    }
}

#[test]
fn free_movements_stay_where_they_were_drawn() {
    // two free points 3 and 1 apart in x and y; ask for distance 5: both slide along their own line, equally
    let mut s = Sketch::default();
    let a = s.point(0.0, 0.0);
    let b = s.point(3.0, 1.0);
    s.constrain(Constraint::Distance { a, b, d: 5.0 });
    let sol = s.solve().unwrap();
    let len = 10f64.sqrt();
    let k = (5.0 - len) / 2.0 / len;
    assert!(near(sol.points[0], -3.0 * k, -k) && near(sol.points[1], 3.0 + 3.0 * k, 1.0 + k), "{:?}", sol.points);
    // a long sketch with one dimension changed: the other points do not move at all
    let mut s = rectangle(4.0, 3.0);
    s.constraints.pop();
    s.points[2] = [4.0, 2.2];
    s.points[3] = [0.0, 2.2];
    let sol = s.solve().unwrap();
    assert!(near(sol.points[2], 4.0, 2.2) && near(sol.points[3], 0.0, 2.2), "{:?}", sol.points);
}
