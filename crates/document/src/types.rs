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
    /// pad footing: the thickness (a type keeps it in its `elevation_m`)
    pub thickness_m: f64,
}

impl TypeValues {
    /// The values of `el` that a type of its kind governs.
    pub fn of(el: &Element) -> Self {
        let thickness_m = match el.geometry {
            Some(crate::Geometry::Pad { thickness_m, .. }) => thickness_m,
            _ if el.category == Category::Type => el.elevation_m,
            _ => 0.0,
        };
        let all = Self {
            width_m: el.width_m, height_m: el.height_m, elevation_m: el.elevation_m, section: el.section,
            material: el.material, usage: el.usage, spacing_m: el.spacing_m, count: el.count, thickness_m,
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
            Category::Footing => Self { material: self.material, thickness_m: self.thickness_m, ..own },
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
            // for a type; `Document::values_ops` turns this into a change of the pad for an element
            Category::Footing => ops.extend([Command::SetMaterial { id, material: self.material }, Command::MoveElevation { id, elevation_m: self.thickness_m }]),
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
        let material = matches!(self, Command::SetMaterial { .. }) && matches!(kind, Category::Beam | Category::Column | Category::WallFooting | Category::Slab | Category::Footing);
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
        let pad = |o: ElementId, t: f64| -> Option<Command> {
            let e = self.get(o)?;
            match e.geometry {
                Some(crate::Geometry::Pad { at, level, .. }) => Some(Command::SetGeometry { id: o, geometry: crate::Geometry::Pad { at, level, thickness_m: t } }),
                _ if e.category == Category::Type => Some(Command::MoveElevation { id: o, elevation_m: t }),
                _ => None,
            }
        };
        let thickness = commands.iter().filter_map(|c| match c {
            Command::SetGeometry { id: x, geometry: crate::Geometry::Pad { thickness_m, .. } } if *x == id && el.category == Category::Footing => Some(*thickness_m),
            _ => None,
        }).flat_map(|t| others.iter().filter_map(move |o| pad(*o, t)).collect::<Vec<_>>()).collect::<Vec<_>>();
        commands.iter().filter(|c| matches!(c, Command::Resize { id: x, .. } | Command::SetSection { id: x, .. } | Command::SetMaterial { id: x, .. } | Command::SetUsage { id: x, .. } | Command::MoveElevation { id: x, .. } | Command::SetBeamSystem { id: x, .. } | Command::SetCount { id: x, .. } if *x == id))
            .flat_map(|c| others.iter().filter_map(move |o| c.retarget(el.category, *o))).chain(thickness).collect()
    }

    /// The commands that give `id`, an element or a type of `kind`, the values `v`. A pad's thickness is part of its
    /// geometry, so for a pad it is set there.
    fn values_ops(&self, id: ElementId, kind: Category, v: &TypeValues) -> Vec<Command> {
        let mut ops = v.commands(id, kind);
        if kind == Category::Footing {
            if let Some(crate::Geometry::Pad { at, level, .. }) = self.get(id).and_then(|e| e.geometry) {
                ops.retain(|c| !matches!(c, Command::MoveElevation { .. }));
                ops.push(Command::SetGeometry { id, geometry: crate::Geometry::Pad { at, level, thickness_m: v.thickness_m } });
            }
        }
        ops
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
        // a family instance's values are named, not a size: the type takes the family and those values with it
        let family = (category == Category::Family).then(|| (el.family.clone(), self.family_values(from)));
        let id = self.next_id();
        let mut ops = vec![
            Command::Create { id, category: Category::Type, name: name.trim().into(), width_m: values.width_m, height_m: values.height_m, elevation_m: 0.0 },
            Command::SetTypeFor { id, category },
        ];
        ops.extend(self.values_ops(id, category, &values));
        if let Some((family, named)) = family { ops.push(Command::SetFamilyType { id, family, values: named }); }
        ops.push(Command::SetType { id: from, type_id: Some(id) });
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// A new type of walls, `thickness_m` thick and `height_m` high, ready to be drawn with (no element has it yet).
    pub fn add_wall_type(&mut self, name: &str, thickness_m: f64, height_m: f64) -> Result<ElementId, EditError> {
        let name = name.trim();
        if name.is_empty() || thickness_m <= 0.0 || height_m <= 0.0 { return Err(EditError::InvalidElement); }
        if self.types_for(Category::Wall).iter().any(|t| t.name == name) { return Err(EditError::InvalidElement); }
        let id = self.next_id();
        let ops = vec![
            Command::Create { id, category: Category::Type, name: name.into(), width_m: thickness_m, height_m, elevation_m: 0.0 },
            Command::SetTypeFor { id, category: Category::Wall },
        ];
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// Gives `ids` the type `type_id`, and with it the values of the type, in one transaction.
    pub fn assign_type(&mut self, ids: &[ElementId], type_id: ElementId) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type).ok_or(EditError::NotFound(type_id))?;
        let (kind, values) = (ty.kind(), TypeValues::of(ty));
        // a type of a family serves that family only: a fan coil type is not a type of a door
        let family = (kind == Category::Family).then(|| ty.family.clone());
        let mut ops = Vec::new();
        for id in ids {
            let el = self.get(*id).ok_or(EditError::NotFound(*id))?;
            if el.category != kind { return Err(EditError::UnsupportedProperty(*id)); }
            if family.as_ref().is_some_and(|f| el.family != *f) { return Err(EditError::UnsupportedProperty(*id)); }
            ops.extend(self.values_ops(*id, kind, &values));
            ops.push(Command::SetType { id: *id, type_id: Some(type_id) });
        }
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Lets `ids` keep the values they have and no longer follow their type. A family instance's type gives it named
    /// values that are not stored on the instance, so detaching writes them into it first.
    pub fn detach_type(&mut self, ids: &[ElementId]) -> Result<(), EditError> {
        let mut ops: Vec<Command> = Vec::new();
        for id in ids {
            let el = self.get(*id).ok_or(EditError::NotFound(*id))?;
            if el.category == Category::Family && el.type_id.is_some() {
                let overrides = self.family_values(*id);
                ops.push(Command::SetFamily { id: *id, family: el.family.clone(), type_name: el.type_name.clone(), overrides, angle_deg: el.angle_deg });
            }
            ops.push(Command::SetType { id: *id, type_id: None });
        }
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Changes the values of a type, and those of every element of that type, in one transaction.
    pub fn edit_type(&mut self, type_id: ElementId, values: &TypeValues) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type).ok_or(EditError::NotFound(type_id))?;
        let kind = ty.kind();
        let values = values.governed(kind);
        let mut ops = self.values_ops(type_id, kind, &values);
        for id in self.instances_of(type_id) { ops.extend(self.values_ops(id, kind, &values)); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Gives a type another name; the name must be new among the types of its kind.
    pub fn rename_type(&mut self, type_id: ElementId, name: &str) -> Result<(), EditError> {
        if self.get(type_id).is_none_or(|t| t.category != Category::Type) { return Err(EditError::NotFound(type_id)); }
        if name.trim().is_empty() { return Err(EditError::InvalidElement); }
        self.transact(self.revision, &[Command::Rename { id: type_id, name: name.into() }])?;
        Ok(())
    }

    /// Makes a new type with the values of `type_id`, called `name`, that no element uses yet.
    pub fn duplicate_type(&mut self, type_id: ElementId, name: &str) -> Result<ElementId, EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type).ok_or(EditError::NotFound(type_id))?;
        let (kind, values) = (ty.kind(), TypeValues::of(ty));
        let layers = ty.is_layered().then(|| (ty.layers.clone(), ty.core));
        // a type of a family is its family and its named values as much as it is a size
        let family = (kind == Category::Family).then(|| (ty.family.clone(), ty.overrides.clone()));
        let id = self.next_id();
        let mut ops = vec![
            Command::Create { id, category: Category::Type, name: name.trim().into(), width_m: values.width_m, height_m: values.height_m, elevation_m: 0.0 },
            Command::SetTypeFor { id, category: kind },
        ];
        ops.extend(self.values_ops(id, kind, &values));
        if let Some((family, named)) = family { ops.push(Command::SetFamilyType { id, family, values: named }); }
        if let Some((layers, core)) = layers { ops.push(Command::SetLayers { id, layers, core }); }
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// A new type of the family `family` called `name`, with the named `values` it gives every instance of it. No
    /// element has it yet. The type is a `Category::Type` element like any other: `types_for(Category::Family)` finds
    /// it, its name is unique among the types of that kind, and an instance points at it through `type_id`.
    ///
    /// The names are not checked against the family's own inputs here: a family the library does not know can still
    /// have a type, and the check that a name is a real input belongs where the library is known (the catalogue
    /// loader, `crates/family::catalogue`).
    pub fn add_family_type(&mut self, family: &str, name: &str, values: &[(String, f64)]) -> Result<ElementId, EditError> {
        let (family, name) = (family.trim(), name.trim());
        if family.is_empty() || name.is_empty() || !Self::family_values_ok(values) { return Err(EditError::InvalidElement); }
        let id = self.next_id();
        let ops = vec![
            Command::Create { id, category: Category::Type, name: name.into(), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetTypeFor { id, category: Category::Family },
            Command::SetFamilyType { id, family: family.into(), values: values.to_vec() },
        ];
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// The named values a type of a family gives its instances, in the order they were set. Empty for anything else.
    pub fn family_type_values(&self, type_id: ElementId) -> Vec<(String, f64)> {
        self.get(type_id)
            .filter(|t| t.category == Category::Type && t.type_for == Some(Category::Family))
            .map(|t| t.overrides.clone())
            .unwrap_or_default()
    }

    /// The values a family instance works with: those of its type first, then its own, which win. This is the one
    /// place that knows how the two layers combine — the geometry, the properties panel, the schedules and the
    /// exports all ask for an instance's values here rather than reading `overrides` themselves.
    pub fn family_values(&self, id: ElementId) -> Vec<(String, f64)> {
        let Some(el) = self.get(id) else { return Vec::new() };
        let mut values = el.type_id.map(|t| self.family_type_values(t)).unwrap_or_default();
        for (n, v) in &el.overrides {
            match values.iter_mut().find(|(m, _)| m == n) {
                Some(slot) => slot.1 = *v,
                None => values.push((n.clone(), *v)),
            }
        }
        values
    }

    /// Changes the values of a type of a family, and with them the values of every instance that takes it, in one
    /// transaction. The instances are not rewritten: they resolve their type's values when they are read, so one
    /// transaction on the type is enough for all of them (and an instance's own values still win over it).
    pub fn edit_family_type(&mut self, type_id: ElementId, values: &[(String, f64)]) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type && t.type_for == Some(Category::Family)).ok_or(EditError::NotFound(type_id))?;
        if !Self::family_values_ok(values) { return Err(EditError::InvalidElement); }
        let family = ty.family.clone();
        self.transact(self.revision, &[Command::SetFamilyType { id: type_id, family, values: values.to_vec() }])?;
        Ok(())
    }

    /// Gives a family instance these named values, keeping the family, its library type and its turn. Values it does
    /// not name keep coming from its type.
    pub fn set_family_values(&mut self, id: ElementId, values: &[(String, f64)]) -> Result<(), EditError> {
        let el = self.get(id).filter(|e| e.category == Category::Family).ok_or(EditError::NotFound(id))?;
        if !Self::family_values_ok(values) { return Err(EditError::InvalidElement); }
        let (family, type_name, angle_deg) = (el.family.clone(), el.type_name.clone(), el.angle_deg);
        self.transact(self.revision, &[Command::SetFamily { id, family, type_name, overrides: values.to_vec(), angle_deg }])?;
        Ok(())
    }

    /// Whether a set of named values is one a family type may hold: named, bounded and finite, as `overrides` are.
    fn family_values_ok(values: &[(String, f64)]) -> bool {
        values.len() <= 64 && values.iter().all(|(n, v)| !n.trim().is_empty() && n.len() <= 64 && v.is_finite())
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
    use crate::{BeamSpec, ColumnSpec, FamilySpec, RebarSpec};

    fn beam(d: &mut Document, y: f64) -> ElementId { d.place_beam(ElementId(10), &BeamSpec::level([1.0, y], [7.0, y], 0.3, 0.5, 2.8)).unwrap() }

    fn fan_coil(d: &mut Document, at: [f64; 2]) -> ElementId {
        let spec = FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 2.6 };
        d.place_family(ElementId(10), at, &spec).unwrap()
    }

    fn named(values: &[(&str, f64)]) -> Vec<(String, f64)> { values.iter().map(|(n, v)| ((*n).to_string(), *v)).collect() }

    #[test]
    fn a_family_type_gives_its_named_values_to_every_instance_and_an_instance_value_wins() {
        let mut d = Document::spatial_sample();
        let (a, b) = (fan_coil(&mut d, [1.0, 1.0]), fan_coil(&mut d, [3.0, 1.0]));
        let t = d.add_family_type("Fan coil unit, ducted", "FCU 400", &named(&[("air_flow", 0.2), ("power", 120.0)])).unwrap();
        assert_eq!(d.get(t).unwrap().family, "Fan coil unit, ducted", "the type says which family it is a type of");
        assert_eq!(d.types_for(Category::Family).iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["FCU 400"]);
        assert_eq!(d.get(t).unwrap().type_for, Some(Category::Family));
        d.assign_type(&[a, b], t).unwrap();
        assert_eq!(d.family_values(a), named(&[("air_flow", 0.2), ("power", 120.0)]));
        // an instance's own value wins over its type's, and the type keeps its own
        d.set_family_values(a, &named(&[("air_flow", 0.3)])).unwrap();
        assert_eq!(d.family_values(a), named(&[("air_flow", 0.3), ("power", 120.0)]));
        assert_eq!(d.family_values(b), named(&[("air_flow", 0.2), ("power", 120.0)]), "the other instance is untouched");
        // one edit to the type reaches every instance in one transaction
        let revision = d.revision();
        d.edit_family_type(t, &named(&[("air_flow", 0.25), ("power", 150.0), ("duct_w", 0.5)])).unwrap();
        assert_eq!(d.revision(), revision + 1, "one transaction");
        assert_eq!(d.family_values(b), named(&[("air_flow", 0.25), ("power", 150.0), ("duct_w", 0.5)]));
        assert_eq!(d.family_values(a), named(&[("air_flow", 0.3), ("power", 150.0), ("duct_w", 0.5)]), "its own air flow still wins");
        assert!(d.undo().unwrap());
        assert_eq!(d.family_values(b), named(&[("air_flow", 0.2), ("power", 120.0)]));
        assert_eq!(d.family_values(a), named(&[("air_flow", 0.3), ("power", 120.0)]));
    }

    #[test]
    fn a_family_type_is_named_once_among_family_types_and_serves_its_own_family_only() {
        let mut d = Document::spatial_sample();
        let coil = fan_coil(&mut d, [1.0, 1.0]);
        let door = d.place_family_in_wall(ElementId(20), 1.0, &FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 0.0 }).unwrap();
        let t = d.add_family_type("Fan coil unit, ducted", "FCU 400", &named(&[("air_flow", 0.2)])).unwrap();
        assert!(d.add_family_type("Fan coil unit, ducted", "FCU 400", &[]).is_err(), "the name is taken");
        assert!(d.add_family_type("Fan coil unit, ducted", "fcu 400", &[]).is_err(), "however it is spelled");
        assert!(d.add_family_type("", "x", &[]).is_err() && d.add_family_type("Fan coil unit, ducted", "  ", &[]).is_err());
        assert!(d.add_family_type("Fan coil unit, ducted", "bad", &named(&[("", 1.0)])).is_err(), "an unnamed value");
        assert!(d.add_family_type("Fan coil unit, ducted", "bad", &named(&[("air_flow", f64::NAN)])).is_err(), "a value that is not a number");
        // the type of one family is not a type of another, and a wall type is not a type of a family instance
        assert_eq!(d.assign_type(&[door], t), Err(EditError::UnsupportedProperty(door)));
        let wt = d.add_wall_type("Exterior 200", 0.2, 3.0).unwrap();
        assert_eq!(d.assign_type(&[coil], wt), Err(EditError::UnsupportedProperty(coil)));
        d.assign_type(&[ElementId(20)], wt).unwrap();
        assert_eq!(d.get(ElementId(20)).unwrap().type_id, Some(wt), "a wall type still serves a wall");
        assert_eq!(d.family_values(coil), Vec::new(), "a refused assignment leaves nothing");
        assert_eq!(d.get(coil).unwrap().type_id, None);
    }

    #[test]
    fn a_family_instance_that_loses_its_type_keeps_the_values_it_had_and_a_type_copies_them() {
        let mut d = Document::spatial_sample();
        let a = fan_coil(&mut d, [1.0, 1.0]);
        d.set_family_values(a, &named(&[("air_flow", 0.3), ("duct_w", 0.6)])).unwrap();
        // a type made from the instance carries the family and the values it had
        let t = d.create_type(a, "FCU 500").unwrap();
        assert_eq!(d.get(t).unwrap().family, "Fan coil unit, ducted");
        assert_eq!(d.family_type_values(t), named(&[("air_flow", 0.3), ("duct_w", 0.6)]));
        let copy = d.duplicate_type(t, "FCU 600").unwrap();
        assert_eq!(d.family_type_values(copy), d.family_type_values(t), "a copy of a type of a family keeps its values");
        assert_eq!(d.get(copy).unwrap().family, "Fan coil unit, ducted");
        // detaching writes the type's values into the instance, so it keeps them
        let b = fan_coil(&mut d, [3.0, 1.0]);
        d.assign_type(&[b], t).unwrap();
        d.edit_family_type(t, &named(&[("air_flow", 0.45), ("duct_w", 0.6)])).unwrap();
        assert_eq!(d.family_values(b), named(&[("air_flow", 0.45), ("duct_w", 0.6)]));
        d.detach_type(&[b]).unwrap();
        let e = d.get(b).unwrap();
        assert_eq!(e.type_id, None);
        assert_eq!(d.family_values(b), named(&[("air_flow", 0.45), ("duct_w", 0.6)]), "it keeps what its type gave it");
        assert_eq!(e.family, "Fan coil unit, ducted", "and its family");
        assert!(d.undo().unwrap());
        assert_eq!(d.get(b).unwrap().type_id, Some(t), "detaching is one undoable step");
        assert_eq!(d.family_values(b), named(&[("air_flow", 0.45), ("duct_w", 0.6)]));
    }

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
    fn walls_share_a_thickness_and_height_and_pads_share_size_thickness_and_material_in_one_undo() {
        use crate::FootingSpec;
        let mut d = Document::spatial_sample();
        let (w1, w2) = (ElementId(20), ElementId(21));
        let wt = d.create_type(w1, "Wall 200").unwrap();
        d.assign_type(&[w2], wt).unwrap();
        let mut v = TypeValues::of(d.get(wt).unwrap());
        assert_eq!((v.width_m, v.height_m), (0.2, 3.0));
        (v.width_m, v.height_m) = (0.25, 3.2);
        d.edit_type(wt, &v).unwrap();
        for id in [w1, w2] { assert_eq!((d.get(id).unwrap().width_m, d.get(id).unwrap().height_m), (0.25, 3.2)); }
        assert_eq!(d.get(ElementId(30)).unwrap().width_m, 0.2, "a wall without the type is left alone");

        let spec = FootingSpec { width_m: 1.2, length_m: 1.0, thickness_m: 0.4, top_offset_m: -0.1, material: Material::Concrete };
        let p1 = d.place_footing(ElementId(10), [2.0, 2.0], &spec).unwrap();
        let p2 = d.place_footing(ElementId(10), [6.0, 2.0], &spec).unwrap();
        let p3 = d.place_footing(ElementId(10), [6.0, 5.0], &FootingSpec { thickness_m: 0.3, ..spec.clone() }).unwrap();
        let pt = d.create_type(p1, "Pad 1200").unwrap();
        assert_eq!(d.get(pt).unwrap().elevation_m, 0.4, "the type keeps the thickness");
        d.assign_type(&[p2, p3], pt).unwrap();
        let pad = |d: &Document, id| match d.get(id).unwrap().geometry { Some(crate::Geometry::Pad { thickness_m, at, .. }) => (thickness_m, at), _ => panic!() };
        assert_eq!(pad(&d, p3), (0.4, [6.0, 5.0]), "assigning the type gives its thickness and keeps the place");
        let mut v = TypeValues::of(d.get(pt).unwrap());
        (v.width_m, v.thickness_m, v.material) = (1.5, 0.6, Material::Steel);
        let revision = d.revision();
        d.edit_type(pt, &v).unwrap();
        assert_eq!(d.revision(), revision + 1);
        for id in [p1, p2, p3] {
            assert_eq!((d.get(id).unwrap().width_m, d.get(id).unwrap().material, pad(&d, id).0), (1.5, Material::Steel, 0.6));
        }
        assert_eq!(pad(&d, p2).1, [6.0, 2.0]);
        // a thickness edit made on one pad is carried to the others by follow_type
        let edit = [Command::SetGeometry { id: p1, geometry: crate::Geometry::Pad { at: [2.0, 2.0], level: ElementId(10), thickness_m: 0.5 } }];
        let mut all = edit.to_vec();
        all.extend(d.follow_type(p1, &edit));
        d.transact(d.revision(), &all).unwrap();
        for id in [p2, p3] { assert_eq!(pad(&d, id).0, 0.5); }
        assert_eq!(d.get(pt).unwrap().elevation_m, 0.5);
        assert_eq!(pad(&d, p3).1, [6.0, 5.0], "their places are untouched");
        assert!(d.undo().unwrap() && d.undo().unwrap());
        assert_eq!(pad(&d, p1).0, 0.4);
    }

    #[test]
    fn a_type_is_renamed_and_duplicated_and_a_taken_name_is_refused() {
        let mut d = Document::spatial_sample();
        let a = beam(&mut d, 1.0);
        let t = d.create_type(a, "B1").unwrap();
        d.rename_type(t, " B2 ").unwrap();
        assert_eq!(d.get(t).unwrap().name, "B2");
        let copy = d.duplicate_type(t, "B3").unwrap();
        assert_eq!(TypeValues::of(d.get(copy).unwrap()), TypeValues::of(d.get(t).unwrap()));
        assert!(d.instances_of(copy).is_empty());
        let revision = d.revision();
        assert!(d.duplicate_type(t, "b2").is_err() && d.rename_type(copy, "B2").is_err() && d.rename_type(copy, "  ").is_err());
        assert!(d.rename_type(a, "x").is_err(), "only types are renamed");
        assert_eq!(d.revision(), revision, "refusals change nothing");
        d.delete_type(copy).unwrap();
        assert_eq!(d.delete_type(t), Err(EditError::TypeInUse(t)));
        assert!(d.undo().unwrap());
        assert!(d.get(copy).is_some());
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
