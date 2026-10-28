# G-M04 — Integration

## Cross-repo release

| Repo | Base SHA | Head SHA | Action |
|---|---|---|---|
| edesign-alike | `34499a2` | `13a2740` | Fix applied to migration branch `test/cross-repo-e2e-round-trip` |

## Consumer verification

- No consumer pins edesign-alike (edesign-alike is a leaf app, not a dependency of revit-alike)
- revit-alike does not depend on edesign-alike crates; no pin bump required
- core does not depend on edesign-alike

## Merge status

- The migration branch `test/cross-repo-e2e-round-trip` is **not yet merged to `main`**
- Merging to `main` is blocked by G-M05 (merge conflicts between PR #111 and main) and G-M07 (no consumer gate / SHA-bump automation)
- G-M04 itself is self-contained: test-only fix in `catalog.rs`; the acceptance criterion is met on the migration branch

## Owner action needed

Owner to merge `test/cross-repo-e2e-round-trip` → `main` as part of the edesign-alike egui 0.37 migration (G-M05/G-M07 gate the merge).
