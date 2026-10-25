# Remediation plan: revit-alike vs the Revit references (2026-10-21)

Status: **proposed, dated 2026-10-21.** Not an agreed design decision; the agreed design is
`docs/revit-style-ui/README.md` and documents 01-06. Where this document and the design docs
disagree, the design docs win.

Baseline under review: `origin/main` at `dcf838297246d1adb0093143ad86a393af8e0303` (merge of PR #57).
Reference material: branch `docs/revit-book-references` at `6ac8addc9550fcbed3d2b5cce868a540fe0a0780`
(7 commits, docs only, +4,304 lines). Its companion evidence is `docs/gap-analysis-2026-10-21.md` and
`docs/audits/2026-10-21-{arch,mep,struct-ifc}-audit.md`.

Scope set by the owner on 2026-10-21: **architecture and MEP first; structure only where MEP needs it
(trusses, supports/hangers, equipment pads, sleeves); the rest of structure is a later stage.**

> **Verification status (2026-10-25).** Re-verified against `main` @ `b6ef0ee`.
> The §5.1 "Easy" tier (B2, B7–B11, B12–B16, B21–B24) is **closed**
> (`b17a1ff`, `fccee7b`, `91eb050`, `f84104a`, `7afe07b`). This doc was last
> verified at `dcf8382` (112 commits stale); rows updated below. Evidence:
> `docs/agent-team/evidence/gap-p0-2026-10-25/`.

---

## 0. How to read this document

### 0.1 The four work classes

The owner asked for the gaps to be split between *what is easy or belongs to a later stage* and
*what has to change in the actual application*. Those are two different axes, so the register below
uses four classes. Class is the axis that decides **who does it and when**; the remediation column
decides **what the code change is**.

| Class | Meaning | Who | When |
|---|---|---|---|
| **1. Reference & documentation** | Paper gaps. The books are distilled, or a document is stale. No application code changes. | `bim-reference` / any | Now; blocks nothing |
| **2. Easy / wiring** | The engine, data structure or crate already does the work; the desktop app does not reach it, or a small defect hides it. No redesign. | `bim-rust` | Now, in the order of section 4 |
| **3. Medium / new feature** | A real feature on the existing architecture. No redesign of the document model. | `bim-rust` | After class 2, MEP first |
| **4. Later stage** | Large, needs a design decision, or deferred by the owner. | design decision first | Not in this plan |

A gap can be class 2 in effort and class 3 in consequence. Class 2 is about **where the code lives**,
not about risk: `B10` (penetrations) is a one-line call site but changes what other BIM tools see.

### 0.2 Where each gap lands

The "lands in" column names the crate and file. `apps/desktop` is the egui shell;
`crates/document` is the model and command boundary; `crates/ifc` is exchange; `crates/family`
is the parametric family engine; `crates/calc` is calculation; `crates/route` is routing and layout.
Nothing in this plan proposes a new crate or a change to the document model's public shape, except
where a row says so explicitly.

### 0.3 Status vocabulary

`WORKS` = wired ribbon -> tool -> pointer event -> command -> result, with a test that exercises it.
`PARTIAL` = wired, with the stated gap. `PLACEHOLDER` = a button exists and only prints
"X is not implemented in this UI prototype" (`apps/desktop/src/main.rs:495`).
`ABSENT` = nothing in the code.

---

## 1. Class 1 - reference material and documentation gaps

These cost no application code and can be closed immediately. They matter because every later class
is measured against the reference: a wrong reference produces wrong remediation.

| ID | Gap | Remediation | Lands in | Effort |
|---|---|---|---|---|
| **R1** | The reference docs and the gap analysis exist **only on the branch** `docs/revit-book-references`. There is no PR for it, so nothing on `main` can cite them. | Open a docs-only PR from `docs/revit-book-references` to `main`. It adds 11 files and touches no code, so it cannot break a build. | GitHub | trivial |
| **R2** | The Structure book is not distilled. The owner deferred structure, so this is correct for now, but the reason is recorded nowhere in the reference index. | Keep `docs/reference/README.md` line 14 as the deferral record; add the Structure book to the extraction queue in `docs/agent-team/` so the stage is not forgotten. | `docs/reference/README.md`, `docs/agent-team/` | trivial |
| **R3** | The reference docs are not linked from `docs/revit-style-ui/README.md`, so a reader of the design never reaches them. | Add a "Reference material" bullet to the Documents list, naming the six files and the `[book]` / `[knowledge]` convention. | `docs/revit-style-ui/README.md` | trivial |
| **R4** | `README.md` lists 5 of the 20 workspace members (its crate table has `geom`, `family`, `place`, `keymap`, `arch`, plus a `spikes/*` row), and is missing `document`, `ifc`, `view`, `route`, `sheets`, `calc`, `automation`, `oplog`, `sketch`, `viewport`, `viewport-egui`, `photo`, `schematic`, `commands` and `apps/desktop`. | Regenerate the table from `Cargo.toml` rather than by hand, and add the one-line "what it is" for each of the missing crates. | `README.md` | trivial |
| **R5** | Stale claims in existing documents. The arch audit lists 15 (D1-D15) and the MEP audit 13; the recurring ones are: `docs/revit-ux-parity-audit.md` claims event-test verification that does not cover the ribbon, options bar or global keys; `docs/3d-view-parity-plan.md` claims no 3D tools and no redo; `docs/agent-team/gap-review.md` claims `main` has no desktop app; `docs/revit-style-ui/06` claims the schematic engine is not in the candidate; `docs/agent-team/gap-review.md` marks `05-backlog.md` MEP-002 "PLANNED/UNVERIFIED" although the backlog row carries no status column at all; `docs/revit-book-review.md` A8 mentions the fixed level tabs but not that they break IFC export in a new project. | Fix each claim at its source. Keep the historical value: mark the superseded statement "superseded 2026-10-21" rather than deleting it, so a reader can see what changed. | see per-row | small |
| **R6** | The reference docs cite "Text extraction `arch.txt`, lines 1134-10368" but the extraction itself is not in the repository, so the line references cannot be reproduced by a reviewer. | Add an extraction manifest to `docs/reference/`: the source file name, its SHA-256, the tool used, and the page/line convention. Do **not** commit the book text or the extraction - the books are copyrighted and only paraphrase may be stored. | `docs/reference/` | small |
| **R7** | The reference acceptance checklists (~90-100 statements for Architecture, plus MEP) are prose. They become gates only when they are tests. | Convert them one feature at a time as the feature is built, carrying the reference section id into the test name (`arch-part1` section 4.1 -> test `..._matches_arch_part1_wall_...`). Do not convert them in bulk before the feature exists. | `crates/*/tests`, `apps/desktop/src/*` | ongoing |
| **R8** | The reference docs mark 155 statements `[knowledge]` (general Revit/IFC knowledge, not in the books). A reader cannot tell which acceptance criteria depend on an unverified premise. | Add a column to the acceptance checklists: `book` / `knowledge` / `unverified`. Where a `[knowledge]` row gates a safety-relevant behaviour, mark it for confirmation against Autodesk's own documentation. | `docs/reference/revit-books/*.md` | small |
| **R9** | `docs/gap-analysis-2026-10-21.md` section 12 states that its own per-step tables and `file:line` citations were **not** re-checked. The citations are the evidence for every class 2 row below. | Re-check the citations against the pinned SHA and record the result in the gap analysis (done in the verification appendix of this document). Any citation that does not hold is struck from the register. | `docs/gap-analysis-2026-10-21.md` | small |

**Class 1 acceptance:** `main` contains the reference docs, `README.md` lists 20 members, every
documented claim that the code contradicts has been corrected or marked superseded, and the
extraction manifest exists.

---

## 2. Class 2 - easy / wiring gaps

Every row here is a defect or a missing call to code that already exists. The estimate is hours to a
few days each; the order in section 4 is the recommended one, not a hard dependency.

### 2.1 The shell blockers (must be fixed before anything else)

| ID | Gap | Remediation | Lands in | Acceptance |
|---|---|---|---|---|
| **B1** | **The plan canvas accepts no input in an empty or level-only project.** `show()` computes `bounds` only from element meshes and grid ends, then returns early when it is not finite. Every tool branch, including Select, is below that line. | Replace the early return with a default view box (a 20 m x 15 m box around the origin) when nothing is drawn, so the model-to-screen transform exists from the first click. | `apps/desktop/src/model_view.rs:143` | A test that starts from `new_model()` and creates a grid and a wall with pointer events. |
| **B2** | **The plan tabs "Level 1", "Level 2" and the Mechanical tab are hard-coded** to `ElementId(10)` / `ElementId(11)`, which belong to the sample model. In a new project a wall drawn in the "Level 1" tab points at a missing level, and IFC export then fails. | Derive the tab list from `document.levels()`. Change `plan_level()` to return `Option<ElementId>` and have each tool refuse with "add a level first". Remove ids 10/11 from the extra-tab filter and from the 3D family-placement fallback. | `apps/desktop/src/model_view.rs:68-75`, `model_edit.rs:315`, `panels.rs:941-948`, `:1003`, `view3d.rs:241` | A new project has exactly the levels the user created, no dead tabs, and a tool refused with a message rather than a dangling level. |
| **B3** | **Esc leaves the tool immediately** in the running app, so the documented two-stage Esc ("Esc ends the chain, Esc again leaves the tool") is unreachable. Tests pass because they call `show()` directly and never run `Desktop::ui`. | Delete the unconditional `choose_tool(Select)` and let each tool step back through its own pending state. Guard it with `egui_wants_keyboard_input` so it does not fire in a text field. | `apps/desktop/src/main.rs:567` | A test that drives `Desktop::ui` and asserts two-stage Esc for the wall and stair tools. |
| **B4** | **No Ctrl+S / Ctrl+N / Ctrl+O, no Save As button**, and the Revit two-letter shortcuts are defined in `crates/keymap` but never consulted. | Wire `crates/keymap` into key handling and add the Save As button to the quick-access bar. The `save_as` command id already exists and is already handled. | `apps/desktop/src/main.rs:476`, `ribbon.rs:46-54`, `crates/keymap` | Every key in the keymap is exercised by a test that sends the key through the UI path. |
| **B5** | **Tool parameters leak between tools.** `slab_offset_mm` / `slab_thickness_mm` are shared by Floor, Roof, Ceiling, Door/Window/Component and the footings. After Ceiling, the Door button places a door 2.7 m up and the "AHU" gets its elevation from the same box. | Give each tool its own state, or reset the shared fields in the command that sets the tool. | `apps/desktop/src/main.rs:450,458-464`, `model_edit.rs:568`, `panels.rs:482-483` | A test that presses Ceiling then Door and asserts the door elevation is the default. |
| **B6** | **`place_level` accepts duplicate names and levels closer than 100 mm**, which `Levels::new` then rejects at export, so a model that was usable becomes unexportable. Levels cannot be deleted, and rename is never surfaced for them even though the model-level rename exists. | Validate name uniqueness and the 100 mm gap at placement, with the same rule `crates/arch` applies. Surface the existing model-level rename for levels, and add delete. | `crates/document/src/lib.rs:689-702`, `crates/arch/src/lib.rs:82-91` | A test that places two levels 50 mm apart with the same name and asserts a refusal, plus rename and delete round-trips. |

**Class 2.1 acceptance (the walking skeleton):** from a pristine *New*, a user can create three
levels, draw grids, draw typed walls, place a floor, place a door and window, and export IFC with
counts, driven entirely through `Desktop::ui` events. This is the gate for every later step.

### 2.2 Exchange and persistence

| ID | Gap | Remediation | Lands in | Acceptance |
|---|---|---|---|---|
| **B7** | **Every duct and pipe is exported with `System::Other`.** The exporter builds each run with `System::Other` and only copies a system from a joined fitting; `MepData.run_systems`, which the tools do set, is never read. A straight "Supply Air" duct with no fitting exports as system "Other 1". | Read `MepData.run_systems` when building the run. Update the stale comment at `export.rs:203`. | `apps/desktop/src/export.rs:205-207` | An IFC written from a straight supply-air duct carries the run's system, asserted by parsing the file. |
| **B8** | **Rooms, grids and the site are dropped from IFC export.** `rooms: vec![]`, Grid/Room/Opening categories skipped, `georef: None`. The IFC crate can write `IfcSpace` with boundaries and quantities and has tests for it; the app never feeds it rooms. | Feed the document's rooms and grids into the scene. The crate already writes `IfcSpace` + boundaries and quantities. | `apps/desktop/src/export.rs:198,291-293` | An IFC from a model with rooms and grids contains `IFCSPACE` and `IFCGRID` with the right counts. |
| **B9** | **User-created systems are not written as `IfcSystem`.** Only one `IfcDistributionSystem` per `System` enum code, named "<System name> 1". Equipment is never assigned to its distribution system. | Write an `IfcSystem` per `MepSystemDef` and assign members with `IfcRelAssignsToGroup`. | `crates/ifc/src/lib.rs:1823-1838`, `apps/desktop/src/export.rs` | An IFC from a named system carries the user's system name and its member list. |
| **B10** | **Wall and floor penetrations are never created.** `openbim_place::penetrations` computes openings, firestops and a register, and has tests; the app passes empty lists and nothing calls it from the UI, so a duct drawn through a wall cuts no hole in 3D or IFC. | Call the penetrations engine when a run crosses a wall or slab, and feed the result to the exporter. | `apps/desktop/src/export.rs:290-291`, `crates/place/src/penetrations.rs` | A duct crossing a wall produces an `IFCOPENINGELEMENT` and a visible hole in the 3D view. |
| **B11** | **GlobalIds are not stable for everything.** Product ids are stable, but storeys, relationships, property sets and systems use a running counter, so adding a level or toggling "include own data" changes them, and a deleted newest element's number is reused. Project key is fixed at 1 and the project name at "Project1", so two projects export identical GUIDs for equal ids. | Derive the non-product GlobalIds from stable ids (level id, system id) and add a real project key/name. | `crates/ifc/src/lib.rs:645`, `crates/document/src/lib.rs` `next_id`, `apps/desktop/src/model_edit.rs:305-306` | Export the same model twice with an unrelated edit in between; product, storey and system GUIDs are unchanged. |
| **B12** | **Compute inputs are not persisted.** The size catalogue, loading-unit curve and cable tables are typed or loaded per session and not saved with the model, so every session re-enters them, and results are not stored either. | Add the tables and the last result to the native document format. | `crates/document/src/native.rs`, `apps/desktop/src/compute_ui.rs` | Save, reopen, and Compute runs with the same inputs and the same result without re-typing. |
| **B13** | **The MEP Settings size lists are ignored by Compute.** The user edits duct widths, heights and pipe diameters in Manage > MEP Settings, and Compute still demands a typed catalogue. | Make Compute read the MEP Settings lists as its default catalogue, and say so in the Compute dialog. | `apps/desktop/src/compute_ui.rs`, `mep_settings_ui.rs` | An empty Compute catalogue that follows the MEP Settings lists sizes a run successfully. |
| **B14** | **Marking an End Point drops the equipment join.** `end_point_click` clears `equip_links` on the clicked port before adding the end point, so marking a Source on a duct end joined to equipment silently unjoins it. After Generate Layout every trunk end is joined, so the normal "mark the source" step breaks the join. | Do not clear the join; only kind 4 ("from the connector it meets") should re-add it. | `apps/desktop/src/compute_ui.rs:132` | A test that marks a Source on a joined end and asserts the `IfcRelConnectsPorts` still exists. |
| **B15** | **The Air preset does not change the velocity limit.** The fluid preset sets density, viscosity and roughness only; `max_velocity` stays at the default 1.5 m/s, so an air duct computed without touching the preset uses water properties and a water velocity limit. | Have the preset set the velocity and friction limits as well as the fluid, and show the resulting limits in the dialog. | `apps/desktop/src/compute_ui.rs:69` | Computing a supply-air duct with the Air preset asserts a limit consistent with air, not water. |
| **B16** | **No Save As.** The `save_as` command id is handled but no button carries it. | Add the button to the quick-access bar. | `apps/desktop/src/ribbon.rs:46-54` | Save As writes a second file and the first is unchanged. |
| **B17** | **Ribbon honesty.** `01-ui-specification.md` says unbuilt tabs and tools are shown disabled; the app instead enables them and prints "not implemented". 45 of 63 HVAC, plumbing and electrical buttons are in this state (HVAC 8 of 21 wired, Plumbing 7 of 22, Electrical 3 of 20). | Show unbuilt buttons disabled, with the tooltip naming the backlog row that will build them. Keep the message path for commands that are intentionally reachable but unimplemented. | `apps/desktop/src/main.rs:495`, `ribbon.rs` | A test that walks every ribbon id and asserts each is either handled or disabled. |
| **B18** | **Compute works on the `System` enum, not on the named system objects**, so two separate chilled-water loops in one model are one computation and fail with `MoreThanOneSource`. | Give Compute a system-object scope: the named system selected in the System Browser. | `crates/document/src/compute.rs:118-122` | Two CHW loops in one model compute independently. |
| **B19** | **Trusses and beam systems lose their parent on export.** Members leave as plain `IfcBeam` / `IfcMember` with no `IfcRelAggregates`, so a truss is not a truss in another tool. | Write the parent as `IfcElementAssembly` (or `IFCTRUSS` where the schema allows) with `IfcRelAggregates` to the members. | `apps/desktop/src/export.rs:198`, `crates/ifc/src/lib.rs` | An IFC from a truss contains one assembly and its members, asserted by parsing. |
| **B20** | **No autosave, backup or crash recovery.** The working file is a text snapshot written atomically on Save; `crates/oplog` (binary log, CRC, snapshots, merge) exists and no crate depends on it. | Add autosave and a recovery copy on startup. This is the first real use of `crates/oplog`, so it needs the owner's decision on the storage model (section 6). | `apps/desktop/src/file.rs`, `crates/oplog` | A killed process leaves a recoverable file; a normal save still round-trips exactly. |
| **B21** | **Rooms cannot be named or numbered** and the room name is fixed "Room N". The reference (arch-part3 2.6) has Number, Name, Department, and the room tag is a real element. | Make Number and Name instance parameters, editable in Properties and in the schedule. | `crates/document/src/rooms.rs`, `apps/desktop/src/panels.rs` | A room's number and name survive save/reopen and appear in the room schedule. |
| **B22** | **Levels have no lines or bubbles in plans**, so a plan cannot be read the way Revit reads it. | Draw the level line and bubble in plan views for levels outside the view range. | `apps/desktop/src/model_view.rs` | A plan with a level above and below shows both bubbles. |
| **B23** | **Tags show the element name, not a parameter.** Revit tags read parameters (size, flow, system). | Make the tag text a parameter reference resolved at draw time. | `crates/document/src/lib.rs` `tag_text`, `apps/desktop/src/model_view.rs` | A duct tag shows its size and system. |
| **B24** | **The equipment pad is not associated with the equipment**, so moving the equipment does not move the pad. | Host the pad on the equipment instance. | `crates/document/src/lib.rs:844` | Moving the unit moves its pad. |
| **B25** | **Undo restores the model but not the view or selection**, so undoing a just-made level leaves `View::Level(gone)` which resolves to `ElementId(10)`. | Restore the view and selection with the snapshot, or clamp the view to a valid level. | `crates/document/src/lib.rs:1275-1296`, `apps/desktop/src/model_edit.rs:316` | Undo of a created level returns to a valid plan. |

| **B26** | *(23 Oct: deprecated and noticed, removal blocked on edesign-alike sizing cables; see `docs/decisions/0002-electrical-engine.md`)* **`crates/calc/src/electrical.rs` duplicates a capability that belongs to edesign-alike.** The file is a complete IEC 60364 engine (~988 lines) with `crates/document/src/compute_elec.rs` behind it and the Compute button wired to it, while `06-mep-schematic-automation.md` decision 2 says electrical calculations are imported and not re-implemented. | Mark the file and `compute_elec.rs` deprecated, remove them from the desktop path, and pin edesign-alike's electrical crates by git SHA. Owner decision of 2026-10-21 (option B). | `crates/calc/src/electrical.rs`, `crates/document/src/compute_elec.rs`, `apps/desktop/src/elec_ui.rs` | No desktop build or Compute test may reference the in-repo engine after the change. |

**Class 2 acceptance:** the IFC written from a representative model contains the run systems, rooms,
grids, named systems, penetrations and stable GlobalIds; Compute is persistent and reads the MEP
Settings lists; the shell blockers are closed by tests that start from an empty document; and no
desktop path references the deprecated electrical engine.

---

## 3. Class 3 - medium feature gaps

New features on the existing architecture. Each is one bounded task; each needs the class 2 shell
fix where it touches the canvas.

### 3.1 MEP content - the largest single unlock

Nothing else in MEP can be built until equipment, terminals and fixtures exist as families with
editable type parameters. Today `openbim_family::library::all()` returns five families - handle,
"Door, single flush", "Window, fixed with mullions", "Door, arched", "Fan coil unit, ducted" - and
the ribbon's "AHU" and "Component" buttons both place the fan coil. The reference names the families
the books actually use: air terminals (grille / register / diffuser), air handling units and VAVs,
mechanical equipment, plumbing fixtures, sprinklers, luminaires, devices, panels and transformers.

| ID | Gap | Remediation | Lands in | Reference |
|---|---|---|---|---|
| **C1** | **No family loader and no Load Family UI.** `crates/family::catalogue` exists but nothing in `apps/desktop` calls it; `load_family` is only an icon id. | Wire the catalogue loader and a Load Family dialog. Families must be user data, not Rust code, so that the owner can add content without a rebuild. | `crates/family/src/catalogue.rs`, `apps/desktop/src/main.rs` | arch-part1 4.1 (Load Family, Specify Types); mep-part2 5-6 |
| **C2** | **Type parameters are not editable with the mouse.** Properties filters to `Scope::Instance`; the fan coil's airflow, water flow, duct and pipe sizes, voltage and power are all Type-scope, and the family defines no types, so no MEP parameter is editable. | Expose Type-scope parameters in a type editor for MEP families, and add `types:` to the fan coil so it can be duplicated. | `crates/family/src/mep.rs`, `apps/desktop/src/type_editor.rs` | mep-part2 6 (properties vary with the family) |
| **C3** | **The MEP content library holds one family.** | Add, in this order: AHU, ceiling diffuser, VAV, pump, valve, WC/basin, sprinkler head, luminaire, panel board, hanger/support. Each needs connectors with a domain, a system classification and a flow direction. | `crates/family/src/library.rs` | mep-part2 5-6, mep-part3 A1, B1 |
| **C4** | **Air terminals do not exist**, so there is nothing for a duct system to serve and no Flow to drive sizing. | Add the terminal families with a Flow instance parameter and the four airflow-direction arrow flags. | `crates/family/src/library.rs` | mep-part2 5 |
| **C5** | **Nothing can be hosted on a ceiling or a face.** | Implement host resolution for families: a ceiling or wall face becomes the host, and level/offset come from it. | `crates/family/src/mep.rs:121-128` (`Host` already exists), `apps/desktop/src/model_edit.rs` | mep-part2 5, mep-part3 A1 |
| **C6** | **Spaces and zones do not exist.** Rooms only, with no airflow or load data, and no colour scheme. The reference treats Spaces as the driver of everything downstream: airflow per space, sprinkler count per space, lighting per space. | Add Space (upper limit, offset, separators, airflow and load data), Zone, and a colour scheme with a legend. | `crates/document` (new module), `apps/desktop/src/panels.rs` | mep-part2 2-4 |
| **C7** | **Systems are groups of equipment, not of ducts and pipes.** Ducts and pipes cannot be members, and connector flow direction is ignored, so the layout ties three supply outlets to one trunk and is not an AHU-to-terminals system. | Let runs be system members, and make connector flow direction part of the join rule. | `crates/document/src/mep.rs:105-111,332`, `crates/document/src/layout.rs:121-140` | mep-part2 7 |
| **C8** | **Generate Layout is Network-only** (two orientations) plus an obstacle-aware Routed solution for exactly two connectors. No Perimeter, no Intersections, no Custom, no editable base, no conversion settings, connectors must face along a plan axis at the base height. | Add Perimeter, Intersections and Custom; accept connectors in any orientation; add the base tools and the conversion settings dialog. The book's counts are Network up to 6 solutions, Perimeter 5, Intersection up to 8; the pipe chapter's own example shows 5 for Network, so the count is complexity-dependent and should not be hard-coded. | `crates/document/src/layout.rs:26-33,121-140`, `apps/desktop/src/systems_ui.rs` | mep-part2 8, mep-part3 A10 |
| **C9** | **Duct and pipe types do not exist.** No type selector, no routing preferences, no round or oval duct, no flex duct, no segment or material, so roughness and fitting selection cannot vary by service. | Add type objects with routing preferences, and let the tool pick fittings from them. | `crates/family`, `apps/desktop/src/mep_tool.rs` | mep-part2 9, mep-part3 A2-A3 |
| **C10** | **Vertical runs cannot be drawn.** All points of a run share one z, so there is no riser and no drop, and shafts are useless for services. | Allow a per-point elevation, and insert transitions and vertical elbows when the elevation changes. | `apps/desktop/src/mep_tool.rs:~164`, `crates/document/src/lib.rs` `Geometry::Linear` | mep-part2 9 |
| **C11** | **Duct and pipe clicks snap to a 50 mm grid only**, not to connectors, walls or grid intersections, and there are no angle snaps. | Reuse the shared plan snap for the MEP tools. | `apps/desktop/src/model_view.rs:942-956`, `mep_tool.rs:167,198,221` | mep-part2 9 |
| **C12** | **Sizing offers one velocity/friction-limit tree.** No Equal Friction, no Static Regain, no velocity reduction, and no choice of friction method beyond the implicit Colebrook. | Add the four methods named in the book, and expose the Colebrook / Haaland choice from the Mechanical Settings. | `crates/calc/src/tree.rs`, `apps/desktop/src/compute_ui.rs` | mep-part2 10, mep-part3 A4 |
| **C13** | **No System Inspector, no Check Duct Systems, no Duct Legend.** | Add the three: a per-section flow inspector, a disconnect report, and a colour-fill legend. | `apps/desktop/src/systems_ui.rs`, `crates/calc` | mep-part2 7 |
| **C14** | **Moving equipment does not drag connected runs**, and moving a joined duct is refused outright. | Move connected runs with their equipment; allow a joined run to be moved by translating the whole network. | `apps/desktop/src/edit.rs:141`, `compute_ui.rs:197,217` | mep-part2 (behaviour) |
| **C15** | **Plumbing fixtures do not exist**, so there is no fixture-unit data, no trap and no vent logic, and sanitary and vent are computed as pressurised flow. `Route::check_fall` exists and is unused. | Add fixture families with WFU / HWFU / CWFU type parameters and connectors, and use them to drive the loading-unit conversion. Add the minimum-slope check and the gravity rules. | `crates/family/src/library.rs`, `crates/calc/src/plumbing.rs`, `crates/route/src/lib.rs` | mep-part3 A1, A9 |
| **C16** | **No sprinkler heads can be placed.** Sprinklers are abstract end points with a K-factor; there is no design area, so every head with a demand operates, and there is no wet or dry system. | Add pendent and upright head families, a design-area concept, and the wet/dry system attribute. Keep the book's rule: pipe above pendent heads and below upright heads. | `crates/family/src/library.rs`, `crates/calc/src/sprinkler.rs`, `crates/document/src/mep.rs` | mep-part3 B1-B4 |
| **C17** | **Electrical is an abstract single-line tree.** "Electrical Node" and "Cable" are records in `MepData`, not elements: no Select, no Properties, no 3D, no schedule, no IFC entity. There is no circuit object (only `grouped_circuits` on the cable, which is a grouping flag, not a circuit), no panel schedule, no cable tray or conduit, and each node accepts exactly one feeder. | Make panels, devices and luminaires elements with families; add circuits (Create Circuit, Select Panel, Edit Circuit); add panel schedules; add cable tray and conduit tools. The cable tree then reads from the circuits. | `crates/document/src/elec.rs`, `crates/family`, `crates/ifc` | mep-part2 12 |
| **C18** | **Hangers and pipe/duct supports do not exist.** They are the single biggest hole in the MEP-support story the owner reduced structure to. | Treat them as MEP families (C3), hosted on beams, slabs or trusses, with auto-placement along a run. | `crates/family`, `apps/desktop/src/mep_tool.rs` | structure-for-MEP scope |

**Class 3 acceptance:** the reference's own acceptance checklists, converted to tests one feature at
a time, pass. Each test name carries the reference section id.

### 3.2 Architecture and documentation

| ID | Gap | Remediation | Lands in | Reference |
|---|---|---|---|---|
| **C19** | **Dimensions are aligned-only and not associative**, so they do not follow the element and cannot be converted from a temporary dimension. | Make the dimension reference two element faces or centres, and add the conversion symbol. | `apps/desktop/src/model_view.rs`, `crates/document` | arch-part3 2.11-2.16 |
| **C20** | **No text notes, callouts, drafting views, detail lines or filled regions.** | Add the annotation set. | `apps/desktop/src/`, `crates/document` | arch-part3 3.1-3.5 |
| **C21** | **Floors have no Pick Walls, no slope and no shape editing** (Add Point, Add Split Line, Modify Sub Elements). | Add boundary editing after placement. | `apps/desktop/src/model_edit.rs:399-405`, `crates/document/src/lib.rs:803` | arch-part2 3.2 |
| **C22** | **Roofs are footprint-only**: no extrusion roof, no overhang, no attach-to-wall, no eave or ridge editing. | Add the extrusion roof and wall attachment. | `crates/document/src/roof.rs` | arch-part2 3.2 |
| **C23** | **Walls have no top constraint, no attach, no layer priority and no wrapping.** The reference is explicit that layer Priority and Default Wrapping - not a join-type choice - govern the corner. | Add the top constraint and attach first, then layer priority and wrapping. | `crates/arch`, `crates/document/src/layers.rs:237-242` | arch-part1 4.1, arch-part2 |
| **C24** | **No ceiling plan views**, and the automatic ceiling outline is copied at placement and does not follow the walls. | Add ceiling plan views and make the automatic ceiling associative. | `apps/desktop/src/panels.rs:945`, `model_edit.rs:460-474` | arch-part2 3.2, mep-part2 5 |
| **C25** | **No Project Units dialog**; units are fixed metric and appear only on the Compute dialog. | Add the units dialog by discipline, with rounding and format. | `apps/desktop/src/`, `crates/document` | mep-part1 3.2 |
| **C26** | **No reference planes, work planes or scope boxes.** | Add the three. | `crates/document`, `apps/desktop/src/` | arch-part2 3.1a |
| **C27** | **Doors and windows: no flip arrows, no Tag on Placement, no swing angle in plan, no family loading, and a door can overhang a wall end without a warning.** | Add the interaction, and clamp the position so the opening fits the wall. | `apps/desktop/src/model_edit.rs:572-574`, `crates/family` | arch-part1 4.1 |
| **C28** | **Grids cannot be renamed, cannot be drawn as arcs or multi-segment, and have no 2D/3D extents.** | Add rename, arc and multi-segment, and the extent property. | `apps/desktop/src/model_edit.rs:998-1005`, `crates/document/src/lib.rs:~678` | arch-part2 3.1 |
| **C29** | **No clash or interference check.** | Start with an in-model check; the linked-model version waits on the collaboration design (class 4). | `apps/desktop/src/`, `crates/geom` | mep-part1 (Navisworks only in the book) |

---

## 4. Class 4 - later stage

These need a design decision, are tied to the collaboration design, or were deferred by the owner.
They are listed so the register is complete; they are not scheduled.

| ID | Gap | Why later |
|---|---|---|
| **D1** | **Massing** (in-place mass, conceptual mass, mass floors, Building Maker). | Needs a form-making geometry kernel. The book review already marks it P3. |
| **D2** | **Toposolid and site.** | Large; the Architecture book's site chapter is thin and the owner's scope is a building, not a site. |
| **D3** | **Curved, stacked, slanted and tapered walls; sweeps and reveals.** | Curves are currently straight facets; a true curved wall needs a geometry change. |
| **D4** | **Curtain walls.** | Placeholder only; needs a mullion/panel model. |
| **D5** | **Stair by sketch, winder, landings, ramps.** | Stair by component exists; the sketched stair is a separate model. |
| **D6** | **Link model, copy/monitor, worksets, phases, design options.** | Tied to the collaboration design (COLLAB-001..003). An interim read-only import layer is the only part that could be pulled forward, as a class 2 row. |
| **D7** | **Fabrication parts.** | Needs the Revit add-in and its content; out of scope for a native app. |
| **D8** | **Family editor.** | The engine exists; the editor is a large UI build. |
| **D9** | **RVT export.** | Out of scope by decision: IFC4 is the exchange format. |
| **D10** | **Structure stage** (structural walls, framing, connections, rebar beyond beams, analytical model, loads, piles). | Deferred by the owner on 2026-10-21. |
| **D11** | **Rendering, walkthrough, solar study, point cloud.** | The Architecture book's ch. 15-16; no BIM-authoring dependency. |
| **D12** | **Undo memory and per-click cost.** Each step clones the whole model with no cap and revalidates every element. | PERF-001 is open; measure before changing. |
| **D13** | **Semantic IFC import** of doors, windows, openings, fittings, equipment, ports and spaces from foreign files. | Extruded-profile import exists; the semantic reader is a project of its own. The owner's rule is to log what could not be read rather than fail the file. |

---

## 5. Remediation order

Each step is a self-contained, reviewed change. The gate for a step is its acceptance criterion, not
the completion of the previous step - but step 0 must land first, because nothing else can be tested
through the real UI until the canvas responds.

**Step 0 - Walking skeleton (class 2.1 + the end-to-end test).** Fix B1-B6, then add the
end-to-end test: new project -> three levels -> grids -> typed walls with layers -> floor -> door and
window -> stair -> roof -> ceiling -> room -> section -> save/reopen -> export IFC with counts, all
through `Desktop::ui` events. This needs one refactor: extract the body of `Desktop::ui` (which
currently takes `&mut eframe::Frame`) into `fn ui_body(&mut self, ui)`, and add a plan
model-to-screen transform so the harness can place clicks deliberately.

**Step 1 - Exchange honesty (class 2.2).** B7-B11, B14, B15, B19. The acceptance is an IFC that
carries what the app already knows.

**Step 2 - Persistence and settings (rest of class 2).** B12, B13, B16-B18, B20-B25. The acceptance
is a save/reopen round-trip that preserves Compute inputs, rooms, tags and the last result.

**Step 3 - MEP content (class 3.1, C1-C5).** The family loader, editable type parameters, the first
library, terminals and hosting. Everything from C6 onward depends on this.

**Step 4 - Spaces, flow and systems (C6-C8, C12, C13).** Space and zone with airflow data, terminals
with Flow, named systems that include terminals and respect connector flow direction, Generate Layout
with the missing solution types and conversion settings, and the four sizing methods.

**Step 5 - Routing completeness (C9-C11, C14, C15, C16).** Vertical runs, types with routing
preferences, connector snapping, moving equipment drags runs, the minimum-slope check, gravity
drainage, and sprinkler heads.

**Step 6 - Electrical as objects (C17).** Panels, devices and luminaires as elements, circuits, panel
schedules, cable tray and conduit; the cable tree reads from circuits.

**Step 7 - Coordination and annotation (C29, then class 3.2).** In-model clash check, then the
architecture and documentation rows.

**Later (class 4).** As listed in section 4.

---

## 6. Open decisions for the owner

These change priorities and cannot be settled by the implementer.

1. **Storage model.** The agreed decision is operation log + snapshots; the app writes a full text
   snapshot and `crates/oplog` is unused. Is the log still wanted? If yes, autosave and recovery
   (B20) is its first use. If no, the decision should be corrected so the backlog stops promising it.
2. **Ribbon honesty now or later.** Show unbuilt buttons disabled as `01-ui-specification.md` says
   (B17), or keep the "not implemented" message while testing? Disabled is cheaper and matches the
   design doc.
3. **Which MEP families first for C3?** Proposed order: AHU, ceiling diffuser, VAV, pump, valve,
   WC/basin, sprinkler head, luminaire, panel board, hanger/support.
4. ~~**Electrical calculations.**~~ **Decided 2026-10-21: option B.** The decision stands:
   `06-mep-schematic-automation.md` decision 2 wins, electrical calculations are imported from
   edesign-alike and are **not** re-implemented in this repository. Consequences, now row **B26**:
   `crates/calc/src/electrical.rs` (~988 lines) and `crates/document/src/compute_elec.rs` are
   marked deprecated and removed from the desktop path in a follow-up, and edesign-alike's
   electrical crates are pinned by git SHA instead. This also matches the project rule that there
   is no duplicate development across the two repositories.
5. **Test building for the end-to-end tests.** The office was chosen on 14 Oct. Should the MEP test
   use a data hall instead?
6. **Does the reference branch merge now?** R1 is a docs-only PR and is the cheapest way to make
   every class 2 and class 3 row citable from `main`.

---

## 7. Agents for this work

The repository already defines five persistent profiles (`docs/agent-team/README.md`):
`bim-research` (study the reference and the current implementation), `bim-rust` (implement approved
scope), `bim-review` (independent inspection), `bim-ux` (exercise the real app against reference
workflows), `bim-perf` (measure compilation and runtime scaling). The gating sequence is research ->
user-approved bounded task -> implementation -> independent review + UX + performance -> coordinator
decision -> user-approved merge/push.

Two roles are missing for this phase, and are proposed in `docs/agent-team/roles/`:

| Role | Responsibility | Deliverable |
|---|---|---|
| `bim-reference` | Own the book-to-reference extraction: keep `docs/reference/revit-books/` in step with the books, keep the `[book]` / `[knowledge]` marks honest, and re-verify the citations in the gap analysis against a pinned SHA. | Reference docs, an extraction manifest, and a citation re-check report. |
| `bim-gap` | Own the gap register: one row per gap, each with a reference citation, a status, a remediation and an acceptance test, and a rule that a row cannot be closed without the test that closes it. | The register, kept current as steps land. |

The register above is the seed for `bim-gap`. Its rules, which the role must enforce:

- A gap row is **open** only if it has a reference citation and an acceptance test, or an explicit
  statement of why no test is possible.
- A row moves to **closed** only when the test that closes it is in the tree at a named SHA.
- `[knowledge]` reference statements cannot close a safety-relevant row without an independent
  confirmation.
- No row may be added for a class 4 item with a scheduled date.

---

## 8. Verification appendix

### 8.1 What was verified for this document

- The reference docs' factual claims were spot-checked against the source books. Five claims were
  checked and all five held: Generate Layout solution counts (Network up to six solutions, Perimeter
  five, Intersection up to eight, each with the book's own wording), the sprinkler coverage rule (one
  head per 130 sq ft, i.e. per 12 m2, and the schedule formula `Area/130` / `Area/12`), the
  colour-scheme "By value / By range" distinction, and the zone perimeter rule (shared edges of
  adjacent spaces count once). The reference doc's own note that the pipe chapter's Network example
  shows five solutions while the duct chapter says up to six was confirmed as the book's, not the
  reference's, inconsistency.
- The workspace has 20 members (19 crates plus `apps/desktop`); `README.md` lists 7.
- The branch `docs/revit-book-references` is 7 commits ahead of `origin/main`, docs only, +4,304
  lines, and has no pull request.

### 8.2 Independent re-check of the gap analysis citations

Four review agents re-checked the `file:line` citations in `docs/gap-analysis-2026-10-21.md` and its
three companion audits against the pinned SHA `dcf8382`, reading code only. Their reports are
committed beside this document:

| Report | Scope | Result |
|---|---|---|
| `docs/audits/verify-ui-shell.md` | A1-A6 shell blockers, ribbon wiring, keymap, save, undo | 6 of 6 CONFIRMED; two precision notes below |
| `docs/audits/verify-mep-engine.md` | M1-M10 MEP content, systems, layout, compute, electrical | 8 of 10 CONFIRMED, 2 APPROXIMATE (one wrong path, one wrong sub-claim) |
| `docs/audits/verify-ifc.md` | I1-I9 IFC export/import, GlobalIds, persistence, undo | 9 of 9 CONFIRMED; two nuances below |
| `docs/audits/verify-docs-decisions.md` | C1-C10 stale documents and agreed decisions | 9 of 10 CONFIRMED, 1 APPROXIMATE |

**Corrections carried into the register.** The substance of the gap analysis holds; five citations
were wrong and are corrected in the rows above:

1. **Generate Layout lives in `crates/document/src/layout.rs` (398 lines), not `apps/desktop/src/layout.rs`.**
   `apps/desktop/src/layout.rs` does not exist at this SHA. Corrected in **C7** and **C8**; the
   constraint lines are `crates/document/src/layout.rs:26-33` (solution kinds), `:121-124`
   (connectors must face along a plan axis and sit at the base connector's height) and `:138-140`.
2. **The electrical "grep `circuit`" sub-claim is wrong.** `crates/document/src/elec.rs` does contain
   circuit-named code; what is missing is a *circuit object* - the only thing that exists is
   `grouped_circuits`, a grouping flag on `ElecCable`. Corrected in **C17**. The substance (no
   circuit, no panel schedule, one feeder per node) is confirmed.
3. **The `05-backlog.md` MEP-002 status is not what the agent-team note says.** The backlog row
   (`05-backlog.md:24`) carries no status column; the "PLANNED/UNVERIFIED" wording is in
   `docs/agent-team/gap-review.md`. Corrected in **R5**.
4. **Level rename is not a missing capability.** The model-level rename exists
   (`crates/document/src/lib.rs`); it is simply never surfaced for levels, and levels cannot be
   deleted. Corrected in **B6**.
5. **`IfcRelAssignsToGroupByFactor` does not exist in the tree.** `IfcRelAssignsToGroup` is used, and
   only for MEP pieces. Corrected in **B9**.

**Resolution of the one real conflict (C7 in the docs-decisions report).** The report confirms the
conflict: `06-mep-schematic-automation.md:10` says revit-alike does not reimplement electrical
calculations, while `crates/calc/src/electrical.rs` is a full IEC 60364 engine and the repository
contains zero references to edesign-alike. The owner decided on 2026-10-21 that **the decision stands
and the code gives way** (option B), which is also what the standing project rule requires. The
remediation is row **B26**.

**Two nuances that do not change a row:**

- **B4 / A4**: no `Key::S`, `Key::N` or `Key::O` handler exists; the `save_as` arm is at
  `main.rs:476`; the keymap is consulted only when it is built (`main.rs:268`) and printed
  (`main.rs:657`). The claim holds as written.
- **B11 / I4**: the running counter for non-product GlobalIds is *stable across unchanged exports*;
  it shifts only when the set of non-element entities changes. The instability is real but narrower
  than "adding a level changes them" - a level changes the set, so the claim survives.

### 8.2.1 Test result at the pinned SHA

`cargo test --workspace --release` was run at `dcf8382` with a clean `CARGO_TARGET_DIR` and
`--jobs=2`; exit code 0. Result: **1,140 passed, 0 failed, 2 ignored** across 19 workspace members.
The `#[test]` attribute count at the same SHA is 1,142 by grep (the difference is tests that are
`#[ignore]`d or in files with no test binary of their own). The raw log is committed beside this
document as `docs/audits/2026-10-21-test-run-dcf8382.log`. This confirms the figure the companion
gap analysis reported from its own run.

### 8.3 Size and measuring convention

Line counts below are `wc -l` over `*.rs` files, excluding any `target/` directory, at
`dcf8382`. `apps/desktop` is counted separately from the crates. The figures are a measuring
convention for this document, not a project metric; a reviewer using a different convention will
get different totals and should say so.

| Where | Lines |
|---|---|
| `apps/desktop` | 18,531 |
| `crates/family` | 17,018 |
| `crates/document` | 12,868 |
| `crates/ifc` | 8,397 |
| `crates/view` | 5,944 |
| `crates/route` | 5,546 |
| `crates/sheets` | 4,607 |
| `crates/place` | 2,865 |
| `crates/calc` | 1,996 |
| `crates/arch` | 1,631 |
| `crates/automation` | 1,485 |
| `crates/geom` | 1,481 |
| `crates/photo` | 1,254 |
| `crates/oplog` | 1,182 |
| `crates/viewport-egui` | 1,139 |
| `crates/sketch` | 1,069 |
| `crates/viewport` | 985 |
| `crates/keymap` | 924 |
| `crates/schematic` | 410 |
| **Total** | **~94,000** |

The test count is 1,142 `#[test]` attributes across `crates/` and `apps/` at the same SHA, by grep
count. The suite result at that SHA is recorded in section 8.1 of the companion gap analysis and is
re-measured here only as a baseline, not as a gate.

### 8.5 Verified line references, per row

These are the real lines the four verification agents read at `dcf8382`. They supersede any
`file:line` in the companion gap analysis where the two differ.

| Row | Verified reference |
|---|---|
| **B1** | `apps/desktop/src/model_view.rs:143` (`if !bounds.is_finite() { return; }`); first tool branch `:482`; bounds written only at `:132` (plan meshes) and `:140` (grid datum); the only Level arm is `:220` and is gated on `three_d`. 3D is unaffected - its return is at `:88`. |
| **B2** | `model_view.rs:68` filters levels by `ElementId(10)`/`(11)`; `:72` hard-codes `("Level 1", View::Plan)` / `("Level 2", View::UpperPlan)`; `:74` hard-codes `"M-101 - Mechanical"`; `model_edit.rs:316`; `panels.rs:931,943`; `plan_export.rs:76-78`; export failure at `export.rs:123`. Ids 10/11 are minted only by `Document::spatial_sample()` (`crates/document/src/spatial.rs:654-655`); `Document::new()` has neither. |
| **B3** | `apps/desktop/src/main.rs:567`, the first statement of `fn ui` (`:565`, `impl eframe::App` at `:564`), before the ribbon (`:589`) and viewport (`:590`), with no `egui_wants_keyboard_input` guard. The two-stage contract the tools implement is at `model_view.rs:563`. No test reaches `Desktop::ui` - `main.rs` has no `mod tests`. |
| **B4** | No `Key::S`, `Key::N` or `Key::O` anywhere in `apps/desktop/src/`. Handled keys are Escape `:567`, Tab `:568`, Ctrl/Cmd+Y\|Z `:578`, Ctrl+Z `:580`, Ctrl+P `:584`. The `save_as` arm at `main.rs:476` is the sole occurrence of that id in the repository, so it is unreachable; Save itself is the icon button `ribbon.rs:49`. The keymap is built at `main.rs:268` and used only for printing at `main.rs:657`; `Matcher::feed` / `command_for` (`crates/keymap/src/lib.rs:366,251`) are never called. |
| **B5** | `main.rs:458` sets `slab_offset_mm = 2700.0`, `slab_thickness_mm = 20.0` for Ceiling; `:459` Roof 200/0; `:462` Door, `:463` Window, `:464` Component reset only `family_*` and never the pair; `model_edit.rs:568` reads `elevation_m = self.slab_offset_mm / 1000.0`. Other consumers of the same fields: `model_edit.rs:465,470,526` (Ceiling), `:533` (Roof), `:541` (Slab), `:379` (Footing), `:390` (WallFooting); exposed in `panels.rs:456,468,483,641,645`. `choose_tool` (`main.rs:303-318`) clears `slab_points` but not the pair. |
| **B6** | `crates/document/src/lib.rs:689-702` `place_level` checks only finite elevation and a 1e-3 m tolerance, and takes an explicit name verbatim with no uniqueness check. `crates/arch/src/lib.rs:18` `MIN_LEVEL_GAP = 0.1` and `:82-92` reject gaps under 0.1 m and duplicate names; `export.rs:289` turns that into an export failure. Deletion is blocked at `model_edit.rs:870`. `Command::Rename` exists (`lib.rs:524`) but no `Command::Rename` is issued anywhere in `apps/desktop/src`. |
| **B7** | `export.rs:205,207` build every run with `System::Other`; only `:322` copies a system from a joined fitting; zero hits for `run_system` in `export.rs`; the comment at `:203` is stale - `MepData.run_systems` exists at `crates/document/src/mep.rs:189`. |
| **B8** | `export.rs:198` drops Level/Grid/Tag/Dimension/Room/Opening/Section/Elevation/BeamSystem/Truss/Rebar; `:291` `rooms: vec![]`; `:292` `georef: None`; `:432` warns the site is not exported. The crate can do it: `crates/ifc/src/lib.rs:964` calls `write_spaces` (`:1497-1556`, `IFCSPACE` + `Qto_SpaceBaseQuantities`), boundaries at `:1553-1555,1576`; tests `tests/ifc.rs:258-277`, `tests/boundaries.rs:191-196`. |
| **B9** | `MepSystemDef` (`crates/document/src/mep.rs:105,178`) is never read by the exporter; `crates/ifc/src/lib.rs:1706,1739,1781` key `by_system` on `System::code()`; `:1831-1838` writes one `IFCDISTRIBUTIONSYSTEM` per code named `"<System name> N"`; `IFCRELASSIGNSTOGROUP` at `:1837` is the only hit (there is no `...BYFACTOR`); equipment (`:1798-1801`) is never assigned to a system. |
| **B10** | `crates/place/src/penetrations.rs` exists (`find` `:279`, `register_csv` `:381`) but `export.rs:290-291` hard-codes empty lists and no file under `apps/` calls the engine. |
| **B11** | Product ids are stable via `gid()` (`crates/ifc/src/lib.rs:638-643`, `guid.rs:14-15`); storeys, relationships, property sets, quantities and systems use the `aux` counter `gen_gid` (`lib.rs:602,645-648`; callers `:751,756,772,774,923,933,935,937,938,942,948`). `next_id` reuses a deleted maximum (`crates/document/src/lib.rs:571`). `apps/desktop/src/model_edit.rs:305` pins `"Project1"` and key 1. |
| **B12** | `apps/desktop/src/file.rs:59-66` writes the `.obim` atomically via a `.saving` temp file and rename; `crates/document/src/native.rs:130-136` are text records; zero hits for autosave, backup, recovery or crash; `crates/oplog/Cargo.toml:2` exists with zero dependants anywhere. |
| **B13** | The size catalogue is session-only dialog text (default empty, `compute_ui.rs:70`); `NoSize` at `crates/calc/src/tree.rs:228`; the MEP Settings lists are not read by Compute. |
| **B14** | `compute_ui.rs:132` clears `equip_links` before adding the end point; only kind 4 (`:144`) restores the join. |
| **B15** | The Air preset leaves `max_velocity` at 1.5 (`compute_ui.rs:69-70`); the fluid preset sets density/viscosity/roughness only. |
| **B17** | The placeholder arm is `main.rs:495`. Wired vs placeholder: HVAC 21/8 wired/13 placeholder, Plumbing 22/7/15, Electrical 20/3/17; 63 buttons, 18 wired, 45 placeholder. |
| **B18** | `crates/document/src/compute.rs:156-157` filters by the `System` enum; `:201` raises `MoreThanOneSource`. |
| **B19** | `export.rs:198` skips `BeamSystem` and `Truss`, so members leave as plain `IFCBEAM` / `IFCMEMBER` with no `IfcRelAggregates` parent. |
| **B21** | `crates/document/src/rooms.rs`; room name is generated (`"Room N"`); no Number/Name/Department instance parameters. |
| **B26** | `crates/calc/src/electrical.rs` (~988 lines) and `crates/document/src/compute_elec.rs`; `06-mep-schematic-automation.md:10` says the opposite; zero references to edesign-alike in the tree. |
| **C1** | `crates/family/src/catalogue.rs` exists; no caller of `Catalogue::from_text` or any family loader under `apps/`; `load_family` is only an icon id (`icons.rs`). |
| **C2** | `export.rs:57-65` filters `Scope::Instance`; the fan coil's values are all Type scope (`library.rs:327-334,441-442`); `library.rs:309-311` compiles exactly five families; `main.rs:464` maps both `"ahu"` and `"component"` to `"Fan coil unit, ducted"`. |
| **C3** | The library holds 19 families: handle, door, window, arched door, fan coil, air handling unit, ceiling diffuser, VAV terminal, pump, valve, WC, washbasin, sprinkler head, luminaire, panel board, pipe/duct hanger, pipe elbow, pipe reducer, pipe tee (`crates/family/src/library.rs:481-503`). |
| **C7** | `crates/document/src/mep.rs:105-111,332`; `crates/document/src/layout.rs:121-140` - flow direction is read only at `apps/desktop/src/systems_ui.rs:168`, to pick the base. |
| **C8** | `crates/document/src/layout.rs` (398 lines): solution kinds at `:26-33`, the axis/height constraint at `:121-124`, only NetworkAlongX/Y + Routed at `:138-143`. |
| **C10** | `apps/desktop/src/mep_tool.rs` - every point of a run shares one z. |
| **C13** | `crates/calc/src/tree.rs:228`; the four sizing methods are not selectable. |
| **C16** | `crates/calc/src/sprinkler.rs`; one `System::FireProtection` value; every end point with a demand operates. |
| **C17** | `crates/document/src/elec.rs:20-38` (`ElecNode`), `:47-63` (`ElecCable`), `:82-91` (one-feeder rule); stored in `MepData` (`crates/document/src/mep.rs:194-196`). |
| **C26** | Reference planes, work planes and scope boxes: absent from the tree. |
| **D10** | The structure scope the owner deferred is recorded in `docs/reference/README.md:14`. |

### 8.4 What was not verified

- The desktop app was not run interactively. Every statement about runtime behaviour is from code
  reading plus the test suite.
- IFC validity was not checked with an independent reader; CI runs IfcOpenShell scripts under
  `spikes/`, which were not inspected.
- No side-by-side comparison with a running Revit was made, so "matches Revit" means "matches the
  book".
