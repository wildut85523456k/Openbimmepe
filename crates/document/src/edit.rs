//! Editing what is already drawn, as Revit does: delete (with what stands on it), move, copy, rotate and mirror. Each
//! gives the commands of one undoable step; the model is not changed until they are applied.
use super::{Category, Command, Document, Element, ElementId, Geometry};
use super::mep::{MepData, MepNode, MepPort};
use std::collections::{BTreeMap, BTreeSet};

type P = [f64; 2];

/// A change of place in the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Xf {
    Move(P),
    /// about a centre, by an angle in radians (counter-clockwise)
    Rotate(P, f64),
    /// across the line through two points
    Mirror(P, P),
}

/// Whether an angle in radians is a whole number of half turns.
fn half_turns(a: f64) -> bool { let r = (a / std::f64::consts::PI).rem_euclid(1.0); r < 1e-9 || 1.0 - r < 1e-9 }

impl Xf {
    pub fn point(self, p: P) -> P {
        match self {
            Xf::Move(d) => [p[0] + d[0], p[1] + d[1]],
            Xf::Rotate(c, a) => {
                let (s, k) = a.sin_cos();
                let (x, y) = (p[0] - c[0], p[1] - c[1]);
                [c[0] + x * k - y * s, c[1] + x * s + y * k]
            }
            Xf::Mirror(a, b) => {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = dx * dx + dy * dy;
                let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2;
                let foot = [a[0] + t * dx, a[1] + t * dy];
                [2.0 * foot[0] - p[0], 2.0 * foot[1] - p[1]]
            }
        }
    }
    /// the turn in degrees an instance has after the change
    fn angle(self, deg: f64) -> f64 {
        match self {
            Xf::Move(_) => deg,
            Xf::Rotate(_, a) => deg + a.to_degrees(),
            Xf::Mirror(a, b) => 2.0 * (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees() - deg,
        }
    }
    fn flips(self) -> bool { matches!(self, Xf::Mirror(..)) }
}

impl Document {
    /// What is hosted by, tagged on or made by the elements of `ids`, and those elements: what goes with them.
    pub fn with_dependents(&self, ids: &[ElementId]) -> BTreeSet<ElementId> {
        let mut all: BTreeSet<ElementId> = ids.iter().copied().filter(|i| self.get(*i).is_some()).collect();
        loop {
            let before = all.len();
            for e in self.elements() {
                let host = match e.geometry { Some(Geometry::InWall { host, .. } | Geometry::OnWall { host } | Geometry::OnMember { host } | Geometry::Tag { host, .. } | Geometry::ViewTag { host, .. }) => Some(host), _ => None };
                if host.is_some_and(|h| all.contains(&h)) || e.parent.is_some_and(|p| all.contains(&p)) { all.insert(e.id); }
            }
            if all.len() == before { return all; }
        }
    }

    fn run_is_joined(&self, id: ElementId) -> bool {
        let m = self.mep();
        let node = |n: &MepNode| matches!(n, MepNode::Run(r) if *r == id);
        m.links.iter().any(|(a, b)| node(&a.node) || node(&b.node)) || m.taps.iter().any(|t| t.run == id)
            || m.equip_links.iter().any(|q| node(&q.port.node)) || m.endpoints.iter().any(|e| node(&e.port.node))
    }

    /// The commands that delete `ids` with what is hosted by them, tagged on them or made by them.
    pub fn delete_commands(&self, ids: &[ElementId]) -> Result<Vec<Command>, String> {
        let all = self.with_dependents(ids);
        if all.is_empty() { return Err("nothing to delete".into()); }
        if let Some(t) = all.iter().find(|i| self.get(**i).is_some_and(|e| e.category == Category::Type)) {
            return Err(format!("element {} is a type: delete it from the Types dialog", t.0));
        }
        if let Some(p) = all.iter().find_map(|i| self.get(*i).filter(|e| e.pinned)) { return Err(format!("{} is pinned: unpin it first", p.name)); }
        // tags first: deleting their host would take them anyway
        let is_tag = |i: &ElementId| self.get(*i).is_some_and(|e| matches!(e.geometry, Some(Geometry::Tag { .. } | Geometry::ViewTag { .. } | Geometry::ViewDimension { .. })));
        let (tags, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(is_tag);
        Ok(tags.into_iter().chain(rest).map(|id| Command::Delete { id }).collect())
    }

    /// The element as it is after `xf`; an error says why it cannot be.
    pub(crate) fn transformed(&self, e: &Element, xf: Xf) -> Result<Element, String> {
        let mut e = e.clone();
        let p = |q: P| xf.point(q);
        match e.geometry {
            Some(Geometry::Grid { start, end }) => {
                e.geometry = Some(Geometry::Grid { start: p(start), end: p(end) });
                // a mirror turns the arc the other way round
                if xf.flips() { e.bulge = -e.bulge; }
            }
            Some(Geometry::Linear { start, end, level }) => {
                let (a, b) = (p(start), p(end));
                // a mirrored wall is drawn the other way round, so that its exterior stays on the left
                let (a, b) = if xf.flips() && e.category == Category::Wall { (b, a) } else { (a, b) };
                e.geometry = Some(Geometry::Linear { start: a, end: b, level });
            }
            Some(Geometry::Point { at, level, top }) => {
                if let Xf::Rotate(_, a) = xf { if !half_turns(a) { return Err(format!("{} cannot be turned to that angle yet (its section stays square to the axes)", e.name)); } }
                e.geometry = Some(Geometry::Point { at: p(at), level, top });
            }
            Some(Geometry::Placed { at, level }) => {
                e.geometry = Some(Geometry::Placed { at: p(at), level });
                e.angle_deg = xf.angle(e.angle_deg);
            }
            Some(Geometry::Pad { at, level, thickness_m }) => {
                if let Xf::Rotate(_, a) = xf { if !half_turns(a) { return Err(format!("{} cannot be turned to that angle yet (a footing stays square to the axes)", e.name)); } }
                e.geometry = Some(Geometry::Pad { at: p(at), level, thickness_m });
            }
            Some(Geometry::Area { level }) => {
                e.outline = e.outline.iter().map(|q| p(*q)).collect();
                if xf.flips() { e.outline.reverse(); e.slope_edges = crate::roof::reversed_edges(&e.slope_edges); }
                e.geometry = Some(Geometry::Area { level });
            }
            Some(Geometry::Tag { host, at, anchor }) => e.geometry = Some(Geometry::Tag { host, at: p(at), anchor: p(anchor) }),
            // as in Revit, moving a dimension moves its line; an end that refers to a wall or grid stays on it (if the
            // wall moves too, the regeneration after the change puts the end there), a free end moves with the line
            Some(Geometry::Dimension { a, b, at, level, refs }) => {
                let end = |q: [f64; 2], r: Option<crate::dimension::DimRef>| if r.is_some() { q } else { p(q) };
                e.geometry = Some(Geometry::Dimension { a: end(a, refs[0]), b: end(b, refs[1]), at: p(at), level, refs });
            }
            Some(Geometry::Room { at, level }) => e.geometry = Some(Geometry::Room { at: p(at), level }),
            Some(Geometry::Shaft { .. }) => {
                e.outline = e.outline.iter().map(|q| p(*q)).collect();
                if xf.flips() { e.outline.reverse(); }
            }
            // it belongs to a view, where it stays when the model moves in the plan
            Some(Geometry::ViewTag { .. } | Geometry::ViewDimension { .. }) => {}
            // hosted by something else: it goes where its host goes
            Some(Geometry::InWall { .. } | Geometry::OnWall { .. } | Geometry::OnMember { .. }) => {}
            Some(Geometry::Level) | None => return Err(format!("{} cannot be moved in the plan", e.name)),
        }
        Ok(e)
    }

    /// The elements `ids` and what goes with them, each already changed by `xf`.
    fn transformed_set(&self, ids: &[ElementId], xf: Xf, for_copy: bool, disjoin: bool) -> Result<Vec<Element>, String> {
        let all = self.with_dependents(ids);
        if all.is_empty() { return Err("nothing selected".into()); }
        let mut out = Vec::new();
        for id in all {
            let e = self.get(id).ok_or("an element is gone")?;
            if matches!(e.category, Category::Level | Category::Type) { return Err(format!("{} cannot be moved in the plan", e.name)); }
            if !for_copy && e.pinned { return Err(format!("{} is pinned: unpin it first", e.name)); }
            // a duct or pipe joined to fittings would leave them behind
            if !for_copy && !disjoin && matches!(e.category, Category::Duct | Category::Pipe) && self.run_is_joined(id) {
                return Err(format!("{} is joined to fittings or equipment; moving a joined network is not supported yet", e.name));
            }
            out.push(self.transformed(e, xf)?);
        }
        Ok(out)
    }

    /// Moves, turns or mirrors `ids` and what goes with them (their doors, windows, footings, tags, bars and members).
    pub fn transform_commands(&self, ids: &[ElementId], xf: Xf) -> Result<Vec<Command>, String> { self.transform_commands_noted(ids, xf).map(|(ops, _)| ops) }

    /// The same, with the ducts and pipes joined to the moved equipment following it, and notes on the ones that
    /// could not: a run joined to a moved family element has that end carried to where the connector went, the other
    /// end stays, and the run keeps its height at both ends (its slope follows). A run that cannot follow without
    /// breaking something - a fitting at its other end that would be left at the wrong angle, a branch on it, or a
    /// length under 20 mm - stays where it is and is unjoined from the equipment, and the note says so.
    pub fn transform_commands_noted(&self, ids: &[ElementId], xf: Xf) -> Result<(Vec<Command>, Vec<String>), String> { self.transform_commands_with(ids, xf, false) }

    /// The Move tool's move: as `transform_commands_noted`, and the walls joined to a moved wall stay joined to it
    /// (see `joined_walls_follow`), as with Revit's Move. Align, Offset and the other edits keep using
    /// `transform_commands`, which moves only what it is given.
    pub fn transform_commands_keeping_joins(&self, ids: &[ElementId], xf: Xf) -> Result<(Vec<Command>, Vec<String>), String> {
        let (mut ops, notes) = self.transform_commands_with(ids, xf, false)?;
        let moved: Vec<ElementId> = self.transformed_set(ids, xf, false, false)?.iter().map(|e| e.id).collect();
        self.joined_walls_follow(&moved, xf, &mut ops)?;
        Ok((ops, notes))
    }

    /// Move with Revit's Disjoin option: the elements move alone. Every join between a moved duct, pipe or family
    /// element and something that stays is broken first (nothing follows, nothing is rebuilt), and the notes say which.
    pub fn transform_commands_disjoined(&self, ids: &[ElementId], xf: Xf) -> Result<(Vec<Command>, Vec<String>), String> { self.transform_commands_with(ids, xf, true) }

    fn transform_commands_with(&self, ids: &[ElementId], xf: Xf, disjoin: bool) -> Result<(Vec<Command>, Vec<String>), String> {
        let moved = self.transformed_set(ids, xf, false, disjoin)?;
        let moved_ids: Vec<ElementId> = moved.iter().map(|e| e.id).collect();
        let mut ops = Vec::new();
        for e in moved {
            let old = self.get(e.id).ok_or("an element is gone")?;
            if e.geometry != old.geometry { ops.push(Command::SetGeometry { id: e.id, geometry: e.geometry.ok_or("no geometry")? }); }
            if e.bulge != old.bulge { ops.push(Command::SetBulge { id: e.id, bulge: e.bulge }); }
            if e.outline != old.outline { ops.push(Command::SetOutline { id: e.id, outline: e.outline.clone() }); }
            if e.slope_edges != old.slope_edges { ops.push(Command::SetRoof { id: e.id, slope_ratio: e.slope_ratio.unwrap_or(crate::roof::DEFAULT_SLOPE), edges: e.slope_edges.clone() }); }
            if e.angle_deg != old.angle_deg && e.category == Category::Family {
                ops.push(Command::SetFamily { id: e.id, family: e.family.clone(), type_name: e.type_name.clone(), overrides: e.overrides.clone(), angle_deg: e.angle_deg });
            }
        }
        let notes = if disjoin { self.disjoin_commands(&moved_ids, &mut ops)? } else { self.follow_commands(&moved_ids, xf, &mut ops) };
        if ops.is_empty() { return Err("nothing changes".into()); }
        Ok((ops, notes))
    }

    /// Walls that stay but are joined to a moved wall keep the join, as in Revit: an end of theirs that meets a moved
    /// wall (at its corner or against its side) slides along its own line to where the moved wall now is, so the
    /// wall stretches or shortens instead of being left apart. Only a move does this; a pinned wall that would have to
    /// change refuses the move. Disjoin moves the walls alone.
    fn joined_walls_follow(&self, moved: &[ElementId], xf: Xf, ops: &mut Vec<Command>) -> Result<(), String> {
        let Xf::Move(d) = xf else { return Ok(()) };
        let moved_walls: Vec<(P, P, f64, ElementId)> = moved.iter().filter_map(|id| self.get(*id)).filter(|e| e.category == Category::Wall)
            .filter_map(|e| match e.geometry { Some(Geometry::Linear { start, end, level }) => Some((start, end, e.width_m, level)), _ => None }).collect();
        if moved_walls.is_empty() { return Ok(()); }
        for e in self.elements().filter(|e| e.category == Category::Wall && !moved.contains(&e.id)) {
            let Some(Geometry::Linear { start, end, level }) = e.geometry else { continue };
            let (ux, uy) = (end[0] - start[0], end[1] - start[1]);
            let len = ux.hypot(uy);
            if len < 1e-9 { continue; }
            let u = [ux / len, uy / len];
            // the slide of an end at `p` that meets a moved wall, along this wall's own line
            let slide = |p: P| -> Option<P> {
                moved_walls.iter().filter(|w| w.3 == level).find_map(|&(a, b, t, _)| {
                    let (wx, wy) = (b[0] - a[0], b[1] - a[1]);
                    let wl = wx.hypot(wy);
                    if wl < 1e-9 { return None; }
                    let (w, n) = ([wx / wl, wy / wl], [-wy / wl, wx / wl]);
                    let along = (p[0] - a[0]) * w[0] + (p[1] - a[1]) * w[1];
                    let across = ((p[0] - a[0]) * n[0] + (p[1] - a[1]) * n[1]).abs();
                    let reach = t / 2.0 + 1e-3;
                    if along < -reach || along > wl + reach || across > reach { return None; }
                    let un = u[0] * n[0] + u[1] * n[1];
                    // a wall running alongside the moved one is not joined to it end-on
                    if un.abs() < 0.1 { return None; }
                    let k = (d[0] * n[0] + d[1] * n[1]) / un;
                    Some([p[0] + u[0] * k, p[1] + u[1] * k])
                })
            };
            let (ns, nt) = (slide(start).unwrap_or(start), slide(end).unwrap_or(end));
            if (ns, nt) == (start, end) { continue; }
            if e.pinned { return Err(format!("{} is pinned and joined to a moved wall: unpin it, or move with Disjoin", e.name)); }
            if (nt[0] - ns[0]).hypot(nt[1] - ns[1]) < 0.01 { return Err(format!("{} would be left with no length", e.name)); }
            ops.push(Command::SetGeometry { id: e.id, geometry: Geometry::Linear { start: ns, end: nt, level } });
            // what is in the wall is measured from its start: when the start slides, it keeps its place in the world
            let ks = (ns[0] - start[0]) * u[0] + (ns[1] - start[1]) * u[1];
            if ks.abs() > 1e-12 {
                for h in self.elements() {
                    if let Some(Geometry::InWall { host, along_m }) = h.geometry {
                        if host == e.id { ops.push(Command::SetGeometry { id: h.id, geometry: Geometry::InWall { host, along_m: along_m - ks } }); }
                    }
                }
            }
        }
        Ok(())
    }

    /// Breaks the joins of `moved` with what stays (see `transform_commands_disjoined`).
    fn disjoin_commands(&self, moved: &[ElementId], ops: &mut Vec<Command>) -> Result<Vec<String>, String> {
        let mut data = self.mep().clone();
        let is_moved_run = |n: &MepNode| matches!(n, MepNode::Run(r) if moved.contains(r));
        if let Some(t) = data.taps.iter().find(|t| moved.contains(&t.run)) {
            return Err(format!("{} has a branch taken off it: it cannot be moved on its own", self.get(t.run).map_or("a run".into(), |e| e.name.clone())));
        }
        let name = |i: ElementId| self.get(i).map_or(String::new(), |e| e.name.clone());
        let mut notes = Vec::new();
        let before = (data.links.len(), data.equip_links.len(), data.fam_links.len(), data.endpoints.len());
        // run <-> fitting: the fitting stays, so every link of a moved run is broken
        data.links.retain(|(a, b)| !(is_moved_run(&a.node) || is_moved_run(&b.node)));
        data.endpoints.retain(|e| !is_moved_run(&e.port.node));
        // run <-> equipment: broken unless both moved
        data.equip_links.retain(|q| {
            let (run_moved, el_moved) = (is_moved_run(&q.port.node), moved.contains(&q.element));
            let keep = run_moved == el_moved && (run_moved || !matches!(q.port.node, MepNode::Run(_)));
            keep || (!run_moved && !el_moved)
        });
        let mut apart: Vec<(String, String)> = Vec::new();
        data.fam_links.retain(|l| { let keep = moved.contains(&l.a) == moved.contains(&l.b); if !keep { apart.push((name(l.a), name(l.b))); } keep });
        if before != (data.links.len(), data.equip_links.len(), data.fam_links.len(), data.endpoints.len()) {
            notes.push("Disjoined: what was joined to the moved elements was left where it was".to_string());
            for (a, b) in apart { notes.push(format!("{a} and {b} are no longer joined")); }
            ops.push(Command::SetMep(Box::new(data)));
        }
        Ok(notes)
    }

    /// The runs joined to the family elements in `moved`, carried along (see `transform_commands_noted`).
    fn follow_commands(&self, moved: &[ElementId], xf: Xf, ops: &mut Vec<Command>) -> Vec<String> {
        let (mut notes, mut data) = (Vec::new(), self.mep().clone());
        let mut runs: Vec<ElementId> = Vec::new();
        for q in &data.equip_links {
            if let MepNode::Run(r) = q.port.node { if moved.contains(&q.element) && !moved.contains(&r) && !runs.contains(&r) { runs.push(r); } }
        }
        let mut mep_changed = false;
        let mut touched: Vec<ElementId> = runs.clone();
        for r in runs {
            let (Some(e), Some(run)) = (self.get(r), self.mep_run(r)) else { continue };
            let Some(Geometry::Linear { start, end, level }) = e.geometry else { continue };
            let carried = |port: usize| data.equip_links.iter().any(|q| q.port == MepPort { node: MepNode::Run(r), port } && moved.contains(&q.element));
            let (ns, ne) = (if carried(0) { xf.point(start) } else { start }, if carried(1) { xf.point(end) } else { end });
            let len = (ne[0] - ns[0]).hypot(ne[1] - ns[1]);
            // a fitting at an end that stays is left at the wrong angle unless the run keeps its direction
            let (od, nd) = ([end[0] - start[0], end[1] - start[1]], [ne[0] - ns[0], ne[1] - ns[1]]);
            let (cross, dot) = (od[0] * nd[1] - od[1] * nd[0], od[0] * nd[0] + od[1] * nd[1]);
            let straight = cross.abs() < 1e-6 * od[0].hypot(od[1]) * len.max(1e-9) && dot > 0.0;
            let far_fixed = [0usize, 1].iter().any(|&p| !carried(p) && data.partner(MepPort { node: MepNode::Run(r), port: p }).is_some());
            let has_taps = data.taps.iter().any(|t| t.run == r);
            // Revit keeps the far fitting: it is rebuilt for the new angle, so the pipe stays joined at both ends
            let mut rebuilt = None;
            let mut why = if len < 0.02 { Some("it would be left shorter than 20 mm".to_string()) } else if has_taps { Some("a branch is taken off it".to_string()) } else { None };
            if why.is_none() && far_fixed && !straight {
                match self.rebuild_far_elbow(r, carried(0), xf, moved, &mut data, &mut touched) {
                    Ok(o) => rebuilt = Some(o),
                    Err(m) => why = Some(format!("the fitting at its other end could not follow ({m})")),
                }
            }
            let names: Vec<String> = data.equip_links.iter().filter(|q| q.port.node == MepNode::Run(r) && moved.contains(&q.element)).filter_map(|q| self.get(q.element).map(|x| x.name.clone())).collect();
            match why {
                None if rebuilt.is_some() => { ops.extend(rebuilt.unwrap_or_default()); mep_changed = true; }
                None => {
                    let slope = (run.b[2] - run.a[2]) / len;
                    ops.push(Command::SetGeometry { id: r, geometry: Geometry::Linear { start: ns, end: ne, level } });
                    ops.push(Command::SetSlope { id: r, slope_ratio: slope });
                }
                Some(why) => {
                    data.equip_links.retain(|q| !(q.port.node == MepNode::Run(r) && moved.contains(&q.element)));
                    mep_changed = true;
                    notes.push(format!("{} stays where it was and is no longer joined to {}: {why}", e.name, names.join(", ")));
                }
            }
        }
        // two fittings joined to each other: moving only one of them takes them apart, as in Revit
        let apart: Vec<(String, String)> = data.fam_links.iter().filter(|l| moved.contains(&l.a) != moved.contains(&l.b))
            .filter_map(|l| Some((self.get(l.a)?.name.clone(), self.get(l.b)?.name.clone()))).collect();
        if !apart.is_empty() {
            data.fam_links.retain(|l| moved.contains(&l.a) == moved.contains(&l.b));
            mep_changed = true;
            for (a, b) in apart { notes.push(format!("{a} and {b} are no longer joined: only one of them moved")); }
        }
        if mep_changed { ops.push(Command::SetMep(Box::new(data))); }
        notes
    }

    /// Rebuilds the elbow at the far end of run `r` (whose end `carried_start` says is the one carried by `xf`) for the
    /// new direction of the run: the corner stays, the elbow is cut again by the router, and both runs it joins are
    /// re-cut to it. Gives the commands for the two runs and updates the fitting in `data`; the links keep their ports.
    fn rebuild_far_elbow(&self, r: ElementId, carried_start: bool, xf: Xf, moved: &[ElementId], data: &mut MepData, touched: &mut Vec<ElementId>) -> Result<Vec<Command>, String> {
        use openbim_place::penetrations::RunSize;
        use openbim_route::snaps::{SnapSettings, Snaps};
        use openbim_route::FittingKind;
        let run = self.mep_run(r).ok_or("not a run")?;
        let (cp, fp) = (usize::from(!carried_start), usize::from(carried_start)); // carried port, port at the fitting
        let link = data.partner(MepPort { node: MepNode::Run(r), port: fp }).ok_or("no fitting")?;
        let MepNode::Fitting(fid) = link.node else { return Err("it is joined to another run".into()) };
        let fi = data.fittings.iter().position(|f| f.id == fid).ok_or("fitting missing")?;
        if !matches!(data.fittings[fi].fitting.kind, FittingKind::Elbow { .. }) { return Err("it is not an elbow".into()); }
        let other = data.partner(MepPort { node: MepNode::Fitting(fid), port: 1 - link.port }).ok_or("the elbow has no run on its other side")?;
        let MepNode::Run(r2) = other.node else { return Err("the elbow is joined to a fitting".into()) };
        if touched.contains(&r2) || moved.contains(&r2) || data.taps.iter().any(|t| t.run == r2) { return Err("the next run cannot be changed".into()); }
        let run2 = self.mep_run(r2).ok_or("not a run")?;
        let level2 = match self.get(r2).and_then(|e| e.geometry.map(|g| (e, g))) { Some((_, Geometry::Linear { level, .. })) => level, _ => return Err("not a run".into()) };
        let level1 = match self.get(r).and_then(|e| e.geometry.map(|g| (e, g))) { Some((_, Geometry::Linear { level, .. })) => level, _ => return Err("not a run".into()) };
        let old = &data.fittings[fi];
        let corner = old.fitting.at;
        let at = |q: &openbim_place::penetrations::Run, port: usize| if port == 0 { q.a } else { q.b };
        let near = at(&run, cp);
        let moved_xy = xf.point([near[0], near[1]]);
        let n = [moved_xy[0], moved_xy[1], near[2]];
        let far2 = at(&run2, 1 - other.port);
        let dim = |s: RunSize| match s { RunSize::Round { diameter } => diameter, RunSize::Rect { width, height, .. } => width.max(height) };
        let mut prefs = crate::mep::default_route_prefs(old.domain, old.system);
        prefs.bend_radius_factor = if dim(run.size) > 0.0 && old.fitting.radius > 0.0 { old.fitting.radius / dim(run.size) } else { prefs.bend_radius_factor };
        let off = Snaps::new(SnapSettings::off());
        let route = openbim_route::route_path(&[n, corner, far2], &[run.size, run2.size], &prefs, &off).map_err(|e| format!("{e}"))?;
        if route.runs.len() != 2 || route.fittings.len() != 1 || !matches!(route.fittings[0].kind, FittingKind::Elbow { .. }) { return Err("no elbow fits the new angle".into()); }
        let mut f = route.fittings[0].clone();
        // keep the elbow's port order, which the links refer to
        let to_n = [n[0] - corner[0], n[1] - corner[1], n[2] - corner[2]];
        let i_n = usize::from(f.ports[0][0] * to_n[0] + f.ports[0][1] * to_n[1] + f.ports[0][2] * to_n[2] <= 0.0);
        if i_n != link.port { f.ports.swap(0, 1); f.port_sizes.swap(0, 1); f.trim.swap(0, 1); }
        f.type_name = old.fitting.type_name.clone();
        f.part_code = old.fitting.part_code.clone();
        data.fittings[fi].fitting = f;
        touched.push(r2);
        // run 1 goes n -> corner trimmed; run 2 goes corner trimmed -> far end
        let (lz1, lz2) = (self.get(level1).map_or(0.0, |l| l.elevation_m), self.get(level2).map_or(0.0, |l| l.elevation_m));
        let mut ops = Vec::new();
        // route run 0 is a = n side, b = fitting side; port 0 of the element is its start
        let (s1, t1) = if carried_start { (route.runs[0].a, route.runs[0].b) } else { (route.runs[0].b, route.runs[0].a) };
        let (s2, t2) = if other.port == 0 { (route.runs[1].a, route.runs[1].b) } else { (route.runs[1].b, route.runs[1].a) };
        for (id, level, lz, s, t) in [(r, level1, lz1, s1, t1), (r2, level2, lz2, s2, t2)] {
            let len = (t[0] - s[0]).hypot(t[1] - s[1]);
            if len < 0.02 { return Err("a run would be left shorter than 20 mm".into()); }
            ops.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: [s[0], s[1]], end: [t[0], t[1]], level } });
            ops.push(Command::MoveElevation { id, elevation_m: s[2] - lz });
            ops.push(Command::SetSlope { id, slope_ratio: (t[2] - s[2]) / len });
        }
        Ok(ops)
    }

    /// Drags one end of a wall to `to`, as the grip of a wall does in Revit: the ends of other walls that meet it
    /// there go with it, so that the corner stays joined.
    pub fn wall_end_commands(&self, id: ElementId, at_start: bool, to: P) -> Result<Vec<Command>, String> {
        let wall = self.get(id).filter(|e| matches!(e.category, Category::Wall | Category::Section)).ok_or("not a wall")?;
        let Some(Geometry::Linear { start, end, level }) = wall.geometry else { return Err("not a wall".into()) };
        if wall.pinned { return Err(format!("{} is pinned: unpin it first", wall.name)); }
        let old = if at_start { start } else { end };
        if (to[0] - old[0]).hypot(to[1] - old[1]) < 1e-9 { return Err("nothing changes".into()); }
        if wall.category == Category::Section {
            // a section line stands alone: only its own end moves, and it keeps a length to look along
            let (ns, nt) = if at_start { (to, end) } else { (start, to) };
            if (ns[0] - nt[0]).hypot(ns[1] - nt[1]) < 0.05 { return Err("a section line would be left with no length".into()); }
            return Ok(vec![Command::SetGeometry { id, geometry: Geometry::Linear { start: ns, end: nt, level } }]);
        }
        let close = |a: P| (a[0] - old[0]).hypot(a[1] - old[1]) < 1e-3;
        let mut ops = Vec::new();
        for e in self.elements().filter(|e| e.category == Category::Wall) {
            let Some(Geometry::Linear { start: s, end: t, level: l }) = e.geometry else { continue };
            if e.id != id && e.pinned && l == level && (close(s) || close(t)) { return Err(format!("{} is pinned and joined here: unpin it first", e.name)); }
            let (ns, nt) = if e.id == id { if at_start { (to, t) } else { (s, to) } }
                else if l == level { (if close(s) { to } else { s }, if close(t) { to } else { t }) } else { continue };
            if (ns, nt) != (s, t) {
                if (ns[0] - nt[0]).hypot(ns[1] - nt[1]) < 0.01 { return Err("a wall would be left with no length".into()); }
                ops.push(Command::SetGeometry { id: e.id, geometry: Geometry::Linear { start: ns, end: nt, level } });
            }
        }
        Ok(ops)
    }

    /// The line a wall, beam or grid is drawn on, with its level for the first two.
    pub fn line_of(&self, id: ElementId) -> Option<(P, P)> {
        let e = self.get(id)?;
        match e.geometry {
            Some(Geometry::Linear { start, end, .. }) if matches!(e.category, Category::Wall | Category::Beam) => Some((start, end)),
            Some(Geometry::Grid { start, end }) => Some((start, end)),
            _ => None,
        }
    }

    /// Trim/Extend to Corner: two walls (or beams), each picked on the part to keep, are trimmed or extended to meet
    /// at the corner of their lines.
    pub fn trim_corner_commands(&self, a: ElementId, pick_a: P, b: ElementId, pick_b: P) -> Result<Vec<Command>, String> {
        if a == b { return Err("pick two different elements".into()); }
        if let Some(p) = [a, b].iter().find_map(|i| self.get(*i).filter(|e| e.pinned)) { return Err(format!("{} is pinned: unpin it first", p.name)); }
        let (la, lb) = (self.line_of(a).filter(|_| self.get(a).is_some_and(|e| e.category != Category::Grid)).ok_or("trim: pick walls or beams")?,
            self.line_of(b).filter(|_| self.get(b).is_some_and(|e| e.category != Category::Grid)).ok_or("trim: pick walls or beams")?);
        let v = super::line_meet(la.0, [la.1[0] - la.0[0], la.1[1] - la.0[1]], lb.0, [lb.1[0] - lb.0[0], lb.1[1] - lb.0[1]]).ok_or("the two lines are parallel and never meet")?;
        let mut ops = Vec::new();
        for (id, (s, t), pick) in [(a, la, pick_a), (b, lb, pick_b)] {
            let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
            let l2 = dx * dx + dy * dy;
            let param = |q: P| ((q[0] - s[0]) * dx + (q[1] - s[1]) * dy) / l2;
            let (ns, nt) = if param(v) < param(pick) { (v, t) } else { (s, v) };
            if (ns[0] - nt[0]).hypot(ns[1] - nt[1]) < 0.01 { return Err("the corner would leave no length".into()); }
            if (ns, nt) == (s, t) { continue; }
            let level = match self.get(id).and_then(|e| e.geometry) { Some(Geometry::Linear { level, .. }) => level, _ => return Err("trim: pick walls or beams".into()) };
            ops.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: ns, end: nt, level } });
        }
        if ops.is_empty() { return Err("they already meet at the corner".into()); }
        Ok(ops)
    }

    /// The move that puts `target` on the line of `reference` (both walls, beams or grids that run the same way).
    pub fn align_xf(&self, reference: ElementId, target: ElementId) -> Result<Xf, String> {
        let (r, t) = (self.line_of(reference).ok_or("align: pick walls, beams or grids")?, self.line_of(target).ok_or("align: pick walls, beams or grids")?);
        let (rd, td) = ([r.1[0] - r.0[0], r.1[1] - r.0[1]], [t.1[0] - t.0[0], t.1[1] - t.0[1]]);
        let (rl, tl) = (rd[0].hypot(rd[1]), td[0].hypot(td[1]));
        if (rd[0] * td[1] - rd[1] * td[0]).abs() / (rl * tl) > 1e-3 { return Err("they do not run the same way".into()); }
        let n = [-rd[1] / rl, rd[0] / rl];
        let d = (r.0[0] - t.0[0]) * n[0] + (r.0[1] - t.0[1]) * n[1];
        if d.abs() < 1e-6 { return Err("they are aligned already".into()); }
        Ok(Xf::Move([n[0] * d, n[1] * d]))
    }

    /// The move of Offset: `dist` metres at right angles to the line of `id`, to the side `toward` is on.
    pub fn offset_xf(&self, id: ElementId, dist: f64, toward: P) -> Result<Xf, String> {
        let (s, t) = self.line_of(id).ok_or("offset: pick a wall, a beam or a grid")?;
        let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
        let l = dx.hypot(dy);
        if dist <= 0.0 { return Err("offset: the distance must be more than zero".into()); }
        let side = (toward[0] - s[0]) * -dy + (toward[1] - s[1]) * dx;
        let k = if side >= 0.0 { 1.0 } else { -1.0 };
        Ok(Xf::Move([-dy / l * dist * k, dx / l * dist * k]))
    }

    /// The nearest line parallel to the wall `id` on each side, as Revit's temporary dimensions show them: a wall, a beam
    /// or a grid alongside it. Each is `(element, distance in metres)`, first the one on the left of the wall's direction.
    pub fn wall_neighbours(&self, id: ElementId) -> [Option<(ElementId, f64)>; 2] {
        let Some((s, t)) = self.line_of(id).filter(|_| self.get(id).is_some_and(|e| e.category == Category::Wall)) else { return [None, None] };
        let level = self.level_of(id);
        let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
        let l = dx.hypot(dy);
        let n = [-dy / l, dx / l];
        let mut best: [Option<(ElementId, f64)>; 2] = [None, None];
        for e in self.elements() {
            if e.id == id { continue; }
            let Some((a, b)) = self.line_of(e.id) else { continue };
            let is_grid = e.category == Category::Grid;
            if !is_grid && self.level_of(e.id) != level { continue; }
            let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
            let el = ex.hypot(ey);
            if (dx * ey - dy * ex).abs() / (l * el) > 1e-3 { continue; }
            if !is_grid {
                let p = |q: P| ((q[0] - s[0]) * dx + (q[1] - s[1]) * dy) / l;
                let (lo, hi) = (p(a).min(p(b)), p(a).max(p(b)));
                if hi.min(l) - lo.max(0.0) < 0.01 { continue; }
            }
            let d = (a[0] - s[0]) * n[0] + (a[1] - s[1]) * n[1];
            if d.abs() < 1e-6 { continue; }
            let side = usize::from(d < 0.0);
            if best[side].is_none_or(|(_, b)| d.abs() < b) { best[side] = Some((e.id, d.abs())); }
        }
        best
    }

    /// Typing a new value into the dimension between the wall `id` and its neighbour `other`: the wall moves, with what
    /// stands in it, so that their lines are `new_m` apart.
    pub fn set_neighbour_distance_commands(&self, id: ElementId, other: ElementId, new_m: f64) -> Result<Vec<Command>, String> {
        if new_m <= 0.0 { return Err("the distance must be more than zero".into()); }
        let (s, t) = self.line_of(id).ok_or("not a wall")?;
        let (a, _) = self.line_of(other).ok_or("not a line")?;
        let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
        let l = dx.hypot(dy);
        let n = [-dy / l, dx / l];
        let d = (a[0] - s[0]) * n[0] + (a[1] - s[1]) * n[1];
        let shift = d - d.signum() * new_m;
        if shift.abs() < 1e-9 { return Err("nothing changes".into()); }
        self.transform_commands(&[id], Xf::Move([n[0] * shift, n[1] * shift]))
    }

    /// Typing a new length into the temporary dimension of the wall `id`: its end moves (the walls joined there follow).
    pub fn set_wall_length_commands(&self, id: ElementId, new_m: f64) -> Result<Vec<Command>, String> {
        if new_m < 0.05 { return Err("a wall needs at least 50 mm".into()); }
        let (s, t) = self.line_of(id).filter(|_| self.get(id).is_some_and(|e| e.category == Category::Wall)).ok_or("not a wall")?;
        let l = (t[0] - s[0]).hypot(t[1] - s[1]);
        self.wall_end_commands(id, false, [s[0] + (t[0] - s[0]) / l * new_m, s[1] + (t[1] - s[1]) / l * new_m])
    }

    /// Wall Joins: allows or disallows the join at one end of a wall. A wall end that is not joined stays square, and so
    /// does the end of a wall that met it.
    pub fn wall_join_commands(&self, id: ElementId, at_start: bool, allow: bool) -> Result<Vec<Command>, String> {
        let e = self.get(id).filter(|e| e.category == Category::Wall).ok_or("select the end of a wall")?;
        let mut joins = e.join_ends;
        let i = usize::from(!at_start);
        if joins[i] == allow { return Err(if allow { "the join is allowed already".into() } else { "the join is disallowed already".into() }); }
        joins[i] = allow;
        Ok(vec![Command::SetJoins { id, start: joins[0], end: joins[1] }])
    }

    /// Copies `ids` and what goes with them, changed by `xf`: the commands and the numbers of the copies of `ids`.
    pub fn copy_commands(&self, ids: &[ElementId], xf: Xf) -> Result<(Vec<Command>, Vec<ElementId>), String> {
        let copies = self.transformed_set(ids, xf, true, false)?;
        let mut next = self.next_id().0;
        let map: BTreeMap<ElementId, ElementId> = copies.iter().map(|e| { let n = ElementId(next); next += 1; (e.id, n) }).collect();
        let re = |h: ElementId| map.get(&h).copied().unwrap_or(h);
        let mut ops = Vec::new();
        for mut e in copies {
            e.id = map[&e.id];
            e.pinned = false;
            e.parent = e.parent.map(re);
            e.geometry = e.geometry.map(|g| match g {
                Geometry::InWall { host, along_m } => Geometry::InWall { host: re(host), along_m },
                Geometry::OnWall { host } => Geometry::OnWall { host: re(host) },
                Geometry::OnMember { host } => Geometry::OnMember { host: re(host) },
                Geometry::Tag { host, at, anchor } => Geometry::Tag { host: re(host), at, anchor },
                Geometry::ViewTag { host, view, at, anchor } => Geometry::ViewTag { host: re(host), view, at, anchor },
                other => other,
            });
            ops.push(Command::Restore(Box::new(e)));
        }
        let firsts = ids.iter().filter_map(|i| map.get(i).copied()).collect();
        Ok((ops, firsts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FamilySpec, FootingSpec, Material};

    fn near(a: P, b: P) -> bool { (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9 }
    fn line(d: &Document, id: ElementId) -> (P, P) { match d.get(id).unwrap().geometry { Some(Geometry::Linear { start, end, .. }) => (start, end), g => panic!("{g:?}") } }

    fn with_door_and_tag() -> (Document, ElementId, ElementId, ElementId) {
        let mut d = Document::spatial_sample();
        let door = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        let door = d.place_family_in_wall(ElementId(20), 3.0, &door).unwrap();
        let tag = d.place_tag(ElementId(20), [2.0, 1.0], [2.0, 0.0]).unwrap();
        (d, ElementId(20), door, tag)
    }

    #[test]
    fn a_section_line_has_no_body_and_either_end_of_it_moves_alone() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let s = d.place_section(level, [1.0, 0.0], [7.0, 0.0], 20.0).unwrap();
        // its view depth (20 m) is not a thickness: it must not become a 20 m wide box in the plan
        assert!(d.mesh(s).is_none());
        let wall_before = line(&d, ElementId(20));
        let ops = d.wall_end_commands(s, true, [2.0, 1.0]).unwrap();
        assert_eq!(ops.len(), 1);
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, s).0, [2.0, 1.0]) && near(line(&d, s).1, [7.0, 0.0]));
        assert_eq!(line(&d, ElementId(20)), wall_before);
        // the other end, and a line that would shrink to nothing is refused
        assert!(d.wall_end_commands(s, false, [2.0, 1.01]).is_err());
        assert!(d.wall_end_commands(s, false, [2.0, 3.0]).is_ok());
    }

    #[test]
    fn an_elevation_marker_holds_up_to_four_views_moves_saves_and_is_not_a_body() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let m = d.place_elevation(level, [4.0, -6.0], 0b1111, 20.0, 30.0).unwrap();
        assert!(d.mesh(m).is_none());
        assert_eq!(d.level_of(m), Some(level));
        // a view may be switched off, but a marker with none is refused
        d.transact(d.revision(), &[Command::SetCount { id: m, count: 0b0101 }]).unwrap();
        assert_eq!(d.get(m).unwrap().count, 0b0101);
        assert!(d.transact(d.revision(), &[Command::SetCount { id: m, count: 0 }]).is_err());
        assert!(d.transact(d.revision(), &[Command::SetCount { id: m, count: 16 }]).is_err());
        // it moves like any point, and undo brings it back
        let ops = d.transform_commands(&[m], Xf::Move([1.0, 2.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(matches!(d.get(m).unwrap().geometry, Some(Geometry::Point { at, .. }) if near(at, [5.0, -4.0])));
        d.undo().unwrap();
        assert!(matches!(d.get(m).unwrap().geometry, Some(Geometry::Point { at, .. }) if near(at, [4.0, -6.0])));
        // it survives saving
        let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        let mut back = Document::new();
        back.transact(0, &crate::native::restore_commands(els, site)).unwrap();
        let e = back.get(m).unwrap();
        assert_eq!((e.category, e.count, e.width_m, e.height_m), (Category::Elevation, 0b0101, 20.0, 30.0));
        // and deleting takes it away
        let ops = d.delete_commands(&[m]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(d.get(m).is_none());
    }

    #[test]
    fn deleting_a_wall_takes_its_door_and_tag_and_undo_brings_them_back() {
        let (mut d, wall, door, tag) = with_door_and_tag();
        let before = d.clone();
        // without help the model refuses: the door would stand in nothing
        assert!(d.transact(d.revision(), &[Command::Delete { id: wall }]).is_err());
        let ops = d.delete_commands(&[wall]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(d.get(wall).is_none() && d.get(door).is_none() && d.get(tag).is_none());
        d.undo().unwrap();
        assert!(d.get(wall).is_some() && d.get(door).is_some() && d.get(tag).is_some());
        assert_eq!(d.elements().count(), before.elements().count());
        // a type is not deleted this way
        let t = d.create_type(ElementId(21), "Wall 200 x 3000").unwrap();
        assert!(d.delete_commands(&[t]).is_err());
    }

    #[test]
    fn a_wall_moves_with_its_door_and_tag_and_a_copy_has_its_own() {
        let (mut d, wall, door, tag) = with_door_and_tag();
        let ops = d.transform_commands(&[wall], Xf::Move([0.0, -10.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, wall).0, [0.0, -10.0]) && near(line(&d, wall).1, [8.0, -10.0]));
        assert!(matches!(d.get(tag).unwrap().geometry, Some(Geometry::Tag { at, anchor, .. }) if near(at, [2.0, -9.0]) && near(anchor, [2.0, -10.0])));
        assert!(matches!(d.get(door).unwrap().geometry, Some(Geometry::InWall { host, along_m }) if host == wall && along_m == 3.0));
        d.undo().unwrap();
        assert!(near(line(&d, wall).0, [0.0, 0.0]));
        // a copy: new wall, new door in the new wall, new tag on the new wall; the old ones stay
        let n = d.elements().count();
        let (ops, firsts) = d.copy_commands(&[wall], Xf::Move([0.0, 20.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.elements().count(), n + 3);
        let new = firsts[0];
        assert!(near(line(&d, new).0, [0.0, 20.0]) && near(line(&d, wall).0, [0.0, 0.0]));
        assert_eq!(d.elements().filter(|e| matches!(e.geometry, Some(Geometry::InWall { host, .. }) if host == new)).count(), 1);
        assert_eq!(d.elements().filter(|e| matches!(e.geometry, Some(Geometry::Tag { host, .. }) if host == new)).count(), 1);
        // the copy can be deleted whole
        let ops = d.delete_commands(&[new]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.elements().count(), n);
    }

    #[test]
    fn rotating_and_mirroring_turn_the_walls_and_instances() {
        let (mut d, wall, door, _) = with_door_and_tag();
        let ops = d.transform_commands(&[wall], Xf::Rotate([0.0, 0.0], std::f64::consts::FRAC_PI_2)).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, wall).1, [0.0, 8.0]), "{:?}", line(&d, wall));
        // mirror across the x axis: a wall is drawn the other way round, so that its exterior stays on the left
        let ops = d.transform_commands(&[ElementId(21)], Xf::Mirror([0.0, 0.0], [1.0, 0.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(line(&d, ElementId(21)), ([8.0, -6.0], [8.0, 0.0]));
        // a family instance turns with it
        let fan = d.place_family(ElementId(10), [2.0, 4.5], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 90.0, elevation_m: 2.2 }).unwrap();
        let ops = d.transform_commands(&[fan], Xf::Rotate([0.0, 0.0], std::f64::consts::FRAC_PI_2)).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.get(fan).unwrap().angle_deg, 180.0);
        assert!(matches!(d.get(fan).unwrap().geometry, Some(Geometry::Placed { at, .. }) if near(at, [-4.5, 2.0])));
        let _ = door;
    }

    #[test]
    fn what_cannot_move_says_why() {
        let d = Document::spatial_sample();
        assert!(d.transform_commands(&[ElementId(10)], Xf::Move([1.0, 0.0])).is_err(), "a level");
        assert!(d.transform_commands(&[], Xf::Move([1.0, 0.0])).is_err());
        assert!(d.transform_commands(&[ElementId(20)], Xf::Move([0.0, 0.0])).is_err(), "no change");
    }

    #[test]
    fn dragging_the_end_of_a_wall_takes_the_walls_joined_there_along() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let b = d.place_wall(level, [8.0, 0.0], [8.0, 6.0], 0.2, 3.0, 0.0).unwrap();
        let ops = d.wall_end_commands(ElementId(20), false, [9.0, 0.0]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, ElementId(20)).1, [9.0, 0.0]) && near(line(&d, b).0, [9.0, 0.0]) && near(line(&d, b).1, [8.0, 6.0]));
        d.undo().unwrap();
        assert!(near(line(&d, b).0, [8.0, 0.0]));
        // the far end of the first wall moves alone
        let ops = d.wall_end_commands(ElementId(20), true, [0.0, -1.0]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, ElementId(20)).0, [0.0, -1.0]) && near(line(&d, b).0, [8.0, 0.0]));
        assert!(d.wall_end_commands(ElementId(20), true, [8.0, 0.0]).is_err(), "no length left");
        assert!(d.wall_end_commands(ElementId(10), true, [1.0, 1.0]).is_err());
    }

    #[test]
    fn trim_align_and_offset_work_on_the_lines_of_walls() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        // two walls that do not reach each other, and one that crosses past the other
        let b = d.place_wall(level, [10.0, 2.0], [10.0, 8.0], 0.2, 3.0, 0.0).unwrap();
        let c = d.place_wall(level, [4.0, -3.0], [4.0, 5.0], 0.2, 3.0, 0.0).unwrap();
        // corner of wall 20 (y=0) and b (x=10): 20 is extended, b is extended down
        let ops = d.trim_corner_commands(ElementId(20), [2.0, 0.0], b, [10.0, 6.0]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, ElementId(20)).1, [10.0, 0.0]) && near(line(&d, ElementId(20)).0, [0.0, 0.0]));
        assert!(near(line(&d, b).0, [10.0, 0.0]) && near(line(&d, b).1, [10.0, 8.0]));
        // c crosses wall 20: picking the upper part of c and the left of 20 trims both back to the crossing
        let ops = d.trim_corner_commands(ElementId(20), [1.0, 0.0], c, [4.0, 3.0]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, ElementId(20)).1, [4.0, 0.0]) && near(line(&d, c).0, [4.0, 0.0]) && near(line(&d, c).1, [4.0, 5.0]));
        assert!(d.trim_corner_commands(ElementId(20), [1.0, 0.0], ElementId(20), [2.0, 0.0]).is_err());
        let par = d.place_wall(level, [0.0, 9.0], [5.0, 9.0], 0.2, 3.0, 0.0).unwrap();
        assert!(d.trim_corner_commands(ElementId(20), [1.0, 0.0], par, [1.0, 9.0]).unwrap_err().contains("parallel"));
        // align: the parallel wall goes onto the line of wall 20 (y = 0)
        let xf = d.align_xf(ElementId(20), par).unwrap();
        let ops = d.transform_commands(&[par], xf).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, par).0, [0.0, 0.0]) && near(line(&d, par).1, [5.0, 0.0]));
        assert!(d.align_xf(ElementId(20), par).unwrap_err().contains("already"));
        assert!(d.align_xf(ElementId(20), c).unwrap_err().contains("same way"));
        // offset: to the side of the pointer, either way
        let up = d.offset_xf(ElementId(20), 1.5, [2.0, 7.0]).unwrap();
        let down = d.offset_xf(ElementId(20), 1.5, [2.0, -7.0]).unwrap();
        assert!(matches!(up, Xf::Move(v) if near(v, [0.0, 1.5])) && matches!(down, Xf::Move(v) if near(v, [0.0, -1.5])), "{up:?} {down:?}");
        assert!(d.offset_xf(ElementId(20), 0.0, [2.0, 7.0]).is_err());
        assert!(d.offset_xf(ElementId(10), 1.0, [2.0, 7.0]).is_err());
    }

    #[test]
    fn wall_types_walls_in_one_step_and_flipping_a_wall() {
        let (mut d, wall, door, _) = with_door_and_tag();
        let level = match d.get(wall).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let t = d.add_wall_type("Exterior 300", 0.3, 3.5).unwrap();
        assert!(d.add_wall_type("Exterior 300", 0.3, 3.5).is_err() && d.add_wall_type("  ", 0.3, 3.0).is_err() && d.add_wall_type("x", 0.0, 3.0).is_err());
        assert_eq!(d.types_for(Category::Wall).iter().filter(|x| x.name == "Exterior 300").count(), 1);
        // four walls of a rectangle in one step, with the type's thickness and height, and one undo takes them all
        let n = d.elements().count();
        let ids = d.place_walls(level, &[([20.0, 0.0], [30.0, 0.0]), ([30.0, 0.0], [30.0, -5.0]), ([30.0, -5.0], [20.0, -5.0]), ([20.0, -5.0], [20.0, 0.0])], 0.1, 2.0, 0.0, Some(t)).unwrap();
        assert_eq!(ids.len(), 4);
        assert!(ids.iter().all(|i| d.get(*i).is_some_and(|w| w.width_m == 0.3 && w.height_m == 3.5 && w.type_id == Some(t))));
        // joined at the corners: the first wall's plan is mitred, wider than 0.3 by nothing and its corner points meet the neighbour's
        assert!(d.wall_plan(ids[0]).is_some());
        d.undo().unwrap();
        assert_eq!(d.elements().count(), n);
        assert!(d.place_walls(level, &[([0.0, 0.0], [0.0, 0.0])], 0.2, 3.0, 0.0, None).is_err());
        assert!(d.place_walls(level, &[([0.0, 0.0], [1.0, 0.0])], 0.2, 3.0, 0.0, Some(wall)).is_err(), "only a wall type can be used");
        // flipping swaps the ends, and the door keeps its place in the world
        let before = d.get(door).unwrap().geometry;
        let ops = d.flip_wall_commands(wall).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, wall).0, [8.0, 0.0]) && near(line(&d, wall).1, [0.0, 0.0]));
        assert!(matches!((before, d.get(door).unwrap().geometry), (Some(Geometry::InWall { along_m: a, .. }), Some(Geometry::InWall { along_m: b, .. })) if (a + b - 8.0).abs() < 1e-9));
        assert!(d.flip_wall_commands(door).is_err());
    }

    #[test]
    fn temporary_dimensions_find_the_neighbours_and_typing_moves_the_wall() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let up = d.place_wall(level, [1.0, 5.0], [6.0, 5.0], 0.2, 3.0, 0.0).unwrap();
        let far = d.place_wall(level, [0.0, -3.0], [8.0, -3.0], 0.2, 3.0, 0.0).unwrap();
        let _beyond = d.place_wall(level, [20.0, 2.0], [28.0, 2.0], 0.2, 3.0, 0.0).unwrap();
        let _across = d.place_wall(level, [4.0, 1.0], [4.0, 9.0], 0.2, 3.0, 0.0).unwrap();
        let grid = d.place_grid([0.0, 2.5], [10.0, 2.5], "").unwrap();
        let [left, right] = d.wall_neighbours(ElementId(20));
        // wall 20 runs along +x: its left is +y; the grid at 2.5 is nearer than the wall at 5; the wall at -3 is on the right
        assert_eq!(left.map(|l| l.0), Some(grid), "{left:?}");
        assert!((left.unwrap().1 - 2.5).abs() < 1e-9 && right.map(|r| r.0) == Some(far) && (right.unwrap().1 - 3.0).abs() < 1e-9);
        // typing 4 m for the distance to the wall at -3: wall 20 moves up to y = 1
        let ops = d.set_neighbour_distance_commands(ElementId(20), far, 4.0).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, ElementId(20)).0, [0.0, 1.0]) && near(line(&d, ElementId(20)).1, [8.0, 1.0]));
        // the sample's own wall at y = 6 is 1 m above the wall at y = 5, and its grid at y = 3 is 2 m below it
        assert_eq!(d.wall_neighbours(up), [Some((ElementId(22), 1.0)), Some((ElementId(104), 2.0))]);
        assert!(d.set_neighbour_distance_commands(ElementId(20), far, 4.0).is_err());
        assert!(d.set_neighbour_distance_commands(ElementId(20), far, 0.0).is_err());
        // typing a length moves the end only
        let ops = d.set_wall_length_commands(ElementId(20), 5.5).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(line(&d, ElementId(20)).0, [0.0, 1.0]) && near(line(&d, ElementId(20)).1, [5.5, 1.0]));
        assert!(d.set_wall_length_commands(ElementId(20), 0.01).is_err() && d.set_wall_length_commands(grid, 3.0).is_err());
        // a dimension between two points on nothing is saved and moves whole with the move command
        let dim = d.place_dimension(level, [0.0, 20.0], [3.0, 20.0], [1.5, 21.0]).unwrap();
        let ops = d.transform_commands(&[dim], Xf::Move([1.0, 1.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(matches!(d.get(dim).unwrap().geometry, Some(Geometry::Dimension { a, b, at, .. }) if near(a, [1.0, 21.0]) && near(b, [4.0, 21.0]) && near(at, [2.5, 22.0])));
        // one whose ends are on a wall keeps them there and only its line moves, as in Revit
        let on_wall = d.place_dimension(level, [2.0, 1.0], [4.0, 1.0], [3.0, 2.0]).unwrap();
        let ops = d.transform_commands(&[on_wall], Xf::Move([0.5, 1.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(matches!(d.get(on_wall).unwrap().geometry, Some(Geometry::Dimension { a, b, at, refs, .. }) if refs[0].is_some() && near(a, [2.0, 1.0]) && near(b, [4.0, 1.0]) && near(at, [3.5, 3.0])), "{:?}", d.get(on_wall).unwrap().geometry);
        assert!(d.place_dimension(level, [0.0, 0.0], [0.0, 0.0], [1.0, 1.0]).is_err());
        let (els, _) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        assert_eq!(els.iter().find(|e| e.id == dim).unwrap().geometry, d.get(dim).unwrap().geometry);
    }

    #[test]
    fn a_wall_join_can_be_disallowed_and_allowed_again() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let ids = d.place_walls(level, &[([50.0, 0.0], [60.0, 0.0]), ([60.0, 0.0], [60.0, 10.0])], 0.2, 3.0, 0.0, None).unwrap();
        let plan = |d: &Document, id: ElementId| d.wall_plan(id).unwrap();
        // the longest extent of a wall along its axis: 10 m when its end is square, more or less when mitred
        let reach = |d: &Document, id: ElementId, axis: usize| { let p = plan(d, id); p.iter().map(|q| q[axis]).fold(f64::MIN, f64::max) };
        let joined = (reach(&d, ids[0], 0), reach(&d, ids[1], 1));
        assert!((joined.0 - 60.1).abs() < 1e-9 && (joined.1 - 10.0).abs() < 1e-9 || joined.0 != 60.0, "mitred: {joined:?}");
        let before = (plan(&d, ids[0]), plan(&d, ids[1]));
        let ops = d.wall_join_commands(ids[0], false, false).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        // the end is square now (it stops at x = 60), and the wall that met it is square too
        assert!((reach(&d, ids[0], 0) - 60.0).abs() < 1e-9, "{:?}", plan(&d, ids[0]));
        assert!(plan(&d, ids[1]) != before.1 && plan(&d, ids[0]) != before.0);
        assert!(d.wall_join_commands(ids[0], false, false).is_err());
        // saved and restored
        let (els, _) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        assert_eq!(els.iter().find(|e| e.id == ids[0]).unwrap().join_ends, [true, false]);
        d.undo().unwrap();
        assert_eq!((plan(&d, ids[0]), plan(&d, ids[1])), before);
        // the other wall can refuse as well, and allowing it again restores the mitre
        let ops = d.wall_join_commands(ids[1], true, false).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(plan(&d, ids[0]) != before.0);
        let ops = d.wall_join_commands(ids[1], true, true).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!((plan(&d, ids[0]), plan(&d, ids[1])), before);
        assert!(d.wall_join_commands(ElementId(10), true, false).is_err());
    }
    #[test]
    fn an_equipment_pad_is_made_by_its_unit_and_moves_copies_and_goes_with_it() {
        let mut d = Document::spatial_sample();
        let fan = d.place_family(ElementId(10), [2.0, 4.5], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 2.2 }).unwrap();
        let spec = FootingSpec { width_m: 1.0, length_m: 1.0, thickness_m: 0.15, top_offset_m: 0.0, material: Material::Concrete };
        let pad = d.place_equipment_pad(fan, &spec).unwrap();
        assert_eq!(d.get(pad).unwrap().parent, Some(fan));
        // a pad cannot be made under something that is not a unit
        assert!(d.place_equipment_pad(ElementId(10), &spec).is_err());
        let at = |d: &Document, id| match d.get(id).unwrap().geometry { Some(Geometry::Pad { at, .. }) => at, _ => panic!() };
        // moving the unit moves the pad
        let ops = d.transform_commands(&[fan], Xf::Move([1.0, 0.5])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(at(&d, pad), [3.0, 5.0]));
        // deleting the unit takes the pad
        let ops = d.delete_commands(&[fan]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(d.get(pad).is_none());
    }
}
