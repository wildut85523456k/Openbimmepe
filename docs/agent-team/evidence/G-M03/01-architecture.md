# G-M03 — Architecture

| Question | Answer |
|---|---|
| Where does the change land? | `edesign-alike` repo, `test/cross-repo-e2e-round-trip` branch |
| Consumer impact | edesign-alike test targets now compile and pass under egui 0.37 (inherited from core `544ee0d`) |
| Breaking? | No — the fix is in test code only; no production code changes |

## Context

- Core `544ee0d` bumped egui from 0.36 to 0.37
- Migration branch `test/cross-repo-e2e-round-trip` (`92587cb`) bumps the openbim-core pin to `544ee0d`, inheriting egui 0.37
- Two test files use APIs that changed in 0.37:
  - `apps/electrical/tests/canvas.rs` — `egui::Event::MouseWheel` now requires a `source` field
  - `apps/electrical/tests/editor.rs` — `egui::Id::from(&str)` removed; `Id::unique` is the replacement (same as the G-M01 fix pattern)
- edesign-alike `main` (`7038d88`) still pins `egui = "0.36"` directly, so it is unaffected

## Placement

No core changes needed. The fix is entirely within edesign-alike test files — same pattern as G-M01 (`Id::new`→`Id::unique`, missing-struct-field additions).
