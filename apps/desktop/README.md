# OpenBIM desktop (Rust / egui)

The desktop uses Rust, egui and wgpu. There is no Qt or web shell.

## Run

```sh
cargo run --locked -p openbim-desktop
```

On Windows the MSVC build tools and Windows SDK are required. Retain the tested
local Cargo.lock correction to windows 0.62.0; this change does not modify it.
On Linux install the usual X11/Wayland and graphics libraries.

## Current features

- Reviewed 3D icon artwork, DPI-aware filtering, quick-access bar, 14 ribbon tabs,
  Systems discipline sub-tabs, contextual parameter controls and Light/Dark themes.
- Compact Properties palette and searchable Project Browser with adjustable widths
  and split, close/restore, and double-click view opening.
- Document-driven Level 1, Level 2 and Mechanical plan footprints and orbitable
  orthographic 3D views of actual in-memory walls, grids, levels, ducts and pipes.
- Geometry picking, category-specific parameters, endpoint editing, finite signed
  slope, Apply/Reset and atomic Undo. Level changes move dependent geometry.
- Wheel zoom, middle-drag pan, right-drag 3D orbit and Fit.

The initial sample is created through document transactions and excluded from
user Undo history. Changes are in memory and disappear when the app closes.

## Scope and review

See `docs/spatial-model-rendering.md` for controls, geometry contracts, review
steps and renderer limitations. `docs/palettes-contextual-editing.md` records the
previous UI milestone, whose illustrative-renderer limits are now superseded.

The renderer is a small-model, depth-sorted orthographic projection. Full view
ranges/cut planes, hidden-surface handling, joined/layered walls, openings, slabs,
placement tools, grips, on-canvas dimensions, full Revit shortcuts, docking,
persistence, IFC integration and collaboration remain future work.

`crates/document` is still separate from `crates/oplog` (issue #24). The interfaces
are kept transaction-based so subsequent integration can use the same edits.