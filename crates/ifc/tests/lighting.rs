use openbim_ifc::lighting::*;

fn full() -> RoomLighting {
    RoomLighting { work_plane_height_m: Some(0.8), boundary_zone_m: Some(0.5), maintenance_factor: Some(0.8), required_lux: Some(300.0), min_uniformity: Some(0.6), max_ugr: Some(19.0), min_cri: Some(80.0), reflectance_ceiling: Some(0.7), reflectance_walls: Some(0.5), reflectance_floor: Some(0.2) }
}

#[test]
fn room_inputs_are_checked_and_missing_ones_listed() {
    assert!(full().check().is_ok() && full().missing_for_calculation().is_empty());
    assert_eq!(RoomLighting::default().missing_for_calculation(), vec!["work plane height", "maintenance factor", "required illuminance", "surface reflectances"]);
    // one reflectance missing still counts as missing reflectances
    assert_eq!(RoomLighting { reflectance_walls: None, ..full() }.missing_for_calculation(), vec!["surface reflectances"]);
    for (bad, why) in [
        (RoomLighting { maintenance_factor: Some(0.0), ..full() }, "maintenance"),
        (RoomLighting { maintenance_factor: Some(1.2), ..full() }, "maintenance"),
        (RoomLighting { required_lux: Some(-1.0), ..full() }, "illuminance"),
        (RoomLighting { work_plane_height_m: Some(f64::NAN), ..full() }, "work plane"),
        (RoomLighting { reflectance_floor: Some(1.5), ..full() }, "floor"),
        (RoomLighting { min_cri: Some(120.0), ..full() }, "rendering"),
        (RoomLighting { min_uniformity: Some(-0.1), ..full() }, "uniformity"),
    ] {
        assert!(bad.check().unwrap_err().contains(why), "{why}");
    }
}
