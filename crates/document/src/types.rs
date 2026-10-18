//! Types: a named set of values (size, section, material, usage, bars) that many elements share. An element with a
//! `type_id` always has the values of its type; changing the type changes all of them in one transaction, so nothing
//! else in the document (cut lengths, meshes, bars, exports) needs to know that types exist.
use crate::{Category, Command, Document, EditError, Element, ElementId, Material, Section, Usage};

/// The values a type holds. Which of them count depends on what the type is a type of; the others stay at their
/// defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TypeValues {
    pub width_m: f64,
    pub height_m: f64,
    /// rebar: the cover
    pub elevation_m: f64,
    pub section: Section,
    pub material: Material,
    pub usage: Usage,
    /// rebar: the distance between stirrups
    pub spacing_m: f64,
    /// rebar: the bars along the bottom
    pub count: u32,
}

impl TypeValues {
    /// The values of `el` that a type of its kind governs.
    pub fn of(el: &Element) -> Self {
        let all = Self {
            width_m: el.width_m, height_m: el.height_m, elevation_m: el.elevation_m, section: el.section,
            material: el.material, usage: el.usage, spacing_m: el.spacing_m, count: el.count,
        };
        all.governed(el.kind())
    }

    /// Only the values a type of `kind` governs.
    pub fn governed(self, kind: Category) -> Self {
        let own = Self { width_m: self.width_m, height_m: self.height_m, ..Self::default() };
        match kind {
            Category::Beam => Self { section: self.section, material: self.material, usage: self.usage, ..own },
            Category::Column => Self { section: self.section, material: self.material, ..own },
            Category::WallFooting | Category::Slab => Self { material: self.material, ..own },
            Category::Rebar => Self { elevation_m: self.elevation_m, spacing_m: self.spacing_m, count: self.count, ..own },
            _ => own,
        }
    }

    /// The commands that give `id`, an element or a type of `kind`, these values.
    pub fn commands(&self, id: ElementId, kind: Category) -> Vec<Command> {
        let mut ops = vec![Command::Resize { id, width_m: self.width_m, height_m: self.height_m }];
        match kind {
            Category::Beam => ops.extend([Command::SetSection { id, section: self.section }, Command::SetMaterial { id, material: self.material }, Command::SetUsage { id, usage: self.usage }]),
            Category::Column => ops.extend([Command::SetSection { id, section: self.section }, Command::SetMaterial { id, material: self.material }]),
            Category::WallFooting | Category::Slab => ops.push(Command::SetMaterial { id, material: self.material }),
            Category::Rebar => ops.extend([Command::MoveElevation { id, elevation_m: self.elevation_m }, Command::SetBeamSystem { id, spacing_m: self.spacing_m, angle_deg: 0.0 }, Command::SetCount { id, count: self.count }]),
            _ => {}
        }
        ops
    }
}

impl Command {
    /// The same command for another element, when it sets a value that a type of `kind` governs.
    pub fn retarget(&self, kind: Category, to: ElementId) -> Option<Command> {
        let size = matches!(self, Command::Resize { .. });
        let section = matches!(self, Command::SetSection { .. }) && matches!(kind, Category::Beam | Category::Column);
        let material = matches!(self, Command::SetMaterial { .. }) && matches!(kind, Category::Beam | Category::Column | Category::WallFooting | Category::Slab);
        let usage = matches!(self, Command::SetUsage { .. }) && kind == Category::Beam;
        let bars = matches!(self, Command::MoveElevation { .. } | Command::SetBeamSystem { .. } | Command::SetCount { .. }) && kind == Category::Rebar;
        if !(size || section || material || usage || bars) { return None; }
        let mut c = self.clone();
        match &mut c {
            Command::Resize { id, .. } | Command::SetSection { id, .. } | Command::SetMaterial { id, .. } | Command::SetUsage { id, .. }
            | Command::MoveElevation { id, .. } | Command::SetBeamSystem { id, .. } | Command::SetCount { id, .. } => *id = to,
            _ => return None,
        }
        Some(c)
    }
}

impl Document {
    /// The commands that carry an edit made to the element `id` over to its type and to every other element of that
    /// type, so that the edit changes them all together. Nothing when the element has no type.
    pub fn follow_type(&self, id: ElementId, commands: &[Command]) -> Vec<Command> {
        let Some(el) = self.get(id) else { return Vec::new() };
        let Some(t) = el.type_id else { return Vec::new() };
        let others: Vec<ElementId> = std::iter::once(t).chain(self.instances_of(t).into_iter().filter(|i| *i != id)).collect();
        commands.iter().filter(|c| matches!(c, Command::Resize { id: x, .. } | Command::SetSection { id: x, .. } | Command::SetMaterial { id: x, .. } | Command::SetUsage { id: x, .. } | Command::MoveElevation { id: x, .. } | Command::SetBeamSystem { id: x, .. } | Command::SetCount { id: x, .. } if *x == id))
            .flat_map(|c| others.iter().filter_map(move |o| c.retarget(el.category, *o))).collect()
    }

    /// The types of `category`, by name.
    pub fn types_for(&self, category: Category) -> Vec<&Element> {
        let mut types: Vec<&Element> = self.elements().filter(|e| e.category == Category::Type && e.type_for == Some(category)).collect();
        types.sort_by(|a, b| a.name.cmp(&b.name));
        types
    }

    /// The elements that have `type_id` as their type.
    pub fn instances_of(&self, type_id: ElementId) -> Vec<ElementId> {
        self.elements().filter(|e| e.type_id == Some(type_id)).map(|e| e.id).collect()
    }

    /// Makes a type called `name` out of the values of `from` and gives it to `from`, in one transaction.
    pub fn create_type(&mut self, from: ElementId, name: &str) -> Result<ElementId, EditError> {
        let el = self.get(from).ok_or(EditError::NotFound(from))?;
        if !Element::can_have_type(el.category) { return Err(EditError::UnsupportedProperty(from)); }
        let (category, values) = (el.category, TypeValues::of(el));
        let id = self.next_id();
        let mut ops = vec![
            Command::Create { id, category: Category::Type, name: name.trim().into(), width_m: values.width_m, height_m: values.height_m, elevation_m: 0.0 },
            Command::SetTypeFor { id, category },
        ];
        ops.extend(values.commands(id, category));
        ops.push(Command::SetType { id: from, type_id: Some(id) });
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// Gives `ids` the type `type_id`, and with it the values of the type, in one transaction.
    pub fn assign_type(&mut self, ids: &[ElementId], type_id: ElementId) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type).ok_or(EditError::NotFound(type_id))?;
        let (kind, values) = (ty.kind(), TypeValues::of(ty));
        let mut ops = Vec::new();
        for id in ids {
            if self.get(*id).ok_or(EditError::NotFound(*id))?.category != kind { return Err(EditError::UnsupportedProperty(*id)); }
            ops.extend(values.commands(*id, kind));
            ops.push(Command::SetType { id: *id, type_id: Some(type_id) });
        }
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Lets `ids` keep the values they have and no longer follow their type.
    pub fn detach_type(&mut self, ids: &[ElementId]) -> Result<(), EditError> {
        let ops: Vec<Command> = ids.iter().map(|id| Command::SetType { id: *id, type_id: None }).collect();
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Changes the values of a type, and those of every element of that type, in one transaction.
    pub fn edit_type(&mut self, type_id: ElementId, values: &TypeValues) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type).ok_or(EditError::NotFound(type_id))?;
        let kind = ty.kind();
        let values = values.governed(kind);
        let mut ops = values.commands(type_id, kind);
        for id in self.instances_of(type_id) { ops.extend(values.commands(id, kind)); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Deletes a type that no element uses.
    pub fn delete_type(&mut self, type_id: ElementId) -> Result<(), EditError> {
        if self.get(type_id).is_none_or(|t| t.category != Category::Type) { return Err(EditError::NotFound(type_id)); }
        if !self.instances_of(type_id).is_empty() { return Err(EditError::TypeInUse(type_id)); }
        self.transact(self.revision, &[Command::Delete { id: type_id }])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeamSpec, ColumnSpec, RebarSpec};

    fn beam(d: &mut Document, y: f64) -> ElementId { d.place_beam(ElementId(10), &BeamSpec::level([1.0, y], [7.0, y], 0.3, 0.5, 2.8)).unwrap() }

    #[test]
    fn a_type_is_made_from_a_beam_shared_by_others_and_changing_it_changes_them_all_in_one_undo() {
        let mut d = Document::spatial_sample();
        let (a, b, c) = (beam(&mut d, 1.0), beam(&mut d, 2.0), beam(&mut d, 3.0));
        let t = d.create_type(a, "Beam 300 x 500").unwrap();
        assert_eq!(d.get(a).unwrap().type_id, Some(t));
        assert_eq!(d.types_for(Category::Beam).len(), 1);
        d.assign_type(&[b], t).unwrap();
        assert_eq!(d.instances_of(t), vec![a, b]);
        // the third beam is free: it keeps its own values
        let mut v = TypeValues::of(d.get(t).unwrap());
        (v.width_m, v.height_m, v.material, v.usage) = (0.4, 0.6, Material::Concrete, Usage::Joist);
        let revision = d.revision();
        d.edit_type(t, &v).unwrap();
        for id in [a, b] {
            let e = d.get(id).unwrap();
            assert_eq!((e.width_m, e.height_m, e.material, e.usage), (0.4, 0.6, Material::Concrete, Usage::Joist));
        }
        assert_eq!(d.get(c).unwrap().width_m, 0.3);
        assert_eq!(d.revision(), revision + 1, "one transaction");
        assert!(d.undo().unwrap());
        assert_eq!((d.get(a).unwrap().width_m, d.get(b).unwrap().width_m, d.get(t).unwrap().width_m), (0.3, 0.3, 0.3));
    }

    #[test]
    fn a_member_of_a_type_cannot_be_given_other_values_one_by_one_until_it_is_detached() {
        let mut d = Document::spatial_sample();
        let a = beam(&mut d, 1.0);
        let t = d.create_type(a, "B").unwrap();
        let resize = Command::Resize { id: a, width_m: 0.35, height_m: 0.5 };
        assert_eq!(d.transact(d.revision(), std::slice::from_ref(&resize)), Err(EditError::InvalidGeometry(a)));
        assert_eq!(d.get(a).unwrap().width_m, 0.3, "nothing changed");
        d.detach_type(&[a]).unwrap();
        d.transact(d.revision(), &[resize]).unwrap();
        assert_eq!(d.get(t).unwrap().width_m, 0.3, "the type is not touched");
    }

    #[test]
    fn types_have_unique_names_per_kind_serve_one_kind_and_cannot_be_deleted_while_used() {
        let mut d = Document::spatial_sample();
        let (a, b) = (beam(&mut d, 1.0), beam(&mut d, 2.0));
        let col = d.place_column(ElementId(10), ElementId(11), [2.0, 4.5], &ColumnSpec { width_m: 0.3, depth_m: 0.3, base_offset_m: 0.0, section: Section::Rectangle, material: Material::Concrete }).unwrap();
        let t = d.create_type(a, "Same").unwrap();
        assert!(d.create_type(b, "Same").is_err(), "the name is taken");
        assert_eq!(d.get(b).unwrap().type_id, None, "a failed transaction leaves nothing");
        // a column type is not for a beam
        let ct = d.create_type(col, "Same").unwrap();
        assert!(d.assign_type(&[b], ct).is_err());
        assert_eq!(d.delete_type(t), Err(EditError::TypeInUse(t)));
        d.detach_type(&[a]).unwrap();
        d.delete_type(t).unwrap();
        assert!(d.get(t).is_none());
        // a type has no place and is not a thing that can be typed
        assert!(d.create_type(ct, "x").is_err());
    }

    #[test]
    fn a_rebar_type_carries_the_bars_and_the_stirrups() {
        let mut d = Document::spatial_sample();
        let a = beam(&mut d, 1.0);
        let r1 = d.place_rebar(a, &RebarSpec { count: 3, bar_m: 0.016, stirrup_m: 0.008, cover_m: 0.03, stirrup_spacing_m: 0.2 }).unwrap();
        let r2 = d.place_rebar(a, &RebarSpec { count: 2, bar_m: 0.02, stirrup_m: 0.01, cover_m: 0.04, stirrup_spacing_m: 0.15 }).unwrap();
        let t = d.create_type(r1, "T16 / R8").unwrap();
        d.assign_type(&[r2], t).unwrap();
        let e = d.get(r2).unwrap();
        assert_eq!((e.count, e.width_m, e.height_m, e.elevation_m, e.spacing_m), (3, 0.016, 0.008, 0.03, 0.2));
        let mut v = TypeValues::of(d.get(t).unwrap());
        v.count = 4;
        d.edit_type(t, &v).unwrap();
        assert_eq!(d.rebar_bars(d.get(r1).unwrap()).unwrap().len(), d.rebar_bars(d.get(r2).unwrap()).unwrap().len());
        assert_eq!(d.get(r1).unwrap().count, 4);
    }
}
