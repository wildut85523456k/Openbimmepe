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

## S3 — future work (NOT started)

- S3 is gated on shared contracts (graph execution G-A02, IFC preservation modes).
  Not undertaken in this fork until the S0/S1/S2 foundation is accepted.

## Out of scope / blocked (applies to the candidate)

- **Mouse remapping at runtime**: owned upstream by `openbim-viewport`'s `Camera::drag_kind`
  (crate at `544ee0d7`). The candidate pins that crate and must not be altered without
  approval, so runtime mouse rebinding is **blocked** at the app layer — the defaults already
  match Revit. `MouseMap`/`Gesture` exist in the `keymap` crate but the viewport widget does
  not consume them (it calls `camera.drag(buttons, mods, …)` directly).
- Adding further keyboard commands beyond the 13 registered ones is an S3 feature-expansion.
