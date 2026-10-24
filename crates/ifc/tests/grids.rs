//! Grid lines leave as one IfcGrid whose axes are the document's lines (gap B8, grids).
use openbim_arch::{Level, Levels};
use openbim_ifc::{export, export_ifc2x3, export_ifc4x3, GridItem, Scene};

fn grid(id: u64, name: &str, start: [f64; 2], end: [f64; 2]) -> GridItem {
    GridItem { id, name: name.into(), start, end, bulge: 0.0 }
}

fn scene(grids: Vec<GridItem>) -> Scene {
    let mut s = empty();
    s.grids = grids;
    s
}

fn empty() -> Scene {
    Scene {
        project_name: "P".into(), project_key: 1, timestamp: "2026-10-23T00:00:00".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], roofs: vec![], railings: vec![],
        stairs: vec![], columns: vec![], beams: vec![], materials: Default::default(), shafts: vec![], floor_penetrations: vec![],
        slab_voids: vec![], networks: vec![], families: vec![], items: vec![], rooms: vec![], grids: vec![], system_defs: vec![], assemblies: vec![],
        room_lighting: Default::default(), wall_types: vec![], types: vec![], member_info: Default::default(), native: vec![],
        wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], proxies: vec![], pset_map: Default::default(),
    }
}

fn count(text: &str, entity: &str) -> usize {
    text.lines().filter(|l| l.contains(&format!("={entity}("))).count()
}

#[test]
fn a_model_without_grids_has_no_grid() {
    let e = export(&scene(vec![])).unwrap();
    assert_eq!(count(&e.text, "IFCGRID"), 0);
    assert_eq!(e.report.grid_axes, 0);
}

#[test]
fn grid_lines_become_one_grid_with_a_named_axis_each() {
    let g = vec![grid(1, "A", [0.0, 0.0], [0.0, 12.0]), grid(2, "B", [6.0, 0.0], [6.0, 12.0]), grid(3, "1", [-1.0, 0.0], [8.0, 0.0])];
    for (name, e) in [("4", export(&scene(g.clone()))), ("4x3", export_ifc4x3(&scene(g.clone()))), ("2x3", export_ifc2x3(&scene(g.clone())))] {
        let e = e.unwrap();
        assert_eq!(count(&e.text, "IFCGRID"), 1, "{name}");
        assert_eq!(count(&e.text, "IFCGRIDAXIS"), 3, "{name}");
        assert_eq!(e.report.grid_axes, 3, "{name}");
        for tag in ["'A'", "'B'", "'1'"] {
            assert!(e.text.contains(&format!("IFCGRIDAXIS({tag},")), "{name}: axis {tag}");
        }
    }
}

#[test]
fn vertical_lines_are_u_axes_and_horizontal_lines_are_v_axes() {
    let e = export(&scene(vec![grid(1, "A", [0.0, 0.0], [0.0, 12.0]), grid(3, "1", [-1.0, 0.0], [8.0, 0.0])])).unwrap();
    let id_of = |tag: &str| e.text.lines().find(|l| l.contains(&format!("IFCGRIDAXIS('{tag}'"))).unwrap().split('=').next().unwrap().to_string();
    let grid_line = e.text.lines().find(|l| l.contains("=IFCGRID(")).unwrap();
    assert!(grid_line.ends_with(&format!(",({}),({}),$);", id_of("A"), id_of("1"))), "U holds A, V holds 1: {grid_line}");
}

#[test]
fn one_direction_only_is_written_as_u_axes_with_no_v_list() {
    let e = export(&scene(vec![grid(1, "1", [0.0, 0.0], [8.0, 0.0]), grid(2, "2", [0.0, 5.0], [8.0, 5.0])])).unwrap();
    let line = e.text.lines().find(|l| l.contains("=IFCGRID(")).unwrap();
    assert!(line.ends_with(",$,$);") || line.ends_with(",$,$)"), "V and W lists are empty: {line}");
}

#[test]
fn the_grid_belongs_to_the_lowest_storey_and_exports_repeatably() {
    let g = vec![grid(1, "A", [0.0, 0.0], [0.0, 12.0])];
    let (a, b) = (export(&scene(g.clone())).unwrap(), export(&scene(g)).unwrap());
    assert_eq!(a.text, b.text);
    let grid_id = a.text.lines().find(|l| l.contains("=IFCGRID(")).unwrap().split('=').next().unwrap().to_string();
    assert!(a.text.lines().any(|l| l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE") && l.contains(&grid_id)), "{grid_id} is contained in a storey");
}

// ---- named systems (gap B9)
mod systems {
    use super::*;
    use openbim_family::mep::{Domain, FlowDir, System};
    use openbim_ifc::SystemDef;
    use openbim_place::penetrations::{Run, RunSize};
    use openbim_route::graph::{NetEquipment, NetSegment, Network, NodeRef, PortRef};

    fn network() -> Network {
        let mut n = Network::new(100);
        let seg = |id, x: f64| NetSegment { id, run: Run::rect("duct", Domain::Duct, System::SupplyAir, [x, 0.0, 2.5], [x + 3.0, 0.0, 2.5], 0.4, 0.3) };
        n.segments = vec![seg(1, 0.0), seg(2, 3.0), seg(3, 20.0)];
        n.equipment = vec![NetEquipment {
            element: 50, connector: "supply".into(), domain: Domain::Duct, system: System::SupplyAir, flow_dir: FlowDir::Out,
            pos: [0.0, 0.0, 2.5], dir: [1.0, 0.0, 0.0], size: RunSize::Rect { width: 0.4, height: 0.3, roll: 0.0 },
        }];
        let p = |node, port| PortRef { node, port };
        n.link(p(NodeRef::Segment(0), 1), p(NodeRef::Segment(1), 0)).unwrap();
        n.link(p(NodeRef::Equipment(0), 0), p(NodeRef::Segment(0), 0)).unwrap();
        n
    }

    fn with(defs: Vec<SystemDef>) -> Scene {
        let mut s = empty();
        s.networks = vec![network()];
        s.system_defs = defs;
        s
    }

    /// the entity numbers a group of that name holds
    fn members(text: &str, name: &str) -> Vec<String> {
        let id = text.lines().find(|l| l.contains("=IFCDISTRIBUTIONSYSTEM(") && l.contains(&format!(",'{name}',"))).map(|l| l.split('=').next().unwrap().to_string());
        let id = id.unwrap_or_else(|| panic!("no group {name}"));
        let rel = text.lines().find(|l| l.contains("=IFCRELASSIGNSTOGROUP(") && l.ends_with(&format!(",$,{id});"))).unwrap();
        let inner = rel.split("$,$,$,(").nth(1).unwrap().split(')').next().unwrap();
        inner.split(',').map(str::to_string).collect()
    }

    #[test]
    fn without_a_named_system_runs_go_into_the_group_of_their_kind() {
        let e = export(&with(vec![])).unwrap();
        assert_eq!(e.report.named_systems, 0);
        assert_eq!(members(&e.text, "Supply air 1").len(), 3);
    }

    #[test]
    fn a_named_system_takes_its_equipment_and_the_runs_connected_to_it() {
        let def = SystemDef { name: "AHU-1 Supply".into(), system: System::SupplyAir, members: vec![50] };
        let e = export(&with(vec![def])).unwrap();
        assert_eq!(e.report.named_systems, 1);
        assert_eq!(members(&e.text, "AHU-1 Supply").len(), 3, "two runs and the equipment");
        assert_eq!(members(&e.text, "Supply air 1").len(), 1, "the run nobody connected stays in the group of its kind");
    }

    #[test]
    fn a_system_whose_equipment_is_not_in_the_model_writes_nothing() {
        let def = SystemDef { name: "Ghost".into(), system: System::SupplyAir, members: vec![999] };
        let e = export(&with(vec![def])).unwrap();
        assert_eq!(e.report.named_systems, 0);
        assert!(!e.text.contains("'Ghost'"));
    }

    #[test]
    fn the_guid_of_a_named_system_does_not_change_when_other_things_are_added() {
        let def = || SystemDef { name: "AHU-1 Supply".into(), system: System::SupplyAir, members: vec![50] };
        let guid = |s: &Scene| {
            let e = export(s).unwrap();
            e.text.lines().find(|l| l.contains("IFCDISTRIBUTIONSYSTEM(") && l.contains("AHU-1")).unwrap().split('(').nth(1).unwrap().split(',').next().unwrap().to_string()
        };
        let a = with(vec![def()]);
        let mut b = with(vec![def()]);
        b.grids = vec![grid(1, "A", [0.0, 0.0], [0.0, 5.0])];
        b.levels = Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }]).unwrap();
        assert_eq!(guid(&a), guid(&b));
    }
}

// ---- trusses and beam systems (gap B19)
mod assemblies {
    use super::*;
    use openbim_geom::profile::Profile;
    use openbim_ifc::{AssemblyDef, AssemblyKind, BeamItem, FramingUsage};

    fn beam(id: u64, y: f64) -> BeamItem {
        BeamItem { id, name: format!("M{id}"), a: [0.0, y, 2.6], b: [4.0, y, 2.6], profile: Profile::Circle { diameter: 0.1 }, usage: FramingUsage::Brace, bars: vec![] }
    }
    fn truss_scene(members: Vec<u64>) -> Scene {
        let mut s = empty();
        s.beams = vec![beam(5, 0.0), beam(6, 1.0), beam(7, 2.0)];
        s.assemblies = vec![AssemblyDef { id: 4, name: "Truss 1".into(), kind: AssemblyKind::Truss, members }];
        s
    }

    #[test]
    fn a_truss_is_one_assembly_over_its_members_and_the_storey_holds_the_assembly() {
        let e = export(&truss_scene(vec![5, 6])).unwrap();
        assert_eq!(e.report.assemblies, 1);
        let asm = e.text.lines().find(|l| l.contains("=IFCELEMENTASSEMBLY(")).expect("an assembly");
        assert!(asm.contains("'Truss 1'") && asm.ends_with(".TRUSS.);"), "{asm}");
        let asm_id = asm.split('=').next().unwrap();
        let agg = e.text.lines().find(|l| l.contains("IFCRELAGGREGATES(") && l.contains(&format!("$,$,$,{asm_id},("))).expect("it aggregates its members");
        assert_eq!(agg.split_once('=').unwrap().1.matches('#').count(), 3, "the assembly and two members: {agg}");
        // the members are parts, so only the assembly and the loose beam are contained in the storey
        let contained: String = e.text.lines().filter(|l| l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE(")).map(|l| l.split_once('=').unwrap().1).collect();
        assert!(contained.contains(asm_id));
        assert_eq!(contained.matches('#').count(), 1 + 1 + 1, "storey + the assembly + the one beam that is not part of it: {contained}");
    }

    #[test]
    fn a_beam_system_is_a_beam_grid_and_a_truss_with_no_written_members_is_reported() {
        let mut s = truss_scene(vec![5, 6]);
        s.assemblies[0].kind = AssemblyKind::BeamGrid;
        assert!(export(&s).unwrap().text.contains(".BEAM_GRID.);"));
        let e = export(&truss_scene(vec![99])).unwrap();
        assert_eq!(e.report.assemblies, 0);
        assert!(e.report.warnings.iter().any(|w| w.contains("Truss 1")), "{:?}", e.report.warnings);
    }

    #[test]
    fn the_guid_of_an_assembly_comes_from_its_element_and_repeats() {
        let a = export(&truss_scene(vec![5, 6])).unwrap();
        let b = export(&truss_scene(vec![5, 6])).unwrap();
        assert_eq!(a.text, b.text);
    }
}

fn curved(id: u64, name: &str, start: [f64; 2], end: [f64; 2], bulge: f64) -> GridItem {
    GridItem { id, name: name.into(), start, end, bulge }
}

#[test]
fn a_curved_line_is_a_trimmed_circle_that_reads_back_with_its_bulge_in_every_schema() {
    let g = vec![curved(1, "A", [10.0, 0.0], [0.0, 10.0], (std::f64::consts::FRAC_PI_2 / 4.0).tan()), curved(2, "B", [0.0, 0.0], [8.0, 0.0], -0.3), grid(3, "1", [0.0, -2.0], [9.0, -2.0])];
    for (name, e) in [("4", export(&scene(g.clone()))), ("4x3", export_ifc4x3(&scene(g.clone()))), ("2x3", export_ifc2x3(&scene(g.clone())))] {
        let e = e.unwrap();
        assert_eq!((count(&e.text, "IFCTRIMMEDCURVE"), count(&e.text, "IFCCIRCLE"), count(&e.text, "IFCPOLYLINE")), (2, 2, 1), "{name}");
        let back = openbim_ifc::reader::read(&e.text).unwrap();
        let by = |n: &str| back.grids.iter().find(|x| x.name == n).unwrap_or_else(|| panic!("{name}: {n}"));
        for (n, want) in [("A", &g[0]), ("B", &g[1]), ("1", &g[2])] {
            let got = by(n);
            assert!((got.bulge - want.bulge).abs() < 1e-6, "{name} {n}: {} {}", got.bulge, want.bulge);
            for k in 0..2 {
                assert!((got.start[k] - want.start[k]).abs() < 1e-5 && (got.end[k] - want.end[k]).abs() < 1e-5, "{name} {n}");
            }
        }
    }
}

#[test]
fn an_arc_over_half_a_circle_reads_as_a_straight_line_and_says_so() {
    // three quarters of a circle of radius 5 about the origin, counter-clockwise from (5,0) to (0,-5)
    let text = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\nFILE_NAME('x','2026-10-24T00:00:00',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n#1=IFCCARTESIANPOINT((0.,0.));\n#2=IFCAXIS2PLACEMENT2D(#1,$);\n#3=IFCCIRCLE(#2,5.);\n#4=IFCCARTESIANPOINT((5.,0.));\n#5=IFCCARTESIANPOINT((0.,-5.));\n#6=IFCTRIMMEDCURVE(#3,(#4),(#5),.T.,.CARTESIAN.);\n#7=IFCGRIDAXIS('Z',#6,.T.);\n#8=IFCGRID('0000000000000000000000',$,'Grid',$,$,$,(#7),$,$,$);\nENDSEC;\nEND-ISO-10303-21;\n";
    let back = openbim_ifc::reader::read(text).unwrap();
    assert_eq!(back.grids.len(), 1);
    assert_eq!((back.grids[0].start, back.grids[0].bulge), ([5.0, 0.0], 0.0));
    assert!(back.diagnostics.iter().any(|d| d.contains("half a circle")), "{:?}", back.diagnostics);
}
