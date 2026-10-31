# S0 preservation ledger — OpenBIMMepe candidate

Candidate: `revit-alike` commit `76963c0` (PR #111), `openbim-core` pinned `544ee0d7`,
egui/eframe 0.37. Fork created with `git clone --no-local` from the local working copy,
then `git checkout -b main 76963c0`, remotes stripped (later `origin` added for
on-demand push). Tag `v0.0.1-candidate-76963c0` marks the provenance commit.

Per START-HERMES.md, native snapshot/oplog, IFC identities, save/open, the family
engine, MEP topology, schedules, IFC, graph preview/apply and approved icons must be
preserved. The following records what is preserved verbatim and what the S0/S1 work
intentionally changes.

## Preserved (unchanged from `76963c0`)

- **Native snapshot/oplog storage** — unchanged. Autosave/journal in `file.rs`,
  `journal.rs` (recovery ledger, transaction framing) intact.
- **IFC identities** — `class_map.rs`, `georef`, export/import unchanged;
  `export_ifc_file()` (model_edit.rs) wired by S0-fix but logic untouched.
- **Save/open + recovery** — `file.rs`, `journal.rs`, recovery path unchanged.
- **Family engine** — `family` crate, `family_types_ui.rs`, `type_editor.rs` unchanged.
- **MEP topology** — `mep.rs::place_mep_route` still rejects vertical/round/rolled
  (REA-021) and `edit.rs::transformed_set` still rejects joined-network moves (REA-022).
  These are `SOURCE-RESTRICTION` rows, source-inspected at `544ee0d7`.
- **Schedules/sheets** — `schedule_ui.rs`, `sheet_draw.rs`, `sheets` model unchanged.
- **Graph preview/apply** — `automation/` untouched. No Dynamo writeback (REA-027,
  PR #112 open on `revit-alike`, not this fork).
- **Approved icons & cross-platform** — egui 0.37, no JS (native Rust). Icons unchanged.
- **G-A02 graph execution direction** — preserved (local automation `Plan`/`apply`).

## Intentionally changed (S0/S1)

| Change | Where | Why |
|---|---|---|
| `ifc_export` command arm wired (`"ifc_export" => self.export_ifc_file()`) | `apps/desktop/src/lib.rs` `run_command` | Already advertised in `IMPLEMENTED`; was an unimplemented arm that panicked tests. No new export logic. |
| `Theme { Light, Dark }` persisted in `AppSettings` | `app_settings.rs`, `file.rs`, `lib.rs` | S1a: theme choice under Customisation. |
| Revit keyboard chords restored (Duct↔DT, Slab↔SB, etc.) + persistence | `sample()`, `new()`, `file.rs` | S1b: shortcuts same as Revit. |
| Help text "Right-drag" → "Shift+Middle-drag" | `lib.rs` help label | Mouse already Revit-style upstream; text was stale. |

## Deliberately NOT changed vs `c3bad7a` working tree

The other agent's uncommitted `c3bad7a` on `revit-alike` adds `RunSize::Oval`,
`Category::Space`, `Category::DuctTerminal` (G-E05). This fork is pinned at
`76963c0` and leaves `c3bad7a` untouched. Those items are recorded in the gap
register as `PARTIAL`/downgraded rather than lost.

## Stale artefact note

`revit-alike/.build-watchdog.log` references egui 0.36 and local path crates —
it describes the uncommitted `c3bad7a` working tree, **not** this candidate.
OpenBIMMeme builds and tests green against the `544ee0d7`/`76963c0` pin (see
`README.md` evidence table).
