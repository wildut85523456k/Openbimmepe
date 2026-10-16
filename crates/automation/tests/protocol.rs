use openbim_automation::{protocol::*,Graph};
use std::sync::atomic::AtomicBool;
fn request() -> PreviewRequest {
    PreviewRequest { version:1,request_id:"test-run".into(),snapshot:Snapshot {
        document_id:"test-document".into(),revision:42,elements:vec![SnapshotElement {
            id:1,category:"Duct".into(),name:"Supply".into(),width_m:0.2,height_m:0.3,elevation_m:2.7,slope_ratio:Some(0.01),geometry:None,
        }],
    },graph:Graph::example(vec![1]) }
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
