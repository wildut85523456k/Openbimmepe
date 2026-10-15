//! Re-routing when equipment moves. The pieces between the moved connector and the next junction (a tee or tap
//! leg, or other equipment) are replaced by a new automatic route from the connector's new place; everything
//! beyond the junction stays exactly as it was. The new runs and fittings take over the element numbers of the
//! old ones in order, so the ids of the pipe (and its IFC GlobalIds) stay. The rest of the network is avoided as
//! hard obstacles, except the runs that meet the junction, which the new route may run into the way a pipe does.
//! If anything fails the network is left exactly as it was.
use crate::auto::{auto_route, AutoError, AutoOptions, AutoRequest, AutoRoute, Zone};
use crate::graph::{Added, Chain, ChainEnd, GraphError, Network, NodeRef, PortRef};
use crate::{ref_dim, RoutingPrefs};
use openbim_family::mep::{up_for, Connector, FlowDir, Size};
use openbim_place::penetrations::RunSize;

#[derive(Clone, Debug, PartialEq)]
pub enum RerouteError {
    Graph(GraphError),
    Auto(AutoError),
    /// the pipe from this connector ends in the open: there is nothing to join the new route to
    OpenEnd,
}

impl std::fmt::Display for RerouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RerouteError {}

/// One connector that moved: which one (by its number in the network), where it is now, and how to route.
#[derive(Clone, Copy, Debug)]
pub struct RerouteRequest<'a> {
    pub equipment: usize,
    pub connector: &'a Connector,
    pub prefs: &'a RoutingPrefs,
    /// obstacles besides the network itself (equipment bodies, walls)
    pub zones: &'a [Zone],
    pub from_own: &'a [usize],
    pub to_own: &'a [usize],
    pub opts: &'a AutoOptions,
    /// keep clear of the other runs of the network
    pub avoid_network: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rerouted {
    pub route: AutoRoute,
    /// what was put in
    pub added: Added,
    /// element numbers of the pieces that went away and were not taken over by a new piece
    pub retired: Vec<u64>,
    /// how many pieces kept their element number
    pub kept: usize,
}

fn junction_connector(net: &Network, p: PortRef) -> Option<Connector> {
    let (domain, system) = net.node_domain_system(p.node)?;
    let (pos, dir, size) = (net.port_pos(p)?, net.port_dir(p)?, net.port_size(p)?);
    let (size, up) = match size {
        RunSize::Round { diameter } => (Size::Round { diameter }, up_for(dir, 0.0)),
        RunSize::Rect { width, height, roll } => (Size::Rect { width, height }, up_for(dir, roll)),
    };
    Some(Connector { name: "junction".into(), domain, system, flow_dir: FlowDir::Both, pos, dir, up, size, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 })
}

/// Route the pipe of one moved connector again.
pub fn reroute_equipment(net: &mut Network, rq: &RerouteRequest) -> Result<Rerouted, RerouteError> {
    let chain: Chain = net.chain_from_equipment(rq.equipment).map_err(RerouteError::Graph)?;
    let ChainEnd::Port(end) = chain.end else {
        return Err(RerouteError::OpenEnd);
    };
    let to = junction_connector(net, end).ok_or(RerouteError::Graph(GraphError::NoSuchPort(end)))?;
    // the size of the pipe: the largest on the way
    let size = chain
        .nodes
        .iter()
        .filter_map(|n| if let NodeRef::Segment(i) = n { Some(net.segments[*i].run.size) } else { None })
        .max_by(|a, b| ref_dim(*a).total_cmp(&ref_dim(*b)))
        .or_else(|| net.port_size(end))
        .ok_or(RerouteError::Graph(GraphError::NoSuchPort(end)))?;
    // obstacles: the caller's, then every run of the network that is not on the chain
    let mut zones: Vec<Zone> = rq.zones.to_vec();
    let mut to_own: Vec<usize> = rq.to_own.to_vec();
    if rq.avoid_network {
        // the runs that meet the junction are the junction's own
        let mut own_nodes: Vec<NodeRef> = Vec::new();
        for k in 0..net.port_count(end.node) {
            if let Some(q) = net.link_of(PortRef { node: end.node, port: k }) {
                own_nodes.push(q.node);
            }
        }
        for (i, s) in net.segments.iter().enumerate() {
            let n = NodeRef::Segment(i);
            if chain.nodes.contains(&n) {
                continue;
            }
            if let Ok(z) = Zone::from_run(&s.run) {
                if own_nodes.contains(&n) {
                    to_own.push(zones.len());
                }
                zones.push(z);
            }
        }
    }
    let route = auto_route(&AutoRequest { from: rq.connector, to: &to, size, prefs: rq.prefs, zones: &zones, from_own: rq.from_own, to_own: &to_own, opts: rq.opts }).map_err(RerouteError::Auto)?;
    let old_segments: Vec<u64> = chain.nodes.iter().filter_map(|n| if let NodeRef::Segment(i) = n { Some(net.segments[*i].id) } else { None }).collect();
    let old_fittings: Vec<u64> = chain.nodes.iter().filter_map(|n| if let NodeRef::Fitting(i) = n { Some(net.fittings[*i].id) } else { None }).collect();
    let added = net.replace_chain(rq.equipment, rq.connector, &chain, &route.route).map_err(RerouteError::Graph)?;
    let (ns, nf) = (added.segments.len(), added.fittings.len());
    let mut retired: Vec<u64> = old_segments.iter().skip(ns).copied().collect();
    retired.extend(old_fittings.iter().skip(nf).copied());
    let kept = ns.min(old_segments.len()) + nf.min(old_fittings.len());
    Ok(Rerouted { route, added, retired, kept })
}

/// Route several moved connectors, one after another (each sees the new pipes of the ones before it). All or nothing:
/// when one fails the network is as it was and the error says which one.
pub fn reroute_many(net: &mut Network, reqs: &[RerouteRequest]) -> Result<Vec<Rerouted>, (usize, RerouteError)> {
    let snapshot = net.clone();
    let mut out = Vec::with_capacity(reqs.len());
    for (i, rq) in reqs.iter().enumerate() {
        match reroute_equipment(net, rq) {
            Ok(r) => out.push(r),
            Err(e) => {
                *net = snapshot;
                return Err((i, e));
            }
        }
    }
    Ok(out)
}
