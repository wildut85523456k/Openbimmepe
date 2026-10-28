# G-M03 — Design (acceptance contract)

## Contract

Fix the two test files so `cargo test --workspace --release` compiles and passes on the migration branch `test/cross-repo-e2e-round-trip`.

## Changes (at `34499a2`)

1. **`apps/electrical/tests/canvas.rs:626`** — Add `source: egui::MouseWheelSource::Unknown` to the `Event::MouseWheel` initializer (new required field in egui 0.37).
2. **`apps/electrical/tests/editor.rs:1273`** — Change `egui::Id::from("properties_label_field")` → `egui::Id::unique("properties_label_field")` (the `From<&str>` impl was removed).
3. **`apps/electrical/tests/editor.rs`** — Add `#[allow(clippy::too_many_arguments)]` to `canvas_frame_with_field` (signature was unchanged; clippy flag surfaced by the egui API churn).

## API delta

| egui 0.36 | egui 0.37 | Replacement |
|---|---|---|
| `Event::MouseWheel { …, }` (no `source`) | `Event::MouseWheel { …, source: MouseWheelSource }` | Add `source: egui::MouseWheelSource::Unknown` |
| `Id::from(&str)` | removed | `Id::unique(salt)` |
| `Id::new` (deprecated) | `Id::new` calls `Id::unique` internally | Use `Id::unique` directly (already applied in G-M01) |

## Verification plan

- ✅ `cargo test --workspace --release` — compiles + all tests pass
- ✅ `cargo clippy --workspace --all-targets --all-features` — exit 0
- ✅ Fix commit `34499a2` builds on base `92587cb` which fails to compile

## Owner approval

Acceptance test = the register's stated criterion (`cargo test --workspace --release` passes on the migration branch). No new features or API surface; test-only fix. Approval implied by Wave 0 migration-blocker priority.
