//! Level authoring (A6): what `place_level` accepts, and renaming and deleting a level.

use openbim_document::{Document, MIN_LEVEL_GAP_M};

#[test]
fn a_level_needs_its_own_name_and_room_below_the_one_above() {
    let mut d = Document::new();
    let l1 = d.place_level("Level 1", 0.0).unwrap();
    // the same name, in any case, is refused
    assert!(d.place_level("level 1", 3.0).is_err(), "a name in use is refused");
    // two levels closer than the minimum gap cannot be told apart as storeys
    assert!(d.place_level("Level 1b", MIN_LEVEL_GAP_M / 2.0).is_err(), "levels {MIN_LEVEL_GAP_M} m apart are refused");
    assert!(d.place_level("Level 1b", MIN_LEVEL_GAP_M).is_ok(), "exactly the minimum gap is accepted");
    // an empty name is given the next free one, and a finite elevation is required
    let auto = d.place_level("   ", 6.0).unwrap();
    assert!(d.get(auto).unwrap().name.starts_with("Level"), "{}", d.get(auto).unwrap().name);
    assert!(d.place_level("Level 9", f64::NAN).is_err());
    assert_eq!(d.get(l1).unwrap().elevation_m, 0.0);
}

#[test]
fn a_level_is_renamed_and_deleted_only_when_nothing_stands_on_it() {
    let mut d = Document::new();
    let l1 = d.place_level("Level 1", 0.0).unwrap();
    let l2 = d.place_level("Level 2", 3.0).unwrap();
    d.rename_level(l2, "Roof").unwrap();
    assert_eq!(d.get(l2).unwrap().name, "Roof");
    assert!(d.rename_level(l2, "Level 1").is_err(), "a name another level has is refused");
    assert!(d.rename_level(l2, "  ").is_err(), "an empty name is refused");
    assert!(d.rename_level(l1, "Ground").is_ok());
    // a level nothing stands on goes, and undo brings it back
    d.delete_level(l2).unwrap();
    assert!(d.get(l2).is_none());
    d.undo().unwrap();
    assert_eq!(d.get(l2).unwrap().name, "Roof");
    // a level a wall stands on is refused
    let wall = d.place_wall(l1, [0.0, 0.0], [4.0, 0.0], 0.2, 3.0, 0.0).unwrap();
    assert!(d.delete_level(l1).is_err(), "a level with elements on it is not deleted");
    assert!(d.delete_level(wall).is_err(), "a wall is not a level");
    assert!(d.rename_level(wall, "Nope").is_err(), "a wall is not renamed");
}

#[test]
fn a_level_moves_up_or_down_without_passing_another_and_what_stands_on_it_follows() {
    let mut d = Document::new();
    let l1 = d.place_level("Ground", 0.0).unwrap();
    let l2 = d.place_level("First", 3.0).unwrap();
    let l3 = d.place_level("Roof", 6.0).unwrap();
    let wall = d.place_wall(l2, [0.0, 0.0], [4.0, 0.0], 0.2, 3.0, 0.0).unwrap();
    d.set_level_elevation(l3, 7.2).unwrap();
    d.set_level_elevation(l2, 3.6).unwrap();
    assert_eq!((d.get(l2).unwrap().elevation_m, d.get(l3).unwrap().elevation_m), (3.6, 7.2));
    // the wall keeps its level and its offset from it, so it now starts at 3.6 m
    assert_eq!(d.level_of(wall), Some(l2));
    assert_eq!(d.get(wall).unwrap().elevation_m, 0.0, "the wall's own offset from its level is unchanged");
    // a level may not pass another, come too close to one, or take a non-finite elevation
    assert!(d.set_level_elevation(l2, 8.0).is_err(), "First cannot go above Roof");
    assert!(d.set_level_elevation(l2, -1.0).is_err(), "First cannot go below Ground");
    assert!(d.set_level_elevation(l2, 7.2 - MIN_LEVEL_GAP_M / 2.0).is_err(), "too close to Roof");
    assert!(d.set_level_elevation(l2, f64::INFINITY).is_err());
    assert!(d.set_level_elevation(wall, 1.0).is_err(), "a wall is not a level");
    assert!(d.set_level_elevation(l1, -0.5).is_ok(), "the lowest level can go down");
    // each move is one undoable step
    d.undo().unwrap();
    assert_eq!(d.get(l1).unwrap().elevation_m, 0.0);
    d.undo().unwrap();
    assert_eq!(d.get(l2).unwrap().elevation_m, 3.0);
    d.redo().unwrap();
    assert_eq!(d.get(l2).unwrap().elevation_m, 3.6);
}
