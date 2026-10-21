# Gap analysis: can a BIM model be built from scratch with the mouse? (2026-10-21)

Baseline: `main` at `dcf8382` (merge of PR #57). Reference: the three CADCIM books (Architecture 2026, MEP 2024, Structure 2024), paraphrased in `docs/reference/revit-books/`. Scope set by the owner on 2026-10-21: **architecture and MEP first; structure only where MEP needs it (trusses, supports for equipment); the rest of structure is a later stage.**

Companion evidence: `docs/audits/2026-10-21-arch-audit.md`, `-mep-audit.md`, `-struct-ifc-audit.md` (line-by-line traces, written by review agents; see section 12 for what was and was not independently re-checked).

---

## 1. The answer in brief

| Discipline | Can a user build it from scratch with the mouse today? |
|---|---|
| **Architecture** (levels, grids, walls, floors, doors/windows, stairs, roofs, ceilings, rooms) | **No, from a new project.** Every drawing tool is dead on an empty canvas (reproduced, section 3). Once that is fixed the tools behind it are largely in place and tested. |
| **Massing** | **No.** Nothing exists (a ribbon tab with one button that places the fan coil). |
| **HVAC ductwork** | **Partly.** Ducts/fittings/branches/insulation/compute work; there is no AHU, air terminal, damper or riser, and the only equipment is one fan coil. |
| **HVAC piping (CHW/HW)** | **Partly, weaker than ducts.** Pipes, slope, fittings, layout, sizing work; no pump, chiller, boiler, valve. |
| **Plumbing** | Pipes only. **No fixtures**, no drainage/vent design logic. |
| **Fire protection** | Calculation only. **No sprinkler heads** can be placed. |
| **Electrical** | A hand-drawn single-line tree with IEC 60364 sizing. **No panels, devices, luminaires, circuits, panel schedules, trays or conduit as elements.** |
| **Structure for MEP** | Truss (flat Warren), beam system, brace, pad footing, rectangular openings work. **Hangers/supports, sleeves: not implemented.** |

The engine underneath is stronger than the UI: 1,140 tests pass (section 12), walls/slabs/roofs/stairs/ducts/pipes/IFC/schedules/sheets are real. The gaps are mostly **(a) a few wiring blockers, (b) no MEP content library, (c) IFC output that drops information the app already holds.**

---

## 2. Tiers used in this report

| Tier | Meaning | Typical size |
|---|---|---|
| **A** | Blocks the from-scratch journey. Fix first. | hours to a few days, local |
| **B** | Easy: the engine/data already exists; needs wiring, validation or a fix. | days |
| **C** | Medium: a new feature on the existing architecture; no redesign. | 1–4 weeks each |
| **D** | Later stage: large, needs design decisions, or deferred by the owner. | months or deferred |

Sizes are my estimates from reading the code, not commitments.

---

## 3. Blockers for the from-scratch journey (Tier A)

| # | Finding | Evidence | Fix |
|---|---|---|---|
| A1 | **The plan canvas ignores clicks in an empty or level-only project.** `show()` returns early when no mesh/grid gives finite bounds, and every tool branch is below that line. | `model_view.rs:143` (`if !bounds.is_finite() { return; }`), tool branches from `:451`. **Reproduced at runtime** (section 12): after *New*, two clicks with the Grid tool and two with the Wall tool create 0 elements, also after adding a level. | Use a default view box when nothing is drawn. Add a test that starts from `new_model()`. |
| A2 | **Level 1, Level 2 and the Mechanical tab are hard-coded** to element ids 10/11 (sample model). A wall drawn in "Level 1" of a new project points at a missing level and IFC export then fails ("a wall stands on a level that is gone"). | `model_view.rs:68-75`, `model_edit.rs:315`, `panels.rs:941-948`, `export.rs:123` | Derive tabs and the browser from the document's levels; refuse tools with "add a level first". |
| A3 | **Esc leaves the tool at once** in the real app (global handler runs before the viewport), so "Esc ends the chain, Esc again leaves the tool" is unreachable. Tests pass because they bypass `Desktop::ui`. | `main.rs:567` | Remove the unconditional `choose_tool(Select)`; let each tool step back. Test through `Desktop::ui`. |
| A4 | **No Ctrl+S / Ctrl+N / Ctrl+O, no Save As button**, and the Revit two-letter shortcuts (WA, DD, ...) are defined but never consulted. | `main.rs:476` handles `save_as` but no button has it; `keymap` only printed in Help (`main.rs:656`) | Wire `crates/keymap` into key handling. |
| A5 | **Tool parameters leak between tools.** Floor, Roof, Ceiling, Door/Window/Component and footings share `slab_offset_mm` / `slab_thickness_mm`; after Ceiling the Door button places a door 2.7 m up, and the "AHU" fan coil gets its elevation from the same box. | `main.rs:450,458-459,462-464`; `model_edit.rs:568` | Per-tool state. |
| A6 | **`place_level` accepts duplicate names and levels < 100 mm apart**, which later make IFC export fail. Levels can't be renamed or deleted. | `crates/document/src/lib.rs:689-702`; `crates/arch/src/lib.rs:82-91` | Validate at placement; add rename/delete. |

Tests prove model-level behaviour well (38 pointer-driven scenarios in `model_view.rs`) but **none starts from a new project and none drives the ribbon, options bar or global keys** (`Desktop::ui` is never run). That is why A1–A4 were not caught; the end-to-end test in section 11 closes it.

---

## 4. Architecture: reference vs app

Reference sections: `arch-part1` (walls, doors/windows, modify), `arch-part2` (datums, floors, roofs, stairs, massing, toposolid), `arch-part3` (annotation, sheets, 3D, phasing, links).

| Revit step (reference) | App today | Tier |
|---|---|---|
| **New project / template / units / project info** (mep-part1 ch2) | *New* gives an empty model; no template, no units dialog (fixed mm/m²/m³), no project name/number (IFC project is always "Project1", key 1), no default levels | B (name/info), C (units/templates) |
| **Levels** are drawn in an elevation/section view; *Make Plan View* creates plans; renaming offers to rename views; only *Building Story* levels become storeys (arch-part2 §3.1a) | Options-bar button "Place Level" (click position ignored), no elevation placement, no level lines, no rename/delete, no plan-type choice | A (A2, A6), C (draw in elevation, level lines/bubbles) |
| **Grids**: line, pick lines, multi-segment, arc, bubbles, 2D/3D extents | Straight grids, auto-named, shown on every level; no rename, arc, pick or extents. Not exported to IFC | B (rename, IfcGrid), C (arc/multi-segment) |
| **Reference planes / work planes / scope boxes** | Absent | C |
| **Massing** (in-place mass, conceptual mass, Mass Floors, Building Maker: wall/floor/roof/curtain by face; arch-part2 ch10, mep-part3 D1) | Absent (one ribbon button re-using the fan-coil command) | **D** (needs a form-making geometry kernel; do not start before MEP content) |
| **Toposolid / site** | Absent | D |
| **Walls**: 12 sketch tools, Location Line, types with layers, joins with layer priority, attach top/base, top constraint + offset (arch-part1 §Wall) | Line/rect/pick lines/polygons/circle/arc (as straight facets), Location Line (6), layered types, joins (miter/butt automatic), per-end allow/disallow. **No top constraint "up to level", no attach, no layer-priority cleanup, no true curved walls, no stacked/slanted/tapered, no sweeps/reveals, no Offset/Radius fields.** No wall type ships (user types a name). Wall button resets type/size each press | B (remember last type; default height from level spacing), C (top constraint/attach, priority), D (curved, stacked, sweeps) |
| **Floors**: pick walls, boundary sketch, slope arrow, shape editing, types with layers | Polygon corners only; no pick walls, no edit-boundary after placement, no slope, no layers | C |
| **Doors/windows**: hosted, flip arrows, tag on placement, Load Family, Place on Face | Hosted in walls and cut in plan/3D/IFC; **5 compiled-in families**, one window; no Load Family UI (catalogue code exists but isn't wired); a door can overhang the wall end | C (family loader), B (clamp position) |
| **Stairs / ramps / railings** | Straight/L/U stairs with railing and sketched railings work. Ramp absent | C (ramp), D (stair by sketch/winder) |
| **Roofs**: footprint, extrusion, attach walls | Footprint with per-edge slope works; no extrusion roof, no overhang, no attach | C |
| **Ceilings** | Automatic and sketched, no ceiling plan view, outline not associative | C |
| **Rooms / areas** (room tag, separators, number/name) | Room from wall faces; name fixed "Room N"; **not exported as IfcSpace**; Area placeholder | B (editable name/number, export IfcSpace), C (separators, tags) |
| **Openings** | Wall, Shaft, Vertical work; By Face and Dormer placeholders | D |
| **Curtain wall** | Placeholder | D |
| **Views, sections, elevations, schedules, sheets, PDF** | Work (pointer-tested) | — |
| **Dimensions / tags / text / callouts / legends** (arch-part3 ch11–12) | Aligned dimension only (not associative), tag shows element name; no text, callouts, drafting views | C |
| **Project browser / View Control Bar** | Work; control bar at the top, not the bottom | B |

---

## 5. MEP: reference vs app, system by system

Reference: `mep-part2` (spaces/zones, HVAC, electrical), `mep-part3` (piping, plumbing, fire protection, documentation, families).

### 5.1 What MEP needs from the architecture model

| Revit behaviour (reference) | App today | Tier |
|---|---|---|
| **Link the architectural model**, Room Bounding on the link, shared coordinates (mep-part1 ch2, mep-part2 §1.1 step 2) | `Link Model` is a placeholder. *Import IFC* **merges** into the host model and skips doors, windows, spaces, openings, equipment | **D** (link semantics are tied to the collaboration design) / B for an interim "read-only import layer" |
| **Copy/Monitor** levels and grids | Absent | D |
| **Spaces** (Analyze > Spaces & Zones): upper limit + offset, plenum/chase spaces, space separators, Area & Volume computation, naming from linked rooms, colour schemes (mep-part2 §2) | Rooms only. No Space, no separator, no airflow/load data | **C, MEP-critical** |
| **Zones**: every space starts in a Default zone (mep-part2 §3) | Absent (the schematic engine has "zones", a different concept, with no UI) | C |
| **Actual vs calculated supply airflow per space** = sum of terminals in the space (mep-part2 §2.2, §5) | Absent (no terminals) | C (after terminals) |
| **Wall/floor penetrations by MEP** | Engine exists (`openbim_place::penetrations`: openings, firestops, register) **but the exporter passes empty lists** and nothing calls it from the UI, so a duct through a wall cuts no hole in 3D or IFC | **B, high value** |
| **Ceilings as hosts** for terminals/luminaires (Place on Face) | Ceiling exists; nothing can be hosted on it | C |

### 5.2 HVAC ductwork

| Revit behaviour (reference) | App today | Tier |
|---|---|---|
| Load families: AHU, VAV, FCU, diffusers/grilles, dampers (mep-part2 §5–6) | **One MEP family: "Fan coil unit, ducted".** The AHU and Component buttons both place it. No other equipment or terminal. No *Load Family*. Type parameters (airflow, connector sizes, kW) are not editable with the mouse | **C, the biggest MEP blocker** |
| Air terminal: Flow parameter, classification supply/return/exhaust, host on ceiling (§5) | Absent | C |
| Create Duct System from terminals + equipment, pick the source connector by number and flow direction (In/Out); return and exhaust are separate systems; only matching classifications accepted (§7) | Systems are groups of equipment only; ducts can't be members; **flow direction is ignored** | C |
| **Generate Layout**: Network (≤6 solutions) / Perimeter (≤5) / Intersection (≤8); routes members *visible in the current plan*; Conversion Settings (main/branch type, offset, flex) applied on Finish; blue main, green branches (§8) | Network (two orientations) + obstacle-aware Routed for exactly two connectors. **Joins like connectors regardless of flow direction** (the test ties three fan-coil supply outlets to one trunk), so it is not AHU→terminals. No Perimeter/Intersection; no conversion settings dialog; connectors must face sideways at the base height | C |
| Duct tool: Automatically Connect, Inherit Elevation/Size, justification, Size Lock, duct **type** selector with routing preferences, round/oval/flex, vertical offsets with transitions (§9) | Auto-connect, inherit, justification, tag, slope work. **Horizontal runs only (no risers/drops), no type selector, no round/oval/flex, no Size Lock, no angle set, clicks snap to a 50 mm grid but not to connectors or walls** | C (risers + types), B (connector snap) |
| Fittings: elbow, tee, transition, cross, union, cap, tap | Elbow, transition, tee, tap automatic; bend with a size change is refused; fittings are not independently selectable | C |
| **Sizing**: Friction, Equal Friction, Velocity, Static Regain (§10) | One velocity/friction-limit tree (Darcy–Weisbach/Colebrook), needs hand-marked End Points and a typed size catalogue. **No equal-friction/static-regain choice.** Air preset leaves a 1.5 m/s water-style limit | C |
| System Inspector, Check Duct Systems, Duct Legend | Absent | C |
| Insulation / lining | Works (not IFC geometry; lining not drawn) | B |
| Move equipment → connected ducts follow | Moving a joined duct is refused; equipment moves don't drag runs until Compute | C |

### 5.3 HVAC piping, plumbing, fire protection (mep-part3)

| Revit behaviour (reference) | App today | Tier |
|---|---|---|
| Pipe types + routing preferences + **conversion settings per system classification** (Main/Branch type and middle elevation) (A2, A4) | Pipe tool with system + diameter + slope list (Mechanical Settings); no pipe type/segment/material | C |
| Pumps, chillers, boilers, valves, accessories, in-line placement | Placeholders | C (families) |
| Slope with allowed-slope list and tooltip | **Works** for plain runs (not for runs leaving a connector or branching) | B |
| **Plumbing fixtures** hosted on floor/wall with fixture units; sanitary/vent with minimum slope, traps, vent tees (S1) | Placeholders. Sanitary/vent are computed as pressurised flow; no minimum-slope check (`check_fall` exists but unused) | C (fixtures), C (gravity logic) |
| Domestic hot/cold sizing from fixture units | Loading units typed per hand-placed End Point; curve is the user's CSV, **not saved with the model** | B (persist), C (from fixtures) |
| **Sprinklers**: hosted pendent / non-hosted upright, wet and dry systems, Area/130 min-heads check, **main/branch elevation must be above pendent and below upright heads** (S2, B1–B4) | No heads; sprinklers are abstract End Points with K-factor; every sprinkler with a demand operates (no design area); one `FireProtection` system value, no wet/dry | C |
| Create Piping System, Generate Layout for pipes | Works for fan-coil connectors only (no other equipment exists) | C |
| Show Disconnects | Absent | B |

### 5.4 Electrical (mep-part2 ch6)

| Revit behaviour | App today | Tier |
|---|---|---|
| Panels, transformers, switchboards as equipment | "Electrical Node" (Source/Board/Load) and "Cable" are **abstract records, not elements**: no Select, no Properties, no 3D, no schedule, no IFC entity | **C, MEP-critical** |
| Devices (sockets, switches), luminaires hosted on ceilings/walls | Placeholders. One luminaire pset path exists in IFC for lighting families, but no family | C |
| **Circuits**: Create Circuit, Select Panel, Edit Circuit, load classifications, demand factors | Absent. Each node has exactly one feeder | C |
| Cable tray / conduit with fittings | Placeholders (IFC writer supports them) | C |
| Panel schedules | Absent | C |
| Calculation | IEC 60364 sizing works with user CSV tables (not saved with the model) | B (persist) |

### 5.5 Coordination

| Item | App today | Tier |
|---|---|---|
| Clash / interference check | Placeholder text only | D (design with link/central model) / C for a simple in-model check |
| Schedules, tags, sheets | Schedules and sheets work; tags show the element name, not size/flow/system | B (tag text from parameters) |
| Worksets, phases, design options | Absent; status bar shows a constant "Workset: Mechanical (sample)" | D |

---

## 6. Cross-cutting: IFC, save, undo, families

| Area | Finding | Evidence | Tier |
|---|---|---|---|
| **IFC: run system lost** | Every duct/pipe is exported with `System::Other`; the per-run system the tools set (`MepData.run_systems`) is never read. A straight "Supply Air" duct with no fitting exports as system "Other 1" | `export.rs:205,207` (comment: "the document does not say which system a run belongs to yet" is stale) | **B** |
| IFC: user systems | Systems the user creates are not written as `IfcSystem`; equipment is never assigned to its distribution system | `lib.rs:1823-1838`; `export.rs` | B |
| IFC: rooms, grids, site | `rooms: vec![]`, grids/rooms/levels-adjacent records skipped **without a warning**, `georef: None`. The IFC crate can write `IfcSpace` + boundaries | `export.rs:198,291-293` | B |
| IFC: penetrations | Not fed (see 5.1) | `export.rs:290-291` | B |
| IFC: property sets / quantities | Only `Pset_StairCommon` and quantities for wall/slab/door/window/space. No `Pset_*Common` (fire rating, IsExternal, LoadBearing), no `Qto_` for ducts, pipes, fittings, equipment, beams | `lib.rs`, `pset_map` empty | C |
| IFC: GlobalIds | Product ids stable. **Storeys, relationships, property sets, systems use a running counter**: adding a level or toggling "include own data" changes them. A deleted newest element's number is reused | `crates/ifc/src/lib.rs:645`, `crates/document/src/lib.rs` `next_id` | B (derive from level/system ids) |
| IFC import | Foreign files: walls, beams, columns, slabs, footings, pipes, ducts as extruded profiles only; doors, windows, openings, fittings, equipment, ports, spaces skipped; foreign GlobalIds discarded; units limited to m/mm. The "round trip loses nothing" result works because the whole model is embedded as text in an `OpenBIM_Document` property set | `import.rs`, `reader.rs:565-596` | C (semantic import of MEP) |
| **Save / recovery** | Working file is a text `.obim` snapshot written on Save (atomic). **No autosave, backup or crash recovery.** `crates/oplog` (binary log, CRC, snapshots, merge) exists but **no crate depends on it** | `file.rs`, `native.rs`; `grep openbim_oplog` outside the crate: none | B (autosave/backup), **decision needed** (section 8) |
| Undo | Works, but each step clones the whole model with no cap | `lib.rs:1267,1292` | C (before large models; PERF-001 is open) |
| Units | Fixed metric; no Project Units dialog | — | C |
| Families / types | Family engine is capable (solver, MEP connectors, catalogue loader) but **no family editor, no load family**; types exist for walls/beams/columns/slabs/footings/rebar only, not for ducts, pipes or fittings | `crates/family`, `type_editor.rs` | C (loader), D (editor) |
| Materials | Wall layer materials are free text; structural material is a 4-value enum | — | C |
| Compute tables | Size catalogue, loading-unit curve and cable tables are typed or loaded per session and **not saved**; Mechanical Settings size lists are not used by Compute | `compute_ui.rs` | B |
| Compute vs systems | Compute works on the `System` enum, not on the named system objects, so two separate CHW loops fail with `MoreThanOneSource` | `compute.rs` | C |
| Marking an End Point on a joined run end | **Drops the equipment join** | `compute_ui.rs:132` (`equip_links.retain`) | B |
| Collaboration | No code for central model, borrowing, links or workset; only `Document::transact` optimistic revision check | — | D |

---

## 7. Structure, limited to what MEP needs (rest deferred)

| MEP need | Status | Tier |
|---|---|---|
| Truss (support frames) | Works; flat Warren only, snaps 50 mm, not exported as a truss (members leave as plain beams/members, no parent) | C |
| Beam system / brace | Work | — |
| Equipment pad | Pad footing works by click + typed offset; **not associated with the equipment** (pad doesn't follow) | B/C |
| **Hangers / pipe and duct supports** | Placeholders; no element, no family, no host-on-structure, no auto-hangers along runs, no IFC class | **C — treat as MEP families** (see 5.2) |
| Sleeves / openings in walls | Rectangular wall opening works; **MEP-driven round sleeves and firestops not wired** | B (wire penetrations) |
| Openings in beams/columns | Absent | D |
| Structural wall, framing, connections, rebar beyond beams, analytical model, loads, boundary conditions, piles | Absent or placeholder | **D (owner-deferred)** |

The Structure book has not been extracted into a reference doc yet; do that when the structure stage starts (extracted text is available locally).

---

## 8. Documented decisions that the code does not (yet) follow

These are the project's own choices; the code is behind them. Please confirm each, since they change priorities.

| Decision (source) | Code reality |
|---|---|
| Project storage = operation log + snapshots; IFC4 is exchange only (agreed 13 Oct) | Working file is a full text snapshot; `crates/oplog` unused; no recovery |
| Unbuilt ribbon tabs/tools are shown **disabled** (`01-ui-specification.md`) | They are enabled and print "X is not implemented in this UI prototype" (`main.rs:495`) — MEP: 45 of 63 HVAC, plumbing and electrical buttons per the MEP audit count (18 wired) |
| Revit default shortcuts, customizable | `crates/keymap` built and tested but never consulted for key events |
| Electrical calculations imported from edesign-alike rather than re-implemented (`06-mep-schematic-automation.md`) | A complete IEC 60364 implementation is in `crates/calc/src/electrical.rs` (988 lines) |
| Walls, grids, levels, tags behave identically to Revit incl. two-stage Esc | Two-stage Esc unreachable (A3); no level lines; no click on empty canvas (A1) |
| Compute tables are supplied as CSV and never shipped (fine) | They are also not saved with the project, so each session re-enters them — probably unintended |
| Stable GlobalIds required | Stable for products only (section 6) |
| Quantity/unit source of truth for U-value and area exports (envelope families) | Layered walls have U-value and envelope areas in `crates/arch`; not checked end to end in this review |

Several docs are stale in the other direction (3D view plan says no redo/no 3D door placement; `gap-review.md` says the desktop app is not on main; `README.md` lists 7 of 19 crates). A docs refresh is a Tier B item.

---

## 9. Remediation order (MEP first)

**Step 0 – Walking skeleton (Tier A + quick B, ~1 week).** Fix A1–A6, then add the end-to-end test of section 11. Acceptance: new project → 3 levels → grids → walls (typed) → floor → door → export IFC passes from `Desktop::ui` events.

**Step 1 – IFC honesty (Tier B, a few days).** Export the run system; export rooms and grids; warn on every dropped category; write user systems and put equipment in them; wire penetrations; derive storey/system/relationship GlobalIds from stable ids; persist Compute tables; fix the End Point unjoin.

**Step 2 – MEP content (Tier C, the largest MEP unlock).** A family loader (use `crates/family::catalogue`) and editable type parameters, then the first library: AHU, diffuser/grille, VAV, pump, valve, plumbing fixture, sprinkler head, luminaire, panel board, hanger/support. Everything in 5.2–5.4 depends on this.

**Step 3 – Spaces, flow and systems (Tier C).** Space/Zone with airflow data → terminals with Flow → named systems that include terminals and respect connector flow direction → Generate Layout from AHU to terminals (conversion settings, Perimeter/Intersection) → Compute on the named system with all four sizing methods.

**Step 4 – Routing completeness (Tier C).** Vertical runs (riser/drop), duct/pipe types with routing preferences, connector snapping, size lock, moving equipment drags runs, minimum-slope check, gravity drainage rules.

**Step 5 – Electrical as objects (Tier C).** Panels/devices/luminaires as elements, circuits, panel schedules, cable tray/conduit; then the cable tree reads from circuits.

**Step 6 – Coordination (Tier C→D).** In-model clash check, then a link model (read-only, reload, visibility) once the collaboration design is settled.

**Later (Tier D).** Massing, toposolid, curved/stacked walls, curtain walls, family editor, RVT export, collaboration (central model, borrowing), phasing/design options, and the structure stage.

---

## 10. Reference book cross-checks that changed earlier assumptions

- Levels, grids and reference planes live in **Architecture ch. 6**, not in the walls chapter; levels are drawn in elevation/section views (arch-part2 §3.1a).
- The MEP book's electrical content is **Chapter 6**; Chapters 7–10 contain piping, fire protection, documentation and families (mep-part3). It has **no clash procedure** (Navisworks only, Case Study 1) and **no duct fitting or routing-preference detail**; those sections in `mep-part2` are marked `[knowledge]`.
- The Architecture book's structure chapter has no footings, slabs-on-grade, piles or rebar, so it cannot be the structure reference; the Structure book is.
- `docs/revit-book-review.md` covered Architecture and MEP only; this review adds datums/massing/toposolid (arch-part2), documentation (arch-part3) and the piping/fire/electrical order of work (mep-part2/3).

---

## 11. Proposed tests (turn the references into gates)

1. **End-to-end "small office from scratch"** (sketch in the architecture audit, section 4.2): new project → levels → grids → type with layers → rectangle walls → floor → door/window → stair → roof → ceiling → room → section → save/reopen → export IFC with counts. Needs: fix A1, a plan model-to-screen transform in the harness, and a way to run `Desktop::ui` (extract the body from the `eframe::Frame` argument).
2. **MEP chain**: fan coil (today) → system → layout → End Points → Compute → Apply sizes → export; assert the system name and ports in the IFC (this test would currently expose the `System::Other` export and the End Point unjoin bugs). Replace the fan coil by AHU + terminals after Step 2.
3. **Acceptance checklists**: each reference file ends with a checklist (~90–100 statements for Architecture; also MEP). Convert them to tests per feature as it is built; keep the id of the reference section in the test name.
4. **Ribbon honesty test**: every ribbon id is either handled or disabled.

---

## 12. How this was verified, and what was not

**Run by me (this session):**
- Cloned `main` at `dcf8382`; `cargo test --workspace --release` built and ran: **1,140 passed, 0 failed, 2 ignored**.
- Wrote a throwaway pointer-event test in `model_view.rs` (reverted, not committed): after *New*, Grid and Wall clicks create 0 elements; after adding a level, Wall clicks still create 0. This confirms A1 at runtime.
- Re-read in the code: `bounds.is_finite()` early return before the tool branches (A1), global Esc (A3), hard-coded level tabs (A2), `System::Other` in the exporter, absence of any `oplog` dependency, no autosave/recovery, rooms/grids skipped in export, one MEP family in the library, AHU/Component both mapped to the fan coil, `equip_links.retain` in End Point marking.

**From the review agents' reading only (not re-checked line by line by me):** the per-step status tables, the IFC coverage matrix, the 15 + 10 + 13 doc/code discrepancy lists, and the `file:line` citations (the structure audit marks nine as approximate with `~`). Treat them as high-quality leads. The agents did not run the app.

**Not done:** the desktop app was not launched or looked at on a screen; no performance measurement; IFC validity was not checked with an independent reader (CI runs IfcOpenShell scripts under `spikes/`); no side-by-side with a running Revit, so "matches Revit" means "matches the book"; Revit-behaviour statements marked `[knowledge]` in the reference docs come from general Revit/IFC knowledge, not the books.

---

## 13. Questions for the owner

1. Should unbuilt ribbon buttons be **disabled** as specified (cheap to do now), or stay as "not implemented" messages while you test?
2. Is the working-file decision (operation log + snapshots) still wanted? If yes, autosave + recovery is the first use of `crates/oplog`.
3. Which MEP families first for Step 2? Proposed: AHU, ceiling diffuser, VAV, pump, valve, WC/basin, sprinkler head, luminaire, panel board, hanger.
4. Electrical calculations: keep the in-repo IEC 60364 implementation, or still import from edesign-alike?
5. Test building for the end-to-end tests: the office was chosen on 14 Oct; should the MEP test use a data hall instead?
