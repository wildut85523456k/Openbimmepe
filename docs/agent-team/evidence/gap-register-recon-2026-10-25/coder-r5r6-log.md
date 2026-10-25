# Coder log — R5 stale-claim marks + R6 extraction manifest

**Role:** coder (apply, then commit). **Worktree:** `wt-gap-register-recon`, branch
`docs/gap-register-reconciliation-2026-10-25`, HEAD `b50f93c` (merge-base `b6ef0ee`).
**Authoritative source:** `docs/agent-team/evidence/gap-register-recon-2026-10-25/research-r5r6.md`
(read-only researcher report, verified against `main` @ `b6ef0ee`).

Do **not** push. Do **not** touch the office P0 worktree `D:/Codex/Revit-alike-P0`.

## What was done

1. Applied the six R5 verdicts as inline **"superseded 2026-10-21"** marks at each claim's source,
   keeping the original wording (nothing deleted), per R5's remedy: *"Keep the historical value:
   mark the superseded statement 'superseded 2026-10-21' rather than deleting it."*
2. Created the R6 extraction manifest at `docs/reference/extraction-manifest.md`.
3. Wrote this log and committed (no push).

Before marking, the load-bearing code citations were re-confirmed by grep at the worktree HEAD
(all held — see "Citation spot-check" below).

## R5 marks applied (verdict → source → mark)

| Claim | Source (verbatim statement kept) | Verdict | Mark text (abridged) |
|---|---|---|---|
| 1 | `docs/revit-ux-parity-audit.md:3` — "Verified by automated event tests … Not verified side by side with Revit, and not clicked by hand." | **STALE** | event tests now exercise the ribbon (`ribbon_click`, `model_view.rs:1406`), options bar (`mep_settings_ui.rs`), global keys (`key_frame`, `model_view.rs:1416`) |
| 2a | `docs/3d-view-parity-plan.md:21` — "Every drawing and modify tool … is plan-only; picking in 3D selects but cannot move." | **PARTIAL** | marked **partly** superseded: 3D Family placement (`view3d.rs:251`) + Beam path (`model_edit.rs:495`) now exist; modify tools still plan-only, 3D pick still cannot move |
| 2b | `docs/3d-view-parity-plan.md:36` — "there is no redo (known)" / "Redo is a separate missing item." | **PARTIAL (redo half = STALE)** | redo implemented: `main.rs:647`, `main.rs:714` `redo_document`, `ribbon.rs:52`, `crates/oplog/src/lib.rs:520`, tested `crates/oplog/tests/oplog.rs` |
| 3 | `docs/agent-team/gap-review.md:26` (GUI-001) — "main does not include the desktop app." | **STALE** | `apps/desktop` is a workspace member (`Cargo.toml`), 40 tracked files at `b6ef0ee` |
| 4 | `docs/revit-style-ui/06-mep-schematic-automation.md:5` — "it is not part of this candidate." | **STALE** | engine in main: `crates/schematic/` (SCH-002), `crates/document/src/schematic.rs` (`SchematicZone:56`, extraction query `:346`), `schematic_view.rs` `generate_schematic` |
| 5 | `docs/agent-team/gap-review.md:38` (MEP-002) — "PLANNED / UNVERIFIED." | **STILL-TRUE** | **No mark** — the mismatch R5 names still holds (gap-review.md labels MEP-002 PLANNED/UNVERIFIED; `05-backlog.md:30` MEP-002 row has no status column). Left as-is. |
| 6 | `docs/revit-book-review.md:63` (A8) — "two fixed level tabs." | **STALE** | tabs now derived from project levels (`plan_tabs`, `model_view.rs:24`; fixed by `7afe07b`); the new-project IFC-export break it implied is moot |

Mark format, identical on every claim: `**[superseded 2026-10-21 — R5 register: <reason>; <code citation>]**`,
appended after the original statement (paragraph end for Claims 1 and 4; inside the last cell of the
row for the table Claims 2, 3, 6). No pipe characters or newlines were introduced into any table row.

## R6 extraction manifest

Created `docs/reference/extraction-manifest.md`. Records, per R6's required fields:

- **Source file names:** `arch.txt` (Architecture book, CADCIM), `mep.txt` (MEP book, CADCIM, Tickoo).
- **SHA-256:** *not recorded* — must come from the owner's licensed PDF; cannot be derived from the
  paraphrase. Action left for the owner.
- **Tool used:** *not recorded anywhere in the repo.* `pdftotext` is the only in-repo PDF-text tool
  (`spikes/s13_sheets_check/check_sheets.py:35`, `docs/sheets-printing.md:38`) but it is used for
  sheet printing, so it is an **unconfirmed** candidate only.
- **Page/line convention:** line numbers of the extracted text file, not PDF pages.
- **Copyright:** neither the book text nor the extraction is committed; paraphrase only.
- **Citation index:** the five `doc:line` citations that name `arch.txt`/`mep.txt` line ranges.
- **To close R6:** owner supplies the SHA-256 of each licensed PDF and confirms the tool/convention.

The extraction files do not exist in the repo (confirmed: `git ls-tree -r HEAD` has no `arch.txt` /
`mep.txt`; only the two unrelated keymap `.txt` presets).

## Citation spot-check (grep at worktree HEAD)

- `model_view.rs`: `ribbon_click` `:1406`, `key_frame` `:1416`; `mep_settings_ui.rs` options-bar test
  `the_options_bar_and_the_window_show_the_lists_of_the_model` `:187` (asserts "Place Pipe",
  "Automatically Connect", etc.).
- `main.rs`: `"redo" => self.redo_document()` `:647`, `fn redo_document` `:714`; `ribbon.rs:52`
  `("redo", "Redo")`; `crates/oplog/src/lib.rs:520` `pub fn redo`; tests `crates/oplog/tests/oplog.rs`.
- `view3d.rs`: `Tool::Family` placement at `:237,240,251,257`; `model_edit.rs:495`
  `place_beam_from_controls`.
- `apps/desktop/` present; `Cargo.toml` `members` includes `"apps/desktop"`.
- `crates/schematic/src/{lib.rs,layout.rs}`; `crates/document/src/schematic.rs` `SchematicZone:56`,
  "The extraction query of doc 06" `:346`; `crates/document/src/schematic_view.rs` present.
- `model_view.rs:24` `fn plan_tabs`, comment `:23` "first two by *position* … ids 10/11"; commit
  `7afe07b` = `fix(desktop): the view tabs say the project's level names, not hard-coded "Level 1"`.

All held.

## Files changed

- `docs/revit-ux-parity-audit.md` — Claim 1 mark (line 3).
- `docs/3d-view-parity-plan.md` — Claim 2a mark (line 21) and Claim 2b mark (line 36).
- `docs/agent-team/gap-review.md` — Claim 3 mark (GUI-001 row).
- `docs/revit-style-ui/06-mep-schematic-automation.md` — Claim 4 mark (line 5).
- `docs/revit-book-review.md` — Claim 6 mark (A8 row).
- `docs/reference/extraction-manifest.md` — **new** (R6 manifest).
- `docs/agent-team/evidence/gap-register-recon-2026-10-25/coder-r5r6-log.md` — **new** (this log).

Claim 5 (`gap-review.md:38`, MEP-002) deliberately **not** marked (STILL-TRUE).
