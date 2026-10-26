//! Moving a wall keeps the walls joined to it joined (Revit's behaviour): their ends slide along their own lines.

use openbim_document::{Document, Geometry, Xf};

fn axis(d: &Document, id: openbim_document::ElementId) -> ([f64; 2], [f64; 2]) {
    match d.get(id).unwrap().geometry { Some(Geometry::Linear { start, end, .. }) => (start, end), _ => panic!("not a wall") }
}

fn close(a: [f64; 2], b: [f64; 2]) -> bool { (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9 }

#[test]
fn the_corner_and_tee_walls_of_a_moved_wall_stretch_to_stay_joined_and_disjoin_leaves_them() {
    let mut d = Document::new();
    let l = d.place_level("Ground", 0.0).unwrap();
    let up = d.place_level("First", 3.0).unwrap();
    // a 6 x 4 m box (axes) with a partition from the bottom wall to the top one
    let bottom = d.place_wall(l, [0.0, 0.0], [6.0, 0.0], 0.2, 3.0, 0.0).unwrap();
    let right = d.place_wall(l, [6.0, 0.0], [6.0, 4.0], 0.2, 3.0, 0.0).unwrap();
    let top = d.place_wall(l, [6.0, 4.0], [0.0, 4.0], 0.2, 3.0, 0.0).unwrap();
    let left = d.place_wall(l, [0.0, 4.0], [0.0, 0.0], 0.2, 3.0, 0.0).unwrap();
    // the partition stops at the bottom wall's face, as one drawn against it does
    let partition = d.place_wall(l, [3.0, 0.1], [3.0, 3.9], 0.1, 3.0, 0.0).unwrap();
    // a wall on another level at the same place is not joined to it
    let above = d.place_wall(up, [6.0, 0.0], [6.0, 4.0], 0.2, 3.0, 0.0).unwrap();
    // a door in the left wall, measured from its start at the top
    let opening = d.place_wall_opening(left, 1.0, 2.0, 0.0, 2.1).unwrap();

    let (ops, _) = d.transform_commands_keeping_joins(&[bottom], Xf::Move([0.0, -0.5])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    assert!(close(axis(&d, bottom).0, [0.0, -0.5]) && close(axis(&d, bottom).1, [6.0, -0.5]));
    // the corner walls' ends slide down their own lines; their other ends stay
    assert!(close(axis(&d, right).0, [6.0, -0.5]) && close(axis(&d, right).1, [6.0, 4.0]), "{:?}", axis(&d, right));
    assert!(close(axis(&d, left).0, [0.0, 4.0]) && close(axis(&d, left).1, [0.0, -0.5]), "{:?}", axis(&d, left));
    // the partition, against the bottom wall's face, stays against it
    assert!(close(axis(&d, partition).0, [3.0, -0.4]) && close(axis(&d, partition).1, [3.0, 3.9]), "{:?}", axis(&d, partition));
    assert!(close(axis(&d, top).0, [6.0, 4.0]), "the top wall is not joined to the bottom one");
    assert!(close(axis(&d, above).0, [6.0, 0.0]), "a wall on another level stays");
    // the left wall's start (top) did not move, so its opening keeps its distance
    assert_eq!(d.wall_openings().iter().find(|o| o.0 == opening).unwrap().2[0], 1.0);
    // one step: undo takes all of it back
    d.undo().unwrap();
    assert!(close(axis(&d, right).0, [6.0, 0.0]) && close(axis(&d, partition).0, [3.0, 0.1]));

    // the right wall's start slides when the bottom moves sideways? No: a slide along the moved wall changes nothing
    // across it, so nothing that meets it end-on has to move
    let (ops, _) = d.transform_commands_keeping_joins(&[bottom], Xf::Move([0.5, 0.0])).unwrap();
    assert!(ops.iter().all(|op| !matches!(op, openbim_document::Command::SetGeometry { id, .. } if *id == right || *id == partition)), "{ops:?}");

    // the plain transform (Align, Offset) moves only the wall it is given
    let ops = d.transform_commands(&[bottom], Xf::Move([0.0, -0.5])).unwrap();
    assert!(ops.iter().all(|op| !matches!(op, openbim_document::Command::SetGeometry { id, .. } if *id == right)), "{ops:?}");
    // Disjoin moves the wall alone
    let (ops, _) = d.transform_commands_disjoined(&[bottom], Xf::Move([0.0, -0.5])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    assert!(close(axis(&d, right).0, [6.0, 0.0]), "Disjoin leaves the joined walls where they were");
}

#[test]
fn an_opening_in_a_wall_whose_start_slides_keeps_its_place_and_a_pinned_joined_wall_refuses_the_move() {
    let mut d = Document::new();
    let l = d.place_level("Ground", 0.0).unwrap();
    let bottom = d.place_wall(l, [0.0, 0.0], [6.0, 0.0], 0.2, 3.0, 0.0).unwrap();
    // the right wall starts at the bottom corner, so moving the bottom wall slides its start
    let right = d.place_wall(l, [6.0, 0.0], [6.0, 4.0], 0.2, 3.0, 0.0).unwrap();
    let opening = d.place_wall_opening(right, 1.0, 2.0, 0.0, 2.1).unwrap();
    let (ops, _) = d.transform_commands_keeping_joins(&[bottom], Xf::Move([0.0, -0.5])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    // the start went 0.5 m back along the wall, so the opening is 0.5 m further from it: the same place in the world
    let o = d.wall_openings().into_iter().find(|o| o.0 == opening).unwrap();
    assert!((o.2[0] - 1.5).abs() < 1e-9, "{:?}", o.2);
    d.undo().unwrap();
    let ops = d.transform_commands_keeping_joins(&[right], Xf::Move([0.0, 0.0])).err();
    assert!(ops.is_some(), "a move by nothing changes nothing");
    d.transact(d.revision(), &[openbim_document::Command::SetPinned { id: right, pinned: true }]).unwrap();
    let refused = d.transform_commands_keeping_joins(&[bottom], Xf::Move([0.0, -0.5]));
    assert!(refused.is_err_and(|e| e.contains("pinned")), "a pinned joined wall refuses the move");
}
