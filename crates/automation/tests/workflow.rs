use openbim_automation::*;
use openbim_document::{Category, Command, Document, ElementId};
fn document() -> Document {
    let mut d = Document::new();
    d.transact(0, &[Command::Create { id: ElementId(1), category: Category::Duct,
        name: "Supply".into(), width_m: 0.2, height_m: 0.3, elevation_m: 2.7 }]).unwrap(); d
}
#[test]
fn named_outputs_deliver_distinct_values_and_apply_atomically() {
    let mut d=document(); let mut g=Graph::example(vec![1]);
    g.nodes[1].kind=NodeKind::Dimensions;
    g.nodes[3].kind=NodeKind::Set { property:Property::Width };
    g.wires[2].from=2; g.wires[2].output=1;
    let plan=g.preview(&d).unwrap();
    assert_eq!(plan.port_values()[&(2,0)],Value::Quantities(vec![0.2]));
    assert_eq!(plan.port_values()[&(2,1)],Value::Quantities(vec![0.3]));
    assert_eq!(plan.port_values()[&(2,2)],Value::Quantities(vec![2.7]));
    let json=g.to_json().unwrap(); assert!(json.contains("\"output\": 1"));
    assert_eq!(Graph::from_json(&json).unwrap(),g);
    plan.apply(&mut d).unwrap(); assert_eq!(d.get(ElementId(1)).unwrap().width_m,0.3);
    d.undo().unwrap(); assert_eq!(d.get(ElementId(1)).unwrap().width_m,0.2);
    g.wires[2].output=17;
    assert_eq!(g.validate(),Err(Error::InvalidOutput(2,17)));
}
#[test]
fn legacy_wires_default_to_first_output() {
    let json=Graph::example(vec![1]).to_json().unwrap();
    assert!(!json.contains("\"output\""));
    let graph=Graph::from_json(&json).unwrap();
    assert!(graph.wires.iter().all(|w|w.output==0));
    assert!(graph.preview(&document()).is_ok());
}
#[test]
fn preview_apply_undo_and_idempotent_rerun() {
    let mut d = document(); let before = d.clone(); let g = Graph::example(vec![1]);
    let plan = g.preview(&d).unwrap();
    assert_eq!(d, before); assert_eq!(plan.changes().len(), 1);
    plan.apply(&mut d).unwrap(); assert_eq!(d.get(ElementId(1)).unwrap().elevation_m, 3.);
    let revision = d.revision();
    let plan = g.preview(&d).unwrap(); assert!(plan.changes().is_empty());
    plan.apply(&mut d).unwrap(); assert_eq!(d.revision(), revision);
    d.undo().unwrap(); assert_eq!(d.get(ElementId(1)).unwrap().elevation_m, 2.7);
}
#[test]
fn stale_preview_is_rejected_without_partial_changes() {
    let mut d = document(); let plan = Graph::example(vec![1]).preview(&d).unwrap();
    d.transact(d.revision(), &[Command::SetSlope { id: ElementId(1), slope_ratio: 0.02 }]).unwrap();
    let before = d.clone(); assert!(plan.apply(&mut d).is_err()); assert_eq!(d, before);
}
#[test]
fn malformed_connections_and_units_are_rejected() {
    let mut g = Graph::example(vec![1]); g.nodes[2].kind = NodeKind::Ratio { value: 0.02 };
    assert!(matches!(g.validate(), Err(Error::TypeMismatch(4,1))));
    let mut g = Graph::example(vec![1]); g.wires.push(g.wires[0].clone());
    assert!(matches!(g.validate(), Err(Error::DuplicateInput(2,0))));
    let mut g = Graph::example(vec![1]); g.wires[0].from = 99;
    assert!(matches!(g.validate(), Err(Error::MissingNode(99))));
}
#[test]
fn invalid_dimensions_fail_in_preview() {
    let d = document(); let mut g = Graph::example(vec![1]);
    g.nodes[2].kind = NodeKind::Length { metres: -1. };
    g.nodes[3].kind = NodeKind::Set { property: Property::Width };
    assert!(matches!(g.preview(&d),Err(Error::Document(_))));
    assert_eq!(d.get(ElementId(1)).unwrap().width_m,0.2);
}
#[test]
fn cycles_and_unsupported_properties_fail() {
    let mut g = Graph::example(vec![1]); g.wires[1].from = 4;
    assert_eq!(g.validate(),Err(Error::Cycle));
    let mut d = document();
    d.transact(d.revision(), &[Command::Create { id: ElementId(2), category: Category::Wall,
        name: "Wall".into(), width_m: 0.2, height_m: 3., elevation_m: 0. }]).unwrap();
    let mut g = Graph::example(vec![2]);
    g.nodes[1].kind = NodeKind::Read { property: Property::Slope };
    assert!(matches!(g.preview(&d),Err(Error::UnsupportedProperty(2))));
}
#[test]
fn graph_roundtrip_and_version_checks() {
    let g = Graph::example(vec![1]);
    assert_eq!(Graph::from_json(&g.to_json().unwrap()).unwrap(),g);
    let mut bad = g; bad.version = 2; assert_eq!(bad.validate(),Err(Error::Version));
}
#[test]
fn independent_width_and_height_writes_compose() {
    let mut d = document(); let mut g = Graph::example(vec![1]);
    g.nodes[3].kind = NodeKind::Set { property: Property::Width };
    g.nodes.push(Node { id:5,kind:NodeKind::Length { metres:2. },position:[0.,0.] });
    g.nodes.push(Node { id:6,kind:NodeKind::Set { property:Property::Height },position:[0.,0.] });
    g.wires.extend([Wire { output:0, from:1,to:6,input:0, route: None },Wire { output:0, from:5,to:6,input:1, route: None }]);
    g.preview(&d).unwrap().apply(&mut d).unwrap();
    let e = d.get(ElementId(1)).unwrap(); assert_eq!((e.width_m,e.height_m),(3.,2.));
}

#[test]
fn duplicate_writers_and_invalid_layout_are_rejected() {
    let mut g=Graph::example(vec![1]);
    g.nodes.push(Node { id:5,kind:NodeKind::Set { property:Property::Elevation },position:[0.,0.] });
    g.wires.extend([Wire { output:0, from:1,to:5,input:0, route: None },Wire { output:0, from:3,to:5,input:1, route: None }]);
    assert!(matches!(g.preview(&document()),Err(Error::ConflictingWrite(1,Property::Elevation))));
    let mut g=Graph::example(vec![1]);g.nodes[0].position[0]=f32::NAN;
    assert_eq!(g.validate(),Err(Error::InvalidNumber(1)));
}

#[test]
fn scheduler_handles_reverse_order_dependencies() {
    let mut nodes=vec![Node { id:5001,kind:NodeKind::Selection { ids:vec![] },position:[0.,0.] },
        Node { id:5002,kind:NodeKind::Length { metres:1. },position:[0.,0.] }];
    let mut wires=Vec::new();
    for id in 1..=5000 {
        nodes.push(Node { id,kind:NodeKind::Set { property:Property::Elevation },position:[0.,0.] });
        wires.push(Wire { output:0, from:id+1,to:id,input:0, route: None });
        wires.push(Wire { output:0, from:5002,to:id,input:1, route: None });
    }
    let order=Graph { version:1,nodes,wires }.validate().unwrap();
    assert_eq!(order.len(),5002);
    assert_eq!(*order.last().unwrap(),1);
}

#[test]
fn visual_wire_routes_roundtrip_without_changing_execution() {
    let original=Graph::example(vec![1]);
    let mut routed=original.clone();
    routed.wires[1].route=Some([50,-40]);
    let decoded=Graph::from_json(&routed.to_json().unwrap()).unwrap();
    assert_eq!(decoded,routed);
    assert_eq!(decoded.preview(&document()).unwrap().changes(),original.preview(&document()).unwrap().changes());
    let old=original.to_json().unwrap();
    assert!(!old.contains("route"));
    assert!(Graph::from_json(&old).unwrap().wires.iter().all(|w|w.route.is_none()));
    routed.wires[0].route=Some([100001,0]);
    assert!(matches!(routed.validate(),Err(Error::InvalidRoute(_, _))));
}
