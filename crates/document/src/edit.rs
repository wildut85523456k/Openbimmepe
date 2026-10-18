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
}
