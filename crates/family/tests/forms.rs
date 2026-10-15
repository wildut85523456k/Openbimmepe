use openbim_family::forms::*;
use openbim_family::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 * (1.0 + b.abs()) }
fn close3(a: [f64; 3], b: [f64; 3]) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9) }

fn one(f: FormDef) -> Family { Family { name: "F".into(), forms: vec![f], ..Default::default() } }
fn solve(f: FormDef) -> Result<Vec<Form>, FamilyError> { one(f).compile()?.solve(&Request::default()).map(|i| i.forms) }
fn ok(f: FormDef) -> Form { solve(f).unwrap().remove(0) }
fn why(f: FormDef) -> String { match solve(f) { Err(FamilyError::Form { why, .. }) => why.to_string(), other => panic!("{other:?}") } }

fn rect(u0: &str, v0: &str, u1: &str, v1: &str) -> ProfileDef { ProfileDef::rect(u0, v0, u1, v1) }

#[test]
fn extrusion_volumes_and_bounds() {
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "3", rect("0", "0", "0.4", "0.2")));
    assert!(close(f.volume(), 0.24) && close(f.profile_area(), 0.08));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [0.0, 0.0, 0.0]) && close3(hi, [0.4, 0.2, 3.0]));
    // L shape, clockwise on input: stored counter-clockwise, same area
    let l = [("0", "0"), ("0", "2"), ("1", "2"), ("1", "1"), ("2", "1"), ("2", "0")];
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "0.5", ProfileDef::polygon(&l)));
    assert!(close(f.volume(), 1.5));
    match &f.outer { Loop::Poly(p) => assert!(p.windows(2).count() == 5 && (0..6).map(|i| p[i][0] * p[(i + 1) % 6][1] - p[(i + 1) % 6][0] * p[i][1]).sum::<f64>() > 0.0), _ => panic!() }
    // circle and pipe
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "2", ProfileDef::circle("0", "0", "0.1")));
    assert!(close(f.volume(), PI * 0.01 * 2.0));
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "3", ProfileDef::circle("0", "0", "0.05")).hole(ProfileDef::circle("0", "0", "0.04")));
    assert!(close(f.volume(), PI * (0.0025 - 0.0016) * 3.0));
    // the other work planes
    let f = ok(FormDef::extrusion("e", Axis::X, "5", "6", rect("1", "0", "2", "1")));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [5.0, 1.0, 0.0]) && close3(hi, [6.0, 2.0, 1.0]));
    let f = ok(FormDef::extrusion("e", Axis::Y, "7", "8", rect("0", "3", "1", "4")));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [3.0, 7.0, 0.0]) && close3(hi, [4.0, 8.0, 1.0]));
    // a circle with a polygon hole, and a polygon with a circle hole
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::circle("0", "0", "1")).hole(rect("-0.5", "-0.5", "0.5", "0.5")));
    assert!(close(f.volume(), PI - 1.0));
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "1", rect("0", "0", "2", "2")).hole(ProfileDef::circle("1", "1", "0.5")));
    assert!(close(f.volume(), 4.0 - PI * 0.25));
    // two holes
    let f = ok(FormDef::extrusion("e", Axis::Z, "0", "1", rect("0", "0", "4", "2")).hole(rect("0.5", "0.5", "1.5", "1.5")).hole(ProfileDef::circle("3", "1", "0.5")));
    assert!(close(f.volume(), 8.0 - 1.0 - PI * 0.25));
}

#[test]
fn formulas_drive_forms() {
    let fam = Family {
        name: "P".into(),
        values: vec![ValueDef::input("d", 0.1, 0.01, 1.0, Scope::Instance), ValueDef::input("len", 2.0, 0.1, 10.0, Scope::Instance), ValueDef::flag("with_pipe", true, Scope::Instance), ValueDef::material("pipe_mat", "Steel", Scope::Type)],
        forms: vec![FormDef::extrusion("pipe", Axis::Z, "0", "len", ProfileDef::circle("0", "0", "d / 2")).visible_if("with_pipe").material("pipe_mat")],
        ..Default::default()
    };
    let c = fam.compile().unwrap();
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.forms.len(), 1);
    assert!(close(i.forms[0].volume(), PI * 0.0025 * 2.0));
    assert_eq!(i.strings[i.form_materials[0].unwrap() as usize], "Steel");
    assert_eq!(c.form_name(0), "pipe");
    let i = c.solve(&Request { overrides: vec![("d", 0.2), ("len", 5.0)], ..Default::default() }).unwrap();
    assert!(close(i.forms[0].volume(), PI * 0.01 * 5.0));
    let i = c.solve(&Request { overrides: vec![("with_pipe", 0.0)], ..Default::default() }).unwrap();
    assert!(i.forms.is_empty() && i.form_materials.is_empty());
}

#[test]
fn revolve_volumes_and_bounds() {
    let cyl = |angle: &str| ok(FormDef::revolve("r", Axis::Z, "0", true, "0", angle, rect("0", "0", "0.5", "2")));
    let f = cyl("360");
    assert!(close(f.volume(), PI * 0.25 * 2.0));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [-0.5, 0.0, -0.5]) && close3(hi, [0.5, 2.0, 0.5]));
    // turning is right-handed about +y: x = r cos, z = -r sin
    let f = cyl("180");
    assert!(close(f.volume(), PI * 0.25));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [-0.5, 0.0, -0.5]) && close3(hi, [0.5, 2.0, 0.0]), "{lo:?} {hi:?}");
    let f = cyl("90");
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [0.0, 0.0, -0.5]) && close3(hi, [0.5, 2.0, 0.0]));
    let f = cyl("270");
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [-0.5, 0.0, -0.5]) && close3(hi, [0.5, 2.0, 0.5]));
    let f = cyl("45");
    let h = 0.5 * 45f64.to_radians().cos();
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [0.0, 0.0, -0.5 * 45f64.to_radians().sin()]) && close3(hi, [0.5, 2.0, 0.0]) && h > 0.0);
    // a cone
    let f = ok(FormDef::revolve("r", Axis::Z, "0", true, "0", "360", ProfileDef::polygon(&[("0", "0"), ("0.5", "0"), ("0", "1")])));
    assert!(close(f.volume(), PI * 0.25 / 3.0));
    // a torus: circle of radius 0.1 around an axis 1 away
    let f = ok(FormDef::revolve("r", Axis::Z, "0", true, "0", "360", ProfileDef::circle("1", "0", "0.1")));
    assert!(close(f.volume(), 2.0 * PI * PI * 1.0 * 0.01));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [-1.1, -0.1, -1.1]) && close3(hi, [1.1, 0.1, 1.1]));
    // the axis away from the profile
    let f = ok(FormDef::revolve("r", Axis::Z, "0", true, "-1", "360", rect("0", "0", "1", "1")));
    assert!(close(f.volume(), 2.0 * PI * 1.0 * 1.5));
    // about the u direction
    let f = ok(FormDef::revolve("r", Axis::Z, "0", false, "0", "360", rect("0", "1", "3", "2")));
    assert!(close(f.volume(), 2.0 * PI * 3.0 * 1.5));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [0.0, -2.0, -2.0]) && close3(hi, [3.0, 2.0, 2.0]));
    // profile on the other side of the axis gives the same volume
    let f = ok(FormDef::revolve("r", Axis::Z, "0", true, "0", "360", rect("-0.5", "0", "0", "2")));
    assert!(close(f.volume(), PI * 0.25 * 2.0));
    // a work plane with normal X: profile (y, z) about the line y = 0 (parallel to z) at x = 4
    let f = ok(FormDef::revolve("r", Axis::X, "4", true, "0", "360", rect("0", "0", "1", "2")));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [3.0, -1.0, 0.0]) && close3(hi, [5.0, 1.0, 2.0]), "{lo:?} {hi:?}");
    // work plane with normal Y: (u, v) = (z, x); axis parallel to v (the x direction) at u = 0 (z = 0)
    let f = ok(FormDef::revolve("r", Axis::Y, "9", true, "0", "360", rect("0", "0", "1", "2")));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [0.0, 8.0, -1.0]) && close3(hi, [2.0, 10.0, 1.0]), "{lo:?} {hi:?}");
}

#[test]
fn revolve_errors() {
    let r = |a: &str, angle: &str| FormDef::revolve("r", Axis::Z, "0", true, a, angle, rect("0", "0", "1", "1"));
    assert!(solve(r("0", "360")).is_ok());
    assert!(solve(r("1", "360")).is_ok(), "touching the axis along an edge is fine");
    assert!(solve(r("0", "0.001")).is_ok());
    assert!(why(r("0.5", "360")).contains("crosses the axis"));
    assert!(why(r("0", "0")).contains("angle"));
    assert!(why(r("0", "-5")).contains("angle"));
    assert!(why(r("0", "360.001")).contains("angle"));
    // about u: profile below and above
    let f = FormDef::revolve("r", Axis::Z, "0", false, "0.5", "360", rect("0", "0", "1", "1"));
    assert!(why(f).contains("crosses the axis"));
}

fn sweep(path: &[(&str, &str)], p: ProfileDef) -> FormDef { FormDef::sweep("s", "1", path, p) }

#[test]
fn sweep_volumes_follow_the_mitre_formula() {
    let a = 0.2 * 0.1;
    let r = |u0: &str, u1: &str| rect(u0, "-0.05", u1, "0.05");
    // straight pipe
    let f = ok(sweep(&[("0", "0"), ("3", "0")], ProfileDef::circle("0", "0", "0.05")));
    assert!(close(f.volume(), PI * 0.0025 * 3.0));
    let (lo, hi) = f.bounds_local();
    assert!(close3(lo, [0.0, -0.05, 0.95]) && close3(hi, [3.0, 0.05, 1.05]));
    // left turn of 90 degrees; profile centred, then on the inside (left), then on the outside (right)
    let l = [("0", "0"), ("4", "0"), ("4", "3")];
    assert!(close(ok(sweep(&l, r("-0.1", "0.1"))).volume(), a * 7.0));
    assert!(close(ok(sweep(&l, r("0", "0.2"))).volume(), a * (7.0 - 0.2)));
    assert!(close(ok(sweep(&l, r("-0.2", "0"))).volume(), a * (7.0 + 0.2)));
    let (lo, hi) = ok(sweep(&l, r("0", "0.2"))).bounds_local();
    assert!(close3(lo, [0.0, 0.0, 0.95]) && close3(hi, [4.0, 3.0, 1.05]), "{lo:?} {hi:?}");
    let (lo, hi) = ok(sweep(&l, r("-0.2", "0"))).bounds_local();
    assert!(close3(lo, [0.0, -0.2, 0.95]) && close3(hi, [4.2, 3.0, 1.05]), "{lo:?} {hi:?}");
    // right turn: the left side is now the outside
    let rt = [("0", "0"), ("4", "0"), ("4", "-3")];
    assert!(close(ok(sweep(&rt, r("0", "0.2"))).volume(), a * (7.0 + 0.2)));
    assert!(close(ok(sweep(&rt, r("-0.2", "0"))).volume(), a * (7.0 - 0.2)));
    // two corners
    let u = [("0", "0"), ("4", "0"), ("4", "3"), ("1", "3")];
    assert!(close(ok(sweep(&u, r("0", "0.2"))).volume(), a * (10.0 - 0.4)));
    // an S: left then right cancel
    let s = [("0", "0"), ("4", "0"), ("4", "3"), ("8", "3")];
    assert!(close(ok(sweep(&s, r("0", "0.2"))).volume(), a * 11.0));
    // 45 degrees
    let d = [("0", "0"), ("4", "0"), ("6", "2")];
    let t = (22.5f64).to_radians().tan();
    assert!(close(ok(sweep(&d, r("0", "0.2"))).volume(), a * (4.0 + 8f64.sqrt() - 0.2 * t)));
    // a hole in the profile
    let f = ok(sweep(&l, r("-0.1", "0.1")).hole(rect("-0.05", "-0.02", "0.05", "0.02")));
    assert!(close(f.volume(), (a - 0.1 * 0.04) * 7.0));
    // moment of a net profile with an off-centre hole is used for the corner
    let f = ok(sweep(&l, r("-0.1", "0.1")).hole(rect("0.02", "-0.02", "0.08", "0.02")));
    let net = a - 0.06 * 0.04;
    let mu = 0.0 - (0.06 * 0.04) * 0.05;
    assert!(close(f.volume(), net * 7.0 - 2.0 * mu));
}

#[test]
fn sweep_errors() {
    let r = rect("-0.1", "-0.05", "0.1", "0.05");
    assert!(why(sweep(&[("0", "0"), ("0", "0")], r.clone())).contains("coincide"));
    assert!(why(sweep(&[("0", "0"), ("4", "0"), ("4", "0.0000001")], r.clone())).contains("coincide"));
    // corners of 150 degrees are accepted when there is room, 151 are not
    let turn = |deg: f64| { let a = deg.to_radians(); [("0".to_string(), "0".to_string()), ("10".to_string(), "0".to_string()), (format!("{}", 10.0 + 10.0 * a.cos()), format!("{}", 10.0 * a.sin()))] };
    fn path(p: &[(String, String)]) -> Vec<(&str, &str)> { p.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect() }
    let p150 = turn(150.0); let p151 = turn(151.0);
    assert!(solve(sweep(&path(&p150), rect("-0.01", "-0.01", "0.01", "0.01"))).is_ok());
    assert!(why(sweep(&path(&p151), rect("-0.01", "-0.01", "0.01", "0.01"))).contains("150"));
    // too wide for a corner: u 0.5 with turn 90 needs 0.5 either side
    let narrow = [("0", "0"), ("4", "0"), ("4", "0.9"), ("8", "0.9")];
    assert!(why(sweep(&narrow, rect("-0.5", "-0.05", "0.5", "0.05"))).contains("too wide"));
    assert!(solve(sweep(&narrow, rect("-0.4", "-0.05", "0.4", "0.05"))).is_ok());
    // the path crosses itself
    let x = [("0", "0"), ("4", "0"), ("4", "2"), ("2", "2"), ("2", "-1")];
    assert!(why(sweep(&x, rect("-0.01", "-0.01", "0.01", "0.01"))).contains("crosses itself"));
    // a path that only touches itself is refused too
    let touch = [("0", "0"), ("4", "0"), ("4", "2"), ("2", "2"), ("2", "0")];
    assert!(why(sweep(&touch, rect("-0.01", "-0.01", "0.01", "0.01"))).contains("crosses itself"));
    // too few and too many points
    assert!(matches!(one(sweep(&[("0", "0")], r.clone())).compile(), Err(FamilyError::Form { .. })));
    let many = |n: usize| (0..n).map(|i| (format!("{}", i), if i % 2 == 0 { "0".to_string() } else { "0.001".to_string() })).collect::<Vec<_>>();
    let m64 = many(64); let m65 = many(65);
    assert!(one(sweep(&path(&m64), r.clone())).compile().is_ok());
    assert_eq!(one(sweep(&path(&m65), r.clone())).compile().err(), Some(FamilyError::TooMany("points in a sweep path")));
}

#[test]
fn profile_errors() {
    let ext = |p: ProfileDef| FormDef::extrusion("e", Axis::Z, "0", "1", p);
    assert!(matches!(one(ext(ProfileDef::polygon(&[("0", "0"), ("1", "0")]))).compile(), Err(FamilyError::Form { .. })));
    assert!(why(ext(ProfileDef::polygon(&[("0", "0"), ("1", "1"), ("1", "0"), ("0", "1")]))).contains("crosses or touches itself"));
    assert!(why(ext(ProfileDef::polygon(&[("0", "0"), ("1", "0"), ("1", "0"), ("1", "1")]))).contains("crosses or touches itself"));
    assert!(why(ext(ProfileDef::polygon(&[("0", "0"), ("1", "0"), ("2", "0")]))).contains("crosses or touches itself"));
    assert!(why(ext(ProfileDef::polygon(&[("0", "0"), ("1", "0"), ("0.5", "0")]))).contains("crosses or touches itself"));
    // a spike that folds back on itself
    assert!(why(ext(ProfileDef::polygon(&[("0", "0"), ("2", "0"), ("1", "0"), ("1", "1")]))).contains("crosses or touches itself"));
    // a vertex touching an edge
    assert!(why(ext(ProfileDef::polygon(&[("0", "0"), ("2", "0"), ("2", "2"), ("1", "0"), ("0", "2")]))).contains("crosses or touches itself"));
    assert!(why(ext(ProfileDef::circle("0", "0", "0"))).contains("too small"));
    assert!(why(ext(ProfileDef::circle("0", "0", "-1"))).contains("too small"));
    assert!(why(ext(ProfileDef::circle("0", "0", "0.00005"))).contains("too small"));
    assert!(solve(ext(ProfileDef::circle("0", "0", "0.0001"))).is_ok());
    assert!(solve(ext(ProfileDef::circle("0", "0", "0.01"))).is_ok());
    assert!(why(ext(rect("0", "0", "5e-5", "1e-4"))).contains("no area"));
    // coordinates
    assert!(solve(ext(rect("0", "0", "100", "1"))).is_ok());
    assert!(why(ext(rect("0", "0", "100.5", "1"))).contains("100 m"));
    // extrusion length
    assert!(why(FormDef::extrusion("e", Axis::Z, "1", "1", rect("0", "0", "1", "1"))).contains("positive length"));
    assert!(why(FormDef::extrusion("e", Axis::Z, "1", "0", rect("0", "0", "1", "1"))).contains("positive length"));
    assert!(why(FormDef::extrusion("e", Axis::Z, "0", "0.0000001", rect("0", "0", "1", "1"))).contains("positive length"));
    assert!(solve(FormDef::extrusion("e", Axis::Z, "0", "0.001", rect("0", "0", "1", "1"))).is_ok());
}

#[test]
fn hole_rules() {
    let ext = |holes: Vec<ProfileDef>| { let mut f = FormDef::extrusion("e", Axis::Z, "0", "1", rect("0", "0", "4", "4")); f.holes = holes; f };
    assert!(solve(ext(vec![rect("1", "1", "2", "2")])).is_ok());
    for bad in [rect("-1", "1", "2", "2"), rect("0", "1", "2", "2"), rect("3", "3", "5", "5"), rect("5", "5", "6", "6")] {
        assert!(why(ext(vec![bad])).contains("must lie inside"));
    }
    assert!(why(ext(vec![ProfileDef::circle("0.5", "2", "0.5")])).contains("must lie inside"));
    assert!(why(ext(vec![ProfileDef::circle("0.4", "2", "0.5")])).contains("must lie inside"));
    assert!(solve(ext(vec![ProfileDef::circle("0.6", "2", "0.5")])).is_ok());
    // holes may not overlap, touch or nest
    assert!(why(ext(vec![rect("1", "1", "2", "2"), rect("1.5", "1.5", "3", "3")])).contains("holes must not"));
    assert!(why(ext(vec![rect("1", "1", "2", "2"), rect("2", "1", "3", "2")])).contains("holes must not"));
    assert!(why(ext(vec![rect("1", "1", "3", "3"), rect("1.5", "1.5", "2", "2")])).contains("holes must not"));
    assert!(why(ext(vec![ProfileDef::circle("2", "2", "0.5"), ProfileDef::circle("2.5", "2", "0.5")])).contains("holes must not"));
    assert!(why(ext(vec![ProfileDef::circle("2", "2", "1"), ProfileDef::circle("2", "2", "0.3")])).contains("holes must not") );
    assert!(why(ext(vec![ProfileDef::circle("1.5", "1.5", "0.6"), rect("1", "1", "2", "2")])).contains("holes must not"));
    assert!(why(ext(vec![rect("1", "1", "3", "3"), ProfileDef::circle("2", "2", "0.2")])).contains("holes must not"));
    assert!(solve(ext(vec![ProfileDef::circle("1", "1", "0.4"), ProfileDef::circle("3", "3", "0.4"), rect("1", "2.5", "1.5", "3.5")])).is_ok());
    assert!(solve(ext(vec![ProfileDef::circle("1", "1", "0.5"), ProfileDef::circle("2.1", "1", "0.5")])).is_ok());
    // circle outer, circle hole
    let mut f = FormDef::extrusion("e", Axis::Z, "0", "1", ProfileDef::circle("0", "0", "1"));
    f.holes = vec![ProfileDef::circle("0.1", "0", "0.9")];
    assert!(why(f.clone()).contains("must lie inside"));
    f.holes = vec![ProfileDef::circle("0.05", "0", "0.9")];
    assert!(solve(f).is_ok());
    // limits
    let many = |n: usize| (0..n).map(|i| ProfileDef::circle(&format!("{}", 0.2 + 0.2 * (i % 17) as f64), &format!("{}", 0.2 + 0.2 * (i / 17) as f64), "0.05")).collect::<Vec<_>>();
    let big = |n: usize| { let mut f = FormDef::extrusion("e", Axis::Z, "0", "1", rect("0", "0", "5", "5")); f.holes = many(n); f };
    assert!(one(big(16)).compile().is_ok());
    assert_eq!(one(big(17)).compile().err(), Some(FamilyError::TooMany("holes in a profile")));
    let pts = |n: usize| (0..n).map(|i| { let a = 2.0 * PI * i as f64 / n as f64; (format!("{}", a.cos()), format!("{}", a.sin())) }).collect::<Vec<_>>();
    let poly = |n: usize| { let p = pts(n); ProfileDef::polygon(&p.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect::<Vec<_>>()) };
    assert!(solve(FormDef::extrusion("e", Axis::Z, "0", "1", poly(256))).is_ok());
    assert_eq!(one(FormDef::extrusion("e", Axis::Z, "0", "1", poly(257))).compile().err(), Some(FamilyError::TooMany("points in a profile")));
}

#[test]
fn naming_and_limits() {
    let e = |n: &str| FormDef::extrusion(n, Axis::Z, "0", "1", rect("0", "0", "1", "1"));
    assert_eq!(one(e("1bad")).compile().err(), Some(FamilyError::BadName("1bad".into())));
    let f = Family { name: "F".into(), forms: vec![e("a"), e("a")], ..Default::default() };
    assert_eq!(f.compile().err(), Some(FamilyError::DuplicateName("a".into())));
    let mk = |n: usize| Family { name: "F".into(), forms: (0..n).map(|i| e(&format!("f{i}"))).collect(), ..Default::default() };
    assert!(mk(200).compile().is_ok());
    assert_eq!(mk(201).compile().err(), Some(FamilyError::TooMany("forms")));
    // unknown name in a formula, text value in a formula
    assert!(matches!(one(FormDef::extrusion("e", Axis::Z, "0", "nope", rect("0", "0", "1", "1"))).compile(), Err(FamilyError::Formula { .. })));
    let mut fam = one(e("e").material("nope"));
    assert!(matches!(fam.compile(), Err(FamilyError::UnknownValue(_))));
    fam.values = vec![ValueDef::input("nope", 1.0, 0.0, 2.0, Scope::Type)];
    assert!(matches!(fam.compile(), Err(FamilyError::WrongKind { .. })));
}

#[test]
fn nested_families_carry_their_forms() {
    let child = Family { name: "Child".into(), forms: vec![FormDef::extrusion("ext", Axis::Z, "0", "1", rect("0", "0", "2", "1")), FormDef::revolve("rev", Axis::Z, "0", true, "0", "360", rect("0", "0", "0.5", "1"))], ..Default::default() };
    let p = |nd: NestedDef| compile_library(&[child.clone(), Family { name: "P".into(), nested: vec![nd], forms: vec![FormDef::extrusion("own", Axis::Z, "0", "1", rect("0", "0", "1", "1"))], ..Default::default() }]).unwrap().remove(1);
    let c = p(NestedDef::new("n", "Child", ("10", "20", "5")));
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.forms.len(), 3);
    assert_eq!((c.form_name(0), c.form_name(1), c.form_name(2)), ("own", "n/ext", "n/rev"));
    assert_eq!(i.forms.iter().map(|f| f.index).collect::<Vec<_>>(), vec![0, 1, 2]);
    let (lo, hi, exact) = i.forms[1].bounds();
    assert!(exact && close3(lo, [10.0, 20.0, 5.0]) && close3(hi, [12.0, 21.0, 6.0]));
    let (lo, hi, _) = i.forms[2].bounds();
    assert!(close3(lo, [9.5, 20.0, 4.5]) && close3(hi, [10.5, 21.0, 5.5]), "{lo:?} {hi:?}");
    // turned by 90 degrees about the origin: (x, y) -> (-y, x)
    let c = p(NestedDef::new("n", "Child", ("10", "20", "5")).rotate_about("90", "0", "0"));
    let i = c.solve(&Request::default()).unwrap();
    let (lo, hi, exact) = i.forms[1].bounds();
    assert!(exact && close3(lo, [-21.0, 10.0, 5.0]) && close3(hi, [-20.0, 12.0, 6.0]), "{lo:?} {hi:?}");
    assert!(close(i.forms[1].volume(), 2.0));
    // not a right angle: the box is a bound, not exact
    let c = p(NestedDef::new("n", "Child", ("0", "0", "0")).rotate_about("45", "0", "0"));
    let (lo, hi, exact) = c.solve(&Request::default()).unwrap().forms[1].bounds();
    assert!(!exact);
    let s = 0.5f64.sqrt();
    assert!(close3(lo, [-s, 0.0, 0.0]) && close3(hi, [2.0 * s, 3.0 * s, 1.0]), "{lo:?} {hi:?}");
    // repeated
    let c = p(NestedDef::new("n", "Child", ("_i * 3", "0", "0")).repeat("3"));
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.forms.len(), 7);
    assert_eq!(i.forms.iter().map(|f| f.index).collect::<Vec<_>>(), vec![0, 1, 2, 1, 2, 1, 2]);
    let (lo, _, _) = i.forms[5].bounds();
    assert!(close(lo[0], 6.0));
    // hidden
    let c = p(NestedDef::new("n", "Child", ("0", "0", "0")).visible_if("0"));
    assert_eq!(c.solve(&Request::default()).unwrap().forms.len(), 1);
}

#[test]
fn nested_materials_and_limits() {
    let child = Family { name: "Child".into(), values: vec![ValueDef::material("m", "Brass", Scope::Type)], forms: vec![FormDef::extrusion("ext", Axis::Z, "0", "1", rect("0", "0", "1", "1")).material("m")], ..Default::default() };
    let parent = Family { name: "P".into(), nested: vec![NestedDef::new("n", "Child", ("0", "0", "0")).set_text("m", TextSrc::Literal("Copper".into()))], ..Default::default() };
    let c = compile_library(&[child.clone(), parent]).unwrap().remove(1);
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.strings[i.form_materials[0].unwrap() as usize], "Copper");
    // limit across nesting: 5 forms x 1000 repeats = 5000 fits, 5001 does not
    let five = Family { name: "Five".into(), forms: (0..5).map(|k| FormDef::extrusion(&format!("f{k}"), Axis::Z, "0", "1", rect("0", "0", "1", "1"))).collect(), ..Default::default() };
    let big = |extra: bool| {
        let mut nested = vec![NestedDef::new("n", "Five", ("0", "0", "0")).repeat("1000")];
        if extra { nested.push(NestedDef::new("m", "One", ("0", "0", "0"))); }
        let one = Family { name: "One".into(), forms: vec![FormDef::extrusion("o", Axis::Z, "0", "1", rect("0", "0", "1", "1"))], ..Default::default() };
        compile_library(&[five.clone(), one, Family { name: "P".into(), nested, ..Default::default() }]).unwrap().remove(2)
    };
    assert_eq!(big(false).solve(&Request::default()).unwrap().forms.len(), 5000);
    assert_eq!(big(true).solve(&Request::default()).err(), Some(FamilyError::TooMany("forms in one placement")));
}
