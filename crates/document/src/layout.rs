//! Generate Layout, as Revit has it for a mechanical or piping system: from the connectors of the equipment in a system
//! it proposes numbered solutions (a trunk with branches along X or along Y, or an obstacle-aware route between two
//! connectors), shown as placeholders, and Finish Layout turns the chosen one into real ducts, pipes and fittings in one
//! undoable step. A solution is only a proposal until then: nothing is in the model.
//!
//! What is not as in Revit: only the Network type (and the routed one for two connectors) exists, not Perimeter or
//! Intersections; all connectors must face along an axis, point sideways (not up or down) and be at the height of the
//! base's connector, because the model holds no vertical ducts or pipes yet; the placeholders are not saved.
use crate::{Command, Document, ElementId, EquipLink, MepNode, MepPort};
use openbim_family::mep::Connector;
use openbim_place::penetrations::RunSize;
use openbim_route::auto::{auto_route, AutoOptions, AutoRequest, Zone};
use openbim_route::snaps::{SnapSettings, Snaps};
use openbim_route::{branch_from_run, conn_run_size, ref_dim, route_path, Route, RoutingPrefs};

type V3 = [f64; 3];

/// A connector of the equipment of a system, in the model.
#[derive(Clone, Debug)]
pub struct LayoutConn {
    pub element: ElementId,
    pub conn: Connector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolutionKind {
    /// a trunk along X with the other connectors tapped off it
    NetworkAlongX,
    /// a trunk along Y
    NetworkAlongY,
    /// the shortest path between two connectors that goes round walls (they cost to cross)
    Routed,
}

/// A path of straight legs, from its first point to its last, with the size of each leg.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutPath {
    pub points: Vec<V3>,
    pub sizes: Vec<RunSize>,
}

#[derive(Clone, Debug)]
pub struct LayoutBranch {
    /// from the point on the trunk to the connector
    pub path: LayoutPath,
    pub end: (ElementId, String),
}

/// One solution: what Finish Layout would place.
#[derive(Clone, Debug)]
pub struct Layout {
    pub label: String,
    pub kind: SolutionKind,
    pub trunk: LayoutPath,
    /// the equipment connectors at the two ends of the trunk
    pub trunk_ends: [(ElementId, String); 2],
    /// the trunk as the router drew it (a routed solution comes with it)
    pub trunk_route: Route,
    pub branches: Vec<LayoutBranch>,
    /// connectors left out, with the reason
    pub unconnected: Vec<(ElementId, String)>,
    pub length_m: f64,
}

/// What Finish Layout did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceReport {
    pub runs: Vec<ElementId>,
    /// equipment connectors joined to the new ducts or pipes
    pub connected: usize,
    pub failed: Vec<(ElementId, String)>,
}

fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn len(a: V3) -> f64 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
fn add_s(a: V3, d: V3, s: f64) -> V3 { [a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s] }
fn unit(a: V3) -> V3 { let l = len(a).max(1e-12); [a[0] / l, a[1] / l, a[2] / l] }

fn path_length(p: &[V3]) -> f64 { p.windows(2).map(|w| len(sub(w[1], w[0]))).sum() }

/// The same size on the same domain (a round pipe or a rectangular duct that is not turned).
fn same_kind(a: RunSize, b: RunSize) -> bool {
    matches!((a, b), (RunSize::Round { .. }, RunSize::Round { .. }) | (RunSize::Rect { .. }, RunSize::Rect { .. }))
}

fn trim_path(points: Vec<V3>, sizes: Vec<RunSize>) -> LayoutPath {
    // a leg of no length goes, and with it its size
    let mut p: Vec<V3> = vec![points[0]];
    let mut s: Vec<RunSize> = Vec::new();
    for i in 1..points.len() {
        if len(sub(points[i], *p.last().unwrap_or(&points[0]))) < 1e-6 { continue; }
        p.push(points[i]);
        s.push(sizes[i - 1]);
    }
    LayoutPath { points: p, sizes: s }
}

impl Document {
    /// The soft zones of the walls of a level: a path may cross a wall, at a price.
    fn wall_zones(&self, level: ElementId) -> Vec<Zone> {
        let z0 = self.get(level).map_or(0.0, |l| l.elevation_m);
        self.elements().filter_map(|w| {
            if w.category != crate::Category::Wall { return None; }
            let Some(crate::Geometry::Linear { level: l, .. }) = w.geometry else { return None };
            if l != level { return None; }
            let r = self.wall_plan(w.id)?;
            Zone::soft("wall", &[r[0], r[1], r[2], r[3]], z0 + w.elevation_m, z0 + w.elevation_m + w.height_m.max(0.1), 5.0).ok()
        }).collect()
    }

    /// The solutions for the connectors of a system on `level`, best first, and why a kind of solution is not offered.
    /// `base` is the index in `conns` of the connector the layout starts from (it sizes the trunk and fixes the height).
    /// `trunk` overrides the size of the trunk (default: the largest connector). Each solution is tried out on a copy of
    /// the model, so what is offered can be built.
    pub fn generate_layouts(&self, level: ElementId, conns: &[LayoutConn], base: usize, trunk: Option<RunSize>, prefs: &RoutingPrefs) -> Result<(Vec<Layout>, Vec<String>), String> {
        let Some(b) = conns.get(base) else { return Err("there is nothing to start from: the system has no connector of its type".into()) };
        let z = b.conn.pos[2];
        let mut unconnected: Vec<(ElementId, String)> = Vec::new();
        let mut usable: Vec<usize> = Vec::new();
        for (i, c) in conns.iter().enumerate() {
            let d = unit(c.conn.dir);
            let sideways = d[2].abs() < 1e-6 && (d[0].abs() > 1.0 - 1e-6 || d[1].abs() > 1.0 - 1e-6);
            if !sideways { unconnected.push((c.element, format!("the connector '{}' does not face along an axis, or points up or down (vertical runs are not supported yet)", c.conn.name))); }
            else if (c.conn.pos[2] - z).abs() > 0.002 { unconnected.push((c.element, format!("the connector '{}' is at a different height from the base's ({:.0} mm against {:.0} mm)", c.conn.name, c.conn.pos[2] * 1000.0, z * 1000.0))); }
            else if conn_run_size(&c.conn).map_or(true, |s| !same_kind(s, conn_run_size(&b.conn).unwrap_or(s))) { unconnected.push((c.element, format!("the connector '{}' has another kind of section", c.conn.name))); }
            else { usable.push(i); }
        }
        if usable.len() < 2 || !usable.contains(&base) {
            let why = unconnected.first().map(|u| format!(": {}", u.1)).unwrap_or_default();
            return Err(format!("a layout needs at least two connectors that can be joined{why}"));
        }
        let largest = usable.iter().filter_map(|i| conn_run_size(&conns[*i].conn).ok()).max_by(|a, b| ref_dim(*a).total_cmp(&ref_dim(*b))).ok_or("the connectors have no section")?;
        let trunk_size = match trunk { Some(t) if same_kind(t, largest) => t, Some(_) => return Err("the trunk is of another kind of section than the connectors".into()), None => largest };
        let mut layouts = Vec::new();
        let mut why = Vec::new();
        let mut label_no = 0;
        let mut offer = |mut l: Layout, layouts: &mut Vec<Layout>| { label_no += 1; l.label = format!("{} {}: {}", "Solution", label_no, l.label); l.unconnected.extend(unconnected.iter().cloned()); layouts.push(l); };
        if usable.len() == 2 {
            match self.routed_layout(level, conns, &usable, trunk_size, prefs) { Ok(l) => offer(l, &mut layouts), Err(e) => why.push(format!("Routed: {e}")) }
        }
        for kind in [SolutionKind::NetworkAlongX, SolutionKind::NetworkAlongY] {
            match network_layout(conns, &usable, base, trunk_size, prefs, kind) { Ok(l) => offer(l, &mut layouts), Err(e) => why.push(format!("{}: {e}", if kind == SolutionKind::NetworkAlongX { "Network along X" } else { "Network along Y" })) }
        }
        // try each out on a copy: what cannot be built is reported with the solution
        let mut tried = Vec::new();
        for mut l in layouts {
            let mut copy = self.clone();
            match copy.place_layout(level, &l, prefs) {
                Ok(r) => { for (el, why) in r.failed { if !l.unconnected.iter().any(|u| u.0 == el && u.1 == why) { l.unconnected.push((el, why)); } } tried.push(l); }
                Err(e) => why.push(format!("{}: {e}", l.label)),
            }
        }
        if tried.is_empty() { return Err(format!("no solution can be built. {}", why.join(" "))); }
        Ok((tried, why))
    }

    fn routed_layout(&self, level: ElementId, conns: &[LayoutConn], usable: &[usize], size: RunSize, prefs: &RoutingPrefs) -> Result<Layout, String> {
        let (a, b) = (&conns[usable[0]], &conns[usable[1]]);
        let zones = self.wall_zones(level);
        let opts = AutoOptions { allow_vertical: false, ..AutoOptions::default() };
        let rq = AutoRequest { from: &a.conn, to: &b.conn, size, prefs, zones: &zones, from_own: &[], to_own: &[], opts: &opts };
        let r = auto_route(&rq).map_err(|e| format!("{e:?}"))?;
        let sizes = vec![size; r.waypoints.len().saturating_sub(1)];
        let crossed = if r.crossed.is_empty() { String::new() } else { format!(", crosses {} wall{}", r.crossed.len(), if r.crossed.len() == 1 { "" } else { "s" }) };
        let length = path_length(&r.waypoints);
        Ok(Layout {
            label: format!("routed between the two connectors, {length:.1} m{crossed}"),
            kind: SolutionKind::Routed,
            trunk: LayoutPath { points: r.waypoints.clone(), sizes },
            trunk_ends: [(a.element, a.conn.name.clone()), (b.element, b.conn.name.clone())],
            trunk_route: r.route,
            branches: Vec::new(),
            unconnected: Vec::new(),
            length_m: length,
        })
    }

    /// Finish Layout: the ducts, pipes and fittings of the solution, joined to the equipment connectors, as one undoable
    /// step. A branch that cannot be built is left out and reported; the trunk failing refuses the whole layout.
    pub fn place_layout(&mut self, level: ElementId, layout: &Layout, prefs: &RoutingPrefs) -> Result<PlaceReport, String> {
        let off = Snaps::new(SnapSettings::off());
        let mark = self.history_mark();
        let mut report = PlaceReport::default();
        let ids = match self.place_mep_route(level, &layout.trunk_route) { Ok(i) => i, Err(e) => return Err(format!("the trunk cannot be placed ({e:?})")) };
        report.runs.extend(ids.iter().copied());
        let mut pool = ids.clone();
        let mut links: Vec<(ElementId, usize, (ElementId, String))> = vec![(ids[0], 0, layout.trunk_ends[0].clone()), (ids[ids.len() - 1], 1, layout.trunk_ends[1].clone())];
        for br in &layout.branches {
            let tap = br.path.points[0];
            // the run of the trunk (or of an earlier branch) that the branch starts on
            let main = pool.iter().filter_map(|id| {
                let r = self.mep_run(*id)?;
                let (dm, ml) = (unit(sub(r.b, r.a)), len(sub(r.b, r.a)));
                if ml < 1e-6 { return None; }
                let s = sub(tap, r.a)[0] * dm[0] + sub(tap, r.a)[1] * dm[1] + sub(tap, r.a)[2] * dm[2];
                let foot = add_s(r.a, dm, s);
                let off_by = len(sub(foot, tap));
                (s > 0.0 && s < ml && off_by < 0.01).then_some((*id, off_by, r))
            }).min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((main_id, _, run)) = main else { report.failed.push((br.end.0, "no straight length of the trunk is at the place where its branch would start".into())); continue };
            let branch = match branch_from_run(&run, &br.path.points, &br.path.sizes, prefs, &off) {
                Ok(b) => b,
                Err(e) => { report.failed.push((br.end.0, format!("its branch cannot be drawn ({e})"))); continue }
            };
            match self.place_mep_branch(level, main_id, &branch) {
                Ok(new) => { report.runs.extend(new.iter().copied()); pool.extend(new.iter().copied()); if let Some(last) = new.last() { links.push((*last, 1, br.end.clone())); } }
                Err(e) => report.failed.push((br.end.0, format!("its branch is refused by the model ({e:?})"))),
            }
        }
        let mut data = self.mep().clone();
        for (id, port, (el, name)) in links {
            let mp = MepPort { node: MepNode::Run(id), port };
            if data.partner(mp).is_none() && !data.equip_links.iter().any(|q| q.port == mp) {
                data.equip_links.push(EquipLink { port: mp, element: el, connector: name });
                report.connected += 1;
            }
        }
        if let Err(e) = self.transact(self.revision(), &[Command::SetMep(Box::new(data))]) {
            self.roll_back_to(mark);
            return Err(format!("the connectors cannot be joined ({e:?})"));
        }
        self.merge_steps_since(mark);
        Ok(report)
    }
}

/// A trunk along one axis with the connectors tapped off it. The trunk lies beside the equipment, on the side each
/// connector faces; the two connectors that are furthest apart along it are its ends, the others branch off.
fn network_layout(conns: &[LayoutConn], usable: &[usize], base: usize, trunk_size: RunSize, prefs: &RoutingPrefs, kind: SolutionKind) -> Result<Layout, String> {
    let ax = if kind == SolutionKind::NetworkAlongX { 0 } else { 1 };
    let cr = 1 - ax;
    let z = conns[base].conn.pos[2];
    let stub = prefs.connector_stub.max(0.05);
    let bend_r = prefs.bend_radius_factor * ref_dim(trunk_size);
    // room for a bend, a reducer and a straight piece between them
    let m = 2.0 * bend_r + prefs.min_transition_len + 2.0 * prefs.min_straight + 0.15;
    struct Pt { idx: usize, p: V3, d: V3, tap_s: f64 }
    let pts: Vec<Pt> = usable.iter().map(|i| {
        let c = &conns[*i].conn;
        let d = unit(c.dir);
        let facing_trunk_axis = d[ax].abs() > 0.5;
        // a connector that faces along the trunk goes out far enough to change size on a straight length before it turns
        let out = if facing_trunk_axis { stub + m } else { stub };
        let tap_s = c.pos[ax] + d[ax] * out;
        Pt { idx: *i, p: c.pos, d, tap_s }
    }).collect();
    // the trunk lies on the side every connector that faces across it faces
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    for q in &pts {
        if q.d[cr] > 0.5 { lo = lo.max(q.p[cr] + stub + m); }
        else if q.d[cr] < -0.5 { hi = hi.min(q.p[cr] - stub - m); }
    }
    if lo > hi { return Err("the connectors face opposite sides of where the trunk would run".into()); }
    let mut across: Vec<f64> = pts.iter().map(|q| q.p[cr]).collect();
    across.sort_by(f64::total_cmp);
    let median = across[across.len() / 2];
    let t = median.clamp(lo, hi);
    // the ends of the trunk
    let (mut a_at, mut b_at) = (0usize, 0usize);
    for (k, q) in pts.iter().enumerate() {
        if q.tap_s < pts[a_at].tap_s { a_at = k; }
        if q.tap_s > pts[b_at].tap_s { b_at = k; }
    }
    if pts[b_at].tap_s - pts[a_at].tap_s < 1e-6 { return Err("the connectors are in line across the trunk: it would have no length".into()); }
    let at = |s: f64| -> V3 { let mut v = [0.0; 3]; v[ax] = s; v[cr] = t; v[2] = z; v };
    // from the connector to the trunk: its own size up to the end of its stub, then `after`
    let approach = |q: &Pt, after: RunSize, conn_size: RunSize| -> (Vec<V3>, Vec<RunSize>) {
        let facing_trunk_axis = q.d[ax].abs() > 0.5;
        let mut points = vec![q.p, add_s(q.p, q.d, stub)];
        if facing_trunk_axis { points.push(add_s(q.p, q.d, stub + m)); }
        points.push(at(q.tap_s));
        let mut sizes = vec![conn_size];
        sizes.extend(std::iter::repeat_n(after, points.len() - 2));
        (points, sizes)
    };
    let size_of = |q: &Pt| conn_run_size(&conns[q.idx].conn).unwrap_or(trunk_size);
    let (qa, qb) = (&pts[a_at], &pts[b_at]);
    let (pa, sa) = approach(qa, trunk_size, size_of(qa));
    let (pb, sb) = approach(qb, trunk_size, size_of(qb));
    let mut points = pa;
    let mut sizes = sa;
    sizes.push(trunk_size);
    points.extend(pb.into_iter().rev());
    sizes.extend(sb.into_iter().rev());
    let trunk = trim_path(points, sizes);
    let off = Snaps::new(SnapSettings::off());
    let trunk_route = route_path(&trunk.points, &trunk.sizes, prefs, &off).map_err(|e| format!("the trunk cannot be drawn ({e})"))?;
    let mut branches = Vec::new();
    for (k, q) in pts.iter().enumerate() {
        if k == a_at || k == b_at { continue; }
        let s = size_of(q);
        let (pts_to_trunk, sizes) = approach(q, s, s);
        // from the trunk to the connector
        let path = trim_path(pts_to_trunk.into_iter().rev().collect(), sizes.into_iter().rev().collect());
        branches.push(LayoutBranch { path, end: (conns[q.idx].element, conns[q.idx].conn.name.clone()) });
    }
    let length = path_length(&trunk.points) + branches.iter().map(|b| path_length(&b.path.points)).sum::<f64>();
    Ok(Layout {
        label: format!("network, trunk along {} at {:.2} m, {} branch{}, {length:.1} m", if ax == 0 { "X" } else { "Y" }, t, branches.len(), if branches.len() == 1 { "" } else { "es" }),
        kind,
        trunk,
        trunk_ends: [(conns[qa.idx].element, conns[qa.idx].conn.name.clone()), (conns[qb.idx].element, conns[qb.idx].conn.name.clone())],
        trunk_route,
        branches,
        unconnected: Vec::new(),
        length_m: length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FamilySpec, Geometry};
    use openbim_family::mep::{Domain, FlowDir, Size, System};
    use openbim_route::{AngleSet, Junction};

    fn prefs(domain: Domain, system: System) -> RoutingPrefs {
        RoutingPrefs {
            domain, system, label: "L".into(), elbow_angles: AngleSet::Any, bend_radius_factor: if domain == Domain::Duct { 1.0 } else { 1.5 }, min_straight: 0.0,
            junction: Junction::Tee, tee_angles: AngleSet::Any, tee_half_factor: 1.0, branch_stub_factor: 1.0, pick_tolerance: 0.02,
            transition_angle_deg: 30.0, min_transition_len: 0.1, connector_stub: 0.5, margin: 0.0, catalogue: None,
        }
    }
    fn pipe_conn(pos: V3, dir: V3, d: f64) -> Connector {
        Connector { name: "chw_in".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::In, pos, dir, up: [0.0, 0.0, 1.0], size: Size::Round { diameter: d }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
    }
    fn level(d: &Document) -> ElementId { match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() } }
    fn units(d: &mut Document, at: &[[f64; 2]]) -> Vec<ElementId> {
        let l = level(d);
        at.iter().map(|p| d.place_family(l, *p, &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 }).unwrap()).collect()
    }

    #[test]
    fn a_network_runs_a_trunk_beside_a_column_of_equipment_and_taps_each_connector_off_it() {
        let mut d = Document::spatial_sample();
        let ys = [20.0, 23.0, 26.0, 29.0];
        let els = units(&mut d, &ys.map(|y| [20.0, y]));
        let conns: Vec<LayoutConn> = els.iter().zip(ys).map(|(e, y)| LayoutConn { element: *e, conn: pipe_conn([21.0, y, 2.7], [1.0, 0.0, 0.0], if y == 20.0 { 0.065 } else { 0.05 }) }).collect();
        let pf = prefs(Domain::Pipe, System::ChilledWaterSupply);
        let lv = level(&d);
        let (layouts, why) = d.generate_layouts(lv, &conns, 0, None, &pf).unwrap();
        // along X the connectors are in line across the trunk: no length; along Y it works
        assert_eq!(layouts.len(), 1, "{:?}", why.iter().chain(layouts.iter().map(|l| &l.label)).collect::<Vec<_>>());
        assert!(why.iter().any(|w| w.starts_with("Network along X")), "{why:?}");
        let l = &layouts[0];
        assert_eq!(l.kind, SolutionKind::NetworkAlongY);
        assert!(l.label.starts_with("Solution 1: network, trunk along Y") && l.branches.len() == 2, "{}", l.label);
        assert!(l.unconnected.is_empty(), "{:?}", l.unconnected);
        // the trunk is the size of the largest connector, the branches the size of their connector
        assert!(l.trunk.sizes.iter().any(|s| matches!(s, RunSize::Round { diameter } if (*diameter - 0.065).abs() < 1e-9)));
        assert!(l.branches.iter().all(|b| b.path.sizes.iter().all(|s| matches!(s, RunSize::Round { diameter } if (*diameter - 0.05).abs() < 1e-9))));
        // the trunk stands to the right of every connector (they face +X) and each branch starts on it
        let t = l.trunk.points.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!(t > 21.5, "{t}");
        assert!(l.branches.iter().all(|b| (b.path.points[0][0] - t).abs() < 1e-9));
        // Finish Layout: runs, fittings and equipment links, one undo step
        let before = (d.elements().count(), d.mep().fittings.len(), d.mep().equip_links.len());
        let steps = d.history_mark();
        let r = d.place_layout(lv, l, &pf).unwrap();
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        assert_eq!(r.connected, 4);
        assert_eq!(d.mep().equip_links.len(), before.2 + 4);
        assert!(d.elements().count() > before.0 + 4 && d.mep().fittings.len() > before.1 + 3);
        assert_eq!(d.history_mark(), steps + 1, "one undo step");
        // every new run is on the system of the layout, and each connector has its link
        assert!(r.runs.iter().all(|id| d.mep().run_system(*id) == Some(System::ChilledWaterSupply)));
        for e in &els { assert!(d.mep().equip_links.iter().any(|q| q.element == *e), "{e:?}"); }
        d.undo().unwrap();
        assert_eq!((d.elements().count(), d.mep().fittings.len(), d.mep().equip_links.len()), before);
    }

    #[test]
    fn connectors_that_cannot_be_joined_are_left_out_with_the_reason_and_two_connectors_are_routed_round_a_wall() {
        let mut d = Document::spatial_sample();
        let els = units(&mut d, &[[20.0, 20.0], [20.0, 24.0], [20.0, 28.0]]);
        let lv = level(&d);
        let pf = prefs(Domain::Pipe, System::ChilledWaterSupply);
        // one connector at another height, one pointing up
        let conns = vec![
            LayoutConn { element: els[0], conn: pipe_conn([21.0, 20.0, 2.7], [1.0, 0.0, 0.0], 0.05) },
            LayoutConn { element: els[1], conn: pipe_conn([21.0, 24.0, 2.0], [1.0, 0.0, 0.0], 0.05) },
            LayoutConn { element: els[2], conn: pipe_conn([21.0, 28.0, 2.7], [0.0, 0.0, 1.0], 0.05) },
        ];
        let e = d.generate_layouts(lv, &conns, 0, None, &pf).unwrap_err();
        assert!(e.contains("at least two connectors") && e.contains("different height"), "{e}");
        // two connectors facing each other across a wall: the route goes round it
        let (a, b) = (pipe_conn([21.0, 20.0, 2.7], [1.0, 0.0, 0.0], 0.05), pipe_conn([27.0, 20.0, 2.7], [-1.0, 0.0, 0.0], 0.05));
        d.place_wall(lv, [24.0, 17.0], [24.0, 21.0], 0.2, 3.0, 0.0).unwrap();
        let two = vec![LayoutConn { element: els[0], conn: a }, LayoutConn { element: els[1], conn: b }];
        let (layouts, _) = d.generate_layouts(lv, &two, 0, None, &pf).unwrap();
        let routed = layouts.iter().find(|l| l.kind == SolutionKind::Routed).expect("a routed solution");
        assert!(routed.branches.is_empty() && routed.label.contains("routed"), "{}", routed.label);
        // it does not go straight through the wall: it is longer than the straight 6 m
        assert!(routed.length_m > 6.5, "{}", routed.length_m);
        let r = d.clone().place_layout(lv, routed, &pf).unwrap();
        assert_eq!(r.connected, 2);
    }
}
