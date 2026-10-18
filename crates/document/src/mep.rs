//! The connections of the ducts and pipes: the fittings between them and which port of which run or fitting is joined
//! to which. A duct or pipe is an element; a fitting is not (it has several legs), so the model keeps the fittings and
//! the links here, replaced as a whole by one command, undone with the rest of the transaction.
//!
//! Ports: a run has two (0 its start, 1 its end), a fitting one for each of its legs.
use crate::{Category, EditError, Element, ElementId, Geometry};
use openbim_family::mep::{Domain, System};
use openbim_route::Fitting;
use std::collections::BTreeMap;

/// Fitting numbers start here, far above any element number, so that a fitting never has the number of an element.
pub const FITTING_BASE: u64 = 1 << 40;

#[derive(Clone, Debug, PartialEq)]
pub struct MepFitting {
    /// stable for the life of the model; at least `FITTING_BASE`
    pub id: u64,
    pub fitting: Fitting,
    pub domain: Domain,
    pub system: System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MepNode {
    /// a duct or pipe element
    Run(ElementId),
    /// a fitting, by its number
    Fitting(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MepPort {
    pub node: MepNode,
    pub port: usize,
}

/// A tap fitting that sits on a run, `distance` along it from its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MepTap {
    pub fitting: u64,
    pub run: ElementId,
    pub distance: f64,
}

/// What an open end of the network is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndRole {
    /// where the water, air or supply comes from: a pump, a fan, a riser, a main board; the root of the computation
    Source,
    /// where it is used: a coil, a diffuser, a fixture, a sprinkler
    Terminal,
}

/// What a terminal takes. Units are the model's: metres, m3/s, watts, kelvin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EndDemand {
    None,
    /// a flow, m3/s
    Flow(f64),
    /// a cooling or heating power across a temperature difference
    Power { watts: f64, delta_t: f64 },
    /// loading units of a fixture (plumbing)
    LoadingUnits(f64),
    /// a sprinkler: its K-factor as printed (l/min per root bar) and the flow the density asks for, l/min
    Sprinkler { k_l_min_bar: f64, min_flow_l_min: f64 },
}

/// A source or a terminal at an open port of a run or fitting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MepEndpoint {
    pub port: MepPort,
    pub role: EndRole,
    pub demand: EndDemand,
    /// the pressure it needs, Pa (a coil's drop, a terminal's, a sprinkler's minimum)
    pub required_pressure_pa: f64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct MepData {
    pub fittings: Vec<MepFitting>,
    pub links: Vec<(MepPort, MepPort)>,
    pub taps: Vec<MepTap>,
    /// the system each duct or pipe belongs to, where the designer has said so (otherwise it takes its fittings')
    pub run_systems: Vec<(ElementId, System)>,
    pub endpoints: Vec<MepEndpoint>,
    /// the electrical network: a source, boards and loads
    pub elec_nodes: Vec<crate::elec::ElecNode>,
    /// the cables between them
    pub elec_cables: Vec<crate::elec::ElecCable>,
}

impl MepData {
    pub fn is_empty(&self) -> bool { self.fittings.is_empty() && self.links.is_empty() && self.taps.is_empty() && self.run_systems.is_empty() && self.endpoints.is_empty() && self.elec_nodes.is_empty() && self.elec_cables.is_empty() }

    /// The system the designer gave a run, if any.
    pub fn run_system(&self, id: ElementId) -> Option<System> { self.run_systems.iter().find(|(r, _)| *r == id).map(|(_, s)| *s) }

    /// The source or terminal at a port, if any.
    pub fn endpoint_at(&self, p: MepPort) -> Option<&MepEndpoint> { self.endpoints.iter().find(|e| e.port == p) }

    /// The first fitting number that no fitting has.
    pub fn next_fitting_id(&self) -> u64 { self.fittings.iter().map(|f| f.id + 1).max().unwrap_or(FITTING_BASE).max(FITTING_BASE) }

    pub fn fitting(&self, id: u64) -> Option<&MepFitting> { self.fittings.iter().find(|f| f.id == id) }

    /// What a port is joined to.
    pub fn partner(&self, p: MepPort) -> Option<MepPort> {
        self.links.iter().find_map(|(a, b)| if *a == p { Some(*b) } else if *b == p { Some(*a) } else { None })
    }

    /// The same data without the links and taps that point at runs that are gone.
    pub(crate) fn pruned(&self, elements: &BTreeMap<ElementId, Element>) -> MepData {
        let alive = |n: &MepNode| match n { MepNode::Run(id) => is_run(elements.get(id)), MepNode::Fitting(_) => true };
        MepData {
            fittings: self.fittings.clone(),
            links: self.links.iter().filter(|(a, b)| alive(&a.node) && alive(&b.node)).copied().collect(),
            taps: self.taps.iter().filter(|t| is_run(elements.get(&t.run))).copied().collect(),
            run_systems: self.run_systems.iter().filter(|(r, _)| is_run(elements.get(r))).copied().collect(),
            endpoints: self.endpoints.iter().filter(|e| alive(&e.port.node)).copied().collect(),
            elec_nodes: self.elec_nodes.iter().filter(|n| elements.get(&n.level).is_some_and(|l| l.category == Category::Level)).cloned().map(|mut n| { if n.element.is_some_and(|e| elements.get(&e).is_none_or(|x| x.category != Category::Family)) { n.element = None; n.connector.clear(); } n }).collect(),
            elec_cables: { let keep: std::collections::BTreeSet<u64> = self.elec_nodes.iter().filter(|n| elements.get(&n.level).is_some_and(|l| l.category == Category::Level)).map(|n| n.id).collect(); self.elec_cables.iter().filter(|c| keep.contains(&c.from) && keep.contains(&c.to)).cloned().collect() },
        }
    }

    /// Every link joins two existing ports once, nothing is joined to itself, and the fittings are sound.
    pub(crate) fn check(&self, elements: &BTreeMap<ElementId, Element>) -> Result<(), EditError> {
        let mut ids = std::collections::BTreeSet::new();
        for f in &self.fittings {
            let n = f.fitting.ports.len();
            if f.id < FITTING_BASE || !ids.insert(f.id) || !(if matches!(f.fitting.kind, openbim_route::FittingKind::Tap { .. }) { 1..=16 } else { 2..=16 }).contains(&n) || f.fitting.port_sizes.len() != n || f.fitting.trim.len() != n || !f.system.domain_ok(f.domain) {
                return Err(EditError::InvalidMep);
            }
        }
        let ports = |n: &MepNode| -> Option<usize> {
            match n {
                MepNode::Run(id) => is_run(elements.get(id)).then_some(2),
                MepNode::Fitting(id) => self.fitting(*id).map(|f| f.fitting.ports.len()),
            }
        };
        let mut used = std::collections::BTreeSet::new();
        for (a, b) in &self.links {
            if a.node == b.node { return Err(EditError::InvalidMep); }
            for p in [a, b] {
                if ports(&p.node).is_none_or(|n| p.port >= n) || !used.insert(*p) { return Err(EditError::InvalidMep); }
            }
        }
        let mut tapped = std::collections::BTreeSet::new();
        for t in &self.taps {
            let tap_fitting = self.fitting(t.fitting).is_some_and(|f| matches!(f.fitting.kind, openbim_route::FittingKind::Tap { .. }));
            if !tap_fitting || !is_run(elements.get(&t.run)) || !t.distance.is_finite() || t.distance < 0.0 || !tapped.insert(t.fitting) { return Err(EditError::InvalidMep); }
        }
        let mut systemed = std::collections::BTreeSet::new();
        for (id, sys) in &self.run_systems {
            let domain = match elements.get(id).map(|e| e.category) { Some(Category::Duct) => Domain::Duct, Some(Category::Pipe) => Domain::Pipe, _ => return Err(EditError::InvalidMep) };
            if !is_run(elements.get(id)) || !sys.domain_ok(domain) || !systemed.insert(*id) { return Err(EditError::InvalidMep); }
        }
        let mut ended = std::collections::BTreeSet::new();
        for e in &self.endpoints {
            let open = ports(&e.port.node).is_some_and(|n| e.port.port < n) && self.partner(e.port).is_none();
            let pos = |v: f64| v.is_finite() && v >= 0.0;
            let demand_ok = match e.demand {
                EndDemand::None => true,
                EndDemand::Flow(q) => pos(q),
                EndDemand::Power { watts, delta_t } => pos(watts) && delta_t.is_finite() && delta_t > 0.0,
                EndDemand::LoadingUnits(u) => pos(u),
                EndDemand::Sprinkler { k_l_min_bar, min_flow_l_min } => k_l_min_bar.is_finite() && k_l_min_bar > 0.0 && pos(min_flow_l_min),
            };
            if !open || !pos(e.required_pressure_pa) || !demand_ok || (e.role == EndRole::Source && e.demand != EndDemand::None) || !ended.insert(e.port) {
                return Err(EditError::InvalidMep);
            }
        }
        if self.elec_nodes.iter().any(|n| n.element.is_some_and(|e| elements.get(&e).is_none_or(|x| x.category != Category::Family))) { return Err(EditError::InvalidMep); }
        let is_level = |id: ElementId| elements.get(&id).is_some_and(|l| l.category == Category::Level);
        if !crate::elec::valid_nodes(&self.elec_nodes, &self.elec_cables, &is_level) { return Err(EditError::InvalidMep); }
        Ok(())
    }

    /// The first electrical node number that no node has.
    pub fn next_node_id(&self) -> u64 { self.elec_nodes.iter().map(|n| n.id + 1).max().unwrap_or(crate::elec::NODE_BASE).max(crate::elec::NODE_BASE) }
}

fn is_run(e: Option<&Element>) -> bool {
    e.is_some_and(|e| matches!(e.category, Category::Duct | Category::Pipe) && matches!(e.geometry, Some(Geometry::Linear { .. })))
}

impl crate::Document {
    /// A drawn duct or pipe as the routing code sees it (3D ends, section, system).
    pub fn mep_run(&self, id: ElementId) -> Option<openbim_place::penetrations::Run> {
        use openbim_place::penetrations::{Run, RunSize};
        let el = self.get(id).filter(|e| is_run(Some(e)))?;
        let Some(Geometry::Linear { start, end, level }) = el.geometry else { return None };
        let plan = (end[0] - start[0]).hypot(end[1] - start[1]);
        let za = self.get(level).map_or(0.0, |l| l.elevation_m) + el.elevation_m;
        let zb = za + el.slope_ratio.unwrap_or(0.0) * plan;
        let system = crate::compute::system_of_run(self, id);
        let (domain, size) = if el.category == Category::Pipe { (Domain::Pipe, RunSize::Round { diameter: el.width_m }) } else { (Domain::Duct, RunSize::Rect { width: el.width_m, height: el.height_m, roll: 0.0 }) };
        Some(Run { label: el.name.clone(), domain, system, a: [start[0], start[1], za], b: [end[0], end[1], zb], size, margin: 0.0 })
    }

    /// Joins a branch to a duct or pipe already in the model, as a tee or a tap, in one transaction. A tee cuts the run
    /// in two: the first half stays the same element, the second half is a new element that takes the end of the run
    /// with its links, end point and taps. A tap leaves the run whole. The branch's own runs and fittings are new.
    /// Gives the new elements: the second half first (tee only), then the branch runs.
    pub fn place_mep_branch(&mut self, level: ElementId, main: ElementId, br: &openbim_route::Branch) -> Result<Vec<ElementId>, EditError> {
        use crate::Command;
        use openbim_place::penetrations::RunSize;
        use openbim_route::graph::{Network, NodeRef};
        use openbim_route::FittingKind;
        let z0 = self.get(level).filter(|l| l.category == Category::Level).ok_or(EditError::NotFound(level))?.elevation_m;
        let run = self.mep_run(main).ok_or(EditError::NotFound(main))?;
        let main_level = match self.get(main).and_then(|e| e.geometry) { Some(Geometry::Linear { level, .. }) => level, _ => return Err(EditError::InvalidMep) };
        if main_level != level { return Err(EditError::InvalidMep); }
        let mut net = Network::new(1);
        net.add_route(&openbim_route::Route { points: vec![], runs: vec![run.clone()], fittings: vec![] }).map_err(|_| EditError::InvalidMep)?;
        net.add_branch(0, br).map_err(|_| EditError::InvalidMep)?;
        let tee = matches!(br.fitting.kind, FittingKind::Tee { .. });
        let first = self.next_id().0;
        let (mut ops, mut ids) = (Vec::new(), Vec::new());
        // segment 0 is the original; the others are new elements in order
        let mut new_ids = std::collections::BTreeMap::new();
        for (i, s) in net.segments.iter().enumerate() {
            let r = &s.run;
            let len = (r.b[0] - r.a[0]).hypot(r.b[1] - r.a[1]);
            let id = if i == 0 { main } else { ElementId(first + new_ids.len() as u64) };
            if len < 1e-9 { return Err(EditError::InvalidGeometry(id)); }
            if i == 0 {
                ops.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: [r.a[0], r.a[1]], end: [r.b[0], r.b[1]], level } });
            } else {
                let (category, w, h) = match (r.domain, r.size) {
                    (Domain::Pipe, RunSize::Round { diameter }) => (Category::Pipe, diameter, diameter),
                    (Domain::Duct, RunSize::Rect { width, height, roll: 0.0 }) => (Category::Duct, width, height),
                    _ => return Err(EditError::UnsupportedProperty(id)),
                };
                new_ids.insert(i, id);
                ops.push(Command::Create { id, category, name: format!("{} {}", r.label, i), width_m: w, height_m: h, elevation_m: r.a[2] - z0 });
                ops.push(Command::SetSlope { id, slope_ratio: (r.b[2] - r.a[2]) / len });
                ops.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: [r.a[0], r.a[1]], end: [r.b[0], r.b[1]], level } });
                ids.push(id);
            }
        }
        let rid = |i: usize| if i == 0 { main } else { new_ids[&i] };
        let mut data = self.mep.clone();
        // the second half of a tee takes over the end of the run
        let half2 = if tee { Some(rid(1)) } else { None };
        let cut = if tee { net.segments.iter().enumerate().skip(1).find(|(_, s)| (s.run.b[0] - run.b[0]).hypot(s.run.b[1] - run.b[1]) < 1e-6 && (s.run.b[2] - run.b[2]).abs() < 1e-6).map(|(i, _)| i) } else { None };
        if tee && cut.is_none() { return Err(EditError::InvalidMep); }
        if let Some(c) = cut {
            let h2 = rid(c);
            let old = MepPort { node: MepNode::Run(main), port: 1 };
            let moved = MepPort { node: MepNode::Run(h2), port: 1 };
            for l in data.links.iter_mut() { if l.0 == old { l.0 = moved; } else if l.1 == old { l.1 = moved; } }
            for e in data.endpoints.iter_mut() { if e.port == old { e.port = moved; } }
            // taps beyond the cut follow
            let cut_at = { let a = net.segments[0].run.a; let b = net.segments[c].run.a; ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt() };
            for t in data.taps.iter_mut() { if t.run == main && t.distance >= cut_at { t.run = h2; t.distance -= cut_at; } }
            if let Some(sys) = data.run_system(main) { data.run_systems.push((h2, sys)); }
        }
        let _ = half2;
        let base = data.next_fitting_id();
        let fid = |i: usize| base + i as u64;
        for (i, f) in net.fittings.iter().enumerate() {
            data.fittings.push(MepFitting { id: fid(i), fitting: f.fitting.clone(), domain: f.domain, system: f.system });
        }
        // the new runs take the system of the branch
        for (i, s) in net.segments.iter().enumerate().skip(1) {
            if Some(i) != cut && data.run_system(rid(i)).is_none() { data.run_systems.push((rid(i), s.run.system)); }
        }
        let node = |n: NodeRef| match n { NodeRef::Segment(i) => Some(MepNode::Run(rid(i))), NodeRef::Fitting(i) => Some(MepNode::Fitting(fid(i))), NodeRef::Equipment(_) => None };
        for (a, b) in net.links() {
            let (Some(na), Some(nb)) = (node(a.node), node(b.node)) else { return Err(EditError::InvalidMep) };
            data.links.push((MepPort { node: na, port: a.port }, MepPort { node: nb, port: b.port }));
        }
        for t in &net.taps { data.taps.push(MepTap { fitting: fid(t.fitting), run: rid(t.segment), distance: t.distance }); }
        ops.push(Command::SetMep(Box::new(data)));
        self.transact(self.revision(), &ops)?;
        if let Some(c) = cut { ids.retain(|i| *i != rid(c)); ids.insert(0, rid(c)); }
        Ok(ids)
    }

    /// Puts a routed path into the model: one duct or pipe element for every run, the fittings between them, and the
    /// links, in one transaction. Ducts are rectangular and pipes round; a vertical run, a turned duct (roll) or a
    /// round duct cannot be held by the model yet and refuse the whole route.
    pub fn place_mep_route(&mut self, level: ElementId, route: &openbim_route::Route) -> Result<Vec<ElementId>, EditError> {
        use crate::Command;
        use openbim_place::penetrations::RunSize;
        use openbim_route::graph::{Network, NodeRef};
        let z0 = self.get(level).filter(|l| l.category == Category::Level).ok_or(EditError::NotFound(level))?.elevation_m;
        let mut net = Network::new(1);
        net.add_route(route).map_err(|_| EditError::InvalidMep)?;
        let first = self.next_id().0;
        let (mut ops, mut ids) = (Vec::new(), Vec::new());
        for (i, s) in net.segments.iter().enumerate() {
            let id = ElementId(first + i as u64);
            let r = &s.run;
            let len = (r.b[0] - r.a[0]).hypot(r.b[1] - r.a[1]);
            if len < 1e-9 { return Err(EditError::InvalidGeometry(id)); }
            let (category, w, h) = match (r.domain, r.size) {
                (Domain::Pipe, RunSize::Round { diameter }) => (Category::Pipe, diameter, diameter),
                (Domain::Duct, RunSize::Rect { width, height, roll: 0.0 }) => (Category::Duct, width, height),
                _ => return Err(EditError::UnsupportedProperty(id)),
            };
            ops.push(Command::Create { id, category, name: format!("{} {}", r.label, i + 1), width_m: w, height_m: h, elevation_m: r.a[2] - z0 });
            ops.push(Command::SetSlope { id, slope_ratio: (r.b[2] - r.a[2]) / len });
            ops.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: [r.a[0], r.a[1]], end: [r.b[0], r.b[1]], level } });
            ids.push(id);
        }
        let mut data = self.mep.clone();
        let base = data.next_fitting_id();
        let fid = |i: usize| base + i as u64;
        for (i, f) in net.fittings.iter().enumerate() {
            data.fittings.push(MepFitting { id: fid(i), fitting: f.fitting.clone(), domain: f.domain, system: f.system });
        }
        let node = |n: NodeRef| match n { NodeRef::Segment(i) => Some(MepNode::Run(ids[i])), NodeRef::Fitting(i) => Some(MepNode::Fitting(fid(i))), NodeRef::Equipment(_) => None };
        for (a, b) in net.links() {
            let (Some(na), Some(nb)) = (node(a.node), node(b.node)) else { return Err(EditError::InvalidMep) };
            data.links.push((MepPort { node: na, port: a.port }, MepPort { node: nb, port: b.port }));
        }
        for t in &net.taps { data.taps.push(MepTap { fitting: fid(t.fitting), run: ids[t.segment], distance: t.distance }); }
        // every run drawn belongs to the system it was drawn for
        for (i, s) in net.segments.iter().enumerate() { data.run_systems.push((ids[i], s.run.system)); }
        ops.push(Command::SetMep(Box::new(data)));
        self.transact(self.revision(), &ops)?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Document};
    use openbim_place::penetrations::RunSize;
    use openbim_route::snaps::{SnapSettings, Snaps};
    use openbim_route::{route_path, AngleSet, Junction, RoutingPrefs};

    fn prefs(domain: Domain, system: System) -> RoutingPrefs {
        RoutingPrefs {
            domain, system, label: "Run".into(), elbow_angles: AngleSet::Any, bend_radius_factor: 1.5, min_straight: 0.0, junction: Junction::Tee,
            tee_angles: AngleSet::Any, tee_half_factor: 1.0, branch_stub_factor: 1.0, pick_tolerance: 0.02, transition_angle_deg: 30.0,
            min_transition_len: 0.1, connector_stub: 0.5, margin: 0.0, catalogue: None,
        }
    }

    fn duct_route(points: &[[f64; 3]]) -> openbim_route::Route {
        let size = RunSize::Rect { width: 0.4, height: 0.3, roll: 0.0 };
        route_path(points, &vec![size; points.len() - 1], &prefs(Domain::Duct, System::SupplyAir), &Snaps::new(SnapSettings::off())).unwrap()
    }

    #[test]
    fn a_routed_duct_with_its_elbow_is_held_by_the_model_linked_and_undone_in_one_step() {
        let mut d = Document::spatial_sample();
        let route = duct_route(&[[1.0, 1.0, 2.7], [5.0, 1.0, 2.7], [5.0, 4.0, 2.7]]);
        assert_eq!((route.runs.len(), route.fittings.len()), (2, 1));
        let revision = d.revision();
        let ids = d.place_mep_route(ElementId(10), &route).unwrap();
        assert_eq!(d.revision(), revision + 1, "one transaction");
        assert_eq!(ids.len(), 2);
        let m = d.mep();
        assert_eq!((m.fittings.len(), m.links.len()), (1, 2));
        let f = m.fittings[0].id;
        assert!(f >= FITTING_BASE);
        // each leg of the elbow is joined to one end of one run
        for leg in 0..2 { assert!(m.partner(MepPort { node: MepNode::Fitting(f), port: leg }).is_some()); }
        // the elements are real ducts on the level
        let run = d.get(ids[0]).unwrap();
        assert_eq!((run.category, run.width_m, run.height_m, run.elevation_m), (Category::Duct, 0.4, 0.3, 2.7));

        // the text form gives the same fittings and links back
        let lines = crate::native::records(&d);
        assert_eq!(crate::native::parse_mep(&lines).unwrap(), *m);

        // deleting a run takes the links to it away, and undo brings everything back
        d.transact(d.revision(), &[Command::Delete { id: ids[1] }]).unwrap();
        assert_eq!(d.mep().links.len(), 1);
        assert!(d.undo().unwrap());
        assert_eq!(d.mep().links.len(), 2);
        assert!(d.undo().unwrap());
        assert!(d.mep().is_empty() && d.get(ids[0]).is_none());
    }

    #[test]
    fn what_the_model_cannot_hold_refuses_the_whole_route_and_bad_links_are_refused() {
        let mut d = Document::spatial_sample();
        let before = d.revision();
        // a riser is vertical
        let up = duct_route(&[[1.0, 1.0, 2.7], [5.0, 1.0, 2.7], [5.0, 1.0, 5.0]]);
        assert!(d.place_mep_route(ElementId(10), &up).is_err());
        let turned = route_path(&[[1.0, 1.0, 2.7], [5.0, 1.0, 2.7]], &[RunSize::Rect { width: 0.4, height: 0.3, roll: 30.0 }], &prefs(Domain::Duct, System::SupplyAir), &Snaps::new(SnapSettings::off())).unwrap();
        assert!(d.place_mep_route(ElementId(10), &turned).is_err());
        assert!(d.place_mep_route(ElementId(999), &duct_route(&[[1.0, 1.0, 2.7], [5.0, 1.0, 2.7]])).is_err());
        assert_eq!((d.revision(), d.elements().count()), (before, Document::spatial_sample().elements().count()), "nothing changed");
        // a link to a port that is not there
        let bad = MepData { links: vec![(MepPort { node: MepNode::Run(ElementId(1)), port: 5 }, MepPort { node: MepNode::Run(ElementId(2)), port: 0 })], ..Default::default() };
        assert_eq!(d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]), Err(EditError::InvalidMep));
    }

    #[test]
    fn run_systems_and_end_points_are_checked_kept_with_the_model_and_come_back_from_the_records() {
        let mut d = Document::spatial_sample();
        let ids = d.place_mep_route(ElementId(10), &duct_route(&[[1.0, 1.0, 2.7], [5.0, 1.0, 2.7], [5.0, 4.0, 2.7]])).unwrap();
        let mut data = d.mep().clone();
        // the first run's start and the second run's end are open
        let src = MepPort { node: MepNode::Run(ids[0]), port: 0 };
        let end = MepPort { node: MepNode::Run(ids[1]), port: 1 };
        data.run_systems.retain(|(r, _)| *r != ids[0]);
        data.run_systems.push((ids[0], System::ReturnAir));
        data.endpoints.push(MepEndpoint { port: src, role: EndRole::Source, demand: EndDemand::None, required_pressure_pa: 0.0 });
        data.endpoints.push(MepEndpoint { port: end, role: EndRole::Terminal, demand: EndDemand::Power { watts: 5000.0, delta_t: 10.0 }, required_pressure_pa: 25.0 });
        d.transact(d.revision(), &[Command::SetMep(Box::new(data.clone()))]).unwrap();
        assert_eq!(d.mep().run_system(ids[0]), Some(System::ReturnAir));
        assert_eq!(d.mep().endpoint_at(end).unwrap().required_pressure_pa, 25.0);
        // the text form gives the same data back, whatever the kind of demand
        let mut every = data.clone();
        every.endpoints[1].demand = EndDemand::Sprinkler { k_l_min_bar: 80.0, min_flow_l_min: 60.0 };
        for dem in [EndDemand::None, EndDemand::Flow(0.003), EndDemand::LoadingUnits(7.5), every.endpoints[1].demand, EndDemand::Power { watts: 0.1 + 0.2, delta_t: 5.0 }] {
            let mut x = data.clone();
            x.endpoints[1].demand = dem;
            let lines = crate::native::mep_records(&x);
            assert_eq!(crate::native::parse_mep(&lines).unwrap(), x);
        }
        // a linked port is not an end; a system of the wrong kind, a second end point and a bad value are refused
        let linked = MepPort { node: MepNode::Run(ids[0]), port: 1 };
        let bad = |f: &dyn Fn(&mut MepData)| { let mut x = data.clone(); f(&mut x); d.clone().transact(d.revision(), &[Command::SetMep(Box::new(x))]) };
        assert_eq!(bad(&|x| x.endpoints.push(MepEndpoint { port: linked, role: EndRole::Terminal, demand: EndDemand::None, required_pressure_pa: 0.0 })), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.run_systems[0].1 = System::ChilledWaterSupply), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.endpoints.push(x.endpoints[0])), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.endpoints[1].demand = EndDemand::Power { watts: 1.0, delta_t: 0.0 }), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.endpoints[0].demand = EndDemand::Flow(1.0)), Err(EditError::InvalidMep));
        // deleting the run takes its end point and system away; undo brings them back
        d.transact(d.revision(), &[Command::Delete { id: ids[1] }]).unwrap();
        assert!(d.mep().endpoint_at(end).is_none() && d.mep().endpoint_at(src).is_some());
        d.transact(d.revision(), &[Command::Delete { id: ids[0] }]).unwrap();
        assert!(d.mep().run_systems.is_empty() && d.mep().endpoints.is_empty());
        assert!(d.undo().unwrap() && d.undo().unwrap());
        assert_eq!(d.mep().endpoints.len(), 2);
    }

    #[test]
    fn a_branch_is_joined_to_a_drawn_run_as_a_tee_or_a_tap_and_undone_in_one_step() {
        for tee in [true, false] {
            let mut d = Document::spatial_sample();
            let ids = d.place_mep_route(ElementId(10), &duct_route(&[[1.0, 1.0, 2.7], [9.0, 1.0, 2.7]])).unwrap();
            let main = ids[0];
            let mut p = prefs(Domain::Duct, System::SupplyAir);
            p.junction = if tee { Junction::Tee } else { Junction::Tap };
            let mainrun = openbim_place::penetrations::Run::rect("Run", Domain::Duct, System::SupplyAir, [1.0, 1.0, 2.7], [9.0, 1.0, 2.7], 0.4, 0.3);
            let size = RunSize::Rect { width: 0.3, height: 0.2, roll: 0.0 };
            let br = openbim_route::branch_from_run(&mainrun, &[[5.0, 1.0, 2.7], [5.0, 5.0, 2.7]], &[size], &p, &Snaps::new(SnapSettings::off())).unwrap();
            let rev = d.revision();
            let new = d.place_mep_branch(ElementId(10), main, &br).unwrap_or_else(|e| panic!("tee {tee}: {e:?}"));
            assert_eq!(d.revision(), rev + 1);
            let m = d.mep();
            assert_eq!(m.fittings.len(), 1);
            assert_eq!(m.taps.len(), usize::from(!tee));
            assert_eq!(new.len(), if tee { 2 } else { 1 });
            // every leg of the fitting is joined (a tap's leg is, its run is held by the tap)
            let f = m.fittings[0].id;
            for leg in 0..m.fittings[0].fitting.ports.len() { assert!(m.partner(MepPort { node: MepNode::Fitting(f), port: leg }).is_some(), "tee {tee} leg {leg}"); }
            // the far end of the original run is now the end of the last piece, and the system is kept on every run
            for id in std::iter::once(main).chain(new.iter().copied()) {
                assert_eq!(crate::compute::system_of_run(&d, id), System::SupplyAir, "tee {tee} run {id:?} of {:?}", d.mep().run_systems);
            }
            let text = crate::native::records(&d);
            assert_eq!(crate::native::parse_mep(&text).unwrap(), *d.mep());
            assert!(d.undo().unwrap());
            assert!(d.mep().fittings.len() == 0 && d.get(main).is_some() && d.get(new[0]).is_none());
            assert_eq!(d.mep().links.len(), 0);
        }
    }

    #[test]
    fn the_electrical_network_is_checked_kept_with_the_model_and_comes_back_from_the_records() {
        use crate::elec::{ElecCable, ElecKind, ElecNode, NODE_BASE};
        let mut d = Document::spatial_sample();
        let (a, b, c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
        let mut load = ElecNode::new(c, "Chiller | 1; a=b", ElecKind::Load, [6.0, 2.0], ElementId(10));
        (load.watts, load.power_factor, load.efficiency, load.motor) = (45000.5, 0.85, 0.93, Some((6.0, 0.35)));
        let mut cable = ElecCable::new(b, c, true);
        cable.via = vec![[3.0, 0.1], [3.0, 2.0]];
        (cable.extra_m, cable.grouped_circuits, cable.ambient_c, cable.demand_factor, cable.rcd_ma) = (3.5, 3, Some(35.0), 0.8, Some(30.0));
        let data = MepData {
            elec_nodes: vec![ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)), ElecNode::new(b, "MSB", ElecKind::Board, [3.0, 0.0], ElementId(10)), load],
            elec_cables: vec![ElecCable::new(a, b, true), cable],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data.clone()))]).unwrap();
        assert_eq!(d.mep().elec_nodes.len(), 3);
        assert_eq!(d.mep().next_node_id(), NODE_BASE + 3);
        let lines = crate::native::records(&d);
        assert_eq!(crate::native::parse_mep(&lines).unwrap(), *d.mep());
        // refused: a load that feeds, a node fed twice, a second feed, a bad factor, a missing level
        let bad = |f: &dyn Fn(&mut MepData)| { let mut x = data.clone(); f(&mut x); d.clone().transact(d.revision(), &[Command::SetMep(Box::new(x))]) };
        assert_eq!(bad(&|x| x.elec_cables.push(ElecCable::new(c, a, true))), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.elec_cables.push(ElecCable::new(a, c, true))), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.elec_nodes[2].power_factor = 1.5), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.elec_cables[0].demand_factor = 0.0), Err(EditError::InvalidMep));
        assert_eq!(bad(&|x| x.elec_nodes[0].watts = 5.0), Err(EditError::InvalidMep));
        // a node on a level that is not there is dropped with its cables, as when the level is deleted
        let mut gone = data.clone();
        gone.elec_nodes[2].level = ElementId(999);
        let mut e = d.clone();
        e.transact(e.revision(), &[Command::SetMep(Box::new(gone))]).unwrap();
        assert_eq!((e.mep().elec_nodes.len(), e.mep().elec_cables.len()), (2, 1));
        // deleting the level takes its nodes and their cables away; undo brings them back
        d.transact(d.revision(), &[Command::Create { id: ElementId(700), category: Category::Level, name: "L9".into(), width_m: 1.0, height_m: 1.0, elevation_m: 9.0 }, Command::SetGeometry { id: ElementId(700), geometry: Geometry::Level }]).unwrap();
        let mut up = d.mep().clone();
        up.elec_nodes[2].level = ElementId(700);
        d.transact(d.revision(), &[Command::SetMep(Box::new(up))]).unwrap();
        d.transact(d.revision(), &[Command::Delete { id: ElementId(700) }]).unwrap();
        assert_eq!((d.mep().elec_nodes.len(), d.mep().elec_cables.len()), (2, 1));
        assert!(d.undo().unwrap());
        assert_eq!((d.mep().elec_nodes.len(), d.mep().elec_cables.len()), (3, 2));
    }
}
