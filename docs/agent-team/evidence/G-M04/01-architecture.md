# G-M04 — Architecture

| Question | Answer |
|---|---|
| Where does the change land? | `edesign-alike` repo, `apps/electrical/tests/catalog.rs` (edesign-alike) on branch `test/cross-repo-e2e-round-trip` |
| Consumer impact | Test suite now passes on Windows where `TMPDIR` is not set (Windows uses `TMP`/`TEMP`) |
| Breaking? | No — test-only change; no production code modified |

## Root cause

`catalog.rs` lines 18 and 48 use:
```rust
tempfile::tempdir_in(std::env::var_os("TMPDIR").unwrap()).unwrap()
```

On Windows, `TMPDIR` is never set — the OS uses `TMP` and `TEMP` instead. So `var_os("TMPDIR")` returns `None`, and `.unwrap()` panics.

`tempfile::tempdir_in()` requires an explicit directory. The fix is to use `std::env::temp_dir()` instead, which queries the platform-appropriate env vars (`TMPDIR` on Unix, `TMP`/`TEMP`/`USERPROFILE` on Windows).

## Placement

Test file only — no core or production code changes needed. No core pin bump required.
