//! The connectivity graph of a piping, duct or tray network: which straight run joins which fitting, and
//! which equipment connector a run starts at. Nodes are straight runs (2 ports: start and end), fittings
//! (one port for each leg) and equipment connectors (1 port). A link joins two ports and says they meet.
//! A tap is a fitting that sits on the side of a run without splitting it; it is recorded as a tapping.
//!
//! The graph is what the IFC export needs (ports and their connections) and what re-routing needs (which
//! pieces move together). It is saved in a compact binary form, not JSON.
use crate::{Branch, Fitting, FittingKind, Route};
use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_place::penetrations::{Run, RunSize};

/// Two points closer than this (metres) are the same point.
pub const TOL: f64 = 1e-6;
pub const MAX_NODES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeRef {
    Segment(usize),
    Fitting(usize),
    Equipment(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortRef {
    pub node: NodeRef,
    pub port: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetSegment {
    pub id: u64,
    pub run: Run,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetFitting {
    pub id: u64,
    pub fitting: Fitting,
    pub domain: Domain,
    pub system: System,
}

/// An equipment connector as seen from the network: the element it belongs to and where it faces.
#[derive(Clone, Debug, PartialEq)]
pub struct NetEquipment {
    /// element number of the equipment in the model
    pub element: u64,
    pub connector: String,
    pub domain: Domain,
    pub system: System,
    /// which way the medium goes through the connector (into the equipment, out of it, or both)
    pub flow_dir: FlowDir,
    pub pos: [f64; 3],
    pub dir: [f64; 3],
    pub size: RunSize,
}

/// A tap fitting sitting on a run: `distance` along the run from its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tapping {
    pub fitting: usize,
    pub segment: usize,
    pub distance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphError {
    TooMany,
    /// a fitting leg did not meet a run end or another fitting leg
    Unconnected { fitting: usize, port: usize },
    /// two ports meet but their sizes differ
    SizeMismatch(PortRef, PortRef),
    /// two ports meet but are in a different domain or system
    SystemMismatch(PortRef, PortRef),
    /// two linked ports are not at the same place, or do not face each other
    NotMeeting(PortRef, PortRef),
    PortTaken(PortRef),
    NoSuchPort(PortRef),
    /// a node linked to itself
    SelfLink(PortRef),
    /// the branch was made from a different run than the one named
    MainMismatch,
    NotATee,
    NotATap,
    NotOnRun,
    /// a run on the way has a tap on it, so it cannot be taken away
    TapOnChain(usize),
    /// no equipment connector with that number
    NoSuchEquipment(usize),
    /// the connector given does not belong to this equipment connector (domain or system differ)
    EquipmentMismatch,
    /// the connector is not at that end of the run, or does not face along the run
    ConnectorMismatch,
    Corrupt(&'static str),
    /// insulation needs a thickness above 0 and below 1 m, and a name
    BadInsulation,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for GraphError {}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Network {
    pub segments: Vec<NetSegment>,
    pub fittings: Vec<NetFitting>,
    pub equipment: Vec<NetEquipment>,
    links: Vec<(PortRef, PortRef)>,
    pub taps: Vec<Tapping>,
    /// insulation laid on every run and fitting of a system, one entry per system
    pub insulation: Vec<(System, Insulation)>,
    next_id: u64,
}

/// Insulation on the pieces of one system: a layer of this thickness all round (Revit's pipe and duct insulation).
#[derive(Clone, Debug, PartialEq)]
pub struct Insulation {
    /// metres
    pub thickness: f64,
    pub material: String,
}

/// How a chain of pieces from an equipment connector ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainEnd {
    /// at this port of the next junction (a tee or tap leg, or another equipment connector)
    Port(PortRef),
    /// the last piece has a run end that leads nowhere
    Open,
}

/// The pieces between an equipment connector and the next junction, in order from the equipment: runs and
/// fittings with two legs. These are what moves when the equipment moves.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    pub nodes: Vec<NodeRef>,
    pub end: ChainEnd,
}

impl Chain {
    /// The port of the junction this chain ends at, if it ends at one.
    pub fn end_port(&self) -> Option<PortRef> {
        match self.end {
            ChainEnd::Port(p) => Some(p),
            ChainEnd::Open => None,
        }
    }
}

/// What `add_route` and `add_branch` created.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Added {
    pub segments: Vec<usize>,
    pub fittings: Vec<usize>,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}
fn neg(a: [f64; 3]) -> [f64; 3] {
    [-a[0], -a[1], -a[2]]
}
fn same_size(a: RunSize, b: RunSize) -> bool {
    match (a, b) {
        (RunSize::Round { diameter: x }, RunSize::Round { diameter: y }) => (x - y).abs() < 1e-9,
        (RunSize::Rect { width: w1, height: h1, .. }, RunSize::Rect { width: w2, height: h2, .. }) => (w1 - w2).abs() < 1e-9 && (h1 - h2).abs() < 1e-9,
        _ => false,
    }
}

impl Network {
    /// `id_base` is the first element number given to the pieces; they count up from it. (The operation log
    /// will hand these out; until then the caller chooses.)
    pub fn new(id_base: u64) -> Network {
        Network { next_id: id_base, ..Default::default() }
    }
    /// Insulate every piece of `system`, replacing any earlier insulation of that system.
    pub fn insulate(&mut self, system: System, thickness: f64, material: &str) -> Result<(), GraphError> {
        if thickness.is_nan() || thickness <= 0.0 || thickness >= 1.0 || material.trim().is_empty() { return Err(GraphError::BadInsulation); }
        let ins = Insulation { thickness, material: material.to_string() };
        match self.insulation.iter_mut().find(|(s, _)| *s == system) {
            Some(e) => e.1 = ins,
            None => self.insulation.push((system, ins)),
        }
        Ok(())
    }
    pub fn insulation_of(&self, system: System) -> Option<&Insulation> {
        self.insulation.iter().find(|(s, _)| *s == system).map(|e| &e.1)
    }
    pub fn next_id(&self) -> u64 {
        self.next_id
    }
    fn take_id(&mut self) -> u64 {
        let i = self.next_id;
        self.next_id += 1;
        i
    }
    pub fn links(&self) -> &[(PortRef, PortRef)] {
        &self.links
    }
    fn check_room(&self, more: usize) -> Result<(), GraphError> {
        if self.segments.len() + self.fittings.len() + self.equipment.len() + more > MAX_NODES {
            return Err(GraphError::TooMany);
        }
        Ok(())
    }

    pub fn port_count(&self, n: NodeRef) -> usize {
        match n {
            NodeRef::Segment(i) if i < self.segments.len() => 2,
            NodeRef::Fitting(i) => self.fittings.get(i).map_or(0, |f| f.fitting.ports.len()),
            NodeRef::Equipment(i) if i < self.equipment.len() => 1,
            _ => 0,
        }
    }

    /// Where a port is.
    pub fn port_pos(&self, p: PortRef) -> Option<[f64; 3]> {
        if p.port >= self.port_count(p.node) {
            return None;
        }
        Some(match p.node {
            NodeRef::Segment(i) => {
                if p.port == 0 {
                    self.segments[i].run.a
                } else {
                    self.segments[i].run.b
                }
            }
            NodeRef::Fitting(i) => {
                let f = &self.fittings[i].fitting;
                let (d, t) = (f.ports[p.port], f.trim.get(p.port).copied().unwrap_or(0.0));
                [f.at[0] + d[0] * t, f.at[1] + d[1] * t, f.at[2] + d[2] * t]
            }
            NodeRef::Equipment(i) => self.equipment[i].pos,
        })
    }

    /// The unit direction leaving the piece through this port (pointing away from the piece's body).
    pub fn port_dir(&self, p: PortRef) -> Option<[f64; 3]> {
        if p.port >= self.port_count(p.node) {
            return None;
        }
        Some(match p.node {
            NodeRef::Segment(i) => {
                let r = &self.segments[i].run;
                let d = unit(sub(r.b, r.a));
                if p.port == 0 {
                    neg(d)
                } else {
                    d
                }
            }
            NodeRef::Fitting(i) => self.fittings[i].fitting.ports[p.port],
            // the connector faces the way the route leaves it
            NodeRef::Equipment(i) => unit(self.equipment[i].dir),
        })
    }

    pub fn port_size(&self, p: PortRef) -> Option<RunSize> {
        if p.port >= self.port_count(p.node) {
            return None;
        }
        Some(match p.node {
            NodeRef::Segment(i) => self.segments[i].run.size,
            NodeRef::Fitting(i) => self.fittings[i].fitting.port_sizes[p.port],
            NodeRef::Equipment(i) => self.equipment[i].size,
        })
    }

    pub fn node_domain_system(&self, n: NodeRef) -> Option<(Domain, System)> {
        match n {
            NodeRef::Segment(i) => self.segments.get(i).map(|s| (s.run.domain, s.run.system)),
            NodeRef::Fitting(i) => self.fittings.get(i).map(|f| (f.domain, f.system)),
            NodeRef::Equipment(i) => self.equipment.get(i).map(|e| (e.domain, e.system)),
        }
    }

    /// The port this port is linked to.
    pub fn link_of(&self, p: PortRef) -> Option<PortRef> {
        self.links.iter().find_map(|(a, b)| if *a == p { Some(*b) } else if *b == p { Some(*a) } else { None })
    }

    /// Join two ports. Checks everything `validate` does for one link.
    pub fn link(&mut self, a: PortRef, b: PortRef) -> Result<(), GraphError> {
        self.check_link(a, b)?;
        let l = if a <= b { (a, b) } else { (b, a) };
        self.links.push(l);
        Ok(())
    }

    fn check_link(&self, a: PortRef, b: PortRef) -> Result<(), GraphError> {
        for p in [a, b] {
            if p.port >= self.port_count(p.node) {
                return Err(GraphError::NoSuchPort(p));
            }
        }
        if a.node == b.node {
            return Err(GraphError::SelfLink(a));
        }
        for p in [a, b] {
            if self.link_of(p).is_some() {
                return Err(GraphError::PortTaken(p));
            }
        }
        let (da, db) = (self.node_domain_system(a.node).ok_or(GraphError::NoSuchPort(a))?, self.node_domain_system(b.node).ok_or(GraphError::NoSuchPort(b))?);
        if da.0 != db.0 || !da.1.compatible(db.1) {
            return Err(GraphError::SystemMismatch(a, b));
        }
        let (sa, sb) = (self.port_size(a).ok_or(GraphError::NoSuchPort(a))?, self.port_size(b).ok_or(GraphError::NoSuchPort(b))?);
        if !same_size(sa, sb) {
            return Err(GraphError::SizeMismatch(a, b));
        }
        let (pa, pb) = (self.port_pos(a).ok_or(GraphError::NoSuchPort(a))?, self.port_pos(b).ok_or(GraphError::NoSuchPort(b))?);
        let (xa, xb) = (self.port_dir(a).ok_or(GraphError::NoSuchPort(a))?, self.port_dir(b).ok_or(GraphError::NoSuchPort(b))?);
        if dist(pa, pb) > TOL || dot(xa, xb) > -1.0 + 1e-6 {
            return Err(GraphError::NotMeeting(a, b));
        }
        Ok(())
    }

    /// Every port of the given nodes that is not linked and has a partner within `TOL` among the nodes
    /// (facing it) is linked to the closest one. Fitting legs must find a partner; run ends need not.
    fn auto_link(&mut self, nodes: &[NodeRef]) -> Result<(), GraphError> {
        let mut ports: Vec<PortRef> = Vec::new();
        for &n in nodes {
            for k in 0..self.port_count(n) {
                ports.push(PortRef { node: n, port: k });
            }
        }
        for &p in &ports {
            if self.link_of(p).is_some() || !matches!(p.node, NodeRef::Fitting(_)) {
                continue;
            }
            let pp = self.port_pos(p).ok_or(GraphError::NoSuchPort(p))?;
            let mut best: Option<(f64, PortRef)> = None;
            for &q in &ports {
                if q.node == p.node || self.link_of(q).is_some() {
                    continue;
                }
                let d = dist(pp, self.port_pos(q).ok_or(GraphError::NoSuchPort(q))?);
                if d <= TOL && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, q));
                }
            }
            match best {
                Some((_, q)) => self.link(p, q)?,
                None => {
                    if let NodeRef::Fitting(f) = p.node {
                        return Err(GraphError::Unconnected { fitting: f, port: p.port });
                    }
                }
            }
        }
        Ok(())
    }

    fn push_route(&mut self, route: &Route) -> Result<Added, GraphError> {
        self.check_room(route.runs.len() + route.fittings.len())?;
        let mut added = Added::default();
        for r in &route.runs {
            let id = self.take_id();
            self.segments.push(NetSegment { id, run: r.clone() });
            added.segments.push(self.segments.len() - 1);
        }
        let (domain, system) = route.runs.first().map_or((Domain::Pipe, System::Other), |r| (r.domain, r.system));
        for f in &route.fittings {
            let id = self.take_id();
            self.fittings.push(NetFitting { id, fitting: f.clone(), domain, system });
            added.fittings.push(self.fittings.len() - 1);
        }
        Ok(added)
    }

    /// Add a route: its runs and fittings, linked where they meet.
    pub fn add_route(&mut self, route: &Route) -> Result<Added, GraphError> {
        let snapshot = self.clone();
        let r = self.push_route(route).and_then(|added| {
            let nodes: Vec<NodeRef> = added.segments.iter().map(|i| NodeRef::Segment(*i)).chain(added.fittings.iter().map(|i| NodeRef::Fitting(*i))).collect();
            self.auto_link(&nodes).map(|_| added)
        });
        if r.is_err() {
            *self = snapshot;
        }
        r
    }

    /// Add a branch made from the run `main`. A tee replaces `main` by its two halves; a tap leaves it whole
    /// and is recorded as a tapping. Nothing changes when this fails.
    pub fn add_branch(&mut self, main: usize, br: &Branch) -> Result<Added, GraphError> {
        let snapshot = self.clone();
        let r = self.add_branch_inner(main, br);
        if r.is_err() {
            *self = snapshot;
        }
        r
    }

    fn add_branch_inner(&mut self, main: usize, br: &Branch) -> Result<Added, GraphError> {
        let orig = self.segments.get(main).ok_or(GraphError::NoSuchPort(PortRef { node: NodeRef::Segment(main), port: 0 }))?.run.clone();
        self.check_room(br.route.runs.len() + br.route.fittings.len() + 3)?;
        let mut touched = vec![NodeRef::Segment(main)];
        let mut added = Added::default();
        match br.fitting.kind {
            FittingKind::Tee { .. } => {
                if br.main.len() != 2 || dist(br.main[0].a, orig.a) > TOL || dist(br.main[1].b, orig.b) > TOL {
                    return Err(GraphError::MainMismatch);
                }
                // the second half takes over the end of the original run and its links
                let new_id = self.take_id();
                self.segments[main].run = br.main[0].clone();
                self.segments.push(NetSegment { id: new_id, run: br.main[1].clone() });
                let new = self.segments.len() - 1;
                let old_end = PortRef { node: NodeRef::Segment(main), port: 1 };
                let new_end = PortRef { node: NodeRef::Segment(new), port: 1 };
                for l in self.links.iter_mut() {
                    if l.0 == old_end {
                        l.0 = new_end;
                    }
                    if l.1 == old_end {
                        l.1 = new_end;
                    }
                }
                for l in self.links.iter_mut() {
                    if l.0 > l.1 {
                        *l = (l.1, l.0);
                    }
                }
                for t in self.taps.iter_mut() {
                    if t.segment == main {
                        let s = dot(sub(self.segments[new].run.a, orig.a), unit(sub(orig.b, orig.a)));
                        if t.distance >= s {
                            t.segment = new;
                            t.distance -= s;
                        }
                    }
                }
                touched.push(NodeRef::Segment(new));
                added.segments.push(new);
            }
            FittingKind::Tap { .. } => {
                if br.main.len() != 1 || dist(br.main[0].a, orig.a) > TOL || dist(br.main[0].b, orig.b) > TOL {
                    return Err(GraphError::MainMismatch);
                }
            }
            _ => return Err(GraphError::NotATee),
        }
        let fit_id = self.take_id();
        self.fittings.push(NetFitting { id: fit_id, fitting: br.fitting.clone(), domain: orig.domain, system: orig.system });
        let fi = self.fittings.len() - 1;
        added.fittings.push(fi);
        touched.push(NodeRef::Fitting(fi));
        if matches!(br.fitting.kind, FittingKind::Tap { .. }) {
            let d = unit(sub(orig.b, orig.a));
            let s = dot(sub(br.fitting.at, orig.a), d);
            let foot = [orig.a[0] + d[0] * s, orig.a[1] + d[1] * s, orig.a[2] + d[2] * s];
            if dist(foot, br.fitting.at) > TOL || s < -TOL || s > dist(orig.a, orig.b) + TOL {
                return Err(GraphError::NotOnRun);
            }
            self.taps.push(Tapping { fitting: fi, segment: main, distance: s });
        }
        let rt = self.push_route(&br.route)?;
        for i in &rt.segments {
            touched.push(NodeRef::Segment(*i));
        }
        for i in &rt.fittings {
            touched.push(NodeRef::Fitting(*i));
        }
        added.segments.extend(rt.segments);
        added.fittings.extend(rt.fittings);
        self.auto_link(&touched)?;
        Ok(added)
    }

    /// Join an equipment connector to the end of a run. The run must start (or end) at the connector and
    /// leave it along the way it faces.
    pub fn attach_equipment(&mut self, element: u64, conn: &Connector, segment: usize, end: End) -> Result<usize, GraphError> {
        self.check_room(1)?;
        let run = self.segments.get(segment).ok_or(GraphError::ConnectorMismatch)?.run.clone();
        let (at, away) = match end {
            End::Start => (run.a, unit(sub(run.b, run.a))),
            End::End => (run.b, unit(sub(run.a, run.b))),
        };
        let size = match (conn.size, run.size) {
            (Size::Round { diameter }, RunSize::Round { .. }) => RunSize::Round { diameter },
            (Size::Rect { width, height }, RunSize::Rect { roll, .. }) => RunSize::Rect { width, height, roll },
            _ => return Err(GraphError::ConnectorMismatch),
        };
        if dist(at, conn.pos) > TOL || dot(away, unit(conn.dir)) < 1.0 - 1e-6 || conn.domain != run.domain || !conn.system.compatible(run.system) {
            return Err(GraphError::ConnectorMismatch);
        }
        self.equipment.push(NetEquipment { element, connector: conn.name.clone(), domain: conn.domain, system: conn.system, flow_dir: conn.flow_dir, pos: conn.pos, dir: unit(conn.dir), size });
        let e = self.equipment.len() - 1;
        let sp = PortRef { node: NodeRef::Segment(segment), port: if end == End::Start { 0 } else { 1 } };
        let r = self.link(PortRef { node: NodeRef::Equipment(e), port: 0 }, sp);
        if r.is_err() {
            self.equipment.pop();
        }
        r.map(|_| e)
    }


    /// Follow the pieces from an equipment connector until a junction (a fitting with other than two legs, or
    /// other equipment) or an open end.
    pub fn chain_from_equipment(&self, equip: usize) -> Result<Chain, GraphError> {
        if equip >= self.equipment.len() {
            return Err(GraphError::NoSuchEquipment(equip));
        }
        let mut nodes = Vec::new();
        let mut port = PortRef { node: NodeRef::Equipment(equip), port: 0 };
        for _ in 0..=self.segments.len() + self.fittings.len() {
            let Some(q) = self.link_of(port) else {
                return Ok(Chain { nodes, end: ChainEnd::Open });
            };
            match q.node {
                NodeRef::Equipment(_) => return Ok(Chain { nodes, end: ChainEnd::Port(q) }),
                NodeRef::Fitting(i) if self.port_count(q.node) != 2 => {
                    let _ = i;
                    return Ok(Chain { nodes, end: ChainEnd::Port(q) });
                }
                NodeRef::Segment(i) if self.taps.iter().any(|t| t.segment == i) => return Err(GraphError::TapOnChain(i)),
                _ => {}
            }
            nodes.push(q.node);
            port = PortRef { node: q.node, port: 1 - q.port };
        }
        Err(GraphError::Corrupt("a chain of pieces loops"))
    }

    /// Take runs and fittings out, with every link that touched them and the taps on them. The numbers of
    /// the other pieces move up to fill the gaps; their element numbers (ids) stay.
    pub fn remove_nodes(&mut self, nodes: &[NodeRef]) {
        let gone_seg: std::collections::HashSet<usize> = nodes.iter().filter_map(|n| if let NodeRef::Segment(i) = n { Some(*i) } else { None }).collect();
        let gone_fit: std::collections::HashSet<usize> = nodes.iter().filter_map(|n| if let NodeRef::Fitting(i) = n { Some(*i) } else { None }).collect();
        let map = |len: usize, gone: &std::collections::HashSet<usize>| -> Vec<Option<usize>> {
            let mut next = 0;
            (0..len)
                .map(|i| {
                    if gone.contains(&i) {
                        None
                    } else {
                        next += 1;
                        Some(next - 1)
                    }
                })
                .collect()
        };
        let (sm, fm) = (map(self.segments.len(), &gone_seg), map(self.fittings.len(), &gone_fit));
        let remap = |p: PortRef| -> Option<PortRef> {
            let node = match p.node {
                NodeRef::Segment(i) => NodeRef::Segment((*sm.get(i)?)?),
                NodeRef::Fitting(i) => NodeRef::Fitting((*fm.get(i)?)?),
                e => e,
            };
            Some(PortRef { node, port: p.port })
        };
        let mut i = 0;
        self.segments.retain(|_| {
            i += 1;
            !gone_seg.contains(&(i - 1))
        });
        let mut i = 0;
        self.fittings.retain(|_| {
            i += 1;
            !gone_fit.contains(&(i - 1))
        });
        self.links = self
            .links
            .iter()
            .filter_map(|&(a, b)| {
                let (a, b) = (remap(a)?, remap(b)?);
                Some(if a <= b { (a, b) } else { (b, a) })
            })
            .collect();
        self.taps = self.taps.iter().filter_map(|t| Some(Tapping { fitting: (*fm.get(t.fitting)?)?, segment: (*sm.get(t.segment)?)?, distance: t.distance })).collect();
    }

    /// Replace the pieces of a chain by a new route that starts at the moved equipment connector `conn`
    /// and ends at the chain's junction. The new runs and fittings take over the element numbers of the old
    /// ones in order (so their ids stay), and get new numbers only when there are more of them. Nothing changes
    /// when this fails. The route must already end where the junction port is, facing into it.
    pub fn replace_chain(&mut self, equip: usize, conn: &Connector, chain: &Chain, route: &Route) -> Result<Added, GraphError> {
        let snapshot = self.clone();
        let r = self.replace_chain_inner(equip, conn, chain, route);
        if r.is_err() {
            *self = snapshot;
        }
        r
    }

    fn replace_chain_inner(&mut self, equip: usize, conn: &Connector, chain: &Chain, route: &Route) -> Result<Added, GraphError> {
        let old = self.equipment.get(equip).ok_or(GraphError::NoSuchEquipment(equip))?.clone();
        if conn.domain != old.domain || !conn.system.compatible(old.system) {
            return Err(GraphError::EquipmentMismatch);
        }
        let mut seg_ids: Vec<u64> = Vec::new();
        let mut fit_ids: Vec<u64> = Vec::new();
        for n in &chain.nodes {
            match n {
                NodeRef::Segment(i) => seg_ids.push(self.segments.get(*i).ok_or(GraphError::NoSuchPort(PortRef { node: *n, port: 0 }))?.id),
                NodeRef::Fitting(i) => fit_ids.push(self.fittings.get(*i).ok_or(GraphError::NoSuchPort(PortRef { node: *n, port: 0 }))?.id),
                NodeRef::Equipment(_) => return Err(GraphError::NoSuchPort(PortRef { node: *n, port: 0 })),
            }
        }
        // the junction port is a node that stays; find it again after the numbers move
        let end_port = match chain.end {
            ChainEnd::Port(p) => Some(p),
            ChainEnd::Open => None,
        };
        let end_after = end_port.map(|p| {
            let shift = |i: usize, gone: &dyn Fn(&NodeRef) -> bool, mk: &dyn Fn(usize) -> NodeRef| i - (0..i).filter(|k| gone(&mk(*k))).count();
            let in_chain = |n: &NodeRef| chain.nodes.contains(n);
            let node = match p.node {
                NodeRef::Segment(i) => NodeRef::Segment(shift(i, &in_chain, &NodeRef::Segment)),
                NodeRef::Fitting(i) => NodeRef::Fitting(shift(i, &in_chain, &NodeRef::Fitting)),
                e => e,
            };
            PortRef { node, port: p.port }
        });
        self.remove_nodes(&chain.nodes);
        // the equipment connector now sits where the new connector is
        let size = crate::conn_run_size(conn).map_err(|_| GraphError::ConnectorMismatch)?;
        {
            let e = &mut self.equipment[equip];
            e.pos = conn.pos;
            e.dir = unit(conn.dir);
            e.size = size;
        }
        self.check_room(route.runs.len() + route.fittings.len())?;
        let mut added = Added::default();
        let (mut si, mut fi) = (seg_ids.into_iter(), fit_ids.into_iter());
        for r in &route.runs {
            let id = si.next().unwrap_or_else(|| self.take_id());
            self.segments.push(NetSegment { id, run: r.clone() });
            added.segments.push(self.segments.len() - 1);
        }
        let (domain, system) = (old.domain, old.system);
        for f in &route.fittings {
            let id = fi.next().unwrap_or_else(|| self.take_id());
            self.fittings.push(NetFitting { id, fitting: f.clone(), domain, system });
            added.fittings.push(self.fittings.len() - 1);
        }
        let nodes: Vec<NodeRef> = added.segments.iter().map(|i| NodeRef::Segment(*i)).chain(added.fittings.iter().map(|i| NodeRef::Fitting(*i))).collect();
        self.auto_link(&nodes)?;
        let (first, last) = match (added.segments.first(), added.segments.last()) {
            (Some(&f), Some(&l)) => (f, l),
            _ => return Err(GraphError::ConnectorMismatch),
        };
        self.link(PortRef { node: NodeRef::Equipment(equip), port: 0 }, PortRef { node: NodeRef::Segment(first), port: 0 })?;
        if let Some(p) = end_after {
            self.link(PortRef { node: NodeRef::Segment(last), port: 1 }, p)?;
        }
        self.validate()?;
        Ok(added)
    }

    /// Ports that are not linked: run ends and fitting legs that lead nowhere.
    pub fn open_ports(&self) -> Vec<PortRef> {
        let mut out = Vec::new();
        let nodes = (0..self.segments.len()).map(NodeRef::Segment).chain((0..self.fittings.len()).map(NodeRef::Fitting)).chain((0..self.equipment.len()).map(NodeRef::Equipment));
        for n in nodes {
            for k in 0..self.port_count(n) {
                let p = PortRef { node: n, port: k };
                if self.link_of(p).is_none() {
                    out.push(p);
                }
            }
        }
        out
    }

    fn all_nodes(&self) -> Vec<NodeRef> {
        (0..self.segments.len()).map(NodeRef::Segment).chain((0..self.fittings.len()).map(NodeRef::Fitting)).chain((0..self.equipment.len()).map(NodeRef::Equipment)).collect()
    }

    /// Nodes that touch this one: through links, and through taps (a tap fitting and the run it sits on).
    pub fn neighbours(&self, n: NodeRef) -> Vec<NodeRef> {
        let mut out = Vec::new();
        for (a, b) in &self.links {
            if a.node == n {
                out.push(b.node);
            } else if b.node == n {
                out.push(a.node);
            }
        }
        for t in &self.taps {
            if n == NodeRef::Fitting(t.fitting) {
                out.push(NodeRef::Segment(t.segment));
            } else if n == NodeRef::Segment(t.segment) {
                out.push(NodeRef::Fitting(t.fitting));
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Groups of nodes that are connected to each other, each sorted, the groups in order of their first node.
    pub fn components(&self) -> Vec<Vec<NodeRef>> {
        let nodes = self.all_nodes();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for &n in &nodes {
            if seen.contains(&n) {
                continue;
            }
            let mut comp = vec![n];
            seen.insert(n);
            let mut i = 0;
            while i < comp.len() {
                for m in self.neighbours(comp[i]) {
                    if seen.insert(m) {
                        comp.push(m);
                    }
                }
                i += 1;
            }
            comp.sort();
            out.push(comp);
        }
        out
    }

    /// Fewest pieces from one node to another, both included; None if they are not connected.
    pub fn path(&self, from: NodeRef, to: NodeRef) -> Option<Vec<NodeRef>> {
        if self.port_count(from) == 0 || self.port_count(to) == 0 {
            return None;
        }
        let mut prev = std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::from([from]);
        prev.insert(from, from);
        while let Some(n) = queue.pop_front() {
            if n == to {
                let mut p = vec![to];
                let mut c = to;
                while c != from {
                    c = prev[&c];
                    p.push(c);
                }
                p.reverse();
                return Some(p);
            }
            for m in self.neighbours(n) {
                if let std::collections::hash_map::Entry::Vacant(v) = prev.entry(m) {
                    v.insert(n);
                    queue.push_back(m);
                }
            }
        }
        None
    }

    /// Check the whole graph: every link joins two existing, distinct, free ports that meet (same place,
    /// facing each other) with the same size, domain and system; no port is used twice; taps lie on their run.
    pub fn validate(&self) -> Result<(), GraphError> {
        let mut used = std::collections::HashSet::new();
        for &(a, b) in &self.links {
            for p in [a, b] {
                if p.port >= self.port_count(p.node) {
                    return Err(GraphError::NoSuchPort(p));
                }
                if !used.insert(p) {
                    return Err(GraphError::PortTaken(p));
                }
            }
            if a.node == b.node {
                return Err(GraphError::SelfLink(a));
            }
            let (da, db) = (self.node_domain_system(a.node).ok_or(GraphError::NoSuchPort(a))?, self.node_domain_system(b.node).ok_or(GraphError::NoSuchPort(b))?);
            if da.0 != db.0 || !da.1.compatible(db.1) {
                return Err(GraphError::SystemMismatch(a, b));
            }
            if !same_size(self.port_size(a).ok_or(GraphError::NoSuchPort(a))?, self.port_size(b).ok_or(GraphError::NoSuchPort(b))?) {
                return Err(GraphError::SizeMismatch(a, b));
            }
            let (pa, pb) = (self.port_pos(a).ok_or(GraphError::NoSuchPort(a))?, self.port_pos(b).ok_or(GraphError::NoSuchPort(b))?);
            let (xa, xb) = (self.port_dir(a).ok_or(GraphError::NoSuchPort(a))?, self.port_dir(b).ok_or(GraphError::NoSuchPort(b))?);
            if dist(pa, pb) > TOL || dot(xa, xb) > -1.0 + 1e-6 {
                return Err(GraphError::NotMeeting(a, b));
            }
        }
        for t in &self.taps {
            let (f, s) = match (self.fittings.get(t.fitting), self.segments.get(t.segment)) {
                (Some(f), Some(s)) => (f, s),
                _ => return Err(GraphError::NotATap),
            };
            if !matches!(f.fitting.kind, FittingKind::Tap { .. }) {
                return Err(GraphError::NotATap);
            }
            let d = unit(sub(s.run.b, s.run.a));
            let at = [s.run.a[0] + d[0] * t.distance, s.run.a[1] + d[1] * t.distance, s.run.a[2] + d[2] * t.distance];
            if dist(at, f.fitting.at) > TOL || t.distance < -TOL || t.distance > dist(s.run.a, s.run.b) + TOL {
                return Err(GraphError::NotOnRun);
            }
        }
        Ok(())
    }
}

// ---------- saving ----------

const MAGIC: &[u8; 4] = b"OBNW";
const VERSION: u8 = 2;
const MAX_STR: usize = 4096;
const MAX_PORTS: usize = 16;

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn v3(&mut self, v: [f64; 3]) {
        for x in v {
            self.f(x);
        }
    }
    fn s(&mut self, v: &str) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v.as_bytes());
    }
    fn size(&mut self, s: RunSize) {
        match s {
            RunSize::Round { diameter } => {
                self.u8(0);
                self.f(diameter);
            }
            RunSize::Rect { width, height, roll } => {
                self.u8(1);
                self.f(width);
                self.f(height);
                self.f(roll);
            }
        }
    }
    fn port(&mut self, p: PortRef) {
        let (t, i) = match p.node {
            NodeRef::Segment(i) => (0u8, i),
            NodeRef::Fitting(i) => (1, i),
            NodeRef::Equipment(i) => (2, i),
        };
        self.u8(t);
        self.u32(i as u32);
        self.u32(p.port as u32);
    }
}

struct Rd<'a>(&'a [u8], usize);
impl Rd<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], GraphError> {
        let end = self.1.checked_add(n).filter(|e| *e <= self.0.len()).ok_or(GraphError::Corrupt("ends too soon"))?;
        let s = &self.0[self.1..end];
        self.1 = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, GraphError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, GraphError> {
        let mut b = [0u8; 4];
        b.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(b))
    }
    fn u64(&mut self) -> Result<u64, GraphError> {
        let mut b = [0u8; 8];
        b.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(b))
    }
    fn f(&mut self) -> Result<f64, GraphError> {
        let v = f64::from_bits(self.u64()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(GraphError::Corrupt("a number is not finite"))
        }
    }
    fn v3(&mut self) -> Result<[f64; 3], GraphError> {
        Ok([self.f()?, self.f()?, self.f()?])
    }
    fn count(&mut self) -> Result<usize, GraphError> {
        let n = self.u32()? as usize;
        if n > MAX_NODES {
            return Err(GraphError::Corrupt("too many items"));
        }
        Ok(n)
    }
    fn s(&mut self) -> Result<String, GraphError> {
        let n = self.u32()? as usize;
        if n > MAX_STR {
            return Err(GraphError::Corrupt("text too long"));
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| GraphError::Corrupt("text is not UTF-8"))
    }
    fn size(&mut self) -> Result<RunSize, GraphError> {
        match self.u8()? {
            0 => Ok(RunSize::Round { diameter: self.f()? }),
            1 => Ok(RunSize::Rect { width: self.f()?, height: self.f()?, roll: self.f()? }),
            _ => Err(GraphError::Corrupt("unknown size kind")),
        }
    }
    fn domain(&mut self) -> Result<Domain, GraphError> {
        Domain::from_code(self.u8()?).ok_or(GraphError::Corrupt("unknown domain"))
    }
    fn system(&mut self) -> Result<System, GraphError> {
        System::from_code(self.u8()?).ok_or(GraphError::Corrupt("unknown system"))
    }
    fn port(&mut self) -> Result<PortRef, GraphError> {
        let t = self.u8()?;
        let i = self.u32()? as usize;
        let port = self.u32()? as usize;
        let node = match t {
            0 => NodeRef::Segment(i),
            1 => NodeRef::Fitting(i),
            2 => NodeRef::Equipment(i),
            _ => return Err(GraphError::Corrupt("unknown node kind")),
        };
        Ok(PortRef { node, port })
    }
}

impl Network {
    /// The saved form: a small binary record, byte for byte the same for the same network.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.0.extend_from_slice(MAGIC);
        w.u8(VERSION);
        w.u64(self.next_id);
        w.u32(self.segments.len() as u32);
        for s in &self.segments {
            w.u64(s.id);
            w.s(&s.run.label);
            w.u8(s.run.domain.code());
            w.u8(s.run.system.code());
            w.v3(s.run.a);
            w.v3(s.run.b);
            w.size(s.run.size);
            w.f(s.run.margin);
        }
        w.u32(self.fittings.len() as u32);
        for f in &self.fittings {
            w.u64(f.id);
            w.u8(f.domain.code());
            w.u8(f.system.code());
            w.s(&f.fitting.type_name);
            w.s(&f.fitting.part_code);
            let (k, a) = match f.fitting.kind {
                FittingKind::Elbow { angle } => (0u8, angle),
                FittingKind::Transition => (1, 0.0),
                FittingKind::Tee { branch_angle } => (2, branch_angle),
                FittingKind::Tap { branch_angle } => (3, branch_angle),
            };
            w.u8(k);
            w.f(a);
            w.v3(f.fitting.at);
            w.f(f.fitting.radius);
            w.u32(f.fitting.ports.len() as u32);
            for i in 0..f.fitting.ports.len() {
                w.v3(f.fitting.ports[i]);
                w.size(f.fitting.port_sizes[i]);
                w.f(f.fitting.trim[i]);
            }
        }
        w.u32(self.equipment.len() as u32);
        for e in &self.equipment {
            w.u64(e.element);
            w.s(&e.connector);
            w.u8(e.domain.code());
            w.u8(e.system.code());
            w.u8(e.flow_dir.code());
            w.v3(e.pos);
            w.v3(e.dir);
            w.size(e.size);
        }
        w.u32(self.links.len() as u32);
        for (a, b) in &self.links {
            w.port(*a);
            w.port(*b);
        }
        w.u32(self.taps.len() as u32);
        for t in &self.taps {
            w.u32(t.fitting as u32);
            w.u32(t.segment as u32);
            w.f(t.distance);
        }
        w.0
    }

    /// Read a saved network. Everything is checked: bad bytes give an error, never a broken graph.
    pub fn from_bytes(b: &[u8]) -> Result<Network, GraphError> {
        let mut r = Rd(b, 0);
        if r.take(4)? != MAGIC {
            return Err(GraphError::Corrupt("not a network record"));
        }
        let version = r.u8()?;
        if version != VERSION && version != 1 {
            return Err(GraphError::Corrupt("unknown version"));
        }
        let mut n = Network::new(r.u64()?);
        for _ in 0..r.count()? {
            let id = r.u64()?;
            let label = r.s()?;
            let (domain, system) = (r.domain()?, r.system()?);
            let (a, bb) = (r.v3()?, r.v3()?);
            let size = r.size()?;
            let margin = r.f()?;
            n.segments.push(NetSegment { id, run: Run { label, domain, system, a, b: bb, size, margin } });
        }
        for _ in 0..r.count()? {
            let id = r.u64()?;
            let (domain, system) = (r.domain()?, r.system()?);
            let type_name = r.s()?;
            let part_code = r.s()?;
            let (k, ang) = (r.u8()?, r.f()?);
            let kind = match k {
                0 => FittingKind::Elbow { angle: ang },
                1 => FittingKind::Transition,
                2 => FittingKind::Tee { branch_angle: ang },
                3 => FittingKind::Tap { branch_angle: ang },
                _ => return Err(GraphError::Corrupt("unknown fitting kind")),
            };
            let at = r.v3()?;
            let radius = r.f()?;
            let np = r.u32()? as usize;
            if np > MAX_PORTS {
                return Err(GraphError::Corrupt("too many ports"));
            }
            let (mut ports, mut port_sizes, mut trim) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..np {
                ports.push(r.v3()?);
                port_sizes.push(r.size()?);
                trim.push(r.f()?);
            }
            n.fittings.push(NetFitting { id, domain, system, fitting: Fitting { type_name, part_code, kind, at, ports, port_sizes, radius, trim } });
        }
        for _ in 0..r.count()? {
            let element = r.u64()?;
            let connector = r.s()?;
            let (domain, system) = (r.domain()?, r.system()?);
            // version 1 did not say, and a connector that does not say passes the medium both ways
            let flow_dir = if version >= 2 { FlowDir::from_code(r.u8()?).ok_or(GraphError::Corrupt("unknown flow direction"))? } else { FlowDir::Both };
            let (pos, dir) = (r.v3()?, r.v3()?);
            let size = r.size()?;
            n.equipment.push(NetEquipment { element, connector, domain, system, flow_dir, pos, dir, size });
        }
        for _ in 0..r.count()? {
            let (a, bb) = (r.port()?, r.port()?);
            n.links.push((a, bb));
        }
        for _ in 0..r.count()? {
            let fitting = r.u32()? as usize;
            let segment = r.u32()? as usize;
            let distance = r.f()?;
            n.taps.push(Tapping { fitting, segment, distance });
        }
        if r.1 != b.len() {
            return Err(GraphError::Corrupt("bytes left over"));
        }
        if n.segments.iter().any(|s| dist(s.run.a, s.run.b) < 1e-9) {
            return Err(GraphError::Corrupt("a run has no length"));
        }
        if n.fittings.iter().any(|f| f.fitting.ports.is_empty() || f.fitting.ports.iter().any(|p| (dot(*p, *p).sqrt() - 1.0).abs() > 1e-6)) {
            return Err(GraphError::Corrupt("a fitting leg is not a unit direction"));
        }
        if n.equipment.iter().any(|e| (dot(e.dir, e.dir).sqrt() - 1.0).abs() > 1e-6) {
            return Err(GraphError::Corrupt("a connector direction is not a unit vector"));
        }
        n.validate()?;
        Ok(n)
    }
}
