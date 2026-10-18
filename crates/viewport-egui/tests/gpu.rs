use egui_wgpu::wgpu;
use openbim_arch::{Level, Levels};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{ColumnItem, Scene, SlabItem, SlabRole, WallItem};
use openbim_place::floors::Slab;
use openbim_view::{Batch, MeshCache};
use openbim_viewport::{render, Camera, Style};
use openbim_viewport_egui::gpu::*;

fn building() -> Batch {
    let mut ring = WallChain::new(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 7.0], [0.0, 7.0]], true, 0.3, 3.0);
    ring.openings = vec![vec![Opening { offset: 2.0, width: 1.2, sill: 0.9, height: 1.2 }], vec![], vec![], vec![]];
    let inner = WallChain::new(vec![[5.0, 0.0], [5.0, 7.0]], false, 0.15, 3.0);
    let s = Scene {
        project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![WallItem { id: 1, name: "outer".into(), chain: ring, wall_type: None, room_bounding: true }, WallItem { id: 2, name: "inner".into(), chain: inner, wall_type: None, room_bounding: true }],
        wall_penetrations: vec![], firestops: Default::default(),
        slabs: vec![SlabItem { id: 3, slab: Slab { name: "S".into(), top: 0.0, thickness: 0.25, outline: vec![[-0.3, -0.3], [10.3, -0.3], [10.3, 7.3], [-0.3, 7.3]], fire_rating_min: None }, role: SlabRole::Structure, room_bounding: true }],
        columns: vec![ColumnItem { id: 4, name: "C".into(), footprint: vec![[8.0, 3.0], [8.4, 3.0], [8.4, 3.4], [8.0, 3.4]], base_z: 0.0, height: 3.0, room_bounding: false }],
        shafts: vec![], floor_penetrations: vec![], networks: vec![], families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(),
    };
    Batch::new(&MeshCache::new().update(&s).unwrap().meshes)
}

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { force_fallback_adapter: false, ..Default::default() })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

#[test]
fn shader_is_valid_wgsl() {
    let module = naga::front::wgsl::parse_str(WGSL).expect("parse");
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("validate");
}

fn cam(b: &Batch) -> Camera {
    let mut hi = [f64::MIN; 3];
    for v in &b.verts { for k in 0..3 { hi[k] = hi[k].max(v[k] as f64); } }
    let mut c = Camera::default_3d([b.origin[0] + hi[0] / 2.0, b.origin[1] + hi[1] / 2.0, b.origin[2] + hi[2] / 2.0], 20.0);
    c.fit(b.origin, [b.origin[0] + hi[0], b.origin[1] + hi[1], b.origin[2] + hi[2]], 4.0 / 3.0);
    c
}

/// The fraction of pixels that are not paper white, and how many of them agree between the two pictures.
fn compare(gpu_px: &[u8], sw: &openbim_viewport::Image) -> (f64, f64) {
    let (mut ink, mut both, mut one) = (0usize, 0usize, 0usize);
    for i in 0..sw.width * sw.height {
        let g = gpu_px[i * 4..i * 4 + 3] != [255, 255, 255];
        let s = sw.rgba[i * 4..i * 4 + 3] != [255, 255, 255];
        if g || s { ink += 1; }
        if g && s { both += 1; }
        if g != s { one += 1; }
    }
    (both as f64 / ink.max(1) as f64, one as f64 / (sw.width * sw.height) as f64)
}

#[test]
fn gpu_shaded_agrees_with_software() {
    let Some((device, queue)) = device() else { eprintln!("no wgpu adapter; skipped"); return };
    let b = building();
    let gpu = Gpu::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let c = cam(&b);
    let scene = gpu.upload(&device, &b, Style::Shaded);
    let px = render_rgba(&device, &queue, &gpu, &scene, &c, [400, 300]);
    let sw = render(&b, &c, 400, 300, Style::Shaded);
    let (overlap, mismatch) = compare(&px, &sw);
    let ink = px.chunks(4).filter(|p| p[..3] != [255, 255, 255]).count();
    assert!(ink > 3000, "the building is drawn: {ink}");
    assert!(overlap > 0.95 && mismatch < 0.01, "overlap {overlap}, mismatch {mismatch}");
}

#[test]
fn gpu_hidden_line_and_wireframe_draw_lines() {
    let Some((device, queue)) = device() else { return };
    let b = building();
    let gpu = Gpu::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let c = cam(&b);
    let dark = |px: &[u8]| px.chunks(4).filter(|p| p[0] < 80 && p[1] < 80).count();
    let hidden = render_rgba(&device, &queue, &gpu, &gpu.upload(&device, &b, Style::HiddenLine), &c, [400, 300]);
    let wire = render_rgba(&device, &queue, &gpu, &gpu.upload(&device, &b, Style::Wireframe), &c, [400, 300]);
    assert!(dark(&hidden) > 300, "hidden line has lines: {}", dark(&hidden));
    assert!(dark(&wire) > dark(&hidden), "wireframe shows what hidden line hides: {} vs {}", dark(&wire), dark(&hidden));
    // no grey fill in either: every pixel is white or part of a line
    assert!(hidden.chunks(4).all(|p| p[0] > 200 || p[0] < 120));
}

#[test]
fn edge_extraction_finds_creases() {
    let b = building();
    let creases = openbim_viewport::crease_edges(&b, 25.0);
    assert!(creases.len() > 50, "{}", creases.len());
    assert!(creases.iter().flatten().flatten().all(|x| x.is_finite()));
}

fn ink(px: &[u8]) -> usize { px.chunks(4).filter(|p| p[..3] != [255, 255, 255]).count() }

#[test]
fn section_box_cuts_the_picture() {
    let Some((device, queue)) = device() else { return };
    let b = building();
    let gpu = Gpu::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let c = cam(&b);
    let scene = gpu.upload(&device, &b, Style::Shaded);
    let all = render_rgba_cut(&device, &queue, &gpu, &scene, &c, [400, 300], None);
    // keep only the west half of the building
    let half = openbim_viewport::SectionBox { min: [-1.0, -1.0, -1.0], max: [5.0, 8.0, 4.0] };
    let cut = render_rgba_cut(&device, &queue, &gpu, &scene, &c, [400, 300], Some(half));
    let (a, k) = (ink(&all), ink(&cut));
    assert!(k > 500 && (k as f64) < a as f64 * 0.8, "cut {k} of {a}");
    // a box around everything changes nothing
    let whole = openbim_viewport::SectionBox { min: [-5.0; 3], max: [50.0; 3] };
    assert_eq!(render_rgba_cut(&device, &queue, &gpu, &scene, &c, [400, 300], Some(whole)), all);
}

#[test]
fn realistic_differs_from_shaded_and_selection_is_orange() {
    let Some((device, queue)) = device() else { return };
    let b = building();
    let gpu = Gpu::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let c = cam(&b);
    let shaded = render_rgba(&device, &queue, &gpu, &gpu.upload(&device, &b, Style::Shaded), &c, [400, 300]);
    let real = render_rgba(&device, &queue, &gpu, &gpu.upload(&device, &b, Style::Realistic), &c, [400, 300]);
    assert_ne!(shaded, real);
    assert!((ink(&shaded) as i64 - ink(&real) as i64).abs() < 200, "same silhouette");
    let orange = |px: &[u8]| px.chunks(4).filter(|p| p[0] > 200 && p[1] > 90 && p[1] < 190 && p[2] < 80).count();
    assert_eq!(orange(&shaded), 0);
    let mut scene = gpu.upload(&device, &b, Style::Shaded);
    let one: std::collections::HashSet<_> = [b.elements[0]].into_iter().collect();
    gpu.select(&device, &mut scene, &b, &one);
    let sel = render_rgba(&device, &queue, &gpu, &scene, &c, [400, 300]);
    assert!(orange(&sel) > 300, "selected element is drawn orange: {}", orange(&sel));
}

#[test]
fn realistic_walls_cast_shadows_on_the_floor() {
    let Some((device, queue)) = device() else { return };
    let b = building();
    let gpu = Gpu::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let mut c = cam(&b);
    c.snap_to(openbim_viewport::CubeDir::TOP);
    let size = [800u32, 600];
    let mut scene = gpu.upload(&device, &b, Style::Realistic);
    // the sun in the south-west and high, as a fixed light would be
    scene.sun = openbim_viewport::Sun { latitude: 51.0, longitude: 0.0, utc_offset: 0.0, month: 6, day: 21, hour: 14.0, project_north_deg: 0.0 };
    let px = render_rgba(&device, &queue, &gpu, &scene, &c, size);
    let lum = |p: [f64; 3]| {
        let s = c.project(p, [800.0, 600.0]).unwrap();
        let i = (s[1] as usize * 800 + s[0] as usize) * 4;
        px[i] as f64 + px[i + 1] as f64 + px[i + 2] as f64
    };
    // the sun is in the south-west: the floor behind the south wall is in shadow, the floor in the north-east is lit
    let shadow = lum([1.0, 2.0, 0.25]);
    let sun = lum([7.8, 5.5, 0.25]);
    assert!(shadow < sun * 0.8, "floor in shadow {shadow} against lit floor {sun}");
}

#[test]
fn the_shadow_follows_the_sun_and_night_has_none() {
    let Some((device, queue)) = device() else { return };
    let b = building();
    let gpu = Gpu::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let mut c = cam(&b);
    c.snap_to(openbim_viewport::CubeDir::TOP);
    let lum = |px: &[u8], p: [f64; 3]| {
        let s = c.project(p, [800.0, 600.0]).unwrap();
        let i = (s[1] as usize * 800 + s[0] as usize) * 4;
        px[i] as f64 + px[i + 1] as f64 + px[i + 2] as f64
    };
    let render_at = |hour: f64| {
        let mut scene = gpu.upload(&device, &b, Style::Realistic);
        scene.sun = openbim_viewport::Sun { month: 3, day: 21, hour, ..Default::default() };
        render_rgba(&device, &queue, &gpu, &scene, &c, [800, 600])
    };
    // in the right room the east wall shades the east side in the morning, and the middle wall shades the west side
    // in the afternoon
    let (east, west) = ([9.0, 5.5, 0.25], [6.0, 5.5, 0.25]);
    let am = render_at(9.0);
    let pm = render_at(15.0);
    assert!(lum(&am, east) < lum(&am, west) * 0.85, "morning: east side shaded {} vs {}", lum(&am, east), lum(&am, west));
    assert!(lum(&pm, west) < lum(&pm, east) * 0.85, "afternoon: west side shaded {} vs {}", lum(&pm, west), lum(&pm, east));
    // at night everything is dim and nothing is brighter than the dim ambient
    let night = render_at(1.0);
    let bright = |px: &[u8]| px.chunks(4).filter(|p| p[..3] != [255, 255, 255]).map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32).max().unwrap_or(0);
    assert!(bright(&night) < bright(&am), "night {} against morning {}", bright(&night), bright(&am));
}
