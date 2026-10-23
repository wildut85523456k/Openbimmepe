use openbim_arch::{Level, Levels};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{ColumnItem, Scene, SlabItem, SlabRole, WallItem};
use openbim_place::floors::Slab;
use openbim_view::*;

fn scene() -> Scene {
    let mut wall = WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0);
    wall.openings = vec![vec![Opening {
        offset: 1.0,
        width: 1.0,
        sill: 0.9,
        height: 1.2,
    }]];
    let slab = |outline: Vec<[f64; 2]>| Slab {
        name: "S".into(),
        top: 0.0,
        thickness: 0.2,
        outline,
        fire_rating_min: None,
    };
    Scene {
        project_name: "P".into(),
        project_key: 1,
        beams: vec![], materials: Default::default(),
        timestamp: "t".into(),
        levels: Levels::new(vec![Level {
            name: "L0".into(),
            elevation: 0.0,
        }])
        .unwrap(),
        walls: vec![WallItem {
            id: 1,
            name: "W".into(),
            chain: wall,
            wall_type: None,
            room_bounding: true,
        }],
        wall_penetrations: vec![],
        firestops: Default::default(),
        // outline given clockwise on purpose
        slabs: vec![SlabItem {
            id: 2,
            slab: slab(vec![[0.0, 0.0], [0.0, 5.0], [6.0, 5.0], [6.0, 0.0]]),
            role: SlabRole::default(),
            room_bounding: true,
        }],
        columns: vec![ColumnItem {
            id: 3,
            name: "C".into(),
            footprint: vec![[1.0, 1.0], [1.4, 1.0], [1.4, 1.4], [1.0, 1.4]],
            base_z: 0.0,
            height: 3.0,
            room_bounding: false,
        }],
        shafts: vec![],
        floor_penetrations: vec![], slab_voids: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        wall_types: vec![],
        wall_joins: vec![],
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        room_lighting: Default::default(),
        pset_map: Default::default(), grids: vec![], types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
    }
}

fn find(m: &[ElementMesh], id: u64) -> &ElementMesh {
    m.iter().find(|e| e.element.id == id).unwrap()
}

#[test]
fn every_element_gets_a_closed_outward_mesh_with_the_right_volume() {
    let m = scene_meshes(&scene()).unwrap();
    assert_eq!(
        m.iter().map(|e| e.element.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let wall = find(&m, 1).mesh.volume();
    assert!(
        (wall - (4.0 * 3.0 - 1.0 * 1.2) * 0.2).abs() < 1e-9,
        "{wall}"
    );
    let slab = find(&m, 2).mesh.volume();
    assert!((slab - 6.0 * 5.0 * 0.2).abs() < 1e-9, "{slab}"); // positive although the outline runs clockwise
    let col = find(&m, 3).mesh.volume();
    assert!((col - 0.4 * 0.4 * 3.0).abs() < 1e-9, "{col}");
}

#[test]
fn a_floor_opening_is_a_hole_in_the_slab_mesh() {
    let mut s = scene();
    s.shafts = vec![openbim_place::floors::ShaftVoid {
        name: "Sh".into(),
        z_bottom: -1.0,
        z_top: 1.0,
        outline: vec![[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]],
    }];
    let m = scene_meshes(&s).unwrap();
    let v = find(&m, 2).mesh.volume();
    assert!((v - (30.0 - 1.0) * 0.2).abs() < 1e-9, "{v}");
}

#[test]
fn the_meshes_do_not_depend_on_the_number_of_threads() {
    let s = scene();
    let key = |m: &[ElementMesh]| {
        m.iter()
            .map(|e| (e.element.id, format!("{:?}{:?}", e.mesh.verts, e.mesh.tris)))
            .collect::<Vec<_>>()
    };
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| scene_meshes(&s).unwrap());
    let many = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| scene_meshes(&s).unwrap());
    assert_eq!(key(&one), key(&many));
}

fn normal_of(m: &openbim_geom::Mesh, t: [u32; 3]) -> [f64; 3] {
    let (a, b, c) = (
        m.verts[t[0] as usize],
        m.verts[t[1] as usize],
        m.verts[t[2] as usize],
    );
    let (u, v) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

#[test]
fn sloped_tops_and_arched_openings_are_in_the_wall_mesh() {
    let mut s = scene();
    let w = &mut s.walls[0].chain;
    w.openings = vec![vec![]];
    w.tops = vec![Some(vec![[0.0, 3.0], [2.0, 3.5], [4.0, 3.0]])];
    w.poly_openings = vec![vec![vec![
        [1.0, 0.0],
        [2.0, 0.0],
        [2.0, 2.0],
        [1.5, 2.4],
        [1.0, 2.0],
    ]]];
    let m = scene_meshes(&s).unwrap();
    let v = find(&m, 1).mesh.volume();
    assert!((v - 0.2 * (13.0 - 2.2)).abs() < 1e-9, "{v}");
    // the roof of the wall has triangles that lean: normals with both x and z
    let mesh = &find(&m, 1).mesh;
    assert!(mesh.tris.iter().any(|&t| {
        let n = normal_of(mesh, t);
        n[0].abs() > 0.05 && n[2] > 0.05
    }));
}

#[test]
fn parts_cover_every_triangle_and_walls_have_exterior_and_interior_materials() {
    use openbim_ifc::walls::{Layer, WallType};
    let mut s = scene();
    s.wall_types = vec![WallType {
        name: "T".into(),
        layers: vec![Layer::new("Brick", 0.1), Layer::new("Plaster", 0.1)],
        core: None,
    }];
    s.walls[0].wall_type = Some(0);
    let m = scene_meshes(&s).unwrap();
    for e in &m {
        let mut next = 0;
        for p in &e.parts {
            assert_eq!(p.tris.start, next);
            next = p.tris.end;
        }
        assert_eq!(next, e.mesh.tris.len(), "{:?}", e.element);
    }
    let w = find(&m, 1);
    for p in &w.parts {
        for &t in &w.mesh.tris[p.tris.clone()] {
            let n = normal_of(&w.mesh, t);
            match p.face {
                FaceKind::Exterior => {
                    assert!(n[1] > 0.0, "left of +x is +y");
                    assert_eq!(p.material.as_deref(), Some("Brick"));
                }
                FaceKind::Interior => {
                    assert!(n[1] < 0.0);
                    assert_eq!(p.material.as_deref(), Some("Plaster"));
                }
                _ => {}
            }
        }
    }
    assert!(
        w.parts.iter().any(|p| p.face == FaceKind::Exterior)
            && w.parts.iter().any(|p| p.face == FaceKind::Interior)
    );
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
    s.wall_types = vec![WallType {
        name: "T".into(),
        layers: vec![Layer::new("Brick", 0.1), Layer::new("Plaster", 0.1)],
        core: None,
    }];
    s.walls[0].wall_type = Some(0);
    let mut cache = MeshCache::new();
    let u = cache.update(&s).unwrap();
    let b = Batch::new(&u.meshes);
    let total: usize = u.meshes.iter().map(|m| m.mesh.tris.len()).sum();
    assert_eq!(
        (
            b.tris.len(),
            b.tri_element.len(),
            b.tri_face.len(),
            b.tri_material.len()
        ),
        (total, total, total, total)
    );
    assert_eq!(
        b.elements.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        b.materials,
        vec!["Brick".to_string(), "Plaster".to_string()]
    );
    // the first triangle belongs to element 0, the last to the column
    assert_eq!((b.tri_element[0], *b.tri_element.last().unwrap()), (0, 2));
    assert_eq!(b.origin, [0.0, -0.1, -0.2]);
    assert!(b
        .tris
        .iter()
        .flatten()
        .all(|&i| (i as usize) < b.verts.len()));
}

#[test]
fn only_what_an_edit_touches_is_meshed_again() {
    let mut s = scene();
    let mut cache = MeshCache::new();
    let first = cache.update(&s).unwrap();
    assert_eq!(first.rebuilt.len(), 3);
    let again = cache.update(&s).unwrap();
    assert!(again.rebuilt.is_empty() && again.removed.is_empty());
    for (a, b) in first.meshes.iter().zip(&again.meshes) {
        assert!(std::sync::Arc::ptr_eq(a, b));
    }
    // edit the wall: only the wall is meshed again, the others are the same objects
    s.walls[0].chain.height = 3.5;
    let third = cache.update(&s).unwrap();
    assert_eq!(
        third.rebuilt.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1]
    );
    assert!(
        std::sync::Arc::ptr_eq(&again.meshes[1], &third.meshes[1])
            && !std::sync::Arc::ptr_eq(&again.meshes[0], &third.meshes[0])
    );
    assert!((third.meshes[0].mesh.volume() - (4.0 * 3.5 - 1.2) * 0.2).abs() < 1e-9);
    // remove the column
    s.columns.clear();
    let fourth = cache.update(&s).unwrap();
    assert_eq!(
        fourth.removed.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![3]
    );
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
    let k = fams
        .iter()
        .position(|c| c.name().starts_with("Window"))
        .unwrap();
    let inst = fams[k].solve(&Request::default()).unwrap();
    let mut s = scene();
    let frame = Frame::on_wall(&s.walls[0].chain, 0, 1.5);
    let volume_of_boxes: f64 = inst.solids.iter().map(|(_, b)| b.volume()).sum();
    let has_material = inst.materials.iter().any(|m| m.is_some());
    s.families = vec![FamilyRef {
        id: 1,
        family: fams[k].clone(),
    }];
    s.items = vec![FamilyItem {
        id: 7,
        name: "W".into(),
        family: 0,
        type_name: None,
        frame,
        instance: inst,
        host: None,
    }];
    let m = scene_meshes(&s).unwrap();
    let w = find(&m, 7);
    assert!(!w.mesh.tris.is_empty());
    assert!(
        (w.mesh.volume() - volume_of_boxes).abs() < 1e-9,
        "{} vs {}",
        w.mesh.volume(),
        volume_of_boxes
    );
    assert_eq!(w.parts.iter().any(|p| p.material.is_some()), has_material);
    assert_eq!(w.parts.last().unwrap().tris.end, w.mesh.tris.len());
}

fn bounds(m: &openbim_geom::Mesh) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for v in &m.verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    (lo, hi)
}

#[test]
fn every_family_of_the_library_is_meshed_from_its_solids() {
    use openbim_family::{library, Request};
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    let fams = library::all().unwrap();
    assert!(fams.len() >= 5);
    for (k, fam) in fams.iter().enumerate() {
        let inst = fam.solve(&Request::default()).unwrap();
        let want: f64 = inst.solids.iter().map(|(_, b)| b.volume()).sum();
        let mut s = scene();
        let frame = Frame {
            origin: [2.0, 2.0, 0.5],
            x: [0.6, 0.8],
        }; // turned, so that every transform is used
        s.families = vec![FamilyRef {
            id: 1,
            family: fam.clone(),
        }];
        s.items = vec![FamilyItem {
            id: 7,
            name: fam.name().into(),
            family: 0,
            type_name: None,
            frame,
            instance: inst,
            host: None,
        }];
        let m = scene_meshes(&s).unwrap();
        let e = find(&m, 7);
        assert!(e.warnings.is_empty(), "{}: {:?}", fam.name(), e.warnings);
        assert!(
            (e.mesh.volume() - want).abs() < 1e-9 * (1.0 + want),
            "{} (family {k}): mesh {} boxes {}",
            fam.name(),
            e.mesh.volume(),
            want
        );
        assert!(e.mesh.verts.iter().flatten().all(|x| x.is_finite()));
        assert!(e
            .mesh
            .tris
            .iter()
            .flatten()
            .all(|&i| (i as usize) < e.mesh.verts.len()));
    }
}

#[test]
fn extrusions_in_every_work_plane_are_meshed_in_place() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{Axis, Family, Request};
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    let rect = |u0, v0, u1, v1| ProfileDef::rect(u0, v0, u1, v1);
    let fam = Family {
        name: "F".into(),
        forms: vec![
            FormDef::extrusion("z", Axis::Z, "0", "3", rect("0", "0", "0.4", "0.2")),
            FormDef::extrusion("x", Axis::X, "5", "6", rect("1", "0", "2", "1")),
            FormDef::extrusion("y", Axis::Y, "7", "8", rect("0", "3", "1", "4")),
            FormDef::extrusion("c", Axis::Z, "0", "2", ProfileDef::circle("10", "0", "0.1")),
        ],
        ..Default::default()
    }
    .compile()
    .unwrap();
    let inst = fam.solve(&Request::default()).unwrap();
    let forms = inst.forms.clone();
    let mut s = scene();
    s.families = vec![FamilyRef { id: 1, family: fam }];
    s.items = vec![FamilyItem {
        id: 7,
        name: "F".into(),
        family: 0,
        type_name: None,
        frame: Frame {
            origin: [0.0; 3],
            x: [1.0, 0.0],
        },
        instance: inst,
        host: None,
    }];
    let m = scene_meshes(&s).unwrap();
    let e = find(&m, 7);
    assert!(e.warnings.is_empty());
    // the first three are boxes: their volumes add up exactly; the circle is a 32-gon, a little less than the circle
    let boxes: f64 = forms[..3].iter().map(|f| f.volume()).sum();
    let circle = forms[3].volume();
    let got = e.mesh.volume();
    assert!(
        got > boxes + 0.99 * circle && got < boxes + circle,
        "{got} vs {boxes} + {circle}"
    );
    // the x-plane box sits at x 5..6, y 1..2, z 0..1
    let (lo, hi) = bounds(&e.mesh);
    assert!(
        (hi[0] - 10.1).abs() < 1e-9 && lo[0].abs() < 1e-9 && (hi[1] - 8.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    assert!(e.mesh.volume() > 0.0);
}

#[test]
fn pipes_ducts_and_fittings_are_meshed_and_a_change_to_one_remeshes_only_it() {
    use openbim_family::{Domain, System};
    use openbim_place::penetrations::{Run, RunSize};
    use openbim_route::{
        graph::{NetFitting, NetSegment, Network},
        Fitting, FittingKind,
    };
    let mut net = Network::new(1);
    net.segments.push(NetSegment {
        id: 1,
        run: Run::round(
            "p",
            Domain::Pipe,
            System::ChilledWaterSupply,
            [0.0, 0.0, 2.5],
            [4.0, 0.0, 2.5],
            0.1,
        ),
    });
    net.segments.push(NetSegment {
        id: 2,
        run: Run::rect(
            "d",
            Domain::Duct,
            System::SupplyAir,
            [0.0, 1.0, 2.5],
            [0.0, 1.0, 3.5],
            0.4,
            0.2,
        ),
    });
    net.fittings.push(NetFitting {
        id: 3,
        domain: Domain::Pipe,
        system: System::ChilledWaterSupply,
        fitting: Fitting {
            type_name: "Elbow".into(),
            part_code: String::new(),
            kind: FittingKind::Elbow { angle: 90.0 },
            at: [4.0, 0.0, 2.5],
            ports: vec![[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            port_sizes: vec![RunSize::Round { diameter: 0.1 }; 2],
            radius: 0.15,
            trim: vec![0.15, 0.15],
        },
    });
    let mut s = scene();
    s.networks = vec![net];
    let mut cache = MeshCache::new();
    let u = cache.update(&s).unwrap();
    let find_kind = |u: &Update, k: Kind, id: u64| {
        u.meshes
            .iter()
            .find(|m| m.element.kind == k && m.element.id == id)
            .unwrap()
            .clone()
    };
    let pipe = find_kind(&u, Kind::Segment, 1);
    let polygon_area = 0.5 * 32.0 * 0.05f64.powi(2) * (std::f64::consts::TAU / 32.0).sin();
    assert!((pipe.mesh.volume() - polygon_area * 4.0).abs() < 1e-9);
    let (lo, hi) = bounds(&pipe.mesh);
    assert!((lo[0]).abs() < 1e-9 && (hi[0] - 4.0).abs() < 1e-9 && lo[2] > 2.44 && hi[2] < 2.56);
    assert_eq!(
        pipe.parts[0].material.as_deref(),
        Some(System::ChilledWaterSupply.ifc_system())
    );
    let duct = find_kind(&u, Kind::Segment, 2);
    assert!(
        (duct.mesh.volume() - 0.4 * 0.2 * 1.0).abs() < 1e-9,
        "{}",
        duct.mesh.volume()
    );
    let (lo, hi) = bounds(&duct.mesh);
    assert!((hi[2] - 3.5).abs() < 1e-9 && (lo[2] - 2.5).abs() < 1e-9);
    let elbow = find_kind(&u, Kind::Fitting, 3);
    // a bend of 90 degrees at radius 0.15: the section carried a quarter of a circle (volume = area x radius x angle)
    let want = polygon_area * 0.15 * std::f64::consts::FRAC_PI_2;
    assert!(
        (elbow.mesh.volume() - want).abs() < 0.01 * want,
        "{} vs {want}",
        elbow.mesh.volume()
    );
    assert!(watertight(&elbow.mesh));
    // the bend runs from the tangent point on each leg: (3.85, 0, 2.5) to (4, 0.15, 2.5)
    let (lo, hi) = bounds(&elbow.mesh);
    assert!(
        (lo[0] - 3.85).abs() < 1e-9 && (hi[1] - 0.15).abs() < 1e-9 && lo[1] > -0.06 && hi[0] < 4.06,
        "{lo:?} {hi:?}"
    );
    // a longer pipe: only the pipe is meshed again
    s.networks[0].segments[0].run.b = [3.0, 0.0, 2.5];
    let v = cache.update(&s).unwrap();
    assert_eq!(
        v.rebuilt,
        vec![Element {
            kind: Kind::Segment,
            id: 1
        }]
    );
    assert!((find_kind(&v, Kind::Segment, 1).mesh.volume() - polygon_area * 3.0).abs() < 1e-9);
}

/// Every edge is used by two triangles, once in each direction: a closed surface with all faces turned the same way.
fn watertight(m: &openbim_geom::Mesh) -> bool {
    let mut edges = std::collections::HashMap::new();
    for t in &m.tris {
        for k in 0..3 {
            *edges.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
        }
    }
    edges
        .iter()
        .all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1))
}

fn family_with(
    forms: Vec<openbim_family::forms::FormDef>,
) -> (openbim_family::Instance, openbim_family::Compiled) {
    use openbim_family::{Family, Request};
    let fam = Family {
        name: "F".into(),
        forms,
        ..Default::default()
    }
    .compile()
    .unwrap();
    (fam.solve(&Request::default()).unwrap(), fam)
}

fn mesh_of(inst: openbim_family::Instance, fam: openbim_family::Compiled) -> ElementMesh {
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    let mut s = scene();
    s.families = vec![FamilyRef { id: 1, family: fam }];
    s.items = vec![FamilyItem {
        id: 7,
        name: "F".into(),
        family: 0,
        type_name: None,
        frame: Frame {
            origin: [0.0; 3],
            x: [1.0, 0.0],
        },
        instance: inst,
        host: None,
    }];
    find(&scene_meshes(&s).unwrap(), 7).clone()
}

#[test]
fn revolves_are_closed_solids_of_the_right_volume_whole_or_partial() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::Axis;
    let rect = ProfileDef::rect("1", "0", "2", "1");
    // a ring about the vertical axis (work plane x: u = y, v = z): 360, 90 degrees, and a hole through the profile
    for (angle, hole) in [("360", false), ("90", false), ("270", true)] {
        let mut f = FormDef::revolve("r", Axis::X, "0", true, "0", angle, rect.clone());
        if hole {
            f = f.hole(ProfileDef::rect("1.25", "0.25", "1.75", "0.75"));
        }
        let (inst, fam) = family_with(vec![f]);
        let want = inst.forms[0].volume();
        let e = mesh_of(inst, fam);
        assert!(e.warnings.is_empty());
        assert!(watertight(&e.mesh), "{angle}: not closed");
        let got = e.mesh.volume();
        assert!(
            got > 0.0 && (got - want).abs() < 0.01 * want,
            "{angle}: mesh {got} form {want}"
        );
    }
}

#[test]
fn sweeps_follow_the_path_with_mitred_bends_and_are_closed() {
    use openbim_family::forms::{FormDef, ProfileDef};
    let profile = ProfileDef::rect("-0.05", "0", "0.05", "0.1");
    let (inst, fam) = family_with(vec![FormDef::sweep(
        "s",
        "0.5",
        &[("0", "0"), ("4", "0"), ("4", "3")],
        profile,
    )]);
    let e = mesh_of(inst, fam);
    assert!(e.warnings.is_empty() && watertight(&e.mesh));
    // a profile centred on the path: the volume is the area times the length of the path
    assert!(
        (e.mesh.volume() - 0.01 * 7.0).abs() < 1e-9,
        "{}",
        e.mesh.volume()
    );
    let (lo, hi) = bounds(&e.mesh);
    assert!(
        (lo[2] - 0.5).abs() < 1e-9
            && (hi[2] - 0.6).abs() < 1e-9
            && (hi[0] - 4.05).abs() < 1e-9
            && (hi[1] - 3.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
}

fn transition(
    a: openbim_place::penetrations::RunSize,
    b: openbim_place::penetrations::RunSize,
    l: f64,
) -> ElementMesh {
    use openbim_family::{Domain, System};
    use openbim_route::{
        graph::{NetFitting, Network},
        Fitting, FittingKind,
    };
    let mut net = Network::new(1);
    net.fittings.push(NetFitting {
        id: 1,
        domain: Domain::Duct,
        system: System::SupplyAir,
        fitting: Fitting {
            type_name: "T".into(),
            part_code: String::new(),
            kind: FittingKind::Transition,
            at: [1.0, 2.0, 3.0],
            ports: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            port_sizes: vec![a, b],
            radius: 0.0,
            trim: vec![0.0, l],
        },
    });
    let mut s = scene();
    s.networks = vec![net];
    let u = MeshCache::new().update(&s).unwrap();
    u.meshes
        .iter()
        .find(|m| m.element.kind == Kind::Fitting)
        .unwrap()
        .as_ref()
        .clone()
}

#[test]
fn a_transition_is_a_loft_from_one_section_to_the_other() {
    use openbim_place::penetrations::RunSize::{Rect, Round};
    // round to round: a frustum of a pyramid on regular 32-gons, volume h/3 (A0 + A1 + sqrt(A0 A1)) exactly
    let area = |r: f64| 0.5 * 32.0 * r * r * (std::f64::consts::TAU / 32.0).sin();
    let (a0, a1) = (area(0.05), area(0.1));
    let e = transition(Round { diameter: 0.1 }, Round { diameter: 0.2 }, 0.4);
    assert!(watertight(&e.mesh));
    assert!(
        (e.mesh.volume() - 0.4 / 3.0 * (a0 + a1 + (a0 * a1).sqrt())).abs() < 1e-9,
        "{}",
        e.mesh.volume()
    );
    let (lo, hi) = bounds(&e.mesh);
    assert!(
        (lo[0] - 1.0).abs() < 1e-9 && (hi[0] - 1.4).abs() < 1e-9 && (hi[1] - 2.1).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    // rectangle to rectangle: the area is quadratic along the length, so Simpson's rule is exact
    let e = transition(
        Rect {
            width: 0.3,
            height: 0.2,
            roll: 0.0,
        },
        Rect {
            width: 0.2,
            height: 0.2,
            roll: 0.0,
        },
        0.5,
    );
    assert!(watertight(&e.mesh));
    let want = 0.5 / 6.0 * (0.06 + 4.0 * 0.25 * 0.2 + 0.04);
    assert!(
        (e.mesh.volume() - want).abs() < 1e-9,
        "{} vs {want}",
        e.mesh.volume()
    );
    // round to rectangular: a closed solid of positive volume between the two
    let e = transition(
        Round { diameter: 0.2 },
        Rect {
            width: 0.3,
            height: 0.2,
            roll: 0.0,
        },
        0.5,
    );
    assert!(watertight(&e.mesh) && e.mesh.volume() > 0.5 * 0.03 && e.mesh.volume() < 0.5 * 0.06);
}

#[test]
fn insulation_is_a_layer_round_runs_and_fittings_of_its_system_and_is_its_own_element() {
    use openbim_family::{Domain, System};
    use openbim_place::penetrations::{Run, RunSize};
    use openbim_route::{
        graph::{NetFitting, NetSegment, Network},
        Fitting, FittingKind,
    };
    let mut net = Network::new(1);
    net.segments.push(NetSegment {
        id: 1,
        run: Run::round(
            "p",
            Domain::Pipe,
            System::ChilledWaterSupply,
            [0.0, 0.0, 2.5],
            [4.0, 0.0, 2.5],
            0.1,
        ),
    });
    net.segments.push(NetSegment {
        id: 2,
        run: Run::round(
            "q",
            Domain::Pipe,
            System::ChilledWaterReturn,
            [0.0, 1.0, 2.5],
            [4.0, 1.0, 2.5],
            0.1,
        ),
    });
    net.fittings.push(NetFitting {
        id: 3,
        domain: Domain::Pipe,
        system: System::ChilledWaterSupply,
        fitting: Fitting {
            type_name: "Elbow".into(),
            part_code: String::new(),
            kind: FittingKind::Elbow { angle: 90.0 },
            at: [4.0, 0.0, 2.5],
            ports: vec![[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            port_sizes: vec![RunSize::Round { diameter: 0.1 }; 2],
            radius: 0.15,
            trim: vec![0.15, 0.15],
        },
    });
    assert!(net
        .insulate(System::ChilledWaterSupply, 0.0, "Foam")
        .is_err());
    assert!(net.insulate(System::ChilledWaterSupply, 0.03, " ").is_err());
    net.insulate(System::ChilledWaterSupply, 0.025, "Foam")
        .unwrap();
    let mut s = scene();
    s.networks = vec![net];
    let mut cache = MeshCache::new();
    let u = cache.update(&s).unwrap();
    let get = |u: &Update, k: Kind, id: u64| {
        u.meshes
            .iter()
            .find(|m| m.element.kind == k && m.element.id == id)
            .map(|m| m.as_ref().clone())
    };
    // the supply pipe and its elbow are insulated, the return pipe is not
    assert!(
        get(&u, Kind::Insulation, 1).is_some()
            && get(&u, Kind::Insulation, 3).is_some()
            && get(&u, Kind::Insulation, 2).is_none()
    );
    let area = |r: f64| 0.5 * 32.0 * r * r * (std::f64::consts::TAU / 32.0).sin();
    let ins = get(&u, Kind::Insulation, 1).unwrap();
    assert!((ins.mesh.volume() - area(0.075) * 4.0).abs() < 1e-9);
    assert_eq!(ins.parts[0].material.as_deref(), Some("Foam"));
    let (lo, hi) = bounds(&ins.mesh);
    assert!(
        lo[1] < -0.0749 && hi[1] > 0.0749 && hi[2] > 2.5749,
        "{lo:?} {hi:?}"
    );
    assert!(watertight(&get(&u, Kind::Insulation, 3).unwrap().mesh));
    // a thicker layer re-meshes the insulation and nothing else
    s.networks[0]
        .insulate(System::ChilledWaterSupply, 0.04, "Foam")
        .unwrap();
    let v = cache.update(&s).unwrap();
    assert_eq!(v.rebuilt.len(), 2);
    assert!(v.rebuilt.iter().all(|e| e.kind == Kind::Insulation));
    // a single piece can be insulated on its own, and wins over its system's insulation
    assert!(s.networks[0].insulate_piece(2, 0.0, "Wool").is_err() && s.networks[0].insulate_piece(2, 0.02, "").is_err());
    s.networks[0].insulate_piece(2, 0.02, "Wool").unwrap();
    s.networks[0].insulate_piece(1, 0.01, "Tape").unwrap();
    s.networks[0].insulate_piece(1, 0.01, "Tape").unwrap();
    assert_eq!(s.networks[0].piece_insulation.len(), 2, "one entry per piece");
    let w = cache.update(&s).unwrap();
    let ret = get(&w, Kind::Insulation, 2).expect("the return pipe is insulated on its own");
    assert!((ret.mesh.volume() - area(0.07) * 4.0).abs() < 1e-9);
    assert_eq!(ret.parts[0].material.as_deref(), Some("Wool"));
    let sup = get(&w, Kind::Insulation, 1).unwrap();
    assert!((sup.mesh.volume() - area(0.06) * 4.0).abs() < 1e-9, "its own 10 mm, not the system's 40 mm");
    assert!(get(&w, Kind::Insulation, 3).is_some(), "the elbow keeps its system's insulation");
}

#[test]
fn blends_and_voids_reach_the_3d_mesh_in_the_model() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{Axis, Family, Request, ValueDef};
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    let r = |a, b, c, d| ProfileDef::rect(a, b, c, d);
    let fam = Family {
        name: "F".into(),
        values: vec![
            ValueDef::plane("x0", Axis::X, "0"),
            ValueDef::plane("x1", Axis::X, "1"),
            ValueDef::plane("y0", Axis::Y, "0"),
            ValueDef::plane("y1", Axis::Y, "1"),
            ValueDef::plane("z0", Axis::Z, "0"),
            ValueDef::plane("z1", Axis::Z, "1"),
        ],
        solids: vec![openbim_family::SolidDef::new(
            "box",
            ("x0", "x1"),
            ("y0", "y1"),
            ("z0", "z1"),
        )],
        forms: vec![
            FormDef::extrusion(
                "hole",
                Axis::Z,
                "-1",
                "2",
                r("0.25", "0.25", "0.75", "0.75"),
            )
            .void(),
            FormDef::blend(
                "b",
                Axis::Z,
                "2",
                "3",
                r("0", "0", "1", "1"),
                r("0.25", "0.25", "0.75", "0.75"),
            ),
        ],
        ..Default::default()
    }
    .compile()
    .unwrap();
    let inst = fam.solve(&Request::default()).unwrap();
    let mut s = scene();
    s.families = vec![FamilyRef { id: 1, family: fam }];
    s.items = vec![FamilyItem {
        id: 7,
        name: "F".into(),
        family: 0,
        type_name: None,
        frame: Frame {
            origin: [10.0, 20.0, 1.0],
            x: [0.0, 1.0],
        },
        instance: inst,
        host: None,
    }];
    let e = find(&scene_meshes(&s).unwrap(), 7).clone();
    assert!(e.warnings.is_empty());
    // the box with its hole (0.75) and the blend above it (a frustum, 1/3 (1 + 0.5 + 0.25)): the blend is not cut
    let want = 0.75 + (1.0 + 0.5 + 0.25) / 3.0;
    assert!(
        (e.mesh.volume() - want).abs() < 1e-9,
        "{} vs {want}",
        e.mesh.volume()
    );
    // turned a quarter and moved: the local x axis points along +y in the model
    let (lo, hi) = bounds(&e.mesh);
    assert!(
        (lo[0] - 9.0).abs() < 1e-9
            && (hi[0] - 10.0).abs() < 1e-9
            && (lo[1] - 20.0).abs() < 1e-9
            && (hi[1] - 21.0).abs() < 1e-9
            && (lo[2] - 1.0).abs() < 1e-9
            && (hi[2] - 4.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn a_void_of_a_hosted_family_cuts_the_wall_it_sits_in() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{Axis, Family, Request, ValueDef};
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    // a family that is only a void: a box 0.5 wide, 0.5 high and 0.1 deep (half the thickness of the wall)
    let fam = Family {
        name: "Niche".into(),
        values: vec![
            ValueDef::plane("x0", Axis::X, "0"),
            ValueDef::plane("x1", Axis::X, "0.5"),
            ValueDef::plane("y0", Axis::Y, "-0.1"),
            ValueDef::plane("y1", Axis::Y, "0"),
            ValueDef::plane("z0", Axis::Z, "0"),
            ValueDef::plane("z1", Axis::Z, "0.5"),
        ],
        forms: vec![FormDef::extrusion(
            "niche",
            Axis::Z,
            "0",
            "0.5",
            ProfileDef::rect("0", "-0.1", "0.5", "0"),
        )
        .void()],
        ..Default::default()
    }
    .compile()
    .unwrap();
    let inst = fam.solve(&Request::default()).unwrap();
    let mut s = scene();
    s.walls[0].chain.openings = vec![vec![]];
    let plain = find(&scene_meshes(&s).unwrap(), 1).mesh.volume();
    assert!((plain - 4.0 * 3.0 * 0.2).abs() < 1e-9);
    let item = |host| FamilyItem {
        id: 7,
        name: "N".into(),
        family: 0,
        type_name: None,
        frame: Frame {
            origin: [2.0, 0.0, 1.0],
            x: [1.0, 0.0],
        },
        instance: inst.clone(),
        host,
    };
    s.families = vec![FamilyRef {
        id: 1,
        family: fam.clone(),
    }];
    // not hosted: the void cuts nothing
    s.items = vec![item(None)];
    assert!((find(&scene_meshes(&s).unwrap(), 1).mesh.volume() - plain).abs() < 1e-9);
    // hosted in the wall: the niche, 0.5 x 0.1 x 0.5, comes out of it
    s.items = vec![item(Some((0, 0, 2.0)))];
    let cut = scene_meshes(&s).unwrap();
    let w = find(&cut, 1);
    assert!(w.warnings.is_empty(), "{:?}", w.warnings);
    assert!(
        (w.mesh.volume() - (plain - 0.5 * 0.1 * 0.5)).abs() < 1e-9,
        "{} vs {}",
        w.mesh.volume(),
        plain - 0.025
    );
    assert!(
        w.parts.iter().map(|p| p.tris.len()).sum::<usize>() == w.mesh.tris.len(),
        "parts cover the triangles"
    );
    // the cache re-meshes the wall (and the family) when the void moves, and nothing else
    let mut cache = MeshCache::new();
    cache.update(&s).unwrap();
    s.items[0].frame.origin[0] = 2.5;
    let u = cache.update(&s).unwrap();
    assert!(
        u.rebuilt.iter().any(|e| e.kind == Kind::Wall)
            && !u
                .rebuilt
                .iter()
                .any(|e| e.kind == Kind::Slab || e.kind == Kind::Column),
        "{:?}",
        u.rebuilt
    );
}

#[test]
fn a_free_standing_void_cuts_the_slab_it_passes_through() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{Axis, Family, Request, ValueDef};
    use openbim_ifc::{FamilyItem, FamilyRef, Frame};
    // a family that is only a void: a square shaft 0.5 x 0.5 and 0.5 high
    let fam = Family {
        name: "Shaft".into(),
        values: vec![
            ValueDef::plane("z0", Axis::Z, "0"),
            ValueDef::plane("z1", Axis::Z, "0.5"),
        ],
        forms: vec![FormDef::extrusion(
            "shaft",
            Axis::Z,
            "0",
            "0.5",
            ProfileDef::rect("0", "0", "0.5", "0.5"),
        )
        .void()],
        ..Default::default()
    }
    .compile()
    .unwrap();
    let inst = fam.solve(&Request::default()).unwrap();
    let mut s = scene();
    let plain = find(&scene_meshes(&s).unwrap(), 2).mesh.volume();
    let item = |z| FamilyItem {
        id: 7,
        name: "Sh".into(),
        family: 0,
        type_name: None,
        frame: Frame {
            origin: [2.0, 2.0, z],
            x: [1.0, 0.0],
        },
        instance: inst.clone(),
        host: None,
    };
    s.families = vec![FamilyRef {
        id: 1,
        family: fam.clone(),
    }];
    // through the slab (-0.3 to 0.2): 0.5 x 0.5 x 0.2 comes out
    s.items = vec![item(-0.3)];
    let m = scene_meshes(&s).unwrap();
    assert!(
        find(&m, 2).warnings.is_empty(),
        "{:?}",
        find(&m, 2).warnings
    );
    assert!(
        (find(&m, 2).mesh.volume() - (plain - 0.05)).abs() < 1e-9,
        "{}",
        find(&m, 2).mesh.volume()
    );
    // well above it: nothing
    s.items = vec![item(1.0)];
    assert!((find(&scene_meshes(&s).unwrap(), 2).mesh.volume() - plain).abs() < 1e-9);
}

#[test]
fn a_beam_is_a_closed_box_and_a_free_void_cuts_a_hole_through_it() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{Axis, Family, Request};
    use openbim_ifc::{BeamItem, FamilyItem, FamilyRef, Frame};
    let mut s = scene();
    // along x, 0.2 wide and 0.4 deep, 4 m long
    s.beams = vec![BeamItem::rect(5, "B", [1.0, 3.0, 2.5], [5.0, 3.0, 2.5], 0.2, 0.4)];
    let m = scene_meshes(&s).unwrap();
    let beam = find(&m, 5);
    assert_eq!(beam.element.kind, Kind::Beam);
    assert!((beam.mesh.volume() - 0.2 * 0.4 * 4.0).abs() < 1e-9, "{}", beam.mesh.volume());
    // a sloped beam has the same section area times its length
    s.beams[0].b = [1.0 + 3.0, 3.0, 2.5 + 3.0];
    let l = (3.0f64 * 3.0 * 2.0).sqrt();
    assert!((find(&scene_meshes(&s).unwrap(), 5).mesh.volume() - 0.2 * 0.4 * l).abs() < 1e-9);
    s.beams[0].b = [5.0, 3.0, 2.5];
    // a sleeve that goes across the web: 0.2 along, 1 across (the beam is 0.2 of it), 0.2 high
    let fam = Family {
        name: "Sleeve".into(),
        forms: vec![FormDef::extrusion("sleeve", Axis::Z, "0", "0.2", ProfileDef::rect("0", "-0.5", "0.2", "0.5")).void()],
        ..Default::default()
    }.compile().unwrap();
    let inst = fam.solve(&Request::default()).unwrap();
    s.families = vec![FamilyRef { id: 1, family: fam }];
    let item = |y| FamilyItem { id: 7, name: "Sl".into(), family: 0, type_name: None, frame: Frame { origin: [2.0, y, 2.4], x: [1.0, 0.0] }, instance: inst.clone(), host: None };
    s.items = vec![item(3.0)];
    let m = scene_meshes(&s).unwrap();
    assert!(find(&m, 5).warnings.is_empty(), "{:?}", find(&m, 5).warnings);
    assert!((find(&m, 5).mesh.volume() - (0.32 - 0.2 * 0.2 * 0.2)).abs() < 1e-9, "{}", find(&m, 5).mesh.volume());
    // beside the beam it cuts nothing
    s.items = vec![item(6.0)];
    assert!((find(&scene_meshes(&s).unwrap(), 5).mesh.volume() - 0.32).abs() < 1e-9);
    // a vertical or empty beam is refused rather than meshed
    s.beams[0].b = [1.0, 3.0, 4.0];
    assert!(scene_meshes(&s).is_err());
}

#[test]
fn a_free_void_cuts_a_hole_through_a_column_and_a_distant_one_cuts_nothing() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{Axis, Family, Request};
    use openbim_geom::profile::Profile;
    use openbim_ifc::{ColumnItem, FamilyItem, FamilyRef, Frame};
    let mut s = scene();
    // 0.4 x 0.4 on (3, 3), from 0 to 3 m
    s.columns = vec![ColumnItem::from_profile(5, "C", &Profile::Rect { width: 0.4, depth: 0.4 }, [3.0, 3.0], 0.0, 3.0, true).unwrap()];
    assert!((find(&scene_meshes(&s).unwrap(), 5).mesh.volume() - 0.48).abs() < 1e-9);
    // a sleeve 0.2 along x, 1 across y (the column is 0.4 of it), 0.2 high
    let fam = Family {
        name: "Sleeve".into(),
        forms: vec![FormDef::extrusion("sleeve", Axis::Z, "0", "0.2", ProfileDef::rect("0", "-0.5", "0.2", "0.5")).void()],
        ..Default::default()
    }.compile().unwrap();
    let inst = fam.solve(&Request::default()).unwrap();
    s.families = vec![FamilyRef { id: 1, family: fam }];
    let item = |x, z| FamilyItem { id: 7, name: "Sl".into(), family: 0, type_name: None, frame: Frame { origin: [x, 3.0, z], x: [1.0, 0.0] }, instance: inst.clone(), host: None };
    s.items = vec![item(2.9, 1.0)];
    let m = scene_meshes(&s).unwrap();
    assert!(find(&m, 5).warnings.is_empty(), "{:?}", find(&m, 5).warnings);
    assert!((find(&m, 5).mesh.volume() - (0.48 - 0.2 * 0.4 * 0.2)).abs() < 1e-9, "{}", find(&m, 5).mesh.volume());
    // beside the column, and above it, it cuts nothing
    for it in [item(5.0, 1.0), item(2.9, 3.5)] {
        s.items = vec![it];
        assert!((find(&scene_meshes(&s).unwrap(), 5).mesh.volume() - 0.48).abs() < 1e-9);
    }
}

#[test]
fn circle_and_i_beams_have_the_volume_of_their_section() {
    use openbim_geom::profile::{polygon_area, Profile};
    use openbim_ifc::BeamItem;
    let mut s = scene();
    let beam = |profile| BeamItem { id: 5, name: "B".into(), a: [1.0, 3.0, 2.5], b: [5.0, 3.0, 2.5], profile, usage: openbim_ifc::FramingUsage::Girder, bars: vec![] };
    // an I-section 0.2 wide, 0.4 deep, web 0.01, flanges 0.02: exact area 2*0.2*0.02 + 0.36*0.01
    let i = Profile::I { width: 0.2, depth: 0.4, web: 0.01, flange: 0.02 };
    assert!((i.area() - 0.0116).abs() < 1e-12 && (polygon_area(&i.outline()) - 0.0116).abs() < 1e-12);
    s.beams = vec![beam(i)];
    let m = scene_meshes(&s).unwrap();
    assert!((find(&m, 5).mesh.volume() - 0.0116 * 4.0).abs() < 1e-9, "{}", find(&m, 5).mesh.volume());
    // a round beam is the polygon of its mesh; its exact area is a little more
    let c = Profile::Circle { diameter: 0.3 };
    s.beams = vec![beam(c)];
    let v = find(&scene_meshes(&s).unwrap(), 5).mesh.volume();
    assert!((v - polygon_area(&c.outline()) * 4.0).abs() < 1e-9);
    assert!(v < c.area() * 4.0 && v > 0.99 * c.area() * 4.0, "{v}");
    // a sloped I-beam is closed: the same area times its length
    s.beams = vec![beam(i)];
    s.beams[0].b = [4.0, 3.0, 5.5];
    let l = (3.0f64 * 3.0 * 2.0).sqrt();
    assert!((find(&scene_meshes(&s).unwrap(), 5).mesh.volume() - 0.0116 * l).abs() < 1e-9);
    // a section that does not check is refused
    s.beams[0].profile = Profile::I { width: 0.2, depth: 0.4, web: 0.3, flange: 0.02 };
    assert!(scene_meshes(&s).is_err());
}
