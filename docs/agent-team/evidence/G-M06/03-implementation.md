# G-M06 — Implementation

## Changes

### In core (`master` @ `bb3aca8`)

1. Added `spikes/` directory — 9 subdirs, 14 Python scripts (copy from revit-alike)
2. Created `.github/workflows/ci.yml`:
   - `rust` job: `cargo test --workspace --release` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo audit`
   - `ifc-checks` job: all IFC spike checks (place, rooms, layers, spaces, connectors, forms, sketch, ifc_scene, sheets)
3. Created `.github/workflows/printing-regressions.yml`: sheets printing regression checks

### In revit-alike (`update-core-deps` @ `f2473fc`)

1. Removed `ifc-checks` job from `.github/workflows/ci.yml` (kept `rust` job)
2. Deleted `.github/workflows/printing-regressions.yml`
3. Deleted `spikes/` directory
4. Updated `.github/workflows/desktop-p0.yml` paths filter: removed `crates/**` (no longer in workspace); removed stray `main` line (pre-existing YAML typo)

## Verification (core)

- `cargo build --release -p openbim-place --example place_scene` → exit 0
- `cargo run --release -p openbim-place --example place_scene` → 321 lines output
- `cargo run --release -p openbim-arch --example arch_scene` → 20 lines output
- All 13 `--example` targets compile (place_scene, arch_scene, envelope_scene, spaces_scene, fan_scene, forms_scene, sketch_scene, ifc_scene, portrait_scene, colour_scene, print_modes_scene, sheets_scene)

## Verification (revit-alike)

- `cargo test --workspace --no-run` → exit 0 (compiles)
- `cargo test --workspace` → 272 passed, 0 failed
- `cargo clippy --workspace --all-targets --all-features` → exit 0

## Merge-tree check

```sh
git merge-tree 7043fa5 9b20d2c update-core-deps | grep -c '<<<<<<<'
# Output: 0 (clean)
```

## Core candidate SHA

`bb3aca8` — "G-M06: add spikes/ verification scripts and CI workflows"

## Revit-alike candidate SHA

`f2473fc` — "G-M06: move IFC/sheets CI+spikes to core; revit-alike keeps only app workflows"
