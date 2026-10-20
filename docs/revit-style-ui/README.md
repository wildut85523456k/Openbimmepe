# Revit-style UI / UX design

Status: **design specification, not implemented application code**.
Source: user-provided *Revit-Style App UI.pdf* (one-page Systems / Place Duct concept), plus October 2026 repository review.

## Goals
- Familiar Revit-style **ribbon + contextual Modify bar + Properties + Project Browser + tabbed drawing viewport + status bar**.
- Linux and Windows desktop clients, written entirely in Rust with egui (decided 15 Oct 2026). Python runs only in the external automation app, OpenBIM Graph.
- Same command semantics in standalone and multi-user central-model modes.
- IFC-native interoperable BIM workflows, with an application-owned parametric model: the project is stored in its own format (operation log and snapshots), and IFC4 is the exchange format, read and written in Rust with stable GlobalIds.
- Design, Coordination and Automation workspaces.
- Close keyboard and mouse parity; provide customizable presets, not copies of Autodesk assets.

## Documents
- [01 — UI specification](01-ui-specification.md): screen regions and interactions derived from supplied reference.
- [02 — Interaction and workflow acceptance criteria](02-workflows-and-acceptance.md): Architecture, Structure, MEP, collaboration and automation.
- [03 — Implementation architecture](03-implementation-architecture.md): Rust command boundary, GUI toolkit evaluation, state and multi-user concerns.
- [04 — Automation](04-visual-automation.md): Dynamo-like graph app (OpenBIM Graph) and MCP server for AI automation runs.
- [04a — Dynamo review](04a-dynamo-review.md): historical/version-scoped feature evidence and proposed refinements; not proof of complete compatibility.
- [04b — Replication and serialization contract](04b-replication-and-serialization.md): proposed normative supported subset, exact fixtures, unresolved implementation gates, cache, partial commits and trusted-code boundary.
- [04c — Dynamo engine port](04c-dynamo-engine-port.md): feasibility study and port plan for re-implementing Dynamo's Code Block/lacing engine semantics in Rust (Apache-2.0, pinned source `8a211c5`).
- [04d — UI parity checklist](04d-ui-parity.md): feature-by-feature UI parity of OpenBIM Graph with Dynamo for Revit: mapping, adopted additions, acceptance list, exclusions and deliberate differences.
- [05 — Prioritized implementation backlog](05-backlog.md): dependencies, phases and verifiable exit criteria.
- [06 — MEP schematic automation](06-mep-schematic-automation.md): single-line schematic generation from the design model — zones, grid layout, per-discipline annotations (proposed 20 Oct 2026).

## Evidence versus proposals
**Observed in supplied PDF:** Systems ribbon tools (Mechanical Equipment, Air Terminal, Duct, Duct Fitting, Flex Duct, Pipe, Pipe Fitting, Plumbing Fixture, Sprinkler, Cable Tray, Lighting Fixture, Device, Component, Model Line); contextual **Modify | Place Duct** bar (Width, Height, Middle Elevation, Inherit Elevation, Inherit Size, Apply); docked Properties and Project Browser; Level 1 / 3D / M-101 tabs; plan with AHU, terminals, grids, rooms; ViewCube, drawing scale and Mechanical workset status.

**Design proposals (not present in PDF):** detachable/split views, search and parameter favorites, live collaborative presence, Design/Coordination/Automation workspaces, command palette, OpenBIM Graph (separate egui program, as close to Dynamo as possible), MCP server, IFC round-trip workflows.

## Decision log
Settled (the design record is the Claude Doc "Open BIM Platform: Design and Decisions"):
1. **GUI toolkit: egui, all in Rust** (15 Oct 2026, issue #22). Qt 6, Slint and the web shell are dropped.
2. **Storage and IFC:** own project format (operation log and snapshots); IFC4 for exchange, read and written in Rust inside the app; element IDs and GlobalIds stable (13 and 15 Oct 2026).
3. **Shortcuts:** Revit's standard shortcuts as defaults, checked against Autodesk's published list, all customizable in settings (14–15 Oct 2026, #15).
4. **Collaboration:** no server. Discipline models (mechanical, plumbing, electrical) on a shared folder, a server share or a cloud-synced folder, each linked to the central file where clashes are found. Element borrowing is required; locked elements turn edits into change requests (15 Oct 2026).
5. **Automation:** one Rust command API for the ribbon, OpenBIM Graph and an MCP server; Python only in OpenBIM Graph (16 Oct 2026, see 04).
6. **Branding:** no Autodesk icons or assets; own ribbon icons, with non-trademark IFC import/export icons.

Still open: define the shared document and transaction API (BIM-001/002, AUTO-001) before production UI and automation work.

The egui application lives in `apps/desktop` (branch `feature/rust-gui` until merged).
