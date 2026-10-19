use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use openbim_viewport::{Camera, CubeDir};
use openbim_viewport_egui::widget::{cube_cells, pick_cell};
use openbim_viewport_egui::Viewport3d;

#[test]
fn default_view_shows_three_faces_and_27_cells() {
    let cam = Camera::default_3d([0.0; 3], 10.0);
    let cells = cube_cells(&cam, Pos2::new(100.0, 100.0), 30.0);
    assert_eq!(cells.len(), 27);
    assert_eq!(cells.iter().filter(|c| c.is_face).count(), 3);
    // from the south-east above: TOP, FRONT and RIGHT are the faces that show
    let faces: Vec<CubeDir> = cells.iter().filter(|c| c.is_face).map(|c| c.dir).collect();
    for f in [CubeDir::TOP, CubeDir::FRONT, CubeDir::RIGHT] { assert!(faces.contains(&f), "{f:?} visible"); }
}

#[test]
fn every_place_can_be_clicked_and_snaps_the_camera() {
    // from a view that shows the corner, click each cell's middle: it picks that place, and snapping goes there
    let mut cam = Camera::default_3d([0.0; 3], 10.0);
    let cells = cube_cells(&cam, Pos2::new(100.0, 100.0), 30.0);
    for c in &cells {
        let mid = Pos2::new(c.quad.iter().map(|q| q.x).sum::<f32>() / 4.0, c.quad.iter().map(|q| q.y).sum::<f32>() / 4.0);
        assert_eq!(pick_cell(&cells, mid), Some(c.dir));
    }
    let top = cells.iter().find(|c| c.dir == CubeDir::TOP).unwrap();
    let mid = Pos2::new(top.quad.iter().map(|q| q.x).sum::<f32>() / 4.0, top.quad.iter().map(|q| q.y).sum::<f32>() / 4.0);
    cam.snap_to(pick_cell(&cells, mid).unwrap());
    assert!(cam.pitch > 1.5, "looking down from above");
    assert_eq!(pick_cell(&cells, Pos2::new(500.0, 500.0)), None);
    // after the snap the cube shows the TOP face square on
    let top_view = cube_cells(&cam, Pos2::new(100.0, 100.0), 30.0);
    assert!(top_view.iter().any(|c| c.is_face && c.dir == CubeDir::TOP));
}

fn frame(ctx: &egui::Context, vp: &mut Viewport3d, events: Vec<Event>, _modifiers: Modifiers) {
    let mut raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events, ..RawInput::default() };
    raw.viewports.entry(raw.viewport_id).or_default().native_pixels_per_point = Some(1.0);
    let out = ctx.run_ui(raw, |ui| { vp.ui(ui); });
    out.drop_without_applying_deltas();
}

#[test]
fn middle_drag_pans_shift_middle_orbits_wheel_zooms() {
    let ctx = egui::Context::default();
    let mut vp = Viewport3d::new();
    let (start, end) = (Pos2::new(300.0, 300.0), Pos2::new(360.0, 330.0));
    let press = |p, down| Event::PointerButton { pos: p, button: PointerButton::Middle, pressed: down, modifiers: Modifiers::NONE };
    frame(&ctx, &mut vp, vec![Event::PointerMoved(start)], Modifiers::NONE);

    // plain middle drag pans: the target moves, the direction of view does not
    let c0 = vp.camera;
    frame(&ctx, &mut vp, vec![press(start, true)], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![Event::PointerMoved(end)], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![press(end, false)], Modifiers::NONE);
    assert_ne!(vp.camera.target, c0.target, "panned");
    assert_eq!((vp.camera.yaw, vp.camera.pitch), (c0.yaw, c0.pitch));

    // Shift + middle drag orbits
    let c1 = vp.camera;
    let shift = Modifiers { shift: true, ..Modifiers::NONE };
    frame(&ctx, &mut vp, vec![Event::ModifiersChanged(shift), Event::PointerMoved(start)], shift);
    frame(&ctx, &mut vp, vec![Event::PointerButton { pos: start, button: PointerButton::Middle, pressed: true, modifiers: shift }], shift);
    frame(&ctx, &mut vp, vec![Event::PointerMoved(end)], shift);
    frame(&ctx, &mut vp, vec![Event::PointerButton { pos: end, button: PointerButton::Middle, pressed: false, modifiers: shift }, Event::ModifiersChanged(Modifiers::NONE)], shift);
    assert_ne!(vp.camera.yaw, c1.yaw, "orbited");
    assert_eq!(vp.camera.target, c1.target);

    // wheel forward comes closer
    let d0 = vp.camera.distance;
    frame(&ctx, &mut vp, vec![Event::PointerMoved(Pos2::new(400.0, 300.0))], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, 50.0), phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE }], Modifiers::NONE);
    assert!(vp.camera.distance < d0, "zoomed in: {} -> {}", d0, vp.camera.distance);
}

#[test]
fn view_cube_snap_is_animated_and_ends_at_the_target() {
    let mut vp = Viewport3d::new();
    let start = vp.camera;
    vp.snap_to(CubeDir::TOP, 10.0);
    assert_eq!(vp.camera, start, "nothing moves at once");
    assert!(vp.step_animation(10.1));
    let mid = vp.camera;
    assert!(mid.pitch > start.pitch && mid.pitch < 1.55, "part of the way: {}", mid.pitch);
    assert!(!vp.step_animation(11.0), "over");
    assert!(vp.camera.pitch > 1.5);
    vp.snap_seconds = 0.0;
    vp.snap_to(CubeDir::FRONT, 20.0);
    assert!(vp.camera.pitch.abs() < 1e-9, "no animation: there at once");
}

#[test]
fn clicking_selects_the_element_under_the_cursor() {
    use openbim_view::Batch;
    let ctx = egui::Context::default();
    let mut vp = Viewport3d::new();
    let batch = {
        use openbim_arch::{Level, Levels};
        use openbim_geom::WallChain;
        use openbim_ifc::{Scene, WallItem};
        let s = Scene {
            project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(),
            levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
            walls: vec![WallItem { id: 1, name: "W".into(), chain: WallChain::new(vec![[0.0, 0.0], [6.0, 0.0]], false, 0.3, 3.0), wall_type: None, room_bounding: true }],
            wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], slab_voids: vec![], networks: vec![],
            families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
        };
        Batch::new(&openbim_view::MeshCache::new().update(&s).unwrap().meshes)
    };
    vp.set_batch(std::sync::Arc::new(batch));
    vp.zoom_to_fit(800.0 / 600.0);
    frame(&ctx, &mut vp, vec![], Modifiers::NONE);
    // the wall is in the middle of the picture
    let p = Pos2::new(400.0, 300.0);
    frame(&ctx, &mut vp, vec![Event::PointerMoved(p)], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    assert_eq!(vp.selection().len(), 1, "wall selected; hit {:?}", vp.last_hit);
    // clicking on empty paper clears it
    let q = Pos2::new(20.0, 580.0);
    frame(&ctx, &mut vp, vec![Event::PointerMoved(q)], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], Modifiers::NONE);
    frame(&ctx, &mut vp, vec![Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    assert!(vp.selection().is_empty());
}
