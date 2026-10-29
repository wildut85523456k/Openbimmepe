# G-M05 — Architecture

| Question | Answer |
|---|---|
| Where does the change land? | `revit-alike` repo, branch `update-core-deps` (PR #111) |
| Consumer impact | After merge, `apps/desktop` consumes all shared crates from core `544ee0d`; `crates/` directory removed from revit-alike |
| Breaking? | No — the `StampEdesignResults` code from main #109 is already in core `544ee0d`; deleting `crates/document/` in revit-alike loses nothing |

## Root cause

- Main `9b20d2c` (#109) added `StampEdesignResults` to `crates/document/src/electrical_exchange.rs` and `lib.rs`
- PR #111 (`update-core-deps`) at `d7f4883` "Migrate shared crates to openbim-core git deps" deletes the entire `crates/` directory, moving those crates to core dependencies
- Both branches touch `crates/document/` → modify/delete conflict

## Resolution

Accept PR #111's deletion of `crates/document/`. The `StampEdesignResults` code is already present in core `544ee0d` (`crates/document/src/lib.rs`), so no code is lost.

## Verification

- Core `544ee0d` `crates/document/src/lib.rs` contains `StampEdesignResults` (4 occurrences — confirmed by grep)
- Merge-tree clean: 0 conflict markers after resolution
- `cargo test --workspace` → 272 passed, 0 failed
- `cargo clippy --workspace --all-targets --all-features` → exit 0
