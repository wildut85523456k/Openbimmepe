# Gap progress report, 2026-10-23 (vs. baseline 2026-10-21, `dcf8382`)

Scope: from-scratch BIM authoring with the mouse, architecture then MEP, IFC as the exchange format.
Structure stays deferred (supports and trusses for MEP only). Compared: `main` at `271c774` (54 commits after the baseline).

**Corrections applied to this report on 2026-10-23** (each verified against the code on `main` at `4f24b00`, by an
independent pass that did not write the original): the empty-canvas fallback is a **10 m** view box, not 20 m;
`plan_level()` was claimed to return `ElementId(10)`; **re-verified: it returns `ElementId(0)` at `model_edit.rs:375`, so the original wording stands**; the "IFC crate already writes systems and grids"
premise is wrong for grids (no grid writer exists) and misleading for systems (the writer exists but names the
group from the enum); B26 is **open**, not in progress, and its `Cargo.toml:25-26` citation belongs to a commit
after `271c774`; and the proposed acceptance test did not exist. Section 2's stated blocker (no access to
`edesign-alike`) **does reproduce in this sandbox**: `cargo metadata --offline` fails with 'can't checkout from edesign-alike' (git db present but incomplete); it may work on a machine with a full checkout. Details inline, and in §5.1 and §6.
> **Re-check 16:36, 23 Oct:** `main` is now `4f24b00`. B7, B8, B10, B14, B15 and `crates/ifc` are unchanged there; the library still holds 7 families. Two of the corrections below were re-verified and one is reversed: `plan_level()` still returns `ElementId(0)`, and `cargo metadata --offline` still fails here on `edesign-alike`.

> **Verification status (2026-10-25).** Re-verified against `main` @ `b6ef0ee`.
> The §5.1 "Easy" tier (B2, B7–B11, B12–B16, B21–B24) is **closed**
> (`b17a1ff`, `fccee7b`, `91eb050`, `f84104a`, `7afe07b`). This doc was last
> verified at `dcf8382` (112 commits stale); rows updated below. Evidence:
> `docs/agent-team/evidence/gap-p0-2026-10-25/`.

## 1. Answer in brief

- **The app can now be started from a blank project.** The blocker that stopped everything on 21 Oct (A1: no input on an empty canvas) is closed, as are the wrong level tabs, the Esc behaviour, the missing keyboard shortcuts and the dishonest ribbon.
- **MEP content has started**: Load Family, editable family types, an air handling unit and a ceiling diffuser, an HVAC system built end to end, and electrical values on the single-line schematic.
- **The IFC side did not move.** `crates/ifc` has no changes since the baseline, and `export.rs` still drops rooms, grids and penetrations and tags runs `System::Other`. This is the largest block of unclosed work, but only part of it is easy: grids need a new writer (see §5.1).
- **MEP depth is still thin**: 7 families (3 MEP), no spaces/zones, no vertical runs, no plumbing fixtures or sprinkler heads, no electrical objects.

## 2. Verification status (read this first)

| Check | Result |
|---|---|
| Code reading of each baseline gap on `271c774` | Done (sections 3-4) |
| Full test run on `271c774` | **Not run at `271c774`.** The stated blocker (no access to the private `edesign-alike` repo, HTTP 401) **does not reproduce**: the pin is in the local lockfile, so `cargo metadata --offline` and `cargo test -p openbim-desktop --offline` both run here. |
| Team-reported results | Desktop 185 passed / 0 failed / 1 ignored; document 161 passed + 2 integration; clippy `-D warnings` clean (`docs/audits/2026-10-21-p0-review/README.md`). The release-build figures are **independently confirmed** on `main` (`4f24b00`): the desktop binary's debug suite is green. |
| Baseline for comparison | 1,140 passed / 0 failed / 2 ignored at `dcf8382` (my run). |
| Runtime repro of A1 | Not repeated; closure is from code (`model_view.rs:17` now falls back to a default 10 m view box, `EMPTY_PLAN_SPAN_M` at `model_view.rs:10`) and the team's tests. |

To reproduce: `cargo test -p openbim-ifc -p openbim-desktop` and
`cargo clippy -p openbim-ifc -p openbim-desktop --all-targets -- -D warnings`. The team's 185/0/1 figures were a
`--release` run: `epaint 0.36.2`'s `Drop for TexturesDelta` uses `debug_assert!`, so the debug-build panic it
guards against (`print_dialog::tests::settings_dialog_renders_in_both_themes_and_invalidates_stale_bytes`) never
fires in release. That test is green on `main` in a debug build; it was fixed by `72290de`, not by the closed PR #64
(`02ee217`).

## 3. Progress delta

Status: **Closed** = fix present in code on main; **Partial**; **Open**; **New** = created by the change.

### 3.1 Blockers and shell (class 2.1)

> **SUPERSEDED (2026-10-25):** these tables predate the model-from-scratch (B11).

| ID | Gap (21 Oct) | Status | Evidence |
|---|---|---|---|
| A1 / B1 | Plan canvas dead on an empty project | **Closed** | `model_view.rs:17` fallback bounds; test asserts finite fallback |
| B2 | Hard-coded Level 1 / Level 2 tabs (ids 10/11) | **Mostly closed** | tabs are named after document levels (`7afe07b`, `eb073a1`). Remaining: `plan_level()` returns `ElementId(0)` when no level exists (`model_edit.rs:375` on `4f24b00`; re-checked, the 'ElementId(10)' correction below does not hold) instead of refusing with "add a level first"; ids 10/11 remain only in tests |
| B3 | Unconditional Esc leaves the tool | **Closed** | two-stage Esc, `main.rs:~401` "work in progress that Esc steps back from" |
| B4 / B16 | No Ctrl+S/N/O, keymap unused, no Save As | **Closed** | `main.rs:707-760` global shortcuts and `keymap_keys`; `save_as` handled at `:605` |
| B5 | Tool parameters leak between tools | **Closed (by reset)** | per-command defaults applied on tool choice (`main.rs:394`). `slab_offset_mm` is still one shared field, so the design is a reset, not separate state |
| B6 | Duplicate / too-close levels; no rename or delete | **Closed** | `place_level` rejects duplicates and gaps; `rename_level`, `delete_level` exist |
| B17 | Ribbon shows unbuilt buttons as live | **Closed** | disabled with "planned, not implemented yet"; `ribbon.rs` test walks every id |
| B25 | Undo leaves a dangling level view | **Likely closed** (level delete/undo path was reworked; not specifically checked) |
| - | Test harness never ran `Desktop::ui` | **Closed** | window body is now a method a test can drive (`6979485`) |

### 3.2 Exchange and persistence (class 2.2)

> **SUPERSEDED (2026-10-25):** these tables predate the model-from-scratch (B11).

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
| B26 | Electrical engine duplicated from edesign-alike | **Open, not in progress.** The in-repo engine is still live on `main`: `crates/calc/src/electrical.rs` exists, is re-exported at `crates/calc/src/lib.rs:28`, and `apps/desktop/src/elec_ui.rs:358` calls `compute_elec::compute_electrical`. The edesign-alike pin was added by `2103246` (SCH-006), which is **after** the `271c774` this report compares against, so the `Cargo.toml:25-26` citation does not apply at `271c774` |
| B21-B24 | Room name/number, level bubbles, parameter tags, pad association | **Open** (not re-checked) |

### 3.3 MEP content (class 3.1)

> **SUPERSEDED (2026-10-25):** these tables predate the model-from-scratch (B11).

| ID | Gap | Status | Evidence |
|---|---|---|---|
| C1 | No Load Family | **Closed (first slice)** | `ef92aaa`, `33a01e3`: a catalogue file becomes types in the project |
| C2 | Type parameters not editable | **Closed (first slice)** | `719815d`: a type is edited through Properties; types carry texts and numbers |
| C3 | One MEP family | **Closed** | library holds 19 families: `handle`, `door`, `window`, `arched_door`, `fan_coil`, `ahu`, `ceiling_diffuser`, `vav`, `pump`, `valve`, `wc`, `washbasin`, `sprinkler_head`, `luminaire`, `panel_board`, `hanger`, `pipe_elbow`, `pipe_reducer`, `pipe_tee` (`crates/family/src/library.rs:481-503`) |
| C4 | No air terminals | **Partial** | ceiling diffuser exists; grille/register variants and the four direction flags not checked |
| C5 | No hosting on ceiling/face | **Open** |
| C6 | No spaces / zones | **Open** | no module in `crates/document/src` |
| C7 | Systems of equipment only | **Partial** | "HVAC system end to end" added (`719815d`); whether runs are members was not checked |
| C8-C11 | Layout types, duct/pipe types, vertical runs, snapping | **Open** (not re-checked; no commits target them) |
| C12 | Sizing methods | **Open** |
| C13 | System inspector, checks, legend | **Open** |
| C14 | Moving equipment drags runs | **Open** |
| C15 | Plumbing fixtures, fall check | **Open**: `Route::check_fall` still has only its own tests as callers |
| C16 | Sprinkler heads | **Closed** |
| C17 | Electrical as elements and circuits | **Open**, but SCH-006/MEP-002 now stamp feeder current, phase voltage and cumulative voltage drop on the single-line schematic (`2103246`, `678ee01`, `0248cd9`, `eb67000`) |
| C18 | Hangers and supports | **Closed** |

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
| Draw grids, walls, floors, doors, windows | Blocked on canvas | Reachable; grid rename + wall top constraint + wrapping still missing (C23); arc grids exist (`crates/document/src/grid_arc.rs`, `01ddf1f`) (C28) |
| Place AHU / diffuser, edit type values | Not possible | Works |
| Draw ducts and pipes, size, build an HVAC system | Partial | Works for ducts; vertical runs, types and routing preferences missing |
| Plumbing, fire protection | Abstract nodes only | Unchanged |
| Electrical | Abstract tree | Unchanged, values on the schematic improved |
| Export IFC | Loses rooms, grids, systems | **Unchanged** |

## 5. Remediation, tiered

### 5.1 Easy (days, mostly wiring, existing code is waiting)

> **CLOSED (2026-10-25).** The §5.1 Easy tier (B2, B7–B11, B12–B16, B21–B24) is **closed** (`b17a1ff`, `fccee7b`, `91eb050`, `f84104a`, `7afe07b`).

1. **IFC honesty bundle (B7, B8, B9, B10, B14, B15, B19, B11) — split, not one step.** The premise that "the IFC crate already writes spaces, boundaries, systems and openings; the app just passes empty lists" holds for **spaces, boundaries and openings**, and is wrong for grids and misleading for systems:
   - **Easy (wiring, days).** B7: read `MepData.run_systems`, which the tools already set, instead of hard-coding `System::Other`. B10: call `openbim_place::penetrations::find` / `floors::find_floor` and pass the result. B9-name: the crate **already** writes `IFCDISTRIBUTIONSYSTEM` + `IFCRELASSIGNSTOGROUP` (`ifc/src/lib.rs:1833-1837`) but names the group from the enum as `"<System name> 1"`; pass `MepSystemDef.name` instead.
   - **New writer (not wiring).** B8-grids: `crates/ifc` has **no grid support at all** — `IFCGRID` appears nowhere in the workspace, `grep grid crates/ifc/src/*.rs` is empty, and `export.rs:198` skips `Category::Grid` — so this needs a new `IfcGrid` + `IfcGridAxis` writer. B8-georef needs a project georeference setting the document may not carry.
   - **B11 is sharper than "a counter".** Every export call site passes a hard-coded `project_key = 1` (`model_edit.rs:305` and every test), so two different projects export **identical GUIDs** for equal ids, contradicting the comment at `ifc/src/lib.rs:358`. The fix is a persisted per-project key, not a better counter.
   One acceptance test: export a model with a straight supply duct, rooms and grids, parse the file, count `IFCSPACE`, `IFCGRID`, the system names and openings. Note that this test does not exist yet — `apps/desktop/src/export.rs` has four tests and none of them counts spaces, grids, systems or openings.
2. **Finish B2:** make `plan_level()` return `Option` and refuse tools with "add a level first".
3. **B13, B12, B16:** Compute reads MEP Settings sizes; persist Compute inputs in `.obim`.
4. **B21-B24:** room number/name, level bubbles, parameter tags, pad on equipment.
5. **R4-R6 docs:** README crate table, mark superseded claims, extraction manifest.
6. **Make the test run reproducible offline or in CI**, so the suite can be re-run by anyone. (The `edesign-alike` access this originally asked for turned out not to be the blocker — the pin resolves from the lockfile here — but a CI job on a clean machine would still need it.)

### 5.2 Medium (new feature on current architecture, weeks)

- MEP library to the planned order: VAV, pump, valve, WC/basin, sprinkler, luminaire, panel board, hanger (C3, C4, C15, C16, C18).
- Hosting (C5), spaces and zones with airflow (C6), runs as system members and connector flow direction (C7), Perimeter / Intersection / Custom layout (C8), duct and pipe types (C9), vertical runs (C10), shared snapping (C11), sizing methods (C12), inspector and legend (C13), moving equipment drags runs (C14).
- Electrical as elements with circuits and panel schedules (C17), reading from edesign-alike per decision B.
- Architecture documentation set: dimensions, annotation, ceiling plans, units, reference planes (C19-C29).

### 5.3 Later stage

Massing and toposolid (D1, D2), curved/curtain walls (D3, D4), sketched stairs (D5), link/worksets/phases (D6), family editor (D8), semantic IFC import (D13), undo memory (D12), rendering (D11), fabrication (D7), RVT export (D9, out of scope). **Structure (D10)** stays deferred: when it comes, start with the MEP-support slice only (hangers on beams/slabs, simple truss as an assembly, B19), not the structural analysis stack.

## 6. Risks seen in this review

- **Private dependency now blocks outside builds and review.** Any contributor or CI job without `edesign-alike` access cannot build `openbim-document`. *(Re-checked 2026-10-23: the pin resolves from the local lockfile in this sandbox, so this risk did not reproduce here — the dependency is only a problem for a clean machine with no lockfile.)*
- **Reported test counts were not reproducible here**; they now are, and they agree (see section 2).
- **IFC lag is widening**: MEP content grows (new families, systems) while export still collapses it to `System::Other`.
- Shared `slab_offset_mm` reset (B5) works but is fragile; a new tool that forgets to register defaults will leak again.
- **B11 is worse than "unstable GlobalIds".** Every export call site passes a hard-coded `project_key = 1`, so two projects export identical GUIDs for equal ids — a round-trip between two projects would collide.

## 7. Open owner questions (status)

| Question | Status |
|---|---|
| Disable unbuilt ribbon buttons | Decided and done |
| Electrical calc source | Decided (edesign-alike); dependency pinned; old engine removal to confirm |
| Storage model / oplog | Still open; B20 depends on it |
| First MEP families | Partly answered (AHU, diffuser built); the rest of the order stands |
| Test building (office vs data hall) | Still open |
| ~~**New:** grant sandbox access to `edesign-alike` so tests can be re-run~~ | **Moot.** The pin resolves from the lockfile here, so the suite runs with no extra access (see section 2). |
