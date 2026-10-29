# G-M05 — Integration

## Cross-repo release

| Repo | Base SHA | Head SHA | Action |
|---|---|---|---|
| revit-alike | `7043fa5` (main before #109) | `b6fe472` (merge commit) | Merged main `9b20d2c` (#109) into `update-core-deps`; resolved `crates/document/` conflict by accepting PR #111's deletion (core holds the code) |

## Consumer verification

- `apps/desktop` consumes all shared crates from core `544ee0d`
- `crates/` directory removed from revit-alike (already done by PR #111's `d7f4883`)
- Core pin (`544ee0d`) unchanged — no core pin bump required

## Merge status

- ✅ `update-core-deps` now merges cleanly with main `9b20d2c`
- The `StampEdesignResults` command (#109) is available through core `openbim-document`
- The migration branch `update-core-deps` is ready for owner review and merge to `main`

## Next step

Owner to review and merge `update-core-deps` → `main` of PR #111 with the G-M05 merge commit.
