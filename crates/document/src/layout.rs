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

/// Revit's Conversion Settings of Generate Layout, as far as they are built: the elevation of the main and of the
/// branches above the level of the plan. With none, the layout stays at the height of the base's connector when every
/// connector can be joined there, and otherwise runs above the highest connector (see `AUTO_CLEARANCE_M`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conversion {
    pub offset_m: Option<f64>,
}

/// The section a connector is joined with. The model holds ducts as rectangles only, so a round duct connector (a
/// diffuser neck) is joined by a square duct of the same area.
fn joined_size(domain: openbim_family::mep::Domain, size: RunSize) -> RunSize {
    match (domain, size) {
        (openbim_family::mep::Domain::Duct, RunSize::Round { diameter }) => {
            let side = diameter * std::f64::consts::PI.sqrt() / 2.0;
            RunSize::Rect { width: side, height: side, roll: 0.0 }
        }
        _ => size,
    }
}

/// How far above the highest connector the main runs when the layout needs another height and none is given: the
/// void above a ceiling that hosts the terminals.
pub const AUTO_CLEARANCE_M: f64 = 0.55;

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
        self.generate_layouts_with(level, conns, base, trunk, prefs, &Conversion::default())
    }

    /// `generate_layouts` with Conversion Settings. When the connectors are not all sideways at one height (a diffuser
    /// takes air from above, a sprinkler head water from above), or an offset is given, the main runs at that elevation
    /// and each connector is reached by a section of its own: the equipment that supplies sideways rises to the main on
    /// a 45 degree section; a connector that faces up or down is reached by a 45 degree section from a branch at the
    /// elevation of the main (the model holds no vertical runs, and a flexible connection is not made yet).
    pub fn generate_layouts_with(&self, level: ElementId, conns: &[LayoutConn], base: usize, trunk: Option<RunSize>, prefs: &RoutingPrefs, conv: &Conversion) -> Result<(Vec<Layout>, Vec<String>), String> {
        let Some(b) = conns.get(base) else { return Err("there is nothing to start from: the system has no connector of its type".into()) };
        let z = b.conn.pos[2];
        let in_plane = |c: &LayoutConn| { let d = unit(c.conn.dir); d[2].abs() < 1e-6 && (d[0].abs() > 1.0 - 1e-6 || d[1].abs() > 1.0 - 1e-6) && (c.conn.pos[2] - z).abs() <= 0.002 };
        let elevated = conv.offset_m.is_some() || conns.iter().any(|c| !in_plane(c));
        if elevated { return self.generate_elevated(level, conns, base, trunk, prefs, conv); }
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

    fn generate_elevated(&self, level: ElementId, conns: &[LayoutConn], base: usize, trunk: Option<RunSize>, prefs: &RoutingPrefs, conv: &Conversion) -> Result<(Vec<Layout>, Vec<String>), String> {
        let b = &conns[base];
        let level_z = self.get(level).map_or(0.0, |l| l.elevation_m);
        let zm = conv.offset_m.map_or_else(|| conns.iter().map(|c| c.conn.pos[2]).fold(f64::NEG_INFINITY, f64::max) + AUTO_CLEARANCE_M, |o| level_z + o);
        let mut unconnected: Vec<(ElementId, String)> = Vec::new();
        let mut usable: Vec<usize> = Vec::new();
        let base_size = joined_size(prefs.domain, conn_run_size(&b.conn).map_err(|e| format!("the base connector has no section ({e:?})"))?);
        for (i, c) in conns.iter().enumerate() {
            match conn_run_size(&c.conn).map(|z| joined_size(prefs.domain, z)) {
                Ok(sz) if same_kind(sz, base_size) => usable.push(i),
                Ok(_) => unconnected.push((c.element, format!("the connector '{}' has another kind of section", c.conn.name))),
                Err(_) => unconnected.push((c.element, format!("the connector '{}' has no section", c.conn.name))),
            }
        }
        if usable.len() < 2 || !usable.contains(&base) { return Err("a layout needs at least two connectors that can be joined".into()); }
        let largest = usable.iter().filter_map(|i| conn_run_size(&conns[*i].conn).ok().map(|z| joined_size(prefs.domain, z))).max_by(|a, b| ref_dim(*a).total_cmp(&ref_dim(*b))).ok_or("the connectors have no section")?;
        let trunk_size = match trunk { Some(t) if same_kind(t, largest) => t, Some(_) => return Err("the trunk is of another kind of section than the connectors".into()), None => largest };
        let (mut layouts, mut why) = (Vec::new(), Vec::new());
        for kind in [SolutionKind::NetworkAlongX, SolutionKind::NetworkAlongY] {
            match elevated_network_layout(conns, &usable, base, trunk_size, zm, prefs, kind) {
                Ok(mut l) => { l.label = format!("Solution {}: {}", layouts.len() + 1, l.label); l.unconnected.extend(unconnected.iter().cloned()); layouts.push(l); }
                Err(e) => why.push(format!("{}: {e}", if kind == SolutionKind::NetworkAlongX { "Network along X" } else { "Network along Y" })),
            }
        }
        let mut tried = Vec::new();
        for mut l in layouts {
            let mut copy = self.clone();
            match copy.place_layout(level, &l, prefs) {
                Ok(r) => { for (el, w) in r.failed { if !l.unconnected.iter().any(|u| u.0 == el && u.1 == w) { l.unconnected.push((el, w)); } } tried.push(l); }
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

/// The network of a system whose connectors are not all at one height or facing sideways: a trunk along one axis at the
/// elevation `zm`. Equipment that leaves sideways rises to it on a 45 degree section; a connector that faces up or down
/// is reached from the trunk by a branch across it and a last section at 45 degrees. The two connectors furthest apart
/// along the trunk that have room for a change of size are its ends, every other connector is tapped off it. A
/// connector too near the trunk, or outside its length, is left out with the reason.
fn elevated_network_layout(conns: &[LayoutConn], usable: &[usize], base: usize, trunk_size: RunSize, zm: f64, prefs: &RoutingPrefs, kind: SolutionKind) -> Result<Layout, String> {
    let ax = if kind == SolutionKind::NetworkAlongX { 0 } else { 1 };
    let cr = 1 - ax;
    let stub = prefs.connector_stub.max(0.05);
    let bend_r = prefs.bend_radius_factor * ref_dim(trunk_size);
    // room for two bends, a change of size and the straight pieces between them
    let m = 2.0 * bend_r + prefs.min_transition_len + 2.0 * prefs.min_straight + 0.15;
    // a branch leaves the trunk with a lead of the size of the trunk (the tee) before it may bend
    let lead = ref_dim(trunk_size) + 0.4;
    let vertical = |c: &Connector| unit(c.dir)[2].abs() > 0.5;
    let axis_aligned = |d: V3| d[0].abs() > 1.0 - 1e-6 || d[1].abs() > 1.0 - 1e-6;
    let own = |i: usize| conn_run_size(&conns[i].conn).map(|z| joined_size(prefs.domain, z)).unwrap_or(trunk_size);
    // the base is the member the layout starts from; with only terminals (sprinkler heads) it is one of them
    // the cross coordinates a trunk may take: beyond the ramp of equipment that faces across it
    let (mut t_lo, mut t_hi) = (f64::NEG_INFINITY, f64::INFINITY);
    for &i in usable {
        let c = &conns[i].conn;
        if vertical(c) { continue; }
        let d = unit(c.dir);
        let reach = stub + (zm - c.pos[2]).abs() + m;
        if d[cr] > 0.5 { t_lo = t_lo.max(c.pos[cr] + reach); } else if d[cr] < -0.5 { t_hi = t_hi.min(c.pos[cr] - reach); }
    }
    if t_lo > t_hi { return Err("the equipment faces opposite sides of where the trunk would run".into()); }
    let verts: Vec<(f64, f64)> = usable.iter().filter(|i| vertical(&conns[**i].conn)).map(|i| (conns[*i].conn.pos[cr], (zm - conns[*i].conn.pos[2]).abs())).collect();
    let crs: Vec<f64> = usable.iter().map(|i| conns[*i].conn.pos[cr]).collect();
    let (lo_c, hi_c) = (crs.iter().copied().fold(f64::INFINITY, f64::min) - 4.0, crs.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 4.0);
    // the cross coordinate that serves most connectors with the shortest branches
    let mut best: Option<(usize, f64, f64)> = None; // (served, total branch length, t)
    let mut k = 0.0;
    while lo_c + k * 0.25 <= hi_c {
        let t = lo_c + k * 0.25;
        k += 1.0;
        if t < t_lo - 1e-9 || t > t_hi + 1e-9 { continue; }
        let served = verts.iter().filter(|(y, dz)| (t - y).abs() >= dz + lead).count();
        let ends = verts.iter().filter(|(y, dz)| (t - y).abs() >= dz + m).count() + usable.iter().filter(|i| !vertical(&conns[**i].conn)).count();
        if ends < 2 { continue; }
        let total: f64 = verts.iter().map(|(y, _)| (t - y).abs()).sum();
        if best.is_none_or(|(s, tl, _)| served > s || (served == s && total < tl - 1e-9)) { best = Some((served, total, t)); }
    }
    let Some((_, _, t)) = best else { return Err("no position of the trunk leaves room for a change of size at both its ends".into()) };
    let at = |s: f64| -> V3 { let mut v = [0.0; 3]; v[ax] = s; v[cr] = t; v[2] = zm; v };
    // from each connector to the trunk
    struct Reach { idx: usize, points: Vec<V3>, tap_s: f64, end_sizes: Vec<RunSize>, end_ok: bool }
    let mut reached: Vec<Reach> = Vec::new();
    let mut left: Vec<(ElementId, String)> = Vec::new();
    if !vertical(&conns[base].conn) && !axis_aligned(unit(conns[base].conn.dir)) { return Err("the base connector does not face along an axis".into()); }
    for &i in usable {
        let c = &conns[i].conn;
        let d = unit(c.dir);
        let sz = own(i);
        if vertical(c) {
            let dz = (zm - c.pos[2]).abs();
            let dist = (t - c.pos[cr]).abs();
            if dist + 1e-6 < dz + lead {
                left.push((conns[i].element, format!("the connector '{}' is too near the trunk: its last section needs {:.2} m across and a lead of {:.2} m, and there is {:.2} m", c.name, dz, lead, dist)));
                continue;
            }
            let side = if t >= c.pos[cr] { 1.0 } else { -1.0 };
            let mut h = c.pos; h[cr] += side * dz; h[2] = zm;
            let tap = at(c.pos[ax]);
            let mut g = h; g[cr] = (h[cr] + tap[cr]) / 2.0;
            reached.push(Reach { idx: i, points: vec![c.pos, h, g, tap], tap_s: c.pos[ax], end_sizes: vec![sz, sz, trunk_size], end_ok: dist >= dz + m });
        } else {
            if !axis_aligned(d) { left.push((conns[i].element, format!("the connector '{}' does not face along an axis", c.name))); continue; }
            let r = (zm - c.pos[2]).abs();
            let along_trunk = d[ax].abs() > 0.5;
            let mut q = add_s(c.pos, d, stub + r + if along_trunk { m } else { 0.0 });
            q[2] = zm;
            if !along_trunk && d[cr] * (t - q[cr]) < -1e-9 { left.push((conns[i].element, format!("the connector '{}' faces away from the trunk", c.name))); continue; }
            let pts = vec![c.pos, add_s(c.pos, d, stub), q, at(q[ax])];
            reached.push(Reach { idx: i, points: pts, tap_s: q[ax], end_sizes: vec![sz, sz, trunk_size], end_ok: true });
        }
    }
    if !reached.iter().any(|r| r.idx == base) { return Err("the base cannot reach the trunk".into()); }
    let ok: Vec<usize> = (0..reached.len()).filter(|k| reached[*k].end_ok).collect();
    if ok.len() < 2 { return Err("fewer than two connectors have room to be the ends of the trunk".into()); }
    let a_at = *ok.iter().min_by(|x, y| reached[**x].tap_s.total_cmp(&reached[**y].tap_s)).unwrap_or(&ok[0]);
    let b_at = *ok.iter().max_by(|x, y| reached[**x].tap_s.total_cmp(&reached[**y].tap_s)).unwrap_or(&ok[0]);
    let (lo_s, hi_s) = (reached[a_at].tap_s, reached[b_at].tap_s);
    if hi_s - lo_s < 1e-6 { return Err("the connectors are in line across the trunk: it would have no length".into()); }
    // an end that faces up or down turns along the trunk first, so that the connectors in its column tap the trunk
    // beside its last leg and not on a corner of it
    let past = lead + m;
    let end_of = |k: usize, dir: f64| -> (Vec<V3>, Vec<RunSize>) {
        let r = &reached[k];
        if !vertical(&conns[r.idx].conn) { return (r.points.clone(), r.end_sizes.clone()); }
        let (h, tap) = (r.points[1], r.points[3]);
        let mut h2 = h; h2[ax] += dir * past;
        let mut tap2 = tap; tap2[ax] += dir * past;
        let mut g = h2; g[cr] = (h2[cr] + tap2[cr]) / 2.0;
        let sz = own(r.idx);
        (vec![r.points[0], h, h2, g, tap2], vec![sz, sz, sz, trunk_size])
    };
    let (pa, sa) = end_of(a_at, -1.0);
    let (pb, sb) = end_of(b_at, 1.0);
    let (lo_s, hi_s) = (pa.last().map_or(lo_s, |p| p[ax]), pb.last().map_or(hi_s, |p| p[ax]));
    let mut points = pa;
    let mut sizes = sa;
    sizes.push(trunk_size);
    points.extend(pb.into_iter().rev());
    sizes.extend(sb.into_iter().rev());
    let trunk = trim_path(points, sizes);
    let off = Snaps::new(SnapSettings::off());
    let trunk_route = route_path(&trunk.points, &trunk.sizes, prefs, &off).map_err(|e| format!("the trunk cannot be drawn ({e})"))?;
    let mut branches = Vec::new();
    // connectors in one column would tap the trunk at one point: each after the first jogs along the trunk, to the
    // side and by a step, so that the tees do not meet
    let gap = 2.0 * ref_dim(trunk_size) + 0.4;
    let mut used: Vec<f64> = reached.iter().filter(|r| r.idx == base).map(|r| r.tap_s).collect();
    let mut order: Vec<usize> = (0..reached.len()).filter(|k| *k != a_at && *k != b_at && reached[*k].idx != base).collect();
    order.sort_by(|x, y| reached[*x].tap_s.total_cmp(&reached[*y].tap_s));
    let base_k = reached.iter().position(|r| r.idx == base).filter(|k| *k != a_at && *k != b_at);
    let mut todo: Vec<usize> = base_k.into_iter().collect();
    todo.extend(order);
    for k in todo {
        let r = &reached[k];
        if r.tap_s < lo_s - 1e-6 || r.tap_s > hi_s + 1e-6 { left.push((conns[r.idx].element, format!("the connector '{}' lies beyond the ends of the trunk", conns[r.idx].conn.name))); continue; }
        // the tap nearest the connector's own column that clears the tees already on the trunk and the ends of it
        let free = |x: f64| x > lo_s + gap && x < hi_s - gap && used.iter().all(|u| (u - x).abs() >= gap - 1e-9);
        let off = if r.idx == base { Some(0.0) } else { (0..40).map(|n| { let m = (n + 1) / 2; (m as f64) * gap * if n % 2 == 1 { 1.0 } else { -1.0 } }).find(|o| free(r.tap_s + o)) };
        let Some(off) = off else { left.push((conns[r.idx].element, format!("the connector '{}' has no room along the trunk for its own tee", conns[r.idx].conn.name))); continue };
        used.push(r.tap_s + off);
        let sz = own(r.idx);
        let mut pts = r.points.clone();
        if off != 0.0 {
            let n = pts.len();
            let mut tap2 = pts[n - 1]; tap2[ax] += off;
            if vertical(&conns[r.idx].conn) {
                // c.pos, h, g, tap -> c.pos, h, h', g', tap'
                let h = pts[1];
                let mut h2 = h; h2[ax] += off;
                let mut g2 = h2; g2[cr] = (h2[cr] + tap2[cr]) / 2.0;
                pts = vec![pts[0], h, h2, g2, tap2];
            } else {
                let mut j = pts[n - 2]; j[ax] += off;
                pts.truncate(n - 1);
                pts.push(j);
                pts.push(tap2);
            }
        }
        // the branch is of the connector's size all the way; the trunk is of its own size at the tee
        let path = trim_path(pts.iter().rev().copied().collect(), vec![sz; pts.len() - 1]);
        branches.push(LayoutBranch { path, end: (conns[r.idx].element, conns[r.idx].conn.name.clone()) });
    }
    let length = path_length(&trunk.points) + branches.iter().map(|b| path_length(&b.path.points)).sum::<f64>();
    Ok(Layout {
        label: format!("network at {:.2} m, trunk along {} at {:.2} m, {} branch{}, {length:.1} m", zm, if ax == 0 { "X" } else { "Y" }, t, branches.len(), if branches.len() == 1 { "" } else { "es" }),
        kind,
        trunk,
        trunk_ends: [(conns[reached[a_at].idx].element, conns[reached[a_at].idx].conn.name.clone()), (conns[reached[b_at].idx].element, conns[reached[b_at].idx].conn.name.clone())],
        trunk_route,
        branches,
        unconnected: left,
        length_m: length,
    })
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
        // they are joined now: the main runs above the highest of them, the one pointing up is reached by a 45 degree section
        let (sols, _) = d.generate_layouts(lv, &conns, 0, None, &pf).expect("an elevated layout");
        assert!(sols[0].label.contains("network at 3.25 m"), "{}", sols[0].label);
        // a connector of another kind of section cannot be joined to the others, and the layout says so
        let mut odd = pipe_conn([21.0, 32.0, 2.7], [1.0, 0.0, 0.0], 0.05);
        odd.size = Size::Rect { width: 0.1, height: 0.1 };
        let mut four = conns.clone();
        four.push(LayoutConn { element: els[2], conn: odd });
        let (sols, _) = d.generate_layouts(lv, &four, 0, None, &pf).expect("the others are still laid out");
        assert!(sols[0].unconnected.iter().any(|u| u.1.contains("another kind of section")), "{:?}", sols[0].unconnected);
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

    fn sprinkler_conn(pos: V3) -> Connector {
        Connector { name: "pipe".into(), domain: Domain::Pipe, system: System::FireProtection, flow_dir: FlowDir::In, pos, dir: [0.0, 0.0, 1.0], up: [1.0, 0.0, 0.0], size: Size::Round { diameter: 0.025 }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
    }

    /// Sprinkler heads on a ceiling take water from above: the main runs above them and each head is reached by a section
    /// at 45 degrees, as the book does it with Conversion Settings (the model holds no vertical run).
    #[test]
    fn heads_on_a_ceiling_are_joined_by_a_main_above_them_and_a_45_degree_section_to_each() {
        let mut d = Document::spatial_sample();
        let l = level(&d);
        let at = [[2.0, 3.0], [5.0, 3.0], [8.0, 3.0], [2.0, 7.0], [5.0, 7.0], [8.0, 7.0]];
        let els = units(&mut d, &at);
        let conns: Vec<LayoutConn> = els.iter().zip(at).map(|(e, p)| LayoutConn { element: *e, conn: sprinkler_conn([p[0], p[1], 3.0]) }).collect();
        let pr = prefs(Domain::Pipe, System::FireProtection);
        let (sols, _) = d.generate_layouts(l, &conns, 0, None, &pr).expect("heads alone can be laid out");
        let s = &sols[0];
        assert!(s.unconnected.is_empty(), "{:?}", s.unconnected);
        let before = d.elements().filter(|e| e.category == crate::Category::Pipe).count();
        let r = d.place_layout(l, s, &pr).unwrap();
        assert_eq!((r.connected, r.failed.len()), (6, 0), "{:?}", r.failed);
        assert!(d.elements().filter(|e| e.category == crate::Category::Pipe).count() > before + 6);
        // the main is above the heads by the clearance, and no section is steeper than 45 degrees
        let top = d.elements().filter(|e| e.category == crate::Category::Pipe).filter_map(|e| d.mep_run(e.id)).map(|r| r.a[2].max(r.b[2])).fold(f64::NEG_INFINITY, f64::max);
        assert!((top - (3.0 + AUTO_CLEARANCE_M)).abs() < 1e-6, "{top}");
        for e in d.elements().filter(|e| e.category == crate::Category::Pipe) {
            let r = d.mep_run(e.id).unwrap();
            let h = (r.b[0] - r.a[0]).hypot(r.b[1] - r.a[1]);
            assert!(h > 1e-9 && ((r.b[2] - r.a[2]) / h).abs() <= 1.0 + 1e-9, "{:?}", r);
        }
    }

    /// Equipment that leaves sideways low down rises to the main on a 45 degree section; the heads hang off the main.
    #[test]
    fn equipment_that_leaves_sideways_rises_to_the_main_at_45_degrees() {
        let mut d = Document::spatial_sample();
        let l = level(&d);
        let at = [[4.0, 8.0], [8.0, 8.0], [12.0, 8.0], [6.0, 0.5]];
        let els = units(&mut d, &at);
        let mut conns: Vec<LayoutConn> = at[..3].iter().zip(&els).map(|(p, e)| LayoutConn { element: *e, conn: sprinkler_conn([p[0], p[1], 3.0]) }).collect();
        let mut riser = sprinkler_conn([6.0, 1.0, 0.5]);
        riser.dir = [0.0, 1.0, 0.0];
        riser.flow_dir = FlowDir::Out;
        conns.insert(0, LayoutConn { element: els[3], conn: riser });
        let pr = prefs(Domain::Pipe, System::FireProtection);
        let (sols, _) = d.generate_layouts(l, &conns, 0, None, &pr).expect("a layout from the riser");
        let s = &sols[0];
        assert!(s.unconnected.is_empty(), "{:?}", s.unconnected);
        let r = d.place_layout(l, s, &pr).unwrap();
        assert_eq!((r.connected, r.failed.len()), (4, 0), "{:?}", r.failed);
        let slopes: Vec<f64> = d.elements().filter(|e| e.category == crate::Category::Pipe).map(|e| { let r = d.mep_run(e.id).unwrap(); (r.b[2] - r.a[2]) / (r.b[0] - r.a[0]).hypot(r.b[1] - r.a[1]) }).collect();
        assert!(slopes.iter().any(|s| (s.abs() - 1.0).abs() < 1e-6), "a 45 degree section: {slopes:?}");
        assert!(slopes.iter().all(|s| s.abs() <= 1.0 + 1e-9), "{slopes:?}");
    }
}
