# G-M04 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M04 |
| **Gap** | `apps/electrical/tests/catalog.rs` unwraps `TMPDIR` (unset on Windows) |
| **Class** | 2 (wiring — code runs but is platform-fragile) |
| **Repos** | edesign-alike |
| **Depends** | — (standalone) |
| **Acceptance** | test passes with `TMPDIR` unset, using `std::env::temp_dir()` |
| **Base SHA** | `34499a2` (tip of `test/cross-repo-e2e-round-trip`) |
| **Candidate SHA** | (to be assigned after fix) |
| **Wave** | 0 — migration blockers |
| **Status** | IN PROGRESS |

## Register entry (verbatim from gap-register)

> G-M04 · `apps/electrical/tests/catalog.rs` unwraps `TMPDIR` (unset on Windows) · Class 2 · edesign-alike · — · test passes with `TMPDIR` unset, using `std::env::temp_dir()`

## Failure reproduced at base SHA 34499a2

```
thread 'literal_test_fixture_imports_real_csv_rows_and_roundtrips_without_inventing_ratings' panicked at apps/electrical/tests/catalog.rs:18:63:
called `Option::unwrap()` on a `None` value

thread 'real_template_import_save_load_has_no_fabricated_products_and_failed_import_clears_state' panicked at apps/electrical/tests/catalog.rs:48:63:
called `Option::unwrap()` on a `None` value

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```
