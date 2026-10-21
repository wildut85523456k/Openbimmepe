## What

P0 of the gap-closure work: the shell blockers that stop a model being built from scratch with the
mouse, plus the owner's ribbon decision. This is the plan's rows **B1–B6** and **B16/B17** as
`docs/gap-remediation-plan-2026-10-21.md` states them, with the acceptance the owner agreed on
21 Oct (recorded in `docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md`).

| Item | What changed |
|---|---|
| A1 (B1) | The plan is fitted to what is drawn, or to a default 20 m box when nothing is: the early `return` above every tool branch is gone, so a click on an empty canvas reaches a tool |
| A2 (B2) | The plan tabs, `plan_level`, the plan export, the Project Browser's plan entries, the Properties "View Name" and a sheet's plan views all resolve their level from the document — no `ElementId(10)`/`(11)` decides anything in the plan path. A drawing tool refuses to start in a project with no level ("Add a level first") |
| A3 (B3) | The window's keys moved into `Desktop::global_keys`; Esc there leaves the tool only when the tool has nothing pending, so the documented two-stage Esc is reachable. The tools that kept a point but left on the first Esc (Beam, Truss, a one-point duct/pipe run) now step back first |
| A4 (B4, B16) | Ctrl+S / Ctrl+N / Ctrl+O, the keymap's typed shortcuts ("W A" chooses Wall), and a Save As button in the quick-access bar |
| A5 (B5) | `command_defaults` gives every command its own options-bar numbers before its arm runs: a door chosen after a ceiling is no longer 2.7 m up, a beam after a rebar is not 16 × 8 mm, and the Foundation button initialises everything an isolated pad consumes |
| A6 (B6) | `place_level` refuses a duplicate name (any case) and a level closer than 100 mm; `rename_level` and `delete_level` added (delete refuses a level elements stand on), surfaced in the Level tool's options bar with a draft that survives the frames between keystrokes |
| A7 (B17) | Unbuilt ribbon tools are **rendered disabled** with a "planned, not implemented yet" tooltip. One list (`IMPLEMENTED`, 86 ids) drives both the ribbon's enabled state and a gate at the top of `run_command`; the catch-all arm carries a `debug_assert` |
| A8 | `eframe::App::ui` no longer holds the window: the body is `Desktop::window`, so a test can drive the whole window. Test-only fields record where the ribbon drew each button and the plan's own model-to-screen transform |
| — | New creates Level 1 (0 m) and Level 2 (3 m), the decision of 21 Oct |

Tests: the small office built from a new project through the **ribbon** (every tool chosen by clicking
its button, the grid drawn by pointer clicks aimed with the viewport's own transform, four walls, a
floor, the room they enclose, a section), then saved, reopened — comparing every element's identity,
geometry and name plus the room's area — and exported to IFC. Plus the ribbon honesty test, the
two-stage Esc coverage, the options-bar isolation, the rename draft over frames, and the level
validation/rename/delete tests.

## Corrections made during review

An independent review returned FAIL with six findings; all six are corrected in `P0 corrections`
(commit 7): the browser's level filter and `View::Level` naming, the missing pending states in the
Esc predicate, the framing/footing defaults, the rename draft rebuilt every frame, the
`clipboard_events` ordering plus the office test not exercising the whole window, and four backlog
rows whose acceptance depended on another row's deliverable.

## Visible behaviour changes

- **New** creates Level 1 and Level 2 (previously an empty model with tools pointing at missing
  levels).
- The quick-access **Settings** button is disabled: it was never wired, and per the 21 Oct decision
  unwired buttons are now shown disabled rather than printing a message.
- The Electrical `Compute` path, MEP content, IFC honesty (B7–B14) and the oplog autosave (B20) are
  **not** in this PR: they are the plan's next rows (backlog IFC-002, MEP-003/004/005, BIM-004).

## Verification

- `cargo test -p openbim-desktop --bin openbim-desktop` — **183 passed, 0 failed, 1 ignored**.
- `cargo test -p openbim-document` — **154 passed** + 2 integration.
- The local pre-push gate (cargo test + clippy `-D warnings` + `cargo audit` on this exact SHA)
  passed before the push.
- This branch contains the print-dialog test fix (`fix/print-dialog-texture-deltas`, PR #64) as its
  base; merge that one first and this PR's diff is just the P0 commits. Its tip also carries one
  follow-up, `fix(clippy): the rename hint is a fixed string` — the pre-push gate's
  `cargo clippy --workspace --all-targets -- -D warnings` rejected a `format!` of a fixed string,
  which is now a literal (behaviour unchanged).

## Review

Independent reviews, both in `C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/`:

- `report2.md` — 12 gates, **all PASS**; the six findings marked corrected; "fit to push and merge as P0".
- `report3.md` — the delta review after the rebase onto `main`: 7 gates, **all PASS**; "the rebased
  `90d1e1a` and `1a8d174` may be pushed as P0".

Rebased onto `main` (`3f7df8b`) before review; `git range-diff` shows 4 of the 7 commits byte-identical
and 3 differing only in context lines and the two conflict resolutions (both in `sample()`'s field
list, keeping `main`'s fields).
