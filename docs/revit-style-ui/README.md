# Revit-style UI / UX design

Status: **design specification, not implemented application code**.
Source: user-provided *Revit-Style App UI.pdf* (one-page Systems / Place Duct concept), plus October 2026 repository review.

## Goals
- Familiar Revit-style **ribbon + contextual Modify bar + Properties + Project Browser + tabbed drawing viewport + status bar**.
- Linux and Windows desktop clients, primarily Rust-backed.
- Same command semantics in standalone and multi-user central-model modes.
- IFC-native interoperable BIM workflows, with an application-owned parametric model.
- Design, Coordination and Automation workspaces.
- Close keyboard and mouse parity; provide customizable presets, not copies of Autodesk assets.

## Documents
- [01 — UI specification](01-ui-specification.md): screen regions and interactions derived from supplied reference.
- [02 — Interaction and workflow acceptance criteria](02-workflows-and-acceptance.md): Architecture, Structure, MEP, collaboration and automation.
- [03 — Implementation architecture](03-implementation-architecture.md): Rust command boundary, GUI toolkit evaluation, state and multi-user concerns.
- [04 — Dynamo-like automation study](04-visual-automation.md): visual graph and Python integration concept.
- [05 — Prioritized implementation backlog](05-backlog.md): dependencies, phases and verifiable exit criteria.

## Evidence versus proposals
**Observed in supplied PDF:** Systems ribbon tools (Mechanical Equipment, Air Terminal, Duct, Duct Fitting, Flex Duct, Pipe, Pipe Fitting, Plumbing Fixture, Sprinkler, Cable Tray, Lighting Fixture, Device, Component, Model Line); contextual **Modify | Place Duct** bar (Width, Height, Middle Elevation, Inherit Elevation, Inherit Size, Apply); docked Properties and Project Browser; Level 1 / 3D / M-101 tabs; plan with AHU, terminals, grids, rooms; ViewCube, drawing scale and Mechanical workset status.

**Design proposals (not present in PDF):** detachable/split views, search and parameter favorites, live collaborative presence, Design/Coordination/Automation workspaces, command palette, Python console, node editor, IFC round-trip workflows.

## Decision log (open)
1. Select GUI toolkit after an executable Qt / Slint / egui prototype using shared Rust model API.
2. Define native project storage and IFC import/export policy.
3. Confirm exact Revit keyboard shortcut mapping from lawful user-provided export.
4. Define the shared document/transaction API before developing scripting or production UI.
5. Set product branding and avoid distributing proprietary Autodesk icons/other UI assets.

This folder intentionally contains specifications only. Add runnable UI prototype code in a separate application crate once the GUI technology decision is made.
