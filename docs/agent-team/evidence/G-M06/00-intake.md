# G-M06 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M06 |
| **Gap** | Verification suites lost: revit-alike `ci.yml` (`ifc-checks` job: `cargo run -p openbim-place/-arch/-family/-sketch/-ifc --example …` + `spikes/s2b…s12/*.py` with IfcOpenShell) and `printing-regressions.yml` (`-p openbim-sheets` examples + `spikes/s13_sheets_check/*.py`) build crates that leave the workspace. Core has no CI |
| **Class** | 3 (new CI + script migration on existing architecture) |
| **Repos** | core, revit-alike |
| **Depends** | — |
| **Acceptance** | `spikes/` scripts + both jobs run from core, all green; revit-alike workflows keep only app jobs (`desktop-p0`) |
| **Candidate SHA** | (to be assigned) |
| **Wave** | 0 — migration blockers |
| **Status** | IN PROGRESS |

## Register entry (verbatim)

> G-M06 · Verification suites lost: revit-alike `ci.yml` (`ifc-checks` job: `cargo run -p openbim-place/-arch/-family/-sketch/-ifc --example …` + `spikes/s2b…s12/*.py` with IfcOpenShell) and `printing-regressions.yml` (`-p openbim-sheets` examples + `spikes/s13_sheets_check/*.py`) build crates that leave the workspace. Core has no CI · Class 3 · core, revit-alike · Acceptance: `spikes/` scripts + both jobs run from core, all green; revit-alike workflows keep only app jobs (`desktop-p0`)

## Scope

1. Move `spikes/` directory (9 subdirs, 14 Python scripts) from revit-alike to core
2. Create `.github/workflows/ci.yml` in core with the `ifc-checks` job
3. Create `.github/workflows/printing-regressions.yml` in core with the `sheets` job
4. Remove `ifc-checks` job from revit-alike `ci.yml`
5. Remove `printing-regressions.yml` from revit-alike
6. Update `desktop-p0.yml` paths filter (remove `crates/**` which no longer exists)
