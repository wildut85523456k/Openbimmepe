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

fn normal_of(m: &openbim_geom::Mesh, t: [u32; 3]) -> [f64; 3] {
    let (a, b, c) = (m.verts[t[0] as usize], m.verts[t[1] as usize], m.verts[t[2] as usize]);
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

#[test]
fn sloped_tops_and_arched_openings_are_in_the_wall_mesh() {
    let mut s = scene();
    let w = &mut s.walls[0].chain;
    w.openings = vec![vec![]];
    w.tops = vec![Some(vec![[0.0, 3.0], [2.0, 3.5], [4.0, 3.0]])];
    w.poly_openings = vec![vec![vec![[1.0, 0.0], [2.0, 0.0], [2.0, 2.0], [1.5, 2.4], [1.0, 2.0]]]];
    let m = scene_meshes(&s).unwrap();
    let v = find(&m, 1).mesh.volume();
    assert!((v - 0.2 * (13.0 - 2.2)).abs() < 1e-9, "{v}");
    // the roof of the wall has triangles that lean: normals with both x and z
    let mesh = &find(&m, 1).mesh;
    assert!(mesh.tris.iter().any(|&t| { let n = normal_of(mesh, t); n[0].abs() > 0.05 && n[2] > 0.05 }));
}

#[test]
fn parts_cover_every_triangle_and_walls_have_exterior_and_interior_materials() {
    use openbim_ifc::walls::{Layer, WallType};
    let mut s = scene();
    s.wall_types = vec![WallType { name: "T".into(), layers: vec![Layer::new("Brick", 0.1), Layer::new("Plaster", 0.1)], core: None }];
    s.walls[0].wall_type = Some(0);
    let m = scene_meshes(&s).unwrap();
    for e in &m {
        let mut next = 0;
        for p in &e.parts { assert_eq!(p.tris.start, next); next = p.tris.end; }
        assert_eq!(next, e.mesh.tris.len(), "{:?}", e.element);
    }
    let w = find(&m, 1);
    for p in &w.parts {
        for &t in &w.mesh.tris[p.tris.clone()] {
            let n = normal_of(&w.mesh, t);
            match p.face {
                FaceKind::Exterior => { assert!(n[1] > 0.0, "left of +x is +y"); assert_eq!(p.material.as_deref(), Some("Brick")); }
                FaceKind::Interior => { assert!(n[1] < 0.0); assert_eq!(p.material.as_deref(), Some("Plaster")); }
                _ => {}
            }
        }
    }
    assert!(w.parts.iter().any(|p| p.face == FaceKind::Exterior) && w.parts.iter().any(|p| p.face == FaceKind::Interior));
    // the slab has a top, a bottom and sides and no material
    let slab = find(&m, 2);
    let kinds: Vec<FaceKind> = slab.parts.iter().map(|p| p.face).collect();
    assert_eq!(kinds, vec![FaceKind::Top, FaceKind::Bottom, FaceKind::Side]);
    assert!(slab.parts.iter().all(|p| p.material.is_none()));
}

#[test]
fn a_batch_names_the_element_and_material_of_every_triangle() {
    use openbim_ifc::walls::{Layer, WallType};
    let mut s = scene();
    s.wall_types = vec![WallType { name: "T".into(), layers: vec![Layer::new("Brick", 0.1), Layer::new("Plaster", 0.1)], core: None }];
    s.walls[0].wall_type = Some(0);
    let mut cache = MeshCache::new();
    let u = cache.update(&s).unwrap();
    let b = Batch::new(&u.meshes);
    let total: usize = u.meshes.iter().map(|m| m.mesh.tris.len()).sum();
    assert_eq!((b.tris.len(), b.tri_element.len(), b.tri_face.len(), b.tri_material.len()), (total, total, total, total));
    assert_eq!(b.elements.iter().map(|e| e.id).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(b.materials, vec!["Brick".to_string(), "Plaster".to_string()]);
    // the first triangle belongs to element 0, the last to the column
    assert_eq!((b.tri_element[0], *b.tri_element.last().unwrap()), (0, 2));
    assert_eq!(b.origin, [0.0, -0.1, -0.2]);
    assert!(b.tris.iter().flatten().all(|&i| (i as usize) < b.verts.len()));
}

#[test]
fn only_what_an_edit_touches_is_meshed_again() {
    let mut s = scene();
    let mut cache = MeshCache::new();
    let first = cache.update(&s).unwrap();
    assert_eq!(first.rebuilt.len(), 3);
    let again = cache.update(&s).unwrap();
    assert!(again.rebuilt.is_empty() && again.removed.is_empty());
    for (a, b) in first.meshes.iter().zip(&again.meshes) { assert!(std::sync::Arc::ptr_eq(a, b)); }
    // edit the wall: only the wall is meshed again, the others are the same objects
    s.walls[0].chain.height = 3.5;
    let third = cache.update(&s).unwrap();
    assert_eq!(third.rebuilt.iter().map(|e| e.id).collect::<Vec<_>>(), vec![1]);
    assert!(std::sync::Arc::ptr_eq(&again.meshes[1], &third.meshes[1]) && !std::sync::Arc::ptr_eq(&again.meshes[0], &third.meshes[0]));
    assert!((third.meshes[0].mesh.volume() - (4.0 * 3.5 - 1.2) * 0.2).abs() < 1e-9);
    // remove the column
    s.columns.clear();
    let fourth = cache.update(&s).unwrap();
    assert_eq!(fourth.removed.iter().map(|e| e.id).collect::<Vec<_>>(), vec![3]);
    assert_eq!((fourth.meshes.len(), cache.len()), (2, 2));
    // a wall that cannot be built is an error and the cache stays as it was
    s.walls[0].chain.height = -1.0;
    assert!(cache.update(&s).is_err());
    assert_eq!(cache.len(), 2);
    s.walls[0].chain.height = 3.5;
    assert!(cache.update(&s).unwrap().rebuilt.is_empty());
}

#[test]
fn a_window_is_meshed_from_its_solids_with_their_materials() {
    use openbim_family::{library, Request};
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    let fams = library::all().unwrap();
    let k = fams.iter().position(|c| c.name().starts_with("Window")).unwrap();
    let inst = fams[k].solve(&Request::default()).unwrap();
    let mut s = scene();
    let frame = Frame::on_wall(&s.walls[0].chain, 0, 1.5);
    let volume_of_boxes: f64 = inst.solids.iter().map(|(_, b)| b.volume()).sum();
    let has_material = inst.materials.iter().any(|m| m.is_some());
    s.families = vec![FamilyRef { id: 1, family: fams[k].clone() }];
    s.items = vec![FamilyItem { id: 7, name: "W".into(), family: 0, type_name: None, frame, instance: inst, host: None }];
    let m = scene_meshes(&s).unwrap();
    let w = find(&m, 7);
    assert!(!w.mesh.tris.is_empty());
    assert!((w.mesh.volume() - volume_of_boxes).abs() < 1e-9, "{} vs {}", w.mesh.volume(), volume_of_boxes);
    assert_eq!(w.parts.iter().any(|p| p.material.is_some()), has_material);
    assert_eq!(w.parts.last().unwrap().tris.end, w.mesh.tris.len());
}
