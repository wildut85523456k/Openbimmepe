use openbim_family::*;
use proptest::prelude::*;

fn fam() -> Compiled {
    let f = Family {
        name: "Arch".into(),
        values: vec![
            ValueDef::input("width", 1.0, 0.1, 10.0, Scope::Instance),
            ValueDef::input("height", 2.4, 0.0, 10.0, Scope::Instance),
            ValueDef::input("rise", 0.0, -10.0, 10.0, Scope::Instance),
            ValueDef::input("sill", 0.0, 0.0, 10.0, Scope::Instance),
        ],
        opening: Some(OpeningDef::new("width", "height", "sill").arched("rise")),
        ..Default::default()
    };
    compile_library(&[f]).unwrap().remove(0)
}
fn solve(f: &Compiled, w: f64, h: f64, r: f64, s: f64) -> Result<Instance, FamilyError> {
    f.solve(&Request {
        type_name: None,
        overrides: vec![("width", w), ("height", h), ("rise", r), ("sill", s)],
        texts: vec![],
    })
}
/// Shoelace area, counter-clockwise positive.
fn area(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
        .sum::<f64>()
        / 2.0
}
/// Hand formula for the polygon: a rectangle plus a fan of N thin triangles from the arc centre, minus the centre-chord triangle.
fn want(w: f64, h: f64, r: f64) -> f64 {
    let half = w / 2.0;
    let big_r = (half * half + r * r) / (2.0 * r);
    let a = (half / big_r).asin();
    let n = ARCH_SEGMENTS as f64;
    w * (h - r) + 0.5 * big_r * big_r * (n * (2.0 * a / n).sin() - (2.0 * a).sin())
}

#[test]
fn a_flat_head_is_a_plain_rectangle() {
    let i = solve(&fam(), 0.9, 2.1, 0.0, 0.0).unwrap();
    assert_eq!(i.opening_rise, 0.0);
    assert_eq!(
        i.opening_outline(1.0).unwrap(),
        vec![[1.0, 0.0], [1.9, 0.0], [1.9, 2.1], [1.0, 2.1]]
    );
}

#[test]
fn the_outline_of_a_segmental_arch_matches_the_hand_formula() {
    let i = solve(&fam(), 1.0, 2.4, 0.3, 0.0).unwrap();
    let p = i.opening_outline(2.0).unwrap();
    assert_eq!(p.len(), 4 + ARCH_SEGMENTS - 1);
    assert!(
        (area(&p) - want(1.0, 2.4, 0.3)).abs() < 1e-12,
        "{} vs {}",
        area(&p),
        want(1.0, 2.4, 0.3)
    );
    // independent: close to the true circular segment
    let big_r: f64 = (0.25 + 0.09) / 0.6;
    let a = (0.5f64 / big_r).asin();
    let true_area = 1.0 * 2.1 + big_r * big_r * (a - a.sin() * a.cos());
    assert!(area(&p) < true_area && (true_area - area(&p)) / true_area < 2e-3);
    // crown exactly on the centre line at the full height; springing at 2.1; both feet on the sill
    assert!(p.contains(&[2.5, 2.4]));
    assert!(
        p.contains(&[2.0, 2.1])
            && p.contains(&[3.0, 2.1])
            && p.contains(&[2.0, 0.0])
            && p.contains(&[3.0, 0.0])
    );
    // every arc point is on the circle of radius R about (2.5, 2.4 - R)
    for q in &p {
        if q[1] > 2.1 + 1e-9 {
            let d = ((q[0] - 2.5).powi(2) + (q[1] - (2.4 - big_r)).powi(2)).sqrt();
            assert!((d - big_r).abs() < 1e-12);
        }
    }
}

#[test]
fn a_semicircle_head_and_a_pure_arch_work() {
    let f = fam();
    let p = solve(&f, 1.0, 2.0, 0.5, 0.0)
        .unwrap()
        .opening_outline(0.0)
        .unwrap();
    assert!((area(&p) - want(1.0, 2.0, 0.5)).abs() < 1e-12);
    // a full circle-top with rise = height has no straight sides: the feet are the arc ends
    let p = solve(&f, 1.0, 0.5, 0.5, 0.7)
        .unwrap()
        .opening_outline(0.0)
        .unwrap();
    assert_eq!(p.len(), 2 + ARCH_SEGMENTS - 1);
    assert!((area(&p) - want(1.0, 0.5, 0.5)).abs() < 1e-12);
    assert!(p.iter().all(|q| q[1] >= 0.7 - 1e-12));
}

#[test]
fn bad_arches_are_refused() {
    let f = fam();
    let bad = |w, h, r| matches!(solve(&f, w, h, r, 0.0), Err(FamilyError::BadArch(_)));
    assert!(bad(1.0, 2.4, 0.5001)); // more than half the width
    assert!(bad(1.0, 0.4, 0.45)); // more than the height
    assert!(bad(1.0, 2.4, 0.0005)); // too flat
    assert!(bad(1.0, 0.3005, 0.3)); // sliver of straight side
                                    // a negative rise says so
    match solve(&f, 1.0, 2.4, -0.5, 0.0) {
        Err(FamilyError::BadArch(m)) => assert!(m.contains("negative")),
        e => panic!("{e:?}"),
    }
    assert!(solve(&f, 1.0, 2.4, 0.5, 0.0).is_ok());
    assert!(solve(&f, 1.0, 0.5, 0.5, 0.0).is_ok());
}

proptest! {
    #[test]
    fn any_valid_arch_has_the_hand_area_and_stays_in_its_box(w in 0.2f64..4.0, rf in 0.02f64..1.0, extra in prop_oneof![Just(0.0f64), 0.01f64..3.0], s in 0.0f64..1.5, off in 0.0f64..5.0) {
        let r = rf * w / 2.0;
        let h = r + extra;
        let i = solve(&fam(), w, h, r, s).unwrap();
        let p = i.opening_outline(off).unwrap();
        prop_assert!((area(&p) - want(w, h, r)).abs() < 1e-9);
        for q in &p {
            prop_assert!(q[0] >= off - 1e-12 && q[0] <= off + w + 1e-12);
            prop_assert!(q[1] >= s - 1e-12 && q[1] <= s + h + 1e-12);
        }
        let top = p.iter().map(|q| q[1]).fold(f64::MIN, f64::max);
        prop_assert!((top - (s + h)).abs() < 1e-12);
    }
}
