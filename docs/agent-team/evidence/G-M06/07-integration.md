# G-M06 — Integration

## Cross-repo release

| Repo | Base SHA | Head SHA | Action |
|---|---|---|---|
| openbim-core | `544ee0d` | `bb3aca8` | Added `spikes/` scripts + `.github/workflows/ci.yml` (ifc-checks) + `.github/workflows/printing-regressions.yml` (sheets) |
| revit-alike | `4444104` (PR #111 head) | `f2473fc` | Removed `ifc-checks` from ci.yml; deleted `printing-regressions.yml`; deleted `spikes/`; fixed `desktop-p0.yml` paths |

## Consumer verification

- revit-alike `update-core-deps` (`f2473fc`) compiles and tests pass (272/272) with core pinned at `544ee0d`
- core `bb3aca8` compiles, tests pass, all 13 spike-referenced examples build
- Merge-tree between `7043fa5`/`9b20d2c` (main) and `update-core-deps` is clean (0 conflict markers)

## Owner action needed

- Core `bb3aca8` needs push to `master` (done — pushed to `origin/master`)
- Reviewer to verify core CI runs green on GitHub Actions (requires IfcOpenShell + Python deps)
- Owner to merge `update-core-deps` → `main` of PR #111 (depends on G-M05 merge resolution)
