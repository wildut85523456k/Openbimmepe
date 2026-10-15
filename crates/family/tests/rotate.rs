use openbim_family::library::{all, handle};
use openbim_family::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn solid_index(c: &Compiled, i: &Instance, name: &str) -> usize { (0..i.solids.len()).find(|&k| c.solid_name(i.solids[k].0) == name).unwrap_or_else(|| panic!("no part {name}")) }
/// plan corners of a part after its turn
fn corners(i: &Instance, k: usize) -> Vec<[f64; 2]> {
    let b = i.solids[k].1;
    [[b.min[0], b.min[1]], [b.max[0], b.min[1]], [b.max[0], b.max[1]], [b.min[0], b.max[1]]].iter().map(|&p| i.xf[k].map_or(p, |x| x.apply(p))).collect()
}
fn has(cs: &[[f64; 2]], x: f64, y: f64) -> bool { cs.iter().any(|c| close(c[0], x) && close(c[1], y)) }

#[test]
fn an_open_door_leaf_turns_about_its_hinge_by_hand_calculation() {
    let c = &all().unwrap()[1];
    // closed: no turn at all
    let i = c.solve(&Request::default()).unwrap();
    // closed: nothing is turned
    assert!(i.xf.iter().all(|x| x.is_none()), "a closed door has no turned part");
    // hinged on the right, open 90 degrees: the leaf becomes x 0.38..0.42, y 0..0.8 (hand calculation in the test notes)
    let i = c.solve(&Request { overrides: vec![("open_angle", 90.0)], ..Default::default() }).unwrap();
    let k = solid_index(c, &i, "leaf");
    let cs = corners(&i, k);
    for (x, y) in [(0.38, 0.8), (0.42, 0.8), (0.38, 0.0), (0.42, 0.0)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    // the frame does not move
    let j = solid_index(c, &i, "jamb_left");
    assert!(i.xf[j].is_none());
    // the handle turns with the leaf: plate centre (-0.33, -0.024) -> (0.376, 0.73)
    let h = solid_index(c, &i, "handle/plate_front");
    let b = i.solids[h].1;
    let mid = i.xf[h].expect("handle turns").apply([(b.min[0] + b.max[0]) / 2.0, (b.min[1] + b.max[1]) / 2.0]);
    assert!(close(mid[0], 0.376) && close(mid[1], 0.73), "{mid:?}");
    // heights do not change
    assert!(close(b.min[2], 0.92) && close(b.max[2], 1.08));
    // hinged on the left: the leaf swings to the same side (+y) about x = -0.4: (x,y) -> (-0.4 - y, x + 0.4) for 90 degrees
    let i = c.solve(&Request { overrides: vec![("open_angle", 90.0)], texts: vec![("swing", "left")], ..Default::default() }).unwrap();
    let cs = corners(&i, solid_index(c, &i, "leaf"));
    for (x, y) in [(-0.42, 0.8), (-0.38, 0.8), (-0.42, 0.0), (-0.38, 0.0)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    // 45 degrees, right-hinged: the free edge end (-0.4, 0) goes to (0.4 - 0.8 cos45, 0.8 sin45)
    let i = c.solve(&Request { overrides: vec![("open_angle", 45.0)], ..Default::default() }).unwrap();
    let cs = corners(&i, solid_index(c, &i, "leaf"));
    let r = 0.8 * std::f64::consts::FRAC_1_SQRT_2;
    // centre line of the free edge: corners (-0.4, +-0.02) -> (0.4 + dy cos... ) check the middle of the two free corners
    let free: Vec<&[f64; 2]> = cs.iter().filter(|p| p[0] < 0.2 || p[1] > 0.3).collect();
    let mx: f64 = free.iter().map(|p| p[0]).sum::<f64>() / free.len() as f64;
    let my: f64 = free.iter().map(|p| p[1]).sum::<f64>() / free.len() as f64;
    assert!(close(my, r) && close(mx, 0.4 - r), "{mx} {my}");
    // range
    assert!(matches!(c.solve(&Request { overrides: vec![("open_angle", 171.0)], ..Default::default() }), Err(FamilyError::OutOfRange { .. })));
    assert!(matches!(c.solve(&Request { overrides: vec![("open_angle", -1.0)], ..Default::default() }), Err(FamilyError::OutOfRange { .. })));
}

fn marker(name: &str, rot: Option<RotateDef>, nested: Vec<NestedDef>) -> Family {
    let mut f = Family {
        name: name.into(),
        values: vec![ValueDef::plane("x0", Axis::X, "1"), ValueDef::plane("x1", Axis::X, "2"), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1"),
                     ValueDef::angle("a", 90.0, -720.0, 720.0, Scope::Instance)],
        solids: vec![SolidDef::new("b", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))],
        nested,
        ..Default::default()
    };
    if let Some(r) = rot { f.solids[0].rotate = Some(r); }
    f
}

#[test]
fn turns_add_up_through_nested_families() {
    // child box x 1..2, y 0..1 turned 90 about the origin: (x, y) -> (-y, x)
    let child = marker("Child", Some(RotateDef::new("a", "0", "0")), vec![]);
    // parent puts it at (1, 0, 0) and turns the whole thing 90 about the origin: total (x, y) -> (-x, -y) + (0, 1)
    let parent = Family { name: "Parent".into(), values: vec![ValueDef::angle("a", 90.0, -720.0, 720.0, Scope::Instance)], nested: vec![NestedDef::new("c", "Child", ("1", "0", "0")).rotate_about("a", "0", "0")], ..Default::default() };
    let v = compile_library(&[parent, child]).unwrap();
    let i = v[0].solve(&Request::default()).unwrap();
    let cs = corners(&i, 0);
    for (x, y) in [(-1.0, 1.0), (-2.0, 1.0), (-2.0, 0.0), (-1.0, 0.0)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    // only the child turned: (x, y) -> (-y, x) then shift (1, 0)
    let parent = Family { name: "Parent".into(), nested: vec![NestedDef::new("c", "Child", ("1", "0", "0"))], ..Default::default() };
    let v = compile_library(&[parent, marker("Child", Some(RotateDef::new("a", "0", "0")), vec![])]).unwrap();
    let cs = corners(&v[0].solve(&Request::default()).unwrap(), 0);
    for (x, y) in [(1.0, 1.0), (1.0, 2.0), (0.0, 2.0), (0.0, 1.0)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    // only the parent turns, the child box is not turned: box corners (1,0)..(2,1) shifted by (1,0) -> (2,0)..(3,1), then turned 90 about origin: (x, y) -> (-y, x)
    let parent = Family { name: "Parent".into(), values: vec![ValueDef::angle("a", 90.0, -720.0, 720.0, Scope::Instance)], nested: vec![NestedDef::new("c", "Child", ("1", "0", "0")).rotate_about("a", "0", "0")], ..Default::default() };
    let v = compile_library(&[parent, marker("Child", None, vec![])]).unwrap();
    let cs = corners(&v[0].solve(&Request::default()).unwrap(), 0);
    for (x, y) in [(0.0, 2.0), (0.0, 3.0), (-1.0, 2.0), (-1.0, 3.0)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
}

#[test]
fn rotation_formulas_are_checked() {
    let mut f = marker("T", Some(RotateDef::new("a", "0", "nope")), vec![]);
    assert!(matches!(f.compile(), Err(FamilyError::Formula { .. })));
    f.solids[0].rotate = Some(RotateDef::new("a * a / (a - a)", "0", "0"));
    let c = f.compile().unwrap();
    assert!(matches!(c.solve(&Request::default()), Err(FamilyError::Formula { .. })));
    // arrays: every bar carries the turn
    let mut f = marker("T", Some(RotateDef::new("30", "0", "0")), vec![]);
    f.solids[0] = f.solids[0].clone().array(Axis::X, "3", "0.1");
    let c = f.compile().unwrap();
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.solids.len(), 3);
    assert!(i.xf.iter().all(|x| x.map(|x| x.angle) == Some(30.0)));
    assert_eq!(i.xf.len(), i.solids.len());
    // the handle can still be compiled alone and has no turn
    let h = handle().compile().unwrap().solve(&Request::default()).unwrap();
    assert!(h.xf.iter().all(|x| x.is_none()));
}

proptest! {
    #[test]
    fn a_turn_is_rigid_and_keeps_the_hinge_still(angle in 0.0f64..170.0, left in any::<bool>(), w in 0.6f64..2.0) {
        let mut f = openbim_family::library::door();
        f.types.push(TypeDef { name: "t".into(), values: vec![("width".into(), w)], ..Default::default() });
        let c = compile_library(&[handle(), f]).unwrap().remove(1);
        let i = c.solve(&Request { type_name: Some("t"), overrides: vec![("open_angle", angle)], texts: vec![("swing", if left { "left" } else { "right" })] }).unwrap();
        let k = solid_index(&c, &i, "leaf");
        let b = i.solids[k].1;
        let cs = corners(&i, k);
        // the two corners on the hinge edge: they sit on the hinge line within half the leaf thickness, ie. distance from the hinge point stays equal
        let hx = if left { b.min[0] } else { b.max[0] };
        let pivot = [hx, 0.0];
        let before = [[b.min[0], b.min[1]], [b.max[0], b.min[1]], [b.max[0], b.max[1]], [b.min[0], b.max[1]]];
        for (p, q) in before.iter().zip(&cs) {
            let d0 = ((p[0] - pivot[0]).powi(2) + (p[1] - pivot[1]).powi(2)).sqrt();
            let d1 = ((q[0] - pivot[0]).powi(2) + (q[1] - pivot[1]).powi(2)).sqrt();
            prop_assert!((d0 - d1).abs() < 1e-9, "distance from the hinge changed");
        }
        // the free edge ends up on the +y side (the leaf swings to the same side for both hinge sides)
        let far = if left { b.max[0] } else { b.min[0] };
        let tip = i.xf[k].unwrap().apply([far, 0.0]);
        prop_assert!(tip[1] >= -1e-12);
        let dx = (far - hx).abs();
        prop_assert!((tip[1] - dx * angle.to_radians().sin()).abs() < 1e-9);
        // area of the turned leaf equals the area of the leaf
        let area = |p: &[[f64; 2]]| 0.5 * (0..4).map(|k| p[k][0] * p[(k + 1) % 4][1] - p[(k + 1) % 4][0] * p[k][1]).sum::<f64>().abs();
        prop_assert!((area(&cs) - area(&before)).abs() < 1e-9);
    }
}

#[test]
fn a_whole_number_of_turns_is_only_a_shift() {
    // child box x 1..2, y 0..1; parent shifts it by (1, 0) and "turns" it 360 degrees: it just moves, with no turn left over
    let parent = Family { name: "Parent".into(), nested: vec![NestedDef::new("c", "Child", ("1", "0", "2")).rotate_about("360", "5", "5")], ..Default::default() };
    let v = compile_library(&[parent, marker("Child", None, vec![])]).unwrap();
    let i = v[0].solve(&Request::default()).unwrap();
    assert!(i.xf[0].is_none());
    let b = i.solids[0].1;
    assert!(close(b.min[0], 2.0) && close(b.max[0], 3.0) && close(b.min[1], 0.0) && close(b.max[1], 1.0) && close(b.min[2], 2.0) && close(b.max[2], 3.0), "{b:?}");
    // 180 degrees about (5, 5) is a real turn
    let parent = Family { name: "Parent".into(), nested: vec![NestedDef::new("c", "Child", ("1", "0", "0")).rotate_about("180", "5", "5")], ..Default::default() };
    let v = compile_library(&[parent, marker("Child", None, vec![])]).unwrap();
    let i = v[0].solve(&Request::default()).unwrap();
    // (x, y) -> shifted (x+1, y) -> about (5,5) by 180: (10 - x', 10 - y)
    let cs = corners(&i, 0);
    for (x, y) in [(8.0, 10.0), (7.0, 10.0), (7.0, 9.0), (8.0, 9.0)] { assert!(has(&cs, x, y), "{x},{y} not in {cs:?}"); }
    assert_eq!(i.xf[0].unwrap().angle, 180.0);
}

#[test]
fn a_turn_about_a_pivot_off_the_axes() {
    // 90 degrees counter-clockwise about (5, 3): (x, y) -> (5 - (y - 3), 3 + (x - 5))
    let x = Xf::about(90.0, 5.0, 3.0);
    for (p, want) in [([5.0, 3.0], [5.0, 3.0]), ([7.0, 3.0], [5.0, 5.0]), ([5.0, 6.0], [2.0, 3.0]), ([0.0, 0.0], [8.0, -2.0])] {
        let q = x.apply(p);
        assert!(close(q[0], want[0]) && close(q[1], want[1]), "{p:?} -> {q:?}, want {want:?}");
    }
    // 30 degrees about (2, -1): the pivot stays, and distances to it are kept
    let x = Xf::about(30.0, 2.0, -1.0);
    let q = x.apply([2.0, -1.0]);
    assert!(close(q[0], 2.0) && close(q[1], -1.0));
    let q = x.apply([4.0, -1.0]);
    assert!(close(q[0], 2.0 + 2.0 * 30f64.to_radians().cos()) && close(q[1], -1.0 + 2.0 * 30f64.to_radians().sin()));
    // the axes are the images of the unit steps, and the turn is counter-clockwise
    let (ax, ay) = Xf::about(90.0, 0.0, 0.0).axes();
    assert!(close(ax[0], 0.0) && close(ax[1], 1.0) && close(ay[0], -1.0) && close(ay[1], 0.0));
    for angle in [0.0, 30.0, 90.0, 135.0, 270.0, -45.0] {
        let x = Xf::about(angle, 1.5, -2.0);
        let (ax, ay) = x.axes();
        let (o, px, py) = (x.apply([0.0, 0.0]), x.apply([1.0, 0.0]), x.apply([0.0, 1.0]));
        assert!(close(px[0] - o[0], ax[0]) && close(px[1] - o[1], ax[1]) && close(py[0] - o[0], ay[0]) && close(py[1] - o[1], ay[1]), "{angle}");
    }
    // then: second turn applied after the first
    let a = Xf::about(90.0, 0.0, 0.0);
    let b = Xf::translate(1.0, 0.0);
    let ab = a.then(&b);                       // turn, then shift
    let q = ab.apply([1.0, 0.0]);              // (1,0) -> (0,1) -> (1,1)
    assert!(close(q[0], 1.0) && close(q[1], 1.0));
    let ba = b.then(&a);                       // shift, then turn: (1,0) -> (2,0) -> (0,2)
    let q = ba.apply([1.0, 0.0]);
    assert!(close(q[0], 0.0) && close(q[1], 2.0));
    assert_eq!(ab.angle, 90.0);
}
