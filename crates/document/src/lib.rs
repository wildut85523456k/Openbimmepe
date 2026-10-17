//! Sprint 1 BIM document foundation.
//!
//! This is a minimal in-memory, typed, transaction-backed editing core.
//! It intentionally does NOT claim persistence, IFC import, a full dependency
//! engine, or central-model concurrency. Element IDs survive edits/undo.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
mod spatial;
pub use spatial::{Geometry, Mesh};

/// Document-local, stable element identity. Future versions require
/// a project-scoped ID allocation strategy for collaborative editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Wall,
    Duct,
    Pipe,
    Equipment,
    Level,
    Grid,
}

impl Category {
    pub fn supports_slope(self) -> bool { matches!(self, Self::Duct | Self::Pipe) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: ElementId,
    pub category: Category,
    pub name: String,
    /// Length-like dimensions in canonical metres, never display millimetres.
    pub width_m: f64,
    pub height_m: f64,
    pub elevation_m: f64,
    /// Signed rise / horizontal run, along the element's future start-to-end
    /// direction. UI percent = ratio * 100. No geometry is generated here.
    /// Only ducts and pipes support this parameter.
    pub slope_ratio: Option<f64>,
    /// Optional spatial definition; parameter-only elements remain supported.
    pub geometry: Option<Geometry>,
    /// Document-wide revision at the last successful change.
    pub revision: u64,
}

impl Element {
    fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && self.name.len() <= 512
            && self.width_m.is_finite()
            && self.height_m.is_finite()
            && self.elevation_m.is_finite()
            && self.width_m > 0.0
            && self.height_m > 0.0
            && match self.slope_ratio {
                Some(slope) => self.category.supports_slope() && slope.is_finite(),
                None => !self.category.supports_slope(),
            }
    }
}

/// Where the project is on the earth, and the clock its times are read on. It is a property of the project, not of
/// a view. The angle has the sign of `openbim_ifc::georef::Georef`: positive when project north (the Y axis) is turned
/// counter-clockwise from true north. Files carry the direction of true north in the project's plan, which has no sign
/// to get wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// degrees, north positive
    pub latitude: f64,
    /// degrees, east positive
    pub longitude: f64,
    /// degrees, project north turned counter-clockwise from true north
    pub angle_to_true_north_deg: f64,
    /// hours ahead of UTC of the project's clock
    pub utc_offset_hours: f64,
}

impl Site {
    fn valid(&self) -> bool {
        self.latitude.is_finite() && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite() && (-180.0..=180.0).contains(&self.longitude)
            && self.angle_to_true_north_deg.is_finite() && (-360.0..=360.0).contains(&self.angle_to_true_north_deg)
            && self.utc_offset_hours.is_finite() && (-12.0..=14.0).contains(&self.utc_offset_hours)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Set where the project is (latitude, longitude, angle to true north, clock offset).
    SetSite(Site),
    Create {
        id: ElementId,
        category: Category,
        name: String,
        width_m: f64,
        height_m: f64,
        elevation_m: f64,
    },
    Resize {
        id: ElementId,
        width_m: f64,
        height_m: f64,
    },
    MoveElevation {
        id: ElementId,
        elevation_m: f64,
    },
    SetSlope { id: ElementId, slope_ratio: f64 },
    SetGeometry { id: ElementId, geometry: Geometry },
    Delete { id: ElementId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    InvalidElement,
    AlreadyExists(ElementId),
    NotFound(ElementId),
    RevisionConflict { expected: u64, actual: u64 },
    EmptyTransaction,
    RevisionOverflow,
    UnsupportedProperty(ElementId),
    InvalidGeometry(ElementId),
    InvalidSite,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    elements: BTreeMap<ElementId, Element>,
    site: Option<Site>,
    revision: u64,
    /// Undo checkpoints are local, bounded by the hosting application's policy.
    history: Vec<(BTreeMap<ElementId, Element>, Option<Site>)>,
}

impl Document {
    pub fn new() -> Self { Self::default() }
    pub fn revision(&self) -> u64 { self.revision }
    /// Where the project is, if it has been set.
    pub fn site(&self) -> Option<Site> { self.site }
    pub fn get(&self, id: ElementId) -> Option<&Element> { self.elements.get(&id) }
    pub fn elements(&self) -> impl Iterator<Item = &Element> { self.elements.values() }
    pub fn can_undo(&self) -> bool { !self.history.is_empty() }
    /// Establish a loaded/new sample as the undo baseline, without changing its revision.
    pub fn clear_undo_history(&mut self) { self.history.clear(); }

    /// Apply a batch atomically. A failed batch never updates state or history.
    /// The expected document revision protects against stale UI commands;
    /// centralized edits will need additional per-element semantic checks.
    pub fn transact(&mut self, expected: u64, ops: &[Command]) -> Result<u64, EditError> {
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        if self.revision != expected {
            return Err(EditError::RevisionConflict { expected, actual: self.revision });
        }
        let revision = self.revision.checked_add(1).ok_or(EditError::RevisionOverflow)?;
        let mut trial = self.elements.clone();
        let mut trial_site = self.site;
        for op in ops {
            match op {
                Command::Create { id, category, name, width_m, height_m, elevation_m } => {
                    if trial.contains_key(id) { return Err(EditError::AlreadyExists(*id)); }
                    let element = Element {
                        id: *id, category: *category, name: name.clone(),
                        width_m: *width_m, height_m: *height_m,
                        elevation_m: *elevation_m, revision,
                        slope_ratio: category.supports_slope().then_some(0.0),
                        geometry: None,
                    };
                    if !element.valid() { return Err(EditError::InvalidElement); }
                    trial.insert(*id, element);
                }
                Command::Resize { id, width_m, height_m } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if matches!(el.category, Category::Level | Category::Grid) {
                        return Err(EditError::UnsupportedProperty(*id));
                    }
                    el.width_m = *width_m;
                    el.height_m = *height_m;
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::MoveElevation { id, elevation_m } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category == Category::Grid { return Err(EditError::UnsupportedProperty(*id)); }
                    el.elevation_m = *elevation_m;
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::SetSlope { id, slope_ratio } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !el.category.supports_slope() { return Err(EditError::UnsupportedProperty(*id)); }
                    el.slope_ratio = Some(*slope_ratio);
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::SetGeometry { id, geometry } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    el.geometry = Some(*geometry);
                    el.revision = revision;
                }
                Command::SetSite(site) => {
                    if !site.valid() { return Err(EditError::InvalidSite); }
                    trial_site = Some(*site);
                }
                Command::Delete { id } => {
                    if trial.remove(id).is_none() { return Err(EditError::NotFound(*id)); }
                }
            }
        }
        spatial::validate(&trial)?;
        self.history.push((std::mem::replace(&mut self.elements, trial), std::mem::replace(&mut self.site, trial_site)));
        self.revision = revision;
        Ok(revision)
    }

    /// Undo the most recent complete transaction. Does not reuse a revision
    /// number, because undo is itself a document change.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        if self.history.is_empty() { return Ok(false); }
        let revision = self.revision.checked_add(1).ok_or(EditError::RevisionOverflow)?;
        (self.elements, self.site) = self.history.pop().expect("checked history");
        self.revision = revision;
        // Elements restored by undo are changed at the new document revision.
        for el in self.elements.values_mut() { el.revision = revision; }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(id: u64) -> Command {
        Command::Create {
            id: ElementId(id), category: Category::Duct, name: "Supply Air".into(),
            width_m: 0.2, height_m: 0.3, elevation_m: 2.7,
        }
    }

    #[test]
    fn the_site_is_validated_set_and_undone_with_the_elements() {
        let site = Site { latitude: 51.5, longitude: -0.12, angle_to_true_north_deg: 30.0, utc_offset_hours: 0.0 };
        let mut d = Document::new();
        assert_eq!(d.site(), None);
        assert_eq!(d.transact(0, &[Command::SetSite(site)]), Ok(1));
        assert_eq!(d.site(), Some(site));
        for bad in [Site { latitude: 91.0, ..site }, Site { longitude: -181.0, ..site }, Site { angle_to_true_north_deg: f64::NAN, ..site }, Site { utc_offset_hours: 20.0, ..site }] {
            assert_eq!(d.transact(1, &[Command::SetSite(bad)]), Err(EditError::InvalidSite));
        }
        assert_eq!((d.revision(), d.site()), (1, Some(site)), "a refused edit changes nothing");
        // one transaction: the site and an element together, and one undo takes both back
        let moved = Site { angle_to_true_north_deg: -45.0, ..site };
        assert_eq!(d.transact(1, &[create(7), Command::SetSite(moved)]), Ok(2));
        assert_eq!(d.undo(), Ok(true));
        assert_eq!((d.site(), d.get(ElementId(7)).is_none()), (Some(site), true));
        assert_eq!(d.undo(), Ok(true));
        assert_eq!(d.site(), None);
    }

    #[test]
    fn create_resize_and_undo_are_atomic() {
        let mut d = Document::new();
        assert_eq!(d.transact(0, &[create(1)]), Ok(1));
        assert_eq!(d.transact(1, &[Command::Resize {
            id: ElementId(1), width_m: 0.5, height_m: 0.4,
        }]), Ok(2));
        assert_eq!(d.get(ElementId(1)).unwrap().width_m, 0.5);
        assert_eq!(d.undo(), Ok(true));
        assert_eq!(d.revision(), 3);
        assert_eq!(d.get(ElementId(1)).unwrap().width_m, 0.2);
    }

    #[test]
    fn invalid_batch_rolls_back_completely() {
        let mut d = Document::new();
        let result = d.transact(0, &[create(1), Command::Resize {
            id: ElementId(1), width_m: f64::NAN, height_m: 0.4,
        }]);
        assert_eq!(result, Err(EditError::InvalidElement));
        assert_eq!(d.revision(), 0);
        assert_eq!(d.elements().count(), 0);
        assert!(!d.can_undo());
    }

    #[test]
    fn stale_revision_rejected() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        assert_eq!(d.transact(0, &[create(2)]),
            Err(EditError::RevisionConflict { expected: 0, actual: 1 }));
        assert!(d.get(ElementId(2)).is_none());
    }

    #[test]
    fn duplicate_and_missing_ids_rejected() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        assert_eq!(d.transact(1, &[create(1)]), Err(EditError::AlreadyExists(ElementId(1))));
        assert_eq!(d.transact(1, &[Command::Delete { id: ElementId(2) }]),
            Err(EditError::NotFound(ElementId(2))));
        assert_eq!(d.revision(), 1);
    }

    #[test]
    fn document_ids_remain_stable_after_undo() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        d.transact(1, &[Command::Delete { id: ElementId(1) }]).unwrap();
        d.undo().unwrap();
        assert_eq!(d.get(ElementId(1)).unwrap().id, ElementId(1));
    }

    #[test]
    fn duct_and_pipe_slope_edits_are_signed_and_undoable() {
        for category in [Category::Duct, Category::Pipe] {
            let mut d = Document::new();
            let mut command = create(1);
            if let Command::Create { category: kind, .. } = &mut command { *kind = category; }
            d.transact(0, &[command]).unwrap();
            d.transact(1, &[Command::SetSlope { id: ElementId(1), slope_ratio: -0.02 }]).unwrap();
            assert_eq!(d.get(ElementId(1)).unwrap().slope_ratio, Some(-0.02));
            d.undo().unwrap();
            assert_eq!(d.get(ElementId(1)).unwrap().slope_ratio, Some(0.0));
        }
    }

    #[test]
    fn invalid_slope_rolls_back_batch_and_other_categories_reject_slope() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        assert_eq!(d.transact(1, &[
            Command::Resize { id: ElementId(1), width_m: 0.8, height_m: 0.6 },
            Command::SetSlope { id: ElementId(1), slope_ratio: f64::INFINITY },
        ]), Err(EditError::InvalidElement));
        assert_eq!(d.get(ElementId(1)).unwrap().width_m, 0.2);
        assert_eq!(d.revision(), 1);
        for category in [Category::Wall, Category::Equipment] {
            let mut d = Document::new();
            let mut command = create(1);
            if let Command::Create { category: kind, .. } = &mut command { *kind = category; }
            d.transact(0, &[command]).unwrap();
            assert_eq!(d.get(ElementId(1)).unwrap().slope_ratio, None);
            assert_eq!(d.transact(1, &[Command::SetSlope { id: ElementId(1), slope_ratio: 0.02 }]),
                Err(EditError::UnsupportedProperty(ElementId(1))));
        }
    }
}
