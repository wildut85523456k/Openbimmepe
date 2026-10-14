//! The window family's calculated U-value against the independent implementation in the arch crate.
use openbim_arch::glazing::*;
use openbim_family::library::window;
use openbim_family::*;
use proptest::prelude::*;

#[allow(clippy::too_many_arguments)] // a test helper taking the eight window numbers
fn solve_u(w: f64, h: f64, fr: f64, mw: f64, ug: f64, uf: f64, psi: f64, n: u32) -> Result<(Compiled, Instance), FamilyError> {
    let mut f = window();
    f.types.push(TypeDef { name: "t".into(), values: vec![("width".into(), w), ("height".into(), h), ("frame".into(), fr), ("mullion_w".into(), mw), ("ug".into(), ug), ("uf".into(), uf), ("psi".into(), psi)], ..Default::default() });
    let c = compile_library(&[f]).unwrap().remove(0);
    let i = c.solve(&Request { type_name: Some("t"), overrides: vec![("mullions", n as f64)], ..Default::default() })?;
    Ok((c, i))
}

#[test]
fn the_default_window_has_the_hand_calculated_u_value() {
    let c = compile_library(&[window()]).unwrap().remove(0);
    let i = c.solve(&Request::default()).unwrap();
    // defaults: 1.2 x 1.2, frame 0.05, one mullion 0.03: pane (1.2 - 0.1 - 0.03)/2 = 0.535, height 1.1
    let ag = 2.0 * 0.535 * 1.1; // 1.177
    let lg = 2.0 * 2.0 * (0.535 + 1.1); // 6.54
    let uw = (ag * 1.1 + (1.44 - ag) * 2.0 + lg * 0.06) / 1.44;
    assert!((c.value(&i, "glass_area").unwrap() - ag).abs() < 1e-12);
    assert!((c.value(&i, "glass_edge_length").unwrap() - lg).abs() < 1e-12);
    assert!((c.value(&i, "u_window").unwrap() - uw).abs() < 1e-12);
    assert!((uw - (1.2947 + 0.526 + 0.3924) / 1.44).abs() < 1e-3);
    let names: Vec<&str> = c.derived_values(&i).iter().map(|d| d.0).collect();
    assert!(names.contains(&"u_window") && names.contains(&"glass_area") && !names.contains(&"x_left"));
}

proptest! {
    #[test]
    fn the_family_formula_and_the_arch_crate_agree(w in 0.6f64..4.0, h in 0.6f64..3.0, fr in 0.02f64..0.15, mw in 0.01f64..0.05, ug in 0.3f64..6.0, uf in 0.5f64..9.0, psi in 0.0f64..0.2, n in 0u32..6) {
        let (c, i) = solve_u(w, h, fr, mw, ug, uf, psi, n).unwrap();
        let r = window_u(WindowGeometry { width: w, height: h, frame_width: fr, bars: n, bar_width: mw }, ug, &Frame { name: "f".into(), uf, psi }).unwrap();
        prop_assert!((c.value(&i, "u_window").unwrap() - r.uw).abs() < 1e-9 * r.uw);
        prop_assert!((c.value(&i, "glass_area").unwrap() - r.ag).abs() < 1e-9);
        prop_assert!((c.value(&i, "frame_area").unwrap() - r.af).abs() < 1e-9);
        prop_assert!((c.value(&i, "glass_edge_length").unwrap() - r.lg).abs() < 1e-9);
    }
}
