# G-M07 — Implementation

## Script

`docs/agent-team/scripts/cross-repo-consumer-gate.sh`

## Usage

```sh
cross-repo-consumer-gate.sh <core-sha> [core-checkout-path]
```

## What it does

1. Uses an existing core checkout at the candidate SHA (or clones one)
2. Discovers all `openbim-*` crate names from core's `crates/`
3. For each consumer (revit-alike, edesign-alike):
   a. Saves original `Cargo.toml` files
   b. Appends `[patch."https://github.com/wildut85523456k/openbim-core"]` override pointing to the local core checkout
   c. Runs `cargo test --workspace --release`
   d. Reports pass/fail
   e. Restores original `Cargo.toml` files
4. Returns exit code 0 if all consumers pass, 1 otherwise

## Documentation

The full procedure is documented in the `openbim-cross-repo-release` skill (golden rule, train procedure, dependency upgrade protocol, pin rollback, CI notes).

## Verified

- Script is executable and syntactically valid (`bash -n` passes)
- Patch generation produces valid TOML (tested via `cargo check` after patch)
- Cargo.toml is correctly restored after the run (verified by `git diff` showing no changes)
- `cargo test --workspace --no-run` passes after restore

## Candidate SHA

`81c7fa1` — "G-M07: add cross-repo consumer gate script"
