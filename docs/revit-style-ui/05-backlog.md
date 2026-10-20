# 05 — Implementation backlog (proposed)

Statuses below are **not started as UI integration** unless indicated. Existing engine features may already cover some behavior.

| ID | Priority | Task | Dependency | Acceptance |
|---|---|---|---|---|
| BIM-001 | P0 | Unified typed document model and IDs | existing crates | Single document can edit walls, families and MEP networks |
| BIM-002 | P0 | Command registry / previews / undo | BIM-001, oplog | Same command callable by UI and API |
| BIM-003 | P0 | Dependency graph and incremental regeneration | BIM-001 | Edit affects only dependency closure; benchmark |
| BIM-004 | P0 | Native project persistence, recovery | BIM-001/002 | Save/open/crash recovery exact committed state |
| IFC-001 | P0 | IFC import, preservation and round-trip | BIM-001 | Supported classes round-trip without identity loss |
| GUI-001 | P0 | egui desktop shell (decided 15 Oct 2026; prototype on `feature/rust-gui`) | BIM-002 | Ribbon with icons, Place Duct end-to-end, measured latency |
| GUI-002 | P0 | Ribbon, context bar and command search | BIM-002, GUI-001 | PDF Systems ribbon and contextual actions implemented |
| GUI-003 | P0 | Project Browser, Properties, dock layouts | BIM-001/002, GUI-001 | Type/instance edits; multi-select; persisted layout |
| GUI-004 | P0 | 2D/3D viewport, tabs, selection, snapping | render, BIM-002 | Plan/3D selection, modify and undo |
| GUI-005 | P0 | Revit-style shortcuts and mouse defaults | keymap, GUI-002/004 | Shortcut import, conflicts, cancel, cycle and overrides |
| COLLAB-001 | P0 | Shared-folder sync without a server: segment file per writer, heartbeats, snapshot writer | BIM-002/004 | Two writers on a server share and on a cloud-synced folder; conflict and recovery test |
| COLLAB-002 | P1 | Element borrowing (required in central mode), locks with change requests, sync UI | COLLAB-001, GUI | Real sync and ownership state; no false synced indicators |
| COLLAB-003 | P1 | Discipline models linked into the central file; clash detection there and in standalone | COLLAB-001 | Linked models read only; clash report from both modes |
| FAMILY-001 | P0 | Parametric family graphical editor (in the first interface) | BIM, sketch/family, GUI | Flex, nested type, host, preview and undo |
| ARCH-001 | P1 | Full architectural authoring workflows | FAMILY-001, GUI | Walls, openings, stairs/roofs etc by staged tests |
| STR-001 | P1 | Structural framing and foundations | FAMILY-001 | Structural classifications, constraints, sections |
| MEP-001 | P1 | Connector/system browser and route editing UI | route, GUI | Connectivity reflected in model and UI |
| MEP-002 | P1 | Hydraulic/airflow calculation modules (electrical calculations imported from edesign-alike, see 06) | MEP-001 | Verified reference calculations with units |
| SCH-001 | P1 | Schematic zones and the extraction query (zone entities, plan assignment, JSON dump of the draft) | MEP-001, GUI-004, sketch | Zones persist with the model; same-level assignment deterministic (edges inclusive, first by zone id); overlaps, ambiguous, unzoned and unplaced end points reported; room-name fallback label |
| SCH-002 | P1 | Grid layout engine (uniform and footprint-proportional spacing, designer-selectable) | MEP-001, SCH-001 | Golden tests from the fixture: numeric values at 1e-9, orders and ids exact, expected results independently derived; largest-to-smallest total order with barycenter and stable-id tie-breaks; both spacing modes; fixtures cover multi-room label fallback, barycenter ties, disconnected and equal-key elements, and equivalent unit representations |
| SCH-003 | P1 | Single-line schematic view, native IEC 60617-style symbols, presentation annotations, `MEP: Generate Schematic` command | SCH-002, DOC-001, GUI-004 | Repeatable output; the 06 mandatory annotation matrix shown with fixture/reference labels (no live calculation claimed); undo/redo; sheet export |
| SCH-004 | P2 | Electrical reuse: pin openbim_electrical_model + openbim_sld from edesign-alike by SHA; DiagramView generator | SCH-002 | Builds against the pinned SHA; generated view passes validate_view; no electrical calculation reimplemented; the generator is integrated by SCH-006 |
| SCH-005 | P2 | Boundary study: push panels/connectivity/loads revit-alike → edesign-alike; import and arrange panels there; return calculated A/V-drop | SCH-001 | Study doc with agreed exchange format; single electrical development |
| SCH-006 | P2 | Live values: MEP-002 hydraulic/airflow wiring, imported edesign-alike electrical results, adopted standard editions | MEP-002, SCH-003, SCH-004, SCH-005 | Every annotation value traceable to a calculation module or an import, with units and the adopted standard edition recorded; matrix conformance with real values; editions resolved before this gate |
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
2. **Usable desktop** — GUI-001–005 and FAMILY-001, with reference Place Duct and wall workflows.
3. **Central collaboration** — COLLAB-001/002/003.
4. **Discipline completeness** — ARCH, STR, MEP, DOC.
5. **Automation** — AUTO-001 to AUTO-007, see `04-visual-automation.md` (AUTO-001 starts during Foundation, with BIM-002).
6. **Schematic automation** — SCH-001–006, see `06-mep-schematic-automation.md`. SCH-001–003 are data, layout and presentation (fixture labels), after MEP-001/GUI-004; SCH-004–006 add the pinned electrical reuse, the exchange study and the live calculated or imported values, so matrix conformance with real values is SCH-006's acceptance (gated on MEP-002, SCH-003, SCH-004 and SCH-005).

## Review checklist for every PR
- Public API/schema and migrations described.
- Unit/geometry/IFC semantics tests where applicable.
- Undo and concurrent-edit handling.
- Keyboard and mouse accessibility.
- Benchmark on realistically sized models if touching hot paths.
- Screenshots or reproducible interaction tests for UI changes.
- Code is platform-tested on target desktop OSes.
