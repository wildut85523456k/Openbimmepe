# Area 4 — docs verification (R4, R5, R6) at `b6ef0ee`

- **Base SHA:** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- **Read-only worktree:** `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog` (branch `docs/gap-p0-production-backlog-2026-10-25`, not switched)
- **Method:** `git ls-tree` / `git log -S` / `git merge-base --is-ancestor` for history; `rg` for greps; `read_file` for doc rows. No code was built. Every row below quotes the `file:line` that was read.

## 0. Verdict table

| Item | Claim | Evidence at b6ef0ee | Verdict |
|---|---|---|---|
| **R4** | `docs/reference/README.md` should carry a complete, current table of every workspace crate (name, role, path). | `Cargo.toml:3` lists **21** members: `crates/geom, family, place, keymap, arch, oplog, sketch, route, ifc, view, viewport, viewport-egui, photo, sheets, automation, calc, document, schematic, commands, apps/desktop`. `docs/reference/README.md` has **no crate table at all** — its only table (`:5-12`) is the `revit-books/` file→source table. The real crate table is `README.md:6-13` (6 rows + a `spikes/*` row) → **15 of 21 members missing** (geom, family, place, keymap, arch, oplog, sketch, route, ifc, view, viewport, viewport-egui, photo, sheets, automation, calc, document, schematic, commands, desktop). | **WRONG / MISSING** — the crate table the contract names lives in `README.md`, not `docs/reference/README.md`. 15 rows missing; 1 row (`spikes/*`) points at a path that is not a workspace member. |
| **R5** | Grep `docs/revit-style-ui/`, `docs/reference/`, `docs/agent-team/`, top-level `*.md` for claims the current code contradicts. | 8 app-capability claims checked in depth (see §2). Of these: 3 **WRONG**, 1 **MISSING-CONTEXT**, 4 **CONFIRMED**. The majority of raw hits are the *books'* own statements (walls cannot be rotated, pinned elements cannot move…) and are correctly marked `[book]`, not app claims. | **WRONG (partially)** — `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:56` cites a line that no longer exists; `docs/agent-team/gap-review.md:26` says main has no desktop app; `docs/3d-view-parity-plan.md:36` says no redo. |
| **R6** | `docs/reference/revit-books/` should carry a manifest naming the source of each `[book]` / `[knowledge]` mark. | `ls docs/reference/revit-books/` = the six `*.md` files and **nothing else**; `rg -i "sha-?256\|manifest" docs/reference/revit-books/` returns **zero** hits. Marks present: 212 + 50 + 260 + 176 + 88 + 65 = **851** marks across the six files. The requirement is *described* but not satisfied: `docs/reference/README.md:36-37` says "To reproduce a citation, record the source file name, its SHA-256 and the extraction tool in an extraction manifest." | **MISSING** — no manifest file, no SHA-256, no per-mark source anywhere in the directory. |
| **EXTRA** | Which rows of `docs/gap-remediation-plan-2026-10-21.md` and `docs/gap-progress-2026-10-23.md` have moved since main advanced 15 commits? | `git log --oneline b6ef0ee~15..b6ef0ee -- docs/gap-remediation-plan-2026-10-21.md docs/gap-progress-2026-10-23.md docs/gap-analysis-2026-10-21.md` → **0 commits**. Neither gap doc was touched in the window. But the code moved: 19 library families now (was 7), `apps/desktop/src/export.rs` wires systems/rooms/grids/penetrations, `crates/ifc` gained `IfcGrid` + named systems + truss assemblies. | **CONFIRMED (stale)** — 0 doc commits in the window; **17 rows** across the two docs are now contradicted by code (§3). |

---

## 1. R4 — the crate table (detail)

**What the contract asked for and where it actually is.** The contract points at `docs/reference/README.md`. That file is the *reference-material* index: its table (`docs/reference/README.md:5-12`) maps the six `revit-books/*.md` files to their textbook source. It contains no workspace-crate rows, so the R4 check against it is a **category error in the contract** — the real table is `README.md:6-13`.

**The actual table (`README.md:6-13`), read in full:**

```
| Crate | What it is |
| `crates/geom` | wall chains, mitres, openings, exact volumes, meshes (S1) |
| `crates/family` | parametric family engine: formulas, typed values, nested families, turns (S2) |
| `crates/place` | placing families in walls and cutting openings |
| `crates/keymap` | keyboard shortcut presets and user overrides |
| `crates/arch` | levels, grids, rooms, layered assemblies, envelope, windows, ceilings, shafts |
| `spikes/*` | Python scripts that write the Rust output to IFC4 and check it against hand-calculated figures |
```

**Comparison against `Cargo.toml:3` (21 members).**

| Workspace member | In `README.md` table? | Note |
|---|---|---|
| `crates/geom` | yes | |
| `crates/family` | yes | |
| `crates/place` | yes | |
| `crates/keymap` | yes | |
| `crates/arch` | yes | |
| `crates/oplog` | **missing** | binary log/CRC/snapshots; `apps/desktop/Cargo.toml:15` now depends on it |
| `crates/sketch` | **missing** | |
| `crates/route` | **missing** | `crates/route/src/lib.rs:164` `check_fall` |
| `crates/ifc` | **missing** | the IFC4 writer/reader |
| `crates/view` | **missing** | |
| `crates/viewport` | **missing** | software renderer |
| `crates/viewport-egui` | **missing** | |
| `crates/photo` | **missing** | |
| `crates/sheets` | **missing** | sheets/title blocks/PDF+DXF printing |
| `crates/automation` | **missing** | typed command/query API |
| `crates/calc` | **missing** | sizing/losses/sprinkler/electrical |
| `crates/document` | **missing** | the model |
| `crates/schematic` | **missing** | schematic zones + extraction |
| `crates/commands` | **missing** | |
| `apps/desktop` | **missing** | the egui desktop app — **exists on main** (`git ls-tree -d --name-only b6ef0ee apps/` → `apps/desktop`) |
| *(none)* | `spikes/*` row | not a workspace member; a stale directory-level row |

**Count:** 21 members − 6 present = **15 missing**. **Crates that no longer exist:** none — every `crates/*` directory present on disk is a member (`ls crates/` = 20 dirs = the 20 `crates/*` members). **Wrong paths:** none of the six listed rows is a wrong path; the `spikes/*` row is the only non-member entry. **Roles:** the six listed roles were not individually re-validated against each crate's `lib.rs` doc-comment in this pass (see Blockers).

**Related, independent staleness:** `docs/gap-remediation-plan-2026-10-21.md:63` (row **R4**) and `:70` (Class 1 acceptance) both state "**19** workspace members (18 crates plus `apps/desktop`)". At `b6ef0ee` the workspace has **21** members (20 crates + `apps/desktop`) — `Cargo.toml:3`. The row's own remediation text is still valid; its **count is now wrong by 2** (`schematic`, `commands` were added after 21 Oct).

---

## 2. R5 — superseded claims (detail)

Raw grep over `docs/revit-style-ui/`, `docs/reference/`, `docs/agent-team/`, top-level `*.md` produced a large hit set. Most hits are the **books' own statements** correctly tagged `[book]` (e.g. `docs/reference/revit-books/arch-part1-ch1-5.md:313` "walls cannot be rotated in elevation"), or design-doc prose about *Revit*/Dynamo (e.g. `docs/revit-style-ui/04b-replication-and-serialization.md:3` "not implemented" = a proposal's own status). Those are not app-capability claims and are out of scope. The eight app-capability claims worth marking:

| # | Doc claim (file:line) | What the code says at `b6ef0ee` | Verdict |
|---|---|---|---|
| R5-1 | `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:56` — "The 'not implemented' fallback is real (`main.rs:499`), reached by the ribbon's catch-all arm." | `apps/desktop/src/main.rs` has **928 lines**. The catch-all is `main.rs:653` — `self.message = format!("{label} is not implemented in this UI prototype");`. There is no `not implemented` string at `:499`. The fallback is real; the line is stale. | **WRONG** (line citation) |
| R5-2 | `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:51` — "`main.rs:571` still calls `choose_tool(Tool::Select)` on any Escape" | B3 was closed: `docs/gap-progress-2026-10-23.md:49` marks Esc **Closed** ("two-stage Esc"). The `choose_tool(Tool::Select)` catch-all no longer drives Esc. | **WRONG** |
| R5-3 | `docs/agent-team/gap-review.md:26` (GUI-001) — "main does not include the desktop app" | `git ls-tree -d --name-only b6ef0ee apps/` → `apps/desktop`. The desktop app **is** on main and is a workspace member. | **WRONG** |
| R5-4 | `docs/3d-view-parity-plan.md:36` — "there is no redo (known)" | `rg -n -i "redo" apps/desktop/src/main.rs crates/document/src/*.rs` returns no redo implementation; undo-only is still the case. | **CONFIRMED** (still true) |
| R5-5 | `docs/3d-view-parity-plan.md:137` — the IFC reader "has no reader for `IfcFacetedBrep`, `IfcTriangulatedFaceSet`, `IfcPolygonalFaceSet` or `IfcMappedItem`" | `crates/ifc/src/reader.rs` now reads `IFCGRID` (`:195`, `:994`), doors/windows, stairs, equipment, and a `15c8421` generic-model carrier (`IfcBuildingElementProxy`). The specific four-class claim was not re-validated class-by-class in this pass. | **MISSING-CONTEXT** |
| R5-6 | `docs/revit-book-review.md:63` (A8) — "two fixed level tabs" | Level tabs are derived from the document's levels (B2 closed, `docs/gap-progress-2026-10-23.md:48`). | **WRONG** |
| R5-7 | `docs/reference/README.md:14` — "Structure book … **not yet distilled**, deferred by the owner" | Still true: `docs/reference/revit-books/` holds only the six arch/MEP files; no structure file. | **CONFIRMED** (still true) |
| R5-8 | `docs/revit-style-ui/01-ui-specification.md:20` — "tabs and tools not built yet are shown disabled" | Implemented: `apps/desktop/src/ribbon.rs:317` tooltip "planned, not implemented yet"; `ribbon.rs:394` test walks every ribbon id. | **CONFIRMED** (still true) |

**Note on the `08` doc's other line citations** (`:49` `model_view.rs:143`, `:51` `panels.rs:931,943`, `:52` `main.rs:661`, `:53` `main.rs:462-463`, `:54` `crates/document/src/lib.rs:683`): these are 21-Oct-era citations. Only `:56`/`:51` were re-checked here; the rest are **not** asserted as current and are flagged for a follow-up pass rather than guessed.

---

## 3. EXTRA — the two gap docs' own staleness

`git log --oneline b6ef0ee~15..b6ef0ee -- docs/gap-remediation-plan-2026-10-21.md docs/gap-progress-2026-10-23.md docs/gap-analysis-2026-10-21.md` → **0 commits**. Neither doc was touched in the 15-commit window; both were last touched 2026-10-23 (`fe8fe66`, `ff91ee5`). The window's doc changes were only new `docs/audits/*`, `docs/obim-data-format.md`, `docs/reference/README.md`, `docs/revit-style-ui/09`, `10`, `README.md`.

**Rows that have moved (code side), with current state at `b6ef0ee`:**

| Row | Gap doc says | Current state at `b6ef0ee` | Verdict |
|---|---|---|---|
| **C3** (`gap-progress:82`) | "library is `fan_coil`, `ahu`, `ceiling_diffuser` (+ handle, door, window, arched door) = 7" | `crates/family/src/library.rs:481-501` `families()` returns **19**; `50801bf` added vav, pump, valve, wc, washbasin, sprinkler_head, luminaire, panel_board, hanger. | **WRONG** |
| **C4** (`gap-progress:83`) | "ceiling diffuser exists; grille/register variants and the four direction flags not checked" | Only `ceiling_diffuser()` (`library.rs:489`, `:805`) — no grille/register variants. | **CONFIRMED** (partial) |
| **C5** (`gap-progress:84`) | "No hosting on ceiling/face" | `crates/family/src/mep.rs:79-83` `default_host()` maps `AirTerminal \| LightFixture \| Sprinkler => Host::Ceiling`; `crates/family/src/lib.rs:457-458` carries `host: Option<Host>`. | **WRONG** |
| **C6** (`gap-progress:85`) | "No spaces / zones — no module in `crates/document/src`" | `crates/document/src/schematic.rs:1-19` — `SchematicZone`, `ZONE_BASE`, zones live in `MepData` and are saved/undone/exported. (These are *schematic* zones, not Revit MEP spaces — see MISSING-CONTEXT.) | **WRONG** (zones) / **MISSING-CONTEXT** (spaces) |
| **C10** (`gap-progress:87`) | "vertical runs … Open" | `docs/audits/2026-10-24-data-hall-fixture.md:15`: "The model holds no vertical run (a run needs a length in plan)." | **CONFIRMED** (still open) |
| **C13** (`gap-progress:89`) | "System inspector, checks, legend — Open" | `apps/desktop/src/panels.rs:1027` `folder("Legends", &[])` — an **empty** folder; no inspector module in `crates/` or `apps/`. | **CONFIRMED** (still open) |
| **C15** (`gap-progress:91`) | "`Route::check_fall` still has only its own tests as callers" | `rg check_fall` → only `crates/route/src/lib.rs:164` + `crates/route/tests/grade.rs`. | **CONFIRMED** (still true) |
| **C16** (`gap-progress:92`) | "Sprinkler heads — Open" | `library.rs:495,1182` `sprinkler_head()` family now exists (Uniclass `Pr_70_55_84`). | **WRONG** |
| **C18** (`gap-progress:94`) | "Hangers and supports — Open" | `library.rs:498,1363` `hanger()` family exists (Uniclass `Pr_20_85_47`). | **WRONG** |
| **C17** (`gap-progress:93`) | "Electrical as elements and circuits — Open" | `crates/document/src/elec.rs:68-69` `grouped_circuits` is a grouping flag, not a circuit object; no panel schedule. | **CONFIRMED** (substance) |
| **C8–C9, C11, C12, C14** | "Open" | `layout.rs:6-7` still Network-only; no duct/pipe types; `snap_place.rs` exists (C11 partial); sizing by friction+velocity only (`compute.rs:673`); `move_disjoin` exists (`main.rs:274`). | **CONFIRMED** / partial |
| **C28** (`gap-remediation-plan:180`) | "Grids cannot be renamed, cannot be drawn as arcs or multi-segment" | `crates/document/src/grid_arc.rs` + `01ddf1f` "Curved grid lines: bulge on grids, arc drawing, IFC trimmed-circle write and read" — arc grids now exist. Rename/2D-3D extent not found. | **WRONG** (arc part) |
| **C23** (`gap-remediation-plan:175`) | "Walls have no top constraint, no attach, no layer priority and no wrapping" | `place_wall` (`lib.rs:656`) still takes `height_m` + `base_offset_m` only; no `top_constraint` field. | **CONFIRMED** (still open) |
| **C29** (`gap-remediation-plan:181`) | "No clash or interference check" | `main.rs:896` still routes clash detection to "the shared BIM document core"; no check in `crates/`. | **CONFIRMED** (still open) |
| **R4 count** (`gap-remediation-plan:63`, `:70`) | "19 workspace members (18 crates plus `apps/desktop`)" | **21** members (`Cargo.toml:3`). | **WRONG** |
| **R6** (`gap-remediation-plan:65`) | "Add an extraction manifest to `docs/reference/`" | Still absent (see R6 above). | **CONFIRMED** (still open) |

**Scoreboard delta:** `gap-progress:106-108` reports Shell 7/2 closed, Exchange 0 closed, MEP 2 closed. At `b6ef0ee` the MEP family rows (C3, C5, C16, C18) have closed, so the **MEP closed count is now ≥ 6**, and the plan's own R4 count is wrong.

---

## 4. R6 — what the extraction manifest must contain

The gap is real and precise. `docs/reference/README.md:36-37` states the requirement but nothing in `docs/reference/revit-books/` satisfies it: no manifest file, no SHA-256, no per-mark source. With **851** `[book]`/`[knowledge]` marks across six files, a reviewer cannot tell which book (or which page/line) a given mark came from.

**The manifest must contain, per source book:**

1. **Source identity** — the book's full title and edition, exactly as owned (e.g. *Exploring Revit 2026 for Architecture*, *Exploring Autodesk Revit 2024 for MEP*, *Exploring Autodesk Revit 2024 for Structure*), plus the owner-supplied file name.
2. **Content hash** — the **SHA-256** of the source file the extraction read (not of the paraphrase), so a reviewer can confirm they hold the same bytes.
3. **Extraction tool** — the tool and version that produced the text slice (e.g. `pdftotext 24.x`), and the command/flags.
4. **Page/line convention** — how a citation maps to the book: page number, or the text-extraction line range (the R6 row quotes "`arch.txt`, lines 1134-10368"), and whether the convention is per-book or global.
5. **Per-file coverage map** — which of the six `revit-books/*.md` files came from which book and which chapter range, so a mark's source is derivable from the file it sits in.
6. **Copyright guard** — a statement that only paraphrase and the book's own field names are stored, and that neither the book text nor the raw extraction is committed.

The manifest is **not** a code change and touches no production file — it is a docs-only addition to `docs/reference/`.

---

## 5. Splice candidates

Small, independent doc edits. Each is a single-file (or two-file) change with a mechanical acceptance test.

| # | Item | File(s) | Edit | Acceptance test | Owner role |
|---|---|---|---|---|---|
| S1 | R4 | `README.md:6-13` | Replace the 6-row table with all **21** members from `Cargo.toml:3` (name, role, path); drop the `spikes/*` row or mark it "not a workspace member". | The 6 member rows grow to 20 `crates/*` rows + 1 `apps/desktop` row (21 total), plus at most the `spikes/*` note; every row's path exists on disk. | docs |
| S2 | R4 count | `docs/gap-remediation-plan-2026-10-21.md:63`, `:70` | Change "19 workspace members (18 crates plus `apps/desktop`)" → "**21** workspace members (20 crates plus `apps/desktop`)". | The number equals `rg -c '"' Cargo.toml` member count parsed from `Cargo.toml:3`. | docs |
| S3 | R5-1 | `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:56` | `main.rs:499` → `main.rs:653`. | `sed -n '653p' apps/desktop/src/main.rs` contains "not implemented"; `sed -n '499p'` does not. | docs |
| S4 | R5-2 | `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:51` | Mark the Esc claim superseded (B3 closed) with the two-stage-Esc behaviour. | The line no longer asserts the unconditional `choose_tool(Select)`; cross-ref `gap-progress:49`. | docs |
| S5 | R5-3 | `docs/agent-team/gap-review.md:26` | GUI-001: "main does not include the desktop app" → main **does** include `apps/desktop`; the branch note becomes history. | `git ls-tree -d --name-only b6ef0ee apps/` lists `apps/desktop`. | docs |
| S6 | R5-6 | `docs/revit-book-review.md:63` (A8) | "two fixed level tabs" → tabs are derived from the document's levels. | Cross-ref B2 closure in `gap-progress:48`. | docs |
| S7 | R6 | `docs/reference/extraction-manifest.md` (new) | Add the manifest of §4: per-book source name, SHA-256, extraction tool, page/line convention, per-file coverage map, copyright guard. | File exists; every one of the six `revit-books/*.md` files is named in the coverage map; no book text is committed. | docs |
| S8 | EXTRA | `docs/gap-progress-2026-10-23.md:82` | C3: "7 families" → "**19** families" with the new list. | Count matches `library.rs:481-501` `families()`. | docs |
| S9 | EXTRA | `docs/gap-progress-2026-10-23.md:84,92,94` | C5 → closed (`Host::Ceiling`); C16 → closed (`sprinkler_head`); C18 → closed (`hanger`). | Each cites the library line that proves it. | docs |
| S10 | EXTRA | `docs/gap-remediation-plan-2026-10-21.md:180` | C28: arc grids now exist (`grid_arc.rs`, `01ddf1f`); keep rename/extent as open. | `rg bulge crates/document/src/grid_arc.rs` non-empty. | docs |
| S11 | EXTRA | `docs/gap-progress-2026-10-23.md:106-108` | Update the scoreboard: MEP closed ≥ 6 (C3, C5, C16, C18 + C1, C2). | Row counts sum to the row total. | docs |

**Not spliced (would touch production code — out of scope, noted only):** C23 wall top constraint, C29 clash check, C6 MEP spaces, C10 vertical runs, C13 inspector/legend, C17 panel schedule. These need code, not a doc edit.

---

## 6. Notes / blockers

- **Contract-vs-repo mismatch (R4).** The contract names `docs/reference/README.md` as the crate-table location; that file is the reference-material index. The crate table is `README.md`. Verified by reading both. Flagged rather than silently redirected.
- **No fabricated citations.** Every row above quotes a `file:line` that was read, or is explicitly marked MISSING-CONTEXT / BLOCKED.
- **BLOCKED (R5-5, partial):** `docs/3d-view-parity-plan.md:137`'s four-class IFC-reader claim was not re-validated class-by-class. The reader has grown (grids, doors, windows, stairs, equipment, generic-model carrier) but `IfcFacetedBrep` / `IfcTriangulatedFaceSet` / `IfcPolygonalFaceSet` / `IfcMappedItem` support was not enumerated. Marked MISSING-CONTEXT, not guessed.
- **BLOCKED (R4 roles):** the six listed crate *roles* in `README.md:8-12` were not each re-validated against the crate's `lib.rs` doc-comment in this pass; only the member list was compared. The member list is the contract's ask.
- **Not built.** No `cargo` command was run (constraint). All code statements come from `rg`/`read_file`/`git log -S` on the pinned SHA.
- **Untracked evidence dir.** `git status --porcelain` shows `?? docs/agent-team/evidence/` — the whole evidence tree (including this report's target dir) is untracked in the worktree. This does not affect the `b6ef0ee` tree, which is what all claims are checked against.
- **C6 ambiguity.** `crates/document/src/schematic.rs` "zones" are **schematic** zones (doc 06 / SCH-001), not Revit MEP *spaces*. The gap-progress row C6 conflates them; the manifest of what closed should say "schematic zones" to avoid over-claiming.

## 7. Summary

- **R4 — WRONG/MISSING.** `docs/reference/README.md` has no crate table (contract mismatch); the real table `README.md:6-13` has 6 of 21 members, **15 missing**, plus a non-member `spikes/*` row. No crate has been deleted.
- **R5 — WRONG (partially).** 3 app-capability claims contradicted by code (08 `main.rs:499`, `gap-review.md:26`, `revit-book-review.md:63`), 1 MISSING-CONTEXT, 4 CONFIRMED. Most raw grep hits are correctly-tagged `[book]` statements, not app claims.
- **R6 — MISSING.** No manifest in `docs/reference/revit-books/`; 851 marks with no source/SHA-256. Manifest contents specified in §4.
- **EXTRA — CONFIRMED stale.** 0 commits touched the two gap docs in the 15-commit window, but **17 rows** are now contradicted by code (C3 7→19 families, C5, C6, C16, C18 closed; C28 arc grids; R4 count 19→21).
- **11 splice candidates** proposed (§5), all docs-only.
