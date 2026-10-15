use openbim_family::forms::*;
use openbim_family::*;
use openbim_sketch::Constraint;

fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-8 * (1.0 + b.abs()) }

fn fam(values: Vec<ValueDef>, forms: Vec<FormDef>) -> Family { Family { name: "F".into(), values, forms, ..Default::default() } }
fn inp(n: &str, v: f64) -> ValueDef { ValueDef::input(n, v, 0.001, 50.0, Scope::Instance) }
fn solve(f: &Family, o: Vec<(&'static str, f64)>) -> Result<Vec<Form>, FamilyError> { f.compile()?.solve(&Request { overrides: o, ..Default::default() }).map(|i| i.forms) }
fn why(f: &Family, o: Vec<(&'static str, f64)>) -> String { match solve(f, o) { Err(FamilyError::Form { why, .. }) => why.to_string(), other => panic!("{other:?}") } }

fn rect_sketch() -> SketchDef {
    SketchDef::new(&[("0.1", "-0.1"), ("1.2", "0.1"), ("1.1", "1.2"), ("-0.1", "0.9")])
        .fixed(0, "0", "0")
        .rel(Constraint::Horizontal { a: 0, b: 1 }).rel(Constraint::Vertical { a: 1, b: 2 })
        .rel(Constraint::Horizontal { a: 2, b: 3 }).rel(Constraint::Vertical { a: 3, b: 0 })
        .distance(0, 1, "w").distance(1, 2, "d")
}

#[test]
fn parameters_drive_a_locked_rectangle() {
    let f = fam(vec![inp("w", 0.4), inp("d", 0.2), inp("h", 3.0)], vec![FormDef::extrusion("slab", Axis::Z, "0", "h", ProfileDef::sketch(rect_sketch()))]);
    let forms = solve(&f, vec![]).unwrap();
    assert!(close(forms[0].volume(), 0.4 * 0.2 * 3.0));
    let (lo, hi) = forms[0].bounds_local();
    assert!(close(lo[0], 0.0) && close(lo[1], 0.0) && close(hi[0], 0.4) && close(hi[1], 0.2) && close(hi[2], 3.0), "{lo:?} {hi:?}");
    // change the parameters: the sketch follows
    let forms = solve(&f, vec![("w", 1.5), ("d", 0.7), ("h", 2.0)]).unwrap();
    assert!(close(forms[0].volume(), 1.5 * 0.7 * 2.0));
    let (_, hi) = forms[0].bounds_local();
    assert!(close(hi[0], 1.5) && close(hi[1], 0.7));
    // a dimension that is a formula of others
    let g = fam(vec![inp("w", 0.4), inp("d", 0.2), inp("h", 1.0)], vec![FormDef::extrusion("slab", Axis::Z, "0", "h", ProfileDef::sketch(rect_sketch().distance(0, 1, "w").distance(1, 2, "w")))]);
    // two dimensions on w twice is the same value twice: redundant, still fine; but d is overridden by the second w/d pair
    assert!(matches!(solve(&g, vec![]), Err(FamilyError::Form { .. })), "w x w conflicts with w x d");
}

#[test]
fn an_l_bracket_with_a_thickness_parameter() {
    let l = SketchDef::new(&[("0", "0"), ("2.1", "0.1"), ("2", "0.3"), ("0.3", "0.2"), ("0.2", "1.9"), ("0.1", "2")])
        .fixed(0, "0", "0")
        .rel(Constraint::Horizontal { a: 0, b: 1 }).rel(Constraint::Vertical { a: 1, b: 2 }).rel(Constraint::Horizontal { a: 2, b: 3 })
        .rel(Constraint::Vertical { a: 3, b: 4 }).rel(Constraint::Horizontal { a: 4, b: 5 }).rel(Constraint::Vertical { a: 5, b: 0 })
        .distance(0, 1, "w").distance(1, 2, "t").distance(2, 3, "w - t").distance(5, 0, "hh");
    let f = fam(vec![inp("w", 2.0), inp("hh", 1.5), inp("t", 0.2), inp("depth", 0.05)], vec![FormDef::extrusion("bracket", Axis::Z, "0", "depth", ProfileDef::sketch(l))]);
    for (w, hh, t) in [(2.0, 1.5, 0.2), (3.0, 2.5, 0.35), (1.0, 1.0, 0.1)] {
        let forms = solve(&f, vec![("w", w), ("hh", hh), ("t", t)]).unwrap();
        let area = w * t + t * (hh - t);
        assert!(close(forms[0].profile_area(), area), "{} vs {area}", forms[0].profile_area());
        assert!(close(forms[0].volume(), area * 0.05));
        let (lo, hi) = forms[0].bounds_local();
        assert!(close(lo[0], 0.0) && close(lo[1], 0.0) && close(hi[0], w) && close(hi[1], hh), "{lo:?} {hi:?}");
    }
}

#[test]
fn an_angle_parameter_makes_a_parallelogram() {
    let sk = SketchDef::new(&[("0", "0"), ("2", "0"), ("2.7", "1.2"), ("0.6", "1.1")])
        .line(0, 3)
        .fixed(0, "0", "0").rel(Constraint::Horizontal { a: 0, b: 1 }).distance(0, 1, "w").distance(0, 3, "s").angle(0, 4, "theta")
        .rel(Constraint::Parallel { l1: 0, l2: 2 }).rel(Constraint::Parallel { l1: 1, l2: 3 });
    let f = fam(vec![inp("w", 2.0), inp("s", 1.0), ValueDef::angle("theta", 60.0, 5.0, 175.0, Scope::Instance)], vec![FormDef::extrusion("p", Axis::Z, "0", "1", ProfileDef::sketch(sk))]);
    for th in [60.0f64, 90.0, 45.0, 120.0] {
        let forms = solve(&f, vec![("theta", th)]).unwrap();
        assert!(close(forms[0].profile_area(), 2.0 * 1.0 * th.to_radians().sin()), "{th}: {}", forms[0].profile_area());
    }
}

#[test]
fn an_under_constrained_sketch_keeps_what_was_drawn() {
    let sk = SketchDef::new(&[("0", "0"), ("1", "0"), ("1", "0.6"), ("0", "0.6")])
        .fixed(0, "0", "0").rel(Constraint::Horizontal { a: 0, b: 1 }).rel(Constraint::Vertical { a: 1, b: 2 }).rel(Constraint::Horizontal { a: 2, b: 3 }).rel(Constraint::Vertical { a: 3, b: 0 })
        .distance(0, 1, "w");
    let f = fam(vec![inp("w", 1.0)], vec![FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::sketch(sk))]);
    let forms = solve(&f, vec![("w", 2.5)]).unwrap();
    assert!(close(forms[0].profile_area(), 2.5 * 0.6), "the free height stays as drawn: {}", forms[0].profile_area());
}

#[test]
fn bad_sketches_are_refused_with_a_reason() {
    let ext = |sk: SketchDef| fam(vec![inp("w", 1.0), inp("d", 1.0)], vec![FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::sketch(sk))]);
    // conflicting dimensions
    assert!(why(&ext(rect_sketch().distance(0, 1, "w + 1 m")), vec![]).contains("conflict"));
    // dimension below zero
    assert!(why(&ext(rect_sketch().distance(0, 1, "w - 1 m")), vec![]).contains("not a usable number"));
    assert!(why(&ext(rect_sketch().distance(0, 1, "0 - w")), vec![]).contains("not a usable number"));
    // an index that is not there
    assert!(why(&ext(rect_sketch().rel(Constraint::Horizontal { a: 0, b: 9 })), vec![]).contains("not there"));
    assert!(why(&ext(rect_sketch().line(0, 9)), vec![]).contains("not there"));
    assert!(why(&ext(rect_sketch().line(2, 2)), vec![]).contains("not there"));
    // the outline needs 3 points
    let two = SketchDef::new(&[("0", "0"), ("1", "0")]);
    assert!(matches!(ext(two).compile(), Err(FamilyError::Form { .. })));
    let mut sk = rect_sketch();
    sk.loop_len = 5;
    assert!(matches!(ext(sk).compile(), Err(FamilyError::Form { .. })));
    // unknown name in a dimension
    assert!(matches!(ext(rect_sketch().distance(0, 1, "nope")).compile(), Err(FamilyError::Formula { .. })));
    // a sketch that ends up crossing itself
    let bow = SketchDef::new(&[("0", "0"), ("1", "1"), ("1", "0"), ("0", "1")]);
    assert!(why(&ext(bow), vec![]).contains("crosses or touches itself"));
    // limits
    let many = |n: usize| { let pts: Vec<(String, String)> = (0..n).map(|i| { let a = std::f64::consts::TAU * i as f64 / n as f64; (format!("{}", a.cos()), format!("{}", a.sin())) }).collect(); SketchDef { points: pts, loop_len: n, lines: vec![], constraints: vec![] } };
    assert!(ext(many(200)).compile().is_ok());
    assert_eq!(ext(many(201)).compile().err(), Some(FamilyError::TooMany("points in a profile")));
    let mut sk = rect_sketch();
    sk.constraints = (0..801).map(|_| SketchCon::Rel(Constraint::Horizontal { a: 0, b: 1 })).collect();
    assert_eq!(ext(sk).compile().err(), Some(FamilyError::TooMany("sketch constraints")));
    let mut sk = rect_sketch();
    sk.lines = vec![(0, 1); 397];
    assert_eq!(ext(sk).compile().err(), Some(FamilyError::TooMany("lines in a sketch")));
    let mut sk = rect_sketch();
    sk.lines = vec![(0, 1); 396];
    assert!(ext(sk).compile().is_ok());
}

#[test]
fn a_sketch_hole_and_a_sketch_in_a_nested_family() {
    let hole = SketchDef::new(&[("0.2", "0.2"), ("0.5", "0.2"), ("0.5", "0.5"), ("0.2", "0.5")])
        .fixed(0, "0.2", "0.2").rel(Constraint::Horizontal { a: 0, b: 1 }).rel(Constraint::Vertical { a: 1, b: 2 }).rel(Constraint::Horizontal { a: 2, b: 3 }).rel(Constraint::Vertical { a: 3, b: 0 })
        .distance(0, 1, "hw").distance(1, 2, "hw");
    let plate = fam(vec![inp("w", 1.0), inp("d", 1.0), inp("hw", 0.3)], vec![FormDef::extrusion("plate", Axis::Z, "0", "0.1", ProfileDef::sketch(rect_sketch())).hole(ProfileDef::sketch(hole))]);
    let forms = solve(&plate, vec![("hw", 0.4)]).unwrap();
    assert!(close(forms[0].profile_area(), 1.0 - 0.16), "{}", forms[0].profile_area());
    // a hole that grows out of the plate is refused
    assert!(why(&plate, vec![("hw", 0.9)]).contains("must lie inside"));
    // nested: the parent places the plate turned by 90 degrees; the form comes up with the sketch already solved
    let child = Family { name: "Plate".into(), ..plate.clone() };
    let parent = Family { name: "P".into(), nested: vec![NestedDef::new("n", "Plate", ("5", "0", "0")).rotate_about("90", "0", "0")], ..Default::default() };
    let c = compile_library(&[child, parent]).unwrap().remove(1);
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.forms.len(), 1);
    let (lo, hi, exact) = i.forms[0].bounds();
    assert!(exact && close(lo[0], -1.0) && close(hi[0], 0.0) && close(lo[1], 5.0) && close(hi[1], 6.0), "{lo:?} {hi:?}");
    assert!(close(i.forms[0].volume(), (1.0 - 0.09) * 0.1));
}

#[test]
fn offsets_angles_construction_points_and_the_100_m_limit() {
    let ext = |sk: SketchDef| fam(vec![inp("w", 1.0)], vec![FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::sketch(sk))]);
    // dimensions along x and y, a lock away from the origin where x and y differ
    let sk = SketchDef::new(&[("2.1", "3.1"), ("3.4", "3.0"), ("3.5", "4.6"), ("2.0", "4.4")])
        .fixed(0, "2", "3").rel(Constraint::Horizontal { a: 0, b: 1 }).rel(Constraint::Vertical { a: 1, b: 2 }).rel(Constraint::Horizontal { a: 2, b: 3 }).rel(Constraint::Vertical { a: 3, b: 0 })
        .distance_x(0, 1, "1.5").distance_y(1, 2, "0.5 * w");
    let forms = solve(&ext(sk), vec![("w", 3.0)]).unwrap();
    let (lo, hi) = forms[0].bounds_local();
    assert!(close(lo[0], 2.0) && close(lo[1], 3.0) && close(hi[0], 3.5) && close(hi[1], 4.5), "{lo:?} {hi:?}");
    // the sign of an angle: the second line is above the first for a positive angle
    let sk = SketchDef::new(&[("0", "0"), ("2", "0"), ("1", "1")]).line(0, 2)
        .fixed(0, "0", "0").fixed(1, "2", "0").angle(0, 3, "45").distance(0, 2, "1.5");
    let forms = solve(&ext(sk), vec![]).unwrap();
    let Loop::Poly(p) = &forms[0].outer else { panic!() };
    assert!(p.iter().any(|q| close(q[0], 1.5 * 0.5f64.sqrt()) && close(q[1], 1.5 * 0.5f64.sqrt())), "{p:?}");
    // construction points do not become corners
    let sk = rect_sketch().extra_point("5", "5").fixed(4, "7", "8");
    let forms = solve(&fam(vec![inp("w", 0.4), inp("d", 0.2)], vec![FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::sketch(sk))]), vec![]).unwrap();
    let Loop::Poly(p) = &forms[0].outer else { panic!() };
    assert_eq!(p.len(), 4);
    assert!(close(forms[0].profile_area(), 0.08));
    // the answer must stay inside 100 m
    let mut far = rect_sketch();
    far.constraints[0] = SketchCon::Fixed { p: 0, x: "99".into(), y: "0".into() };
    let msg = why(&fam(vec![inp("w", 5.0), inp("d", 1.0)], vec![FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::sketch(far))]), vec![]);
    assert!(msg.contains("ends outside 100 m"), "{msg}");
    // exactly 800 constraints is allowed
    let mut sk = rect_sketch();
    sk.constraints = (0..800).map(|_| SketchCon::Rel(Constraint::Horizontal { a: 0, b: 1 })).collect();
    assert!(ext(sk).compile().is_ok());
}
