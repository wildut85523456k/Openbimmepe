//! Editing what is already drawn, as Revit does: delete (with what stands on it), move, copy, rotate and mirror. Each
//! gives the commands of one undoable step; the model is not changed until they are applied.
use super::{Category, Command, Document, Element, ElementId, Geometry};
use super::mep::MepNode;
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
                let host = match e.geometry { Some(Geometry::InWall { host, .. } | Geometry::OnWall { host } | Geometry::OnMember { host } | Geometry::Tag { host, .. }) => Some(host), _ => None };
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
        // tags first: deleting their host would take them anyway
        let is_tag = |i: &ElementId| self.get(*i).is_some_and(|e| matches!(e.geometry, Some(Geometry::Tag { .. })));
        let (tags, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(is_tag);
        Ok(tags.into_iter().chain(rest).map(|id| Command::Delete { id }).collect())
    }

    /// The element as it is after `xf`; an error says why it cannot be.
    fn transformed(&self, e: &Element, xf: Xf) -> Result<Element, String> {
        let mut e = e.clone();
        let p = |q: P| xf.point(q);
        match e.geometry {
            Some(Geometry::Grid { start, end }) => e.geometry = Some(Geometry::Grid { start: p(start), end: p(end) }),
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
                if xf.flips() { e.outline.reverse(); }
                e.geometry = Some(Geometry::Area { level });
            }
            Some(Geometry::Tag { host, at, anchor }) => e.geometry = Some(Geometry::Tag { host, at: p(at), anchor: p(anchor) }),
            Some(Geometry::Dimension { a, b, at, level }) => e.geometry = Some(Geometry::Dimension { a: p(a), b: p(b), at: p(at), level }),
            // hosted by something else: it goes where its host goes
            Some(Geometry::InWall { .. } | Geometry::OnWall { .. } | Geometry::OnMember { .. }) => {}
            Some(Geometry::Level) | None => return Err(format!("{} cannot be moved in the plan", e.name)),
        }
        Ok(e)
    }

    /// The elements `ids` and what goes with them, each already changed by `xf`.
    fn transformed_set(&self, ids: &[ElementId], xf: Xf, for_copy: bool) -> Result<Vec<Element>, String> {
        let all = self.with_dependents(ids);
        if all.is_empty() { return Err("nothing selected".into()); }
        let mut out = Vec::new();
        for id in all {
            let e = self.get(id).ok_or("an element is gone")?;
            if matches!(e.category, Category::Level | Category::Type) { return Err(format!("{} cannot be moved in the plan", e.name)); }
            // a duct or pipe joined to fittings would leave them behind
            if !for_copy && matches!(e.category, Category::Duct | Category::Pipe) && self.run_is_joined(id) {
                return Err(format!("{} is joined to fittings or equipment; moving a joined network is not supported yet", e.name));
            }
            out.push(self.transformed(e, xf)?);
        }
        Ok(out)
    }

    /// Moves, turns or mirrors `ids` and what goes with them (their doors, windows, footings, tags, bars and members).
    pub fn transform_commands(&self, ids: &[ElementId], xf: Xf) -> Result<Vec<Command>, String> {
        let moved = self.transformed_set(ids, xf, false)?;
        let mut ops = Vec::new();
        for e in moved {
            let old = self.get(e.id).ok_or("an element is gone")?;
            if e.geometry != old.geometry { ops.push(Command::SetGeometry { id: e.id, geometry: e.geometry.ok_or("no geometry")? }); }
            if e.outline != old.outline { ops.push(Command::SetOutline { id: e.id, outline: e.outline.clone() }); }
            if e.angle_deg != old.angle_deg && e.category == Category::Family {
                ops.push(Command::SetFamily { id: e.id, family: e.family.clone(), type_name: e.type_name.clone(), overrides: e.overrides.clone(), angle_deg: e.angle_deg });
            }
        }
        if ops.is_empty() { return Err("nothing changes".into()); }
        Ok(ops)
    }

    /// Drags one end of a wall to `to`, as the grip of a wall does in Revit: the ends of other walls that meet it
    /// there go with it, so that the corner stays joined.
    pub fn wall_end_commands(&self, id: ElementId, at_start: bool, to: P) -> Result<Vec<Command>, String> {
        let wall = self.get(id).filter(|e| e.category == Category::Wall).ok_or("not a wall")?;
        let Some(Geometry::Linear { start, end, level }) = wall.geometry else { return Err("not a wall".into()) };
        let old = if at_start { start } else { end };
        if (to[0] - old[0]).hypot(to[1] - old[1]) < 1e-9 { return Err("nothing changes".into()); }
        let close = |a: P| (a[0] - old[0]).hypot(a[1] - old[1]) < 1e-3;
        let mut ops = Vec::new();
        for e in self.elements().filter(|e| e.category == Category::Wall) {
            let Some(Geometry::Linear { start: s, end: t, level: l }) = e.geometry else { continue };
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

    /// Copies `ids` and what goes with them, changed by `xf`: the commands and the numbers of the copies of `ids`.
    pub fn copy_commands(&self, ids: &[ElementId], xf: Xf) -> Result<(Vec<Command>, Vec<ElementId>), String> {
        let copies = self.transformed_set(ids, xf, true)?;
        let mut next = self.next_id().0;
        let map: BTreeMap<ElementId, ElementId> = copies.iter().map(|e| { let n = ElementId(next); next += 1; (e.id, n) }).collect();
        let re = |h: ElementId| map.get(&h).copied().unwrap_or(h);
        let mut ops = Vec::new();
        for mut e in copies {
            e.id = map[&e.id];
            e.parent = e.parent.map(re);
            e.geometry = e.geometry.map(|g| match g {
                Geometry::InWall { host, along_m } => Geometry::InWall { host: re(host), along_m },
                Geometry::OnWall { host } => Geometry::OnWall { host: re(host) },
                Geometry::OnMember { host } => Geometry::OnMember { host: re(host) },
                Geometry::Tag { host, at, anchor } => Geometry::Tag { host: re(host), at, anchor },
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
    use crate::FamilySpec;

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
        // a dimension between two points is saved and moves with the move command
        let dim = d.place_dimension(level, [0.0, 0.0], [3.0, 0.0], [1.5, 1.0]).unwrap();
        let ops = d.transform_commands(&[dim], Xf::Move([1.0, 1.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(matches!(d.get(dim).unwrap().geometry, Some(Geometry::Dimension { a, b, at, .. }) if near(a, [1.0, 1.0]) && near(b, [4.0, 1.0]) && near(at, [2.5, 2.0])));
        assert!(d.place_dimension(level, [0.0, 0.0], [0.0, 0.0], [1.0, 1.0]).is_err());
        let (els, _) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        assert_eq!(els.iter().find(|e| e.id == dim).unwrap().geometry, d.get(dim).unwrap().geometry);
    }
}
