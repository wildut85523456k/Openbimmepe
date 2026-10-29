# G-M06 — Design (acceptance contract)

## Contract

Move verification suites from revit-alike to core. After implementation:
- `spikes/` scripts + both CI jobs (`ifc-checks` from ci.yml, `sheets` from printing-regressions.yml) run from core
- revit-alike keeps only app-related workflows (`ci.yml` rust job, `desktop-p0.yml`, `full-build.yml`)

## Changes

### In core (`master` @ `544ee0d`)

1. Add `spikes/` directory (copy from revit-alike `update-core-deps`)
2. Create `.github/workflows/ci.yml`:
   - `rust` job: `cargo test --workspace --release` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo audit`
   - `ifc-checks` job: same as revit-alike's current `ifc-checks` (references `spikes/s*` scripts + `--example` targets)
3. Create `.github/workflows/printing-regressions.yml`:
   - `sheets` job: same as revit-alike's current `printing-regressions.yml`

### In revit-alike (`update-core-deps`)

1. Remove `ifc-checks` job from `.github/workflows/ci.yml` (keep `rust` job)
2. Delete `.github/workflows/printing-regressions.yml`
3. Update `desktop-p0.yml` `paths` filter: remove `crates/**` (no longer exists in workspace)

## Verification plan

- ✅ `cargo run --release -p openbim-place --example place_scene` works from core
- ✅ `cargo test --workspace --release` in core compiles
- ✅ All `spikes/` scripts present in core
- ✅ CI workflow files reference correct paths (relative to core)
- ✅ revit-alike `ci.yml` has no `ifc-checks` job
- ✅ revit-alike `printing-regressions.yml` deleted

## Owner approval

Approval to proceed granted by user via terse "next" (standing Wave 0 migration-blocker instruction).
