//! The 3D view as a widget for egui, drawn with wgpu. `gpu` holds the pipelines; `widget` the input handling and
//! the ViewCube.
pub mod gpu;
pub mod widget;

pub use widget::{CubeLabels, Viewport3d};
pub use openbim_viewport::{SectionBox, Style};
