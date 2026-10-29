# AUTO-001 typed command/query API — independent verification report

- **Profile:** `bim-review` (independent Rust reviewer, READ-ONLY on judged source)
- **Timestamp:** 2026-10-23 05:26:56 +0700 (Asia/Phnom_Penh)
- **Host / OS:** Windows 11, host `CDC_COOlaptop`
- **Toolchain:** rustc 1.99.1 (58cf8f9c3 2026-10-13); cargo 1.99.1 (5f94df478 2026-08-27); clippy 0.1.99
- **Candidate SHA:** `40eb766afa0bb755debdc1f42922adca90e29d97` (branch `feat/auto-001-commands`)
- **Base branch / tip:** `main` @ `480927122d4b4300611fdfd37442b1f910676586`
- **Branch point (merge-base):** `f0dc0b7d6c927c24c7ecb99be775acf4712c72e4` — confirmed: `git merge-base main 40eb766` → `f0dc0b7`
- **Scope inspected:** own detached worktree `C:\c\Users\wilfr\hermes-first-task\revit-alike-wt-pr71` at `40eb766`, isolated `CARGO_TARGET_DIR=C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-target`

## Scope verification

The contract states the candidate's own diff is `git diff f0dc0b7 40eb766` = **10 files, +1472/-1**, all under `crates/commands/`. Verified exactly:

```
$ git -C <wt> diff --stat f0dc0b7 40eb766
 Cargo.lock                          |   8 +
 Cargo.toml                          |   2 +-
 crates/commands/Cargo.toml          |  15 +
 crates/commands/src/command.rs      | 358 +
 crates/commands/src/dimension.rs    |  75 +
 crates/commands/src/lib.rs          |  27 +
 crates/commands/src/parameter.rs    | 405 +
 crates/commands/src/quantity.rs     | 281 +
 crates/commands/src/unit.rs         | 188 +
 crates/commands/tests/acceptance.rs | 114 +
 10 files changed, 1472 insertions(+), 1 deletion(-)
```

One commit on top of the merge-base (`git log --oneline f0dc0b7..40eb766` → `40eb766 AUTO-001: typed command and query API (openbim-commands)`).
The single `-1` is the workspace `members` line in `Cargo.toml` (`crates/commands` appended); `Cargo.lock` gains only the `openbim-commands` package block. No production file outside `crates/commands/` is touched, and nothing is deleted.

## Commands actually run (and exit codes)

| # | Command | Result |
|---|---|---|
| A6 | `cargo test -p openbim-commands` | **exit 0** — `12 passed; 0 failed` (unit) + `4 passed; 0 failed` (acceptance) + `0` doc-tests |
| A7a | `cargo clippy -p openbim-commands --all-targets -- -D warnings` | **exit 0** — 0 `warning:` lines, 0 `error`/`error[` lines |
| A7b | `cargo test --workspace --release --no-fail-fast` | **exit 0** — 110 test binaries `test result: ok`, 0 `FAILED`, 0 `failures:` |
| A1 | `cargo test -p openbim-commands a_dimension_mismatch_is_an_error` | **exit 0** — `1 passed; 0 failed` (the 3 m + 3000 mm case) |
| A8 | `grep -rn unsafe crates/commands/src` | only the 6 `#![forbid(unsafe_code)]` attributes; no `unsafe` block |

`cargo test` was given exactly one filter per invocation (the A1 probe); the 12+4 counts come from the single unfiltered crate-suite run.

## Acceptance items

| Item | Result | Evidence |
|---|---|---|
| **A1 Quantity semantics** | **PASS** | `quantity.rs:13-16` `Quantity { si: f64, dimension: Dimension }`; `DimensionMismatch { left, right }` at `quantity.rs:20-23`; `checked_add`/`checked_sub` call `require()` (`quantity.rs:141-150`, `157-166`) which returns `Err(DimensionMismatch { left: self.dimension, right: dimension })`. Same-dimension combine across units verified by test `a_dimension_mismatch_is_an_error` (`tests/acceptance.rs:20-25`: 3 m + 3000 mm → `si == 6.0`, dimension `Length`). Wire form `{"value":<number>,"unit":"<symbol>"}` at `quantity.rs:177-200`; test `the_wire_form_is_value_and_unit` asserts `{"value":6.0,"unit":"m/s"}` and round-trip. |
| **A2 Unit conversion** | **PASS** | `unit.rs:46-48` `to_si = scale * value + offset`; `from_si = (si - offset) / scale` (`unit.rs:51-53`). Only temperature affine: `K` is `linear` (offset 0), `°C`/`°F` are `affine` (`unit.rs:94-101`). `UNITS` contains exactly **37** entries, 37 unique symbols (counted programmatically from the `pub static UNITS` block). `temperature_units_convert_through_kelvin` asserts 100 °C = 212 °F = 373.15 K. |
| **A3 Dimension** | **PASS** | `dimension.rs:11-45` declares exactly **16** variants (Length … RotationalSpeed). `canonical()` (`unit.rs:123-142`) names one unit per dimension and `every_dimension_names_a_unit_and_the_symbol_round_trips` asserts `by_symbol(canonical(d).symbol) == Some(canonical(d))` and `from_si(to_si(1.5)) ≈ 1.5` for all 16. |
| **A4 Parameter registry** | **PASS** | `ParameterId(&'static str)` (`parameter.rs:13`), `ParameterRegistry` (`parameter.rs:164`), `Resolve::{Id, ByLabel{parameter,label}}` (`parameter.rs:128-146`), `ResolveError { given }` (`parameter.rs:150`). `BUILTIN_PARAMETERS` has exactly **16** entries, each with non-empty EN + FR labels (asserted by `ids_are_stable_and_unique_and_dimensions_are_set`). `resolve()` tries ID → EN label → FR label → `Err` (`parameter.rs:185-204`); `a_label_resolves_to_the_stable_id_with_a_warning` and `a_label_looks_up_the_stable_id` assert EN "Width" and FR "Largeur" both resolve to `ParameterId("element.width")` as `Resolve::ByLabel`, and `resolve("nope")`/`resolve("Largeur du conduit")` are refused. `ifc_group` field exists on every parameter; only `wall.thickness` carries a non-empty group (`Pset_WallCommon`) — see finding 4. |
| **A5 Command registry** | **PASS** | `CommandDef` (`command.rs:60-73`), `InputSpec` (`command.rs:22-29`), `ValueKind` (`command.rs:9-18`). `BUILTIN` has exactly **13** entries with unique ids: Manage/automation 4 (`automation_editor`, `automation_player`, `automation_mcp`, `automation_snapshot`), Systems 2 (`duct`, `pipe`), Architecture 5 (`wall`, `door`, `window`, `level`, `slab`), Structure 2 (`beam`, `column`). `lookup`/`tab`/`groups` at `command.rs:292-310`. `the_registry_has_unique_ids_and_known_parameters` asserts every `InputSpec` parameter resolves in the parameter registry (21 input specs, all known). Every `CommandDef` declares `writes` (9 `true`, 4 `false`); `a_command_declares_whether_it_writes_the_model` covers both. |
| **A6 Tests** | **PASS** | `cargo test -p openbim-commands` → **exit 0**, `12 passed; 0 failed` (unit) + `4 passed; 0 failed` (acceptance). Individual `ok` lines: `a_command_declares_whether_it_writes_the_model`, `commands_are_found_by_id_and_listed_by_tab`, `a_label_resolves_to_the_stable_id_with_a_warning`, `the_registry_has_unique_ids_and_known_parameters`, `text_is_read_and_written_in_the_canonical_unit`, `ids_are_stable_and_unique_and_dimensions_are_set`, `a_dimension_mismatch_is_an_error`, `an_ifc_group_resolves_through_the_project_map`, `units_convert_to_si_and_back`, `every_dimension_names_a_unit_and_the_symbol_round_trips`, `temperature_units_convert_through_kelvin`, `the_wire_form_is_value_and_unit`; acceptance: `a_dimension_mismatch_is_an_error`, `a_label_looks_up_the_stable_id`, `the_ribbon_commands_are_registered`, `a_quantity_travels_as_a_value_and_a_unit`. |
| **A7 Gates** | **PASS** | clippy `-p openbim-commands --all-targets -- -D warnings` **exit 0**, 0 diagnostics. `cargo test --workspace --release --no-fail-fast` **exit 0**, 110 `test result: ok` lines, no failures. The branch tip on `origin` is `40eb766afa0bb755debdc1f42922adca90e29d97` (`git ls-remote origin refs/heads/feat/auto-001-commands`), matching the reviewed SHA. |
| **A8 Safety / style** | **PASS** | `crates/commands/Cargo.toml:9` `unsafe_code = "forbid"` plus `#![forbid(unsafe_code)]` in all 6 modules and `tests/acceptance.rs:3`; no `unsafe` block anywhere in `crates/commands/src`. `serde = { version = "1", features = ["derive"] }` (`Cargo.toml:15`) with `serde_json` as a dev-dependency only — correct, since the wire form is exercised in tests. Clippy `--all-targets -D warnings` is clean, so no dead code and no lint suppressions beyond the documented `#[allow(clippy::too_many_arguments)]` at `command.rs:78`. |

## Findings (severity-ranked; none block the gate)

1. **[medium] Label lookup is ambiguous and silently order-dependent — `parameter.rs:185-204`.**
   The English labels `Width`, `Height` and `Diameter` are each used by more than one parameter
   (`element.width`/`duct.width`; `element.height`/`duct.height`; `duct.diameter`/`pipe.diameter`),
   as are the French `Largeur`, `Hauteur`, `Diamètre`. `resolve()` returns the **first** match in
   registration order, so `resolve("Width")` always yields `element.width` and can never yield
   `duct.width`. Verified independently (probe log `04-probe.log`):
   `Width → ByLabel element.width`, `Height → ByLabel element.height`, `Diameter → ByLabel duct.diameter`.
   The doc comment frames this as "like Dynamo's get parameter by name", and Dynamo's by-name lookup is
   indeed ambiguous — but the acceptance test `a_label_resolves_to_the_stable_id_with_a_warning`
   (`parameter.rs:347-353`) hard-codes the registration-order winner, so a graph node that names a duct's
   "Width" binds the element parameter with no warning that the name was ambiguous. Not a gate failure
   (A4's stated requirement — a label resolves to a stable id, unknown refused — is met), but a real
   trap worth a follow-up: either namespace labels per family or surface the ambiguity.

2. **[medium] The registry has no consumer — rule 9 is not realised by this diff.**
   `grep -rn 'openbim_commands\|openbim-commands'` over `crates/` and `apps/` matches **only**
   `crates/commands/tests/acceptance.rs`; no crate's `Cargo.toml` lists `openbim-commands` as a
   dependency. Rule 9 (`docs/revit-style-ui/04-visual-automation.md`) states "The ribbon, keymap, graph
   nodes and MCP tools are all generated from it", and the AUTO-001 backlog acceptance
   (`docs/revit-style-ui/05-backlog.md:39`) is "Ribbon uses it; unit-mismatch and label-lookup tests".
   Only the test half is satisfied here: this is a foundation crate with no integration yet. This is
   consistent with AUTO-001 being the API layer, but the backlog criterion is **not met** by this PR and
   should not be recorded as met.

3. **[low] `RotationalSpeed` treats rpm as the SI base — `unit.rs:113-114`.**
   `Unit::linear("rpm", …, 1.0)` and `Unit::linear("rps", …, 60.0)`. The relative conversion is correct
   (60 rpm = 1 rps, verified: `60 rpm → si 60`, `1 rps → si 60`), but the field named `si` holds rpm,
   not the SI unit of rotational speed (rad/s ≈ 0.1047 per rpm). Harmless for add/sub (same dimension),
   misleading for any consumer that reads `.si()` expecting SI.

4. **[low] 15 of 16 parameters carry an empty IFC group — `parameter.rs:58`, `214-327`.**
   Only `wall.thickness` has a non-empty `ifc_group` (`Pset_WallCommon`); the other 15 are `""`.
   Rule 8 says parameters are "mapped to IFC property sets". The code documents this as intended
   ("empty when no mapping is agreed yet, which is the state the pset map ships in") and
   `an_ifc_group_resolves_through_the_project_map` asserts `ifc_target(&[]) == None`, so the behaviour is
   deliberate — but the IFC-group half of rule 8 is only 1/16 populated.

5. **[low] `slab` names its thickness input `element.height` — `command.rs:244-257`.**
   Description: "Lay a slab: thickness and top offset." Inputs: `element.height` (Height/Hauteur) and
   `element.elevation`. A slab's thickness is not its height; the parameter name and the description
   disagree. Cosmetic, but it is exactly the kind of label/parameter mismatch rule 8 exists to prevent.

6. **[low] The wire form carries the canonical unit, not the parameter's display unit — `quantity.rs:177-186`, `unit.rs:123-142`.**
   `Serialize` always uses `canonical(dimension)`, so a `duct.width` quantity of 600 mm serializes as
   `{"value":0.6,"unit":"m"}` (probe `04-probe.log` line (a)), not `{"value":600,"unit":"mm"}` even though
   the parameter declares `mm` as its display unit (`parameter.rs:264-270`). Likewise the canonical unit
   of `Temperature` is the affine `°C`, so a quantity at 0 K serializes as `{"value":-273.15,"unit":"°C"}`.
   Both are documented (`quantity.rs:240-242`) and the acceptance test pins the canonical form, so this is
   a design decision, not a defect — but consumers must not assume the wire unit equals the parameter's
   display unit.

7. **[info] Unused public API surface — `command.rs:9-18`, `parameter.rs:60-64`.**
   `ValueKind::{Text, Flag, Element, Elements}` and `Parameter::{default, min, max}` are declared but no
   built-in command or parameter uses them. They are `pub`, so the dead-code lint does not fire and clippy
   is clean; noted only as surface that is not yet exercised.

No HIGH-severity defects found.

## Overall verdict

**PASS** — all eight acceptance items (A1–A8) PASS on candidate SHA
`40eb766afa0bb755debdc1f42922adca90e29d97`.

Unmet requirements: **none within A1–A8.** One backlog-level criterion sits outside the gate and is
**not** satisfied by this diff: `docs/revit-style-ui/05-backlog.md:39` AUTO-001 acceptance "Ribbon uses
it" — no crate consumes `openbim-commands` yet (finding 2). The PR should not be recorded as closing that
criterion.

## Raw evidence paths

- `C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-evidence\01-crate-test.log` — crate suite, exit 0 (12 + 4 passed)
- `C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-evidence\02-clippy.log` — clippy `-D warnings`, exit 0, 0 diagnostics
- `C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-evidence\03-workspace-release.log` — workspace release test, exit 0, 110 ok binaries
- `C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-evidence\04-probe.log` — independent probe (canonical-unit wire form, rpm/rps, ambiguous labels)
- Reviewer probe crate (not part of the candidate): `C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-probe\`
- Isolated build dir: `C:\c\Users\wilfr\AppData\Local\hermes\cache\scratch\pr71-target\`

## Cleanup

The detached worktree `C:\c\Users\wilfr\hermes-first-task\revit-alike-wt-pr71` is removed after this
review (`git worktree remove --force`). The only file written is this report under
`docs/agent-team/evidence/`; no judged source, and no branch, was modified, pushed or merged.
