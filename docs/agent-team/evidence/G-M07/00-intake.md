# G-M07 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M07 |
| **Gap** | No consumer gate / SHA-bump automation (README promises a bot). Core bumps break apps silently (G-M01..M03 are the proof) |
| **Class** | 3 (new tooling on existing architecture) |
| **Repos** | core, both apps |
| **Depends** | G-M06 (closed) |
| **Acceptance** | documented and scripted procedure: a core candidate is built and tested by both consumers via `[patch]` before the bump lands (openbim-cross-repo-release) |
| **Candidate SHA** | `81c7fa1` (revit-alike, includes script) |
| **Wave** | 0 — migration blockers |
| **Status** | ✅ closed @ `81c7fa1` |

## Register entry (verbatim)

> G-M07 · No consumer gate / SHA-bump automation (README promises a bot). Core bumps break apps silently (G-M01..M03 are the proof) · Class 3 · core, both apps · G-M06 · documented and scripted procedure: a core candidate is built and tested by both consumers via `[patch]` before the bump lands (openbim-cross-repo-release)
