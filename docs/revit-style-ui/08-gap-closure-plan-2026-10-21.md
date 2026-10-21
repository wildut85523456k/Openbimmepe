# 08 — Gap closure plan (proposed 21 Oct 2026)

Status: **proposed plan**, dated 21 Oct 2026, derived from the gap analysis in
[07 — Gap analysis](07-gap-analysis-2026-10-21.md) and the owner's decisions of the same date
(section 1). It is a plan, not a record of implemented behaviour: every item below is unstarted
unless the acceptance column says otherwise.

Baseline: `main` at `c90cbc8`. The analysis was written against `dcf8382`; the six Tier A
blockers were re-checked against `c90cbc8` before this plan was written (section 2).

Scope: architecture and MEP first; structure only where MEP needs it; the rest of structure is a
later stage (unchanged, 21 Oct 2026).

---

## 1. Owner decisions taken (21 Oct 2026)

| # | Question | Decision | Effect on this plan |
|---|---|---|---|
| 1 | Unbuilt ribbon buttons: disable, or keep the "not implemented" message? | **Disable them** as `01-ui-specification.md` specifies | GAP-07, now P0: cheap while the ribbon is being touched anyway. The disabled state must come from the command registry (04-visual-automation Rule 9), not a second hand-written list |
| 2 | Working file: operation log + snapshots? | **Yes, agreed.** Autosave + recovery is the first use of `crates/oplog` | GAP-15: `crates/oplog` gains its first dependent; BIM-004's acceptance grows |
| 3 | First MEP families for the content library | **AHU, ceiling diffuser, VAV, pump, valve, WC/basin, sprinkler head, luminaire, panel board, hanger/support** | GAP-18 fixes the library list; the hanger/support family covers the structure-for-MEP need (7 in the analysis) without a structure stage |
| 4 | Electrical calculations: keep the in-repo IEC 60364 module, or import from edesign-alike? | **Export and import from edesign-alike** | GAP-24/25: the in-repo module becomes a migration source, not the destination. This is a correctness decision as well as a de-duplication one — the project's rule is no duplicate development across the two repositories |
| 5 | End-to-end test building | **Data hall for the MEP test; keep the office for architecture** | GAP-08 builds the harness and the office (TEST-001, P0); GAP-19/MEP-004 adds the data hall at P3. The data hall stresses routing, flow and schedules in an open plan; the office keeps the wall/door/stair path |
| 6 | *New* creates default levels (raised by this plan) | **Yes** | GAP-09: *New* creates Level 1 and Level 2, so no tool can point at a level that does not exist |

---

## 2. What was re-checked against `c90cbc8` (not taken on trust)

The analysis carries three review agents' reading that was not re-checked line by line. Before
scheduling work on it, the following were verified in the tree:

- **Tier A still reproduces.** `model_view.rs:143` (`if !bounds.is_finite() { return; }`) is still
  above every tool branch; the plan tabs are still the hard-coded `Level 1` / `Level 2` pair
  (`model_view.rs:72`, `panels.rs:931,943`); `main.rs:571` still calls
  `choose_tool(Tool::Select)` on any Escape; the keymap is still only printed in Help
  (`main.rs:661`); the shared `slab_thickness_mm` / `slab_offset_mm` state is still set by
  Ceiling, Roof, Component and footings (`main.rs:462-463`); `place_level` still accepts a
  duplicate name and a second level within 100 mm (`crates/document/src/lib.rs:683`).
- **The "not implemented" fallback is real** (`main.rs:499`), reached by the ribbon's catch-all
  arm. The count of unwired MEP buttons (45 of 63) is the MEP audit's, not re-counted here.
- **The in-repo electrical module is live, not dead code.** `crates/calc/src/electrical.rs` (988
  lines) is called from `apps/desktop/src/elec_ui.rs:358`. Importing from edesign-alike is
  therefore a migration with an equivalence gate (GAP-25), not a deletion.
- **The edesign-alike pin is cycle-free.** `openbim-electrical-model`, `openbim-electrical-solver`
  and `openbim-sld` have no `openbim-family` dependency (only `openbim-electrical-api` and
  `openbim-breaker-catalog` do). Pinning those three by SHA in this workspace does not close the
  loop that edesign-alike already has on this repository's family crate.
- **The analysis and its companion audits are not in this repository.** Only
  `docs/agent-team/gap-review.md` and `gap-inventory.json` exist; the three per-discipline audits
  the analysis cites (`docs/audits/2026-10-21-*-audit.md`) are held outside it. Landing 07 is
  GAP-16; the audits stay an external lead until their authors supply them, and 07 must not
  claim they are tracked files.

Still to verify before the item that depends on it: the family catalogue loader's real interface
(GAP-17), the oplog crate's API surface (GAP-15) and the electrical solver's coverage against the
in-repo module (GAP-25). Each is the first step of its own work package.

---

## 3. Work packages

Effort is an estimate in working days, not a commitment.

### P0 — Walking skeleton (blocks the from-scratch journey)

| ID | Fix | Evidence in the analysis | Est. | Acceptance |
|---|---|---|---|---|
| GAP-01 | Default view box when no mesh or grid gives finite bounds | A1 | 1 | A test starting from `new_model()`: Grid and Wall clicks create elements |
| GAP-02 | Derive the plan tabs and the Project Browser from the document's levels; refuse a tool with "add a level first" | A2 | 2 | No hard-coded level ids; a wall in a new project references a real level; IFC export passes |
| GAP-03 | Remove the unconditional `choose_tool(Select)` on Escape; let each tool step back first | A3 | 1 | Two-stage Esc proven through `Desktop::ui`, not around it |
| GAP-04 | Consult `crates/keymap` for key events: Ctrl+S / Ctrl+N / Ctrl+O, a Save As button, the Revit two-letter shortcuts | A4 | 2 | Key events drive real commands; the Help page stops being the only consumer |
| GAP-05 | Per-tool parameter state instead of the shared slab fields | A5 | 1 | Ceiling then Door places the door at floor level |
| GAP-06 | Validate level name and spacing at placement; rename and delete levels | A6 | 2 | Duplicate names and levels under 100 mm apart are refused; rename/delete are undoable |
| GAP-07 | Ribbon honesty: unbuilt tools are **disabled** with a planned-state tooltip, resolved from the registry | section 8, decision 1 | 2 | Ribbon honesty test: every ribbon id is handled or disabled; no "not implemented" path remains |
| GAP-08 | End-to-end harness: extract the `Desktop::ui` body from the `eframe::Frame` argument, add the plan model-to-screen transform | section 11 | 3–5 | Pointer-driven tests drive the real ribbon, options bar and global keys |
| GAP-09 | *New* creates Level 1 and Level 2 | section 4 | 1 | A new project can host a wall immediately; the "level is gone" export failure is unreachable |

**P0 exit:** from `Desktop::ui` — new project → 3 levels → grids → typed walls → floor →
door/window → stair → roof → ceiling → room → section → save/reopen → IFC export with element
counts; plus the ribbon honesty test. This is the gate for every later package: nothing after P0
is tested except through this harness.

### P1 — IFC honesty and persistence

| ID | Fix | Evidence | Est. | Acceptance |
|---|---|---|---|---|
| GAP-10 | Export the run system per run and the user's named systems with their equipment | section 6 | 2 | A straight "Supply Air" duct exports as that system, not "Other" |
| GAP-11 | Export rooms as `IfcSpace` with boundaries, grids as `IfcGrid`, georeference; **warn** for every dropped category | section 6 | 3 | Every skipped category is reported; a dropped item is never silent |
| GAP-12 | Wire `openbim_place::penetrations` into export and the UI | 5.1 | 2 | A duct through a wall cuts an opening in 3D and in IFC |
| GAP-13 | Derive storey, relationship, property-set and system GlobalIds from stable ids | section 6 | 2 | Adding a level does not change unrelated GlobalIds |
| GAP-14 | Persist the compute tables (size catalogue, loading-unit curve, cable tables) with the project; fix the End Point unjoin | section 6 | 2 | Tables survive save/reopen; marking an End Point keeps the equipment join |
| GAP-15 | Autosave, backup and crash recovery on `crates/oplog` (decision 2) | section 6 | 3–5 | A killed session recovers the last committed state; `crates/oplog` has its first dependent |
| GAP-16 | Land 07, refresh the stale docs (3D view plan, `gap-review.md`, README crate list) | section 8 | 1 | Documented behaviour matches the code; no doc claims a feature that is absent |

### P2 — MEP content (the largest MEP unlock)

| ID | Fix | Est. | Acceptance |
|---|---|---|---|
| GAP-17 | Load Family and editable type parameters: wire `crates/family::catalogue` and the type editor into the UI | 5–10 | A catalogue imports, places and its type parameters edit with the mouse |
| GAP-18 | The ten families of decision 3, each with connectors, host rules and an IFC class | 15–30 | Each places, hosts, exports its IFC class and appears in a schedule |
Everything in 5.2–5.4 of the analysis depends on GAP-17/18.

### P3 — Spaces, flow and systems

GAP-20 Space and Zone with airflow data (MEP-critical) · GAP-21 terminals with Flow and
classification; named systems that include terminals and respect connector flow direction ·
GAP-22 Generate Layout from equipment to terminals with conversion settings, and the Perimeter and
Intersection methods · GAP-23 Compute on the named system with all four sizing methods
(friction, equal friction, velocity, static regain) · GAP-19 the data-hall MEP chain end to end
(decision 5), which is MEP-004 in the backlog: it sits here, at P3, because it needs the terminals,
the named systems and the layout this package builds.

### P4 — Routing completeness

GAP-24 risers and drops, duct and pipe types with routing preferences, connector snapping, size
lock, moving equipment dragging its runs, the minimum-slope check and gravity drainage rules.

### P5 — Electrical as objects, and the imported calculations

| ID | Fix | Est. | Acceptance |
|---|---|---|---|
| GAP-25 | Pin `openbim-electrical-model` and `openbim-electrical-solver` (and `openbim-sld`, already SCH-004) by SHA; capture the in-repo module's results as golden tests; prove equivalence; then retire `crates/calc/src/electrical.rs` and rewire `elec_ui.rs` | 5–10 | Identical results at 1e-9 on the golden fixture set, or every difference documented and approved; no electrical calculation duplicated in this repository |
| GAP-26 | Panels, devices and luminaires as elements; circuits; panel schedules; cable tray and conduit | 10–20 | A panel schedule reads from circuits, not from the abstract node records |

### P6 — Coordination

GAP-27 in-model clash check, then a read-only link model once the collaboration design is settled.

### Later (Tier D, unchanged)

Massing, toposolid, curved and stacked walls, curtain walls, the family editor, RVT export,
collaboration (central model, borrowing), phasing and design options, and the structure stage.

---

## 4. Mapping to `05-backlog.md`

The backlog keeps one owner per deliverable, so most of this plan **extends an existing row's
acceptance** rather than adding a second row for the same thing.

| Plan item | Backlog row | Change |
|---|---|---|
| GAP-01…05, 08, 09 | **GUI-006** (new) | New row: the from-scratch journey and its gates |
| GAP-07 | **GUI-007** (new) | New row: ribbon honesty |
| GAP-06, GAP-02 | **BIM-005** (new) | New row: level authoring and validation |
| GAP-15 | BIM-004 | Acceptance extended: autosave and crash recovery as the first `crates/oplog` consumers |
| GAP-01, GAP-02 | GUI-004 | Acceptance extended: no click is lost on an empty canvas; the plan tabs come from the document's levels |
| GAP-04 | GUI-005 | Acceptance extended: the keymap is consulted for real key events |
| GAP-10…13 | IFC-002 (new) + IFC-001 | New row for export honesty; IFC-001's acceptance names systems, rooms, grids and penetrations |
| GAP-17, GAP-18 | MEP-003 (new) | New row: the MEP family library; FAMILY-001 keeps the editor |
| GAP-25 | MEP-005 (new) | New row: the imported electrical calculations and the retirement of the in-repo module |
| GAP-25 | MEP-002, SCH-004 | Acceptance extended: electrical values come from the pinned crates |
| GAP-08 | TEST-001 (new) | New row: the harness and the office building |
| GAP-19 | MEP-004 (new) | New row: the data-hall MEP chain, at P3 where the terminals and systems it needs are |
| GAP-16 | DOC-002 (new) | New row: docs refresh |

Phase gating in `05-backlog.md` gains a step 2a: the walking skeleton (GUI-006/007, BIM-005) gates
the rest of *Usable desktop* and every later phase's tests.

---

## 5. Sequencing and gates

Each work package runs the repository's gating sequence: research → owner-approved bounded task →
implementation → independent review in a separate profile → coordinator decision →
owner-approved merge. Implementation and review stay separate, and no push, merge or remote write
happens without explicit approval.

P0 is one branch and one review. P1 is a second branch. P2 onward may not start before P0's exit
test exists, because that test is how their results are accepted.

---

## 6. Risks and open items

- **Repository-level dependency cycle (GAP-25).** edesign-alike already pins this repository's
  family crate by SHA. Pinning its electrical crates here is cycle-free at crate level (verified,
  section 2) but couples release order: a change to a pinned crate requires a SHA bump on both
  sides. Keep the pin to the three leaf crates and record the SHA in the manifest comment.
- **Ribbon honesty versus testing (GAP-07).** Disabling 45 unwired MEP buttons is correct per the
  spec, but the disabled state must not hide what is planned: the tooltip and the registry entry
  stay, so the ribbon remains a progress map.
- **The data hall is a fixture, not a design (GAP-19).** It exists to stress routing and flow; its
  geometry should be generated by the test, not hand-modelled, so it cannot drift.
- **Effort figures are estimates** from reading the code, not measured. P2 and P5 are the two
  packages most likely to grow.
- **Unverified in the analysis:** the per-discipline status tables, the IFC coverage matrix, the
  doc/code discrepancy lists and the `file:line` citations (the structure audit marks nine as
  approximate). The desktop app has still not been launched and looked at, no performance
  measurement exists, and IFC validity has not been checked with an independent reader. These
  remain leads, not established facts, and the items above inherit that status.
