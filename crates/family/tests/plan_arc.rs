//! A plan symbol can be an arc; the library door draws its swing with one.
use openbim_family::library;
use openbim_family::*;

fn door() -> Compiled {
    library::all()
        .unwrap()
        .into_iter()
        .find(|c| c.name() == "Door, single flush")
        .unwrap()
}

#[test]
fn a_door_draws_the_leaf_and_the_arc_its_free_end_sweeps_on_the_hinge_side() {
    // width 0.9, frame 0.05: the clear opening is 0.8 wide, x from -0.4 to 0.4
    let right = door().solve(&Request::default()).unwrap();
    assert_eq!(
        right.plan,
        vec![
            PlanShape::Line([0.4, 0.0, 0.4, 0.8]),
            PlanShape::Arc {
                c: [0.4, 0.0],
                r: 0.8,
                a0: 90.0,
                a1: 180.0
            }
        ]
    );
    let left = door()
        .solve(&Request {
            texts: vec![("swing", "left")],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        left.plan,
        vec![
            PlanShape::Line([-0.4, 0.0, -0.4, 0.8]),
            PlanShape::Arc {
                c: [-0.4, 0.0],
                r: 0.8,
                a0: 0.0,
                a1: 90.0
            }
        ]
    );
    // a wider type draws a longer swing
    let wide = door()
        .solve(&Request {
            type_name: Some("Type B"),
            ..Default::default()
        })
        .unwrap();
    // width 1.2: the hinge is at 0.6 - 0.05 and the leaf is 1.1 long
    let PlanShape::Arc { c, r, a0, a1 } = wide.plan[1] else {
        panic!("an arc")
    };
    assert!(
        (c[0] - 0.55).abs() < 1e-12
            && c[1] == 0.0
            && (r - 1.1).abs() < 1e-12
            && a0 == 90.0
            && a1 == 180.0
    );
}

fn with_arc(r: &str, a0: &str, a1: &str) -> Family {
    Family {
        name: "Arc".into(),
        plan: vec![PlanDef::arc(("0", "0"), r, a0, a1)],
        ..Default::default()
    }
}

#[test]
fn bad_arcs_are_refused_and_a_full_turn_has_no_sweep() {
    assert!(matches!(
        with_arc("0 m", "0 deg", "90 deg")
            .compile()
            .unwrap()
            .solve(&Request::default()),
        Err(FamilyError::Plan { .. })
    ));
    assert!(matches!(
        with_arc("1 m", "10 deg", "370 deg")
            .compile()
            .unwrap()
            .solve(&Request::default()),
        Err(FamilyError::Plan {
            why: "an arc has no sweep",
            ..
        })
    ));
    // a sweep that runs through 0 is fine: 350 to 10 degrees is 20 degrees
    let i = with_arc("1 m", "350 deg", "10 deg")
        .compile()
        .unwrap()
        .solve(&Request::default())
        .unwrap();
    assert_eq!(
        i.plan,
        vec![PlanShape::Arc {
            c: [0.0, 0.0],
            r: 1.0,
            a0: 350.0,
            a1: 10.0
        }]
    );
    // lengths where angles belong, and the reverse, are unit errors
    assert!(with_arc("1 m", "1 m", "90 deg").compile().is_err());
    assert!(with_arc("90 deg", "0 deg", "90 deg").compile().is_err());
}
