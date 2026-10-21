## What

`settings_dialog_renders_in_both_themes_and_invalidates_stale_bytes` dropped two egui
`TexturesDelta` values unapplied. epaint asserts on that in a debug build (`Drop for TexturesDelta`
uses `debug_assert!`), so the test failed under `cargo test` while CI's `--release` run never reached
the assertion — which is why the failure was invisible to the gate that was believed green.

The test now clears both deltas, as the other rendering tests in the app do.

## Why it matters

`cargo test --workspace` is fail-fast: this one failure aborted the suite before every target ran, so
a debug run reported 305 tests instead of ~1,150 and hid the state of the rest. With this fixed, the
desktop binary is green (183 passed, 0 failed, 1 ignored).

## Verification

- `cargo test -p openbim-desktop --bin openbim-desktop print_dialog::tests::` — 2 passed, and the
  binary carrying the fix is newer than the patched source.
- Reproduced the failure on the base (`c90cbc8` / `main`) before changing anything: it is
  pre-existing, not caused by any other branch.
- Test-only change: no product code touched.

## Review

Independently reviewed: `report2.md` gate **G9 PASS** and `report3.md` gate **D4 PASS** (byte-identical
content after the rebase onto `main`), both in
`C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/`.
