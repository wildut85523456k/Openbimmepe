# G-M05 — Implementation

## Branch

`update-core-deps` (PR #111)

## Merge

```sh
git checkout update-core-deps
git merge --no-edit 9b20d2c  # main with #109 StampEdesignResults
```

## Conflict

```
CONFLICT (modify/delete): crates/document/src/electrical_exchange.rs deleted in HEAD and modified in 9b20d2c.
CONFLICT (modify/delete): crates/document/src/lib.rs deleted in HEAD and modified in 9b20d2c.
```

## Resolution

Accept PR #111's deletion — the `StampEdesignResults` code from #109 is already in core `544ee0d`:

```sh
rm crates/document/src/electrical_exchange.rs crates/document/src/lib.rs
git add -A
git commit --no-edit -m "Merge: resolve crates/document conflict by accepting core (544ee0d has StampEdesignResults)"
```

## Candidate SHA

`b6fe472` — Merge commit resolving the `crates/document/` conflict

## Core verification

```sh
# Core document crate at 544ee0d contains StampEdesignResults
grep -c "StampEdesignResults" $CARGO_HOME/git/checkouts/openbim-core-*/544ee0d/crates/document/src/lib.rs
# Output: 4
```

## Merge-tree verification

```sh
git merge-tree 7043fa5 9b20d2c update-core-deps | grep -c '<<<<<<<'
# Output: 0 (clean)
```

## Machine / toolchain

- OS: Windows 11 (git-bash)
- rustc: 1.99.1, edition 2021
