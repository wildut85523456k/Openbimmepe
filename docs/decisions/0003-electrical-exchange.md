# 0003 — Electrical exchange with edesign-alike

Date: 23 Oct 2026. Follows 0002 (the electrical calculation is edesign-alike's, not ours).

## Decision
The app writes one JSON file, `openbim-electrical-exchange` version 1, holding everything the model knows that a full
electrical computation needs. Window: Compute electrical > "Export for edesign...". Code:
`crates/document/src/electrical_exchange.rs` (`build`), `apps/desktop/src/elec_ui.rs` (`exchange_basis`, `exchange_text`).

## Why our own format
edesign-alike has no import contract yet and its model types have no serde (only the editor DTOs). A versioned,
self-describing file lets its importer be written against a fixed document. A change that is not backward compatible
raises `version`.

## What is in it
- `project`: name, project key (hex), model revision, site.
- `design_basis`: only what the user typed in the window (voltage, limits, earthing, fault level if the fault check is on,
  source kVA if given, the tables). Nothing is defaulted into it that the user did not choose, except earthing, which the
  window always holds.
- `nodes`: source, board, load; level, plan position, power, power factor, efficiency, motor start, and the family
  instance behind it with its **IFC GlobalId** (same id as the IFC export of the same project key) and connector.
- `cables`: ends, three-phase, drawn route and its length in metres (plan route plus level difference plus extra), grouping,
  ambient, demand factor, RCD, resistance and reactance per km when set.
- `rooms`: number, name, department, level, area, GlobalId.
- `readiness`: `blocking` (no source, several sources, no load with power, a node no cable feeds, a cable to a missing
  node) and `missing` (what the model cannot give: fault level, cable type/conductor/installation method, protective
  devices, cables without R/X, loads without power).

## Not in it
No result of any calculation. No cable choice. Cable type, installation method and protective devices are not held by
the model; they are listed under `missing` so edesign-alike chooses or asks.

## Open
- edesign-alike needs an importer; the file shape is ours to change until it exists.
- Results coming back (write-back) are not designed; edesign's README says live BIM write-back is not implemented.
