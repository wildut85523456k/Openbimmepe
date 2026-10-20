//! openbim-schematic: the pure grid layout engine behind `MEP: Generate Schematic` (doc 06, SCH-002).
//!
//! The crate has no model, I/O or rendering dependency: [`layout`] is a pure function from the
//! extracted schematic data to symbol slots and wire routes, golden-tested against
//! `docs/revit-style-ui/fixtures/schematic-layout-v1.json`. The wiring from a document draft to
//! this input is SCH-003.
#![forbid(unsafe_code)]

pub mod layout;

pub use layout::{
    layout, Connection, Layout, LayoutElement, LayoutInput, LayoutLevel, Slot, SpacingMode, WireRoute,
    DEFAULT_COLUMN_PITCH, DEFAULT_MIN_SLOT, DEFAULT_ROW_PITCH, MAX_PASSES,
};
