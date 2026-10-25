# Current capability register

Baseline: `main` at `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`, reviewed 25 October 2026.
This register supersedes current-status claims in the 19-23 October audits; those documents remain historical evidence.
Reference: owner's *Exploring Revit 2026 for Architecture*, 22nd edition, CADCIM (2025).
Chapter mapping is a coverage assessment, not a claim that every tutorial was exercised.

## Evidence rules

- **Implemented (source)**: a reachable implementation was inspected; not a new runtime PASS.
- **Partial**: usable subset with explicit exclusions.
- **Open**: required workflow has no verified implementation.
- **Branch-only**: not established on the baseline. An open PR does not prove absence from main.
- **Blocked validation**: the check could not execute. Never count this as PASS or product failure.
- A passing test must name its candidate SHA, platform, command and result. Source inspection is separate.
- Every future status change updates this register and links the acceptance evidence.

Paths below refer to the baseline unless marked candidate. No tests were rerun during the source review.

| ID / book | Workflow | Status | Source / existing test evidence | Remaining acceptance |
|---|---|---|---|---|
| P0-UI / ch1-2 | New project, tools, shortcuts, level views | Partial | `apps/desktop/src/main.rs`, `model_view.rs`; empty-canvas and global-key tests | Complete office journey through real controls, including narrow-window behaviour |
| P0-SAVE / ch2 | Save, reopen, crash recovery | Implemented (source) | `file.rs`, `journal.rs`; saved-model, recovery and torn-write tests | Native OS execution and abrupt-process recovery evidence |
| A-WALL / ch3 | Sketch and edit typed walls | Partial | `model_edit.rs`, `wall_shapes.rs`, `crates/document/src/layers.rs` | Level/top constraints, attach, wrapping/priority joins, non-faceted curves, advanced wall kinds |
| A-HOST / ch4 | Doors/windows and openings | Partial | `model_edit.rs`, `crates/document/src/opening.rs` | Wider hosts, placement controls, face/dormer openings |
| A-MOD / ch5 | Modify and clipboard | Partial | `modify_tools.rs`, `crates/document/src/modify.rs` | Split, arrays, groups, selection filters and broader trim workflows |
| A-VIEW / ch6 | Levels, grids, plans, sections | Partial | `view_ui.rs`, `section_ui.rs`, `crates/document/src/grid_arc.rs` | Work/reference planes, crop/scope, templates/filters, dependent and reflected-ceiling views |
| A-ENV / ch7 | Floors, roofs, ceilings, rooms | Partial | `model_edit.rs`, document roof/rooms modules | Shape editing, richer roof tools, associative boundaries |
| A-STAIR / ch8 | Stairs and railings | Partial | document stair/railing modules | Ramps, winders, sketched stairs, richer railing/host and curtain-system support |
| A-SITE / ch9 | Site modelling | Open (georeference subset exists) | `georef_ui.rs`, IFC georef module | Toposolid, grading, contours and property lines |
| A-FAM / ch10 | Family authoring | Partial | `family_types_ui.rs`, family crate, Load Family in `model_edit.rs` | Graphical editor, constraints and flex/host acceptance; massing |
| D-DIM / ch11 | Dimensions and tags | Partial | `view_notes.rs`: view dimensions store coordinate pairs | Persistent geometry references, update/undo tests, other dimension kinds, text/keynotes |
| D-SCHED / ch12 | Schedules and detailing | Partial | `schedule_ui.rs`, document schedule module | Calculated fields, takeoffs, keys, drafting/callout views and regions |
| D-SHEET / ch13 | Sheets and export | Partial | `sheet_ui.rs`, `sheet_draw.rs`, `print_dialog.rs`, sheets crate | Revisions, title-block families, drawing fidelity and actual company standards |
| A-3D / ch14-15 | Model viewing and presentation | Partial | `view3d.rs`, viewport and viewport-egui crates | Full navigation/camera workflows, rendering and walkthrough acceptance |
| C-COLLAB / ch16 | Shared-folder collaboration | Open | UI specification and COLLAB-001..003 | Two writers, ownership, conflicts, linked read-only models and crash recovery |
| D-PURGE / ch16 | Purge unused | Open | `docs/agent-team/purge-unused.md` | Scan/apply/undo with reference-chain and stale-scan validation |
| X-IFC | IFC export | Partial, advanced since 23 Oct | `export.rs`, `model_edit.rs`: rooms, grids, named systems, penetrations, georef and persisted project key | Independent external reader and supported-entity coverage; no full interoperability claim |
| X-IMPORT | External IFC import | Partial | `import.rs`: rooms, hosted fillers, grids, selected families, limited stairs and generic carriers | External roof/flight/landing/brep coverage; ports and connections; diagnostics |
| API-CMD | Common UI/API execution | Partial | commands crate; issues #75/#76 | Registry consumers and unambiguous stable parameter IDs |

## Corrections to historical reports

- `gap-progress-2026-10-23.md` is not current: exported rooms/grids/georeference, system naming, persisted project keys,
  room metadata and autosave/recovery have subsequent implementations. Its old scoreboard must not be reused.
- `revit-book-review.md` has conflicting redo claims and obsolete family-loading/level-tab claims.
- `apps/desktop/README.md` previously said edits disappear on exit and persistence/IFC were future work; corrected here.
- PR #48's description says main has no sheets crate; main contains it. Its base is the branch `m1c-sheets-printing`, not `main` (20 files, mergeable): re-target or rebase onto main and review the actual diff before any merge.
- `crates/sheets/src/model.rs` already defines revisions and revision clouds plus revision title-block fields. The Open item for ch13 is UI, title-block and PDF wiring, not the model (wiring not checked).
- The 19 Oct CI failures (every run, zero steps executed) were GitHub refusing to start the jobs over failed account payments or a spending limit. They are not code failures and prove nothing about the build. Billing was reported fixed on 25 Oct; at that time a dispatched run was still queued with no jobs, so it is unverified.
- Separately, `crates/document` pins two git dependencies from the private `edesign-alike` repo; `cargo metadata` returns HTTP 401 without credentials, so no hosted run can pass until `P0_DEPENDENCY_READ_TOKEN` (or equivalent) exists.
- Free-tier note: `ci.yml` and `printing-regressions.yml` now run only on pull requests into `main` and on manual dispatch, not on every push.
- Actions minutes (25 Oct): the account's included minutes for the current cycle are used up; the `desktop-p0` push run on `930fb12` and the PR's `ifc-checks` job were refused before starting (GitHub annotation: payments/spending limit). That is BLOCKED validation, not a failure. To stretch the allowance, `desktop-p0` no longer runs on push to `main` (the PR run already tests the merged tree) and its macOS job (billed 10x) runs only on manual dispatch. Until the next cycle, record local runs (SHA, OS, command, result) here.
- The 23 October office fixture report's external-import limitations were partly superseded on 24 October.
- The P0 candidate places walls, floor, room and section through window pointer events; the small-office regression now
  PASSES on the candidate (1,349/1,349, room 24.0 m²). The remaining gap is that the app still does not draw the room
  outline to the model as a pickable curve between its wall corners (the room lives in document metadata), matching Revit's
  behaviour where picking the room in the plan is not itself modelled.
- New projects create two levels and tools guard missing levels in `choose_tool`; the internal
  `plan_level()` fallback to zero alone is not proof that normal user placement accepts a missing level.

## P0 gates and remaining work

| Gate | Candidate deliverable | Current evidence |
|---|---|---|
| P0-1 status | This register, refreshed entry points, historical notices | Source review complete |
| P0-2 native build | `.github/workflows/desktop-p0.yml`: Linux, Windows, Apple Silicon macOS debug/release tests and release executable | PASS on GitHub-hosted runners: [run 40193462682](https://github.com/wildut85523456k/revit-alike/actions/runs/40193462682) (pull_request, PR #101 head `8ff4b30`, 25 Oct). Ubuntu 24.04, Windows Server 2022 and macOS 14 each passed the office pointer regression, desktop debug tests, workspace release tests, Clippy `-D warnings` and the release build. `8ff4b30` already contained main (`2ce7c0e`), and its tree (`ccef302`) is identical to merged main `930fb12`. Per-test counts and toolchain versions are in the job logs, not copied here. Not a native-window startup, file-dialog or performance check. |
| P0-3 geometry UI | Existing small-office regression converted to pointer geometry placement | Candidate `56d7717` (Windows 11, rustc 1.99.1, cargo 1.99.1): `cargo test --workspace` (debug) 1,349/1,349 pass (0 failed) incl. the small-office pointer regression (room 24.0 m²); `cargo clippy --workspace --all-targets -- -D warnings` clean. Gated by PR #98 pre-push hook: `cargo test --workspace --release` + clippy + audit passed on ad1d493. |
| P0-4 full office | Two-storey authoring, hosted edits, annotation, schedule, sheet, PDF/IFC through real controls | OPEN; see `p0-office-acceptance.md` |

Native startup, visual parity and performance are separate gates; compilation and headless egui tests do not close them.
