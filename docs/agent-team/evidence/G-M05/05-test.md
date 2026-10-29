# G-M05 — Test

## Gate commands (all run on revit-alike `update-core-deps` @ `b6fe472`)

### 1. Merge-tree clean check

```sh
git merge-tree 7043fa5 9b20d2c update-core-deps | grep -c '<<<<<<<'
```
**Exit code: 0**
```
0
```

### 2. Core contains StampEdesignResults

```sh
grep -c "StampEdesignResults" C:/Users/wilfr/.cargo/git/checkouts/openbim-core-*/544ee0d/crates/document/src/lib.rs
```
**Exit code: 0**
```
4
```

### 3. Test suite

```sh
cargo test --workspace
```
**Exit code: 0**
```
test result: ok. 272 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

### 4. Clippy

```sh
cargo clippy --workspace --all-targets --all-features
```
**Exit code: 0**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.22s
```

## Verdict

**PASS** — merge-tree clean, core contains StampEdesignResults, all tests + clippy pass.
