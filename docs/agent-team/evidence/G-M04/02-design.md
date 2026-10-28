# G-M04 — Design (acceptance contract)

## Contract

Fix `apps/electrical/tests/catalog.rs` so both tests pass with `TMPDIR` unset, using `std::env::temp_dir()` instead of `std::env::var_os("TMPDIR").unwrap()`.

## Changes

Replace both occurrences of:
```rust
let dir = tempfile::tempdir_in(std::env::var_os("TMPDIR").unwrap()).unwrap();
```
with:
```rust
let dir = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
```

- Line 18: `literal_test_fixture_imports_real_csv_rows_and_roundtrips_without_inventing_ratings`
- Line 48: `real_template_import_save_load_has_no_fabricated_products_and_failed_import_clears_state`

## API delta

| Before (broken) | After (fixed) |
|---|---|
| `std::env::var_os("TMPDIR").unwrap()` | `std::env::temp_dir()` |
| Panics on Windows (TMPDIR unset) | Cross-platform: queries TMPDIR/TMP/TEMP correctly |

## Verification plan

- ✅ FAIL with `TMPDIR` unset at base SHA `34499a2` (reproduced, 2 failures)
- ✅ PASS with `TMPDIR` unset at candidate SHA (fix applied)
- ✅ `cargo clippy --workspace --all-targets --all-features` — exit 0

## Owner approval

Approval to proceed with G-M04 granted by user via terse "next" (standing Wave 0 migration-blocker instruction). The fix is test-only, minimal, and follows the standard Rust cross-platform pattern.
