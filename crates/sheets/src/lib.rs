//! Sheets, title blocks and printing, the Revit way.
//!
//! * a **title block** is a template for one paper size: border, fixed lines and boxes, text bound to
//!   named fields (`{Sheet Number}`), a revision table;
//! * a **sheet** carries a number, a name, parameters, viewports (a view at a scale) and revision clouds;
//! * a **view** has a discipline, a detail level, per-category overrides, rule filters and a template;
//!   other disciplines are shown, halftoned or hidden, as the view says;
//! * a **sheet set** can be created or updated from an Excel file whose column names match field names;
//! * printing writes vector PDF and DXF at true scale.
//!
//! No standard values are built in: paper sizes are geometry, margins, line weights, templates and
//! discipline defaults are editable data. Everything is a pure function of its inputs.

pub mod dxf;
pub mod export;
pub mod import;
pub mod model;
pub mod paper;
pub mod pdf;
pub mod profile;
pub mod render;
pub mod titleblock;
pub mod visibility;

pub const MAX_NAME: usize = 128;
pub const MAX_TEXT: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum SheetError {
    NotFinite(&'static str),
    BadPaper(String),
    BadName { what: &'static str, value: String },
    Duplicate { what: &'static str, value: String },
    Unknown { what: &'static str, value: String },
    TooMany(&'static str),
    BadTemplate(String),
    OutOfBounds(String),
    BadScale(u32),
    BadValue { what: &'static str, value: String },
    Import(String),
}

impl std::fmt::Display for SheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use SheetError::*;
        match self {
            NotFinite(w) => write!(f, "{w} is not a finite number"),
            BadPaper(s) => write!(f, "paper size: {s}"),
            BadName { what, value } => write!(f, "{what} '{value}' is empty, too long or has forbidden characters"),
            Duplicate { what, value } => write!(f, "{what} '{value}' is used twice"),
            Unknown { what, value } => write!(f, "no {what} called '{value}'"),
            TooMany(w) => write!(f, "too many {w}"),
            BadTemplate(s) => write!(f, "text template: {s}"),
            OutOfBounds(s) => write!(f, "{s} lies outside the sheet"),
            BadScale(n) => write!(f, "scale 1:{n} is not acceptable"),
            BadValue { what, value } => write!(f, "{what} '{value}' is not acceptable"),
            Import(s) => write!(f, "import: {s}"),
        }
    }
}

impl std::error::Error for SheetError {}

pub(crate) fn finite(v: f64, what: &'static str) -> Result<f64, SheetError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(SheetError::NotFinite(what))
    }
}

/// Names and free text: no control characters, not empty, not longer than the limit.
pub(crate) fn clean_text(v: &str, limit: usize) -> bool {
    !v.trim().is_empty() && v.chars().count() <= limit && !v.chars().any(|c| c.is_control())
}

pub(crate) fn clean_name(v: &str, what: &'static str) -> Result<(), SheetError> {
    if clean_text(v, MAX_NAME) {
        Ok(())
    } else {
        Err(SheetError::BadName { what, value: v.to_string() })
    }
}
