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

- Script syntax valid (`bash -n` passes) — fixed: patch quoting (printf) + MSYS path normalization
- Patch generation produces **valid TOML** — `cargo` accepts the `[patch]` section and begins resolving dependencies (warnings about unused patches are expected; not all core crates are used by each app)
- Path normalization (`normalize_path`): converts MSYS-style `/c/Users/...` to Windows `C:/Users/...` so native cargo reads local paths correctly
- Workspace-root-only patching: `[patch]` in member `Cargo.toml` is ignored by cargo — script now patches only the workspace root `Cargo.toml`
- Cargo.toml is correctly restored after each consumer run (verified by `git diff` showing no changes to tracked Cargo.toml files)

## Bugs found and fixed during verification

1. **TOML quoting** — `echo` with `\\\"` produced `crates/"arch""` (invalid TOML). Fix: switched to `printf '%s = { path = "%s/crates/%s" }\n'` which emits clean `path = "..."`.
2. **Member Cargo.toml** — `[patch]` in `apps/desktop/Cargo.toml` was ignored by cargo. Fix: removed member_toml from CONSUMERS array; patch workspace root only.
3. **MSYS path conversion** — `/c/Users/...` mangled by native Windows cargo into `C:\c\Users\...`. Fix: `normalize_path()` converts to `C:/Users/...` for cargo; `git -C` handles both formats.

## Candidate SHA

`81c7fa1` — "G-M07: add cross-repo consumer gate script"
`102e406` — "G-M07: fix patch quoting (printf) + MSYS path normalization for native cargo"
