//! The electrical network: a source (transformer, generator, incoming supply), boards and loads, joined by cables.
//! Every node but the source is fed by one cable, so the network is a tree. Nodes are points on a level in plan; a
//! cable runs between two nodes by way of the plan points the designer clicked. It is kept with the rest of the
//! pipework data (`MepData`), so it is saved, undone and exported with it.
use crate::ElementId;

/// First number of an electrical node (kept apart from fittings and elements).
pub const NODE_BASE: u64 = 1 << 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElecKind {
    /// the root: where the supply comes from
    Source,
    /// a distribution board or switchboard: feeds cables, may carry a load of its own
    Board,
    /// something that consumes power
    Load,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ElecNode {
    pub id: u64,
    pub name: String,
    pub kind: ElecKind,
    pub at: [f64; 2],
    pub level: ElementId,
    /// the real power taken, W (a source takes none)
    pub watts: f64,
    pub power_factor: f64,
    /// of a motor or driver: shaft power over electrical power in, (0, 1]
    pub efficiency: f64,
    /// a motor started direct: the start current over the running current, and the power factor while starting
    pub motor: Option<(f64, f64)>,
    /// the family instance (a chiller, a fan coil, a pump) this load belongs to, and the name of its electrical connector:
    /// the node sits at the connector and takes its power and power factor from it
    pub element: Option<ElementId>,
    pub connector: String,
}

impl ElecNode {
    pub fn new(id: u64, name: &str, kind: ElecKind, at: [f64; 2], level: ElementId) -> Self {
        Self { id, name: name.into(), kind, at, level, watts: 0.0, power_factor: 0.9, efficiency: 1.0, motor: None, element: None, connector: String::new() }
    }
}

/// A cable from the feeding node to the fed one.
#[derive(Clone, Debug, PartialEq)]
pub struct ElecCable {
    pub from: u64,
    pub to: u64,
    /// plan points between the two nodes the route passes
    pub via: Vec<[f64; 2]>,
    pub three_phase: bool,
    /// length added to the drawn route (drops, rises, slack), m
    pub extra_m: f64,
    /// circuits grouped with this one, itself included
    pub grouped_circuits: u32,
    pub ambient_c: Option<f64>,
    /// the share of what hangs below that is on at once, (0, 1]
    pub demand_factor: f64,
    /// residual-current device sensitivity, mA
    pub rcd_ma: Option<f64>,
}

impl ElecCable {
    pub fn new(from: u64, to: u64, three_phase: bool) -> Self {
        Self { from, to, via: Vec::new(), three_phase, extra_m: 0.0, grouped_circuits: 1, ambient_c: None, demand_factor: 1.0, rcd_ma: None }
    }
}

pub(crate) fn valid_nodes(nodes: &[ElecNode], cables: &[ElecCable], levels: &dyn Fn(ElementId) -> bool) -> bool {
    let pos = |v: f64| v.is_finite() && v >= 0.0;
    let mut ids = std::collections::BTreeSet::new();
    for n in nodes {
        let ok = n.id >= NODE_BASE && ids.insert(n.id) && levels(n.level) && n.at.iter().all(|v| v.is_finite())
            && pos(n.watts) && n.power_factor.is_finite() && n.power_factor > 0.0 && n.power_factor <= 1.0
            && n.efficiency.is_finite() && n.efficiency > 0.0 && n.efficiency <= 1.0
            && n.motor.is_none_or(|(m, p)| m.is_finite() && m >= 1.0 && p.is_finite() && p > 0.0 && p <= 1.0)
            && (n.kind != ElecKind::Source || (n.watts == 0.0 && n.motor.is_none()));
        if !ok { return false; }
    }
    let mut fed = std::collections::BTreeSet::new();
    for c in cables {
        let src = nodes.iter().find(|n| n.id == c.from);
        let dst = nodes.iter().find(|n| n.id == c.to);
        let ok = src.is_some_and(|n| n.kind != ElecKind::Load) && dst.is_some_and(|n| n.kind != ElecKind::Source) && c.from != c.to
            && fed.insert(c.to) && c.via.iter().all(|p| p.iter().all(|v| v.is_finite())) && pos(c.extra_m) && c.grouped_circuits >= 1
            && c.ambient_c.is_none_or(f64::is_finite) && c.demand_factor.is_finite() && c.demand_factor > 0.0 && c.demand_factor <= 1.0
            && c.rcd_ma.is_none_or(|m| m.is_finite() && m > 0.0);
        if !ok { return false; }
    }
    true
}
