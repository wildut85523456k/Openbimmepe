//! Sprint 1 BIM document foundation.
//!
//! This is a minimal in-memory, typed, transaction-backed editing core.
//! It intentionally does NOT claim persistence, IFC import, a full dependency
//! engine, or central-model concurrency. Element IDs survive edits/undo.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

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
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    elements: BTreeMap<ElementId, Element>,
    revision: u64,
    /// Undo checkpoints are local, bounded by the hosting application's policy.
    history: Vec<BTreeMap<ElementId, Element>>,
}

impl Document {
    pub fn new() -> Self { Self::default() }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn get(&self, id: ElementId) -> Option<&Element> { self.elements.get(&id) }
    pub fn elements(&self) -> impl Iterator<Item = &Element> { self.elements.values() }
    pub fn can_undo(&self) -> bool { !self.history.is_empty() }

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
        for op in ops {
            match op {
                Command::Create { id, category, name, width_m, height_m, elevation_m } => {
                    if trial.contains_key(id) { return Err(EditError::AlreadyExists(*id)); }
                    let element = Element {
                        id: *id, category: *category, name: name.clone(),
                        width_m: *width_m, height_m: *height_m,
                        elevation_m: *elevation_m, revision,
                    };
                    if !element.valid() { return Err(EditError::InvalidElement); }
                    trial.insert(*id, element);
                }
                Command::Resize { id, width_m, height_m } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    el.width_m = *width_m;
                    el.height_m = *height_m;
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::MoveElevation { id, elevation_m } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    el.elevation_m = *elevation_m;
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::Delete { id } => {
                    if trial.remove(id).is_none() { return Err(EditError::NotFound(*id)); }
                }
            }
        }
        self.history.push(std::mem::replace(&mut self.elements, trial));
        self.revision = revision;
        Ok(revision)
    }

    /// Undo the most recent complete transaction. Does not reuse a revision
    /// number, because undo is itself a document change.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        if self.history.is_empty() { return Ok(false); }
        let revision = self.revision.checked_add(1).ok_or(EditError::RevisionOverflow)?;
        self.elements = self.history.pop().expect("checked history");
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
}
