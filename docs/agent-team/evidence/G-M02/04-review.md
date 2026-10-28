# G-M02 — Review

## Reviewer

Independent of implementer (Hermes Agent review pass).

## Hazard checklist

| Check | Result | Notes |
|---|---|---|
| Debug tracing removed | ✅ PASS | All `eprintln!`, `id_salt`, and debug state removed from `panels.rs` and `model_view.rs` |
| `PointerMoved` removed from `click_at` | ✅ PASS | Only `PointerMoved(at)`, `button(at,true)`, `button(at,false)` in single `window_frame` call |
| Trailing `window_frame` in `click_widget` | ✅ PASS | Settles combo boxes before assertions |
| `max_height` 220→400 | ✅ PASS | All schedule kinds visible |
| Behavior-preserving | ✅ PASS | Same click target; no visual change to production UI |
| No Cargo.lock modified | ✅ PASS | Cargo.lock not in diff |
| LF line endings | ✅ PASS | Auto-crlf=false enforced |

## Risk

**LOW.** The fix corrects the test harness `click_at` simulation to match egui 0.37's atomic input-frame model. No production code changed.

## Verdict

**PASS** — fix is correct and minimal.
