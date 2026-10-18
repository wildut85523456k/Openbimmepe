# PRT-006: shared export command groundwork

## Implemented in this change

`openbim_sheets::export::ExportCommand` is a typed, side-effect-free entry point for PDF and DXF. It accepts an ordered sheet selection and an explicit validated print profile, then returns bytes, suggested basenames, page-to-sheet mappings and warnings. It uses the existing renderer and encoders, preserving title-block geometry, profile pens, mode and sheet scale.

Preflight rejects empty, duplicate and unknown selections, and missing viewport content adapters. An explicitly connected empty view remains valid. No partial file bundle is returned when validation or rendering fails. PDF returns one multipage file; DXF returns one file per selected sheet with portable ordinal basenames. Caller code owns destination selection, overwrite behavior and filesystem writes.

Regression tests cover request order, real PDF page dimensions, DXF extents, invalid requests, disconnected content, warnings and source immutability. Tests are added to the existing sheets suite.

## Verification status

Source-reviewed against the existing renderer, profile API and PDF/DXF encoders. Local compilation, tests and Clippy were NOT run: this execution environment has no Rust toolchain and cannot authenticate a Git clone. GitHub Actions is the validation gate; do not infer success from this document. No dependency or lockfile changes are required.

## PRT-006 remains incomplete

This is shared-command groundwork only. No desktop Print/Export dialog, profile selector, live document-to-plan adapter, file persistence or OS printer integration is included.

The inspected printing baseline is `fix/prt-002-profile-data` at `e184d59892b113bdc01e82fe0e11901f333bb90c`; the desktop baseline is `feature/rust-gui` at `499d48c5eba29394d4ab3697e6e04bb9cf65f7d7`. The printing baseline has no desktop app, and the desktop baseline has no sheets crate. An integrated branch and regenerated lockfile need validation before wiring desktop UI.

Next implementation: combine those baselines without dropping either branch's changes; adapt visible plan geometry from the document with explicit level/category handling; add Print/Export settings and destination UI; invoke this command for both formats; show warnings and write errors; test edits, undo, selected level, scale and both orientations through real exports.

Company PRT-002 acceptance and PRT-005 layer mapping remain blocked by original PC3/media definitions, approved margins/pen tables and company title block/layer rules. The synthetic profile fixture is not company evidence. PR #45 remains documentation-only. Existing PRs #48/#49 are not merged by this change.
