# Gap-register reconciliation — R5 (stale claims) + R6 (extraction manifest)

**Researcher (read-only).** Verified against `main` @ **`b6ef0ee`**
(`b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`, 2026-10-24 20:00 +0700).
Evidence read with `git show b6ef0ee:<path>` (tree identical to the worktree
`C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`, whose `HEAD` `b50f93c` has
`b6ef0ee` as its merge-base). No repository file was edited except this report.

**Line-number note.** The register rows are cited two ways:
`docs/gap-remediation-plan-2026-10-21.md:70` (R5) and `:71` (R6) are the line numbers at the
**worktree HEAD** (`b50f93c`, after the coder added the verification-status note). At `b6ef0ee`
the same rows are `:64` (R5) and `:65` (R6). The text is identical.

**Verdicts:** 1 STALE · 2 PARTIAL · 3 STALE · 4 STALE · 5 STILL-TRUE · 6 STALE.

---

## R5 — the six stale-document claims

R5 (verbatim, `docs/gap-remediation-plan-2026-10-21.md:64` @ `b6ef0ee` / `:70` @ HEAD):

> Stale claims in existing documents. The arch audit lists 15 (D1-D15) and the MEP audit 13; the
> recurring ones are: `docs/revit-ux-parity-audit.md` claims event-test verification that does not
> cover the ribbon, options bar or global keys; `docs/3d-view-parity-plan.md` claims no 3D tools
> and no redo; `docs/agent-team/gap-review.md` claims `main` has no desktop app;
> `docs/revit-style-ui/06` claims the schematic engine is not in the candidate;
> `docs/agent-team/gap-review.md` marks `05-backlog.md` MEP-002 "PLANNED/UNVERIFIED" although the
> backlog row carries no status column at all; `docs/revit-book-review.md` A8 mentions the fixed
> level tabs but not that they break IFC export in a new project.

### Claim 1 — revit-ux-parity-audit.md event-test coverage → **STALE**

Source statement (`docs/revit-ux-parity-audit.md:3`, verbatim):

> Standing rule: every tool mimics Revit's user experience. Verified by automated event tests
> (simulated mouse and keys) and by reading the code. Not verified side by side with Revit, and
> not clicked by hand.

R5's claim: that event-test verification does not cover the ribbon, options bar or global keys.

Code at `b6ef0ee` — all three areas **are** covered by event/UI-driven tests:
- **Global keys.** `key_frame` runs `app.global_keys(ctx)` then the viewport through
  `ctx.run_ui` with `egui::Event::Key` (`apps/desktop/src/model_view.rs:1416-1420`). Tests:
  `escape_steps_back_inside_the_wall_tool_before_leaving_it` (`model_view.rs:1426`),
  `the_keymap_shortcuts_choose_a_tool_and_ctrl_n_starts_a_new_model` (`model_view.rs:1446`,
  exercises the keymap "W A" shortcut and Ctrl+N).
- **Ribbon.** `ribbon_click` (`model_view.rs:1406-1413`) sends `PointerMoved` + `PointerButton`
  events through `window_frame` (the whole window). Tests:
  `the_ribbon_starts_a_tool_and_a_button_that_is_not_wired_changes_nothing` (`model_view.rs:1475`)
  and the ribbon-driven office build (`model_view.rs:1494`, ribbon_click for
  level/grid/wall/floor/room/section). `git log -S ribbon_click` → `6979485` "P0 (gap closure):
  the whole window as a method, so a test can drive the ribbon".
- **Options bar.** `the_options_bar_and_the_window_show_the_lists_of_the_model`
  (`apps/desktop/src/mep_settings_ui.rs:188`) draws the window and asserts the options-bar text
  (Place Pipe / Diameter / Automatically Connect / Inherit Elevation / Inherit Size / Tag on
  Placement / Justification / Slope / ▾); `a_level_rename_keeps_its_draft_across_frames_and_commits_on_enter`
  (`model_view.rs:1645`) drives `window_frame` over the options bar.

**Reason:** at `b6ef0ee` event tests exercise the ribbon, the options bar and the global keys, so
R5's "does not cover" no longer holds.

### Claim 2 — 3d-view-parity-plan.md no-3D-tools / no-redo → **PARTIAL**

Source statements (`docs/3d-view-parity-plan.md`, verbatim):

> `:21` | Not available in 3D | Every drawing and modify tool (Wall, Move, Rotate, Mirror, Copy,
> Align, Trim, Offset, Dimension, Tag, Room, Stair, Shaft...) is plan-only; picking in 3D
> selects but cannot move |

> `:36` | Undo / Redo | The document's undo works from 3D; there is no redo (known) | Redo is a
> separate missing item for the whole app |

Code at `b6ef0ee`:
- **Redo exists.** `apps/desktop/src/main.rs:647` `"redo" => self.redo_document()`;
  `main.rs:714-719` `redo_document()`; `main.rs:748-750` Ctrl+Y / Ctrl+Shift+Z; ribbon button
  `ribbon.rs:52`; oplog `redo()` (`crates/oplog/src/lib.rs:520-532`) with tests
  (`crates/oplog/tests/oplog.rs:20-37`). Added by `051f303` "Redo, Pin, Cut/Copy/Paste, Match Type,
  Select All Instances and Create Similar".
- **Some 3D drawing now exists.** `apps/desktop/src/view3d.rs:251-257` — `if app.tool ==
  crate::Tool::Family { … app.family_click(level, at, wall) }` (a click in 3D places a family;
  `view3d.rs:236-240` previews it). `place_beam_from_controls`
  (`apps/desktop/src/model_edit.rs:495`) is wired to the "Place Beam" button
  (`panels.rs:718`) and tested by `a_beam_is_placed_from_the_options_bar_shown_in_3d_and_undone`
  (`model_edit.rs:1549`).
- **Still true.** Only `Tool::Family` is handled in `view3d.rs` (lines 237, 240, 251); the modify
  tools remain plan-only; `view3d.rs:37-40` a click in 3D selects but cannot move.

**Reason:** the "no redo" half is stale (redo is implemented) and the 3D view has gained a Family
placement tool plus a Beam path, so "every drawing and modify tool is plan-only" is no longer
wholly accurate; the modify tools and "picking cannot move" still hold.

### Claim 3 — agent-team/gap-review.md "main has no desktop app" → **STALE**

Source statement (`docs/agent-team/gap-review.md:26`, verbatim):

> | GUI-001 | P0 | egui desktop shell (decided 15 Oct 2026; prototype on `feature/rust-gui`) |
> PROTOTYPE ON BRANCH: origin/feature/rust-gui contains apps/desktop and an expanded workspace;
> main does not include the desktop app. No GUI build or interaction run performed in this review. |

Code at `b6ef0ee`: `apps/desktop/` is present with 40 tracked files (`apps/desktop/src/main.rs`,
`ribbon.rs`, `model_view.rs`, `view3d.rs`, `panels.rs`, …) and `apps/desktop` is a workspace member
(`Cargo.toml` `members = [ …, "apps/desktop"]`).

**Reason:** the desktop app is in `main` at `b6ef0ee`, so "main does not include the desktop app"
is false.

### Claim 4 — revit-style-ui/06 schematic-not-in-candidate → **STALE**

Source statement (`docs/revit-style-ui/06-mep-schematic-automation.md:5`, verbatim):

> Implementation status of this proposal: nothing in this document is implemented or accepted. A
> first engine slice — the zone data model, the assignment rules and the extraction query with its
> JSON dump — exists on the separate local branch `feature/sch-001-schematic-zones` (commit
> `e37996c`); it is not part of this candidate, it is not reviewed here, and no SCH-001 acceptance
> is claimed for it.

Code at `b6ef0ee` — the engine is in `main`:
- `crates/schematic/` (SCH-002 layout engine): `src/lib.rs` + `src/layout.rs` (396 lines, golden
  tests `golden_uniform_from_fixture`, `golden_footprint_from_fixture`, …).
- `crates/document/src/schematic.rs:1` — "Schematic zones and the extraction query behind
  `MEP: Generate Schematic` (doc 06, backlog SCH-001)": `SchematicZone` (`:56`),
  `overlapping_zones` (`:67`), `room_fallback_label` (`:81`), `zone_display_label` (`:98`), "The
  extraction query of doc 06" (`:346`).
- `crates/document/src/schematic_view.rs:243`/`:264` `generate_schematic_commands` /
  `generate_schematic`.

**Reason:** the zone model (SCH-001), the extraction query and the grid layout engine (SCH-002) are
all in `main` @ `b6ef0ee`, so "it is not part of this candidate" is stale.

### Claim 5 — agent-team/gap-review.md MEP-002 PLANNED/UNVERIFIED → **STILL-TRUE**

Source statement (`docs/agent-team/gap-review.md:38`, verbatim):

> | MEP-002 | P1 | Hydraulic/airflow/electrical calculation modules | PLANNED / UNVERIFIED:
> hydraulic, airflow and electrical calculation modules require units-aware reference validations;
> no engineering calculation compliance is claimed. |

R5's claim: gap-review.md marks `05-backlog.md` MEP-002 "PLANNED/UNVERIFIED" although the backlog
row carries no status column.

At `b6ef0ee`, `docs/revit-style-ui/05-backlog.md:30` (verbatim):

> | MEP-002 | P1 | Hydraulic and airflow calculation modules (the electrical calculations are
> imported from edesign-alike by MEP-005, see 06) | MEP-001 | Verified reference calculations
> with units |

Header (`05-backlog.md:5`): `| ID | Priority | Task | Dependency | Acceptance |` — **five columns,
no status column**. And `gap-review.md:17` states the "PLANNED/UNVERIFIED" label is gap-review.md's
own initial assessment.

**Reason:** the mismatch R5 names still exists — gap-review.md:38 labels MEP-002 PLANNED/UNVERIFIED
while the backlog's MEP-002 row has no status column. (Context: `crates/calc` does exist at
`b6ef0ee` with `fluid.rs`/`plumbing.rs`/`sprinkler.rs`/`electrical.rs`, but the backlog acceptance
is "Verified reference calculations with units", so "UNVERIFIED" is not contradicted — the label
is still only gap-review.md's.)

### Claim 6 — revit-book-review.md A8 fixed-level-tabs / IFC-export → **STALE**

Source statement (`docs/revit-book-review.md:63`, verbatim):

> | A8 | **Levels and grids**: Create Plan View check, plan view types, level copy/monitor, story
> elevation display; grid Multi-Segment and Pick Lines, bubbles and 2D/3D extents; **Reference
> Planes and Work Planes** (set work plane, show) | Level and Grid tools exist; two fixed level
> tabs; no reference planes, no work planes | P1 (level tabs, reference planes), P2 (rest) |

Code at `b6ef0ee`: the tabs are **no longer fixed**. `apps/desktop/src/model_view.rs:24-31`
`plan_tabs(levels: &[(ElementId, String, f64)])` derives them from the project's levels by
position; the comment at `:26-27` reads "taking the first two by *position* rather than by the
sample's ids 10/11 is what makes that true (A2)". `show()` uses `plan_tabs(&app.levels())`
(`model_view.rs:90`). Fixed by `7afe07b` "fix(desktop): the view tabs say the project's level names,
not hard-coded "Level 1"".

The doc's line 63 is unchanged and still does not mention an IFC-export break.

**Reason:** the level tabs are derived from the project's levels (ids 10/11 are no longer used),
so "two fixed level tabs" no longer matches the code; the IFC-export consequence R5 points at is
also moot now that the tabs follow the levels.

---

## R6 — extraction manifest data

R6 (verbatim, `docs/gap-remediation-plan-2026-10-21.md:65` @ `b6ef0ee` / `:71` @ HEAD):

> The reference docs cite "Text extraction `arch.txt`, lines 1134-10368" but the extraction itself
> is not in the repository, so the line references cannot be reproduced by a reviewer. | Add an
> extraction manifest to `docs/reference/`: the source file name, its SHA-256, the tool used, and the
> page/line convention. Do **not** commit the book text or the extraction - the books are
> copyrighted and only paraphrase may be stored. | `docs/reference/` | small |

### Citations found (file:line, all at `b6ef0ee`)

| File | Line | Text |
|---|---|---|
| `docs/reference/revit-books/arch-part1-ch1-5.md` | `:3` | Source: "Exploring Revit 2026 for Architecture" (CADCIM). Text extraction `arch.txt`, lines 1134-10368. |
| `docs/reference/revit-books/arch-part1-ch1-5.md` | `:5` | \| Chapter \| Title \| Approx. line range in arch.txt \| |
| `docs/reference/revit-books/arch-part2-ch6-10.md` | `:3` | Source: "Exploring Autodesk Revit 2026 for Architecture" (CADCIM), text extraction `arch.txt`, lines 10368-19842. |
| `docs/reference/revit-books/arch-part3-ch11-16.md` | `:3` | Source: "Exploring Autodesk Revit 2026 for Architecture" (CADCIM), extracted text `arch.txt`. |
| `docs/reference/revit-books/mep-part2-ch4-6.md` | `:3` | Source: "Exploring Autodesk Revit 2024 for MEP" (CADCIM, Tickoo). Text extract `mep.txt`, lines 7102-12820. |

### Manifest fields

- **Cited source file name:** `arch.txt` — "Exploring Revit 2026 for Architecture" / "Exploring
  Autodesk Revit 2026 for Architecture" (CADCIM), the Architecture book. `mep.txt` — "Exploring
  Autodesk Revit 2024 for MEP" (CADCIM, Tickoo), the MEP book. (`docs/reference/README.md:6-11`
  names the three Architecture and three MEP part-files that these cover.)
- **Does the extraction file exist in the repo?** **No.** `git ls-tree -r b6ef0ee` has no
  `arch.txt` and no `mep.txt`; the only tracked `.txt` files are
  `crates/keymap/presets/revit-style.txt` and `crates/keymap/presets/revit-style-mouse.txt`
  (unrelated). `docs/reference/README.md:36-37` confirms it: "the extraction itself is not
  committed."
- **Tool used:** **not recorded anywhere in the repo.** `README.md:36-37` and
  `docs/agent-team/roles/bim-reference.md:25-26` ask for the tool in a manifest, but no manifest
  exists. The only PDF-text extraction tool in the repository is `pdftotext`
  (`spikes/s13_sheets_check/check_sheets.py:35`, `docs/sheets-printing.md:38`) — used for **sheet
  printing**, not the books, so it is only a candidate and is **unconfirmed**.
- **Page/line convention:** line numbers of the **extracted text file**, not PDF pages. Citations
  are "lines 1134-10368" (`arch.txt`) and "lines 7102-12820" (`mep.txt`); no page number is cited
  anywhere in `docs/reference/`.
- **SHA-256:** not recorded in the repo (it must be taken from the owner's licensed PDF; it cannot
  be derived from the paraphrase).
- **Manifest destination:** `docs/reference/` (`README.md:36-37`; `roles/bim-reference.md:25-26`).

---

## Method and limits

- Read-only: every file was read with `git show b6ef0ee:<path>`; no checkout, commit, push or edit
  was made. The only write is this report.
- The verdicts are code/document facts at `b6ef0ee`; they are not a judgement on whether the
  register's *remedy* (mark each superseded statement "superseded 2026-10-21") is desirable — that
  is the coder's decision.
- Claim 2's "no 3D tools" half is PARTIAL by nature: `view3d.rs` handles only `Tool::Family`, so
  most of the doc's list is still plan-only; the Beam evidence (`model_edit.rs:1549`) is a
  model-side call (`place_beam_from_controls`), not a simulated 3D event.
- The extraction tool (R6) is genuinely unrecorded; `pdftotext` is an inference from the only
  PDF-text tooling present, not evidence that it produced `arch.txt`/`mep.txt`.
