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

#[derive(Clone, Debug, PartialEq, Default)]
pub struct MepData {
    pub fittings: Vec<MepFitting>,
    pub links: Vec<(MepPort, MepPort)>,
    pub taps: Vec<MepTap>,
}

impl MepData {
    pub fn is_empty(&self) -> bool { self.fittings.is_empty() && self.links.is_empty() && self.taps.is_empty() }

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
        }
    }

    /// Every link joins two existing ports once, nothing is joined to itself, and the fittings are sound.
    pub(crate) fn check(&self, elements: &BTreeMap<ElementId, Element>) -> Result<(), EditError> {
        let mut ids = std::collections::BTreeSet::new();
        for f in &self.fittings {
            let n = f.fitting.ports.len();
            if f.id < FITTING_BASE || !ids.insert(f.id) || !(2..=16).contains(&n) || f.fitting.port_sizes.len() != n || f.fitting.trim.len() != n || !f.system.domain_ok(f.domain) {
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
        Ok(())
    }
}

fn is_run(e: Option<&Element>) -> bool {
    e.is_some_and(|e| matches!(e.category, Category::Duct | Category::Pipe) && matches!(e.geometry, Some(Geometry::Linear { .. })))
}

impl crate::Document {
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
}
