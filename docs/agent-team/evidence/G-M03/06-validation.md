# G-M03 — Validation

## Requirement check

| Acceptance criterion | Met? | Evidence |
|---|---|---|
| `cargo test --workspace --release` compiles | ✅ | `05-test.md` §2, exit 0 |
| All tests pass on migration branch | ✅ | 78 passed, 0 failed |
| Fails on base SHA | ✅ | `05-test.md` §1, exit 101 |
| Clippy clean | ✅ | `05-test.md` §3, exit 0 |

## Cross-repo impact

- No core changes → no core pin bump needed
- edesign-alike `main` (`7038d88`) still uses `egui = "0.36"` directly and is unaffected
- The migration branch `test/cross-repo-e2e-round-trip` needs to be merged to `main` for G-M03 to be resolved on `main` (that is an owner step, tracked under G-M05/G-M07)

## OS

Tests run natively on Windows 11 via git-bash (confirmed `.exe` artifacts in `target/`). No macOS-specific code in scope.

## Verdict

**PASS** — requirement met. Fix is minimal, test-only, and follows the established G-M01 egui 0.37 migration pattern.
