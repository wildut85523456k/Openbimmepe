//! Revit's Modify tools that work on what exists: Pin and Unpin, Cut/Copy/Paste through a clipboard that outlives
//! the elements, Match Type Properties and Select All Instances.
use super::edit::Xf;
use super::{Category, Command, Document, EditError, Element, ElementId, Geometry};
use std::collections::BTreeMap;

type P = [f64; 2];

/// What Copy or Cut puts on the clipboard: the elements as they were, with what stands in or is tagged on them. It
/// holds the data, so it can be pasted after the originals are gone (Cut) and into another level.
#[derive(Clone, Debug, PartialEq)]
pub struct Clipboard {
    pub elements: Vec<Element>,
    /// The elements that were selected; the others came along with them.
    pub primary: Vec<ElementId>,
    /// The centre of what was selected in the plan: the point that follows the pointer when it is pasted.
    pub base: P,
}

/// The points of an element in the plan, for the centre of a selection and for the ghost of a paste.
pub fn plan_points(e: &Element) -> Vec<P> {
    match e.geometry {
        Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) => vec![start, end],
        Some(Geometry::Point { at, .. } | Geometry::Placed { at, .. } | Geometry::Pad { at, .. } | Geometry::Room { at, .. } | Geometry::Tag { at, .. }) => vec![at],
        Some(Geometry::Dimension { a, b, .. }) => vec![a, b],
        Some(Geometry::Area { .. } | Geometry::Shaft { .. }) => e.outline.clone(),
        _ => Vec::new(),
    }
}

impl Clipboard {
    /// The outlines of what is on the clipboard in the plan, after `d` is added: for the ghost of a paste. A line
    /// has two points, a point one, an area its corners.
    pub fn shapes(&self, d: P) -> Vec<Vec<P>> {
        self.elements.iter().map(|e| plan_points(e).into_iter().map(|p| [p[0] + d[0], p[1] + d[1]]).collect::<Vec<_>>()).filter(|v| !v.is_empty()).collect()
    }
}

/// The element moved to `level` when its geometry belongs to a level.
fn on_level(mut e: Element, level: ElementId) -> Element {
    e.geometry = e.geometry.map(|g| match g {
        Geometry::Linear { start, end, .. } => Geometry::Linear { start, end, level },
        Geometry::Point { at, top, .. } => Geometry::Point { at, level, top },
        Geometry::Placed { at, .. } => Geometry::Placed { at, level },
        Geometry::Pad { at, thickness_m, .. } => Geometry::Pad { at, level, thickness_m },
        Geometry::Area { .. } => Geometry::Area { level },
        Geometry::Room { at, .. } => Geometry::Room { at, level },
        Geometry::Dimension { a, b, at, .. } => Geometry::Dimension { a, b, at, level },
        other => other,
    });
    e
}

fn host_of(g: Option<Geometry>) -> Option<ElementId> {
    match g { Some(Geometry::InWall { host, .. } | Geometry::OnWall { host } | Geometry::OnMember { host } | Geometry::Tag { host, .. } | Geometry::ViewTag { host, .. }) => Some(host), _ => None }
}

impl Document {
    /// Pin or Unpin: the commands for the elements whose state changes. Levels and types cannot be pinned here.
    pub fn pin_commands(&self, ids: &[ElementId], pinned: bool) -> Result<Vec<Command>, String> {
        if ids.is_empty() { return Err("select something first".into()); }
        let mut ops = Vec::new();
        for id in ids {
            let e = self.get(*id).ok_or("an element is gone")?;
            if e.category == Category::Type { return Err(format!("{} is a type; only elements are pinned", e.name)); }
            if e.pinned != pinned { ops.push(Command::SetPinned { id: *id, pinned }); }
        }
        if ops.is_empty() { return Err(if pinned { "already pinned".into() } else { "none of them is pinned".into() }); }
        Ok(ops)
    }

    /// Copy or Cut: the selection and what goes with it, to be pasted later. A door, window or tag cannot be copied
    /// without the wall it stands on or is tagged on.
    pub fn clipboard(&self, ids: &[ElementId]) -> Result<Clipboard, String> {
        let all = self.with_dependents(ids);
        if all.is_empty() { return Err("select something first".into()); }
        let mut elements = Vec::new();
        for id in &all {
            let e = self.get(*id).ok_or("an element is gone")?;
            if matches!(e.category, Category::Level | Category::Type) { return Err(format!("{} cannot be copied to the clipboard", e.name)); }
            if let Some(h) = host_of(e.geometry) { if !all.contains(&h) { return Err(format!("{} belongs to another element: select that one too", e.name)); } }
            elements.push(e.clone());
        }
        let primary: Vec<ElementId> = ids.iter().copied().filter(|i| all.contains(i)).collect();
        let pts: Vec<P> = primary.iter().filter_map(|i| self.get(*i)).flat_map(plan_points).collect();
        if pts.is_empty() { return Err("this has no place in the plan to paste".into()); }
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &pts { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
        Ok(Clipboard { elements, primary, base: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0] })
    }

    /// Paste: copies of the clipboard moved by `d`, optionally onto `level` (Paste Aligned to a level). The commands
    /// and the numbers of the pasted elements that were selected when copied.
    pub fn paste_commands(&self, cb: &Clipboard, d: P, level: Option<ElementId>) -> Result<(Vec<Command>, Vec<ElementId>), String> {
        if let Some(l) = level { if self.get(l).is_none_or(|e| e.category != Category::Level) { return Err("pick a level to paste to".into()); } }
        let mut next = self.next_id().0;
        let map: BTreeMap<ElementId, ElementId> = cb.elements.iter().map(|e| { let n = ElementId(next); next += 1; (e.id, n) }).collect();
        let exists_or_new = |h: ElementId| -> Result<ElementId, String> {
            map.get(&h).copied().or_else(|| self.get(h).map(|_| h)).ok_or_else(|| "the element it stands on is gone".to_string())
        };
        let mut ops = Vec::new();
        for src in &cb.elements {
            let mut e = self.transformed(src, Xf::Move(d))?;
            if let Some(l) = level { if host_of(e.geometry).is_none() { e = on_level(e, l); } }
            e.id = map[&src.id];
            e.parent = e.parent.map(exists_or_new).transpose()?;
            if let Some(t) = e.type_id { if self.get(t).is_none() { return Err(format!("the type of {} was deleted", e.name)); } }
            e.geometry = match e.geometry {
                Some(Geometry::InWall { host, along_m }) => Some(Geometry::InWall { host: exists_or_new(host)?, along_m }),
                Some(Geometry::OnWall { host }) => Some(Geometry::OnWall { host: exists_or_new(host)? }),
                Some(Geometry::OnMember { host }) => Some(Geometry::OnMember { host: exists_or_new(host)? }),
                Some(Geometry::Tag { host, at, anchor }) => Some(Geometry::Tag { host: exists_or_new(host)?, at, anchor }),
                Some(Geometry::ViewTag { host, view, at, anchor }) => Some(Geometry::ViewTag { host: exists_or_new(host)?, view, at, anchor }),
                other => other,
            };
            e.pinned = false;
            ops.push(Command::Restore(Box::new(e)));
        }
        let firsts = cb.primary.iter().filter_map(|i| map.get(i).copied()).collect();
        Ok((ops, firsts))
    }

    /// Match Type Properties: gives `targets` the type of `source` (a family instance, its type name). They must be
    /// of the same kind as the source.
    pub fn match_type(&mut self, source: ElementId, targets: &[ElementId]) -> Result<(), String> {
        let s = self.get(source).ok_or("pick an element to match")?.clone();
        if s.category == Category::Type { return Err("pick an element, not a type".into()); }
        let mut ops = Vec::new();
        for t in targets.iter().filter(|t| **t != source) {
            let e = self.get(*t).ok_or("an element is gone")?;
            if e.category != s.category || e.family != s.family { return Err(format!("{} is not the same kind as {}", e.name, s.name)); }
            if s.category == Category::Family {
                if e.type_name != s.type_name {
                    ops.push(Command::SetFamily { id: *t, family: s.family.clone(), type_name: s.type_name.clone(), overrides: e.overrides.clone(), angle_deg: e.angle_deg });
                }
            }
        }
        if s.category == Category::Family {
            if ops.is_empty() { return Err("they are of that type already".into()); }
            return self.transact(self.revision(), &ops).map(|_| ()).map_err(|e| format!("{e:?}"));
        }
        let Some(tid) = s.type_id else { return Err(format!("{} has no type: make one from it first (Edit Type, Duplicate)", s.name)) };
        let todo: Vec<ElementId> = targets.iter().copied().filter(|t| *t != source && self.get(*t).is_some_and(|e| e.type_id != Some(tid))).collect();
        if todo.is_empty() { return Err("they are of that type already".into()); }
        self.assign_type(&todo, tid).map_err(|e| match e { EditError::UnsupportedProperty(_) => "they are not the same kind".to_string(), other => format!("{other:?}") })
    }

    /// Select All Instances: the elements of the same type as `id` (the same kind and values for an element with no
    /// type), in the whole project or only those on `level`.
    pub fn select_similar(&self, id: ElementId, level: Option<ElementId>) -> Vec<ElementId> {
        let Some(s) = self.get(id) else { return Vec::new() };
        let same = |e: &Element| -> bool {
            if e.category != s.category || e.category == Category::Type { return false; }
            if s.category == Category::Family { return e.family == s.family && e.type_name == s.type_name; }
            let same_values = e.type_id.is_none() && (e.width_m - s.width_m).abs() < 1e-9 && (e.height_m - s.height_m).abs() < 1e-9;
            s.type_id.map_or(same_values, |t| e.type_id == Some(t))
        };
        let mut out: Vec<ElementId> = self.elements().filter(|e| same(e) && level.is_none_or(|l| self.level_of(e.id).is_none_or(|x| x == l))).map(|e| e.id).collect();
        // the one picked comes first, as the primary element whose values the panels show
        out.retain(|i| *i != id);
        out.insert(0, id);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FamilySpec;

    fn wall_line(d: &Document, id: ElementId) -> (P, P) { match d.get(id).unwrap().geometry { Some(Geometry::Linear { start, end, .. }) => (start, end), g => panic!("{g:?}") } }
    fn door(d: &mut Document, wall: ElementId, along: f64) -> ElementId {
        let spec = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        d.place_family_in_wall(wall, along, &spec).unwrap()
    }

    #[test]
    fn a_pinned_element_cannot_be_moved_trimmed_dragged_or_deleted_until_it_is_unpinned() {
        let mut d = Document::spatial_sample();
        let w = ElementId(20);
        d.transact(d.revision(), &d.pin_commands(&[w], true).unwrap()).unwrap();
        assert!(d.get(w).unwrap().pinned);
        assert!(d.pin_commands(&[w], true).is_err(), "already pinned");
        for (what, r) in [
            ("move", d.transform_commands(&[w], Xf::Move([1.0, 0.0])).map(|_| ())),
            ("rotate", d.transform_commands(&[w], Xf::Rotate([0.0, 0.0], 0.5)).map(|_| ())),
            ("mirror", d.transform_commands(&[w], Xf::Mirror([0.0, 0.0], [1.0, 0.0])).map(|_| ())),
            ("delete", d.delete_commands(&[w]).map(|_| ())),
            ("grip", d.wall_end_commands(w, false, [7.0, 0.0]).map(|_| ())),
            ("length", d.set_wall_length_commands(w, 5.0).map(|_| ())),
            ("trim", d.trim_corner_commands(w, [1.0, 0.0], ElementId(23), [0.0, 1.0]).map(|_| ())),
        ] {
            assert!(r.as_ref().is_err_and(|e| e.contains("pinned")), "{what}: {r:?}");
        }
        // dragging the end of a free wall that is joined to the pinned one would move the pinned wall's end too
        assert!(d.wall_end_commands(ElementId(23), false, [0.0, 1.0]).is_err_and(|e| e.contains("pinned")));
        // copying is allowed, and the copy is not pinned
        let (ops, new) = d.copy_commands(&[w], Xf::Move([0.0, 10.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(!d.get(new[0]).unwrap().pinned);
        // pinning goes with the file and with undo
        let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        assert!(els.iter().find(|e| e.id == w).unwrap().pinned && !els.iter().find(|e| e.id == new[0]).unwrap().pinned);
        let _ = site;
        d.undo().unwrap();
        d.transact(d.revision(), &d.pin_commands(&[w], false).unwrap()).unwrap();
        assert!(!d.get(w).unwrap().pinned);
        assert!(d.pin_commands(&[w], false).is_err(), "not pinned");
        d.transact(d.revision(), &d.transform_commands(&[w], Xf::Move([1.0, 0.0])).unwrap()).unwrap();
        // a type is not pinned
        let t = d.create_type(ElementId(21), "Wall 200 x 3000").unwrap();
        assert!(d.pin_commands(&[t], true).is_err());
    }

    #[test]
    fn a_cut_wall_is_pasted_with_its_door_where_the_pointer_goes_and_onto_another_level() {
        let mut d = Document::spatial_sample();
        let w = ElementId(24);
        let dr = door(&mut d, w, 3.0);
        let tag = d.place_tag(w, [3.0, 1.0], [3.0, 0.0]).unwrap();
        let cb = d.clipboard(&[w]).unwrap();
        assert_eq!((cb.primary.clone(), cb.elements.len()), (vec![w], 3));
        assert!((cb.base[0] - 4.0).abs() < 1e-9 && (cb.base[1] - 3.0).abs() < 1e-9, "the centre of the wall: {:?}", cb.base);
        // Cut: the originals go, the clipboard keeps the data
        d.transact(d.revision(), &d.delete_commands(&[w]).unwrap()).unwrap();
        assert!(d.get(w).is_none() && d.get(dr).is_none() && d.get(tag).is_none());
        let before = d.elements().count();
        let (ops, new) = d.paste_commands(&cb, [1.0, 2.0], None).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!((d.elements().count(), new.len()), (before + 3, 1));
        let n = new[0];
        assert_eq!(wall_line(&d, n), ([5.0, 2.0], [5.0, 8.0]));
        // the pasted wall sits one metre and two metres from where the cut wall stood (Wall 5 ran (4,0) to (4,6))
        // the door and the tag belong to the pasted wall, not to a missing one
        let hosted: Vec<_> = d.elements().filter(|x| matches!(x.geometry, Some(Geometry::InWall { host, .. }) if host == n)).collect();
        assert_eq!(hosted.len(), 1);
        assert!(d.elements().any(|x| matches!(x.geometry, Some(Geometry::Tag { host, .. }) if host == n)));
        // one undo takes the whole paste back
        d.undo().unwrap();
        assert_eq!(d.elements().count(), before);
        // Paste Aligned to another level: the same place, on Level 2
        let (ops, up) = d.paste_commands(&cb, [0.0, 0.0], Some(ElementId(11))).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.level_of(up[0]), Some(ElementId(11)));
        assert_eq!(wall_line(&d, up[0]), ([4.0, 0.0], [4.0, 6.0]));
        // a level is not pasted to a thing that is not a level
        assert!(d.paste_commands(&cb, [0.0, 0.0], Some(ElementId(100))).is_err());
        // a level cannot go on the clipboard
        assert!(d.clipboard(&[ElementId(10)]).is_err());
    }

    #[test]
    fn a_door_cannot_be_copied_without_its_wall() {
        let mut d = Document::spatial_sample();
        let dr = door(&mut d, ElementId(24), 3.0);
        let err = d.clipboard(&[dr]).unwrap_err();
        assert!(err.contains("select that one too"), "{err}");
    }

    #[test]
    fn match_type_gives_walls_the_type_of_the_first_and_refuses_other_kinds() {
        let mut d = Document::spatial_sample();
        // the source has no type yet
        assert!(d.match_type(ElementId(20), &[ElementId(21)]).unwrap_err().contains("no type"));
        let t = d.create_type(ElementId(20), "Wall 200 x 3000").unwrap();
        // make the other wall different
        d.transact(d.revision(), &[Command::Resize { id: ElementId(21), width_m: 0.3, height_m: 2.5 }]).unwrap();
        d.match_type(ElementId(20), &[ElementId(21), ElementId(22)]).unwrap();
        for id in [21, 22] {
            let e = d.get(ElementId(id)).unwrap();
            assert_eq!((e.type_id, e.width_m, e.height_m), (Some(t), 0.2, 3.0));
        }
        assert!(d.match_type(ElementId(20), &[ElementId(21)]).unwrap_err().contains("already"));
        // not between a wall and a duct
        assert!(d.match_type(ElementId(20), &[ElementId(1)]).unwrap_err().contains("not the same kind"));
        // one undo restores both walls
        d.undo().unwrap();
        assert_eq!(d.get(ElementId(21)).unwrap().width_m, 0.3);
        assert_eq!(d.get(ElementId(22)).unwrap().type_id, None);
    }

    #[test]
    fn match_type_between_family_instances_needs_the_same_family() {
        let mut d = Document::spatial_sample();
        let a = door(&mut d, ElementId(24), 1.0);
        let b = door(&mut d, ElementId(24), 2.5);
        // two doors of one type: nothing to do
        assert!(d.match_type(a, &[b]).unwrap_err().contains("already"));
        // a door and a wall are not alike
        assert!(d.match_type(a, &[ElementId(20)]).unwrap_err().contains("not the same kind"));
    }

    #[test]
    fn select_all_instances_finds_the_elements_of_one_type_in_the_project_or_on_a_level() {
        let mut d = Document::spatial_sample();
        let t = d.create_type(ElementId(20), "Wall 200 x 3000").unwrap();
        d.assign_type(&[ElementId(21), ElementId(30)], t).unwrap();
        let all = d.select_similar(ElementId(21), None);
        assert_eq!(all[0], ElementId(21), "the one picked comes first");
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(sorted, vec![ElementId(20), ElementId(21), ElementId(30)]);
        let level1 = d.select_similar(ElementId(21), Some(ElementId(10)));
        let mut sorted = level1.clone();
        sorted.sort();
        assert_eq!(sorted, vec![ElementId(20), ElementId(21)]);
        // walls with no type are alike when their sizes are: the other three walls are 0.2 x 3.0, and so are 22 to 24
        let plain = d.select_similar(ElementId(22), None);
        assert_eq!(plain.len(), 3);
        // a duct does not match a wall
        assert_eq!(d.select_similar(ElementId(1), None), vec![ElementId(1)]);
    }
}
