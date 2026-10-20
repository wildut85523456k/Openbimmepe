//! Tests for the IFC import: the STEP (ISO 10303-21) parser and the IFC model reader.
//! The key test is a round-trip: a scene exported by the writer is read back by the reader,
//! proving the product can open and parse IFC.
use openbim_arch::{Level, Levels};
use openbim_geom::WallChain;
use openbim_ifc::reader;
use openbim_ifc::step_read;
use openbim_ifc::*;

fn levels() -> Levels {
    Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap()
}
fn scene(walls: Vec<WallItem>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        timestamp: "t".into(),
        levels: levels(),
        walls,
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
        pset_map: Default::default(),
    }
}
fn wall(id: u64) -> WallItem {
    WallItem {
        id,
        name: "W".into(),
        chain: WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0),
        wall_type: None,
        room_bounding: true,
    }
}

/// A minimal but valid IFC4 file: a project, site, building and one storey, no elements.
const MINIMAL_IFC4: &str = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION((''),'2;1');
FILE_NAME('t.ifc','2026-01-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCPERSON($,$,'Test',$,$,$,$,$);
#2=IFCORGANIZATION($,'TestOrg',$,$,$);
#3=IFCPERSONANDORGANIZATION(#1,#2,$);
#4=IFCAPPLICATION(#2,'1.0','TestApp','Test');
#5=IFCOWNERHISTORY(#3,#4,$,.ADDED.,$,$,$,0);
#6=IFCCARTESIANPOINT((0.,0.,0.));
#7=IFCDIRECTION((0.,0.,1.));
#8=IFCAXIS2PLACEMENT3D(#6,$,#7);
#9=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#8,$);
#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#11=IFCUNITASSIGNMENT((#10));
#12=IFCPROJECT('0000000040000000000001',#5,'P','',$,$,$,(#9),#11);
#13=IFCSITE('0000000040000000000002',#5,'S','',$,$,#8,$,$,.ELEMENT.,$,$,$,$,$);
#14=IFCBUILDING('0000000040000000000003',#5,'B','',$,$,#8,$,$,.ELEMENT.,$,$,$);
#15=IFCBUILDINGSTOREY('0000000040000000000004',#5,'L0','',$,$,#8,$,$,.ELEMENT.,0.);
#16=IFCRELAGGREGATES('0000000040000000000010',#5,$,$,#12,(#13));
#17=IFCRELAGGREGATES('0000000040000000000011',#5,$,$,#13,(#14));
#18=IFCRELAGGREGATES('0000000040000000000012',#5,$,$,#14,(#15));
ENDSEC;
END-ISO-10303-21;
"#;

#[test]
fn the_step_parser_reads_a_minimal_ifc4_file() {
    let f = step_read::parse(MINIMAL_IFC4).unwrap();
    assert_eq!(f.schema, "IFC4");
    assert!(f.entities.len() >= 18, "entities: {}", f.entities.len());
    let proj = f.entities.values().find(|e| e.class == "IFCPROJECT").expect("a project");
    assert_eq!(proj.args.first().and_then(|v| v.as_str()), Some("0000000040000000000001"));
}

#[test]
fn the_step_parser_rejects_a_non_step_file() {
    assert!(matches!(step_read::parse("this is not an IFC file"), Err(step_read::ReadError::NotStep)));
}

#[test]
fn the_reader_reads_the_storey_from_a_minimal_file() {
    let got = reader::read(MINIMAL_IFC4).unwrap();
    assert_eq!(got.schema, "IFC4");
    assert_eq!(got.storeys.len(), 1);
    assert_eq!(got.storeys[0].name, "L0");
    assert_eq!(got.storeys[0].elevation, 0.0);
    assert!(got.members.is_empty(), "no building elements in the minimal file");
}

#[test]
fn exported_ifc_round_trips_through_the_reader() {
    let text = export(&scene(vec![wall(5)])).unwrap().text;
    let got = reader::read(&text).unwrap();
    assert_eq!(got.schema, "IFC4");
    // the level comes back as a storey
    assert_eq!(got.storeys.len(), 1);
    assert_eq!(got.storeys[0].name, "L0");
    assert_eq!(got.storeys[0].elevation, 0.0);
    // the wall comes back as an IFCWALL member, in that storey, with its GlobalId
    assert_eq!(got.members.len(), 1);
    let w = &got.members[0];
    assert_eq!(w.class, "IFCWALL");
    assert_eq!(w.name, "W");
    assert!(!w.guid.is_empty());
    assert_eq!(w.storey, Some(0));
    // the wall's extruded body: a 4 m long, 0.2 m thick footprint, extruded 3 m up
    let body = w.body.as_ref().expect("the wall has an extruded body");
    assert!(matches!(body.section, reader::Section::Polygon), "section: {:?}", body.section);
    assert!((body.dir[2] - 1.0).abs() < 1e-9, "extruded vertically: {:?}", body.dir);
    assert!((body.depth - 3.0).abs() < 1e-9, "the wall is 3 m high: {}", body.depth);
    assert_eq!(body.outline.len(), 4);
    let xs: Vec<f64> = body.outline.iter().map(|p| p[0]).collect();
    let (xmin, xmax) = xs.iter().cloned().fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    assert!((xmin - 0.0).abs() < 1e-9 && (xmax - 4.0).abs() < 1e-9, "x extent 0..4: {:?}", xs);
    // a clean round-trip leaves nothing out
    assert!(got.skipped.is_empty(), "skipped: {:?}", got.skipped);
}

#[test]
fn the_reader_keeps_the_wall_global_id_stable_across_exports() {
    let a = reader::read(&export(&scene(vec![wall(5)])).unwrap().text).unwrap();
    let b = reader::read(&export(&scene(vec![wall(5)])).unwrap().text).unwrap();
    assert_eq!(a.members[0].guid, b.members[0].guid);
    assert_eq!(a.members[0].guid, "0000000040000000000005");
}
