use openbim_arch::{Level, Levels};
use openbim_geom::WallChain;
use openbim_ifc::{ColumnItem, Scene, WallItem};
use openbim_view::{Batch, MeshCache};
use openbim_viewport::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-6 }
const SIZE: [f64; 2] = [800.0, 600.0];

#[test]
fn the_default_3d_view_looks_from_the_south_east_and_above() {
    let c = Camera::default_3d([0.0; 3], 10.0);
    let e = c.eye();
    assert!(e[0] > 0.0 && e[1] < 0.0 && e[2] > 0.0, "{e:?}");
    assert!(close(e[0], -e[1]));
    let f = c.forward();
    assert!(f[0] < 0.0 && f[1] > 0.0 && f[2] < 0.0);
    assert!(c.up()[2] > 0.0);
}

#[test]
fn mouse_buttons_map_as_in_revit() {
    let m = |b, s| Camera::drag_kind(Buttons { middle: b, ..Default::default() }, Mods { shift: s, ctrl: false });
    assert_eq!(m(true, false), Drag::Pan);
    assert_eq!(m(true, true), Drag::Orbit);
    assert_eq!(m(false, true), Drag::Nothing);
    assert_eq!(Camera::drag_kind(Buttons { left: true, ..Default::default() }, Mods::default()), Drag::Nothing);
}

#[test]
fn orbit_turns_about_the_target_and_cannot_go_over_the_pole() {
    let mut c = Camera::default_3d([1.0, 2.0, 3.0], 10.0);
    let d0 = c.distance;
    c.orbit(120.0, -40.0);
    assert!(close(c.distance, d0) && c.target == [1.0, 2.0, 3.0]);
    let e = c.eye();
    let dist = ((e[0] - 1.0).powi(2) + (e[1] - 2.0).powi(2) + (e[2] - 3.0).powi(2)).sqrt();
    assert!(close(dist, 10.0));
    c.orbit(0.0, 1e6);
    assert!(c.pitch < std::f64::consts::FRAC_PI_2);
    c.orbit(0.0, -1e7);
    assert!(c.pitch > -std::f64::consts::FRAC_PI_2);
}

#[test]
fn pan_moves_the_target_in_the_view_plane_by_the_pixels_dragged() {
    let mut c = Camera::default_3d([0.0; 3], 10.0);
    let p = [3.0, 4.0, 1.0];
    let at = c.project(p, SIZE).unwrap();
    c.pan(50.0, -20.0, SIZE[1]);
    let after = c.project(p, SIZE).unwrap();
    // the picture follows the mouse: a point seen at x moves to x + 50 and y - 20 (for points at the target depth, closely)
    assert!((after[0] - at[0] - 50.0).abs() < 8.0 && (after[1] - at[1] + 20.0).abs() < 8.0, "{at:?} {after:?}");
    assert!(close(c.distance, 10.0));
}

#[test]
fn zoom_keeps_the_point_under_the_cursor_where_it_is() {
    let mut c = Camera::default_3d([0.0; 3], 20.0);
    let cursor = [600.0, 200.0];
    // the model point under the cursor on the plane through the target
    let s = c.metres_per_pixel(SIZE[1]);
    let off = [cursor[0] - SIZE[0] / 2.0, cursor[1] - SIZE[1] / 2.0];
    let r = c.right();
    let u = c.up();
    let p = [r[0] * off[0] * s - u[0] * off[1] * s, r[1] * off[0] * s - u[1] * off[1] * s, r[2] * off[0] * s - u[2] * off[1] * s];
    let a = c.project(p, SIZE).unwrap();
    assert!((a[0] - cursor[0]).abs() < 1e-6 && (a[1] - cursor[1]).abs() < 1e-6);
    c.zoom_at(cursor, 0.5, SIZE);
    let b = c.project(p, SIZE).unwrap();
    assert!((b[0] - cursor[0]).abs() < 1e-6 && (b[1] - cursor[1]).abs() < 1e-6, "{b:?}");
    assert!(close(c.distance, 10.0));
}

#[test]
fn zoom_to_fit_puts_every_corner_of_the_box_inside_the_picture() {
    let (min, max) = ([-3.0, 2.0, 0.0], [17.0, 12.0, 6.0]);
    let mut c = Camera::default_3d([0.0; 3], 1.0);
    c.fit(min, max, SIZE[0] / SIZE[1]);
    for x in [min[0], max[0]] { for y in [min[1], max[1]] { for z in [min[2], max[2]] {
        let p = c.project([x, y, z], SIZE).unwrap();
        assert!(p[0] > 0.0 && p[0] < SIZE[0] && p[1] > 0.0 && p[1] < SIZE[1], "{p:?}");
    } } }
}

#[test]
fn the_view_cube_snaps_to_faces_edges_and_corners() {
    assert_eq!(CubeDir::all().len(), 26);
    let mut c = Camera::default_3d([0.0; 3], 10.0);
    c.snap_to(CubeDir::FRONT);
    let f = c.forward();
    assert!(close(f[0], 0.0) && close(f[1], 1.0) && close(f[2], 0.0), "{f:?}"); // looking north
    assert_eq!(CubeDir::nearest(&c), CubeDir::FRONT);
    c.snap_to(CubeDir::TOP);
    assert!(c.pitch > 1.5);
    c.snap_to(CubeDir([1, -1, 1]));
    assert_eq!(CubeDir::nearest(&c), CubeDir([1, -1, 1]));
    // the default view is nearest to the south-east-top corner
    assert_eq!(CubeDir::nearest(&Camera::default_3d([0.0; 3], 10.0)), CubeDir([1, -1, 1]));
}

fn model() -> Batch {
    let mut s = Scene {
        project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], slab_voids: vec![], networks: vec![],
        families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
    };
    // a wall in front of a column, seen from the south
    s.walls = vec![WallItem { id: 1, name: "W".into(), chain: WallChain::new(vec![[-1.0, 0.0], [1.0, 0.0]], false, 0.2, 2.0), wall_type: None, room_bounding: true }];
    s.columns = vec![ColumnItem { id: 2, name: "C".into(), footprint: vec![[-0.3, 3.0], [0.3, 3.0], [0.3, 3.6], [-0.3, 3.6]], base_z: 0.0, height: 4.0, room_bounding: false }];
    Batch::new(&MeshCache::new().update(&s).unwrap().meshes)
}

fn south_camera() -> Camera {
    let mut c = Camera::default_3d([0.0, 1.5, 1.5], 12.0);
    c.snap_to(CubeDir::FRONT);
    c
}

#[test]
fn nearer_things_hide_farther_ones_in_the_shaded_picture() {
    let b = model();
    let c = south_camera();
    let img = render(&b, &c, 400, 300, Style::ConsistentColors);
    // the column's top (z 4) shows above the wall (z 2); at the wall's height the wall covers the column
    let col = c.project([0.0, 3.0, 3.5], [400.0, 300.0]).unwrap();
    let wall = c.project([0.0, -0.1, 1.0], [400.0, 300.0]).unwrap();
    let pc = img.pixel(col[0] as usize, col[1] as usize);
    let pw = img.pixel(wall[0] as usize, wall[1] as usize);
    assert_ne!(pc, [255, 255, 255]);
    assert_ne!(pw, [255, 255, 255]);
    assert_ne!(pc, pw, "wall and column have different colours");
    let hidden = c.project([0.0, 3.0, 1.0], [400.0, 300.0]).unwrap();
    assert_eq!(img.pixel(hidden[0] as usize, hidden[1] as usize), pw, "the column behind the wall is hidden by it");
    // the corner of the picture is background
    assert_eq!(img.pixel(2, 2), [255, 255, 255]);
}

#[test]
fn hidden_line_draws_edges_that_are_seen_and_not_those_behind_a_face() {
    let b = model();
    let c = south_camera();
    let img = render(&b, &c, 400, 300, Style::HiddenLine);
    // a line is one pixel wide: look at the pixel and its neighbours
    let dark = |x: f64, y: f64| (-1i32..=1).any(|dy| (-1i32..=1).any(|dx| img.pixel((x as i32 + dx) as usize, (y as i32 + dy) as usize)[0] < 80));
    // the wall's front top edge is a line
    let a = c.project([-1.0, -0.1, 2.0], [400.0, 300.0]).unwrap();
    let e = c.project([1.0, -0.1, 2.0], [400.0, 300.0]).unwrap();
    assert!(dark((a[0] + e[0]) / 2.0, (a[1] + e[1]) / 2.0));
    // the foot of the column is behind the wall: its edge does not show
    let f0 = c.project([-0.3, 3.0, 0.0], [400.0, 300.0]).unwrap();
    let f1 = c.project([0.3, 3.0, 0.0], [400.0, 300.0]).unwrap();
    assert!(!dark((f0[0] + f1[0]) / 2.0, (f0[1] + f1[1]) / 2.0));
    // Wireframe shows it
    let wf = render(&b, &c, 400, 300, Style::Wireframe);
    let (mx, my) = ((f0[0] + f1[0]) / 2.0, (f0[1] + f1[1]) / 2.0);
    assert!((-1i32..=1).any(|dy| (-1i32..=1).any(|dx| wf.pixel((mx as i32 + dx) as usize, (my as i32 + dy) as usize)[0] < 80)));
}

/// Walls that meet: the end of one runs into the face of the other, so no edge of either mesh lies on the line
/// where their faces cross. It has to be drawn all the same.
fn t_junction() -> Batch {
    let mut s = Scene {
        project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], slab_voids: vec![], networks: vec![],
        families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
    };
    s.walls = vec![
        WallItem { id: 1, name: "A".into(), chain: WallChain::new(vec![[-3.0, 0.0], [3.0, 0.0]], false, 0.2, 2.0), wall_type: None, room_bounding: true },
        WallItem { id: 2, name: "B".into(), chain: WallChain::new(vec![[0.0, 0.0], [0.0, -3.0]], false, 0.15, 2.0), wall_type: None, room_bounding: true },
    ];
    Batch::new(&MeshCache::new().update(&s).unwrap().meshes)
}

#[test]
fn hidden_line_draws_the_line_where_two_walls_meet() {
    let b = t_junction();
    let mut c = Camera::default_3d([0.0, -1.0, 1.0], 12.0);
    c.fit([-3.0, -3.0, 0.0], [3.0, 0.1, 2.0], 4.0 / 3.0);
    let img = render(&b, &c, 800, 600, Style::HiddenLine);
    // B's east face (x = 0.075) meets A's south face (y = -0.1): a vertical line from z = 0 to 2
    for z in [0.4, 1.0, 1.6] {
        let p = c.project([0.075, -0.1, z], [800.0, 600.0]).unwrap();
        let dark = (-1i32..=1).any(|dx| (-1i32..=1).any(|dy| img.pixel((p[0] as i32 + dx) as usize, (p[1] as i32 + dy) as usize)[0] < 80));
        assert!(dark, "no line at z = {z}, pixel {p:?}");
    }
    // and nothing is drawn in the middle of the open face of A, away from any edge
    let q = c.project([2.0, -0.1, 1.0], [800.0, 600.0]).unwrap();
    assert_eq!(img.pixel(q[0] as usize, q[1] as usize), [255, 255, 255]);
}

#[test]
fn picking_finds_the_front_element_and_respects_the_section_box() {
    use openbim_viewport::{pick, SectionBox};
    let b = model();
    let mut cam = Camera::default_3d([0.0, 1.8, 2.0], 20.0);
    cam.fit([-1.0, 0.0, 0.0], [1.0, 3.6, 4.0], 4.0 / 3.0);
    let size = [400.0, 300.0];
    // scan the picture: something is hit, and the pixel of the hit point finds the same element again
    let mut hit = None;
    'scan: for y in (0..300).step_by(10) { for x in (0..400).step_by(10) {
        if let Some(h) = pick(&b, &cam, [x as f64, y as f64], size, None) { hit = Some((h, [x as f64, y as f64])); break 'scan; }
    } }
    let (hit, px) = hit.expect("the building is somewhere in the picture");
    assert!(hit.distance > 0.0);
    assert_eq!(pick(&b, &cam, px, size, None).unwrap().element, hit.element);
    assert!(pick(&b, &cam, [0.0, 0.0], size, None).is_none(), "nothing in the corner");
    let away = SectionBox { min: [100.0; 3], max: [101.0; 3] };
    assert!(pick(&b, &cam, px, size, Some(away)).is_none());
}

#[test]
fn blending_cameras_turns_the_short_way() {
    let a = Camera { target: [0.0; 3], distance: 10.0, yaw: 3.0, pitch: 0.2 };
    let b = Camera { target: [2.0, 0.0, 0.0], distance: 40.0, yaw: -3.0, pitch: 0.6 };
    let m = Camera::blend(&a, &b, 0.5);
    assert!((m.distance - 20.0).abs() < 1e-9, "geometric mean");
    assert!(m.yaw.abs() > 3.0 || (m.yaw.abs() - std::f64::consts::PI).abs() < 0.2, "through the back, not through zero: {}", m.yaw);
    assert_eq!(Camera::blend(&a, &b, 0.0), a);
    assert!((Camera::blend(&a, &b, 1.0).pitch - 0.6).abs() < 1e-12);
}

#[test]
fn the_sun_is_where_astronomy_puts_it() {
    use openbim_viewport::Sun;
    // Phnom Penh on the equinox: noon sun almost overhead, rising in the east and setting in the west
    let at = |hour: f64| Sun { month: 3, day: 20, hour, ..Sun::default() }.position();
    let noon = (0..240).map(|i| at(10.0 + i as f64 / 60.0).elevation).fold(f64::MIN, f64::max);
    assert!((noon - (90.0 - 11.5564)).abs() < 1.5, "noon elevation {noon}");
    let (dawn, dusk) = (at(6.0), at(18.0));
    assert!((dawn.azimuth - 90.0).abs() < 8.0 && dawn.elevation.abs() < 8.0, "{dawn:?}");
    assert!((dusk.azimuth - 270.0).abs() < 8.0 && dusk.elevation.abs() < 8.0, "{dusk:?}");
    assert!(at(0.0).elevation < -50.0, "night");
    // London at the summer solstice: 62 degrees at noon, due south; at the winter solstice 15 degrees
    let london = |m, d| Sun { latitude: 51.5, longitude: 0.0, utc_offset: 0.0, month: m, day: d, hour: 12.0, project_north_deg: 0.0 }.position();
    let (s, w) = (london(6, 21), london(12, 21));
    assert!((s.elevation - 62.0).abs() < 1.0 && (s.azimuth - 180.0).abs() < 4.0, "{s:?}");
    assert!((w.elevation - 15.0).abs() < 1.0, "{w:?}");
    // southern hemisphere: the noon sun is in the north
    let sydney = Sun { latitude: -33.9, longitude: 151.2, utc_offset: 10.0, month: 6, day: 21, hour: 12.0, project_north_deg: 0.0 }.position();
    assert!(sydney.azimuth < 30.0 || sydney.azimuth > 330.0, "{sydney:?}");
    // the direction vector points the same way: east of south in the morning has positive x
    let d = Sun { month: 3, day: 20, hour: 9.0, ..Sun::default() }.direction();
    assert!(d[0] > 0.3 && d[2] > 0.3, "{d:?}");
    assert!(Sun { hour: 0.0, ..Sun::default() }.strength() == 0.0);
}

#[test]
fn project_north_turns_the_sun_with_the_georeference() {
    use openbim_ifc::georef::{CoordinateBase, Crs, Georef};
    use openbim_viewport::Sun;
    // Phnom Penh at dawn on the equinox: the sun is in the east
    let dawn = Sun { month: 3, day: 20, hour: 6.0, ..Sun::default() };
    let d = dawn.direction();
    assert!(d[0] > 0.9, "east is +x when project north is true north: {d:?}");
    // project north turned 90 degrees counter-clockwise from true north points west, so east is project south
    let turned = Sun { project_north_deg: 90.0, ..dawn }.direction();
    assert!(turned[1] < -0.9 && turned[0].abs() < 0.2, "{turned:?}");
    // a full turn changes nothing
    let full = Sun { project_north_deg: 360.0, ..dawn }.direction();
    assert!((0..3).all(|k| (full[k] - d[k]).abs() < 1e-9));
    // the georeference gives the place and the angle
    let g = Georef { crs: Crs { name: "site".into(), geodetic_datum: None, vertical_datum: None, map_projection: None, map_zone: None },
        project_base_point: [0.0; 3], survey_point: [0.0; 3], survey_shared: [0.0; 3], angle_to_true_north_deg: 30.0, scale: 1.0,
        coordinate_base: CoordinateBase::InternalOrigin, latitude: 51.5, longitude: -0.12, elevation: 0.0 };
    let s = Sun::default().at_site(&g);
    assert_eq!((s.latitude, s.longitude, s.project_north_deg), (51.5, -0.12, 30.0));
    assert_eq!((s.month, s.hour), (Sun::default().month, Sun::default().hour));
}

#[test]
fn the_sun_and_the_ifc_true_north_direction_agree_on_the_sign_of_the_angle() {
    use openbim_ifc::georef::{CoordinateBase, Crs, Georef};
    use openbim_viewport::Sun;
    // at the June solstice at noon the sun of Phnom Penh is due north, so its horizontal direction in the model is the
    // direction of true north in the model's plan, which is what a file carries as TrueNorth
    for angle in [0.0, 33.0, 90.0, -40.0, 180.0] {
        let g = Georef { crs: Crs { name: "site".into(), geodetic_datum: None, vertical_datum: None, map_projection: None, map_zone: None },
            project_base_point: [0.0; 3], survey_point: [0.0; 3], survey_shared: [0.0; 3], angle_to_true_north_deg: angle, scale: 1.0,
            coordinate_base: CoordinateBase::InternalOrigin, latitude: 11.5564, longitude: 104.9282, elevation: 0.0 };
        let sun = Sun { month: 6, day: 21, hour: 12.1, utc_offset: 7.0, ..Sun::default() }.at_site(&g);
        let p = sun.position();
        assert!(p.azimuth < 5.0 || p.azimuth > 355.0, "due north: {p:?}");
        let d = sun.direction();
        let h = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let (tx, ty) = g.true_north();
        assert!((d[0] / h - tx).abs() < 0.12 && (d[1] / h - ty).abs() < 0.12, "angle {angle}: sun {:?} against TrueNorth {:?}", (d[0] / h, d[1] / h), (tx, ty));
    }
}
