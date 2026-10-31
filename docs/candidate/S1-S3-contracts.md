# S0–S3 contracts for OpenBIMMepe

Candidate: forked from `revit-alike@76963c0`, core pinned to `544ee0d7`, egui 0.37.
Repository: `github.com/wildut85523456k/Openbimmepe` (public). `main` tracks `origin/main`.

## S0 — provenance & defect repair (DONE — pushed)

- Fork is a clean clone of exactly `76963c0` (PR #111 committed state), `origin` stripped
  on creation, re-added later for on-demand push per the authorisation in force.
- `S0-fix`: `ifc_export` was listed in `IMPLEMENTED` but had no `run_command` arm, which
  panicked in `model_view::tests` via `debug_assert!(false, ...)`. Wired
  `"ifc_export" => self.export_ifc_file()` (lib.rs `run_command`).
  - Commit `a4ebb05` → tests `275 passed, 0 failed, 1 ignored`, exit 0.
- Preservation ledger written (see `S0-PRESERVATION-LEDGER.md`).

## S1 — implementation streams (DONE — pushed)

### S1a — theme preference under Customisation (DONE)
- `Theme { Light, Dark }` enum persisted as `theme=light|dark` in `AppSettings`
  (`%APPDATA%/openbim/settings.txt`, `key=value`).
- Startup restores via `Desktop::apply_theme`; toolbar toggle persists; Options window
  "Customisation › Appearance › Theme" selector. `..Default::default()` test fixture fixed.
- Commit `fa29309` → `276 passed, 0 failed, 1 ignored`.
- Closes **REA-007**.

### S1b — keyboard & mouse same as Revit (DONE)
- Keyboard: the 13 registered commands now carry Revit's actual chords for the commands
  they stand for (e.g. Duct `DT`, Slab `SB`, Wall `WA`, Pipe `PI`, Beam `BM`, Column `CL`,
  Footing `FT`, Beam System `BS`, Family `CM`). Dispatch keeps the display-name →
  `run_command` id bridge (`keymap_command_id`). Overrides persist across restart via
  `keymap.save_overrides()` / `load_overrides()` in `AppSettings`.
- Mouse: the upstream `openbim-viewport` camera already implements Revit's mapping —
  `Camera::drag_kind`: **middle-drag pans, Shift+middle-drag orbits**, wheel zooms
  at the cursor. The app's stale "Right-drag: orbit" help text was corrected.
- Options window gains a "Keyboard (Revit defaults)" pane listing every command's chords
  and a Reset-all-to-Revit-defaults button.
- Commit `10e0347` → `277 passed, 0 failed, 1 ignored`; new test
  `the_keymap_defaults_match_revit`.
- Closes **REA-046**.

## S2 — gap register refresh & documentation (THIS PASS)

- Gap register (`docs/candidate/OpenBIM-GAP-REGISTER-2026-10-31.csv`) committed and
  refreshed against candidate `544ee0d7`: REA-007 and REA-046 promoted to `VERIFIED`.
- Preservation ledger, contracts, dependency diagram authored under `docs/candidate/`.

## S3 — implementation (DONE — pushed)

**REA-006** (open view tabs with stable IDs) is complete. Delivered in three increments, each
verified green before push (279 → 282 passed, 0 failed, 1 ignored; review step held each):

- `View::id(&self) -> u64` — stable, restart-invariant, disjoint-per-variant view id.
- `MAX_OPEN_VIEWS = 10` (Revit-like tab cap) and `view_open_policy(open_count)` →
  `Continue` until the cap, then `Replace`.
- `open_views: Vec<View>` + `active_view: usize` are the open tab set; `view` is always
  `open_views[active_view]`, so the ~176 read-only call sites are untouched (regression-safe).
- `activate_view_at`/`add_view`/`set_view`/`close_view_at`/`reset_view` keep the tab set and
  the active view in sync.
- **Per-view state** (`ViewState: pan/zoom/camera_yaw/camera_pitch/section_xf`) is snapshotted
  on leave and restored on switch, so each tab remembers its camera. A tab with no saved state yet
  (just opened) inherits the live camera, so pointing at a view keeps the picture continuous (as in
  Revit) until the user leaves and returns; state is stored as `Vec<Option<ViewState>>` so "no snapshot
  yet" is distinct from a snapshot equal to the default. The 3D viewport widget keeps its own keyed
  camera. (Refinement of Increment 2: needed so browser/section/elevation opens keep camera
  continuity while still opening a tab.)
- **Tab bar** in `model_view.rs::show()`: one tab per open view, active highlighted, per-tab
  close button; available views (levels / {3D} / facings / sections) open-or-switch as tabs.
- **Replace/cancel prompt at the cap**: a pending view (`view_replace_pending`) opens a prompt
  offering a per-tab "Replace" or "Cancel" (evict the picked tab, or drop the attempt). Note:
  reorder/overflow-scroll and the 3D-viewport internal orbit per tab remain follow-up polish.
- Tests: `view_ids_are_stable_and_disjoint`, `the_open_view_cap_is_ten`,
  `open_views_add_activate_close_and_cap`, `per_view_state_survives_a_tab_switch`,
  `at_cap_opening_a_view_prompts_replace_then_evicts_or_cancels`.



## REA-009 — properties browser & docking (in progress)

**Increment 1 — a view selected in the Project Browser displays in the active viewport.**
A single click on a plan / level plan / section / elevation-marker view now opens (or switches to)
that view in the active viewport and as a tab, matching the user's "select a view, it displays"
point. Verified green before any push (283 passed; was 282; the 5 `double_click_widget("view:…")`
cases in the two-storey-office e2e test still pass).

- All Project-Browser and Properties view-open sites route through `Desktop::set_view` (the tab
  model) instead of assigning `app.view` directly, so the selected view becomes a real tab and the
  active-view distinction (acceptance: "active view distinction") is established:
  `app.view == open_views[active_view]`.
  - `tree_node` single-click — now opens + displays (was highlight-only); double-click open-or-switch.
  - `level_plans` single-click — now opens + displays; double-click open-or-switch.
  - browser sections / elevation markers — open-or-switch (single-click).
  - Properties "Open" (elevation marker) and "Open view" (section line) — open-or-switch.
- Camera continuity: because `set_view` adds a new tab that inherits the live camera (`None`
  state, see the Incremental 2 refinement above), opening a view from the browser does not snap the
  viewport to a default — the 5 double-click e2e assertions on continuous zoom/pan/yaw hold unchanged.
- Open-or-switch: re-selecting an already-open view just activates its tab (no duplicate).
- Test: `browser_single_click_opens_the_view_in_the_active_viewport` (single-click "view:Level 1"
  → `app.view == View::Plan`, present in `open_views`, active tab).
- Deferred (post-Increment 2): full Properties panel type-vs-instance + mixed multiselect + grouped
  searchable properties + numeric commit/cancel; persistent docking splitters/width.

**Increment 2 — browser rename/duplicate/delete.** Each view in the Project Browser now has a
right-click context menu (Rename / Duplicate / Delete), matching Revit's view-context actions.

- **Rename**: opens an inline single-line field committed on Enter, OK, or blur; cancelled on Escape.
  A level plan renames its level (`Desktop::rename_level`); a section or elevation-marker view stores a
  desktop view-name override (`view_names: HashMap<u64,String>`) because the pinned document cannot rename
  those element categories — `el.name` is preserved and `view_name()` resolves the override so the tab
  label, viewport title, and browser node all stay consistent. `rename_view_pending`/
  `rename_view_text` hold the in-progress edit; the field re-renders the node.
- **Duplicate**: `place_section` (sections) / `place_elevation` (markers) re-creates the view at the
  element's geometry; the copy is placed beside the original and opened as a tab.
- **Delete**: `delete_commands` + `transact` remove the backing element and `close_view_at` closes its
  tab (level plans reuse `delete_level`, refused if the level holds elements). Delete carries an undo.
- Tests: `a_section_view_can_be_renamed_deleted_and_duplicated_from_the_browser` (method-level: rename
  → `view_name()` reflects the override; duplicate → +1 element of the same category with a new id and
  a new tab; delete → element removed + tab closed) and `a_section_view_context_menu_renames_from_the_browser`
  (full harness: right-click "view:Section 1" → Rename → type "Ref Plan" → OK → rename committed). 285 passed,
  0 failed (was 283; +2).

**Persistent docking / splitter width.** The left docking sidebar (browser + properties) width and
the internal Properties/Browser split are now persisted to the single settings file
(`%APPDATA%/openbim/settings.txt`), matching the theme/keymap persistence pattern.

- New `AppSettings` fields `sidebar_width` (default 310.0, clamped to the panel's 260–520 px range)
  and `properties_split` (default 0.55, clamped 0.1–0.9), round-tripped through `to_text`/`from_text`
  as `sidebar_width={:.1}` / `properties_split={:.3}` (one line per key, unknown keys ignored).
- Applied as the panels' `default_size` (`egui::Panel::left("side")` and the top `properties_split`
  panel) in `window()` / `panels::sidebar()`.
- A drag on either panel is captured from the `PanelResponse` rect and written back to settings only
  when no pointer button is held (drag ended) and the value changed by >1 px / >0.01 — so a live drag
  does not thrash the file. The capture is gated on `AppSettings::path().is_some()`, which is `None`
  under `cfg(test)`, so tests never touch the user's settings file.
- Test: `dock_widths_round_trip_defaults_and_clamping` (round-trip + default values + out-of-range
  clamping). 286 passed, 0 failed (was 285; +1).

## Out of scope / blocked (applies to the candidate)

- **Runtime mouse remapping**: owned upstream by `openbim-viewport`'s `Camera::drag_kind`
  (crate pinned `544ee0d7`). The candidate pins that crate and must not be altered without
  approval, so runtime mouse rebinding is **blocked** at the app layer — the defaults already
  match Revit. `MouseMap`/`Gesture` exist in the `keymap` crate but the viewport widget does
  not consume them (it calls `camera.drag(buttons, mods, …)` directly).
- Adding keyboard commands beyond the 13 registered ones is an S3 feature-expansion.
