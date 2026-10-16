# OpenBIM desktop (egui)

On 15 Oct 2026 egui was chosen as the UI toolkit: the whole app is in Rust, drawn with wgpu next to the future 3D viewport. This replaces the Qt 6 and web-shell options (issue #22).

**Status:** this is a UI shell around a sample scene. Duct width, height, elevation and slope are stored in an in-memory Rust document and can be undone. There is no file save or load, no IFC, no model geometry and no collaboration yet.

## Run

```bash
cargo run -p openbim-desktop
```

On Linux the usual X11/Wayland and OpenGL libraries must be installed.

## What it has

- **Quick-access toolbar** with the 3D common icons: New, Open, Save, Undo, Redo, Measure and Settings.
- **14 ribbon tabs**, each with an icon. The Systems tab has HVAC, Plumbing & Piping, Electrical and All sub-tabs.
- **Ribbon panels** of large icon buttons, with the group title underneath, using the icons in `assets/icons/`. Every command has a stable ID, which is also its icon ID. Duct, Pipe and Wall start their tools. The other commands report in the status bar that they are not implemented yet.
- **Light and Dark themes.** Line-drawing icons switch to their dark variant.
- **Compact Properties and Project Browser palettes** with a resizable split, selection-specific grouped parameters, searchable view hierarchy and double-click view opening. The selected duct has shared size, elevation and slope controls in Properties and the contextual options bar, with Apply, Reset and Undo. See `docs/palettes-contextual-editing.md`. The sample plan supports wheel zoom, middle-drag pan and click selection.

## Not yet

- The 3D view.
- Real model geometry and picking.
- Docking for the panels: they are fixed side panels today.
- Redo.
- Full Revit shortcut matching through `openbim-keymap::Matcher`.
- French labels.
- Persistence and IFC.

`crates/document` is still a separate transaction model and must be merged with `crates/oplog` (issue #24).

## Verified on 15 Oct 2026

- `cargo build -p openbim-desktop` builds with no warnings.
- The app was run on Linux in a virtual display. Screenshots were checked for the Systems tab (HVAC and Electrical), Modify, and Annotate in Dark mode.
- It has not yet been run on macOS or Windows.
