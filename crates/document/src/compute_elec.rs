//! DEPRECATED (gap B26, owner decision of 2026-10-21, option B). Electrical calculation belongs to edesign-alike and is
//! not to be developed further here: no new features, only fixes. This engine stays because edesign-alike (pinned at
//! 57ef018, checked 23 Oct 2026) does not yet size cables: its solver checks a cable's ampacity that is given to it
//! (Ib <= Iz), and reads currents, voltage drop and short-circuit from it, but does not choose a cable from tables
//! with correction factors, check the protective device, trip and breaking, or size the protective conductor. When it
//! does, move the Compute electrical window to it and delete this file (`docs/decisions/0002-electrical-engine.md`).
//! Cable and protection sizing of the drawn electrical network: the source, boards and loads become a tree for
//! `openbim-calc` (IEC 60364 method, the owner's tables), and the answer is given back per cable and per load.
//! Cable length is the drawn route (source to node through the plan points clicked) plus the height between the two
//! levels plus the allowance on the cable. Cables are not written back to the model: the report is the result.
use crate::{Document, ElecKind};

/// Shown in the Compute electrical window: what this tool is and where the calculation is going.
pub const DEPRECATION_NOTICE: &str = "This sizing tool is deprecated: electrical calculation is moving to edesign-alike, which does not size cables yet. It stays until it does.";
use openbim_calc as calc;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ElecComputeSettings {
    /// all the tables, devices and limits; every cable takes the first table
    pub calc: calc::ESettings,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElecComputeError {
    NoSource,
    MoreThanOneSource,
    /// a board or load that no cable feeds
    NotFed(u64),
    /// the cables form a loop at this node
    Loop(u64),
    NoLoad,
    /// the calculation refused: the error and the node it is about
    Calc(calc::EError, Option<u64>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CableReport {
    pub from: u64,
    pub to: u64,
    pub length_m: f64,
    /// real and reactive power the cable carries, W and var
    pub power: (f64, f64),
    pub design_current: f64,
    pub device_a: f64,
    pub cable: String,
    pub parallel: u32,
    pub capacity_a: f64,
    pub drop_pct: f64,
    pub path_drop_pct: f64,
    pub pe_area: f64,
    pub fault_end_min_a: f64,
    pub trip_ok: bool,
    pub breaking_ok: bool,
    pub rcd_ok: Option<bool>,
    pub fails: bool,
    pub critical: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ElecReport {
    pub cables: Vec<CableReport>,
    /// per load or board with no cable below it: the drop to it, the margin to the limit, the motor starting drop
    pub ends: Vec<(u64, f64, f64, Option<f64>)>,
    pub source_power_factor: f64,
    pub source_loading_pct: Option<f64>,
    pub total_kw: f64,
    pub total_kva: f64,
    pub critical_nodes: Vec<u64>,
}

pub fn compute_electrical(doc: &Document, s: &ElecComputeSettings) -> Result<ElecReport, ElecComputeError> {
    let m = doc.mep();
    let sources: Vec<usize> = (0..m.elec_nodes.len()).filter(|i| m.elec_nodes[*i].kind == ElecKind::Source).collect();
    match sources.len() { 0 => return Err(ElecComputeError::NoSource), 1 => {}, _ => return Err(ElecComputeError::MoreThanOneSource) }
    if !m.elec_nodes.iter().any(|n| n.kind != ElecKind::Source && n.watts > 0.0) { return Err(ElecComputeError::NoLoad); }
    let index: BTreeMap<u64, usize> = m.elec_nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    let z = |n: &crate::ElecNode| doc.get(n.level).map_or(0.0, |l| l.elevation_m);
    let mut nodes: Vec<calc::ENode> = Vec::new();
    for n in &m.elec_nodes {
        let load = (n.kind != ElecKind::Source && n.watts > 0.0).then(|| calc::Load {
            watts: n.watts, power_factor: n.power_factor, efficiency: n.efficiency,
            motor: n.motor.map(|(a, b)| calc::Motor { start_multiple: a, start_power_factor: b }),
        });
        nodes.push(calc::ENode { parent: None, load, branch: calc::EBranch::new(0.0, calc::Phases::Three) });
    }
    for c in &m.elec_cables {
        let (Some(&a), Some(&b)) = (index.get(&c.from), index.get(&c.to)) else { continue };
        let (na, nb) = (&m.elec_nodes[a], &m.elec_nodes[b]);
        let mut pts = vec![na.at];
        pts.extend(c.via.iter().copied());
        pts.push(nb.at);
        let plan: f64 = pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
        let mut br = calc::EBranch::new(plan + (z(na) - z(nb)).abs() + c.extra_m, if c.three_phase { calc::Phases::Three } else { calc::Phases::Single });
        br.grouped_circuits = c.grouped_circuits;
        br.ambient_c = c.ambient_c;
        br.demand_factor = c.demand_factor;
        br.rcd_ma = c.rcd_ma;
        nodes[b].parent = Some(a);
        nodes[b].branch = br;
    }
    if let Some(i) = (0..nodes.len()).find(|i| nodes[*i].parent.is_none() && *i != sources[0]) { return Err(ElecComputeError::NotFed(m.elec_nodes[i].id)); }
    let tree = calc::ETree { nodes };
    let r = calc::compute_electrical(&tree, &s.calc).map_err(|e| {
        use calc::EError as E;
        let node = match e { E::BadParent(i) | E::Disconnected(i) | E::BadValue(i, _) | E::NoRating(i) | E::NoCable(i) | E::DropNotMet(i) => m.elec_nodes.get(i).map(|n| n.id), _ => None };
        match (&e, node) { (E::Disconnected(_), Some(id)) => ElecComputeError::Loop(id), _ => ElecComputeError::Calc(e, node) }
    })?;
    let label = |t: usize, row: Option<usize>| row.and_then(|r| s.calc.tables.get(t).and_then(|t| t.rows.get(r))).map_or(String::new(), |r| r.label.clone());
    let critical: std::collections::BTreeSet<usize> = r.critical_path.iter().copied().collect();
    let mut cables = Vec::new();
    for c in &m.elec_cables {
        let Some(&b) = index.get(&c.to) else { continue };
        let x = &r.branches[b];
        cables.push(CableReport {
            from: c.from, to: c.to, length_m: tree.nodes[b].branch.length_m, power: x.power, design_current: x.design_current, device_a: x.device,
            cable: label(x.table, x.cable), parallel: x.parallel, capacity_a: x.capacity, drop_pct: x.drop_pct, path_drop_pct: x.path_drop_pct, pe_area: x.pe_area,
            fault_end_min_a: x.fault_end_min, trip_ok: x.trip_ok, breaking_ok: x.breaking_ok, rcd_ok: x.rcd_ok, fails: x.fails, critical: critical.contains(&b),
        });
    }
    let src = &r.branches[sources[0]];
    Ok(ElecReport {
        cables,
        ends: r.ends.iter().map(|e| (m.elec_nodes[e.node].id, e.path_drop_pct, e.margin_pct, e.start_drop_pct)).collect(),
        source_power_factor: r.source_power_factor,
        source_loading_pct: r.source_loading_pct,
        total_kw: src.power.0 / 1000.0,
        total_kva: src.power.0.hypot(src.power.1) / 1000.0,
        critical_nodes: r.critical_path.iter().map(|i| m.elec_nodes[*i].id).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_says_it_is_deprecated_and_where_the_calculation_is_going() {
        assert!(DEPRECATION_NOTICE.contains("deprecated") && DEPRECATION_NOTICE.contains("edesign-alike"));
    }
    use crate::{Command, ElecCable, ElecNode, ElementId, MepData};
    use crate::elec::NODE_BASE;

    const TABLE: &str = "1.5mm2,1.5,12.1,0.1,19\n2.5mm2,2.5,7.41,0.1,26\n4mm2,4,4.61,0.1,35\n6mm2,6,3.08,0.1,44\n10mm2,10,1.83,0.08,60\n16mm2,16,1.15,0.08,80\n25mm2,25,0.727,0.08,105\n35mm2,35,0.524,0.08,128\n50mm2,50,0.387,0.08,155\n70mm2,70,0.268,0.08,200\n95mm2,95,0.193,0.08,243\n120mm2,120,0.153,0.08,282\n";

    fn settings() -> ElecComputeSettings {
        let t = calc::CableTable::from_csv("Cu PVC", 115.0, TABLE).unwrap();
        ElecComputeSettings { calc: calc::ESettings::new(400.0, vec![t], 5.0) }
    }

    fn network() -> Document {
        let mut d = Document::spatial_sample();
        let (a, b, c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
        let mut chiller = ElecNode::new(c, "Chiller", ElecKind::Load, [20.0, 10.0], ElementId(11));
        chiller.watts = 45000.0;
        chiller.power_factor = 0.85;
        let mut cable = ElecCable::new(b, c, true);
        cable.via = vec![[20.0, 0.0]];
        let data = MepData {
            elec_nodes: vec![ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)), ElecNode::new(b, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10)), chiller],
            elec_cables: vec![ElecCable::new(a, b, true), cable],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        d
    }

    #[test]
    fn the_drawn_network_is_sized_with_the_route_length_and_errors_name_the_node() {
        let d = network();
        let r = compute_electrical(&d, &settings()).unwrap();
        assert_eq!(r.cables.len(), 2);
        let feeder = r.cables.iter().find(|c| c.to == NODE_BASE + 2).unwrap();
        // 10 m along, 10 m across (20 m in plan) and 3 m up between the levels
        assert!((feeder.length_m - 23.0).abs() < 1e-9, "{}", feeder.length_m);
        assert!(feeder.design_current > 70.0 && feeder.design_current < 90.0, "45 kW at 0.85 on 400 V is about 76 A: {}", feeder.design_current);
        assert!(!feeder.cable.is_empty() && feeder.capacity_a >= feeder.design_current);
        assert!(r.cables.iter().all(|c| c.critical));
        assert!((r.total_kw - 45.0).abs() < 1e-9);
        assert_eq!(r.ends.len(), 1);
        // errors
        let mut none = d.clone();
        let mut m = none.mep().clone();
        m.elec_cables.pop();
        none.transact(none.revision(), &[Command::SetMep(Box::new(m))]).unwrap();
        assert_eq!(compute_electrical(&none, &settings()), Err(ElecComputeError::NotFed(NODE_BASE + 2)));
        let mut nosrc = d.clone();
        let mut m = nosrc.mep().clone();
        m.elec_nodes.remove(0);
        m.elec_cables.remove(0);
        nosrc.transact(nosrc.revision(), &[Command::SetMep(Box::new(m))]).unwrap();
        assert_eq!(compute_electrical(&nosrc, &settings()), Err(ElecComputeError::NoSource));
        // a table too small for the load
        let t = calc::CableTable::from_csv("small", 115.0, "1.5mm2,1.5,12.1,0.1,19\n").unwrap();
        let tiny = ElecComputeSettings { calc: calc::ESettings::new(400.0, vec![t], 5.0) };
        assert!(matches!(compute_electrical(&d, &tiny), Err(ElecComputeError::Calc(calc::EError::NoCable(_), Some(_)))));
    }
}
