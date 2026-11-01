//! Placing a family on an open connector, as Revit does: with the Family tool, a click near the open end of a duct or
//! pipe (or an open connector of other equipment) puts the family there, turned so that its connector faces the one
//! it meets, at the height of that connector, with the type of the table whose size fits it, and joined to it. A click
//! away from any open connector places the family where it was clicked, as before. Spacebar turns the family by 90
//! degrees before the click; Tab takes the next connector of the family as the one that meets.
use crate::elec_ui::instance_connectors;
use crate::Desktop;
use openbim_document::{Category, Document, ElementId, EquipLink, FamLink, FamilySpec, Geometry, MepNode, MepPort};
use openbim_family::mep::{Connector, Domain, FlowDir, Pose, Size, System};
use openbim_family::Compiled;
use openbim_place::penetrations::RunSize;

/// How far from the click an open connector still takes the family (metres, in plan).
pub(crate) const PICK_M: f64 = 0.4;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JoinTo { Run { id: ElementId, port: usize }, Equipment { id: ElementId, connector: String } }

/// An open connector in the model: where it is, which way it faces (away from what it belongs to) and what it carries.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpenEnd { pub pos: [f64; 3], pub dir: [f64; 3], pub domain: Domain, pub system: System, pub size: Size, pub flow_dir: Option<FlowDir>, pub to: JoinTo }

/// What snapping decided: the pose of the family, the type to give it (a table type of the right size, if there is
/// one) and the connector it joins.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SnapPlan { pub at: [f64; 2], pub angle_deg: f64, pub elevation_m: f64, pub type_id: Option<ElementId>, pub connector: String, pub target: OpenEnd, pub size_note: Option<String> }

fn unit(v: [f64; 3]) -> [f64; 3] { let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if l < 1e-12 { v } else { [v[0] / l, v[1] / l, v[2] / l] } }

fn size_of_run(s: RunSize) -> Size { match s { RunSize::Round { diameter } => Size::Round { diameter }, RunSize::Rect { width, height, .. } => Size::Rect { width, height }, RunSize::Oval { width, height, .. } => Size::Rect { width, height } } }

fn same_size(a: &Size, b: &Size) -> bool {
    match (a, b) {
        (Size::Round { diameter: x }, Size::Round { diameter: y }) => (x - y).abs() < 1e-3,
        (Size::Rect { width: w1, height: h1 }, Size::Rect { width: w2, height: h2 }) => (w1 - w2).abs() < 1e-3 && (h1 - h2).abs() < 1e-3,
        _ => false,
    }
}

fn describe(s: &Size) -> String {
    match s { Size::Round { diameter } => format!("{:.0} mm", diameter * 1000.0), Size::Rect { width, height } => format!("{:.0} x {:.0} mm", width * 1000.0, height * 1000.0), _ => "no size".into() }
}

/// Every duct and pipe end, and every connector of a family instance, on `level` that nothing is joined to.
pub(crate) fn open_ends(doc: &Document, level: ElementId) -> Vec<OpenEnd> {
    let mut out = Vec::new();
    let m = doc.mep();
    for e in doc.elements().filter(|e| matches!(e.category, Category::Duct | Category::Pipe) && matches!(e.geometry, Some(Geometry::Linear { level: l, .. }) if l == level)) {
        let Some(run) = doc.mep_run(e.id) else { continue };
        for (port, pos, other) in [(0usize, run.a, run.b), (1usize, run.b, run.a)] {
            let p = MepPort { node: MepNode::Run(e.id), port };
            if m.partner(p).is_some() || m.equip_links.iter().any(|q| q.port == p) { continue; }
            out.push(OpenEnd { pos, dir: unit([pos[0] - other[0], pos[1] - other[1], pos[2] - other[2]]), domain: run.domain, system: run.system, size: size_of_run(run.size), flow_dir: None, to: JoinTo::Run { id: e.id, port } });
        }
    }
    for e in doc.elements().filter(|e| e.category == Category::Family && matches!(e.geometry, Some(Geometry::Placed { level: l, .. }) if l == level)) {
        for c in instance_connectors(doc, e.id) {
            if m.equip_links.iter().any(|q| q.element == e.id && q.connector == c.name) || m.fam_links.iter().any(|l| (l.a == e.id && l.a_connector == c.name) || (l.b == e.id && l.b_connector == c.name)) { continue; }
            out.push(OpenEnd { pos: c.pos, dir: unit(c.dir), domain: c.domain, system: c.system, size: c.size, flow_dir: Some(c.flow_dir), to: JoinTo::Equipment { id: e.id, connector: c.name } });
        }
    }
    out
}

/// The connectors of the family in its own frame, solved for a type of the document (or for the library type of
/// that name, or the defaults).
fn local_connectors(doc: &Document, f: &Compiled, type_id: Option<ElementId>, type_name: &str) -> Vec<Connector> {
    let mut parts = openbim_document::RequestParts::default();
    if let Some(t) = type_id {
        for (n, v) in doc.family_type_values(t) { if f.input_scope(&n) == Some(openbim_family::Scope::Type) { parts.type_values.push((n, v)); } else { parts.overrides.push((n, v)); } }
        for (n, v) in doc.family_type_texts(t) { if f.input_scope(&n) == Some(openbim_family::Scope::Type) { parts.type_texts.push((n, v)); } else { parts.texts.push((n, v)); } }
    }
    f.solve(&parts.request((type_id.is_none() && !type_name.is_empty()).then_some(type_name))).map(|i| i.connectors).unwrap_or_default()
}

fn compatible(c: &Connector, t: &OpenEnd, fitting: bool) -> bool {
    c.domain == t.domain && (fitting || c.system == t.system) && !matches!((c.flow_dir, t.flow_dir), (FlowDir::In, Some(FlowDir::In)) | (FlowDir::Out, Some(FlowDir::Out)))
}

/// Where and how to put `family` (a type named `type_name`, or the default) so that its connector number `k` (counting
/// the ones that can meet the target) meets the open connector nearest to `cursor` within `PICK_M`. None when no open
/// connector is near, or none of the family's can meet it.
pub(crate) fn snap(doc: &Document, level: ElementId, family: &str, type_name: &str, cursor: [f64; 2], k: usize) -> Option<SnapPlan> {
    let f = crate::export::library().iter().find(|f| f.name() == family)?;
    let fitting = matches!(f.category(), openbim_family::Category::PipeFitting | openbim_family::Category::DuctFitting);
    let mut ends = open_ends(doc, level);
    ends.retain(|t| (t.pos[0] - cursor[0]).hypot(t.pos[1] - cursor[1]) <= PICK_M);
    ends.sort_by(|a, b| (a.pos[0] - cursor[0]).hypot(a.pos[1] - cursor[1]).total_cmp(&((b.pos[0] - cursor[0]).hypot(b.pos[1] - cursor[1]))));
    let level_z = doc.get(level).map_or(0.0, |l| l.elevation_m);
    for target in ends {
        // the types to try: the one asked for first, then the table's, so a size that fits wins over one that does not
        let mut tries: Vec<Option<ElementId>> = vec![None];
        tries.extend(doc.types_for(Category::Family).into_iter().filter(|t| t.family == family).map(|t| Some(t.id)));
        let mut best: Option<SnapPlan> = None;
        for ty in tries {
            let local = local_connectors(doc, f, ty, type_name);
            let fits: Vec<&Connector> = local.iter().filter(|c| compatible(c, &target, fitting)).collect();
            if fits.is_empty() { continue; }
            let c = fits[k % fits.len()];
            // turn the family so that the connector faces back at the target
            let w = [-target.dir[0], -target.dir[1], -target.dir[2]];
            let (hl, hw) = (c.dir[0].hypot(c.dir[1]), w[0].hypot(w[1]));
            let angle = if hl < 1e-6 && hw < 1e-6 { 0.0 } else if hl < 1e-6 || hw < 1e-6 { continue } else {
                let mut a = (w[1].atan2(w[0]) - c.dir[1].atan2(c.dir[0])).to_degrees();
                while a > 180.0 { a -= 360.0; }
                while a <= -180.0 { a += 360.0; }
                a
            };
            // it must come out right: facing back, on the target, whatever the family's own shape
            let probe = c.world(&Pose { x: 0.0, y: 0.0, z: 0.0, angle });
            let (x, y, z) = (target.pos[0] - probe.pos[0], target.pos[1] - probe.pos[1], target.pos[2] - probe.pos[2]);
            let done = c.world(&Pose { x, y, z, angle });
            let dot = done.dir[0] * target.dir[0] + done.dir[1] * target.dir[1] + done.dir[2] * target.dir[2];
            if dot > -0.999 { continue; }
            let exact = same_size(&c.size, &target.size);
            let note = (!exact).then(|| format!("the connector is {} and the one it meets is {}", describe(&c.size), describe(&target.size)));
            let plan = SnapPlan { at: [x, y], angle_deg: angle, elevation_m: z - level_z, type_id: ty, connector: c.name.clone(), target: target.clone(), size_note: note };
            if exact { return Some(plan); }
            if best.is_none() { best = Some(plan); }
        }
        if best.is_some() { return best; }
    }
    None
}

impl Desktop {
    /// The click of the Family tool on an open connector: places the family on it and joins them in one undo step.
    /// Gives None when no open connector is near, and then nothing has changed.
    pub(crate) fn family_snap_click(&mut self, level: ElementId, cursor: [f64; 2]) -> Option<()> {
        let plan = snap(&self.document, level, &self.family_pick, &self.family_type, cursor, self.family_snap_k)?;
        let mark = self.document.history_mark();
        let spec = FamilySpec { family: self.family_pick.clone(), type_name: self.family_type.clone(), overrides: Vec::new(), angle_deg: plan.angle_deg, elevation_m: plan.elevation_m };
        let id = match self.document.place_family(level, plan.at, &spec) {
            Ok(id) => id,
            Err(e) => { self.message = format!("Family rejected: {e:?}"); return Some(()); }
        };
        if let Some(t) = plan.type_id { if let Err(e) = self.document.assign_type(&[id], t) { self.document.roll_back_to(mark); self.message = format!("Family rejected: {e:?}"); return Some(()); } }
        let mut data = self.document.mep().clone();
        match &plan.target.to {
            JoinTo::Run { id: run, port } => data.equip_links.push(EquipLink { port: MepPort { node: MepNode::Run(*run), port: *port }, element: id, connector: plan.connector.clone() }),
            JoinTo::Equipment { id: other, connector } => data.fam_links.push(FamLink { a: id, a_connector: plan.connector.clone(), b: *other, b_connector: connector.clone() }),
        }
        let joined = self.document.transact(self.document.revision(), &[openbim_document::Command::SetMep(Box::new(data))]).is_ok();
        self.document.merge_steps_since(mark);
        let ty = plan.type_id.and_then(|t| self.document.get(t)).map_or(String::new(), |t| format!(" of type {}", t.name));
        self.message = format!("{} placed{ty} on the open connector, its {} meeting it{}{}; undo removes it. Spacebar turns it, Tab takes another connector", self.family_pick, plan.connector,
            if joined { " and joined" } else { "" }, plan.size_note.map_or(String::new(), |n| format!(" ({n}: add that size to the family's table)")));
        Some(())
    }
}

/// What the pointer shows before the click: where the family would stand, as the click would put it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ghost {
    /// the outline of the body in plan (convex hull of its mesh)
    pub hull: Vec<[f64; 2]>,
    /// the connectors where they would be, and whether each would be joined
    pub connectors: Vec<([f64; 3], bool)>,
    /// the lowest and highest point of the body
    pub z: (f64, f64),
    /// what the click would do, in words; None for a plain placement
    pub label: Option<String>,
    /// the click would be refused
    pub refused: bool,
}

pub(crate) type GhostKey = (u64, String, String, [i64; 2], u64, usize, u64);

fn convex_hull(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
    if pts.len() < 3 { return pts; }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut h: Vec<[f64; 2]> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let it: Box<dyn Iterator<Item = &[f64; 2]>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in it {
            while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 { h.pop(); }
            h.push(p);
        }
        h.pop();
    }
    h
}

impl Desktop {
    /// Where a hit in the 3D view puts a family: the level of what was hit (or the first level), the point on the
    /// 50 mm grid as in the plan, and the wall when a wall was hit.
    pub(crate) fn family_hit_target(&self, hit: &openbim_viewport_egui::Hit) -> (Option<ElementId>, [f64; 2], Option<ElementId>) {
        let id = ElementId(hit.element.id);
        let wall = self.document.get(id).is_some_and(|e| e.category == Category::Wall).then_some(id);
        let level = self.document.get(id).and_then(|e| match e.geometry { Some(Geometry::Linear { level, .. }) | Some(Geometry::Placed { level, .. }) => Some(level), _ => None })
            .or_else(|| self.levels().first().map(|l| l.0));
        let snap = |v: f64| (v / 0.05).round() * 0.05;
        (level, [snap(hit.point[0]), snap(hit.point[1])], wall)
    }

    /// The ghost for the point the pointer is over in the 3D view.
    pub(crate) fn family_ghost_at_hit(&mut self, hit: &openbim_viewport_egui::Hit) -> Option<Ghost> {
        let (level, at, wall) = self.family_hit_target(hit);
        if wall.is_some() { return None; }
        self.family_ghost(level?, at)
    }

    /// The ghost of the Family tool at `cursor`: the family drawn where a click would put it - on the open connector
    /// it would snap to, across the run it would cut, or at the pointer - with the connectors that would be joined
    /// marked. Kept until the pointer, the choice or the model changes.
    pub(crate) fn family_ghost(&mut self, level: ElementId, cursor: [f64; 2]) -> Option<Ghost> {
        let key: GhostKey = (self.document.revision(), self.family_pick.clone(), self.family_type.clone(), [(cursor[0] * 1000.0).round() as i64, (cursor[1] * 1000.0).round() as i64], self.angle_deg.to_bits(), self.family_snap_k, self.slab_offset_mm.to_bits());
        if let Some((k, g)) = &self.ghost_cache { if *k == key { return g.clone(); } }
        let g = self.build_ghost(level, cursor);
        self.ghost_cache = Some((key, g.clone()));
        g
    }

    fn build_ghost(&self, level: ElementId, cursor: [f64; 2]) -> Option<Ghost> {
        let (family, type_name) = (self.family_pick.as_str(), self.family_type.as_str());
        if family.is_empty() || family.starts_with("Door") || family.starts_with("Window") { return None; }
        let doc = &self.document;
        // the same decision the click makes
        let (at, angle, elevation, type_id, label, joined): ([f64; 2], f64, f64, Option<ElementId>, Option<String>, Vec<String>) =
            if let Some(p) = snap(doc, level, family, type_name, cursor, self.family_snap_k) {
                let note = p.size_note.clone().map_or(String::new(), |n| format!(" ({n})"));
                (p.at, p.angle_deg, p.elevation_m, p.type_id, Some(format!("joins its {}{note}", p.connector)), vec![p.connector])
            } else {
                match splice(doc, level, family, type_name, cursor, self.family_snap_k) {
                    Ok(Some(p)) => (p.at, p.angle_deg, p.elevation_m, p.type_id, Some("cuts into the run".into()), vec![p.left.0.clone(), p.right.0.clone()]),
                    Err(why) => return self.ghost_at(level, family, type_name, cursor, self.angle_deg, self.slab_offset_mm / 1000.0, None, Some(format!("cannot go into the run: {why}")), Vec::new(), true),
                    Ok(None) => (cursor, self.angle_deg, self.slab_offset_mm / 1000.0, None, None, Vec::new()),
                }
            };
        self.ghost_at(level, family, type_name, at, angle, elevation, type_id, label, joined, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn ghost_at(&self, level: ElementId, family: &str, type_name: &str, at: [f64; 2], angle: f64, elevation: f64, type_id: Option<ElementId>, label: Option<String>, joined: Vec<String>, refused: bool) -> Option<Ghost> {
        // a scratch model of the level and the types, so the body is meshed exactly as it will be
        let small = self.document.filtered(&|e| e.id == level || e.category == Category::Family && e.geometry.is_none() && e.family == family);
        let mut small = small;
        let spec = FamilySpec { family: family.to_string(), type_name: type_name.to_string(), overrides: Vec::new(), angle_deg: angle, elevation_m: elevation };
        let id = small.place_family(level, at, &spec).ok()?;
        if let Some(t) = type_id { small.assign_type(&[id], t).ok()?; }
        // not the cached `scene_meshes`: this scratch model must neither read nor replace the picture of the real one
        let scene = crate::export::scene_for_view(&small).ok().and_then(|(scene, _)| openbim_view::scene_meshes(&scene).ok())?;
        let em = scene.iter().find(|m| m.element.id == id.0)?;
        let hull = convex_hull(em.mesh.verts.iter().map(|v| [v[0], v[1]]).collect());
        let connectors = instance_connectors(&small, id).into_iter().map(|c| (c.pos, joined.contains(&c.name))).collect();
        let (z0, z1) = em.mesh.verts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v[2]), b.max(v[2])));
        Some(Ghost { hull, connectors, z: (z0, z1), label, refused })
    }
}

/// What dropping a family on the middle of a run decided: the pose, the type, and the two cut points.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SplicePlan { pub run: ElementId, pub at: [f64; 2], pub angle_deg: f64, pub elevation_m: f64, pub type_id: Option<ElementId>, pub left: (String, [f64; 3]), pub right: (String, [f64; 3], (f64, f64)) }

/// The two connectors of a family that are opposite faces of one straight axis (a tee's run, a reducer's ends, a
/// valve's ports): indices, with the second beyond the first along the axis.
fn through_pair(local: &[Connector]) -> Option<(usize, usize)> {
    for (i, a) in local.iter().enumerate() {
        for (j, b) in local.iter().enumerate() {
            if i == j { continue; }
            let dot = a.dir[0] * b.dir[0] + a.dir[1] * b.dir[1] + a.dir[2] * b.dir[2];
            let d = [b.pos[0] - a.pos[0], b.pos[1] - a.pos[1], b.pos[2] - a.pos[2]];
            let (l, cross) = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(), [d[1] * a.dir[2] - d[2] * a.dir[1], d[2] * a.dir[0] - d[0] * a.dir[2], d[0] * a.dir[1] - d[1] * a.dir[0]]);
            // opposite faces, with b ahead of a along a's inward direction, on one line, horizontal
            let inward = -(d[0] * a.dir[0] + d[1] * a.dir[1] + d[2] * a.dir[2]);
            if dot < -0.999 && l > 1e-6 && inward > l * 0.999 && cross.iter().all(|c| c.abs() < 1e-6) && a.dir[2].abs() < 1e-6 && a.domain == b.domain { return Some((i, j)); }
        }
    }
    None
}

fn size_pair(s: &Size) -> Option<(f64, f64)> { match s { Size::Round { diameter } => Some((*diameter, *diameter)), Size::Rect { width, height } => Some((*width, *height)), _ => None } }

/// Where to put `family` so that it cuts the horizontal duct or pipe under `cursor` (not near an end: that is a snap).
/// Ok(None) when no run is there or the family has no straight-through pair of connectors (an elbow bends the run,
/// it is not cut into it); Err(reason) when it should splice but cannot.
pub(crate) fn splice(doc: &Document, level: ElementId, family: &str, type_name: &str, cursor: [f64; 2], k: usize) -> Result<Option<SplicePlan>, String> {
    let Some(f) = crate::export::library().iter().find(|f| f.name() == family) else { return Ok(None) };
    let fitting = matches!(f.category(), openbim_family::Category::PipeFitting | openbim_family::Category::DuctFitting);
    // the run nearest the click, with the click's foot on it
    let mut best: Option<(f64, ElementId, openbim_place::penetrations::Run, f64)> = None;
    for e in doc.elements().filter(|e| matches!(e.category, Category::Duct | Category::Pipe) && matches!(e.geometry, Some(Geometry::Linear { level: l, .. }) if l == level)) {
        let Some(r) = doc.mep_run(e.id) else { continue };
        let len = (r.b[0] - r.a[0]).hypot(r.b[1] - r.a[1]);
        if len < 1e-6 { continue; }
        let u = [(r.b[0] - r.a[0]) / len, (r.b[1] - r.a[1]) / len];
        let s = (cursor[0] - r.a[0]) * u[0] + (cursor[1] - r.a[1]) * u[1];
        let off = ((cursor[0] - r.a[0]) * u[1] - (cursor[1] - r.a[1]) * u[0]).abs();
        if off <= 0.2 && s >= 0.0 && s <= len && best.as_ref().is_none_or(|b| off < b.0) { best = Some((off, e.id, r, s)); }
    }
    let Some((_, run_id, run, s)) = best else { return Ok(None) };
    let len = (run.b[0] - run.a[0]).hypot(run.b[1] - run.a[1]);
    let u = [(run.b[0] - run.a[0]) / len, (run.b[1] - run.a[1]) / len];
    let level_z = doc.get(level).map_or(0.0, |l| l.elevation_m);
    let run_size = size_of_run(run.size);
    let mut tries: Vec<Option<ElementId>> = vec![None];
    tries.extend(doc.types_for(Category::Family).into_iter().filter(|t| t.family == family).map(|t| Some(t.id)));
    let mut fallback: Option<SplicePlan> = None;
    let mut refusal: Option<String> = None;
    let mut any_pair = false;
    for ty in tries {
        let local = local_connectors(doc, f, ty, type_name);
        let Some((i, j)) = through_pair(&local) else { continue };
        any_pair = true;
        // which end faces back along the run: the one that is the run's own size, else the first (Tab turns it round)
        let (mut c1, mut c2) = (&local[i], &local[j]);
        if (k % 2 == 1) != (!same_size(&c1.size, &run_size) && same_size(&c2.size, &run_size)) { std::mem::swap(&mut c1, &mut c2); }
        if c1.domain != run.domain || !(fitting || c1.system == run.system) { continue; }
        let w = [-u[0], -u[1]];
        let mut a = (w[1].atan2(w[0]) - c1.dir[1].atan2(c1.dir[0])).to_degrees();
        while a > 180.0 { a -= 360.0; }
        while a <= -180.0 { a += 360.0; }
        let probe = |p: &Connector| p.world(&Pose { x: 0.0, y: 0.0, z: 0.0, angle: a });
        let (p1, p2) = (probe(c1), probe(c2));
        // the span of the element along the run, and where it may stand so both pieces keep 20 mm
        let span = (p2.pos[0] - p1.pos[0]) * u[0] + (p2.pos[1] - p1.pos[1]) * u[1];
        if span <= 0.0 { continue; }
        if len < span + 0.04 { refusal = Some(format!("{} is {:.0} mm long between its connectors and the pipe is only {:.0} mm", family, span * 1000.0, len * 1000.0)); continue; }
        let s1 = (s - span / 2.0).clamp(0.02, len - 0.02 - span);
        // a sloped run: the element stands level at the height of the run under its middle
        let zl = |s: f64| run.a[2] + (run.b[2] - run.a[2]) * s / len;
        let zm = zl(s1 + span / 2.0);
        if (zl(s1) - zm).abs() > 0.06 || (zl(s1 + span) - zm).abs() > 0.06 { refusal = Some(format!("the run is too steep for {} to stand level in it (more than 60 mm off the run at either end)", family)); continue; }
        let first = [run.a[0] + u[0] * s1, run.a[1] + u[1] * s1, zm];
        let (x, y, z) = (first[0] - p1.pos[0], first[1] - p1.pos[1], first[2] - p1.pos[2]);
        let second = [first[0] + u[0] * span, first[1] + u[1] * span, zm];
        let Some(rs) = size_pair(&c2.size) else { continue };
        let plan = SplicePlan { run: run_id, at: [x, y], angle_deg: a, elevation_m: z - level_z, type_id: ty, left: (c1.name.clone(), first), right: (c2.name.clone(), second, rs) };
        if same_size(&c1.size, &run_size) { return Ok(Some(plan)); }
        if fallback.is_none() { fallback = Some(plan); }
    }
    if let Some(p) = fallback { return Ok(Some(p)); }
    if !any_pair { return Ok(None); }
    Err(refusal.unwrap_or_else(|| "its connectors do not suit this run (kind or system)".into()))
}

impl Desktop {
    /// The click of the Family tool on the middle of a duct or pipe, with a family that goes in line (a tee, a
    /// reducer, a valve): the run is cut and the family put in the gap, joined to both pieces, in one undo step.
    /// None when this is not such a click.
    pub(crate) fn family_splice_click(&mut self, level: ElementId, cursor: [f64; 2]) -> Option<()> {
        let plan = match splice(&self.document, level, &self.family_pick, &self.family_type, cursor, self.family_snap_k) {
            Ok(Some(p)) => p,
            Ok(None) => return None,
            Err(why) => { self.message = format!("{} not put into the run: {why}", self.family_pick); return Some(()); }
        };
        let mark = self.document.history_mark();
        let spec = FamilySpec { family: self.family_pick.clone(), type_name: self.family_type.clone(), overrides: Vec::new(), angle_deg: plan.angle_deg, elevation_m: plan.elevation_m };
        let id = match self.document.place_family(level, plan.at, &spec) { Ok(id) => id, Err(e) => { self.message = format!("Family rejected: {e:?}"); return Some(()); } };
        let ok = plan.type_id.is_none_or(|t| self.document.assign_type(&[id], t).is_ok())
            && self.document.splice_into_run(plan.run, id, &openbim_document::Splice { left_end: plan.left.1, right_start: plan.right.1, right_size: plan.right.2, left_conn: plan.left.0.clone(), right_conn: plan.right.0.clone() }).is_ok();
        if !ok { self.document.roll_back_to(mark); self.message = format!("{} not put into the run: the model refused the cut (a branch sits where it would go?)", self.family_pick); return Some(()); }
        self.document.merge_steps_since(mark);
        let ty = plan.type_id.and_then(|t| self.document.get(t)).map_or(String::new(), |t| format!(" of type {}", t.name));
        self.message = format!("{} put into the run{ty}: the run is cut and both pieces are joined to it; undo takes it out. Tab turns it end for end", self.family_pick);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_family::mep::{Domain, System};

    fn pipe_to(app: &mut Desktop, pts: &[[f64; 3]], d: f64) -> ElementId {
        let off = openbim_route::snaps::Snaps::new(openbim_route::snaps::SnapSettings::off());
        let s = RunSize::Round { diameter: d };
        let sizes: Vec<RunSize> = pts.windows(2).map(|_| s).collect();
        let route = openbim_route::route_path(pts, &sizes, &crate::export::tests_support::prefs(Domain::Pipe, System::ChilledWaterSupply), &off).unwrap();
        app.document.place_mep_route(ElementId(10), &route).unwrap()[0]
    }

    fn placed(app: &Desktop, name: &str) -> ElementId { app.document.elements().filter(|e| e.category == Category::Family && e.family == name).map(|e| e.id).max().unwrap() }

    #[test]
    fn a_click_near_the_open_end_of_a_pipe_puts_the_elbow_on_it_turned_to_face_it_and_joined() {
        let mut app = Desktop::sample();
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        let before = app.document.history_mark();
        // 0.2 m from the end at (5, 1): the grid snap of the click does not matter
        assert!(app.family_snap_click(ElementId(10), [5.1, 1.15]).is_some(), "{}", app.message);
        let el = placed(&app, "Pipe elbow");
        let c = crate::elec_ui::instance_connectors(&app.document, el);
        let (inlet, out) = (&c[0], &c[1]);
        // the elbow's inlet sits on the pipe end, at the pipe's height, facing back along the pipe
        assert!((inlet.pos[0] - 5.0).abs() < 1e-6 && (inlet.pos[1] - 1.0).abs() < 1e-6 && (inlet.pos[2] - 1.0).abs() < 1e-6, "{:?}", inlet.pos);
        assert!((inlet.dir[0] + 1.0).abs() < 1e-6, "{:?}", inlet.dir);
        assert!(out.dir[1] > 0.99 && out.pos[0] > 5.0, "the bend turns left (+y) beyond the pipe end: {:?} {:?}", out.pos, out.dir);
        // joined to the pipe end, and the whole thing is one undo step
        assert!(app.document.mep().equip_links.iter().any(|q| q.element == el && q.port.node == MepNode::Run(run) && q.port.port == 1));
        assert_eq!(app.document.history_mark(), before + 1);
        app.document.undo().unwrap();
        assert!(app.document.get(el).is_none() && app.document.mep().equip_links.is_empty());
        // the end that is now joined is no longer offered; an end that is not near gives nothing
        assert!(app.family_snap_click(ElementId(10), [3.0, 3.0]).is_none());
    }

    #[test]
    fn the_family_turns_to_whatever_way_the_pipe_runs_and_tab_takes_the_other_connector() {
        let mut app = Desktop::sample();
        // a pipe going up the plan (+y), ending at (2, 4)
        pipe_to(&mut app, &[[2.0, 1.0, 1.0], [2.0, 4.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        assert!(app.family_snap_click(ElementId(10), [2.0, 4.1]).is_some());
        let el = placed(&app, "Pipe elbow");
        let c = crate::elec_ui::instance_connectors(&app.document, el);
        assert!((c[0].pos[0] - 2.0).abs() < 1e-6 && (c[0].pos[1] - 4.0).abs() < 1e-6);
        assert!((c[0].dir[1] + 1.0).abs() < 1e-6, "the inlet faces back down the pipe: {:?}", c[0].dir);
        // Tab: the other connector (the outlet) meets the open end of a second pipe instead
        let mut app = Desktop::sample();
        pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        app.family_snap_k = 1;
        assert!(app.family_snap_click(ElementId(10), [5.0, 1.0]).is_some());
        let el = placed(&app, "Pipe elbow");
        let c = crate::elec_ui::instance_connectors(&app.document, el);
        assert!((c[1].pos[0] - 5.0).abs() < 1e-6 && (c[1].pos[1] - 1.0).abs() < 1e-6, "the outlet is on the pipe end: {:?}", c[1].pos);
        let dot = c[1].dir[0] * 1.0;
        assert!(dot < -0.999, "and it faces back at the pipe: {:?}", c[1].dir);
    }

    #[test]
    fn the_table_type_whose_size_fits_the_pipe_is_the_one_used_and_a_size_that_is_missing_is_said() {
        let mut app = Desktop::sample();
        let mut d = crate::family_types_ui::FamilyTypesDialog::open(&app.document, "Pipe elbow").unwrap();
        d.import_csv("type,pipe_d (mm)\nDN50,50\nDN65,65\nDN80,80\n").unwrap();
        assert!(app.apply_family_types(&mut d), "{}", d.error);
        pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.065);
        app.family_pick = "Pipe elbow".into();
        assert!(app.family_snap_click(ElementId(10), [5.0, 1.0]).is_some());
        let el = placed(&app, "Pipe elbow");
        let t = app.document.get(el).unwrap().type_id.and_then(|t| app.document.get(t)).map(|t| t.name.clone());
        assert_eq!(t.as_deref(), Some("DN65"), "{}", app.message);
        assert_eq!(crate::elec_ui::instance_connectors(&app.document, el)[0].size, Size::Round { diameter: 0.065 });
        assert!(!app.message.contains("add that size"), "{}", app.message);
        // a 100 mm pipe has no type in the table: the elbow is still placed and the message says so
        let mut app2 = Desktop::sample();
        pipe_to(&mut app2, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.1);
        app2.family_pick = "Pipe elbow".into();
        assert!(app2.family_snap_click(ElementId(10), [5.0, 1.0]).is_some());
        assert!(app2.message.contains("100 mm") && app2.message.contains("add that size"), "{}", app2.message);
    }

    #[test]
    fn a_family_that_cannot_meet_the_connector_is_not_snapped_and_is_placed_where_clicked() {
        let mut app = Desktop::sample();
        pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.05);
        // a ceiling diffuser has a duct connector: it cannot meet a pipe
        app.family_pick = "Ceiling diffuser, square".into();
        assert!(app.family_snap_click(ElementId(10), [5.0, 1.0]).is_none());
        app.family_click(ElementId(10), [5.0, 1.0], None);
        assert!(app.message.contains("placed as element"), "{}", app.message);
    }

    fn run_ends(app: &Desktop, id: ElementId) -> ([f64; 3], [f64; 3]) { let r = app.document.mep_run(id).unwrap(); (r.a, r.b) }

    #[test]
    fn a_tee_dropped_on_the_middle_of_a_pipe_cuts_it_and_joins_both_pieces_in_one_step() {
        let mut app = Desktop::sample();
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0], [5.0, 4.0, 1.0]], 0.05);
        let links_before = app.document.mep().links.clone();
        assert_eq!(links_before.len(), 2, "run, elbow, run");
        app.family_pick = "Pipe tee".into();
        let old_end = run_ends(&app, run).1;
        let mark = app.document.history_mark();
        let n_before = app.document.elements().filter(|e| e.category == Category::Pipe).count();
        app.family_click(ElementId(10), [3.0, 1.1], None);
        assert!(app.message.contains("put into the run"), "{}", app.message);
        let tee = placed(&app, "Pipe tee");
        let pipes: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Pipe).map(|e| e.id).collect();
        assert_eq!(pipes.len(), n_before + 1, "the run was cut in two");
        let (a, cut_a) = run_ends(&app, run);
        let c = crate::elec_ui::instance_connectors(&app.document, tee);
        let (rin, rout) = (c.iter().find(|c| c.name == "run in").unwrap(), c.iter().find(|c| c.name == "run out").unwrap());
        assert!((a[0] - 1.0).abs() < 1e-9 && (cut_a[0] - rin.pos[0]).abs() < 1e-6 && (cut_a[2] - 1.0).abs() < 1e-9, "the first piece ends at the tee's inlet");
        let new = *pipes.iter().find(|p| **p != run && app.document.mep_run(**p).is_some_and(|r| (r.a[0] - rout.pos[0]).abs() < 1e-6 && (r.a[1] - 1.0).abs() < 1e-6)).expect("a piece starts at the tee's outlet");
        let (b0, b1) = run_ends(&app, new);
        assert!((b0[0] - rout.pos[0]).abs() < 1e-6 && (b1[0] - old_end[0]).abs() < 1e-9 && (b1[1] - 1.0).abs() < 1e-9, "and runs on to the old end: {b0:?} {b1:?}");
        assert!(rin.pos[0] < 3.1 && rout.pos[0] > 2.9 && rout.pos[0] - rin.pos[0] > 0.1, "the tee stands at the click: {:?} {:?}", rin.pos, rout.pos);
        assert!((rin.pos[1] - 1.0).abs() < 1e-6, "on the pipe's axis");
        // the elbow's join moved to the new piece; the tee is joined to both pieces
        let m = app.document.mep();
        assert!(m.links.iter().any(|(x, y)| [x, y].iter().any(|p| p.node == MepNode::Run(new) && p.port == 1)), "{:?}", m.links);
        assert!(!m.links.iter().any(|(x, y)| [x, y].iter().any(|p| p.node == MepNode::Run(run) && p.port == 1)));
        assert!(m.equip_links.iter().any(|q| q.element == tee && q.port == MepPort { node: MepNode::Run(run), port: 1 } && q.connector == "run in"));
        assert!(m.equip_links.iter().any(|q| q.element == tee && q.port == MepPort { node: MepNode::Run(new), port: 0 } && q.connector == "run out"));
        // one undo step puts the pipe back whole
        assert_eq!(app.document.history_mark(), mark + 1);
        app.document.undo().unwrap();
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Pipe).count(), n_before);
        assert!((run_ends(&app, run).1[0] - old_end[0]).abs() < 1e-9 && app.document.mep().links == links_before && app.document.mep().equip_links.is_empty());
    }

    #[test]
    fn a_reducer_in_a_pipe_changes_the_size_after_it_and_an_elbow_is_not_cut_in() {
        let mut app = Desktop::sample();
        let mut d = crate::family_types_ui::FamilyTypesDialog::open(&app.document, "Pipe reducer").unwrap();
        d.import_csv("type,pipe d (mm),pipe d2 (mm),length (mm)\nR100x65,100,65,120\n").unwrap();
        assert!(app.apply_family_types(&mut d), "{}", d.error);
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [6.0, 1.0, 1.0]], 0.1);
        app.family_pick = "Pipe reducer".into();
        app.family_type.clear();
        app.family_click(ElementId(10), [3.0, 1.0], None);
        assert!(app.message.contains("put into the run"), "{}", app.message);
        let red = placed(&app, "Pipe reducer");
        let c = crate::elec_ui::instance_connectors(&app.document, red);
        let (large, small) = (c.iter().find(|c| c.name == "large").unwrap(), c.iter().find(|c| c.name == "small").unwrap());
        assert_eq!(large.size, Size::Round { diameter: 0.1 }, "the table type of the pipe's size was used: {}", app.message);
        assert!(large.pos[0] < small.pos[0], "the large end is on the side the 100 mm pipe comes from");
        let after = app.document.elements().filter(|e| e.category == Category::Pipe && e.id != run).max_by_key(|e| e.id).unwrap();
        assert!((after.width_m - 0.065).abs() < 1e-9, "the piece after it is 65 mm: {}", after.width_m);
        assert!((app.document.get(run).unwrap().width_m - 0.1).abs() < 1e-9);
        // Tab turns it end for end: the small end then faces back, so the first piece must take the small size... it is
        // refused as a fallback only when nothing fits, so the reducer still goes in with the large end first
        // an elbow does not cut a run: it is placed where clicked
        let mut app = Desktop::sample();
        pipe_to(&mut app, &[[1.0, 1.0, 1.0], [6.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        let n = app.document.elements().filter(|e| e.category == Category::Pipe).count();
        app.family_click(ElementId(10), [3.0, 1.0], None);
        assert!(app.message.contains("placed as element"), "{}", app.message);
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Pipe).count(), n);
    }

    #[test]
    fn a_cut_that_leaves_too_little_pipe_or_is_off_the_pipe_is_refused_and_nothing_changes() {
        let mut app = Desktop::sample();
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [3.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe tee".into();
        app.family_click(ElementId(10), [2.0, 1.0], None);
        let tee = placed(&app, "Pipe tee");
        app.document.undo().unwrap();
        let rev = app.document.revision();
        // a cut 10 mm from the end leaves a piece shorter than 20 mm
        assert!(app.document.splice_into_run(run, tee, &openbim_document::Splice { left_end: [2.99, 1.0, 1.0], right_start: [2.995, 1.0, 1.0], right_size: (0.05, 0.05), left_conn: "run in".into(), right_conn: "run out".into() }).is_err());
        // points that are not on the pipe, or out of order
        assert!(app.document.splice_into_run(run, tee, &openbim_document::Splice { left_end: [2.0, 1.5, 1.0], right_start: [2.2, 1.5, 1.0], right_size: (0.05, 0.05), left_conn: "run in".into(), right_conn: "run out".into() }).is_err());
        assert!(app.document.splice_into_run(run, tee, &openbim_document::Splice { left_end: [2.2, 1.0, 1.0], right_start: [2.0, 1.0, 1.0], right_size: (0.05, 0.05), left_conn: "run in".into(), right_conn: "run out".into() }).is_err());
        assert_eq!(app.document.revision(), rev, "a refused cut changes nothing");
    }

    /// A pump with a pipe of its first horizontal pipe connector's size running 3 m away from that connector, joined to it.
    fn pump_with_pipe(app: &mut Desktop, far_elbow: bool) -> (ElementId, ElementId, [f64; 3], [f64; 3]) {
        let spec = FamilySpec { family: "Pump, end suction".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 0.5 };
        let pump = app.document.place_family(ElementId(10), [2.0, 2.0], &spec).unwrap();
        let c = crate::elec_ui::instance_connectors(&app.document, pump).into_iter().find(|c| c.domain == Domain::Pipe && c.dir[2].abs() < 1e-6).unwrap();
        let d = match c.size { Size::Round { diameter } => diameter, _ => 0.05 };
        let far = [c.pos[0] + c.dir[0] * 3.0, c.pos[1] + c.dir[1] * 3.0, c.pos[2]];
        let mut pts = vec![c.pos, far];
        if far_elbow { pts.push([far[0] - c.dir[1] * 2.0, far[1] + c.dir[0] * 2.0, c.pos[2]]); }
        let run = pipe_to(app, &pts, d);
        let mut data = app.document.mep().clone();
        data.equip_links.push(EquipLink { port: MepPort { node: MepNode::Run(run), port: 0 }, element: pump, connector: c.name.clone() });
        app.document.transact(app.document.revision(), &[openbim_document::Command::SetMep(Box::new(data))]).unwrap();
        (pump, run, c.pos, c.dir)
    }

    #[test]
    fn moving_the_pump_carries_the_end_of_its_pipe_and_keeps_the_other_end_and_the_heights() {
        let mut app = Desktop::sample();
        let (pump, run, cpos, cdir) = pump_with_pipe(&mut app, false);
        let (_, far0) = run_ends(&app, run);
        let side = [-cdir[1], cdir[0]];
        let d = [side[0] * 1.0 + cdir[0] * 0.5, side[1] * 1.0 + cdir[1] * 0.5];
        let (ops, notes) = app.document.transform_commands_noted(&[pump], openbim_document::Xf::Move(d)).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let before = app.document.history_mark();
        app.document.transact(app.document.revision(), &ops).unwrap();
        let c = crate::elec_ui::instance_connectors(&app.document, pump).into_iter().find(|c| (c.pos[0] - (cpos[0] + d[0])).abs() < 1e-9 && (c.pos[1] - (cpos[1] + d[1])).abs() < 1e-9).expect("the connector moved by the move");
        let (a, b) = run_ends(&app, run);
        assert!((a[0] - c.pos[0]).abs() < 1e-9 && (a[1] - c.pos[1]).abs() < 1e-9 && (a[2] - cpos[2]).abs() < 1e-9, "the pipe starts on the moved connector: {a:?} {:?}", c.pos);
        assert!((b[0] - far0[0]).abs() < 1e-9 && (b[1] - far0[1]).abs() < 1e-9 && (b[2] - far0[2]).abs() < 1e-9, "the other end stays: {b:?} {far0:?}");
        assert!(app.document.mep().equip_links.iter().any(|q| q.element == pump), "and it is still joined");
        // one undo puts pump and pipe back
        assert_eq!(app.document.history_mark(), before + 1);
        app.document.undo().unwrap();
        let (a0, _) = run_ends(&app, run);
        assert!((a0[0] - cpos[0]).abs() < 1e-9 && (a0[1] - cpos[1]).abs() < 1e-9);
    }

    #[test]
    fn the_elbow_at_the_far_end_is_rebuilt_for_the_new_angle_so_the_pipe_stays_joined_at_both_ends() {
        let mut app = Desktop::sample();
        let (pump, run, cpos, cdir) = pump_with_pipe(&mut app, true);
        let side = [-cdir[1], cdir[0]];
        // along the pipe: it just gets longer, the elbow at the far end stays right
        let (ops, notes) = app.document.transform_commands_noted(&[pump], openbim_document::Xf::Move([cdir[0] * 0.5, cdir[1] * 0.5])).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        app.document.transact(app.document.revision(), &ops).unwrap();
        let (a, _) = run_ends(&app, run);
        assert!((a[0] - (cpos[0] + cdir[0] * 0.5)).abs() < 1e-9 && (a[1] - (cpos[1] + cdir[1] * 0.5)).abs() < 1e-9);
        app.document.undo().unwrap();
        let (before_fit, before_links) = (app.document.mep().fittings.clone(), app.document.mep().links.clone());
        let r2_id = app.document.elements().filter(|e| e.category == Category::Pipe && e.id != run).map(|e| e.id).max().unwrap();
        let far2_before = run_ends(&app, r2_id).1;
        // across the pipe: it turns, and the elbow at its end is made again for the new angle
        let mark = app.document.history_mark();
        let (ops, notes) = app.document.transform_commands_noted(&[pump], openbim_document::Xf::Move([side[0] * 1.0, side[1] * 1.0])).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert_eq!(app.document.history_mark(), mark + 1, "one undo step");
        let (a, b) = run_ends(&app, run);
        assert!((a[0] - (cpos[0] + side[0])).abs() < 1e-9 && (a[1] - (cpos[1] + side[1])).abs() < 1e-9, "the pipe starts on the moved connector: {a:?}");
        assert!(app.document.mep().equip_links.iter().any(|q| q.element == pump), "and is still joined to the pump");
        let m = app.document.mep();
        assert_eq!(m.fittings.len(), 1);
        assert_eq!(m.links, before_links, "the links keep their ports");
        assert_ne!(m.fittings[0].fitting.ports, before_fit[0].fitting.ports, "the elbow was cut for another angle");
        // the elbow is where it was, and both runs end exactly on its ports
        let f = &m.fittings[0].fitting;
        assert!((0..3).all(|i| (f.at[i] - before_fit[0].fitting.at[i]).abs() < 1e-9), "the corner stays");
        let on_port = |p: [f64; 3], k: usize| (0..3).all(|i| (p[i] - (f.at[i] + f.ports[k][i] * f.trim[k])).abs() < 1e-6);
        let linked = |port: usize| m.links.iter().find_map(|(x, y)| if x.node == MepNode::Run(run) && x.port == port { Some(y.port) } else if y.node == MepNode::Run(run) && y.port == port { Some(x.port) } else { None }).unwrap();
        assert!(on_port(b, linked(1)), "the pipe ends on the elbow: {b:?} {f:?}");
        let r2 = app.document.elements().filter(|e| e.category == Category::Pipe && e.id != run).map(|e| e.id).max().unwrap();
        let (a2, b2) = run_ends(&app, r2);
        assert!(on_port(a2, m.links.iter().find_map(|(x, y)| if x.node == MepNode::Run(r2) { Some(y.port) } else if y.node == MepNode::Run(r2) { Some(x.port) } else { None }).unwrap()), "the next pipe starts on the elbow");
        assert!((0..3).all(|i| (b2[i] - far2_before[i]).abs() < 1e-9), "the far end of the next pipe stays: {b2:?} {far2_before:?}");
        app.document.undo().unwrap();
        assert_eq!(app.document.mep().fittings, before_fit, "undo puts the elbow back");
    }

    #[test]
    fn a_pipe_that_cannot_follow_stays_and_is_unjoined_with_a_note() {
        let mut app = Desktop::sample();
        let (pump, run, cpos, cdir) = pump_with_pipe(&mut app, true);
        // nearly the whole length along the pipe: it would be left shorter than 20 mm
        let (ops, notes) = app.document.transform_commands_noted(&[pump], openbim_document::Xf::Move([cdir[0] * 2.99, cdir[1] * 2.99])).unwrap();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("no longer joined"), "{}", notes[0]);
        app.document.transact(app.document.revision(), &ops).unwrap();
        let (a, _) = run_ends(&app, run);
        assert!((a[0] - cpos[0]).abs() < 1e-9 && (a[1] - cpos[1]).abs() < 1e-9, "the pipe stayed");
        assert!(app.document.mep().equip_links.iter().all(|q| q.element != pump), "and is unjoined");
    }

    #[test]
    fn a_level_tee_goes_into_a_sloped_pipe_and_the_two_pieces_take_the_slope_that_joins_them_to_it() {
        let mut app = Desktop::sample();
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [6.0, 1.0, 0.9]], 0.05);
        let end0 = run_ends(&app, run).1;
        app.family_pick = "Pipe tee".into();
        app.family_click(ElementId(10), [3.5, 1.0], None);
        assert!(app.message.contains("put into the run"), "{}", app.message);
        let tee = placed(&app, "Pipe tee");
        let c = crate::elec_ui::instance_connectors(&app.document, tee);
        let (rin, rout) = (c.iter().find(|c| c.name == "run in").unwrap(), c.iter().find(|c| c.name == "run out").unwrap());
        assert!((rin.pos[2] - rout.pos[2]).abs() < 1e-9, "the tee stands level");
        let new = app.document.elements().filter(|e| e.category == Category::Pipe && e.id != run).map(|e| e.id).max().unwrap();
        let (a, cut) = run_ends(&app, run);
        let (r0, b) = run_ends(&app, new);
        assert!((cut[2] - rin.pos[2]).abs() < 1e-9 && (r0[2] - rout.pos[2]).abs() < 1e-9, "both pieces meet the tee at its height: {cut:?} {r0:?}");
        assert!((a[2] - 1.0).abs() < 1e-9 && (b[2] - end0[2]).abs() < 1e-9, "and keep their far ends' heights");
        assert!(cut[2] < 1.0 && b[2] < r0[2], "both still fall the way the pipe fell");
        // the line the pipe was on is within 60 mm of the level tee at both ends
        assert!((rin.pos[2] - (1.0 - 0.1 * (rin.pos[0] - 1.0) / 5.0)).abs() < 0.06);
        // too steep for a level element: refused at the model
        let mut doc = app.document.clone();
        doc.undo().unwrap();
        let steep = doc.splice_into_run(run, tee, &openbim_document::Splice { left_end: [3.4, 1.0, 1.5], right_start: [3.6, 1.0, 1.5], right_size: (0.05, 0.05), left_conn: "run in".into(), right_conn: "run out".into() });
        assert!(steep.is_err(), "100 mm off the line at the cut is too much");
    }

    #[test]
    fn turning_the_pump_turns_the_end_of_its_pipe_with_the_connector() {
        let mut app = Desktop::sample();
        let (pump, run, _, _) = pump_with_pipe(&mut app, false);
        let (_, far0) = run_ends(&app, run);
        let at = match app.document.get(pump).unwrap().geometry { Some(Geometry::Placed { at, .. }) => at, _ => panic!() };
        let (ops, notes) = app.document.transform_commands_noted(&[pump], openbim_document::Xf::Rotate(at, std::f64::consts::FRAC_PI_2)).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        app.document.transact(app.document.revision(), &ops).unwrap();
        let on_pipe_end = crate::elec_ui::instance_connectors(&app.document, pump).into_iter().any(|c| { let (a, _) = run_ends(&app, run); (c.pos[0] - a[0]).abs() < 1e-9 && (c.pos[1] - a[1]).abs() < 1e-9 });
        assert!(on_pipe_end, "a connector of the turned pump is where the pipe now starts");
        let (_, b) = run_ends(&app, run);
        assert!((b[0] - far0[0]).abs() < 1e-9 && (b[1] - far0[1]).abs() < 1e-9, "the far end stays");
    }

    /// An elbow placed on the open end of a pipe, then a second elbow clicked on the open connector of the first.
    fn two_elbows(app: &mut Desktop) -> (ElementId, ElementId) {
        pipe_to(app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        assert!(app.family_snap_click(ElementId(10), [5.1, 1.1]).is_some(), "{}", app.message);
        let first = placed(app, "Pipe elbow");
        let free = crate::elec_ui::instance_connectors(&app.document, first).into_iter().find(|c| c.name.starts_with("out")).unwrap();
        assert!(app.family_snap_click(ElementId(10), [free.pos[0] + 0.05, free.pos[1] + 0.05]).is_some(), "{}", app.message);
        let second = placed(app, "Pipe elbow");
        assert_ne!(first, second);
        (first, second)
    }

    #[test]
    fn a_fitting_clicked_on_the_open_connector_of_another_fitting_is_joined_to_it() {
        let mut app = Desktop::sample();
        let (a, b) = two_elbows(&mut app);
        let links = &app.document.mep().fam_links;
        assert_eq!(links.len(), 1, "{links:?}");
        assert_eq!((links[0].b, links[0].a), (a, b));
        // their joined connectors sit on one point and face each other
        let (ca, cb) = (crate::elec_ui::instance_connectors(&app.document, a), crate::elec_ui::instance_connectors(&app.document, b));
        let (x, y) = (ca.iter().find(|c| c.name == links[0].b_connector).unwrap(), cb.iter().find(|c| c.name == links[0].a_connector).unwrap());
        assert!((0..3).all(|i| (x.pos[i] - y.pos[i]).abs() < 1e-6) && (0..3).map(|i| x.dir[i] * y.dir[i]).sum::<f64>() < -0.999);
        // the joined connector is no longer offered
        let open = open_ends(&app.document, ElementId(10));
        assert!(!open.iter().any(|o| matches!(&o.to, JoinTo::Equipment { id, connector } if (*id == a && *connector == links[0].b_connector) || (*id == b && *connector == links[0].a_connector))));
        // saved and read back
        let back = openbim_document::native::parse_mep(&openbim_document::native::mep_records(app.document.mep())).unwrap();
        assert_eq!(back.fam_links, app.document.mep().fam_links);
        // the whole of it is one undo step per click; undoing the second takes the join with it
        app.document.undo().unwrap();
        assert!(app.document.mep().fam_links.is_empty());
    }

    #[test]
    fn moving_one_of_two_joined_fittings_takes_them_apart_and_moving_both_keeps_them_joined() {
        let mut app = Desktop::sample();
        let (a, b) = two_elbows(&mut app);
        let (ops, notes) = app.document.transform_commands_noted(&[a, b], openbim_document::Xf::Move([0.0, 1.0])).unwrap();
        assert!(notes.iter().all(|n| !n.contains("no longer joined")), "{notes:?}");
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert_eq!(app.document.mep().fam_links.len(), 1, "moved together they stay joined");
        let (ops, notes) = app.document.transform_commands_noted(&[b], openbim_document::Xf::Move([0.0, 1.0])).unwrap();
        assert!(notes.iter().any(|n| n.contains("no longer joined")), "{notes:?}");
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert!(app.document.mep().fam_links.is_empty());
    }

    #[test]
    fn a_connector_cannot_be_in_two_joins() {
        let mut app = Desktop::sample();
        let (a, b) = two_elbows(&mut app);
        let mut data = app.document.mep().clone();
        let l = data.fam_links[0].clone();
        data.fam_links.push(FamLink { a: l.a, a_connector: l.a_connector.clone(), b, b_connector: "elsewhere".into() });
        assert!(app.document.transact(app.document.revision(), &[openbim_document::Command::SetMep(Box::new(data))]).is_err());
        let mut data = app.document.mep().clone();
        data.fam_links[0].b = data.fam_links[0].a;
        assert!(app.document.transact(app.document.revision(), &[openbim_document::Command::SetMep(Box::new(data))]).is_err(), "not to itself");
        let _ = a;
    }

    fn sorted(mut v: Vec<[f64; 3]>) -> Vec<[f64; 3]> { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v }

    #[test]
    fn the_ghost_stands_exactly_where_the_click_puts_the_family_and_marks_what_it_joins() {
        let mut app = Desktop::sample();
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        // on the open end
        let ghost = app.family_ghost(ElementId(10), [5.1, 1.15]).expect("a ghost");
        assert!(ghost.label.as_deref().is_some_and(|l| l.starts_with("joins")), "{:?}", ghost.label);
        assert!(!ghost.refused && ghost.hull.len() >= 3);
        let joined: Vec<[f64; 3]> = ghost.connectors.iter().filter(|c| c.1).map(|c| c.0).collect();
        assert_eq!(joined.len(), 1);
        assert!(joined[0].iter().zip([5.0, 1.0, 1.0]).all(|(a, b)| (a - b).abs() < 1e-6), "the joined connector is on the pipe end: {joined:?}");
        let before = app.document.revision();
        assert!(app.family_snap_click(ElementId(10), [5.1, 1.15]).is_some());
        assert_ne!(app.document.revision(), before);
        let el = placed(&app, "Pipe elbow");
        let after = sorted(crate::elec_ui::instance_connectors(&app.document, el).into_iter().map(|c| c.pos).collect());
        let shown = sorted(ghost.connectors.iter().map(|c| c.0).collect());
        assert_eq!(after.len(), shown.len());
        assert!(after.iter().zip(&shown).all(|(a, b)| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)), "{after:?} vs {shown:?}");
        // the body is drawn around the connectors
        assert!(ghost.hull.iter().any(|p| (p[0] - 5.0).abs() < 0.2));
        let _ = run;
    }

    #[test]
    fn the_ghost_shows_a_cut_across_a_run_and_a_plain_placement_away_from_everything() {
        let mut app = Desktop::sample();
        pipe_to(&mut app, &[[1.0, 1.0, 1.0], [6.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe tee".into();
        let cut = app.family_ghost(ElementId(10), [3.5, 1.0]).expect("a ghost");
        assert_eq!(cut.label.as_deref(), Some("cuts into the run"));
        assert_eq!(cut.connectors.iter().filter(|c| c.1).count(), 2, "the two run connectors are joined");
        let free = app.family_ghost(ElementId(10), [30.0, -20.0]).expect("a ghost");
        assert_eq!(free.label, None);
        assert!(free.connectors.iter().all(|c| !c.1));
        assert!(free.hull.iter().all(|p| (p[0] - 30.0).abs() < 1.5 && (p[1] + 20.0).abs() < 1.5), "{:?}", free.hull);
        // turning it (Spacebar) turns the ghost: a new key, a new picture
        let before: Vec<[f64; 3]> = sorted(free.connectors.iter().map(|c| c.0).collect());
        app.angle_deg = 90.0;
        let turned = app.family_ghost(ElementId(10), [30.0, -20.0]).unwrap();
        assert_ne!(sorted(turned.connectors.iter().map(|c| c.0).collect()), before);
        // doors and windows go in walls: no ghost here
        app.family_pick = "Door".into();
        assert!(app.family_ghost(ElementId(10), [30.0, -20.0]).is_none());
    }

    #[test]
    fn disjoin_moves_the_pump_alone_and_breaks_its_joins() {
        let mut app = Desktop::sample();
        let (pump, run, cpos, cdir) = pump_with_pipe(&mut app, true);
        let side = [-cdir[1], cdir[0]];
        let (a0, b0) = run_ends(&app, run);
        let fittings = app.document.mep().fittings.clone();
        let (ops, notes) = app.document.transform_commands_disjoined(&[pump], openbim_document::Xf::Move([side[0], side[1]])).unwrap();
        assert!(notes.iter().any(|n| n.contains("Disjoined")), "{notes:?}");
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert_eq!(run_ends(&app, run), (a0, b0), "the pipe did not move");
        assert_eq!(app.document.mep().fittings, fittings, "nor did the elbow");
        assert!(app.document.mep().equip_links.iter().all(|q| q.element != pump), "the pump is no longer joined");
        let c = crate::elec_ui::instance_connectors(&app.document, pump);
        assert!(c.iter().all(|c| (c.pos[0] - cpos[0]).hypot(c.pos[1] - cpos[1]) > 0.5 || c.pos != cpos), "the pump moved");
        app.document.undo().unwrap();
        assert!(app.document.mep().equip_links.iter().any(|q| q.element == pump), "undo joins it again");
    }

    #[test]
    fn disjoin_lets_a_joined_pipe_be_moved_alone_and_a_plain_move_still_refuses() {
        let mut app = Desktop::sample();
        let (_, run, _, _) = pump_with_pipe(&mut app, true);
        assert!(app.document.transform_commands_noted(&[run], openbim_document::Xf::Move([0.0, 1.0])).is_err(), "joined pipes do not move on their own");
        let (ops, _) = app.document.transform_commands_disjoined(&[run], openbim_document::Xf::Move([0.0, 1.0])).unwrap();
        let (a0, _) = run_ends(&app, run);
        app.document.transact(app.document.revision(), &ops).unwrap();
        let (a, _) = run_ends(&app, run);
        assert!((a[1] - a0[1] - 1.0).abs() < 1e-9, "it moved by 1 m");
        let m = app.document.mep();
        assert!(m.equip_links.is_empty() && m.links.iter().all(|(x, y)| x.node != MepNode::Run(run) && y.node != MepNode::Run(run)), "and is joined to nothing");
    }

    #[test]
    fn the_three_d_ghost_follows_the_hit_under_the_pointer_and_matches_the_click() {
        use openbim_viewport_egui::Hit;
        let mut app = Desktop::sample();
        let run = pipe_to(&mut app, &[[1.0, 1.0, 1.0], [5.0, 1.0, 1.0]], 0.05);
        app.family_pick = "Pipe elbow".into();
        let hit = |x: f64, y: f64| Hit { triangle: 0, element: openbim_view::Element { kind: openbim_view::Kind::Segment, id: run.0 }, face: openbim_view::FaceKind::Side, distance: 1.0, point: [x, y, 1.0] };
        // a pointer 0.13 m off the open end: the ghost snaps to it
        let ghost = app.family_ghost_at_hit(&hit(5.12, 1.13)).expect("a ghost over the pipe");
        assert!(ghost.label.as_deref().is_some_and(|l| l.starts_with("joins")), "{:?}", ghost.label);
        assert!(ghost.z.1 > ghost.z.0, "it has a height for the 3D outline");
        let (level, at, wall) = app.family_hit_target(&hit(5.12, 1.13));
        assert!(wall.is_none() && level == Some(ElementId(10)));
        assert!((at[0] - 5.1).abs() < 1e-9 && (at[1] - 1.15).abs() < 1e-9, "on the 50 mm grid like the plan: {at:?}");
        // and the click there does what the ghost showed
        app.family_click(level.unwrap(), at, None);
        let el = placed(&app, "Pipe elbow");
        let c = crate::elec_ui::instance_connectors(&app.document, el);
        assert!(ghost.connectors.iter().filter(|g| g.1).all(|g| c.iter().any(|x| (0..3).all(|i| (x.pos[i] - g.0[i]).abs() < 1e-6))));
        // over a wall nothing is previewed (doors and windows have their own placement)
        let wall_id = app.document.elements().find(|e| e.category == Category::Wall).unwrap().id;
        let on_wall = Hit { element: openbim_view::Element { kind: openbim_view::Kind::Wall, id: wall_id.0 }, ..hit(0.0, 0.0) };
        assert!(app.family_ghost_at_hit(&on_wall).is_none());
    }
}
