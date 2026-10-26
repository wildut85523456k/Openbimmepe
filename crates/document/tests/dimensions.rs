//! Associative plan dimensions (D-DIM): an end picked on a wall or grid follows it.

use openbim_document::{native, Command, Document, ElementId, Geometry, Xf};

fn dim(d: &Document, id: ElementId) -> ([f64; 2], [f64; 2], bool, bool) {
    match d.get(id).unwrap().geometry {
        Some(Geometry::Dimension { a, b, refs, .. }) => (a, b, refs[0].is_some(), refs[1].is_some()),
        _ => panic!("not a dimension"),
    }
}

fn length(d: &Document, id: ElementId) -> f64 { let (a, b, ..) = dim(d, id); (b[0] - a[0]).hypot(b[1] - a[1]) }

fn two_walls() -> (Document, ElementId, ElementId, ElementId) {
    let mut d = Document::new();
    let l = d.place_level("Ground", 0.0).unwrap();
    // two parallel walls 4 m apart (axes), 200 mm thick: their facing faces are 3.8 m apart
    let w1 = d.place_wall(l, [0.0, 0.0], [6.0, 0.0], 0.2, 3.0, 0.0).unwrap();
    let w2 = d.place_wall(l, [0.0, 4.0], [6.0, 4.0], 0.2, 3.0, 0.0).unwrap();
    (d, l, w1, w2)
}

#[test]
fn a_dimension_between_two_wall_faces_follows_a_moved_wall_and_undo_restores_both() {
    let (mut d, l, w1, w2) = two_walls();
    let dim_id = d.place_dimension(l, [3.0, 0.1], [3.0, 3.9], [4.0, 2.0]).unwrap();
    assert_eq!(dim(&d, dim_id).2..=dim(&d, dim_id).3, true..=true, "both ends refer to the walls they were picked on");
    assert!((length(&d, dim_id) - 3.8).abs() < 1e-9);
    // move the second wall 500 mm away: the dimension reads 4.3 m in the same change
    let ops = d.transform_commands(&[w2], Xf::Move([0.0, 0.5])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    assert!((length(&d, dim_id) - 4.3).abs() < 1e-9, "{}", length(&d, dim_id));
    // a wall stretched along its line keeps the end at the same fraction of it
    let ops = d.transform_commands(&[w1], Xf::Move([1.0, 0.0])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    let (a, ..) = dim(&d, dim_id);
    assert!((a[0] - 4.0).abs() < 1e-9 && (a[1] - 0.1).abs() < 1e-9, "the end on the moved wall moved with it: {a:?}");
    // one undo takes the wall and the dimension back together
    d.undo().unwrap();
    d.undo().unwrap();
    assert!((length(&d, dim_id) - 3.8).abs() < 1e-9);
    d.redo().unwrap();
    assert!((length(&d, dim_id) - 4.3).abs() < 1e-9);
    let _ = w1;
}

#[test]
fn a_free_end_stays_a_deleted_wall_leaves_the_dimension_and_a_grid_is_a_reference_too() {
    let (mut d, l, _w1, w2) = two_walls();
    let grid = d.place_grid([-2.0, -1.0], [-2.0, 5.0], "A").unwrap();
    // from the grid to a point on nothing
    let free = d.place_dimension(l, [-2.0, 2.0], [-5.0, 2.0], [-3.0, 3.0]).unwrap();
    assert_eq!((dim(&d, free).2, dim(&d, free).3), (true, false));
    let ops = d.transform_commands(&[grid], Xf::Move([-1.0, 0.0])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    let (a, b, ..) = dim(&d, free);
    assert!((a[0] + 3.0).abs() < 1e-9 && (b[0] + 5.0).abs() < 1e-9, "the grid end follows, the free end stays: {a:?} {b:?}");
    // a dimension to a wall that is deleted stays, at its last place, no longer associative there
    let to_wall = d.place_dimension(l, [3.0, 0.1], [3.0, 3.9], [4.0, 2.0]).unwrap();
    d.transact(d.revision(), &[Command::Delete { id: w2 }]).unwrap();
    let (_, b, ra, rb) = dim(&d, to_wall);
    assert!(ra && !rb && (b[1] - 3.9).abs() < 1e-9, "{b:?}");
}

#[test]
fn the_references_are_saved_and_read_back_and_an_old_record_has_none() {
    let (mut d, l, _, w2) = two_walls();
    let dim_id = d.place_dimension(l, [3.0, 0.1], [3.0, 3.9], [4.0, 2.0]).unwrap();
    let lines = native::records(&d);
    let line = lines.iter().find(|l| l.contains("cat=Dimension")).unwrap();
    assert!(line.contains(&format!(",{}:", w2.0)), "the record carries the references: {line}");
    let (elements, site) = native::parse_records(&lines).unwrap();
    let mut back = Document::new();
    back.transact(0, &native::restore_commands(elements, site)).unwrap();
    assert_eq!(back.get(dim_id).unwrap().geometry, d.get(dim_id).unwrap().geometry, "the references come back");
    // a record from before references reads as a free dimension
    let old: Vec<String> = lines.iter().map(|l| if l.contains("cat=Dimension") {
        let i = l.find("geo=N,").unwrap();
        let j = i + l[i..].find('|').unwrap();
        let fields: Vec<&str> = l[i..j].split(',').take(8).collect();
        format!("{}{}{}", &l[..i], fields.join(","), &l[j..])
    } else { l.clone() }).collect();
    let (elements, site) = native::parse_records(&old).unwrap();
    let mut back = Document::new();
    back.transact(0, &native::restore_commands(elements, site)).unwrap();
    assert!(matches!(back.get(dim_id).unwrap().geometry, Some(Geometry::Dimension { refs: [None, None], .. })));
}
