# G-M04 — Test

## Gate commands (all run on edesign-alike `test/cross-repo-e2e-round-trip`)

### 1. Test at base SHA (FAIL expected)

Base SHA: `34499a2` (fix not yet applied)

```sh
git checkout 34499a2
env -u TMPDIR cargo test -p openbim-electrical-app --test catalog
```

**Exit code: 101 (failure)**
```
thread 'literal_test_fixture_imports_real_csv_rows_roundtrips_without_inventing_ratings' panicked at apps/electrical/tests/catalog.rs:18:63:
called `Option::unwrap()` on a `None` value
thread 'real_template_import_save_load_has_no_fabricated_products_and_failed_import_clears_state' panicked at apps/electrical/tests/catalog.rs:48:63:
called `Option::unwrap()` on a `None` value
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

### 2. Test at candidate SHA (PASS expected)

Candidate SHA: `13a2740` (fix applied)

```sh
git checkout 13a2740
env -u TMPDIR cargo test -p openbim-electrical-app --test catalog
```

**Exit code: 0 (success)**
```
test literal_test_fixture_imports_real_csv_rows_roundtrips_without_inventing_ratings ... ok
test real_template_import_save_load_has_no_fabricated_products_and_failed_import_clears_state ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 3. Clippy (PASS expected)

```sh
cargo clippy --workspace --all-targets --all-features
```

**Exit code: 0 (success)**
```
Checking openbim-electrical-app v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s
```

## Verdict

**PASS** — fails on base SHA, passes on candidate SHA. Test passes with `TMPDIR` unset on Windows.
