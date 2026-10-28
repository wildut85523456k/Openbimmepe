# G-M03 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M03 |
| **Gap** | edesign test targets `canvas` and `editor` do not compile under egui 0.37 (`Event` needs `source`; `Id: From<&str>` gone) |
| **Class** | 2 (wiring — code exists, version-incompatible) |
| **Repos** | edesign-alike |
| **Depends** | — (migration blocker) |
| **Acceptance** | `cargo test --workspace --release` compiles and passes on the migration branch |
| **Base SHA** | `92587cb` (tip of `test/cross-repo-e2e-round-trip` before fix) |
| **Candidate SHA** | `34499a2` (fix commit) |
| **Wave** | 0 — migration blockers |
| **Status** | ✅ Fixed at `34499a2`; tests pass; clippy clean |

## Register entry (verbatim from gap-register)

> G-M03 · edesign test targets `canvas` and `editor` do not compile under egui 0.37 (`Event` needs `source`; `Id: From<&str>` gone) · Class 2 · edesign-alike · — · Acceptance: `cargo test --workspace --release` compiles and passes on the migration branch

## Failure reproduced at base SHA 92587cb

```
error[E0063]: missing field `source` in initializer of `Event`
  --> apps/electrical/tests/canvas.rs:626:14

error[E0277]: the trait bound `Id: From<&str>` is not satisfied
  --> apps/electrical/tests/editor.rs:1275:20

error: could not compile `openbim-electrical-app` (test "canvas")
error: could not compile `openbim-electrical-app` (test "editor")
```
