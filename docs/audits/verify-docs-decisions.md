# Documentation and agreed-decision verification

> **Scope.** Docs in `docs/` and `docs/revit-style-ui/` checked against the code at `6ac8addc9550fcbed3d2b5cce868a540fe0a0780` (detached HEAD of the `revit-book-refs` worktree, base `main` `dcf8382`).
> **Method.** Code reading only (`sed`/`grep`/`git ls-tree`/`git show`). **No cargo was run** (another process is building); no file was edited, checked out, committed or pushed. Nothing below is a test result — every line is a quoted source line at the pinned SHA.
> **Verdict vocabulary.** CONFIRMED = the claim is true as written; WRONG = the claim is false as written; APPROXIMATE = the claim is true in substance but wrong in detail (line, number, or qualifier).

---

## C1 — ribbon tabs "not built yet are shown disabled"

**Verdict: CONFIRMED.**

**Real lines.**
- Doc: `docs/revit-style-ui/01-ui-specification.md:20`
- Code: `apps/desktop/src/main.rs:495` (fallback arm of `Desktop::run_command`)
- Code: `apps/desktop/src/ribbon.rs:22-37` (`pub const TABS` — 14 tabs), `ribbon.rs:292` (`pub fn big_button`)

**Quoted text.**
- Doc: `- All 14 ribbon tabs stay visible; tabs and tools not built yet are shown disabled.`
- Code: `_ => self.message = format!("{label} is not implemented in this UI prototype"),`

**What the code shows.** All 14 tabs exist and are all clickable; `ribbon::big_button` has no `enabled`/`disabled` parameter and no `Sense`/`ui.set_enabled` gate — `grep -i "disabled\|\.enabled(" apps/desktop/src/ribbon.rs` returns **nothing**. Every ribbon command id is routed to `run_command`, and ids with no arm (e.g. `curtain_wall`, `model_line`, `area`, `ramp`, `panel`, `transformer`, `generator`, `disconnect`, `brace_frame`, `framing` — all present in `ribbon.rs` groups) fall through to the message at line 495. So unbuilt tools are **enabled and clickable**, producing "X is not implemented in this UI prototype".

**Impact.** The spec sentence is not implemented. Users can click any unbuilt tool and get a status message instead of a greyed-out control. Fix the doc (state tools are enabled-but-unimplemented) or add a `disabled` flag to `big_button` + the ribbon table.

---

## C2 — "Verified by automated event tests (simulated mouse and keys)"

**Verdict: CONFIRMED (as written), but the second half of the claim is unverifiable and partly contradicted by the audit's own finding.**

**Real lines.**
- Doc: `docs/revit-ux-parity-audit.md:3`
- Code: `apps/desktop/src/main.rs:75` (`struct Desktop`), `main.rs:565` (`impl eframe::App for Desktop { fn ui(...) }`), `main.rs:589` (`self.ribbon(ui);`), `main.rs:540` (`panels::options(self, ui);`)

**Quoted text.**
- Doc: `Standing rule: every tool mimics Revit's user experience. Verified by automated event tests (simulated mouse and keys) and by reading the code. Not verified side by side with Revit, and not clicked by hand.`
- Code: `apps/desktop/src/main.rs:565`: `fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {`

**What the code shows.** `Desktop` is a **private** struct (`struct Desktop {` at `main.rs:75`, no `pub`), and `fn ui` is an `eframe::App` trait method taking `&mut eframe::Frame` — it cannot be constructed from a test. `grep -rn "Desktop::ui" apps/` → **0 hits**; `grep -rn "Desktop::ribbon\|app.ribbon(" apps/desktop/src/*.rs` → only the production call at `main.rs:589`. All 169 `#[test]`s in `apps/desktop/src` call `model_view::show` / `view3d::show` / `panels::options` sub-functions or direct model methods. The repo's own audit states this explicitly: `docs/audits/2026-10-21-arch-audit.md:221`: "It exercises `model_view::show` only (viewport + view tabs + control bar). It does not run `Desktop::ui` (main.rs:565), the ribbon (`Desktop::ribbon`), the options bar (`panels::options`), the side panels, or the global Esc/Tab/undo shortcuts." Global keys (Esc at `main.rs:567`, Tab at `:568`, Ctrl+Y/Z at `:578-582`, Ctrl+P at `:584`) live **only** in `fn ui`, so no test drives them.

**Impact.** The header overstates coverage: no test drives the ribbon, the options bar, `Desktop::ui` or the global key handlers. Either reword the header ("verified by direct-call and viewport-harness tests; `Desktop::ui` itself is not test-driven") or add the refactor the audit asks for (`main.rs:280` suggests extracting `fn ui_body`).

---

## C3 — 3D plan: "every drawing tool is plan-only" and "there is no redo (known)"

**Verdict: CONFIRMED.**

**Real lines.**
- Doc: `docs/3d-view-parity-plan.md:21` and `:36`; status line `:3`
- Code: `apps/desktop/src/view3d.rs:237-246` (3D click placement), `crates/document/src/lib.rs:1288` (`pub fn redo`), `apps/desktop/src/main.rs:547` (`fn redo_document`), `main.rs:578-579` (Ctrl+Y binding)

**Quoted text.**
- Doc line 21: `| Not available in 3D | Every drawing and modify tool (Wall, Move, Rotate, Mirror, Copy, Align, Trim, Offset, Dimension, Tag, Room, Stair, Shaft...) is plan-only; picking in 3D selects but cannot move |`
- Doc line 36: `| Undo / Redo | The document's undo works from 3D; there is no redo (known) | Redo is a separate missing item for the whole app |`
- Doc line 3: `... This is a plan, not built work: nothing here has been started.`
- Code `view3d.rs:237-246`: `if app.tool == crate::Tool::Family { // a click in 3D places the family: a door or window goes into the wall under the click, anything else stands // on the level of what it was dropped on. ... app.family_click(level, [hit.point[0], hit.point[1]], wall); }`
- Code `lib.rs:1288`: `pub fn redo(&mut self) -> Result<bool, EditError> {` — with the doc comment at `:1286`: `/// Put back what the last undo took away (Ctrl+Y).`
- Code `main.rs:578`: `let redo = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))));`

**What the code shows.** (a) 3D click **does** place elements: Door/Window/Component via `Tool::Family` + `view3d.last_hit` → `family_click` (a test at `view3d.rs:288` `a_click_in_the_three_d_view_places_a_door_in_the_wall_under_it` injects the hit). The doc's "every drawing and modify tool … is plan-only" is therefore false for family placement; it remains true for Wall/Beam/Column/Duct/Pipe. (b) Redo **exists** at the document layer (`lib.rs:1288`), is wired in the app (`main.rs:547`), and is bound to Ctrl+Y / Ctrl+Shift+Z (`main.rs:578`). The doc's "there is no redo (known)" is false.

**Impact.** The plan's two factual premises are stale (3D family placement, redo). Its status line "nothing here has been started" is false. Update lines 3, 21 and 36 before anyone re-plans from it.

---

## C4 — gap-review: "GUI-001 prototype on branch; main does not include the desktop app" + "ARCH-001 PARTIAL"

**Verdict: CONFIRMED.**

**Real lines.**
- Doc: `docs/agent-team/gap-review.md:26` (GUI-001), `:35` (ARCH-001), `:9` (finding), `:69` (method)
- Code: `git ls-tree --name-only main apps/` → `apps/desktop`; `Cargo.toml:3` (`"apps/desktop"` is a member)

**Quoted text.**
- `| GUI-001 | P0 | egui desktop shell (decided 15 Oct 2026; prototype on `feature/rust-gui`) | PROTOTYPE ON BRANCH: origin/feature/rust-gui contains apps/desktop and an expanded workspace; main does not include the desktop app. No GUI build or interaction run performed in this review. |`
- `| ARCH-001 | P1 | Full architectural authoring workflows | PARTIAL: main has walls, placement, rooms and arch primitives; complete architectural authoring workflows remain staged targets. |`
- Finding at `:9`: `- Do not start a second GUI: `origin/feature/rust-gui` already has a desktop prototype, document code and automation/MCP work.`

**What the code shows.** `main` **does** include `apps/desktop` (34 `.rs` files, **18,502 lines** of Rust under `apps/desktop/src` at `main`; 18,531 lines / 34 files at the pinned SHA `6ac8add`). `ARCH-001 PARTIAL` is **still accurate** — the `apps/desktop/src/main.rs:495` fallback and the empty-canvas blocker (`model_view.rs:143` `if !bounds.is_finite() { return; }`, confirmed at `docs/audits/2026-10-21-arch-audit.md:13`) mean full architectural authoring is not delivered. The repo's own audit already flags the GUI-001 row: `docs/audits/2026-10-21-arch-audit.md:296`: `| D9 | docs/agent-team/gap-review.md "ARCH-001 PARTIAL", "GUI-001 prototype on branch; main does not include the desktop app" | Out of date: main now contains apps/desktop (18.5k lines in src) |`

**Impact.** `gap-review.md:26` and `:9` are factually wrong and still steer a reader away from the desktop app that now exists on main. `ARCH-001 PARTIAL` needs no change. The BIM-004 row (`:24`, "PARTIAL / UNVERIFIED") is half-stale too (save/open exists, recovery does not).

---

## C5 — 06 says the schematic engine slice is on `feature/sch-001…`, "not part of this candidate"

**Verdict: CONFIRMED — the claim is stale.**

**Real lines.**
- Doc: `docs/revit-style-ui/06-mep-schematic-automation.md:3` (status), `:5` (implementation status)
- Code: `crates/schematic/src/lib.rs` (14 lines), `crates/schematic/src/layout.rs` (396 lines), `crates/document/src/schematic.rs` (839 lines), `crates/document/src/schematic_view.rs` (296 lines), `crates/document/src/lib.rs:530` (`GenerateSchematic { diagram: Box<SchematicView> }`), `lib.rs:1239` (apply), `schematic_view.rs:195` (generator), `schematic_view.rs:202` (redraw)

**Quoted text.**
- Doc `:3`: `Status: **proposed 20 Oct 2026; user decisions recorded below; not implemented.**`
- Doc `:5`: `Implementation status of this proposal: nothing in this document is implemented or accepted. A first engine slice — the zone data model, the assignment rules and the extraction query with its JSON dump — exists on the separate local branch `feature/sch-001-schematic-zones` (commit `e37996c`); it is not part of this candidate, it is not reviewed here, and no SCH-001 acceptance is claimed for it.`

**What the code shows.** The "first engine slice" is **on the candidate**, not only on the branch:
- `git ls-tree -r --name-only feature/sch-001-schematic-zones crates/document/src/schematic.rs` → `crates/document/src/schematic.rs` exists on that branch.
- At the pinned SHA: `crates/schematic/` (new crate, 410 lines), `crates/document/src/schematic.rs` (839 lines), `crates/document/src/schematic_view.rs` (296 lines, 4 `#[test]`), `crates/schematic/src/layout.rs` (4 tests). `Cargo.toml:3` lists `"crates/schematic"`.
- The command is wired in the document layer: `lib.rs:530` `GenerateSchematic { diagram: Box<SchematicView> }`, applied at `lib.rs:1239`, produced at `schematic_view.rs:195`.

**Desktop caller: none.** `grep -rn -i "schematic" apps/` → **0 hits**. `apps/desktop/Cargo.toml` does not depend on `openbim-schematic`. There is **no** `apps/desktop/src/schematic_view.rs` at this SHA (`ls` → No such file). The `Command::GenerateSchematic` exists and is applied by the document, but no UI path reaches it.

**Impact.** The status line "nothing in this document is implemented" is false: zones/extraction (`schematic.rs`), layout (`crates/schematic/src/layout.rs`) and the generator/command (`schematic_view.rs`) exist on the candidate. Only the **presentation + desktop view** slice (SCH-003) is missing, and the command has no caller. Reword line 5 to scope the branch claim to SCH-001 only, and note the command is unwired.

---

## C6 — backlog MEP-002 marked "PLANNED/UNVERIFIED"

**Verdict: APPROXIMATE — the "PLANNED/UNVERIFIED" wording does not appear in the doc at all; the row is a bare task with no status column, and it is out of date in substance.**

**Real lines.**
- Doc: `docs/revit-style-ui/05-backlog.md:24` (MEP-002), `:3` (status line), `:30` (SCH-006)
- Code: `crates/calc/src/electrical.rs` (988 lines, 16 `#[test]`), `fluid.rs` (54), `friction.rs` (159, 6 tests), `plumbing.rs` (107, 3 tests), `sprinkler.rs` (175, 5 tests), `tree.rs` (482), `crates/document/src/compute.rs` (659, 9 tests), `crates/document/src/compute_elec.rs` (184, 1 test), `apps/desktop/src/compute_ui.rs` (488, 1 test), `apps/desktop/src/elec_ui.rs` (782, 4 tests)

**Quoted text (the backlog's own wording).**
- Header `:3`: `Statuses below are **not started as UI integration** unless indicated. Existing engine features may already cover some behavior.`
- Row `:24`: `| MEP-002 | P1 | Hydraulic/airflow calculation modules (electrical calculations imported from edesign-alike, see 06) | MEP-001 | Verified reference calculations with units |`
- Row `:30`: `| SCH-006 | P2 | Live values: MEP-002 hydraulic/airflow wiring, imported edesign-alike electrical results, adopted standard editions | MEP-002, SCH-003, SCH-004, SCH-005 | ... |`

**What the code shows.** `grep -n -i "PLANNED\|UNVERIFIED" docs/revit-style-ui/05-backlog.md` → **no matches**. The table has no status column, so "PLANNED/UNVERIFIED" is the audit's paraphrase, not the doc's words. The audit states the finding at `docs/audits/2026-10-21-mep-audit.md:220`: `MEP-002 "PLANNED/UNVERIFIED" is out of date: calculation modules and the Compute UI exist with unit tests (calc/*, document/compute.rs)`. The substance is right: `crates/calc/*` (1,996 lines, 30 tests) and the Compute/Electrical UI exist. But the row's own acceptance — *"Verified reference calculations with units"* — is **not** met: the header says `No standard values (material properties, U-values, sizes) are built into the program; test values are illustrative` (root `README.md:18`), so there are no hand-derived reference figures.

**Impact.** Two separate staleness problems: (1) the audit quotes a status string that is not in the doc; (2) MEP-002 is not started as *UI integration* but its engine + UI half is built, while its acceptance ("verified reference calculations") is unmet. Add a status column and split "engine exists" from "reference verified".

---

## C7 — agreed decision 2: electrical calculations imported, not reimplemented

**Verdict: CONFIRMED — the doc decision says "does not reimplement"; the repo contains a complete IEC 60364 implementation.**

**Real lines.**
- Doc: `docs/revit-style-ui/06-mep-schematic-automation.md:10` (decision 2), `:74`, `:100` (SCH-004)
- Code: `crates/calc/src/electrical.rs` (988 lines, 16 `#[test]`), `crates/document/src/compute_elec.rs` (184 lines, 1 test), `apps/desktop/src/elec_ui.rs` (782 lines, 4 tests)

**Quoted text.**
- Doc `:10`: `2. **Electrical reuse (approved, "option A").** revit-alike pins `openbim_electrical_model` and `openbim_sld` from the edesign-alike repository by git SHA. Electrical calculations are produced by edesign-alike and imported; revit-alike does not reimplement them. ... Single development, no duplicate electrical engine.`
- Doc `:74`: `- Every annotation value comes from an MEP-002 calculation module (hydraulic/airflow) or an imported edesign-alike result (electrical). ... (AUTO-007 rule: no calculation reimplemented in nodes).`
- Doc `:100`: `| SCH-004 | Electrical reuse: pinned crates + `DiagramView` generator | ... no calculation reimplemented; ... |`

**What the code shows.** `crates/calc/src/electrical.rs` is **988 lines** of local electrical design (IEC 60364 sizing, 16 tests), plus `crates/document/src/compute_elec.rs` (184 lines) and an electrical UI (`apps/desktop/src/elec_ui.rs`, 782 lines). `grep -rn "edesign\|openbim_electrical_model\|openbim_sld" --include=*.rs --include=*.toml .` → **0 hits**: no pinned SHA, no import path, no companion-app dependency anywhere in the workspace. The audit records the same contradiction at `docs/audits/2026-10-21-mep-audit.md:220`: `Electrical: doc 06 decision 2 says electrical calcs are imported from edesign-alike and not reimplemented; the repo contains its own full IEC 60364 implementation (crates/calc/src/electrical.rs, 988 lines, plus compute_elec.rs). Direct contradiction of the documented decision (or the decision is not yet applied).`

**Impact.** Direct conflict between an agreed user decision and the code. Either the decision is superseded (record it and update 06 §Decisions 2, :74 and SCH-004) or the implementation must be removed/pinned. As written, the design record says one thing and the tree does another.

---

## C8 — project storage = operation log + snapshots, IFC4 exchange only

**Verdict: CONFIRMED — decision written in `docs/revit-style-ui/README.md` (lines 10 and 34); code persists a full text snapshot and `crates/oplog` is unused.**

**Real lines.**
- Doc: `docs/revit-style-ui/README.md:10` and `:34` (decision log item 2, "13 and 15 Oct 2026")
- Code: `apps/desktop/src/file.rs:44-80` (save/open), `crates/document/src/native.rs:130-136` (`pub fn records` → one line per element), `crates/oplog/src/lib.rs` (complete log/CRC/snapshot/merge engine, 0 dependents)

**Quoted text.**
- Doc `:10`: `- IFC-native interoperable BIM workflows, with an application-owned parametric model: the project is stored in its own format (operation log and snapshots), and IFC4 is the exchange format, read and written in Rust with stable GlobalIds.`
- Doc `:34`: `2. **Storage and IFC:** own project format (operation log and snapshots); IFC4 for exchange, read and written in Rust inside the app; element IDs and GlobalIds stable (13 and 15 Oct 2026).`
- Code `native.rs:130-131`: `/// All the lines of a document: the format, the site, then the elements by number.` / `pub fn records(doc: &Document) -> Vec<String> {`
- Audit `docs/audits/2026-10-21-struct-ifc-audit.md:157`: `crates/oplog is a standalone crate ... grep for `openbim_oplog`/`openbim-oplog` outside the crate: 0 hits ... The task statement "the working file is the app's own operation-log + snapshot format" is NOT what the code does: the working file is a full text snapshot written on Save; the op-log engine exists but is unused by the app`

**What the code shows.** Save writes a **full text snapshot** of every element (`native::records`, one record per element, `F|`/`S|` header lines) atomically to `.obim` (`file.rs:44-72`). Undo/redo is also whole-state: `Document::undo` at `crates/document/src/lib.rs:1275` does `self.future.push((std::mem::replace(&mut self.elements, before.0), ...))` — a full clone per step, no limit. `crates/oplog` is a workspace member (`Cargo.toml:3`) with a real engine (`Transaction::apply`, `frame`, `read_log`, `replay`, `snapshot`, `load_snapshot`, `merge`) but **no crate depends on it**: `grep -rn "oplog" --include=*.rs crates/ apps/` (excluding the crate itself) → **0 hits**; `apps/desktop/Cargo.toml` has no `openbim-oplog`.

**Impact.** The agreed storage decision is unimplemented. No autosave, no crash recovery, no log replay, no merge. The gap analysis already raises it as an open decision: `docs/gap-analysis-2026-10-21.md:258`: `2. Is the working-file decision (operation log + snapshots) still wanted? If yes, autosave + recovery is the first use of `crates/oplog`.`

---

## C9 — revit-book-review: "two fixed level tabs" / dangling-level export failure

**Verdict: CONFIRMED — the "two fixed level tabs" text exists; the review does **not** mention the dangling-level IFC export failure.**

**Real lines.**
- Doc: `docs/revit-book-review.md:63` (A8)
- Code: `apps/desktop/src/model_view.rs:68` and `:72` (fixed tab labels), `crates/arch/src/lib.rs:82-93` (`Levels::new` rejects duplicate names / levels too close), `apps/desktop/src/export.rs:289` (`Levels::new(levels).map_err(...)`)
- Cross-ref: `docs/audits/2026-10-21-arch-audit.md:301` (D14), `:205` (finding 7)

**Quoted text.**
- Doc `:63`: `| A8 | **Levels and grids**: Create Plan View check, plan view types, level copy/monitor, story elevation display; grid Multi-Segment and Pick Lines, bubbles and 2D/3D extents; **Reference Planes and Work Planes** (set work plane, show) | Level and Grid tools exist; two fixed level tabs; no reference planes, no work planes | P1 (level tabs, reference planes), P2 (rest) |`
- Code `model_view.rs:72`: `let tabs = [("Level 1".to_string(), View::Plan), ("Level 2".to_string(), View::UpperPlan)].into_iter()`
- Code `model_view.rs:68`: `let added: Vec<(View, String)> = app.levels().into_iter().filter(|l| l.0 != ElementId(10) && l.0 != ElementId(11)).map(|l| (View::Level(l.0.0), l.1)).collect();`
- Audit `:301`: `| D14 | docs/revit-book-review.md A8 "two fixed level tabs" | Matches, but the consequence (dangling level 10/11 in a new project, IFC export failure) is not mentioned |`

**What the code shows.** The "two fixed level tabs" are literal: `model_view.rs:72` hard-codes `"Level 1"` → `View::Plan` and `"Level 2"` → `View::UpperPlan`; any further level is appended (`:68`, filtering out `ElementId(10)`/`ElementId(11)`). In a **new** project (`file.rs:99-104` `reset_model` → `Document::new()`, no levels) those two tabs point at non-existent levels. IFC export resolves each element's level with `level_of` and **hard-fails** when it is gone: `export.rs:123` `(Category::Wall, Some(Geometry::Linear { start, end, level })) => { ... chain.base_z = level_of(level).ok_or("a wall stands on a level that is gone")? ... }`, and `export.rs:289` `levels: Levels::new(levels).map_err(|e| format!("levels: {e:?}"))?` where `Levels::new` (`crates/arch/src/lib.rs:82-93`) rejects `LevelsTooClose` and `DuplicateName`. `grep -rn -i "dangling" docs/revit-book-review.md` → **0 hits**; the word appears only in `docs/audits/2026-10-21-arch-audit.md:205,301`.

**Impact.** The review's A8 row is accurate but incomplete: it states the symptom ("two fixed level tabs") without the consequence the audit names (a new project's dangling levels 10/11 make IFC export fail). Low severity — the fact is right, the omission hides a real bug. Add the dangling-level/export-failure consequence to A8.

---

## C10 — root README lists only 7 crates vs ~19 in the workspace

**Verdict: CONFIRMED.**

**Real lines.**
- Doc: `README.md:6-13` (crate table)
- Code: `Cargo.toml:3` (workspace `members`)

**Quoted text.**
- Doc table rows (`README.md:8-13`): `crates/geom`, `crates/family`, `crates/place`, `crates/keymap`, `crates/arch`, plus `spikes/*` — **5 crates** named, 6 table rows.
- Code `Cargo.toml:3`: `members = ["crates/geom", "crates/family", "crates/place", "crates/keymap", "crates/arch", "crates/oplog", "crates/sketch", "crates/route", "crates/ifc", "crates/view", "crates/viewport", "crates/viewport-egui", "crates/photo", "crates/sheets", "crates/automation", "crates/calc", "crates/document", "crates/schematic", "apps/desktop"]`

**What the code shows.** **19 members**: 18 `crates/*` + `apps/desktop`. `grep -c 'name = "openbim-' crates/*/Cargo.toml` = **18 crates**. The README table names **5** of them (`geom`, `family`, `place`, `keymap`, `arch`) and omits `oplog`, `sketch`, `route`, `ifc`, `view`, `viewport`, `viewport-egui`, `photo`, `sheets`, `automation`, `calc`, `document`, `schematic` — 13 missing, and `apps/desktop` is absent entirely. (The claim's "only 7" counts the 5 crate rows + `spikes/*` + a trailing row; the doc's own crate rows are 5, so the claim is conservative in the doc's favour.)

**Impact.** The README no longer describes the workspace. A reader cannot tell that the desktop app, the IFC writer, the document model, the calculation modules or the schematic engine exist. Update the table to the 18 crates + `apps/desktop`.

---

## Stale-document table

| file | stale claim (quote) | what the code shows | severity |
|---|---|---|---|
| `docs/revit-style-ui/01-ui-specification.md:20` | "All 14 ribbon tabs stay visible; tabs and tools not built yet are shown disabled." | All 14 tabs clickable; `big_button` has no disabled path; unbuilt ids fall to `main.rs:495` "not implemented in this UI prototype". | High (spec vs code) |
| `docs/revit-ux-parity-audit.md:3` | "Verified by automated event tests (simulated mouse and keys)" | No test reaches `Desktop::ui` (`main.rs:565`, private, needs `eframe::Frame`); ribbon/options bar/global keys untested (`arch-audit.md:221`). | High (false verification claim) |
| `docs/3d-view-parity-plan.md:21` | "Every drawing and modify tool (Wall, Move, Rotate, Mirror, Copy, Align, Trim, Offset, Dimension, Tag, Room, Stair, Shaft...) is plan-only" | `view3d.rs:237-246` places Door/Window/Component by 3D click (`Tool::Family` + `last_hit` → `family_click`). | High |
| `docs/3d-view-parity-plan.md:36` | "there is no redo (known)" | `Document::redo` (`lib.rs:1288`), `Desktop::redo_document` (`main.rs:547`), Ctrl+Y (`main.rs:578`). | High |
| `docs/3d-view-parity-plan.md:3` | "This is a plan, not built work: nothing here has been started." | 3D view, orbit, picking, section box, memory-budget refusal all exist and are tested. | High |
| `docs/agent-team/gap-review.md:26` | "PROTOTYPE ON BRANCH: ... main does not include the desktop app." | `main` has `apps/desktop` (34 files, 18,502 lines). | High |
| `docs/agent-team/gap-review.md:9` | "Do not start a second GUI: `origin/feature/rust-gui` already has a desktop prototype..." | Same; implies the desktop is not on main. | Medium |
| `docs/agent-team/gap-review.md:24` | "BIM-004 ... PARTIAL / UNVERIFIED: oplog and document primitives do not establish native save/open" | Save/open exist and are tested (`file.rs:44-80`, test at `file.rs:140`); only recovery is unmet. | Low |
| `docs/revit-style-ui/06-mep-schematic-automation.md:3` | "Status: **proposed 20 Oct 2026; user decisions recorded below; not implemented.**" | `crates/schematic` (410 ln), `document/src/schematic.rs` (839 ln), `schematic_view.rs` (296 ln, 4 tests), `Command::GenerateSchematic` (`lib.rs:530,1239`). | High |
| `docs/revit-style-ui/06-mep-schematic-automation.md:5` | "A first engine slice ... exists on the separate local branch `feature/sch-001-schematic-zones` ...; it is not part of this candidate" | That slice is on the candidate; `crates/schematic` is a workspace member (`Cargo.toml:3`). No desktop caller (`grep -i schematic apps/` = 0). | High |
| `docs/revit-style-ui/05-backlog.md:24` | MEP-002 row (no status) — audit calls it "PLANNED/UNVERIFIED" | `crates/calc/*` (1,996 ln, 30 tests) + Compute/Elec UI exist; acceptance "Verified reference calculations with units" unmet (no standard data). | Medium |
| `docs/revit-style-ui/README.md:10,34` | "the project is stored in its own format (operation log and snapshots)" | Working file is a full text snapshot (`native.rs:130`); `crates/oplog` has 0 dependents. | High (agreed decision) |
| `docs/revit-style-ui/README.md:42` | "The egui application lives in `apps/desktop` (branch `feature/rust-gui` until merged)." | `apps/desktop` is on `main` and a workspace member. | Medium |
| `docs/revit-style-ui/03-implementation-architecture.md:5` | "The egui desktop app (`apps/desktop`) and a provisional `crates/document` are on branch `feature/rust-gui`; `crates/document` is to be merged into the oplog model (#24)." | Both are on `main`; `crates/document` is not merged into oplog (oplog still 0 dependents). | Medium |
| `docs/revit-style-ui/03-implementation-architecture.md:4` | "`crates/geom`, `family`, `place`, `keymap`, `arch`, `oplog`, `sketch`, `route`, `ifc`." | Workspace has 18 crates; the list omits 10. | Low |
| `docs/revit-style-ui/04-visual-automation.md:133` | "## Prototype on `feature/rust-gui` (decision 17 Oct 2026, proposed)" | `crates/automation` is on `main` (`Cargo.toml:3`) and is a desktop dependency. | Low |
| `docs/3d-view-and-elevations.md:3` | "Status: plan. Elevations are implemented in `crates/view` ...; the 3D view is not started." | 3D view exists (`apps/desktop/src/view3d.rs`, `crates/viewport`, `crates/viewport-egui`). | High |
| `README.md:8-13` | Table lists `geom`, `family`, `place`, `keymap`, `arch` (+`spikes/*`) | 19 workspace members (18 crates + `apps/desktop`); 13 crates and the desktop app omitted. | Medium |
| `docs/revit-book-review.md:63` | "Level and Grid tools exist; two fixed level tabs; no reference planes, no work planes" | Text is true but silent on the consequence: fixed tabs `Level 1`/`Level 2` (`model_view.rs:72`) dangle in a new project and IFC export hard-fails (`export.rs:123,289`; `arch/lib.rs:82-93`). | Low |
| `docs/agent-team/printing-compatibility.md:3` | "Printing does not exist on `main` ... or in the `feature/rust-gui` desktop app" | `apps/desktop/src/print_dialog.rs`, `plan_export.rs` (`openbim_sheets::export::ExportCommand`), `mod print_dialog` (`main.rs:29`). | Medium |
| `docs/prt-001-verification.md:3` / `docs/prt-004-verification.md:3` | "PR #45 remains documentation-only. No desktop Print dialog or branch integration is claimed." | Desktop Print dialog exists (`print_dialog.rs`, `main.rs:29,384,585`). | Low |
| `docs/prt-003-verification.md:3` | "desktop controls and shared application command integration are still PRT-006" | Print dialog + `ExportCommand` wired in the desktop. | Low |
| `docs/gap-analysis-2026-10-21.md:191` | "Project storage = operation log + snapshots; IFC4 is exchange only (agreed 13 Oct)" | Correct as a *finding*; the decision it cites is not in `docs/revit-style-ui/03-implementation-architecture.md` — it is in `docs/revit-style-ui/README.md:34`. | Low (attribution) |

---

## Agreed decisions the code does not follow

1. **Ribbon disabled state** (`01-ui-specification.md:20`, agreed design) — unbuilt tools are enabled and emit a status message (`main.rs:495`) instead of being disabled. *C1.*
2. **Electrical calculation reuse, decision 2** (`06-mep-schematic-automation.md:10`, approved "option A", 20 Oct 2026) — code reimplements a full IEC 60364 engine (`crates/calc/src/electrical.rs`, 988 lines) and has **no** `edesign-alike` pin or import anywhere. *C7.*
3. **Project storage = operation log + snapshots** (`docs/revit-style-ui/README.md:10,34`, decision log item 2, 13 and 15 Oct 2026) — code writes a full text snapshot (`native.rs:130`) and never uses `crates/oplog` (0 dependents). *C8.*
4. **Document model converges into the oplog model (#24)** (`03-implementation-architecture.md:5`; `04-visual-automation.md:140` "crates/document converges with crates/oplog (BIM-001) before automation builds on either") — `crates/document` is on main and still standalone; `crates/oplog` remains unconsumed.
5. **3D editing scope** (`3d-view-parity-plan.md:21,36`) — the plan's stated limits (no 3D placement, no redo) are already exceeded by the code; the plan, not the code, is out of date.
6. **Schematic "not part of this candidate"** (`06-mep-schematic-automation.md:5`) — the engine slice is on the candidate; the code is ahead of the status line, and the command has no desktop caller.

---

*End of report. All line numbers are at `6ac8addc9550fcbed3d2b5cce868a540fe0a0780`; `main`-only figures are marked `main`. Nothing was compiled or executed.*
