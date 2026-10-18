use openbim_arch::{Level, Levels};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{ColumnItem, Scene, SlabItem, SlabRole, WallItem};
use openbim_place::floors::{ShaftVoid, Slab};
use openbim_view::*;

fn scene(walls: Vec<WallChain>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        beams: vec![], materials: Default::default(),
        timestamp: "t".into(),
        levels: Levels::new(vec![
            Level {
                name: "L0".into(),
                elevation: 0.0,
            },
            Level {
                name: "L1".into(),
                elevation: 3.0,
            },
        ])
        .unwrap(),
        walls: walls
            .into_iter()
            .enumerate()
            .map(|(i, chain)| WallItem {
                id: 100 + i as u64,
                name: format!("W{i}"),
                chain,
                wall_type: None,
                room_bounding: true,
            })
            .collect(),
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
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        room_lighting: Default::default(),
        pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], stairs: vec![], native: vec![],
    }
}

/// a 4 m wall along x at height y, 0.2 thick and 3 m high, with the openings given
fn wall_at(y: f64, openings: Vec<Opening>) -> WallChain {
    let mut c = WallChain::new(vec![[0.0, y], [4.0, y]], false, 0.2, 3.0);
    c.openings = vec![openings];
    c
}
fn door(offset: f64) -> Opening {
    Opening {
        offset,
        width: 1.0,
        sill: 0.0,
        height: 2.1,
    }
}

fn sec(a: [f64; 2], b: [f64; 2]) -> Section {
    Section {
        a,
        b,
        bottom: -1.0,
        top: 4.0,
        depth: 2.0,
    }
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
type Ring = Vec<[f64; 2]>;

fn regions(v: &SectionView, kind: Kind) -> Vec<(Ring, Vec<Ring>)> {
    v.items
        .iter()
        .filter(|i| i.element.kind == kind)
        .filter_map(|i| {
            if let Shape::Region { outer, holes } = &i.shape {
                Some((outer.clone(), holes.clone()))
            } else {
                None
            }
        })
        .collect()
}
fn extent(r: &[[f64; 2]]) -> [f64; 4] {
    r.iter()
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        })
}
fn beyond_lines(v: &SectionView) -> usize {
    v.items.iter().filter(|i| i.class == Class::Beyond).count()
}

#[test]
fn a_section_across_a_wall_cuts_its_thickness_and_a_door_is_a_gap_only_where_the_line_crosses_it() {
    // the line goes up the page at x = 2 across the wall at y = 0: u = y + 2, so the wall is u 1.9 to 2.1
    let s = scene(vec![wall_at(0.0, vec![door(1.5)])]);
    let v = section_view(&s, sec([2.0, -2.0], [2.0, 2.0])).unwrap();
    assert!(v.warnings.is_empty() && close(v.length, 4.0));
    let r = regions(&v, Kind::Wall);
    assert_eq!(r.len(), 1);
    let e = extent(&r[0].0);
    assert!(
        close(e[0], 1.9) && close(e[2], 2.1) && close(e[1], 0.0) && close(e[3], 3.0),
        "{e:?}"
    );
    // the door (s 1.5 to 2.5) is crossed at s = 2: the gap is as wide as the wall and 2.1 m high
    assert_eq!(r[0].1.len(), 1);
    let h = extent(&r[0].1[0]);
    assert!(
        close(h[0], 1.9) && close(h[2], 2.1) && close(h[1], 0.0) && close(h[3], 2.1),
        "{h:?}"
    );
    // at x = 1 the door is not there
    let v = section_view(&s, sec([1.0, -2.0], [1.0, 2.0])).unwrap();
    assert_eq!(regions(&v, Kind::Wall)[0].1.len(), 0);
    assert!(v
        .items
        .iter()
        .filter(|i| i.element.kind == Kind::Wall)
        .all(|i| i.class == Class::Cut));
}

#[test]
fn a_section_along_a_wall_shows_the_door_where_it_is_and_the_direction_decides_left_and_right() {
    let s = scene(vec![wall_at(0.0, vec![door(0.5)])]);
    // forward: u = x, the door (x 0.5 to 1.5) is at u 0.5 to 1.5
    let v = section_view(&s, sec([0.0, 0.0], [4.0, 0.0])).unwrap();
    let r = regions(&v, Kind::Wall);
    let (e, h) = (extent(&r[0].0), extent(&r[0].1[0]));
    assert!(
        close(e[0], 0.0) && close(e[2], 4.0) && close(h[0], 0.5) && close(h[2], 1.5),
        "{e:?} {h:?}"
    );
    // reversed: u = 4 - x, the same door is at u 2.5 to 3.5
    let v = section_view(&s, sec([4.0, 0.0], [0.0, 0.0])).unwrap();
    let r = regions(&v, Kind::Wall);
    let h = extent(&r[0].1[0]);
    assert!(close(h[0], 2.5) && close(h[2], 3.5), "{h:?}");
}

#[test]
fn walls_in_front_of_the_plane_within_the_depth_are_beyond_outlines_and_those_behind_are_not_drawn()
{
    // looking along +x the viewer faces left of the line, which is +y
    let s = scene(vec![wall_at(1.5, vec![door(1.5)]), wall_at(-1.5, vec![])]);
    let v = section_view(
        &s,
        Section {
            a: [0.0, 0.0],
            b: [4.0, 0.0],
            bottom: -1.0,
            top: 4.0,
            depth: 2.0,
        },
    )
    .unwrap();
    // the wall at y = 1.5: an outline of the whole wall (4 lines) and one of the door (4 lines); nothing of y = -1.5
    assert_eq!(beyond_lines(&v), 8);
    assert!(v
        .items
        .iter()
        .all(|i| i.element.id == 100 && i.class == Class::Beyond || i.element.kind == Kind::Level));
    // nearer than the depth only: wall at y = 1.5 has its nearest face at 1.4, so a depth of 1 does not reach it
    let v = section_view(
        &s,
        Section {
            a: [0.0, 0.0],
            b: [4.0, 0.0],
            bottom: -1.0,
            top: 4.0,
            depth: 1.0,
        },
    )
    .unwrap();
    assert_eq!(beyond_lines(&v), 0);
    // turned round, the other wall is in front
    let v = section_view(
        &s,
        Section {
            a: [4.0, 0.0],
            b: [0.0, 0.0],
            bottom: -1.0,
            top: 4.0,
            depth: 2.0,
        },
    )
    .unwrap();
    assert_eq!(beyond_lines(&v), 4);
    assert!(v
        .items
        .iter()
        .filter(|i| i.class == Class::Beyond)
        .all(|i| i.element.id == 101));
}

#[test]
fn slabs_and_columns_are_cut_and_a_shaft_void_splits_the_slab() {
    let mut s = scene(vec![]);
    s.slabs = vec![SlabItem {
        id: 50,
        slab: Slab {
            name: "S".into(),
            top: 0.0,
            thickness: 0.2,
            outline: vec![[0.0, -1.0], [6.0, -1.0], [6.0, 1.0], [0.0, 1.0]],
            fire_rating_min: None,
        },
        role: SlabRole::Structure,
        room_bounding: true,
    }];
    s.columns = vec![ColumnItem {
        id: 80,
        name: "C".into(),
        footprint: vec![[3.0, -0.15], [3.3, -0.15], [3.3, 0.15], [3.0, 0.15]],
        base_z: 0.0,
        height: 3.0,
        room_bounding: false,
    }];
    // along y = 0 from x = 1 to 5: the slab is u 0 to 4, z -0.2 to 0; the column is at u 2 to 2.3
    let v = section_view(&s, sec([1.0, 0.0], [5.0, 0.0])).unwrap();
    let sl = regions(&v, Kind::Slab);
    assert_eq!(sl.len(), 1);
    let e = extent(&sl[0].0);
    assert!(
        close(e[0], 0.0) && close(e[2], 4.0) && close(e[1], -0.2) && close(e[3], 0.0),
        "{e:?}"
    );
    let c = regions(&v, Kind::Column);
    let e = extent(&c[0].0);
    assert!(
        c.len() == 1 && close(e[0], 2.0) && close(e[2], 2.3) && close(e[3], 3.0),
        "{e:?}"
    );
    // a shaft through the slab at x 2 to 3 (u 1 to 2): the slab is two pieces
    s.shafts = vec![ShaftVoid {
        name: "Sh".into(),
        outline: vec![[2.0, -0.5], [3.0, -0.5], [3.0, 0.5], [2.0, 0.5]],
        z_bottom: -3.0,
        z_top: 3.0,
    }];
    let v = section_view(&s, sec([1.0, 0.0], [5.0, 0.0])).unwrap();
    let sl = regions(&v, Kind::Slab);
    let mut ext: Vec<(f64, f64)> = sl
        .iter()
        .map(|r| {
            let e = extent(&r.0);
            (e[0], e[2])
        })
        .collect();
    ext.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(ext.len(), 2);
    assert!(
        close(ext[0].0, 0.0)
            && close(ext[0].1, 1.0)
            && close(ext[1].0, 2.0)
            && close(ext[1].1, 4.0),
        "{ext:?}"
    );
    // a line that misses the slab and the column cuts nothing
    assert!(regions(
        &section_view(&s, sec([1.0, 5.0], [5.0, 5.0])).unwrap(),
        Kind::Slab
    )
    .is_empty());
}

#[test]
fn levels_are_datum_lines_with_names_and_the_view_height_clips_everything() {
    let s = scene(vec![wall_at(0.0, vec![])]);
    let v = section_view(&s, sec([2.0, -2.0], [2.0, 2.0])).unwrap();
    let datums: Vec<&Item> = v.items.iter().filter(|i| i.class == Class::Datum).collect();
    assert_eq!(datums.len(), 2);
    assert_eq!(
        v.labels.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        vec!["L0", "L1"]
    );
    assert_eq!(datums[1].shape, Shape::Line([0.0, 3.0], [4.0, 3.0]));
    // a view from 1 to 2 m holds no level, and clips the wall to those heights
    let v = section_view(
        &s,
        Section {
            a: [2.0, -2.0],
            b: [2.0, 2.0],
            bottom: 1.0,
            top: 2.0,
            depth: 0.0,
        },
    )
    .unwrap();
    assert!(v.labels.is_empty() && v.items.iter().all(|i| i.class != Class::Datum));
    let e = extent(&regions(&v, Kind::Wall)[0].0);
    assert!(close(e[1], 1.0) && close(e[3], 2.0));
    // levels exactly at the edges of the view count
    let v = section_view(
        &s,
        Section {
            a: [2.0, -2.0],
            b: [2.0, 2.0],
            bottom: 0.0,
            top: 3.0,
            depth: 0.0,
        },
    )
    .unwrap();
    assert_eq!(v.labels.len(), 2);
}

#[test]
fn arched_openings_sloped_tops_and_bad_sections() {
    // an arched opening (a triangle in s and z): square on at its apex the gap is 2 m high; along the wall it is the polygon
    let mut c = wall_at(0.0, vec![]);
    c.poly_openings = vec![vec![vec![[1.0, 0.0], [3.0, 0.0], [2.0, 2.0]]]];
    let s = scene(vec![c.clone()]);
    let h = extent(
        &regions(
            &section_view(&s, sec([2.0, -2.0], [2.0, 2.0])).unwrap(),
            Kind::Wall,
        )[0]
        .1[0],
    );
    assert!(
        close(h[0], 1.9) && close(h[2], 2.1) && close(h[1], 0.0) && close(h[3], 2.0),
        "{h:?}"
    );
    let along = section_view(&s, sec([0.0, 0.0], [4.0, 0.0])).unwrap();
    let hole = &regions(&along, Kind::Wall)[0].1[0];
    assert_eq!(hole.len(), 3);
    assert!(hole.iter().any(|p| close(p[0], 2.0) && close(p[1], 2.0)));
    // seen beyond the plane, the arch is left out and said so
    let front = scene(vec![{
        let mut c2 = c.clone();
        c2.points = vec![[0.0, 1.5], [4.0, 1.5]];
        c2
    }]);
    let v = section_view(&front, sec([0.0, 0.0], [4.0, 0.0])).unwrap();
    assert!(
        v.warnings.iter().any(|w| w.contains("arched")),
        "{:?}",
        v.warnings
    );
    // a sloped top is drawn flat, with a warning
    let mut sl = wall_at(0.0, vec![]);
    sl.tops = vec![Some(vec![[0.0, 0.0], [4.0, 3.0]])];
    let v = section_view(&scene(vec![sl]), sec([2.0, -2.0], [2.0, 2.0])).unwrap();
    assert!(v.warnings.iter().any(|w| w.contains("sloped")));
    // bad input
    let base = sec([0.0, 0.0], [4.0, 0.0]);
    let s = scene(vec![]);
    for bad in [
        Section {
            b: [0.0, 0.0],
            ..base
        },
        Section {
            bottom: 5.0,
            ..base
        },
        Section {
            depth: -1.0,
            ..base
        },
        Section {
            a: [f64::NAN, 0.0],
            ..base
        },
    ] {
        assert!(
            matches!(section_view(&s, bad), Err(ViewError::BadSection(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn a_section_is_the_same_on_any_number_of_threads_and_draws_as_svg() {
    let walls: Vec<WallChain> = (0..100)
        .map(|k| wall_at(k as f64 * 0.5, vec![door(1.0)]))
        .collect();
    let s = scene(walls);
    let sc = sec([2.0, -1.0], [2.0, 60.0]);
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| section_view(&s, sc))
        .unwrap();
    let four = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| section_view(&s, sc))
        .unwrap();
    assert_eq!(one, four);
    let d = section_svg(&one);
    assert!(
        d.starts_with("<svg ") && d.contains(">L0</text>") && d.contains("fill-rule=\"evenodd\"")
    );
    assert_eq!(d, section_svg(&four));
}
