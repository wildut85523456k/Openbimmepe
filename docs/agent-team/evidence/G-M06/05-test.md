# G-M06 — Test

## Gate commands (core @ `bb3aca8`)

### 1. Core compile + test

```sh
cargo test --workspace --release
```
**Exit code: 0**
```
Finished `release` profile [optimized + debuginfo] target(s) in 32.37s
```

### 2. Core clippy

```sh
cargo clippy --workspace --all-targets -- -D warnings
```
**Exit code: 0**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.22s
```

### 3. Core examples

```sh
cargo build --release -p openbim-place --example place_scene -p openbim-arch --example arch_scene -p openbim-arch --example envelope_scene -p openbim-arch --example spaces_scene -p openbim-family --example fan_scene -p openbim-family --example forms_scene -p openbim-sketch --example sketch_scene -p openbim-ifc --example ifc_scene -p openbim-sheets --example portrait_scene -p openbim-sheets --example colour_scene -p openbim-sheets --example print_modes_scene
```
**Exit code: 0**
```
Finished `release` profile [optimized] target(s) in 32.37s
```

### 4. Core example execution

```sh
cargo run --release -p openbim-place --example place_scene
# Output: 321 lines
cargo run --release -p openbim-arch --example arch_scene
# Output: 20 lines
```

## Gate commands (revit-alike @ `f2473fc`)

### 5. Merge-tree clean

```sh
git merge-tree 7043fa5 9b20d2c update-core-deps | grep -c '<<<<<<<'
```
**Output: 0 (clean)**

### 6. Revit-alike tests

```sh
cargo test --workspace
```
**Exit code: 0**
```
test result: ok. 272 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

## Verdict

**PASS** — core examples compile and run; both repos test + clippy clean; merge-tree clean.
