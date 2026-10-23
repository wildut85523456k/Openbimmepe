//! What this app hands to edesign-alike for a full electrical computation (gap B26, decision 0002).
//!
//! One JSON document, `openbim-electrical-exchange` version 1, written from the model as it is now. It carries the
//! network that is drawn (source, boards, loads, cables with their routes and lengths), every electrical input the model
//! holds on them, the design basis the user gave, the rooms (for lighting loads), and the IFC GlobalId of every
//! element it refers to, so that the design tool can tie its results back to the IFC file exported with the same
//! project key. It also says what it could not supply and whether the network is complete enough to compute at all.
//!
//! Nothing here calculates. Lengths are the drawn route; every other number is a value the user typed or a family gave.
//! The format is this app's: edesign-alike reads it with an importer of its own, and a change that is not backward
//! compatible raises `version`.
use crate::{Document, ElecKind, ElementId};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const FORMAT: &str = "openbim-electrical-exchange";
pub const VERSION: u32 = 1;

fn kind_name(k: ElecKind) -> &'static str {
    match k { ElecKind::Source => "source", ElecKind::Board => "board", ElecKind::Load => "load" }
}

/// The exchange document. `design_basis` is the object of system inputs the user gave (voltage, earthing, fault level,
/// limits); `global_id` gives the IFC GlobalId an element has in the file exported with the same project key.
pub fn build(doc: &Document, project_name: &str, project_key: u64, design_basis: Value, global_id: &dyn Fn(ElementId) -> String) -> Value {
    let mep = doc.mep();
    let node = |id: u64| mep.elec_nodes.iter().find(|n| n.id == id);
    let level_json = |id: ElementId| doc.get(id).map(|l| json!({ "id": l.id.0, "name": l.name, "elevation_m": l.elevation_m }));

    let nodes: Vec<Value> = mep.elec_nodes.iter().map(|n| {
        let element = n.element.and_then(|e| doc.get(e)).map(|e| json!({
            "id": e.id.0, "ifc_global_id": global_id(e.id), "family": e.family, "type": e.type_name, "name": e.name, "connector": n.connector,
        }));
        json!({
            "id": n.id, "label": n.name, "kind": kind_name(n.kind),
            "level": level_json(n.level), "at_m": n.at,
            "active_power_w": n.watts, "power_factor": n.power_factor, "efficiency": n.efficiency,
            "motor_start": n.motor.map(|(ratio, pf)| json!({ "current_over_running": ratio, "power_factor": pf })),
            "element": element,
        })
    }).collect();

    let cables: Vec<Value> = mep.elec_cables.iter().map(|c| {
        let length = match (node(c.from), node(c.to)) { (Some(a), Some(b)) => Some(crate::schematic_electrical::cable_length_m(doc, a, c, b)), _ => None };
        json!({
            "from": c.from, "to": c.to, "three_phase": c.three_phase,
            "route_plan_points_m": c.via, "extra_m": c.extra_m, "length_m": length,
            "grouped_circuits": c.grouped_circuits, "ambient_c": c.ambient_c, "demand_factor": c.demand_factor, "rcd_ma": c.rcd_ma,
            "resistance_ohm_per_km": c.r_ohm_per_km, "reactance_ohm_per_km": c.x_ohm_per_km,
        })
    }).collect();

    let rooms: Vec<Value> = doc.elements().filter(|e| e.category == crate::Category::Room).filter_map(|e| {
        let crate::Geometry::Room { level, .. } = e.geometry? else { return None };
        let area = doc.room_shape(e.id).ok().map(|s| s.area());
        Some(json!({
            "id": e.id.0, "ifc_global_id": global_id(e.id), "number": doc.room_text(e.id, "Number"), "name": e.name,
            "department": doc.room_text(e.id, "Department"), "level": level_json(level), "area_m2": area,
        }))
    }).collect();

    let (blocking, missing) = readiness(doc, &design_basis);
    json!({
        "format": FORMAT, "version": VERSION,
        "project": {
            "name": project_name, "project_key": format!("{project_key:x}"), "model_revision": doc.revision(),
            "site": doc.site().map(|s| json!({ "latitude": s.latitude, "longitude": s.longitude, "angle_to_true_north_deg": s.angle_to_true_north_deg, "utc_offset_hours": s.utc_offset_hours })),
        },
        "design_basis": design_basis,
        "nodes": nodes, "cables": cables, "rooms": rooms,
        "readiness": { "can_compute": blocking.is_empty(), "blocking": blocking, "missing": missing },
    })
}

/// What stops a computation outright, and what the model cannot supply (the design tool must ask or choose).
fn readiness(doc: &Document, basis: &Value) -> (Vec<String>, Vec<String>) {
    let mep = doc.mep();
    let (mut blocking, mut missing) = (Vec::new(), Vec::new());
    let sources = mep.elec_nodes.iter().filter(|n| n.kind == ElecKind::Source).count();
    match sources { 0 => blocking.push("no source".to_string()), 1 => {}, n => blocking.push(format!("{n} sources: one supply is computed at a time")) }
    if !mep.elec_nodes.iter().any(|n| n.kind != ElecKind::Source && n.watts > 0.0) { blocking.push("no load with a power".into()); }
    // every board and load must be reached from the source through the cables
    let mut fed: BTreeSet<u64> = mep.elec_nodes.iter().filter(|n| n.kind == ElecKind::Source).map(|n| n.id).collect();
    loop {
        let before = fed.len();
        for c in &mep.elec_cables { if fed.contains(&c.from) { fed.insert(c.to); } }
        if fed.len() == before { break; }
    }
    for n in mep.elec_nodes.iter().filter(|n| !fed.contains(&n.id)) { blocking.push(format!("\"{}\" is not fed by any cable", n.name)); }
    if mep.elec_cables.iter().any(|c| !mep.elec_nodes.iter().any(|n| n.id == c.from) || !mep.elec_nodes.iter().any(|n| n.id == c.to)) { blocking.push("a cable ends at a node that is gone".into()); }

    let has = |k: &str| basis.get(k).is_some_and(|v| !v.is_null());
    if !has("short_circuit_ka") && !has("source_kva") { missing.push("the source's short-circuit level or rating (the model has none)".into()); }
    if !has("earthing") { missing.push("the earthing system".into()); }
    let no_rx = mep.elec_cables.iter().filter(|c| c.r_ohm_per_km.is_none() || c.x_ohm_per_km.is_none()).count();
    if no_rx > 0 { missing.push(format!("{no_rx} cable(s) with no conductor resistance and reactance: the design tool chooses the cable")); }
    missing.push("cable type, conductor material, cross-section catalogue and installation method (the model does not hold them)".into());
    missing.push("protective devices and their settings (the model does not hold them)".into());
    let unpowered = mep.elec_nodes.iter().filter(|n| n.kind == ElecKind::Load && n.watts <= 0.0).count();
    if unpowered > 0 { missing.push(format!("{unpowered} load(s) with no power")); }
    (blocking, missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, ElecCable, ElecNode, MepData};
    use crate::elec::NODE_BASE;

    fn doc_with(nodes: Vec<ElecNode>, cables: Vec<ElecCable>) -> Document {
        let mut d = Document::spatial_sample();
        d.transact(d.revision(), &[Command::SetMep(Box::new(MepData { elec_nodes: nodes, elec_cables: cables, ..Default::default() }))]).unwrap();
        d
    }

    fn gid(e: ElementId) -> String { format!("GID{}", e.0) }

    fn network() -> Document {
        let l = ElementId(10);
        let (a, b, c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
        let mut load = ElecNode::new(c, "Chiller", ElecKind::Load, [10.0, 0.0], l);
        load.watts = 55_000.0;
        load.motor = Some((6.0, 0.35));
        let mut cable = ElecCable::new(b, c, true);
        cable.via = vec![[5.0, 0.0]];
        cable.extra_m = 2.0;
        cable.grouped_circuits = 3;
        doc_with(vec![ElecNode::new(a, "Utility", ElecKind::Source, [0.0, 0.0], l), ElecNode::new(b, "MSB", ElecKind::Board, [2.0, 0.0], l), load], vec![ElecCable::new(a, b, true), cable])
    }

    #[test]
    fn the_drawn_network_leaves_with_its_lengths_loads_and_the_design_basis() {
        let d = network();
        let v = build(&d, "Plant", 0xabc, json!({ "line_voltage_v": 400.0, "earthing": "TN", "short_circuit_ka": 25.0 }), &gid);
        assert_eq!((v["format"].as_str(), v["version"].as_u64()), (Some(FORMAT), Some(1)));
        assert_eq!(v["project"]["project_key"], "abc");
        assert_eq!(v["nodes"].as_array().unwrap().len(), 3);
        let chiller = &v["nodes"][2];
        assert_eq!((chiller["kind"].as_str(), chiller["active_power_w"].as_f64()), (Some("load"), Some(55_000.0)));
        assert_eq!(chiller["motor_start"]["current_over_running"], 6.0);
        // MSB (2,0) to the chiller (10,0) through (5,0): 3 + 5 = 8 m of plan, plus 2 m extra, on one level
        assert!((v["cables"][1]["length_m"].as_f64().unwrap() - 10.0).abs() < 1e-9, "{}", v["cables"][1]);
        assert_eq!(v["cables"][1]["grouped_circuits"], 3);
        assert_eq!(v["design_basis"]["earthing"], "TN");
        assert_eq!(v["readiness"]["can_compute"], true, "{}", v["readiness"]);
        // what the model cannot give is said, not guessed
        let missing = v["readiness"]["missing"].to_string();
        assert!(missing.contains("protective devices") && missing.contains("cable type"));
        assert!(!missing.contains("short-circuit") && !missing.contains("earthing"), "the basis gave those: {missing}");
    }

    #[test]
    fn a_network_that_cannot_be_computed_says_why() {
        let l = ElementId(10);
        // no source, a load nobody feeds
        let mut load = ElecNode::new(NODE_BASE, "Orphan", ElecKind::Load, [1.0, 1.0], l);
        load.watts = 1000.0;
        let v = build(&doc_with(vec![load], vec![]), "P", 1, json!({}), &gid);
        let blocking = v["readiness"]["blocking"].to_string();
        assert_eq!(v["readiness"]["can_compute"], false);
        assert!(blocking.contains("no source") && blocking.contains("Orphan"), "{blocking}");
        assert!(v["readiness"]["missing"].to_string().contains("short-circuit") && v["readiness"]["missing"].to_string().contains("earthing"));
        // two sources
        let two = doc_with(vec![ElecNode::new(NODE_BASE, "A", ElecKind::Source, [0.0, 0.0], l), ElecNode::new(NODE_BASE + 1, "B", ElecKind::Source, [1.0, 0.0], l)], vec![]);
        assert!(build(&two, "P", 1, json!({}), &gid)["readiness"]["blocking"].to_string().contains("2 sources"));
    }

    #[test]
    fn a_load_that_belongs_to_a_unit_carries_the_units_ifc_id_and_the_rooms_leave_with_their_numbers() {
        let mut d = Document::spatial_sample();
        let l = ElementId(10);
        let fan = d.place_family(l, [2.0, 4.5], &crate::FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 2.2 }).unwrap();
        d.place_walls(l, &[([100.0, 0.0], [106.0, 0.0]), ([106.0, 0.0], [106.0, 4.0]), ([106.0, 4.0], [100.0, 4.0]), ([100.0, 4.0], [100.0, 0.0])], 0.2, 3.0, 0.0, None).unwrap();
        let room = d.place_room(l, [103.0, 2.0]).unwrap();
        d.set_room_data(room, "101", "Plant room", "Services").unwrap();
        let mut load = ElecNode::new(NODE_BASE + 1, "FCU", ElecKind::Load, [2.0, 4.5], l);
        load.watts = 300.0;
        load.element = Some(fan);
        load.connector = "Power".into();
        d.transact(d.revision(), &[Command::SetMep(Box::new(MepData { elec_nodes: vec![ElecNode::new(NODE_BASE, "S", ElecKind::Source, [0.0, 0.0], l), load], elec_cables: vec![ElecCable::new(NODE_BASE, NODE_BASE + 1, false)], ..Default::default() }))]).unwrap();
        let v = build(&d, "P", 1, json!({}), &gid);
        assert_eq!(v["nodes"][1]["element"]["ifc_global_id"], format!("GID{}", fan.0));
        assert_eq!(v["nodes"][1]["element"]["connector"], "Power");
        let r = &v["rooms"][0];
        assert_eq!((r["number"].as_str(), r["name"].as_str(), r["department"].as_str()), (Some("101"), Some("Plant room"), Some("Services")));
        assert!((r["area_m2"].as_f64().unwrap() - 5.8 * 3.8).abs() < 1e-6);
        assert_eq!(r["ifc_global_id"], format!("GID{}", room.0));
    }
}
