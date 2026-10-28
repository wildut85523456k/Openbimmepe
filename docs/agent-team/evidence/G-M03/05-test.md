# G-M03 — Test

## Gate commands (all run on edesign-alike `test/cross-repo-e2e-round-trip` @ `34499a2`)

### 1. Compile test at base SHA (FAIL expected)

```sh
git checkout 92587cb
cargo test --workspace --no-run
```

**Exit code: 101 (failure)**
```
error[E0063]: missing field `source` in initializer of `Event`
  --> apps/electrical/tests/canvas.rs:626:14
error[E0277]: the trait bound `Id: From<&str>` is not satisfied
  --> apps/electrical/tests/editor.rs:1275:20
error: could not compile `openbim-electrical-app` (test "canvas")
error: could not compile `openbim-electrical-app` (test "editor")
```

### 2. Compile + run tests at candidate SHA (PASS expected)

```sh
git checkout 34499a2
cargo test --workspace --release
```

**Exit code: 0 (success)**

```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (lib)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (canvas)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (editor)
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (report)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (relay)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (svg)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (chain)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (electrical)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; (round_trip)
```

Total: 78 tests pass, 0 fail.

### 3. Clippy (PASS expected)

```sh
cargo clippy --workspace --all-targets --all-features
```

**Exit code: 0 (success)**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.58s
```

## Verdict

**PASS** — fails on base SHA, passes on candidate SHA. Acceptance test criterion met.
