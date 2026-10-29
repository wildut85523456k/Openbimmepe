# G-M06 — Validation

## Requirement check

| Acceptance criterion | Met? | Evidence |
|---|---|---|
| `spikes/` scripts run from core | ✅ | `03-implementation.md` §3; core examples compile + run |
| Both CI jobs (ifc-checks + sheets) run from core | ✅ | `03-implementation.md` §1; workflows created in core `.github/workflows/` |
| All green (core) | ✅ | `05-test.md` §1-4: test, clippy, examples all pass |
| revit-alike keeps only app workflows | ✅ | `03-implementation.md` §2; ifc-checks removed, printing-regressions deleted, only desktop-p0 remains (plus ci.yml rust job and full-build.yml) |
| Merge-tree clean | ✅ | `05-test.md` §5: 0 conflict markers |

## OS

Core: tested on Windows 11 (git-bash). The CI workflows run on `ubuntu-latest` (as before).
Revit-alike: tested on Windows 11 (git-bash).

## Owner action needed

Core `bb3aca8` needs to be reviewed and accepted on `master`. The `spikes/` scripts and CI workflows are new additions — the owner should verify the core CI runs green on GitHub Actions (requires IfcOpenShell + Python deps installed).

## Verdict

**PASS** — requirement met. Verification suites relocated to core; revit-alike retains only app workflows.
