use openbim_arch::{Level, Levels};
use openbim_ifc::georef::*;
use openbim_ifc::*;

fn gr() -> Georef {
    Georef {
        crs: Crs { name: "X:1".into(), geodetic_datum: None, vertical_datum: None, map_projection: None, map_zone: None },
        project_base_point: [0.0; 3],
        survey_point: [0.0; 3],
        survey_shared: [1000.0, 2000.0, 5.0],
        angle_to_true_north_deg: 0.0,
        scale: 1.0,
        coordinate_base: CoordinateBase::InternalOrigin,
        latitude: 10.0,
        longitude: 100.0,
        elevation: 5.0,
    }
}
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
fn near3(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| near(a[i], b[i]))
}

#[test]
fn the_survey_point_has_the_shared_coordinates_it_was_given_wherever_it_is() {
    let g = Georef { survey_point: [3.0, 4.0, 1.0], survey_shared: [500.0, 700.0, 20.0], angle_to_true_north_deg: 33.0, scale: 0.9996, ..gr() };
    assert!(near3(g.to_shared([3.0, 4.0, 1.0]), [500.0, 700.0, 20.0]));
    // the internal origin is then not at the shared origin
    let o = g.origin_shared();
    assert!(!near(o[0], 500.0) && near3(g.to_shared([0.0; 3]), o));
    // the project base point is another point of the model, with shared coordinates of its own
    let h = Georef { project_base_point: [10.0, 0.0, 0.0], ..g.clone() };
    let p = h.project_base_point_shared();
    let d = ((p[0] - o[0]).powi(2) + (p[1] - o[1]).powi(2)).sqrt();
    assert!(near(d, 10.0 * 0.9996));
}

#[test]
fn the_axes_land_where_the_map_says_and_the_angle_turns_project_north() {
    let g = gr();
    assert_eq!(g.to_shared([0.0, 0.0, 0.0]), [1000.0, 2000.0, 5.0]);
    assert_eq!(g.to_shared([3.0, 4.0, 1.0]), [1003.0, 2004.0, 6.0]);
    // project north 90 degrees counter-clockwise of true north: the model's X axis points to map north, its Y axis to map west
    let q = Georef { angle_to_true_north_deg: 90.0, ..gr() };
    assert!(near3(q.to_shared([1.0, 0.0, 0.0]), [1000.0, 2001.0, 5.0]));
    assert!(near3(q.to_shared([0.0, 1.0, 0.0]), [999.0, 2000.0, 5.0]));
}

#[test]
fn scale_multiplies_lengths_and_heights_and_the_inverse_undoes_it() {
    let g = Georef { scale: 0.9996, angle_to_true_north_deg: 33.0, survey_point: [1.0, 2.0, 3.0], ..gr() };
    let (a, b) = (g.to_shared([0.0, 0.0, 0.0]), g.to_shared([10.0, 0.0, 2.0]));
    assert!(near(((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt(), 9.996));
    assert!(near(b[2] - a[2], 2.0 * 0.9996));
    for p in [[0.0, 0.0, 0.0], [12.5, -3.0, 4.0], [-100.0, 250.0, -2.0]] {
        let back = g.to_model(g.to_shared(p));
        assert!(near3(back, p), "{p:?} {back:?}");
    }
}

#[test]
fn true_north_in_the_model_is_map_north_turned_back_by_the_angle() {
    assert_eq!(gr().true_north(), (0.0, 1.0));
    // project north turned 90 degrees counter-clockwise of true north: true north is to the right of project north
    let t = Georef { angle_to_true_north_deg: 90.0, ..gr() }.true_north();
    assert!(near(t.0, 1.0) && near(t.1, 0.0), "{t:?}");
    assert_eq!(Georef { angle_to_true_north_deg: 180.0, ..gr() }.true_north(), (0.0, -1.0));
    assert_eq!(Georef { angle_to_true_north_deg: -90.0, ..gr() }.true_north(), (-1.0, 0.0));
    assert_eq!(Georef { angle_to_true_north_deg: 270.0, ..gr() }.x_axis(), (0.0, -1.0));
    // a vector pointing to true north in the model maps to map north
    let g = Georef { angle_to_true_north_deg: 21.0, ..gr() };
    let (x, y) = g.true_north();
    let (a, b) = (g.to_shared([0.0; 3]), g.to_shared([x, y, 0.0]));
    assert!(near(b[0] - a[0], 0.0) && near(b[1] - a[1], 1.0));
}

#[test]
fn each_coordinate_base_puts_the_files_origin_where_revit_does() {
    let g = Georef { project_base_point: [2.0, 3.0, 0.5], survey_point: [10.0, 20.0, 1.0], survey_shared: [500.0, 600.0, 30.0], angle_to_true_north_deg: 30.0, ..gr() };
    let p = [12.0, 23.0, 1.5];
    // internal origin: the model as it is; the conversion maps its origin
    let i = Georef { coordinate_base: CoordinateBase::InternalOrigin, ..g.clone() };
    assert_eq!(i.to_file(p), p);
    assert!(near3(i.map_conversion().0, i.origin_shared()));
    // project base point and survey point: the model moves so that the point is the origin; the conversion maps that point
    let b = Georef { coordinate_base: CoordinateBase::ProjectBasePoint, ..g.clone() };
    assert_eq!(b.to_file(p), [10.0, 20.0, 1.0]);
    assert_eq!(b.to_file([2.0, 3.0, 0.5]), [0.0, 0.0, 0.0]);
    assert!(near3(b.map_conversion().0, b.project_base_point_shared()));
    let s = Georef { coordinate_base: CoordinateBase::SurveyPoint, ..g.clone() };
    assert_eq!(s.to_file([10.0, 20.0, 1.0]), [0.0, 0.0, 0.0]);
    assert!(near3(s.map_conversion().0, [500.0, 600.0, 30.0]));
    // shared coordinates: the model is written in the surveyor's system, and the conversion does nothing
    let c = Georef { coordinate_base: CoordinateBase::SharedCoordinates, ..g };
    assert!(near3(c.to_file([10.0, 20.0, 1.0]), [500.0, 600.0, 30.0]));
    assert_eq!(c.map_conversion(), ([0.0; 3], (1.0, 0.0)));
    assert_eq!(c.file_true_north(), (0.0, 1.0));
    for q in [[0.0; 3], p, [-4.0, 7.0, 2.0]] {
        assert!(near3(c.to_file(q), c.to_shared(q)), "{q:?}");
    }
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
    assert_eq!(Georef { survey_shared: [f64::NAN, 0.0, 0.0], ..gr() }.check(), Err(GeorefError::NotFinite("survey point shared coordinates")));
    assert_eq!(Georef { latitude: 90.5, ..gr() }.check(), Err(GeorefError::OutOfRange("latitude")));
    assert_eq!(Georef { latitude: -90.5, ..gr() }.check(), Err(GeorefError::OutOfRange("latitude")));
    assert_eq!(Georef { longitude: 180.5, ..gr() }.check(), Err(GeorefError::OutOfRange("longitude")));
    assert_eq!(Georef { longitude: -180.5, ..gr() }.check(), Err(GeorefError::OutOfRange("longitude")));
    assert_eq!(Georef { scale: 0.0, ..gr() }.check(), Err(GeorefError::NotInvertible));
    assert_eq!(Georef { scale: -1.0, ..gr() }.check(), Err(GeorefError::NotInvertible));
    assert_eq!(Georef { scale: 1001.0, ..gr() }.check(), Err(GeorefError::NotInvertible));
    assert!(Georef { scale: 1000.0, ..gr() }.check().is_ok());
    assert_eq!(Georef { survey_shared: [0.0, 2e9, 0.0], ..gr() }.check(), Err(GeorefError::OutOfRange("survey point shared coordinates")));
    assert_eq!(Georef { survey_point: [0.0, f64::INFINITY, 0.0], ..gr() }.check(), Err(GeorefError::NotFinite("survey point")));
    assert_eq!(Georef { project_base_point: [1e10, 0.0, 0.0], ..gr() }.check(), Err(GeorefError::OutOfRange("project base point")));
    assert_eq!(Georef { angle_to_true_north_deg: f64::NAN, ..gr() }.check(), Err(GeorefError::NotFinite("angle to true north")));
    assert_eq!(Georef { coordinate_base: CoordinateBase::SharedCoordinates, scale: 0.9996, ..gr() }.check(), Err(GeorefError::OutOfRange("scale with the shared coordinates as base")));
    assert!(Georef { coordinate_base: CoordinateBase::SharedCoordinates, ..gr() }.check().is_ok());
    assert!(Georef { latitude: 90.0, longitude: -180.0, ..gr() }.check().is_ok());
    assert!(gr().check().is_ok());
}

fn scene(g: Option<Georef>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        beams: vec![], materials: Default::default(),
        timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![],
        wall_penetrations: vec![],
        firestops: Default::default(),
        slabs: vec![],
        columns: vec![],
        shafts: vec![],
        floor_penetrations: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        wall_types: vec![],
        wall_joins: vec![],
        georef: g,
        room_settings: Default::default(),
        separation_lines: vec![],
        room_lighting: Default::default(),
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
    let g = Georef { angle_to_true_north_deg: 90.0, latitude: -11.5, longitude: 104.5, ..gr() };
    let out = export(&scene(Some(g))).unwrap();
    assert_eq!(out.report.map_conversions, 1);
    assert_eq!(out.text.matches("IFCMAPCONVERSION(").count(), 1);
    let ctx = out.text.lines().find(|l| l.contains("IFCGEOMETRICREPRESENTATIONCONTEXT(")).unwrap();
    assert!(ctx.contains(",#") && !ctx.ends_with(",$);"), "{ctx}");
    assert!(out.text.contains("IFCDIRECTION((1.0,0.))") || out.text.contains("IFCDIRECTION((1.0,0.0))"), "true north is written");
    assert!(out.text.contains("IFCPROJECTEDCRS('X:1',$,$,$,$,$,#"));
    assert!(out.text.contains("1000.0,2000.0,5.0,0.,1.0,1.0)"), "{}", out.text.lines().find(|l| l.contains("MAPCONV")).unwrap());
    assert!(out.text.contains("(-11,-30,0,0),(104,30,0,0),5.0,$,$)"));
}

#[test]
fn a_bad_position_stops_the_export() {
    let g = Georef { latitude: 91.0, ..gr() };
    assert!(matches!(export(&scene(Some(g))), Err(ExportError::BadGeoref(GeorefError::OutOfRange("latitude")))));
}

#[test]
fn the_site_carries_the_model_to_the_chosen_origin() {
    let g = Georef { project_base_point: [2.0, 3.0, 0.5], survey_point: [10.0, 20.0, 1.0], survey_shared: [500.0, 600.0, 30.0], ..gr() };
    let has = |base: CoordinateBase, needle: &str| {
        let out = export(&scene(Some(Georef { coordinate_base: base, ..g.clone() }))).unwrap();
        out.text.contains(needle)
    };
    assert!(has(CoordinateBase::ProjectBasePoint, "IFCCARTESIANPOINT((-2.0,-3.0,-0.5))"));
    assert!(has(CoordinateBase::SurveyPoint, "IFCCARTESIANPOINT((-10.0,-20.0,-1.0))"));
    assert!(!has(CoordinateBase::InternalOrigin, "IFCCARTESIANPOINT((-"));
    // the shared coordinates: the model's origin at its shared place, turned by the angle (none here)
    let shared = Georef { coordinate_base: CoordinateBase::SharedCoordinates, angle_to_true_north_deg: 90.0, ..g.clone() };
    let out = export(&scene(Some(shared.clone()))).unwrap();
    let o = shared.origin_shared();
    assert!(out.text.contains(&format!("IFCCARTESIANPOINT(({},{},{}))", f(o[0]), f(o[1]), f(o[2]))), "{o:?}");
    assert!(out.text.lines().any(|l| l.contains("IFCMAPCONVERSION") && l.ends_with(",0.,0.,0.,1.0,0.,1.0);")), "the conversion does nothing");
}
fn f(v: f64) -> String {
    format!("{v:?}")
}
