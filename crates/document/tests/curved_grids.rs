//! Curved grid lines: a grid element with a bulge (DXF meaning: tan of a quarter of the sweep, positive
//! counter-clockwise) between its two ends.

use openbim_document::{native, Xf, Command, Document, ElementId, Geometry};

fn grid_doc(bulge: f64) -> (Document, ElementId) {
    let mut d = Document::new();
    let id = d.place_arc_grid([0.0, 0.0], [10.0, 0.0], bulge, "A").unwrap();
    (d, id)
}

#[test]
fn a_bulge_is_kept_only_for_a_grid_and_only_in_range() {
    let (mut d, id) = grid_doc(0.4);
    assert_eq!(d.get(id).unwrap().bulge, 0.4);
    for bad in [1.5, -1.01, f64::NAN, f64::INFINITY] {
        assert!(d.transact(d.revision(), &[Command::SetBulge { id, bulge: bad }]).is_err(), "{bad}");
    }
    assert_eq!(d.get(id).unwrap().bulge, 0.4, "a refused change leaves the line as it was");
    d.transact(d.revision(), &[Command::SetBulge { id, bulge: 0.0 }]).unwrap();
    assert_eq!(d.get(id).unwrap().bulge, 0.0);
    // a wall is not a curve
    let level = d.elements().find(|e| e.category == openbim_document::Category::Level).map(|e| e.id);
    assert!(level.is_none(), "an empty document has no level");
    let sample = Document::spatial_sample();
    let mut s = sample.clone();
    assert!(s.transact(s.revision(), &[Command::SetBulge { id: ElementId(20), bulge: 0.3 }]).is_err());
}

#[test]
fn undo_takes_the_curve_back_to_a_straight_line() {
    let (mut d, id) = grid_doc(0.0);
    d.transact(d.revision(), &[Command::SetBulge { id, bulge: -0.5 }]).unwrap();
    assert_eq!(d.get(id).unwrap().bulge, -0.5);
    d.undo().unwrap();
    assert_eq!(d.get(id).unwrap().bulge, 0.0);
}

#[test]
fn the_bulge_is_saved_and_a_file_without_one_is_straight() {
    let (d, id) = grid_doc(0.25);
    let lines = native::records(&d);
    let (els, site) = native::parse_records(&lines).unwrap();
    assert_eq!(els.iter().find(|e| e.id == id).unwrap().bulge, 0.25);
    let _ = site;
    // an older file has no bg= field
    let old: Vec<String> = lines.iter().map(|l| l.split('|').filter(|f| !f.starts_with("bg=")).collect::<Vec<_>>().join("|")).collect();
    assert!(old.iter().all(|l| !l.contains("|bg=")));
    let (els, _) = native::parse_records(&old).unwrap();
    assert_eq!(els.iter().find(|e| e.id == id).unwrap().bulge, 0.0);
}

#[test]
fn mirroring_turns_the_curve_the_other_way_and_rotating_keeps_it() {
    let (mut d, id) = grid_doc(0.4);
    let ops = d.transform_commands(&[id], Xf::Mirror([0.0, 0.0], [1.0, 0.0])).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    assert_eq!(d.get(id).unwrap().bulge, -0.4);
    let ops = d.transform_commands(&[id], Xf::Rotate([0.0, 0.0], 1.0)).unwrap();
    d.transact(d.revision(), &ops).unwrap();
    assert_eq!(d.get(id).unwrap().bulge, -0.4);
    assert!(matches!(d.get(id).unwrap().geometry, Some(Geometry::Grid { .. })));
}
