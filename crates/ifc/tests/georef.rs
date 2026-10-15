use openbim_arch::{Level, Levels};
use openbim_ifc::georef::*;
use openbim_ifc::*;

fn gr() -> Georef {
    Georef { crs: Crs { name: "X:1".into(), geodetic_datum: None, vertical_datum: None, map_projection: None, map_zone: None }, easting: 1000.0, northing: 2000.0, height: 5.0, grid_angle_deg: 0.0, scale: 1.0, true_north_deg: 0.0, latitude: 10.0, longitude: 100.0, elevation: 5.0 }
}
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn the_origin_and_the_axes_land_where_the_map_says() {
    let g = gr();
    assert_eq!(g.to_map([0.0, 0.0, 0.0]), [1000.0, 2000.0, 5.0]);
    assert_eq!(g.to_map([3.0, 4.0, 1.0]), [1003.0, 2004.0, 6.0]);
    // turned a quarter: the model's X axis points to map north and its Y axis to map west
    let q = Georef { grid_angle_deg: 90.0, ..gr() };
    let m = q.to_map([1.0, 0.0, 0.0]);
    assert!(near(m[0], 1000.0) && near(m[1], 2001.0), "{m:?}");
    let m = q.to_map([0.0, 1.0, 0.0]);
    assert!(near(m[0], 999.0) && near(m[1], 2000.0), "{m:?}");
}

#[test]
fn scale_multiplies_lengths_and_heights_and_the_inverse_undoes_it() {
    let g = Georef { scale: 0.9996, grid_angle_deg: 33.0, ..gr() };
    let (a, b) = (g.to_map([0.0, 0.0, 0.0]), g.to_map([10.0, 0.0, 2.0]));
    assert!(near(((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt(), 9.996));
    assert!(near(b[2] - a[2], 2.0 * 0.9996));
    for p in [[0.0, 0.0, 0.0], [12.5, -3.0, 4.0], [-100.0, 250.0, -2.0]] {
        let back = g.to_local(g.to_map(p));
        assert!((0..3).all(|i| (back[i] - p[i]).abs() < 1e-9), "{p:?} {back:?}");
    }
}

#[test]
fn true_north_is_the_y_axis_turned_anticlockwise() {
    assert_eq!(gr().true_north(), (-0.0, 1.0));
    assert_eq!(Georef { true_north_deg: 180.0, ..gr() }.true_north(), (-0.0, -1.0));
    assert_eq!(Georef { true_north_deg: -90.0, ..gr() }.true_north(), (1.0, 0.0));
    assert_eq!(Georef { grid_angle_deg: 270.0, ..gr() }.x_axis(), (0.0, -1.0));
    let t = Georef { true_north_deg: 90.0, ..gr() }.true_north();
    assert!(near(t.0, -1.0) && near(t.1, 0.0), "{t:?}");
}

#[test]
fn compound_angles_carry_their_sign_and_do_not_round_up_into_60() {
    assert_eq!(compound_angle(11.5), [11, 30, 0, 0]);
    assert_eq!(compound_angle(-11.5), [-11, -30, 0, 0]);
    assert_eq!(compound_angle(0.0), [0, 0, 0, 0]);
    // a second less a hair is a second, not 59 seconds and 1000000 millionths
    assert_eq!(compound_angle(1.0 / 3600.0 - 1e-15), [0, 0, 1, 0]);
    assert_eq!(compound_angle(104.928_333_3), [104, 55, 41, 999_880]);
    assert_eq!(compound_angle(-0.000_000_5), [0, 0, 0, -1_800]);
}

#[test]
fn a_bad_position_is_refused() {
    assert_eq!(Georef { crs: Crs { name: "  ".into(), ..gr().crs }, ..gr() }.check(), Err(GeorefError::NoCrsName));
    assert_eq!(Georef { easting: f64::NAN, ..gr() }.check(), Err(GeorefError::NotFinite("easting")));
    assert_eq!(Georef { latitude: 90.5, ..gr() }.check(), Err(GeorefError::OutOfRange("latitude")));
    assert_eq!(Georef { latitude: -90.5, ..gr() }.check(), Err(GeorefError::OutOfRange("latitude")));
    assert_eq!(Georef { longitude: 180.5, ..gr() }.check(), Err(GeorefError::OutOfRange("longitude")));
    assert_eq!(Georef { longitude: -180.5, ..gr() }.check(), Err(GeorefError::OutOfRange("longitude")));
    assert_eq!(Georef { scale: 0.0, ..gr() }.check(), Err(GeorefError::NotInvertible));
    assert_eq!(Georef { scale: -1.0, ..gr() }.check(), Err(GeorefError::NotInvertible));
    assert_eq!(Georef { scale: 1001.0, ..gr() }.check(), Err(GeorefError::NotInvertible));
    assert!(Georef { scale: 1000.0, ..gr() }.check().is_ok());
    assert_eq!(Georef { northing: 2e9, ..gr() }.check(), Err(GeorefError::OutOfRange("map coordinate")));
    assert!(Georef { latitude: 90.0, longitude: -180.0, ..gr() }.check().is_ok());
    assert!(gr().check().is_ok());
}

fn scene(g: Option<Georef>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![],
        wall_penetrations: vec![],
        slabs: vec![],
        shafts: vec![],
        floor_penetrations: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        wall_types: vec![],
        wall_joins: vec![],
        georef: g,
        pset_map: Default::default(),
    }
}

#[test]
fn without_a_position_the_file_has_no_map_and_an_empty_site() {
    let out = export(&scene(None)).unwrap();
    assert_eq!(out.report.map_conversions, 0);
    assert!(!out.text.contains("IFCMAPCONVERSION") && !out.text.contains("IFCPROJECTEDCRS"));
    assert!(out.text.contains("IFCSITE(") && out.text.contains(".ELEMENT.,$,$,$,$,$)"));
    assert!(out.text.contains("1.E-6,#2,$)"), "no true north");
}

#[test]
fn with_a_position_the_file_has_the_map_the_north_and_the_site_position() {
    let g = Georef { grid_angle_deg: 90.0, true_north_deg: 90.0, latitude: -11.5, longitude: 104.5, ..gr() };
    let out = export(&scene(Some(g))).unwrap();
    assert_eq!(out.report.map_conversions, 1);
    assert_eq!(out.text.matches("IFCMAPCONVERSION(").count(), 1);
    let ctx = out.text.lines().find(|l| l.contains("IFCGEOMETRICREPRESENTATIONCONTEXT(")).unwrap();
    assert!(ctx.contains(",#") && !ctx.ends_with(",$);"), "{ctx}");
    assert!(out.text.contains("IFCDIRECTION((-1.0,0.0))") || out.text.contains("IFCDIRECTION((-1.0,0.))"), "true north is written");
    assert!(out.text.contains("IFCPROJECTEDCRS('X:1',$,$,$,$,$,#"));
    assert!(out.text.contains("1000.0,2000.0,5.0,0.,1.0,1.0)"), "{}", out.text.lines().find(|l| l.contains("MAPCONV")).unwrap());
    assert!(out.text.contains("(-11,-30,0,0),(104,30,0,0),5.0,$,$)"));
}

#[test]
fn a_bad_position_stops_the_export() {
    let g = Georef { latitude: 91.0, ..gr() };
    assert!(matches!(export(&scene(Some(g))), Err(ExportError::BadGeoref(GeorefError::OutOfRange("latitude")))));
}
