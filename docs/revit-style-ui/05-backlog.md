# 05 — Implementation backlog (proposed)

Statuses below are **not started as UI integration** unless indicated. Existing engine features may already cover some behavior.

| ID | Priority | Task | Dependency | Acceptance |
|---|---|---|---|---|
| BIM-001 | P0 | Unified typed document model and IDs | existing crates | Single document can edit walls, families and MEP networks |
| BIM-002 | P0 | Command registry / previews / undo | BIM-001, oplog | Same command callable by UI and API |
| BIM-003 | P0 | Dependency graph and incremental regeneration | BIM-001 | Edit affects only dependency closure; benchmark |
| BIM-004 | P0 | Native project persistence, recovery | BIM-001/002 | Save/open/crash recovery exact committed state |
| IFC-001 | P0 | IFC import, preservation and round-trip | BIM-001 | Supported classes round-trip without identity loss |
| GUI-001 | P0 | Qt/Slint/egui comparative UI spike | BIM-002 | Documented benchmark/choice; Place Duct end-to-end |
| GUI-002 | P0 | Ribbon, context bar and command search | BIM-002, GUI-001 | PDF Systems ribbon and contextual actions implemented |
| GUI-003 | P0 | Project Browser, Properties, dock layouts | BIM-001/002, GUI-001 | Type/instance edits; multi-select; persisted layout |
| GUI-004 | P0 | 2D/3D viewport, tabs, selection, snapping | render, BIM-002 | Plan/3D selection, modify and undo |
| GUI-005 | P0 | Revit-style shortcuts and mouse defaults | keymap, GUI-002/004 | Shortcut import, conflicts, cancel, cycle and overrides |
| COLLAB-001 | P0 | Server-authoritative central commits | BIM-002/004 | Two concurrent clients, conflict and recovery test |
| COLLAB-002 | P1 | Workset/ownership and sync UI | COLLAB-001, GUI | Real sync and ownership state; no false synced indicators |
| FAMILY-001 | P1 | Parametric family graphical editor | BIM, sketch/family, GUI | Flex, nested type, host, preview and undo |
| ARCH-001 | P1 | Full architectural authoring workflows | FAMILY-001, GUI | Walls, openings, stairs/roofs etc by staged tests |
| STR-001 | P1 | Structural framing and foundations | FAMILY-001 | Structural classifications, constraints, sections |
| MEP-001 | P1 | Connector/system browser and route editing UI | route, GUI | Connectivity reflected in model and UI |
| MEP-002 | P1 | Hydraulic/airflow/electrical calculation modules | MEP-001 | Verified reference calculations with units |
| DOC-001 | P1 | Annotations, sheets, schedules, print/export | BIM, render, GUI | Repeatable view/sheet output |
| AUTO-001 | P0 | Typed command and query API: quantities with units, stable parameter IDs (EN/FR labels) | BIM-002 | Ribbon uses it; unit-mismatch and label-lookup tests |
| AUTO-002 | P1 | Per-element commit rules: element revisions, last writer wins with flag, locks give change requests, all-or-nothing marks | oplog, COLLAB-002 | Bulk edit of 500 elements with 1 clash applies 499 and reports 1 |
| AUTO-003 | P1 | MCP server in the app (stdio and local HTTP with token), read-only then accepted proposals | AUTO-001, AUTO-002 | Firestop CSV via an MCP client: preview, accept, undo; changes logged under the client |
| AUTO-004 | P1 | Headless graph engine `openbim-graph`: Dynamo lacing, parallel, cache, versioned graph file | AUTO-001 | Same graph run twice gives identical change sets; lacing tests |
| AUTO-005 | P1 | OpenBIM Graph app (egui, Dynamo-like): library, canvas, Run bar, Watch, 3D preview, Player | AUTO-004, GUI-002 | Firestop graph end to end; Player runs it from the app |
| AUTO-006 | P1 | Python Script node in OpenBIM Graph: worker processes, declared permissions, code warning | AUTO-005 | Denied permission refused by the app; shared graph with code warns |
| AUTO-007 | P2 | Discipline node libraries calling the calculation modules | AUTO-005, MEP-002 | No calculation reimplemented in nodes; standard and edition recorded |
| PERF-001 | P0 | Incremental edit benchmark, issue #14 | BIM-003 | Demonstrate local rather than global rebuild |
| PERF-002 | P1 | Spatial index for auto-router, issue #21 | route | Query performance scaling benchmark |

## Phase gating
1. **Foundation** — BIM-001/002/003/004 and IFC-001.
2. **Usable desktop** — GUI-001–005 with reference Place Duct and wall workflows.
3. **Central collaboration** — COLLAB-001/002.
4. **Discipline completeness** — FAMILY, ARCH, STR, MEP, DOC.
5. **Automation** — AUTO-001 to AUTO-007, see `04-visual-automation.md` (AUTO-001 starts during Foundation, with BIM-002).

## Review checklist for every PR
- Public API/schema and migrations described.
- Unit/geometry/IFC semantics tests where applicable.
- Undo and concurrent-edit handling.
- Keyboard and mouse accessibility.
- Benchmark on realistically sized models if touching hot paths.
- Screenshots or reproducible interaction tests for UI changes.
- Code is platform-tested on target desktop OSes.
