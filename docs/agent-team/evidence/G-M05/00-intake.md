# G-M05 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M05 |
| **Gap** | PR #111 conflicts with main (#109 changed `crates/document/src/electrical_exchange.rs`, `lib.rs`; branch deletes `crates/`). Core already holds #109's code |
| **Class** | 2 (wiring — merge conflict between two branches) |
| **Repos** | revit-alike |
| **Depends** | — |
| **Acceptance** | merge-tree clean; diff proves core `document` contains `StampEdesignResults` |
| **Base SHA** | `7043fa5` (main before #109) |
| **Main SHA** | `9b20d2c` (#109 StampEdesignResults) |
| **Candidate SHA** | `b6fe472` (merge commit resolving the conflict) |
| **Wave** | 0 — migration blockers |
| **Status** | ✅ closed @ `b6fe472` |

## Register entry (verbatim)

> G-M05 · PR #111 conflicts with main (#109 changed `crates/document/src/electrical_exchange.rs`, `lib.rs`; branch deletes `crates/`). Core already holds #109's code · Class 2 · revit-alike · Acceptance: merge-tree clean; diff proves core `document` contains `StampEdesignResults`

## Conflict reproduced

```
CONFLICT (modify/delete): crates/document/src/electrical_exchange.rs deleted in HEAD and modified in 9b20d2c.
CONFLICT (modify/delete): crates/document/src/lib.rs deleted in HEAD and modified in 9b20d2c.
Automatic merge failed; fix conflicts and then commit the result.
```
