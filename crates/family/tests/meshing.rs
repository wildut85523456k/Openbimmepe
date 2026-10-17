use openbim_family::forms::*;
use openbim_family::meshing::TriMesh;
use openbim_family::*;

fn rect(u0: &str, v0: &str, u1: &str, v1: &str) -> ProfileDef { ProfileDef::rect(u0, v0, u1, v1) }

/// planes of a unit cube: x0, x1, y0, y1, z0, z1
fn planes() -> Vec<ValueDef> {
    vec![ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "1"), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1")]
}
fn inst(solids: Vec<SolidDef>, forms: Vec<FormDef>) -> Instance {
    Family { name: "F".into(), values: planes(), solids, forms, ..Default::default() }.compile().unwrap().solve(&Request::default()).unwrap()
}
fn total(i: &Instance) -> f64 { i.body().pieces.iter().map(|p| p.mesh.volume()).sum() }
fn polygon_area(r: f64) -> f64 { 0.5 * 32.0 * r * r * (std::f64::consts::TAU / 32.0).sin() }
fn why(solids: Vec<SolidDef>, forms: Vec<FormDef>) -> String {
    match (Family { name: "F".into(), values: planes(), solids, forms, ..Default::default() }.compile().and_then(|c| c.solve(&Request::default()))) { Err(FamilyError::Form { why, .. }) => why.to_string(), other => panic!("{other:?}") }
}

#[test]
fn a_blend_is_a_loft_between_two_profiles_with_exact_volume() {
    // a box that narrows: 1 x 1 at the bottom to 0.5 x 0.5 at the top, 2 m high
    let f = FormDef::blend("b", Axis::Z, "0", "2", rect("0", "0", "1", "1"), rect("0.25", "0.25", "0.75", "0.75"));
    let i = inst(vec![], vec![f]);
    let want = 2.0 / 3.0 * (1.0 + 0.5 + 0.25); // a frustum of a pyramid
    assert!((i.forms[0].volume() - want).abs() < 1e-12, "{}", i.forms[0].volume());
    let m = &i.body().pieces[0].mesh;
    assert!((m.volume() - want).abs() < 1e-12, "{}", m.volume());
    let (lo, hi) = m.bounds().unwrap();
    assert!(lo == [0.0, 0.0, 0.0] && hi == [1.0, 1.0, 2.0], "{lo:?} {hi:?}");
    assert_eq!(i.forms[0].bounds_local(), (lo, hi));
    // work planes: the same blend along x
    let f = FormDef::blend("b", Axis::X, "5", "7", rect("0", "0", "1", "1"), rect("0", "0", "2", "2"));
    let i = inst(vec![], vec![f]);
    let (lo, hi) = i.body().pieces[0].mesh.bounds().unwrap();
    assert!(lo == [5.0, 0.0, 0.0] && hi == [7.0, 2.0, 2.0], "{lo:?} {hi:?}");
    assert!(i.body().pieces[0].mesh.volume() > 0.0);
    assert!((i.body().pieces[0].mesh.volume() - i.forms[0].volume()).abs() < 1e-12);
    // a circle becoming a smaller circle: a cone frustum on 32-gons
    let f = FormDef::blend("b", Axis::Z, "0", "1", ProfileDef::circle("0", "0", "0.2"), ProfileDef::circle("0", "0", "0.1"));
    let i = inst(vec![], vec![f]);
    let (a0, a1) = (polygon_area(0.2), polygon_area(0.1));
    assert!((total(&i) - 1.0 / 3.0 * (a0 + a1 + (a0 * a1).sqrt())).abs() < 1e-12);
    assert!((i.forms[0].volume() - 1.0 / 3.0 * std::f64::consts::PI * (0.04 + 0.02 + 0.01)).abs() < 1e-12);
}

#[test]
fn a_swept_blend_changes_its_profile_along_the_path() {
    let f = FormDef::swept_blend("s", "0", &[("0", "0"), ("4", "0")], rect("-0.05", "0", "0.05", "0.1"), rect("-0.15", "0", "0.15", "0.3"));
    let i = inst(vec![], vec![f]);
    let (a0, a1): (f64, f64) = (0.1 * 0.1, 0.3 * 0.3);
    let want = 4.0 / 3.0 * (a0 + (a0 * a1).sqrt() + a1); // similar sections
    assert!((i.forms[0].volume() - want).abs() < 1e-12, "{}", i.forms[0].volume());
    let m = &i.body().pieces[0].mesh;
    assert!((m.volume() - want).abs() < 1e-12, "{}", m.volume());
    let (lo, hi) = m.bounds().unwrap();
    assert!(lo == [0.0, -0.15, 0.0] && hi == [4.0, 0.15, 0.3], "{lo:?} {hi:?}");
    // a bent path: closed solid of positive volume, close to the straight estimate
    let f = FormDef::swept_blend("s", "0", &[("0", "0"), ("2", "0"), ("2", "2")], rect("-0.05", "-0.05", "0.05", "0.05"), rect("-0.1", "-0.1", "0.1", "0.1"));
    let i = inst(vec![], vec![f]);
    let (v, est) = (total(&i), i.forms[0].volume());
    assert!(v > 0.0 && (v - est).abs() < 0.05 * est, "{v} vs {est}");
}

#[test]
fn blends_check_their_profiles() {
    let tri = ProfileDef::polygon(&[("0", "0"), ("1", "0"), ("0", "1")]);
    assert!(why(vec![], vec![FormDef::blend("b", Axis::Z, "0", "1", rect("0", "0", "1", "1"), tri)]).contains("same number of points"));
    assert!(why(vec![], vec![FormDef::blend("b", Axis::Z, "0", "1", rect("0", "0", "1", "1"), ProfileDef::circle("0", "0", "1"))]).contains("same number of points"));
    assert!(why(vec![], vec![FormDef::blend("b", Axis::Z, "1", "1", rect("0", "0", "1", "1"), rect("0", "0", "1", "1"))]).contains("positive length"));
    assert!(why(vec![], vec![FormDef::blend("b", Axis::Z, "0", "1", rect("0", "0", "1", "1"), rect("0", "0", "1", "1")).hole(ProfileDef::circle("0.5", "0.5", "0.1"))]).contains("holes"));
    let mut e = FormDef::extrusion("e", Axis::Z, "0", "1", rect("0", "0", "1", "1"));
    e.second = Some(rect("0", "0", "1", "1"));
    assert!(why(vec![], vec![e]).contains("only a blend"));
}

#[test]
fn a_void_form_cuts_the_solids_and_adds_no_volume() {
    let cube = || SolidDef::new("box", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"));
    // a square hole through the middle
    let hole = FormDef::extrusion("h", Axis::Z, "-1", "2", rect("0.25", "0.25", "0.75", "0.75")).void();
    let i = inst(vec![cube()], vec![hole]);
    assert!(i.forms[0].void);
    assert!((total(&i) - 0.75).abs() < 1e-9, "{}", total(&i));
    // a round hole drilled sideways (plane Y: u = z, v = x), the polygon of the cylinder is cut exactly
    let drill = FormDef::extrusion("d", Axis::Y, "-1", "2", ProfileDef::circle("0.5", "0.5", "0.2")).void();
    let i = inst(vec![cube()], vec![drill]);
    assert!((total(&i) - (1.0 - polygon_area(0.2))).abs() < 1e-6, "{}", total(&i));
    // a notch in a corner: removes the part inside only
    let notch = FormDef::extrusion("n", Axis::Z, "0.5", "2", rect("0.5", "0.5", "2", "2")).void();
    let i = inst(vec![cube()], vec![notch]);
    assert!((total(&i) - (1.0 - 0.25 * 0.5)).abs() < 1e-9, "{}", total(&i));
    // far away: nothing changes; swallowing the whole solid: nothing is left
    let far = FormDef::extrusion("f", Axis::Z, "0", "1", rect("5", "5", "6", "6")).void();
    assert!((total(&inst(vec![cube()], vec![far])) - 1.0).abs() < 1e-12);
    let all = FormDef::extrusion("a", Axis::Z, "-1", "2", rect("-1", "-1", "2", "2")).void();
    assert!(inst(vec![cube()], vec![all]).body().pieces.is_empty());
    // two voids, and a form cut as well as a box
    let v1 = FormDef::extrusion("v1", Axis::Z, "-1", "2", rect("0", "0", "0.5", "1")).void();
    let v2 = FormDef::extrusion("v2", Axis::Z, "-1", "2", rect("0.5", "0", "1", "0.5")).void();
    let solid = FormDef::extrusion("s", Axis::Z, "0", "1", rect("2", "0", "3", "1"));
    let i = inst(vec![cube()], vec![v1, v2, solid]);
    assert!((total(&i) - (0.25 + 1.0)).abs() < 1e-9, "{}", total(&i));
    // the voids add nothing to the pieces
    assert_eq!(i.body().pieces.len(), 2);
}

#[test]
fn the_mesh_of_a_cut_solid_is_one_closed_surface() {
    let cube = SolidDef::new("box", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"));
    let hole = FormDef::extrusion("h", Axis::Z, "-1", "2", rect("0.25", "0.25", "0.75", "0.75")).void();
    let i = inst(vec![cube], vec![hole]);
    let m: TriMesh = i.body().pieces[0].mesh.clone();
    // outward everywhere: each triangle's normal points away from the solid, so the total surface area is the
    // outside (6) plus the hole walls (4 x 0.5 x 1)
    let area: f64 = m.tris.iter().map(|t| {
        let (a, b, c) = (m.verts[t[0] as usize], m.verts[t[1] as usize], m.verts[t[2] as usize]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
    }).sum();
    // top and bottom lose the hole (2 x 0.25), the sides stay, the hole walls are added
    assert!((area - (6.0 - 0.5 + 2.0)).abs() < 1e-9, "{area}");
}
