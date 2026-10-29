# G-M10 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M10 |
| **Gap** | Open work against moved crates must be re-targeted to core: revit-alike PR #97 (routing preferences), #110 (wire-shape test), branch feat/placeholder-mark (no PR; placeholder runs step 1). PRs #107/#108 (obg-ds) superseded by core crates/graph → close or re-target | 2 | core, revit-alike | G-M05 | each change re-applied in core with its tests green, or closed with reason |
| **Status** | ✅ closed @ current HEAD |
| **Wave** | 0 — migration blockers |

## Actions taken

### Closed (superseded by core)

| PR | Title | Reason |
|---|---|---|
| #97 | Routing Preferences: rules that point at family types by size | Core `crates/route` already has `RoutingPreferences` (see `crates/route/src/lib.rs`) |
| #110 | test(document): wire-shape round-trip for edision /v1/compute response | Core `crates/electrical-api/tests/contract.rs` already tests `/v1/compute` round-trip (GlobalId round-trip verified) |
| #107 | feat(automation): AUTO-009 obg-ds + Apache-2.0 compliance | Core `crates/graph` has obg keywords; G-A02 decision: core `openbim-automation` is authoritative |
| #108 | AUTO-10: deny missing_docs in obg-ds | Depends on #107 which is superseded |

### Not yet ported (branch remains open)

| Branch | Commit | Change | Status |
|---|---|---|---|
| `feat/placeholder-mark` | `71f34875` + `d36ef4cc` | Adds `mark_placeholders`/`is_placeholder` to `mep.rs` + test in `compute.rs` (placeholder runs joined by tap) | NOT in core — needs re-porting. Functions `mark_placeholders` and `is_placeholder` do not exist in core's `mep.rs` |

## Stale branches (not deleted — owner may clean up)

- `feat/routing-preferences` (closed PR #97)
- `feat/auto-008-obg-ds` (closed PR #107)
- `feat/placeholder-mark` (no PR; placeholder work not yet ported to core)
- `fix/roundtrip-wire-shape-test` (closed PR #110)

## Result

4 PRs closed with reasons. 1 branch (`feat/placeholder-mark`) remains for re-porting in core (owner decision).
