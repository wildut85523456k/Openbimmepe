# Gap progress report, 2026-10-23 (vs. baseline 2026-10-21, `dcf8382`)

Scope: from-scratch BIM authoring with the mouse, architecture then MEP, IFC as the exchange format.
Structure stays deferred (supports and trusses for MEP only). Compared: `main` at `271c774` (54 commits after the baseline).

## 1. Answer in brief

- **The app can now be started from a blank project.** The blocker that stopped everything on 21 Oct (A1: no input on an empty canvas) is closed, as are the wrong level tabs, the Esc behaviour, the missing keyboard shortcuts and the dishonest ribbon.
- **MEP content has started**: Load Family, editable family types, an air handling unit and a ceiling diffuser, an HVAC system built end to end, and electrical values on the single-line schematic.
- **The IFC side did not move.** `crates/ifc` has no changes since the baseline, and `export.rs` still drops rooms, grids and penetrations and tags runs `System::Other`. This is now the largest block of easy, unclosed work.
- **MEP depth is still thin**: 7 families (3 MEP), no spaces/zones, no vertical runs, no plumbing fixtures or sprinkler heads, no electrical objects.

## 2. Verification status (read this first)

| Check | Result |
|---|---|
| Code reading of each baseline gap on `271c774` | Done (sections 3-4) |
| Full test run on `271c774` | **Not run.** `openbim-document` now depends on the private `edesign-alike` repo (rev `57ef018`); this sandbox has no access (HTTP 401) and access was not granted. |
| Team-reported results | Desktop 185 passed / 0 failed / 1 ignored; document 161 passed + 2 integration; clippy `-D warnings` clean (`docs/audits/2026-10-21-p0-review/README.md`). Not independently reproduced. |
| Baseline for comparison | 1,140 passed / 0 failed / 2 ignored at `dcf8382` (my run). |
| Runtime repro of A1 | Not repeated; closure is from code (`model_view.rs:17` now falls back to a default 20 m view box) and the team's tests. |

To reproduce: grant this session access to `edesign-alike`, or run `cargo test --workspace --release` locally.

## 3. Progress delta

Status: **Closed** = fix present in code on main; **Partial**; **Open**; **New** = created by the change.

### 3.1 Blockers and shell (class 2.1)

| ID | Gap (21 Oct) | Status | Evidence |
|---|---|---|---|
| A1 / B1 | Plan canvas dead on an empty project | **Closed** | `model_view.rs:17` fallback bounds; test asserts finite fallback |
| B2 | Hard-coded Level 1 / Level 2 tabs (ids 10/11) | **Mostly closed** | tabs are named after document levels (`7afe07b`, `eb073a1`). Remaining: `plan_level()` returns `ElementId(0)` when no level exists instead of refusing with "add a level first"; ids 10/11 remain only in tests |
| B3 | Unconditional Esc leaves the tool | **Closed** | two-stage Esc, `main.rs:~401` "work in progress that Esc steps back from" |
| B4 / B16 | No Ctrl+S/N/O, keymap unused, no Save As | **Closed** | `main.rs:707-760` global shortcuts and `keymap_keys`; `save_as` handled at `:605` |
| B5 | Tool parameters leak between tools | **Closed (by reset)** | per-command defaults applied on tool choice (`main.rs:394`). `slab_offset_mm` is still one shared field, so the design is a reset, not separate state |
| B6 | Duplicate / too-close levels; no rename or delete | **Closed** | `place_level` rejects duplicates and gaps; `rename_level`, `delete_level` exist |
| B17 | Ribbon shows unbuilt buttons as live | **Closed** | disabled with "planned, not implemented yet"; `ribbon.rs` test walks every id |
| B25 | Undo leaves a dangling level view | **Likely closed** (level delete/undo path was reworked; not specifically checked) |
| - | Test harness never ran `Desktop::ui` | **Closed** | window body is now a method a test can drive (`6979485`) |

### 3.2 Exchange and persistence (class 2.2)

| ID | Gap | Status | Evidence |
|---|---|---|---|
| B7 | Runs exported as `System::Other` | **Open** | `export.rs:305,307` |
| B8 | Rooms, grids, site dropped | **Open** | `export.rs:393-395`: `rooms: vec![]`, `georef: None` |
| B9 | No `IfcSystem` per user system | **Open** | `crates/ifc` unchanged |
| B10 | No wall/floor penetrations | **Open** | `wall_penetrations: vec![]`, `floor_penetrations: vec![]` |
| B11 | Unstable GlobalIds | **Open** | `gen_gid` counter still at `ifc/src/lib.rs:645` |
| B12 | Compute inputs not persisted | **Open** (not re-checked in `native.rs`) |
| B13 | MEP Settings sizes ignored by Compute | **Open** (not re-checked) |
| B14 | End point drops equipment join | **Open** | `compute_ui.rs:132` unchanged |
| B15 | Air preset keeps water velocity limit | **Open** | `compute_ui.rs:69` `max_velocity: 1.5` |
| B18 | Compute per enum, not per named system | **Open** (not re-checked) |
| B19 | Trusses lose parent on export | **Open** |
| B20 | No autosave / recovery; `oplog` unused | **Open**; `oplog` still not a dependency of any crate |
| B26 | Electrical engine duplicated from edesign-alike | **In progress**: `document` now pins edesign-alike by git rev (`Cargo.toml:25-26`). Removal of the in-repo `crates/calc/src/electrical.rs` was not verified |
| B21-B24 | Room name/number, level bubbles, parameter tags, pad association | **Open** (not re-checked) |

### 3.3 MEP content (class 3.1)

| ID | Gap | Status | Evidence |
|---|---|---|---|
| C1 | No Load Family | **Closed (first slice)** | `ef92aaa`, `33a01e3`: a catalogue file becomes types in the project |
| C2 | Type parameters not editable | **Closed (first slice)** | `719815d`: a type is edited through Properties; types carry texts and numbers |
| C3 | One MEP family | **Partial** | library is `fan_coil`, `ahu`, `ceiling_diffuser` (+ handle, door, window, arched door) = 7. Pump, valve, VAV, fixtures, sprinkler, luminaire, panel, hanger still missing |
| C4 | No air terminals | **Partial** | ceiling diffuser exists; grille/register variants and the four direction flags not checked |
| C5 | No hosting on ceiling/face | **Open** (not re-checked) |
| C6 | No spaces / zones | **Open** | no module in `crates/document/src` |
| C7 | Systems of equipment only | **Partial** | "HVAC system end to end" added (`719815d`); whether runs are members was not checked |
| C8-C11 | Layout types, duct/pipe types, vertical runs, snapping | **Open** (not re-checked; no commits target them) |
| C12 | Sizing methods | **Open** |
| C13 | System inspector, checks, legend | **Open** |
| C14 | Moving equipment drags runs | **Open** |
| C15 | Plumbing fixtures, fall check | **Open**: `Route::check_fall` still has only its own tests as callers |
| C16 | Sprinkler heads | **Open** |
| C17 | Electrical as elements and circuits | **Open**, but SCH-006/MEP-002 now stamp feeder current, phase voltage and cumulative voltage drop on the single-line schematic (`2103246`, `678ee01`, `0248cd9`, `eb67000`) |
| C18 | Hangers and supports | **Open** |

### 3.4 Automation and documentation (new since baseline)

- **New:** `crates/commands` (AUTO-001, typed command and query API) and AUTO-002 (pending handles, dependency groups, atomic apply). These matter for the "act as Revit" goal because they give a scripted path to the same operations the mouse has.
- **Docs closed:** reference docs, gap analysis, remediation plan and closure plan are on main; the team re-verified my citations (`docs/audits/verify-*.md`) and recorded five corrections.
- Class 1 items R2-R9 (README crate table, stale claims, extraction manifest, acceptance checklists as tests) were not re-checked.

### 3.5 Scoreboard

| Group | Baseline rows | Closed | Partial | Open |
|---|---|---|---|---|
| Shell blockers (A1, B1-B6, B17, B25) | 9 | 7 | 2 | 0 |
| Exchange / persistence (B7-B15, B18-B24, B26) | 17 | 0 | 1 | 16 |
| MEP content (C1-C18) | 18 | 2 | 3 | 13 |

## 4. What this means for "build a model from scratch with the mouse"

| Step | 21 Oct | 23 Oct |
|---|---|---|
| New project, add levels, rename, delete | Blocked | Works (code + team tests) |
| Draw grids, walls, floors, doors, windows | Blocked on canvas | Reachable; grid rename/arcs, wall top constraint and wrapping still missing (C23, C28) |
| Place AHU / diffuser, edit type values | Not possible | Works |
| Draw ducts and pipes, size, build an HVAC system | Partial | Works for ducts; vertical runs, types and routing preferences missing |
| Plumbing, fire protection | Abstract nodes only | Unchanged |
| Electrical | Abstract tree | Unchanged, values on the schematic improved |
| Export IFC | Loses rooms, grids, systems | **Unchanged** |

## 5. Remediation, tiered

### 5.1 Easy (days, mostly wiring, existing code is waiting)

1. **IFC honesty bundle (B7, B8, B9, B10, B14, B15, B19, B11).** The IFC crate already writes spaces, boundaries, systems and openings; the app just passes empty lists. One acceptance test: export a model with a straight supply duct, rooms and grids, parse the file, count `IFCSPACE`, `IFCGRID`, the system names and openings. This is the best next step: no new design needed and it fixes what users see in other tools.
2. **Finish B2:** make `plan_level()` return `Option` and refuse tools with "add a level first".
3. **B13, B12, B16:** Compute reads MEP Settings sizes; persist Compute inputs in `.obim`.
4. **B21-B24:** room number/name, level bubbles, parameter tags, pad on equipment.
5. **R4-R6 docs:** README crate table, mark superseded claims, extraction manifest.
6. **Make the test run reproducible offline or in CI with access to `edesign-alike`**, so the suite can be re-run by anyone.

### 5.2 Medium (new feature on current architecture, weeks)

- MEP library to the planned order: VAV, pump, valve, WC/basin, sprinkler, luminaire, panel board, hanger (C3, C4, C15, C16, C18).
- Hosting (C5), spaces and zones with airflow (C6), runs as system members and connector flow direction (C7), Perimeter / Intersection / Custom layout (C8), duct and pipe types (C9), vertical runs (C10), shared snapping (C11), sizing methods (C12), inspector and legend (C13), moving equipment drags runs (C14).
- Electrical as elements with circuits and panel schedules (C17), reading from edesign-alike per decision B.
- Architecture documentation set: dimensions, annotation, ceiling plans, units, reference planes (C19-C29).

### 5.3 Later stage

Massing and toposolid (D1, D2), curved/curtain walls (D3, D4), sketched stairs (D5), link/worksets/phases (D6), family editor (D8), semantic IFC import (D13), undo memory (D12), rendering (D11), fabrication (D7), RVT export (D9, out of scope). **Structure (D10)** stays deferred: when it comes, start with the MEP-support slice only (hangers on beams/slabs, simple truss as an assembly, B19), not the structural analysis stack.

## 6. Risks seen in this review

- **Private dependency now blocks outside builds and review.** Any contributor or CI job without `edesign-alike` access cannot build `openbim-document`.
- **Reported test counts are not reproducible here**, so closure of the shell blockers rests on the team's tests plus my code reading.
- **IFC lag is widening**: MEP content grows (new families, systems) while export still collapses it to `System::Other`.
- Shared `slab_offset_mm` reset (B5) works but is fragile; a new tool that forgets to register defaults will leak again.

## 7. Open owner questions (status)

| Question | Status |
|---|---|
| Disable unbuilt ribbon buttons | Decided and done |
| Electrical calc source | Decided (edesign-alike); dependency pinned; old engine removal to confirm |
| Storage model / oplog | Still open; B20 depends on it |
| First MEP families | Partly answered (AHU, diffuser built); the rest of the order stands |
| Test building (office vs data hall) | Still open |
| **New:** grant sandbox access to `edesign-alike` so tests can be re-run | Open |
