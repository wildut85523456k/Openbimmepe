use openbim_automation::{protocol::*,Graph};
use std::sync::atomic::AtomicBool;
fn request() -> PreviewRequest {
    PreviewRequest { version:1,request_id:"test-run".into(),snapshot:Snapshot {
        document_id:"test-document".into(),revision:42,elements:vec![SnapshotElement {
            id:1,category:"Duct".into(),name:"Supply".into(),width_m:0.2,height_m:0.3,elevation_m:2.7,slope_ratio:Some(0.01),geometry:None,section:None,usage:None,material:None,outline:vec![],slab_kind:None,
        }],
    },graph:Graph::example(vec![1]) }
}
#[test]
fn report_exports_all_named_port_values() {
    let mut r=request();r.graph.nodes[1].kind=openbim_automation::NodeKind::Dimensions;
    let report=r.evaluate(None,&AtomicBool::new(false)).unwrap();
    let json=serde_json::to_string(&report).unwrap();
    let restored:PreviewResponse=serde_json::from_str(&json).unwrap();
    let outputs:Vec<_>=restored.outputs.iter().filter(|o|o.node==2).collect();
    assert_eq!(outputs.len(),3);
    assert_eq!(outputs[1].port,1);
    assert_eq!(outputs[1].value,openbim_automation::Value::Quantities(vec![0.3]));
}
#[test]
fn snapshot_report_preserves_source_identity_and_revision() {
    let request=request();
    let response=request.evaluate(None,&AtomicBool::new(false)).unwrap();
    assert_eq!(response.source_revision,42);
    assert_eq!(response.document_id,"test-document");
    assert_eq!(response.request_id,"test-run");
    assert_eq!(response.changes.len(),1);
    assert_eq!(response.changes[0].before,2.7);
    assert_eq!(response.changes[0].after,3.);
    assert_eq!(request.snapshot.elements[0].elevation_m,2.7);
}
#[test]
fn bad_snapshot_and_protocol_are_rejected() {
    let mut r=request();r.version=2;
    assert!(r.evaluate(None,&AtomicBool::new(false)).is_err());
    let mut r=request();r.snapshot.elements.push(r.snapshot.elements[0].clone());
    assert!(r.snapshot.materialize().is_err());
    let mut r=request();r.snapshot.elements[0].slope_ratio=None;
    assert!(r.snapshot.materialize().is_err());
}
#[test]
fn request_json_roundtrip() {
    let r=request();
    let parsed:PreviewRequest=serde_json::from_str(&r.to_json().unwrap()).unwrap();
    assert_eq!(parsed.snapshot.revision,42);
    assert_eq!(parsed.graph,r.graph);
}

#[test]
fn spatial_snapshot_preserves_meshes_levels_and_host_constraints() {
    use openbim_document::{Document,ElementId};
    use openbim_automation::{mcp_model::SnapshotService,Property};
    let document=Document::spatial_sample();
    let snapshot=Snapshot::capture("spatial-test",&document);
    let decoded:Snapshot=serde_json::from_str(&snapshot.to_json().unwrap()).unwrap();
    let restored=decoded.materialize().unwrap();
    assert_eq!(restored.elements().count(),document.elements().count());
    for id in [1,2,20,30] {
        assert_eq!(restored.mesh(ElementId(id)),document.mesh(ElementId(id)));
    }
    let service=SnapshotService::new(decoded).unwrap();
    assert_eq!(service.query("spatial-test",Some("Level"),None,0,10).unwrap().total,2);
    assert_eq!(service.query("spatial-test",Some("Grid"),None,0,10).unwrap().total,6);
    assert!(service.build_property_graph("spatial-test",vec![2],Property::Width,0.2).is_err(),"Pipe geometry must remain circular");
    assert!(service.build_property_graph("spatial-test",vec![100],Property::Elevation,1.0).is_err(),"Grid elevation is unsupported");
}

#[test]
fn snapshots_carry_sections_usages_materials_and_columns_through_json() {
    use openbim_automation::protocol::Snapshot;
    use openbim_document::{BeamSpec, ColumnSpec, Document, ElementId, Material, Section, SlabKind, SlabSpec, Usage};
    let mut d = Document::spatial_sample();
    d.place_beam(ElementId(10), &BeamSpec { section: Section::IShape { web_m: 0.01, flange_m: 0.02 }, usage: Usage::Joist, material: Material::Steel, ..BeamSpec::level([0.0, 1.0], [4.0, 1.0], 0.2, 0.4, 2.8) }).unwrap();
    d.place_beam(ElementId(10), &BeamSpec { usage: Usage::Brace, slope_ratio: 0.75, section: Section::Circle, ..BeamSpec::level([5.0, 1.0], [5.0, 5.0], 0.1, 0.1, 0.2) }).unwrap();
    d.place_column(ElementId(10), ElementId(11), [2.0, 2.0], &ColumnSpec { width_m: 0.4, depth_m: 0.4, base_offset_m: 0.1, section: Section::Circle, material: Material::Concrete }).unwrap();
    d.place_slab(ElementId(10), &SlabSpec { outline: vec![[0.0, 0.0], [6.0, 0.0], [6.0, 3.0], [3.0, 3.0], [3.0, 6.0], [0.0, 6.0]], thickness_m: 0.25, top_offset_m: -0.05, kind: SlabKind::Foundation, material: Material::Concrete }).unwrap();
    d.place_footing(ElementId(10), [2.0, 2.0], &openbim_document::FootingSpec { width_m: 1.5, length_m: 2.0, thickness_m: 0.5, top_offset_m: -0.1, material: Material::Concrete }).unwrap();
    let wall = d.elements().find(|e| e.category == openbim_document::Category::Wall).unwrap().id;
    d.place_wall_footing(wall, &openbim_document::WallFootingSpec { width_m: 0.6, depth_m: 0.4, top_offset_m: -0.05, material: Material::Concrete }).unwrap();
    let json = Snapshot::capture("doc", &d).to_json().unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    let rebuilt = back.materialize().unwrap();
    for el in d.elements() {
        let r = rebuilt.get(el.id).unwrap_or_else(|| panic!("{:?} lost", el.id));
        assert_eq!((r.category, r.width_m, r.height_m, r.elevation_m, r.slope_ratio, r.geometry, r.section, r.usage, r.material, &r.outline, r.slab_kind),
                   (el.category, el.width_m, el.height_m, el.elevation_m, el.slope_ratio, el.geometry, el.section, el.usage, el.material, &el.outline, el.slab_kind), "{}", el.name);
    }
    assert_eq!(d.elements().count(), rebuilt.elements().count());
    assert!(json.contains("IShape") && json.contains("\"Brace\"") && json.contains("Point") && json.contains("Area") && json.contains("Foundation") && json.contains("Pad") && json.contains("OnWall"));
}
