# G-M02 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M02 |
| **Gap** | P0-4 two-storey office journey fails under egui 0.37: room Number/Name typed in Properties not applied |
| **Class** | 2 (wiring — platform migration regression) |
| **Repos** | revit-alike |
| **Depends** | — |
| **Acceptance** | `the_two_storey_office_is_drawn_saved_reopened_and_exported_through_the_window` passes on the candidate; whole desktop suite 0 failed |
| **Base SHA** | `7038d88` (main, egui 0.36) — was pre-existing failure |
| **Candidate SHA** | `d8cd1c3` (includes G-M01 fix `d867ab1`) |
| **Wave** | 0 — migration blockers |
| **Status** | ✅ closed @ `d8cd1c3` |

## Register entry (verbatim)

> G-M02 · P0-4 two-storey office journey fails under egui 0.37: room Number/Name typed in Properties not applied (`apps/desktop/src/model_view.rs` ~L1819). Passes at core `d7cdc1d` (egui 0.36) · Class 2 · revit-alike (+core if widget ids) · Acceptance: `the_two_storey_office_is_drawn_saved_reopened_and_exported_through_the_window` passes on the candidate; whole desktop suite 0 failed
