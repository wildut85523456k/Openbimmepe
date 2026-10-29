# G-M05 — Design (acceptance contract)

## Contract

Resolve the modify/delete conflict on `crates/document/` by accepting PR #111's deletion (core `544ee0d` already holds `StampEdesignResults`). After resolution:
- merge-tree clean (0 conflict markers)
- core `document` contains `StampEdesignResults`
- `cargo test --workspace` passes
- `cargo clippy --workspace --all-targets --all-features` clean

## Changes

1. Merge `9b20d2c` (main) into `update-core-deps`
2. Resolve 2 modify/delete conflicts by deleting `crates/document/src/electrical_exchange.rs` and `lib.rs` (accept PR #111 version)
3. Commit merge `b6fe472`

## API delta

No API change. The `StampEdesignResults` command remains available through the core `openbim-document` crate.

## Verification plan

- ✅ Merge-tree clean (0 conflict markers)
- ✅ Core `document` contains `StampEdesignResults` (grep, 4 occurrences)
- ✅ `cargo test --workspace` → 272 passed, 0 failed
- ✅ `cargo clippy --workspace --all-targets --all-features` → exit 0

## Owner approval

Approval to proceed with G-M05 granted by user via terse "next" (standing Wave 0 migration-blocker instruction). The fix is a merge conflict resolution with no code change; the #109 code is already in core.
