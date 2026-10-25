# 05 — Implementation backlog (proposed)

Statuses below are **not started as UI integration** unless indicated. Existing engine features may already cover some behavior.

| ID | Priority | Task | Dependency | Acceptance |
|---|---|---|---|---|
| BIM-001 | P0 | Unified typed document model and IDs | existing crates | Single document can edit walls, families and MEP networks |
| BIM-002 | P0 | Command registry / previews / undo | BIM-001, oplog | Same command callable by UI and API |
| BIM-003 | P0 | Dependency graph and incremental regeneration | BIM-001 | Edit affects only dependency closure; benchmark |
| BIM-005 | P0 | Level authoring: validate name and spacing at placement, rename and delete levels, derive tabs and browser from the document | BIM-001 | Duplicate names and levels closer than 100 mm are refused; rename and delete are undoable; a wall in a new project never references a missing level |
| BIM-004 | P0 | Native project persistence, recovery | BIM-001/002 | Save/open/crash recovery exact committed state; autosave and crash recovery are the first consumers of `crates/oplog` (decision 21 Oct 2026) |
| IFC-002 | P1 | IFC honesty: export the run system and the user's named systems with their equipment, rooms as `IfcSpace` with boundaries, grids as `IfcGrid`, georeference, warn on every dropped category, derive storey/system/relationship/property-set GlobalIds from stable ids, feed penetrations, persist the compute tables | IFC-001, MEP-001 | A straight "Supply Air" duct exports as that system; every skipped category is reported; adding a level does not change unrelated GlobalIds; a duct through a wall cuts an opening in 3D and IFC; compute tables survive save/reopen |
| IFC-001 | P0 | IFC import, preservation and round-trip | BIM-001 | Supported classes round-trip without identity loss |
| GUI-001 | P0 | egui desktop shell (decided 15 Oct 2026; prototype on `feature/rust-gui`) | BIM-002 | Ribbon with icons, Place Duct end-to-end, measured latency |
| GUI-002 | P0 | Ribbon, context bar and command search | BIM-002, GUI-001 | PDF Systems ribbon and contextual actions implemented |
| GUI-003 | P0 | Project Browser, Properties, dock layouts | BIM-001/002, GUI-001 | Type/instance edits; multi-select; persisted layout |
| GUI-004 | P0 | 2D/3D viewport, tabs, selection, snapping | render, BIM-002 | Plan/3D selection, modify and undo; no click is lost on an empty canvas and the plan tabs come from the document's levels |
| GUI-006 | P0 | From-scratch journey: default view box on an empty canvas, level-derived plan tabs, per-tool parameter state, two-stage Esc, *New* creates Level 1 and Level 2 | GUI-004, BIM-005 | From `Desktop::ui`: a new project builds levels, grids, typed walls, floor, door/window, stair, roof, ceiling, room and section, saves/reopens and exports IFC with element counts; no tool click is lost on an empty canvas; Esc steps back once before leaving the tool |
| GUI-007 | P0 | Ribbon honesty: unbuilt tools render disabled with a planned-state tooltip, resolved from the command registry (const table as fallback, plus a drift test) | GUI-002 | Ribbon honesty test: every ribbon id is handled or disabled; no "not implemented" path remains |
| GUI-005 | P0 | Revit-style shortcuts and mouse defaults | keymap, GUI-002/004 | Shortcut import, conflicts, cancel, cycle and overrides; the keymap crate is consulted for real key events (Ctrl+S/N/O and the two-letter shortcuts) |
| COLLAB-001 | P0 | Shared-folder sync without a server: segment file per writer, heartbeats, snapshot writer | BIM-002/004 | Two writers on a server share and on a cloud-synced folder; conflict and recovery test |
| COLLAB-002 | P1 | Element borrowing (required in central mode), locks with change requests, sync UI | COLLAB-001, GUI | Real sync and ownership state; no false synced indicators |
| COLLAB-003 | P1 | Discipline models linked into the central file; clash detection there and in standalone | COLLAB-001 | Linked models read only; clash report from both modes |
| FAMILY-001 | P0 | Parametric family graphical editor (in the first interface) | BIM, sketch/family, GUI | Flex, nested type, host, preview and undo |
| ARCH-001 | P1 | Full architectural authoring workflows | FAMILY-001, GUI | Walls, openings, stairs/roofs etc by staged tests; the from-scratch journey is gated by GUI-006 and TEST-001 |
| STR-001 | P1 | Structural framing and foundations | FAMILY-001 | Structural classifications, constraints, sections |
| MEP-001 | P1 | Connector/system browser and route editing UI | route, GUI | Connectivity reflected in model and UI |
| MEP-003 | P1 | MEP family library: AHU, ceiling diffuser, VAV, pump, valve, WC/basin, sprinkler head, luminaire, panel board, hanger/support, each with connectors, host rules and an IFC class; Load Family UI and editable type parameters | FAMILY-001, GUI-002 | Each family places, hosts correctly, carries editable type parameters, exports its IFC class and appears in a schedule |
| MEP-005 | P1 | Electrical calculations from edesign-alike: pin openbim_electrical_model and openbim_electrical_solver by SHA, prove equivalence with the in-repo IEC 60364 module against captured golden results, then retire `crates/calc/src/electrical.rs` and rewire the Compute electrical UI | MEP-001 | Identical results at 1e-9 on the golden fixture set, or every difference documented and approved; the pinned crates build in this workspace; no electrical calculation remains duplicated in this repository |
| MEP-002 | P1 | Hydraulic and airflow calculation modules (the electrical calculations are imported from edesign-alike by MEP-005, see 06) | MEP-001 | Verified reference calculations with units |
| SCH-001 | P1 | Schematic zones and the extraction query (zone entities, plan assignment, JSON dump of the draft) | MEP-001, GUI-004, sketch | Zones persist with the model; same-level assignment deterministic (edges inclusive, first by zone id); overlaps, ambiguous, unzoned and unplaced end points reported; room-name fallback label |
| SCH-002 | P1 | Grid layout engine (uniform and footprint-proportional spacing, designer-selectable) | MEP-001, SCH-001 | Golden tests from the fixture: numeric values at 1e-9, orders and ids exact, expected results independently derived; largest-to-smallest total order with barycenter and stable-id tie-breaks; both spacing modes; fixtures cover multi-room label fallback, barycenter ties, disconnected and equal-key elements, and equivalent unit representations |
| SCH-003 | P1 | Single-line schematic view, native IEC 60617-style symbols, presentation annotations, `MEP: Generate Schematic` command | SCH-002, DOC-001, GUI-004 | Repeatable output; the 06 mandatory annotation matrix shown with fixture/reference labels (no live calculation claimed); undo/redo; sheet export |
| SCH-004 | P2 | Electrical reuse: the DiagramView generator over the crates MEP-005 pins (openbim_electrical_model, openbim_electrical_solver, openbim_sld); the pin itself is MEP-005's deliverable | MEP-005, SCH-002 | Builds against the pinned SHA; generated view passes validate_view; no electrical calculation reimplemented; the generator is integrated by SCH-006 |
| SCH-005 | P2 | Boundary study: push panels/connectivity/loads revit-alike → edesign-alike; import and arrange panels there; return calculated A/V-drop | SCH-001 | Study doc with agreed exchange format; single electrical development |
| SCH-006 | P2 | Live values: MEP-002 hydraulic/airflow wiring, imported edesign-alike electrical results, adopted standard editions | MEP-002, SCH-003, SCH-004, SCH-005 | Every annotation value traceable to a calculation module or an import, with units and the adopted standard edition recorded; matrix conformance with real values; editions resolved before this gate |
| DOC-002 | P1 | Documentation refresh: land the gap analysis (07) and the closure plan (08), refresh the stale 3D-view plan, `gap-review.md` and the README crate list | none | Documented behaviour matches the code; no document claims a feature that is absent, or omits one that exists |
| DOC-001 | P1 | Annotations, sheets, schedules, print/export | BIM, render, GUI | Repeatable view/sheet output |
| AUTO-001 | P0 | Typed command and query API: quantities with units, stable parameter IDs (EN/FR labels) | BIM-002 | Ribbon uses it; unit-mismatch and label-lookup tests. *2026-10-25:* label lookup refuses ambiguous labels (#75) and the ribbon reads labels from the registry for the registered commands with drift tests (#76); NOT yet met: ribbon generated from the registry for all commands, keymap, graph and MCP consumers |
| AUTO-002 | P1 | Per-element/dependency-group commits and atomic project ownership/undo (04b §3–4) | AUTO-001, oplog, COLLAB-002, BIM-004 | Independent 500-element case applies 499 and requests locked 1; dependent host failure/lock suppresses entire group and mapping; wrong-run/type handles rejected; partial apply, cancel, save/reopen, undo/rerun, stable-key reorder/shrink/detach/create-new tests in 04b pass |
| AUTO-003 | P1 | MCP server in the app (stdio and local HTTP with token), read-only then accepted proposals | AUTO-001, AUTO-002 | Firestop CSV via an MCP client: preview, accept, undo; changes logged under the client |
| AUTO-000 | P0 | Staged prototype convergence; never merge prototype's divergent 04 | AUTO-001, BIM-001; exit also requires AUTO-002/003/004/005/006 | A0 registry/ribbon parity; A1 live-model MCP and scoped token tests replace snapshot backend; A2 schema migration/typed units/replication/cache tests; A3 separate process/no desktop Python and moved canvas undo tests; A4 worker/trust/install tests; preserve preview/plan/apply/undo workflow tests adapted to dependency groups, not whole-document veto. Staging may begin at A0; completion requires all listed convergence gates |
| AUTO-004 | P1 | A2 headless engine, own schema v1 and Dynamo-evidence replication profile (04b); conservative generation-scoped cache | AUTO-001; proposal apply acceptance requires AUTO-002 | Repeat evaluation on identical model identity/snapshot/captured resources/settings gives equal canonical values/proposals excluding run-local IDs. Cache-on/full-run equality for empty collector insertion, deletion/filter/link changes, same-path CSV byte changes and library/API changes. Exact fixtures in 04b; unresolved combinations refuse run until sourced/reviewed, not silently approximated. Canonical round-trip/unknown schema/migration tests and cross-language numeric vectors required |
| AUTO-005 | P1 | A3 separate egui Graph app/canvas/library/run/preview; A5 Player subtask (not A3 exit) | AUTO-004, AUTO-002, GUI-002; A5 Python graphs also AUTO-006 | Firestop end-to-end; post-commit new-snapshot update_previous rerun is no-op with stable IDs, not identical creation proposal. Automatic never commits; A3 cancel leaves diagnostic-only previews; A5 typed stable-bound Player inputs, mandatory per-run proposal/destructive review, no persistent auto-apply; 04b managed-output cases pass |
| AUTO-006 | P1 | A4 trusted-local-code Python workers; model tokens not an OS sandbox; locked isolated wheel environment and separate install consent | AUTO-005 A3, AUTO-003 | Restricted/untrusted Python refused before imports; informed no-filesystem/network-confinement warning; denied installs/no open-side effects/hash mismatch tests; model scope denial; separate MCP source/edit/code/run/path gates; code/dependency/revision changes invalidate approvals (04b §5) |
| AUTO-007 | P2 | Discipline node libraries calling the calculation modules | AUTO-005, MEP-002 | No calculation reimplemented in nodes; standard and edition recorded |
| AUTO-008 | P0 | Phase 0 spike `obg-ds`: Code Block parser/evaluator and replication algorithm, port-subset of Dynamo ProtoCore @8a211c5 (04c); conformance fixtures | none (precedes A0) | cargo test green: all nine non-gated `replication-v1.json` cases (`auto-flat`, `shortest-flat`, `longest-flat`, `cross-flat`, `guides-zip`, `guides-cross-reversed`, `auto-scalar`, `level-flat`, `level-keep`) byte-equal, plus a pinned manifest of transcribed Dynamo `Replication.cs`/`AtLevelTest.cs` input→output cases (source method, expected output, comparison encoding recorded); gated cases stay refused |
| AUTO-009 | P0 | Apache-2.0 porting compliance: LICENSE.txt, THIRD_PARTY_NOTICES (Dynamo ABOUT.txt verbatim), PORTING.md, per-file source-SHA header convention, licence/notice delivery with redistributed artifacts (04c §8) | AUTO-008 | Files present; **every** ported file carries its source-SHA header and modification statement; reused fixtures cleared; no closed-source Autodesk code ported; packaged-release contents and notice display verified |
| PERF-001 | P0 | Incremental edit benchmark, issue #14 | BIM-003 | Demonstrate local rather than global rebuild |
| TEST-001 | P0 | End-to-end harness: `Desktop::window` without an `eframe::Frame`, the plan's model-to-screen transform for tests, and the office building fixture | GUI-004 | Pointer-driven tests drive the real ribbon, options bar and global keys; the office builds, saves, reopens and exports through the UI path; the data-hall fixture and its MEP chain are MEP-004 |
| MEP-004 | P3 | Data-hall MEP chain end to end on the data-hall fixture: AHU, terminals, a named system, Generate Layout, End Points, Compute, Apply sizes, and the IFC system and port asserts | MEP-003, MEP-001, TEST-001 | The chain runs from the placed equipment to the exported system name and ports; the fixture is generated by the test, not hand-modelled |
| PERF-002 | P1 | Spatial index for auto-router, issue #21 | route | Query performance scaling benchmark |
| PRT-001 | P1 | Portrait sheets: the title block keeps its orientation (today `PaperSize::custom` silently swaps to landscape) | DOC-001 | A 841 x 1189 title block gives a tall PDF page and DXF extents; no silent swap |
| PRT-002 | P1 | Company print profile as data: named papers, pen table per scale, margins, colour mode | DOC-001 | Team profile loads; the team's 10 AutoCAD media are mapped with a difference report |
| PRT-003 | P1 | Print modes colour / grayscale / black lines (Revit Print Setup) | DOC-001 | PDF strokes black or grey on request; halftone stays distinguishable |
| PRT-004 | P1 | DXF colours: nearest of the full ACI palette, or DXF 2000+ with true colour (420) and lineweight (370) | DOC-001 | Olive 122,122,0 no longer exports as red; ezdxf round-trip check |
| PRT-005 | P2 | Export layer mapping: category to company layer name, colour and pen, fallback to `{D}-{CATEGORY}_W{n}` | PRT-002 | DXF uses company layer names when a mapping is loaded |
| PRT-006 | P2 | Print / Export dialog in the desktop app (sheet selection, profile, mode, combined or separate files) | PRT-002, GUI-002, AUTO-001 | Same typed command from the UI and the API; UX interaction test |
| PRT-007 | P3 | Optional direct output to OS printers | PRT-006 | Only if direct plotting is needed; PDF output already covers the team's Adobe PDF use |
| DOC-PURGE | P1 | Purge Unused: scan, reference chain, opt-in per item, one undoable operation, shared typed command (`apply` needs a scan ID) | BIM-002, AUTO-001 | Exact unused list against an independent count; apply then undo gives an identical snapshot; blind apply refused |

## Phase gating
1. **Foundation** — BIM-001/002/003/004/005 and IFC-001/IFC-002.
2. **Usable desktop** — GUI-001–007 and FAMILY-001, with reference Place Duct and wall workflows.
2a. **Walking skeleton** — GUI-006/GUI-007 and BIM-005 gate the rest of *Usable desktop* and every later phase's tests: a from-scratch journey through the real UI is the only way those phases are exercised (see 08).
3. **Central collaboration** — COLLAB-001/002/003.
4. **Discipline completeness** — ARCH, STR, MEP, DOC (with PRT, DOC-PURGE and TEST-001; details in `docs/agent-team/printing-compatibility.md` and `docs/agent-team/purge-unused.md`).
5. **Automation** — AUTO-000 to AUTO-009, see `04-visual-automation.md` (AUTO-001 starts during Foundation, with BIM-002; AUTO-008 spike and AUTO-009 licence compliance precede A0).
6. **Schematic automation** — SCH-001–006, see `06-mep-schematic-automation.md`. SCH-001–003 are data, layout and presentation (fixture labels), after MEP-001/GUI-004; SCH-004–006 add the pinned electrical reuse, the exchange study and the live calculated or imported values, so matrix conformance with real values is SCH-006's acceptance (gated on MEP-002, SCH-003, SCH-004 and SCH-005).

### Proposed automation milestone mapping (17 Oct corrections)

| Milestone | Backlog owner and exit |
|---|---|
| Pre-A0 | AUTO-008 spike + AUTO-009 license compliance (20 Oct port plan, 04/04c) |
| A0 | AUTO-001 + AUTO-000 registry/document convergence stage |
| A1 | AUTO-003 on AUTO-002 commit foundation; AUTO-000 live MCP stage |
| A2 | AUTO-004 supported profile/schema/cache; AUTO-002 owns apply/ownership semantics; unresolved Dynamo behaviors remain blocked, not a compatibility claim |
| A3 | AUTO-005 canvas/app subtask + AUTO-000 separate-process convergence; no Player requirement at A3 |
| A4 | AUTO-006 worker/trust/install and MCP code/run gates + AUTO-000 worker migration |
| A5 | AUTO-005 Player subtask + AUTO-007 libraries as M4–M9 arrive; Python-containing Player graphs require A4 |

AUTO-000 is an integration umbrella, not an engine prerequisite; the staged dependencies above avoid a cycle. Proposed 04b is the acceptance contract; its documentation validator does not replace executing future engine/security tests or independent review.

## Review checklist for every PR
- Public API/schema and migrations described.
- Unit/geometry/IFC semantics tests where applicable.
- Undo and concurrent-edit handling.
- Keyboard and mouse accessibility.
- Benchmark on realistically sized models if touching hot paths.
- Screenshots or reproducible interaction tests for UI changes.
- Code is platform-tested on target desktop OSes.
