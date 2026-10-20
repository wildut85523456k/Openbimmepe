use openbim_automation::{mcp_model::SnapshotService,protocol::{Snapshot,SnapshotElement},Property,Graph,Node,NodeKind,Type,Wire};
fn service()->SnapshotService {
    SnapshotService::new(Snapshot { document_id:"test".into(),revision:42,elements:vec![
        SnapshotElement { id:2,category:"Pipe".into(),name:"Drain Branch".into(),width_m:0.1,height_m:0.1,elevation_m:1.2,slope_ratio:Some(0.01),geometry:None , section: None, usage: None, material: None, outline: vec![], slope_edges: vec![], slab_kind: None, spacing_m: None, angle_deg: None, parent: None, join_ends: None, count: None, family: None, type_name: String::new(), overrides: vec![], texts: vec![], type_id: None, type_for: None},
        SnapshotElement { id:1,category:"Duct".into(),name:"Supply Branch".into(),width_m:0.2,height_m:0.3,elevation_m:2.7,slope_ratio:Some(0.),geometry:None , section: None, usage: None, material: None, outline: vec![], slope_edges: vec![], slab_kind: None, spacing_m: None, angle_deg: None, parent: None, join_ends: None, count: None, family: None, type_name: String::new(), overrides: vec![], texts: vec![], type_id: None, type_for: None},
        SnapshotElement { id:3,category:"Wall".into(),name:"Corridor".into(),width_m:0.2,height_m:3.,elevation_m:0.,slope_ratio:None,geometry:None , section: None, usage: None, material: None, outline: vec![], slope_edges: vec![], slab_kind: None, spacing_m: None, angle_deg: None, parent: None, join_ends: None, count: None, family: None, type_name: String::new(), overrides: vec![], texts: vec![], type_id: None, type_for: None},
    ] }).unwrap()
}
#[test]
fn pagination_is_stable_and_filters_compose() {
    let s=service();let page=s.query("test",None,None,0,1).unwrap();
    assert_eq!(page.items[0].id,1);assert_eq!(page.total,3);assert_eq!(page.next_offset,Some(1));
    assert_eq!(s.query("test",None,None,1,1).unwrap().items[0].id,2);
    let page=s.query("test",Some("Pipe"),Some("BRANCH"),0,10).unwrap();
    assert_eq!(page.total,1);assert_eq!(page.items[0].id,2);
    assert!(s.query("test",None,None,99,10).unwrap().items.is_empty());
}
#[test]
fn read_only_graph_workflow_preserves_source() {
    let s=service();let graph=s.build_property_graph("test",vec![1,2],Property::Elevation,3.).unwrap();
    let first=s.preview("test",42,&graph,0,1).unwrap();
    assert_eq!(first.total,2);assert_eq!(first.next_offset,Some(1));
    assert_eq!(first.changes[0].element,1);assert_eq!(first.changes[0].after,3.);
    assert_eq!(s.preview("test",42,&graph,1,1).unwrap().changes[0].element,2);
    assert_eq!(s.element("test",1).unwrap().elevation_m,2.7);
    assert_eq!(s.snapshot().revision,42);
}
#[test]
fn identities_limits_and_invalid_properties_are_rejected() {
    let s=service();let graph=Graph::example(vec![1]);
    assert!(s.preview("other",42,&graph,0,10).is_err());
    assert!(s.preview("test",41,&graph,0,10).is_err());
    assert!(s.query("test",None,None,0,101).is_err());
    assert!(s.query("test",Some("Unknown"),None,0,10).is_err());
    assert!(s.element("test",99).is_err());
    assert!(s.build_property_graph("test",vec![3],Property::Slope,0.02).is_err());
    assert!(s.build_property_graph("test",vec![1],Property::Width,0.).is_err());
}
#[test]
fn slope_graph_uses_ratio_type_and_python_is_unavailable() {
    let s=service();let graph=s.build_property_graph("test",vec![2],Property::Slope,0.02).unwrap();
    assert!(matches!(graph.nodes[2].kind,NodeKind::Ratio { value:0.02 }));
    assert_eq!(s.preview("test",42,&graph,0,10).unwrap().changes[0].after,0.02);
    let g=Graph { groups:vec![], version:1,nodes:vec![
        Node { id:1,kind:NodeKind::Length { metres:1. },position:[0.,0.] },
        Node { id:2,kind:NodeKind::Python { expression:"x".into(),quantity:Type::Length },position:[0.,0.] },
    ],wires:vec![Wire { output:0, from:1,to:2,input:0, route: None }] };
    assert!(s.validate(&g).unwrap_err().contains("Python nodes are disabled"));
}
