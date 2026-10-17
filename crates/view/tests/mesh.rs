use openbim_arch::{Level, Levels};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{ColumnItem, Scene, SlabItem, SlabRole, WallItem};
use openbim_place::floors::Slab;
use openbim_view::*;

fn scene() -> Scene {
    let mut wall = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0);
    wall.openings = vec![vec![Opening { offset: 1.0, width: 1.0, sill: 0.9, height: 1.2 }]];
    let slab = |outline: Vec<[f64; 2]>| Slab { name: "S".into(), top: 0.0, thickness: 0.2, outline, fire_rating_min: None };
    Scene {
        project_name: "P".into(), project_key: 1, timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![WallItem { id: 1, name: "W".into(), chain: wall, wall_type: None, room_bounding: true }],
        wall_penetrations: vec![], firestops: Default::default(),
        // outline given clockwise on purpose
        slabs: vec![SlabItem { id: 2, slab: slab(vec![[0.0, 0.0], [0.0, 5.0], [6.0, 5.0], [6.0, 0.0]]), role: SlabRole::default(), room_bounding: true }],
        columns: vec![ColumnItem { id: 3, name: "C".into(), footprint: vec![[1.0, 1.0], [1.4, 1.0], [1.4, 1.4], [1.0, 1.4]], base_z: 0.0, height: 3.0, room_bounding: false }],
        shafts: vec![], floor_penetrations: vec![], networks: vec![],
        families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(),
    }
}

fn find(m: &[ElementMesh], id: u64) -> &ElementMesh { m.iter().find(|e| e.element.id == id).unwrap() }

#[test]
fn every_element_gets_a_closed_outward_mesh_with_the_right_volume() {
    let m = scene_meshes(&scene()).unwrap();
    assert_eq!(m.iter().map(|e| e.element.id).collect::<Vec<_>>(), vec![1, 2, 3]);
    let wall = find(&m, 1).mesh.volume();
    assert!((wall - (4.0 * 3.0 - 1.0 * 1.2) * 0.2).abs() < 1e-9, "{wall}");
    let slab = find(&m, 2).mesh.volume();
    assert!((slab - 6.0 * 5.0 * 0.2).abs() < 1e-9, "{slab}"); // positive although the outline runs clockwise
    let col = find(&m, 3).mesh.volume();
    assert!((col - 0.4 * 0.4 * 3.0).abs() < 1e-9, "{col}");
}

#[test]
fn a_floor_opening_is_a_hole_in_the_slab_mesh() {
    let mut s = scene();
    s.shafts = vec![openbim_place::floors::ShaftVoid { name: "Sh".into(), z_bottom: -1.0, z_top: 1.0, outline: vec![[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]] }];
    let m = scene_meshes(&s).unwrap();
    let v = find(&m, 2).mesh.volume();
    assert!((v - (30.0 - 1.0) * 0.2).abs() < 1e-9, "{v}");
}

#[test]
fn the_meshes_do_not_depend_on_the_number_of_threads() {
    let s = scene();
    let key = |m: &[ElementMesh]| m.iter().map(|e| (e.element.id, format!("{:?}{:?}", e.mesh.verts, e.mesh.tris))).collect::<Vec<_>>();
    let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| scene_meshes(&s).unwrap());
    let many = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(|| scene_meshes(&s).unwrap());
    assert_eq!(key(&one), key(&many));
}
