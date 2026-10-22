//! Typed command and query API (AUTO-001).
//!
//! One Rust command and query API inside the app, shared by the ribbon and
//! shortcuts, OpenBIM Graph and the MCP server
//! (`docs/revit-style-ui/04-visual-automation.md`).
//!
//! Every value that crosses this API is a [`Quantity`]: an SI number plus a
//! [`Dimension`], so a dimension mismatch is a value error and not a silent unit
//! mix-up. Every parameter has a stable [`ParameterId`] with English and French
//! labels and an IFC property-set group, so a renamed label does not break a saved
//! graph. [`CommandDef`] is the registry the ribbon, keymap, graph node set and
//! MCP tools are generated from.
#![forbid(unsafe_code)]

pub mod command;
pub mod dimension;
pub mod parameter;
pub mod quantity;
pub mod unit;

pub use command::{groups, lookup, tab, CommandDef, InputSpec, ValueKind, BUILTIN};
pub use dimension::Dimension;
pub use parameter::{
    builtin, Parameter, ParameterId, ParameterRegistry, Resolve, ResolveError, BUILTIN_PARAMETERS,
};
pub use quantity::{DimensionMismatch, ParseError, Quantity};
pub use unit::{by_symbol, canonical, Unit, UNITS};
