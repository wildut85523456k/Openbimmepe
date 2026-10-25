# Initial feature-gap review

Scope: all entries in the project implementation backlog, not all Autodesk Revit features. This is a repository-backed first-pass inventory, not a UX certification. No feature implementation has been approved.

Baseline: `main`, commit `2c4e63bd681f55b8a6e7a11e088ce08a925daf37`. Existing release workspace tests and strict workspace Clippy both exited 0 on this Windows PC. Those results apply to main, not to unbuilt feature branches.

## Important findings

- Do not start a second GUI: `origin/feature/rust-gui` already has a desktop prototype, document code and automation/MCP work.
- Do not call all documentation features absent: `origin/m1c-sheets-printing` already has a sheets crate.
- The checked-in architecture chooses Rust/egui with wgpu and drops Qt; old Qt issues and prototype branches are historical evidence, not the implementation direction.
- Rust export exists despite the still-open issue titled “IFC is only written by the Python check scripts”. Issue closure and current implementation must be reconciled.
- Passing engine tests does not demonstrate Revit-like command state, interaction quality, recovery or multicore scaling.

## Status inventory

A status is an initial evidence assessment. PLANNED/UNVERIFIED means acceptance has not been demonstrated; it is not proof no experimental code exists anywhere.

| ID | Priority | Feature | Initial assessment |
|---|---|---|---|
| BIM-001 | P0 | Unified typed document model and IDs | PARTIAL / BRANCH: main has domain engines and oplog; provisional document code is on GUI/foundation branches. Typed model and oplog convergence still needs review (#24). |
| BIM-002 | P0 | Command registry / previews / undo | PARTIAL: oplog provides transaction primitives. A unified UI/API command registry is still a documented target (#37); branch automation exists but does not prove registry completion. |
| BIM-003 | P0 | Dependency graph and incremental regeneration | OPEN GAP: incremental dependency-closure regeneration is required; #14 reports whole-model rebuilds. The issue is evidence of a reported gap, not a fresh benchmark. |
| BIM-004 | P0 | Native project persistence, recovery | PARTIAL / UNVERIFIED: oplog and document primitives do not establish native save/open, schema migration and crash recovery acceptance. |
| IFC-001 | P0 | IFC import, preservation and round-trip | PARTIAL: main contains a native Rust IFC writer and independent checker scripts. Import/preservation/round-trip completeness is not established by exporter tests. |
| GUI-001 | P0 | egui desktop shell (decided 15 Oct 2026; prototype on `feature/rust-gui`) | PROTOTYPE ON BRANCH: origin/feature/rust-gui contains apps/desktop and an expanded workspace; main does not include the desktop app. No GUI build or interaction run performed in this review. **[superseded 2026-10-21 — R5 register: main now includes apps/desktop as a workspace member (Cargo.toml members list, 40 tracked files at b6ef0ee), so "main does not include the desktop app" is false]** |
| GUI-002 | P0 | Ribbon, context bar and command search | PROTOTYPE / UNVERIFIED: GUI branch includes ribbon.rs and icons.rs; contextual command completeness and latency remain to be tested. |
| GUI-003 | P0 | Project Browser, Properties, dock layouts | PROTOTYPE / UNVERIFIED: GUI branch includes panels.rs. Type/instance/multiselect/docking/layout persistence need behavioral verification. |
| GUI-004 | P0 | 2D/3D viewport, tabs, selection, snapping | PROTOTYPE / UNVERIFIED: GUI branch includes model_view.rs and model_edit.rs. Picking, snapping, cross-view selection and undo need real interaction tests. |
| GUI-005 | P0 | Revit-style shortcuts and mouse defaults | PARTIAL: main has keymap presets and tests; full UI integration, Tab/Escape/Space/override behavior still needs verification. #15 must be reconciled with current code. |
| COLLAB-001 | P0 | Shared-folder sync without a server: segment file per writer, heartbeats, snapshot writer | PARTIAL PRIMITIVES / UNVERIFIED: oplog exists; shared-folder segment protocol, two-writer sync and cloud-folder recovery acceptance have not been demonstrated. |
| COLLAB-002 | P1 | Element borrowing (required in central mode), locks with change requests, sync UI | PLANNED / UNVERIFIED: borrowing, ownership locks, change requests and truthful sync UI require end-to-end evidence. |
| COLLAB-003 | P1 | Discipline models linked into the central file; clash detection there and in standalone | PLANNED / UNVERIFIED: linked read-only discipline models and central/standalone clash-report acceptance are not demonstrated. |
| FAMILY-001 | P0 | Parametric family graphical editor (in the first interface) | PARTIAL: main has family/sketch engines; a graphical editor with host/flex/nesting/undo is not established by engine tests. |
| ARCH-001 | P1 | Full architectural authoring workflows | PARTIAL: main has walls, placement, rooms and arch primitives; complete architectural authoring workflows remain staged targets. |
| STR-001 | P1 | Structural framing and foundations | PLANNED / UNVERIFIED: structural framing/foundations workflow is a declared backlog target; no complete UI workflow was exercised. |
| MEP-001 | P1 | Connector/system browser and route editing UI | PARTIAL: route, fitting/connectivity primitives and branch desktop edits exist; complete system browser and route editing acceptance remains unverified. |
| MEP-002 | P1 | Hydraulic/airflow/electrical calculation modules | PLANNED / UNVERIFIED: hydraulic, airflow and electrical calculation modules require units-aware reference validations; no engineering calculation compliance is claimed. |
| DOC-001 | P1 | Annotations, sheets, schedules, print/export | BRANCH WORK EXISTS: origin/m1c-sheets-printing workspace includes crates/sheets. Annotation/sheets/schedules/printing completeness must be reviewed there; do not duplicate it based on main alone. |
| AUTO-001 | P0 | Typed command and query API: quantities with units, stable parameter IDs (EN/FR labels) | PARTIAL / BRANCH: GUI branch includes crates/automation with protocol/model code. Unified stable typed command API remains a documented target; do not infer completion from filenames. |
| AUTO-002 | P1 | Per-element commit rules: element revisions, last writer wins with flag, locks give change requests, all-or-nothing marks | OPEN DESIGN GAP: #33/#34 track bulk-edit and per-element conflict semantics. Candidate implementation must prove the 499-success/1-conflict case. |
| AUTO-003 | P1 | MCP server in the app (stdio and local HTTP with token), read-only then accepted proposals | PROTOTYPE / BRANCH: GUI branch includes openbim-mcp-server.rs and stdio/protocol tests. Permission scope, accepted proposals and undo still require current behavior checks. |
| AUTO-004 | P1 | Headless graph engine `openbim-graph`: Dynamo lacing, parallel, cache, versioned graph file | PARTIAL / UNVERIFIED: automation engine code exists on GUI branch, but deterministic graph runs, Dynamo lacing and parallel cache acceptance require separate evidence (#38). |
| AUTO-005 | P1 | OpenBIM Graph app (egui, Dynamo-like): library, canvas, Run bar, Watch, 3D preview, Player | PROTOTYPE / BRANCH: GUI branch includes automation_canvas.rs/groups.rs/slider.rs. Separate-app architecture, Player and firestop end-to-end workflow are not proven. |
| AUTO-006 | P1 | Python Script node in OpenBIM Graph: worker processes, declared permissions, code warning | PROTOTYPE / BRANCH: worker.py, worker.rs and Python tests exist. Permission denial and shared-graph warning acceptance have not been independently exercised. |
| AUTO-007 | P2 | Discipline node libraries calling the calculation modules | PLANNED / UNVERIFIED: discipline libraries must call validated calculation modules and preserve standard/edition metadata (#41). |
| PERF-001 | P0 | Incremental edit benchmark, issue #14 | OPEN PERFORMANCE GATE: existing benchmark examples are available, but no measured incremental-edit latency/scaling report has been produced in this review. |
| PERF-002 | P1 | Spatial index for auto-router, issue #21 | OPEN PERFORMANCE GATE: issue #21 reports router scaling. Inspect branch spatial index work and measure identical workloads before recommending replacements. |

## Recommended review order, before selecting code work

1. Reconcile the main, Rust GUI, foundation, sheets and MEP branches. Compare actual code, not only open issues; identify stale issue descriptions and conflicting architecture.
2. Review document identity/units/oplog integration, transaction/undo, persistence/recovery and incremental dependencies. These underpin every familiar interaction.
3. Build and exercise the Rust GUI branch. Inventory ribbon/context, Properties, Browser, viewport, shortcuts, snapping and save/reopen with recorded interactions.
4. Audit family/architecture/structure/MEP and documentation end to end against declared workflows. Separate absent controls from controls wired only to placeholders.
5. Run independent IFC semantics, collaboration/recovery, automation permissions and deterministic multicore checks.
6. Agree one bounded first task after the gap review; no coding scope is assumed.

## Unknown or outside current first-pass scope

Reference Revit version/edition/language and licensed direct comparison access remain unspecified. Complete native RVT authoring and Dynamo .dyn compatibility are explicitly non-goals for the first UI milestone in the current specification. Additional Revit feature families, unsupported formats and specialty workflows need a version-specific requirements catalogue before anyone can call the review exhaustive.

No GUI interaction, runtime speedup, clean-build speedup, Python IFC checker, formatting or dependency-audit PASS is claimed by this first-pass review.

## Evidence locations

- `Cargo.toml`, `README.md`, `.github/workflows/ci.yml` at the baseline SHA.
- `docs/revit-style-ui/01-ui-specification.md`, `02-workflows-and-acceptance.md`, `03-implementation-architecture.md`, `05-backlog.md`.
- `git branch -r`, `git show origin/feature/rust-gui:Cargo.toml`, recursive tree of its `apps`/`crates`, and `git show origin/m1c-sheets-printing:Cargo.toml`.
- GitHub open issue titles retrieved with `gh issue list`; these are reported issues and can be stale, not independent confirmation that each defect persists.
- Agent roles: `docs/agent-team/roles/`; task handoffs: `task-template.md`.
