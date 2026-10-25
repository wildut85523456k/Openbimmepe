# P0 gap status reconciliation and production backlog (2026-10-25)

> **Status:** PROPOSED — a read-only analysis; nothing here is merged or pushed.
> **Base:** `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be` ("Pipe fittings as families … #96").
> **Scope:** **Reading B** — the register's §5.1 "Easy" tier (B2, B7–B11, B12–B16, B21–B24) plus the docs rows R4–R6.
> **Evidence:** four parallel read-only researchers; reports at
> `docs/agent-team/evidence/gap-p0-2026-10-25/reports/area-{1..4}.md`. Every `file:line` below was
> read at the base SHA; the load-bearing "CLOSED" claims were re-read directly by the coordinator.
>
> **Counting convention:** "open / closed" is measured against each register row's *own* acceptance
> text, read at the base SHA. A row is **closed** when the code at the cited lines satisfies that
> acceptance; a **residual** below is a coverage/follow-on item the row's acceptance does *not*
> require.

## 1. Headline — the §5.1 "Easy" tier is closed

**All sixteen code rows of the §5.1 tier are CLOSED at `main`.** The closing work is in five commits,
every one an ancestor of the base:

| Commit | Closes |
|---|---|
| `b17a1ff` + `fccee7b` | IFC honesty bundle — B7, B8, B9, B10, B11, B14, B15, B19 |
| `91eb050` | Shell blockers — B5, B12, B13, B18, B20–B25 |
| `f84104a` | Save As quick-access button — B16 |
| `7afe07b` | Level tabs named from `document.levels()` — B2 |

The register (`docs/gap-remediation-plan-2026-10-21.md`) was verified at `dcf8382`, which is **112
commits** behind the base (`git rev-list --count dcf8382..b6ef0ee` → 112; `dcf8382` is an ancestor).
The register file did not exist at `dcf8382` (added by `f7436e7`), so its "verified at `dcf8382`"
header refers to the code and the gap analysis, not to the register as it stands. Every register
citation has therefore moved, and the register reads **open** where the code is **closed**.

**Implication.** The P0 "Easy" work is not a backlog to be built — it is a **reconciliation** to be
landed. The real remaining work is (2.1) the stale docs, (2.2) three genuinely open docs rows, and
(2.3) a set of small coverage/follow-on residuals.

## 2. The delta — what is actually left

### 2.1 The register and progress docs are stale — 17 rows

Neither gap doc was touched in the 15-commit window, but the code moved under them. Notable rows
(the full list is `reports/area-4.md` §3):

| Row | Gap doc says | At `b6ef0ee` |
|---|---|---|
| C3 | 7 families | **19** (`crates/family/src/library.rs:481-501`) |
| C5 | no ceiling/face hosting | closed (`crates/family/src/mep.rs:79-83`) |
| C6 | no spaces/zones module | schematic zones exist; MEP spaces still open |
| C16 | sprinkler heads open | closed (`library.rs:495,1182`) |
| C18 | hangers open | closed (`library.rs:498,1363`) |
| C28 | grids cannot be arcs | arc grids exist (`crates/document/src/grid_arc.rs`, `01ddf1f`) |
| R4 count | "19 members (18 crates + desktop)" | **21** (`Cargo.toml:3`) |
| R5-1 | catch-all at `main.rs:499` | now `main.rs:653` |
| R5-2 | Esc = `choose_tool(Select)` | B3 closed |
| R5-3 | "main has no desktop app" | `apps/desktop` is on main |
| R5-6 | "two fixed level tabs" | B2 closed |

### 2.2 Docs rows R4 / R5 / R6 — genuinely open

- **R4 (WRONG / MISSING).** The crate table the row points at (`docs/reference/README.md`) is the
  reference-material index, not the crate table. The real table is `README.md:6-13` and lists **6 of
  21** members — 15 missing (`oplog`, `sketch`, `route`, `ifc`, `view`, `viewport`, `viewport-egui`,
  `photo`, `sheets`, `automation`, `calc`, `document`, `schematic`, `commands`, `apps/desktop`), plus a
  non-member `spikes/*` row. No crate was deleted.
- **R5 (WRONG, partial).** Three app-capability claims are contradicted by code (R5-1/2/3 above);
  one is MISSING-CONTEXT (the IFC-reader four-class claim, `docs/3d-view-parity-plan.md:137`, not
  re-validated class-by-class); four CONFIRMED. Most raw grep hits are the books' own `[book]`
  statements, correctly tagged, not app claims.
- **R6 (MISSING).** `docs/reference/revit-books/` has **no extraction manifest**; **851**
  `[book]`/`[knowledge]` marks carry no source name, SHA-256, tool, page/line convention or
  coverage map. The requirement is described at `docs/reference/README.md:36-37` but not satisfied.

### 2.3 Small residuals inside the closed work

These are **coverage and follow-on items, not open register rows** — every row's own acceptance
passes. Fourteen items, each hours:

| ID | Residual | Row | Key files | Acceptance | Owner |
|---|---|---|---|---|---|
| S-C1 | Grids asserted only on the import path | B8 | `apps/desktop/src/export.rs` (test) | assert `IFCGRID`/`IFCGRIDAXIS` counts on the export path | bim-rust |
| S-C2 | Assemblies asserted only on a hand-built `Scene` | B19 | `apps/desktop/src/export.rs` (test) | one `IFCELEMENTASSEMBLY` + `IFCRELAGGREGATES` on a real model | bim-rust |
| S-C3 | Penetration register + firestops never fed | B10 | `crates/document/src/mep.rs`, `native.rs`, `export.rs`, new `penetration_ui.rs` | firestop columns populated; register round-trips | bim-rust |
| S-C4 | `next_id` reuses a freed maximum | B11 | `crates/document/src/lib.rs`, `native.rs` | new element id > deleted id; GUIDs unchanged | bim-rust |
| S-C5 | 3D slab hole untested for service penetrations | B10 | `crates/view` tests | slab `Shape::Region` has the penetration hole | bim-rust |
| S-C6 | `plan_level()` still returns an `ElementId` sentinel | B2 | `model_edit.rs` + callers | returns `None` from an empty document | bim-rust |
| S-C7 | Dead `"Level 1"`/`"M-101"` literals in the browser | B2 | `apps/desktop/src/panels.rs` | renamed levels show in the browser | bim-rust |
| S-C8 | Last compute result not persisted | B12 | `compute_ui.rs`, `compute.rs` | panel shows the previous run on reopen | bim-rust + doc |
| S-C9 | Electrical cable tables not persisted | B12 | `apps/desktop/src/elec_ui.rs` | CSV tables survive save/reopen | bim-rust |
| S-C10 | Room `Number` unique per level, not global | B21 | `crates/document/src/rooms.rs` | two rooms, same number, different levels → `Err` | doc-crate |
| S-C11 | Datum strip screen-fixed, not model-anchored | B22 | `apps/desktop/src/model_view.rs` | bubbles move with level elevations | desktop |
| S-C12 | Tag parameter hard-coded per `Category` | B23 | `crates/document/src/lib.rs` | tag shows a chosen parameter | doc-crate |
| S-C13 | Pad host rejects in-wall/on-member families | B24 | `crates/document/src/lib.rs` | pad follows an in-wall family | doc-crate |
| S-C14 | Level strip absent from exported plans/sheets | B22 | `apps/desktop/src/plan_export.rs`, `crates/sheets` | exported plan carries a datum line + bubble | sheets |

## 3. Production backlog — spliced small items

Each item is sized for one owner, one review, one test. `S-C*` are the code residuals; `S-D*` are the
docs edits. They carry **no new gap ID** — each references an existing register row (§5).

### 3.1 Docs reconciliation (S-D1 … S-D11)

| ID | Item | Files |
|---|---|---|
| S-D1 | R4 crate table → all 21 members; drop/mark the `spikes/*` row | `README.md:6-13` |
| S-D2 | R4 count "19" → "21" | `docs/gap-remediation-plan-2026-10-21.md:63,70` |
| S-D3 | R5-1 `main.rs:499` → `:653` | `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:56` |
| S-D4 | R5-2 Esc superseded by two-stage Esc | `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md:51` |
| S-D5 | R5-3 "main has no desktop app" → history | `docs/agent-team/gap-review.md:26` |
| S-D6 | R5-6 "two fixed level tabs" → derived from levels | `docs/revit-book-review.md:63` |
| S-D7 | R6 extraction manifest (source, SHA-256, tool, page/line, coverage, copyright) | new `docs/reference/extraction-manifest.md` |
| S-D8 | C3 "7 families" → "19" | `docs/gap-progress-2026-10-23.md:82` |
| S-D9 | C5 / C16 / C18 closed | `docs/gap-progress-2026-10-23.md:84,92,94` |
| S-D10 | C28 arc grids exist | `docs/gap-remediation-plan-2026-10-21.md:180` |
| S-D11 | Scoreboard: MEP closed ≥ 6 | `docs/gap-progress-2026-10-23.md:106-108` |

### 3.2 Code residuals (S-C1 … S-C14)

See §2.3 for the table. Owner = `bim-rust` (or `doc-crate` / `desktop` / `sheets` where noted).
Dependencies: S-C3 depends on S-C1 (same export test module); all others are independent.

### 3.3 Items needing an owner decision before coding

- **S-C10** — room `Number` uniqueness: Revit scopes it to the level in some workflows; confirm the
  intended rule before coding.
- **S-C12** — a chosen tag parameter touches `Command` (the undo surface); flag for review.
- **S-C14** — crosses into `crates/sheets/src`; mark as cross-cutting, not a row expansion.
- **S-C8** — store the last result by value or by `(revision, recompute-on-load)` reference.
- **R5-5** — the IFC-reader four-class claim was not re-validated class-by-class; re-check before
  touching `docs/3d-view-parity-plan.md:137`.

## 4. Ordered build sequence

1. **S-D1, S-D2, S-D7, S-D8–S-D11** — the docs reconciliation, first. It is the cheapest and it makes
   the register trustworthy again; everything else is judged against it.
2. **S-C1, S-C2, S-C4, S-C5** — the acceptance halves of the closed IFC rows (no new behaviour, only
   the missing assertion). Land with their review.
3. **S-C6, S-C7, S-C9** — small shell/persistence follow-ons, independent.
4. **S-C3** — the penetration register + firestops feed (touches model data, `native.rs`, a new UI
   window); the largest residual.
5. **S-C8, S-C10 … S-C14** — the residuals that need an owner decision or a cross-crate change.

## 5. Mapping to existing rows (no parallel ID space)

This document adds no new gap ID. `S-C*` are residuals of existing rows (B2, B7–B11, B12, B13, B16,
B19, B21–B24); `S-D*` are edits to the docs that own those rows (`docs/gap-remediation-plan-2026-10-21.md`,
`docs/gap-progress-2026-10-23.md`, `README.md`, `docs/reference/`). It does not replace either gap
document.

## 6. Coordination — the office-P0 worktree

A parallel workstream runs in `D:/Codex/Revit-alike-P0` (branch `ci/private-dependency-auth`, based on
`b6ef0ee`, +6 commits): the **office** walking-skeleton event journey, a native CI matrix
(`.github/workflows/desktop-p0.yml`), `docs/capability-register.md`, and `docs/p0-office-acceptance.md`
/ `docs/p0-validation.md`. Owner decision 2026-10-25: **reference it as a parallel workstream** — do
not duplicate its capability-register, and keep this document's verification on `main`. Shared-file
boundary: the office work edited `apps/desktop/src/model_view.rs` (+83, tests only); the Reading-B
code (`export.rs`, `native.rs`, `compute_ui.rs`, `rooms.rs`) is untouched there. Note: that worktree's
`p0-validation.md` records that its native CI has not actually run on this host (DNS blocked the
clone), so its matrix is draft, not green.

## 7. What this document is not

- Not a second gap analysis — it is a delta against the existing register and progress report, and
  cites their row IDs.
- Not an implementation, and not authorization to implement any item.
- Not a merge or a push. Nothing here leaves this branch without the owner's approval.

## 8. Verification appendix

| Area | Rows | Closed | Open | Residuals | Report |
|---|---|---|---|---|---|
| 1 — IFC export honesty | B7, B8, B9, B10, B11, B14, B15, B19 | 8 | 0 | 5 (S-C1…S-C5) | `reports/area-1.md` |
| 2 — shell + persistence | B2, B12, B13, B16 | 4 | 0 | 4 (S-C6…S-C9) | `reports/area-2.md` |
| 3 — rooms / tags / hosting | B21, B22, B23, B24 | 4 | 0 | 5 (S-C10…S-C14) | `reports/area-3.md` |
| 4 — docs (R4, R5, R6 + staleness) | 26 | 12 | 14 | 11 (S-D1…S-D11) | `reports/area-4.md` |

Coordinator spot-checks re-read at the base SHA: `apps/desktop/src/export.rs:272`
(`doc.mep().run_system(...)`), `apps/desktop/src/model_view.rs:24-29` (`plan_tabs` from levels),
`apps/desktop/src/ribbon.rs:50` (`save_as`), `crates/document/src/rooms.rs:128-139` (`set_room_data`),
`apps/desktop/src/compute_ui.rs:80` (`U|kind=compute|`). All four closing commits confirmed as
ancestors of `b6ef0ee`; `git rev-list --count dcf8382..b6ef0ee` = 112.
