//! Sizing and losses of a drawn system: the Compute button's engine. The drawn ducts and pipes of one system are read
//! as a tree from its source (a `Source` end point) out to its terminals (end points with a demand), handed to
//! `openbim-calc`, and the answer is given back per run, so that sizes can be written to the model in one step.
//!
//! Sizes: a pipe's width is its inside diameter and a duct's width and height are its inside dimensions, so what is
//! drawn is what is computed. Fitting losses are the owner's coefficients by kind of fitting; a fitting's coefficient is
//! put on every run leaving it (a tee is therefore a little pessimistic for the main). A loop is refused (ring mains
//! are not yet computed).
use crate::{Category, Command, Document, EditError, ElementId, EndDemand, EndRole, Geometry, MepNode, MepPort};
use openbim_calc as calc;
use openbim_family::mep::System;
use openbim_route::FittingKind;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Loss coefficients (velocity heads) of the fittings, as the owner's tables give them.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FittingLosses {
    pub elbow: f64,
    pub transition: f64,
    pub tee: f64,
    pub tap: f64,
}

#[derive(Clone, Debug)]
pub struct ComputeSettings {
    pub system: System,
    pub calc: calc::Settings,
    pub losses: FittingLosses,
    /// keep the drawn sizes (the result only reports whether they meet the limits) instead of choosing sizes
    pub keep_sizes: bool,
    /// the named system object to compute (B18): only the ducts or pipes reachable from its equipment are read, so two
    /// loops of one type in one model compute apart. `None` reads every run of `system`, as before.
    pub scope: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComputeError {
    /// no source end point on this system
    NoSource,
    /// more than one source; one pump or fan per computation
    MoreThanOneSource,
    /// no terminal with a demand is connected to the source
    NoTerminal,
    /// the run closes a loop
    Loop(ElementId),
    /// the named system is not in the model
    UnknownSystem,
    Calc(calc::TreeError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunReport {
    pub run: ElementId,
    /// the flow at its start, m3/s
    pub flow: f64,
    pub size: calc::Size,
    pub velocity: f64,
    pub friction_per_metre: f64,
    /// friction, fittings and height over the whole run, Pa
    pub dp_total: f64,
    pub length: f64,
    /// a drawn size that breaks a limit (only with `keep_sizes`)
    pub exceeds_limits: bool,
    /// on the most resistant path
    pub critical: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EndReport {
    pub port: Option<MepPort>,
    /// the pressure the source must give for this end point, Pa
    pub needed_at_source: f64,
    /// what a balancing valve or damper must take up, Pa
    pub excess: f64,
    pub flow: f64,
    /// the pressure it gets (sprinklers and the like), Pa
    pub pressure: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemReport {
    pub system: System,
    pub runs: Vec<RunReport>,
    pub ends: Vec<EndReport>,
    /// the pump or fan duty, Pa
    pub source_pressure: f64,
    pub total_flow: f64,
    /// terminals of this system that are not connected to its source
    pub unreached: Vec<MepPort>,
    pub critical_runs: Vec<ElementId>,
}

impl SystemReport {
    /// The commands that set the computed sizes on the runs (those that differ from what is drawn).
    pub fn resize_commands(&self, doc: &Document) -> Vec<Command> {
        self.runs.iter().filter_map(|r| {
            let e = doc.get(r.run)?;
            let (w, h) = match r.size.shape { calc::Shape::Round { diameter } => (diameter, diameter), calc::Shape::Rect { width, height } => (width, height) };
            ((e.width_m - w).abs() > 1e-9 || (e.height_m - h).abs() > 1e-9).then_some(Command::Resize { id: r.run, width_m: w, height_m: h })
        }).collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key { Port(MepPort), Mid(ElementId, usize) }

struct Uf(Vec<usize>);
impl Uf {
    fn find(&mut self, a: usize) -> usize { let p = self.0[a]; if p == a { a } else { let r = self.find(p); self.0[a] = r; r } }
    fn union(&mut self, a: usize, b: usize) { let (a, b) = (self.find(a), self.find(b)); if a != b { self.0[b] = a; } }
}

struct Ids { map: BTreeMap<Key, usize>, uf: Uf }
impl Ids {
    fn id(&mut self, k: Key) -> usize {
        if let Some(i) = self.map.get(&k) { return *i; }
        let i = self.uf.0.len();
        self.uf.0.push(i);
        self.map.insert(k, i);
        i
    }
}

struct Edge { run: ElementId, a: usize, b: usize, len: f64 }

/// The system a run belongs to: what the designer said, otherwise that of the fitting it is joined to.
pub fn system_of_run(doc: &Document, id: ElementId) -> System {
    let m = doc.mep();
    if let Some(s) = m.run_system(id) { return s; }
    let fitting_system = |f: u64| m.fitting(f).map(|f| f.system);
    for (a, b) in &m.links {
        for (x, y) in [(a, b), (b, a)] {
            if x.node == MepNode::Run(id) { if let MepNode::Fitting(f) = y.node { if let Some(s) = fitting_system(f) { return s; } } }
        }
    }
    m.taps.iter().find(|t| t.run == id).and_then(|t| fitting_system(t.fitting)).unwrap_or(System::Other)
}

fn run_z(doc: &Document, id: ElementId) -> Option<(f64, f64, f64)> {
    let e = doc.get(id)?;
    let Some(Geometry::Linear { start, end, level }) = e.geometry else { return None };
    let plan = (end[0] - start[0]).hypot(end[1] - start[1]);
    let z0 = doc.get(level).map_or(0.0, |l| l.elevation_m) + e.elevation_m;
    let z1 = z0 + e.slope_ratio.unwrap_or(0.0) * plan;
    Some((plan, z0, z1))
}

fn demand_of(d: EndDemand) -> calc::Demand {
    match d {
        EndDemand::None => calc::Demand::None,
        EndDemand::Flow(q) => calc::Demand::Flow(q),
        EndDemand::Power { watts, delta_t } => calc::Demand::Power { watts, delta_t },
        EndDemand::LoadingUnits(u) => calc::Demand::LoadingUnits(u),
        EndDemand::Sprinkler { k_l_min_bar, min_flow_l_min } => calc::Demand::Sprinkler { k: calc::k_factor(k_l_min_bar), min_flow: min_flow_l_min / 60_000.0 },
    }
}

/// The ducts and pipes reachable from the equipment of the named system, through fittings, joints and taps.
fn system_reach(doc: &Document, name: &str) -> Result<BTreeSet<ElementId>, ComputeError> {
    let m = doc.mep();
    let def = m.systems.iter().find(|d| d.name == name).ok_or(ComputeError::UnknownSystem)?;
    let mut adj: BTreeMap<MepNode, Vec<MepNode>> = BTreeMap::new();
    let mut join = |a: MepNode, b: MepNode| { adj.entry(a).or_default().push(b); adj.entry(b).or_default().push(a); };
    for (a, b) in &m.links { join(a.node, b.node); }
    for t in &m.taps { join(MepNode::Run(t.run), MepNode::Fitting(t.fitting)); }
    let mut seen: BTreeSet<MepNode> = BTreeSet::new();
    let mut queue: VecDeque<MepNode> = m.equip_links.iter().filter(|q| def.members.contains(&q.element)).map(|q| q.port.node).collect();
    while let Some(n) = queue.pop_front() {
        if !seen.insert(n) { continue; }
        for &v in adj.get(&n).map(Vec::as_slice).unwrap_or(&[]) { if !seen.contains(&v) { queue.push_back(v); } }
    }
    Ok(seen.into_iter().filter_map(|n| if let MepNode::Run(r) = n { Some(r) } else { None }).collect())
}

/// Reads one system of the drawing as a tree, sizes it and reports. See the module description.
pub fn compute_system(doc: &Document, s: &ComputeSettings) -> Result<SystemReport, ComputeError> {
    let data = doc.mep();
    let runs: Vec<ElementId> = doc.elements().filter(|e| matches!(e.category, Category::Duct | Category::Pipe) && run_z(doc, e.id).is_some())
        .map(|e| e.id).filter(|id| system_of_run(doc, *id) == s.system).collect();
    let runs: Vec<ElementId> = match &s.scope {
        None => runs,
        Some(name) => { let reach = system_reach(doc, name)?; runs.into_iter().filter(|r| reach.contains(r)).collect() }
    };
    let mut ids = Ids { map: BTreeMap::new(), uf: Uf(Vec::new()) };
    // pieces: a run is cut where a tap sits on it
    let mut edges: Vec<Edge> = Vec::new();
    for &r in &runs {
        let (plan, _, _) = run_z(doc, r).unwrap_or((0.0, 0.0, 0.0));
        let el = doc.get(r).map_or(0.0, |e| e.slope_ratio.unwrap_or(0.0));
        let total = plan * (1.0 + el * el).sqrt();
        let mut taps: Vec<&crate::MepTap> = data.taps.iter().filter(|t| t.run == r).collect();
        taps.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        let mut stops = vec![(Key::Port(MepPort { node: MepNode::Run(r), port: 0 }), 0.0)];
        for (i, t) in taps.iter().enumerate() {
            let mid = ids.id(Key::Mid(r, i));
            if let Some(f) = data.fitting(t.fitting) {
                for leg in 0..f.fitting.ports.len() { let k = ids.id(Key::Port(MepPort { node: MepNode::Fitting(t.fitting), port: leg })); ids.uf.union(mid, k); }
            }
            stops.push((Key::Mid(r, i), if plan > 0.0 { t.distance.min(plan) / plan * total } else { 0.0 }));
        }
        stops.push((Key::Port(MepPort { node: MepNode::Run(r), port: 1 }), total));
        for w in stops.windows(2) {
            let (a, b) = (ids.id(w[0].0), ids.id(w[1].0));
            edges.push(Edge { run: r, a, b, len: (w[1].1 - w[0].1).max(0.0) });
        }
    }
    for f in &data.fittings {
        let first = ids.id(Key::Port(MepPort { node: MepNode::Fitting(f.id), port: 0 }));
        for leg in 1..f.fitting.ports.len() { let k = ids.id(Key::Port(MepPort { node: MepNode::Fitting(f.id), port: leg })); ids.uf.union(first, k); }
    }
    for (a, b) in &data.links { let (x, y) = (ids.id(Key::Port(*a)), ids.id(Key::Port(*b))); ids.uf.union(x, y); }
    let mut ends_at: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, e) in data.endpoints.iter().enumerate() { let k = ids.id(Key::Port(e.port)); let r = ids.uf.find(k); ends_at.entry(r).or_default().push(i); }
    for e in &mut edges { e.a = ids.uf.find(e.a); e.b = ids.uf.find(e.b); }
    // fittings by junction, for their loss coefficients
    let mut fit_at: BTreeMap<usize, BTreeSet<u64>> = BTreeMap::new();
    for f in &data.fittings { let k = ids.id(Key::Port(MepPort { node: MepNode::Fitting(f.id), port: 0 })); let r = ids.uf.find(k); fit_at.entry(r).or_default().insert(f.id); }
    let k_of = |junction: usize| -> f64 {
        fit_at.get(&junction).map_or(0.0, |set| set.iter().filter_map(|id| data.fitting(*id)).map(|f| match f.fitting.kind {
            FittingKind::Elbow { .. } => s.losses.elbow, FittingKind::Transition => s.losses.transition, FittingKind::Tee { .. } => s.losses.tee, FittingKind::Tap { .. } => s.losses.tap,
        }).sum())
    };

    let on_edges: BTreeSet<usize> = edges.iter().flat_map(|e| [e.a, e.b]).collect();
    let sources: Vec<usize> = data.endpoints.iter().enumerate().filter(|(_, e)| e.role == EndRole::Source)
        .filter_map(|(i, _)| { let k = ids.map.get(&Key::Port(data.endpoints[i].port)).copied()?; let r = ids.uf.find(k); on_edges.contains(&r).then_some(r) }).collect();
    let root = match sources.len() { 0 => return Err(ComputeError::NoSource), 1 => sources[0], _ => return Err(ComputeError::MoreThanOneSource) };

    // the tree, by breadth from the source
    let mut adj: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() { adj.entry(e.a).or_default().push(i); adj.entry(e.b).or_default().push(i); }
    struct Built { junction: usize, parent: Option<usize>, edge: Option<usize> }
    let mut nodes: Vec<Built> = vec![Built { junction: root, parent: None, edge: None }];
    let mut seen: BTreeMap<usize, usize> = BTreeMap::from([(root, 0)]);
    let mut used = BTreeSet::new();
    let mut queue = VecDeque::from([0usize]);
    while let Some(u) = queue.pop_front() {
        let uj = nodes[u].junction;
        for &ei in adj.get(&uj).map(Vec::as_slice).unwrap_or(&[]) {
            if !used.insert(ei) { continue; }
            let e = &edges[ei];
            let v = if e.a == uj { e.b } else { e.a };
            if v == uj || seen.contains_key(&v) { return Err(ComputeError::Loop(e.run)); }
            seen.insert(v, nodes.len());
            nodes.push(Built { junction: v, parent: Some(u), edge: Some(ei) });
            queue.push_back(nodes.len() - 1);
        }
    }
    let size_of_run = |id: ElementId| -> calc::Size {
        let e = doc.get(id).expect("a run that exists");
        if e.category == Category::Pipe { calc::Size::round(&format!("{:.1} mm", e.width_m * 1000.0), e.width_m) }
        else { calc::Size::rect(&format!("{:.0} x {:.0}", e.width_m * 1000.0, e.height_m * 1000.0), e.width_m, e.height_m) }
    };
    // the height of every junction, from the ends of the runs (and, for a tap, along its run)
    let mut zmap: BTreeMap<usize, f64> = BTreeMap::new();
    for &r in &runs {
        let Some((plan, z0, z1)) = run_z(doc, r) else { continue };
        for (port, z) in [(0usize, z0), (1, z1)] {
            if let Some(k) = ids.map.get(&Key::Port(MepPort { node: MepNode::Run(r), port })).copied() { let j = ids.uf.find(k); zmap.entry(j).or_insert(z); }
        }
        let mut taps: Vec<&crate::MepTap> = data.taps.iter().filter(|t| t.run == r).collect();
        taps.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        for (i, t) in taps.iter().enumerate() {
            if let Some(k) = ids.map.get(&Key::Mid(r, i)).copied() {
                let j = ids.uf.find(k);
                let f = if plan > 0.0 { (t.distance / plan).clamp(0.0, 1.0) } else { 0.0 };
                zmap.entry(j).or_insert(z0 + (z1 - z0) * f);
            }
        }
    }
    let z_of_junction = |j: usize| -> f64 { zmap.get(&j).copied().unwrap_or(0.0) };
    let build = |locked: &BTreeMap<ElementId, calc::Size>| -> calc::Tree {
        calc::Tree { nodes: nodes.iter().map(|n| {
            let mut demand = calc::Demand::None;
            let mut required = 0.0;
            for &ei in ends_at.get(&n.junction).map(Vec::as_slice).unwrap_or(&[]) {
                let e = &data.endpoints[ei];
                if e.role == EndRole::Terminal && e.demand != EndDemand::None { demand = demand_of(e.demand); required = e.required_pressure_pa; }
            }
            let (length, k, lock) = match (n.edge, n.parent) {
                (Some(ei), Some(p)) => {
                    let run = edges[ei].run;
                    (edges[ei].len, k_of(nodes[p].junction), locked.get(&run).cloned().or_else(|| s.keep_sizes.then(|| size_of_run(run))))
                }
                _ => (0.0, 0.0, None),
            };
            calc::Node { parent: n.parent, demand, required_pressure: required, elevation: z_of_junction(n.junction), branch: calc::Branch { length, fitting_k: k, locked: lock } }
        }).collect() }
    };
    let solve = |t: &calc::Tree| -> Result<(calc::Results, Option<Vec<calc::SprinklerResult>>), ComputeError> {
        if t.nodes.iter().any(|n| matches!(n.demand, calc::Demand::Sprinkler { .. })) {
            let r = calc::solve_sprinklers(t, &s.calc).map_err(ComputeError::Calc)?;
            Ok((r.results, Some(r.sprinklers)))
        } else {
            Ok((calc::compute(t, &s.calc).map_err(ComputeError::Calc)?, None))
        }
    };
    let mut tree = build(&BTreeMap::new());
    if !tree.nodes.iter().any(|n| n.demand != calc::Demand::None) { return Err(ComputeError::NoTerminal); }
    let (mut results, mut sprinklers) = solve(&tree)?;
    // a run cut by taps has one size: the largest any piece needed, and then everything is computed again with it
    let mut per_run: BTreeMap<ElementId, Vec<usize>> = BTreeMap::new();
    for (i, n) in nodes.iter().enumerate() { if let Some(ei) = n.edge { per_run.entry(edges[ei].run).or_default().push(i); } }
    if !s.keep_sizes && per_run.values().any(|v| v.len() > 1) {
        let locked: BTreeMap<ElementId, calc::Size> = per_run.iter().filter_map(|(r, v)| {
            v.iter().filter_map(|i| results.branches[*i].size.clone()).max_by(|a, b| a.area().total_cmp(&b.area())).map(|z| (*r, z))
        }).collect();
        tree = build(&locked);
        (results, sprinklers) = solve(&tree)?;
    }
    let on_path: BTreeSet<usize> = results.critical_path.iter().copied().collect();
    let mut reports = Vec::new();
    for (run, pieces) in &per_run {
        let first = pieces.iter().copied().min_by_key(|i| nodes[*i].edge.map(|e| (edges[e].a, edges[e].b)).unwrap_or((0, 0))).unwrap_or(pieces[0]);
        let head = pieces.iter().copied().max_by(|a, b| results.branches[*a].flow.total_cmp(&results.branches[*b].flow)).unwrap_or(first);
        let b = &results.branches[head];
        let length: f64 = pieces.iter().map(|i| edges[nodes[*i].edge.unwrap_or(0)].len).sum();
        reports.push(RunReport {
            run: *run, flow: b.flow, size: b.size.clone().unwrap_or_else(|| size_of_run(*run)), velocity: b.velocity, friction_per_metre: b.friction_per_metre,
            dp_total: pieces.iter().map(|i| results.branches[*i].dp_total).sum(), length,
            exceeds_limits: pieces.iter().any(|i| results.branches[*i].exceeds_limits),
            critical: pieces.iter().any(|i| on_path.contains(i)),
        });
    }
    let port_of = |node: usize| -> Option<MepPort> { ends_at.get(&nodes[node].junction).and_then(|v| v.first()).map(|i| data.endpoints[*i].port) };
    let ends: Vec<EndReport> = results.ends.iter().map(|e| EndReport {
        port: port_of(e.node), needed_at_source: e.needed_at_source, excess: e.excess,
        flow: results.branches[e.node].flow,
        pressure: sprinklers.as_ref().and_then(|v| v.iter().find(|x| x.node == e.node)).map(|x| x.pressure),
    }).collect();
    let reached: BTreeSet<usize> = nodes.iter().map(|n| n.junction).collect();
    let unreached: Vec<MepPort> = data.endpoints.iter().enumerate().filter(|(_, e)| e.role == EndRole::Terminal && e.demand != EndDemand::None)
        .filter(|(i, _)| { let k = ids.map.get(&Key::Port(data.endpoints[*i].port)).copied(); k.is_some_and(|k| !reached.contains(&ids.uf.find(k))) && s.system == system_of_endpoint(doc, data.endpoints[*i].port) })
        .map(|(_, e)| e.port).collect();
    let mut critical_runs: Vec<ElementId> = reports.iter().filter(|r| r.critical).map(|r| r.run).collect();
    critical_runs.sort();
    Ok(SystemReport { system: s.system, runs: reports, ends, source_pressure: results.source_pressure, total_flow: results.branches[0].flow, unreached, critical_runs })
}

fn system_of_endpoint(doc: &Document, p: MepPort) -> System {
    match p.node { MepNode::Run(id) => system_of_run(doc, id), MepNode::Fitting(f) => doc.mep().fitting(f).map_or(System::Other, |f| f.system) }
}

/// Every duct or pipe joined to `start` through links, fittings and taps (the line the designer would call one system).
pub fn connected_runs(doc: &Document, start: ElementId) -> Vec<ElementId> {
    let data = doc.mep();
    let mut seen: BTreeSet<MepNode> = BTreeSet::new();
    let mut todo = vec![MepNode::Run(start)];
    while let Some(n) = todo.pop() {
        if !seen.insert(n) { continue; }
        let mut next: Vec<MepNode> = Vec::new();
        for (a, b) in &data.links {
            if a.node == n { next.push(b.node); } else if b.node == n { next.push(a.node); }
        }
        for t in &data.taps {
            if n == MepNode::Run(t.run) { next.push(MepNode::Fitting(t.fitting)); }
            if n == MepNode::Fitting(t.fitting) { next.push(MepNode::Run(t.run)); }
        }
        todo.extend(next);
    }
    seen.into_iter().filter_map(|n| if let MepNode::Run(id) = n { Some(id) } else { None }).collect()
}

/// Gives these runs a system, in one transaction. A run whose kind does not suit the system (a duct on a water system)
/// refuses the whole change.
pub fn assign_system(doc: &mut Document, runs: &[ElementId], system: System) -> Result<(), EditError> {
    let mut data = doc.mep().clone();
    for &r in runs {
        data.run_systems.retain(|(id, _)| *id != r);
        data.run_systems.push((r, system));
    }
    doc.transact(doc.revision(), &[Command::SetMep(Box::new(data))])?;
    Ok(())
}

/// Sets the computed sizes on the drawing in one transaction; returns how many runs changed.
pub fn apply_sizes(doc: &mut Document, report: &SystemReport) -> Result<usize, EditError> {
    let ops = report.resize_commands(doc);
    if ops.is_empty() { return Ok(0); }
    let n = ops.len();
    doc.transact(doc.revision(), &ops)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mep::{MepEndpoint, MepFitting, FITTING_BASE};
    use crate::MepData;
    use openbim_family::mep::Domain;
    use openbim_place::penetrations::RunSize;
    use openbim_route::{Fitting, FittingKind};

    const LEVEL: ElementId = ElementId(10);

    fn catalogue() -> Vec<calc::Size> {
        // test data only: inside diameters, not any standard's
        [("A", 0.0217), ("B", 0.0273), ("C", 0.0357), ("D", 0.0419), ("E", 0.0539), ("F", 0.0685), ("G", 0.0825), ("H", 0.1071)].iter().map(|(l, d)| calc::Size::round(l, *d)).collect()
    }

    fn settings(system: System) -> ComputeSettings {
        ComputeSettings {
            system,
            calc: calc::Settings { fluid: calc::Fluid::WATER, roughness: 0.045e-3, criteria: calc::Criteria { max_velocity: Some(1.5), max_friction: None }, catalogue: catalogue(), count_static: false, loading: None, hazen_williams: None },
            losses: FittingLosses { elbow: 0.3, transition: 0.1, tee: 0.5, tap: 0.5 },
            keep_sizes: false, scope: None,
        }
    }

    fn pipe(d: &mut Document, a: [f64; 2], b: [f64; 2], dia: f64) -> ElementId {
        let id = d.next_id();
        d.transact(d.revision(), &[
            Command::Create { id, category: Category::Pipe, name: "Pipe".into(), width_m: dia, height_m: dia, elevation_m: 2.7 },
            Command::SetSlope { id, slope_ratio: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: a, end: b, level: LEVEL } },
        ]).unwrap();
        id
    }

    fn fitting(id: u64, kind: FittingKind, legs: usize) -> MepFitting {
        MepFitting {
            id, domain: Domain::Pipe, system: System::ChilledWaterSupply,
            fitting: Fitting { type_name: "Test".into(), part_code: String::new(), kind, at: [0.0, 0.0, 2.7], ports: vec![[1.0, 0.0, 0.0]; legs], port_sizes: vec![RunSize::Round { diameter: 0.05 }; legs], radius: 0.0, trim: vec![0.0; legs] },
        }
    }

    fn run_port(id: ElementId, port: usize) -> MepPort { MepPort { node: MepNode::Run(id), port } }
    fn fit_port(id: u64, port: usize) -> MepPort { MepPort { node: MepNode::Fitting(id), port } }
    fn end(port: MepPort, role: EndRole, demand: EndDemand, rp: f64) -> MepEndpoint { MepEndpoint { port, role, demand, required_pressure_pa: rp } }

    fn darcy(q: f64, d: f64, len: f64, k: f64) -> f64 {
        let v = q / (std::f64::consts::PI * d * d / 4.0);
        let re = 998.0 * v * d / 1.0e-3;
        calc::colebrook(re, 0.045e-3 / d) * len / d * 998.0 * v * v / 2.0 + k * 998.0 * v * v / 2.0
    }

    /// pipe 1 (4 m) -> elbow -> pipe 2 (3 m), a coil of 50 kW over 5 K at the end
    fn elbow_line() -> (Document, ElementId, ElementId) { elbow_line_of(0.05) }

    fn elbow_line_of(dia: f64) -> (Document, ElementId, ElementId) {
        let mut d = Document::spatial_sample();
        let (p1, p2) = (pipe(&mut d, [1.0, 1.0], [5.0, 1.0], dia), pipe(&mut d, [5.0, 1.0], [5.0, 4.0], dia));
        let data = MepData {
            fittings: vec![fitting(FITTING_BASE, FittingKind::Elbow { angle: 90.0 }, 2)],
            links: vec![(run_port(p1, 1), fit_port(FITTING_BASE, 0)), (fit_port(FITTING_BASE, 1), run_port(p2, 0))],
            endpoints: vec![end(run_port(p1, 0), EndRole::Source, EndDemand::None, 0.0), end(run_port(p2, 1), EndRole::Terminal, EndDemand::Power { watts: 50_000.0, delta_t: 5.0 }, 20_000.0)],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        (d, p1, p2)
    }

    #[test]
    fn two_loops_of_one_type_compute_apart_when_each_is_a_named_system() {
        let mut d = Document::spatial_sample();
        let spec = crate::FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        let (u1, u2) = (d.place_family(LEVEL, [1.0, 1.0], &spec).unwrap(), d.place_family(LEVEL, [21.0, 1.0], &spec).unwrap());
        let (a1, a2) = (pipe(&mut d, [1.0, 1.0], [5.0, 1.0], 0.05), pipe(&mut d, [5.0, 1.0], [5.0, 4.0], 0.05));
        let (b1, b2) = (pipe(&mut d, [21.0, 1.0], [25.0, 1.0], 0.05), pipe(&mut d, [25.0, 1.0], [25.0, 4.0], 0.05));
        let data = MepData {
            links: vec![(run_port(a1, 1), run_port(a2, 0)), (run_port(b1, 1), run_port(b2, 0))],
            endpoints: vec![
                end(run_port(a1, 0), EndRole::Source, EndDemand::None, 0.0), end(run_port(a2, 1), EndRole::Terminal, EndDemand::Power { watts: 50_000.0, delta_t: 5.0 }, 20_000.0),
                end(run_port(b1, 0), EndRole::Source, EndDemand::None, 0.0), end(run_port(b2, 1), EndRole::Terminal, EndDemand::Power { watts: 20_000.0, delta_t: 5.0 }, 20_000.0),
            ],
            equip_links: vec![
                crate::EquipLink { port: run_port(a1, 0), element: u1, connector: "Supply".into() },
                crate::EquipLink { port: run_port(b1, 0), element: u2, connector: "Supply".into() },
            ],
            systems: vec![
                crate::MepSystemDef { name: "CHW North".into(), system: System::ChilledWaterSupply, members: vec![u1], base: None },
                crate::MepSystemDef { name: "CHW South".into(), system: System::ChilledWaterSupply, members: vec![u2], base: None },
            ],
            run_systems: [a1, a2, b1, b2].iter().map(|r| (*r, System::ChilledWaterSupply)).collect(),
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        // together they are two sources, as before
        assert_eq!(compute_system(&d, &settings(System::ChilledWaterSupply)).unwrap_err(), ComputeError::MoreThanOneSource);
        // each named system is its own computation, with its own load
        let north = compute_system(&d, &ComputeSettings { scope: Some("CHW North".into()), ..settings(System::ChilledWaterSupply) }).unwrap();
        let south = compute_system(&d, &ComputeSettings { scope: Some("CHW South".into()), ..settings(System::ChilledWaterSupply) }).unwrap();
        assert!((north.total_flow - 50_000.0 / (998.0 * 4186.0 * 5.0)).abs() < 1e-12);
        assert!((south.total_flow - 20_000.0 / (998.0 * 4186.0 * 5.0)).abs() < 1e-12);
        assert!(north.runs.iter().all(|r| [a1, a2].contains(&r.run)) && south.runs.iter().all(|r| [b1, b2].contains(&r.run)));
        assert_eq!(compute_system(&d, &ComputeSettings { scope: Some("Missing".into()), ..settings(System::ChilledWaterSupply) }).unwrap_err(), ComputeError::UnknownSystem);
    }

    #[test]
    fn a_drawn_pipe_line_is_sized_and_its_losses_are_darcy_weisbach_plus_the_elbow() {
        let (d, p1, p2) = elbow_line();
        let r = compute_system(&d, &settings(System::ChilledWaterSupply)).unwrap();
        let q = 50_000.0 / (998.0 * 4186.0 * 5.0);
        // the smallest catalogue size under 1.5 m/s
        let dia = catalogue().iter().filter_map(|z| match z.shape { calc::Shape::Round { diameter } => Some(diameter), _ => None }).find(|d| q / (std::f64::consts::PI * d * d / 4.0) <= 1.5).unwrap();
        assert!((dia - 0.0539).abs() < 1e-12);
        let (a, b) = (r.runs.iter().find(|x| x.run == p1).unwrap(), r.runs.iter().find(|x| x.run == p2).unwrap());
        assert!((a.flow - q).abs() < 1e-12 && (b.flow - q).abs() < 1e-12);
        assert_eq!((a.size.label.as_str(), b.size.label.as_str()), ("E", "E"));
        // the elbow's coefficient is on the run that leaves it; lengths are 4 m and 3 m
        assert!((a.dp_total - darcy(q, dia, 4.0, 0.0)).abs() < 1e-6, "{} {}", a.dp_total, darcy(q, dia, 4.0, 0.0));
        assert!((b.dp_total - darcy(q, dia, 3.0, 0.3)).abs() < 1e-6);
        assert!((r.source_pressure - (a.dp_total + b.dp_total + 20_000.0)).abs() < 1e-6);
        assert!((r.total_flow - q).abs() < 1e-12 && r.ends.len() == 1 && r.ends[0].excess == 0.0 && r.unreached.is_empty());
        assert_eq!(r.critical_runs, vec![p1, p2]);
        assert_eq!(r.ends[0].port, Some(run_port(p2, 1)));
    }

    #[test]
    fn computed_sizes_are_written_in_one_step_and_then_the_drawn_sizes_meet_the_limits() {
        let (mut d, p1, p2) = elbow_line();
        let s = settings(System::ChilledWaterSupply);
        let r = compute_system(&d, &s).unwrap();
        let rev = d.revision();
        assert_eq!(apply_sizes(&mut d, &r).unwrap(), 2);
        assert_eq!(d.revision(), rev + 1, "one transaction");
        assert!((d.get(p1).unwrap().width_m - 0.0539).abs() < 1e-12 && (d.get(p2).unwrap().height_m - 0.0539).abs() < 1e-12);
        let keep = ComputeSettings { keep_sizes: true, ..s.clone() };
        let again = compute_system(&d, &keep).unwrap();
        assert!(again.runs.iter().all(|x| !x.exceeds_limits) && (again.source_pressure - r.source_pressure).abs() < 1e-6);
        assert_eq!(apply_sizes(&mut d, &again).unwrap(), 0, "nothing left to change");
        // 40 mm drawn: 2.39 l/s is 1.9 m/s, over the 1.5 limit, so a kept size is reported; 50 mm (1.2 m/s) is not
        let (d0, ..) = elbow_line_of(0.04);
        assert!(compute_system(&d0, &keep).unwrap().runs.iter().all(|x| x.exceeds_limits));
        let (d1, ..) = elbow_line();
        assert!(compute_system(&d1, &keep).unwrap().runs.iter().all(|x| !x.exceeds_limits));
        assert!(d.undo().unwrap());
        assert!((d.get(p1).unwrap().width_m - 0.05).abs() < 1e-12);
    }

    /// pipe 1 (4 m) to a tee; pipe 2 (4 m) and pipe 3 (5 m) leave it; coils of 30 kW and 20 kW
    fn tee_network() -> (Document, [ElementId; 3]) {
        let mut d = Document::spatial_sample();
        let (p1, p2, p3) = (pipe(&mut d, [0.0, 0.0], [4.0, 0.0], 0.05), pipe(&mut d, [4.0, 0.0], [8.0, 0.0], 0.05), pipe(&mut d, [4.0, 0.0], [4.0, 5.0], 0.05));
        let f = FITTING_BASE;
        let data = MepData {
            fittings: vec![fitting(f, FittingKind::Tee { branch_angle: 90.0 }, 3)],
            links: vec![(run_port(p1, 1), fit_port(f, 0)), (fit_port(f, 1), run_port(p2, 0)), (fit_port(f, 2), run_port(p3, 0))],
            endpoints: vec![
                end(run_port(p1, 0), EndRole::Source, EndDemand::None, 0.0),
                end(run_port(p2, 1), EndRole::Terminal, EndDemand::Power { watts: 30_000.0, delta_t: 5.0 }, 20_000.0),
                end(run_port(p3, 1), EndRole::Terminal, EndDemand::Power { watts: 20_000.0, delta_t: 5.0 }, 20_000.0),
            ],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        (d, [p1, p2, p3])
    }

    #[test]
    fn a_branch_is_summed_into_its_main_and_the_most_resistant_path_sets_the_duty() {
        let (d, [p1, p2, p3]) = tee_network();
        let r = compute_system(&d, &settings(System::ChilledWaterSupply)).unwrap();
        let by = |id: ElementId| r.runs.iter().find(|x| x.run == id).unwrap();
        let k = 998.0 * 4186.0 * 5.0;
        assert!((by(p1).flow - 50_000.0 / k).abs() < 1e-12 && (by(p2).flow - 30_000.0 / k).abs() < 1e-12 && (by(p3).flow - 20_000.0 / k).abs() < 1e-12);
        // each of the two ends needs: main + branch + 20 kPa
        let need = |branch: ElementId| by(p1).dp_total + by(branch).dp_total + 20_000.0;
        let (n2, n3) = (need(p2), need(p3));
        let (critical, other) = if n2 >= n3 { (p2, p3) } else { (p3, p2) };
        assert!((r.source_pressure - n2.max(n3)).abs() < 1e-6);
        assert_eq!(r.critical_runs, { let mut v = vec![p1, critical]; v.sort(); v });
        assert!(!by(other).critical);
        let spare = r.ends.iter().map(|e| e.excess).fold(0.0, f64::max);
        assert!((spare - (n2 - n3).abs()).abs() < 1e-6 && r.ends.iter().any(|e| e.excess == 0.0));
        // the tee's coefficient is on both runs leaving it
        let dia = |id: ElementId| match by(id).size.shape { calc::Shape::Round { diameter } => diameter, _ => 0.0 };
        assert!((by(p2).dp_total - darcy(by(p2).flow, dia(p2), 4.0, 0.5)).abs() < 1e-6);
    }

    #[test]
    fn a_tap_cuts_its_run_in_two_and_the_run_keeps_the_size_its_larger_part_needs() {
        let mut d = Document::spatial_sample();
        let (main, branch) = (pipe(&mut d, [0.0, 0.0], [10.0, 0.0], 0.05), pipe(&mut d, [4.0, 0.0], [4.0, 6.0], 0.05));
        let f = FITTING_BASE;
        let data = MepData {
            fittings: vec![fitting(f, FittingKind::Tap { branch_angle: 90.0 }, 2)],
            links: vec![(fit_port(f, 0), run_port(branch, 0))],
            taps: vec![crate::MepTap { fitting: f, run: main, distance: 4.0 }],
            endpoints: vec![
                end(run_port(main, 0), EndRole::Source, EndDemand::None, 0.0),
                end(run_port(main, 1), EndRole::Terminal, EndDemand::Power { watts: 10_000.0, delta_t: 5.0 }, 10_000.0),
                end(run_port(branch, 1), EndRole::Terminal, EndDemand::Power { watts: 60_000.0, delta_t: 5.0 }, 10_000.0),
            ],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        let r = compute_system(&d, &settings(System::ChilledWaterSupply)).unwrap();
        let m = r.runs.iter().find(|x| x.run == main).unwrap();
        let k = 998.0 * 4186.0 * 5.0;
        // the first 4 m carry 70 kW, the last 6 m only 10 kW; one size for the run, the one 70 kW needs
        assert!((m.flow - 70_000.0 / k).abs() < 1e-12, "{}", m.flow);
        let q = 70_000.0 / k;
        let need = catalogue().iter().filter_map(|z| match z.shape { calc::Shape::Round { diameter } => Some(diameter), _ => None }).find(|d| q / (std::f64::consts::PI * d * d / 4.0) <= 1.5).unwrap();
        assert_eq!(m.size.shape, calc::Shape::Round { diameter: need });
        assert!((m.length - 10.0).abs() < 1e-9);
        // 4 m at 70 kW + 6 m at 10 kW, all in the one size, the tap's coefficient on the part and branch leaving it
        let expect = darcy(q, need, 4.0, 0.0) + darcy(10_000.0 / k, need, 6.0, 0.5);
        assert!((m.dp_total - expect).abs() < 1e-6, "{} {}", m.dp_total, expect);
    }

    #[test]
    fn what_cannot_be_computed_is_named() {
        let (mut d, p1, p2) = elbow_line();
        let s = settings(System::ChilledWaterSupply);
        // another system finds nothing
        assert_eq!(compute_system(&d, &settings(System::SupplyAir)).unwrap_err(), ComputeError::NoSource);
        // two sources
        let mut data = d.mep().clone();
        data.endpoints[1] = end(run_port(p2, 1), EndRole::Source, EndDemand::None, 0.0);
        d.transact(d.revision(), &[Command::SetMep(Box::new(data.clone()))]).unwrap();
        assert_eq!(compute_system(&d, &s).unwrap_err(), ComputeError::MoreThanOneSource);
        // no terminal
        data.endpoints.pop();
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        assert_eq!(compute_system(&d, &s).unwrap_err(), ComputeError::NoTerminal);
        // a catalogue too small for the flow
        let (d2, ..) = elbow_line();
        let small = ComputeSettings { calc: calc::Settings { catalogue: catalogue().into_iter().take(2).collect(), ..s.calc.clone() }, ..s.clone() };
        assert!(matches!(compute_system(&d2, &small).unwrap_err(), ComputeError::Calc(calc::TreeError::NoSize(_))));
        let _ = (p1, p2);
    }

    #[test]
    fn a_run_that_closes_a_loop_is_refused() {
        let mut d = Document::spatial_sample();
        let p = pipe(&mut d, [0.0, 0.0], [4.0, 0.0], 0.05);
        let q = pipe(&mut d, [4.0, 0.0], [8.0, 0.0], 0.05);
        let (f, g) = (FITTING_BASE, FITTING_BASE + 1);
        // p leaves leg 0 of a tee and comes back to its leg 1: a ring; the source is on leg 2, a terminal beyond the tee g
        let data = MepData {
            fittings: vec![fitting(f, FittingKind::Tee { branch_angle: 90.0 }, 3), fitting(g, FittingKind::Tee { branch_angle: 90.0 }, 3)],
            links: vec![(run_port(p, 0), fit_port(f, 0)), (run_port(p, 1), fit_port(f, 1)), (fit_port(f, 2), fit_port(g, 0)), (fit_port(g, 1), run_port(q, 0))],
            endpoints: vec![end(fit_port(g, 2), EndRole::Source, EndDemand::None, 0.0), end(run_port(q, 1), EndRole::Terminal, EndDemand::Flow(0.001), 0.0)],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        assert_eq!(compute_system(&d, &settings(System::ChilledWaterSupply)).unwrap_err(), ComputeError::Loop(p));
    }

    #[test]
    fn plumbing_loading_units_and_sprinklers_are_computed_the_same_way() {
        let mut d = Document::spatial_sample();
        let (p1, p2) = (pipe(&mut d, [0.0, 0.0], [6.0, 0.0], 0.05), pipe(&mut d, [6.0, 0.0], [6.0, 4.0], 0.05));
        let link = vec![(run_port(p1, 1), run_port(p2, 0))];
        let mut data = MepData { links: link, run_systems: vec![(p1, System::DomesticCold), (p2, System::DomesticCold)], ..Default::default() };
        data.endpoints = vec![end(run_port(p1, 0), EndRole::Source, EndDemand::None, 0.0), end(run_port(p2, 1), EndRole::Terminal, EndDemand::LoadingUnits(20.0), 100_000.0)];
        d.transact(d.revision(), &[Command::SetMep(Box::new(data.clone()))]).unwrap();
        let curve = calc::LoadingCurve::from_csv("lu,l/s\n1,0.1\n10,0.398107\n100,1.584893").unwrap();
        let mut s = settings(System::DomesticCold);
        s.calc.loading = Some(curve.clone());
        let r = compute_system(&d, &s).unwrap();
        assert!((r.total_flow - curve.flow(20.0).unwrap()).abs() < 1e-12);
        // without a curve it is refused and says so
        assert_eq!(compute_system(&d, &settings(System::DomesticCold)).unwrap_err(), ComputeError::Calc(calc::TreeError::BadSettings("no loading curve for loading units")));

        // a sprinkler at the end of the same line, on the fire system
        data.run_systems = vec![(p1, System::FireProtection), (p2, System::FireProtection)];
        data.endpoints[1] = end(run_port(p2, 1), EndRole::Terminal, EndDemand::Sprinkler { k_l_min_bar: 80.0, min_flow_l_min: 48.0 }, 50_000.0);
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        let mut f = settings(System::FireProtection);
        f.calc.hazen_williams = Some(120.0);
        f.calc.criteria.max_velocity = Some(6.0);
        let r = compute_system(&d, &f).unwrap();
        let end = &r.ends[0];
        let p = end.pressure.unwrap();
        assert!((end.flow - calc::k_factor(80.0) * p.sqrt()).abs() < 1e-12, "q = K sqrt(p)");
        assert!((p - 50_000.0).abs() < 1.0, "the only sprinkler sits at its minimum: {p}");
        assert!(r.source_pressure > 50_000.0);
    }

    #[test]
    fn a_drawn_duct_is_sized_by_friction_and_velocity_with_the_equivalent_diameter() {
        let mut d = Document::spatial_sample();
        let id = d.next_id();
        d.transact(d.revision(), &[
            Command::Create { id, category: Category::Duct, name: "Duct".into(), width_m: 0.3, height_m: 0.2, elevation_m: 2.7 },
            Command::SetSlope { id, slope_ratio: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: [0.0, 0.0], end: [20.0, 0.0], level: LEVEL } },
            Command::SetMep(Box::new(MepData {
                run_systems: vec![(id, System::SupplyAir)],
                endpoints: vec![end(run_port(id, 0), EndRole::Source, EndDemand::None, 0.0), end(run_port(id, 1), EndRole::Terminal, EndDemand::Power { watts: 12_000.0, delta_t: 10.0 }, 30.0)],
                ..Default::default()
            })),
        ]).unwrap();
        let cat = vec![calc::Size::rect("300x200", 0.3, 0.2), calc::Size::rect("500x300", 0.5, 0.3), calc::Size::rect("800x400", 0.8, 0.4), calc::Size::rect("1000x500", 1.0, 0.5)];
        let s = ComputeSettings {
            system: System::SupplyAir,
            calc: calc::Settings { fluid: calc::Fluid::AIR, roughness: 0.09e-3, criteria: calc::Criteria { max_velocity: Some(6.0), max_friction: Some(1.0) }, catalogue: cat, count_static: false, loading: None, hazen_williams: None },
            losses: FittingLosses::default(), keep_sizes: false, scope: None,
        };
        let r = compute_system(&d, &s).unwrap();
        // 12 kW over 10 K of air: 0.99 m3/s
        let q = 12_000.0 / (1.2 * 1006.0 * 10.0);
        let run = &r.runs[0];
        assert!((run.flow - q).abs() < 1e-12 && run.velocity <= 6.0 && run.friction_per_metre <= 1.0);
        assert_eq!(run.size.label, "800x400", "500x300 runs at 6.6 m/s, over the limit; 300x200 is too small for {q} m3/s; {} Pa/m {} m/s", run.friction_per_metre, run.velocity);
        assert!((r.source_pressure - (run.dp_total + 30.0)).abs() < 1e-9);
        let cmds = r.resize_commands(&d);
        assert_eq!(cmds, vec![Command::Resize { id, width_m: 0.8, height_m: 0.4 }]);
    }

    #[test]
    fn a_line_is_found_through_its_fittings_and_taps_and_given_a_system_in_one_step() {
        let (mut d, [p1, p2, p3]) = tee_network();
        assert_eq!(connected_runs(&d, p3), vec![p1, p2, p3]);
        let lone = pipe(&mut d, [20.0, 20.0], [24.0, 20.0], 0.05);
        assert_eq!(connected_runs(&d, lone), vec![lone]);
        let rev = d.revision();
        let line = connected_runs(&d, p1);
        assign_system(&mut d, &line, System::HeatingWaterSupply).unwrap();
        assert_eq!(d.revision(), rev + 1);
        assert!([p1, p2, p3].iter().all(|r| system_of_run(&d, *r) == System::HeatingWaterSupply) && system_of_run(&d, lone) == System::Other);
        // a duct system on a pipe is refused and nothing changes
        let rev = d.revision();
        assert_eq!(assign_system(&mut d, &[p1], System::SupplyAir), Err(EditError::InvalidMep));
        assert_eq!(d.revision(), rev);
        assert!(d.undo().unwrap());
        assert_eq!(system_of_run(&d, p1), System::ChilledWaterSupply);
    }
}
