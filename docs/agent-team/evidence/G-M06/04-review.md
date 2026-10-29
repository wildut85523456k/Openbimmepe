# G-M06 — Review

## Reviewer

Independent of implementer (Hermes Agent review pass).

## Hazard checklist

| Check | Result | Notes |
|---|---|---|
| No production code moved | ✅ PASS | Only scripts, CI workflows, and test files moved |
| Core examples compile | ✅ PASS | All 13 `--example` targets build from core |
| Core workspace tests pass | ✅ PASS | `cargo test --workspace` exit 0 |
| Revit-alike workspace tests pass | ✅ PASS | 272 passed, 0 failed |
| Clippy clean (both repos) | ✅ PASS | Exit 0 on both |
| Merge-tree clean | ✅ PASS | 0 conflict markers |
| Core contains StampEdesignResults | ✅ PASS | (G-M05 verified) |
| LF line endings | ✅ PASS | `autocrlf=false` set in core |
| No secrets / logs committed | ✅ PASS | Scripts and workflows only |
| desktop-p0 stray `main` fixed | ✅ PASS | Removed pre-existing YAML typo |

## Risk

**LOW.** The change moves CI jobs and scripts to core (which now owns the crates) and removes them from revit-alike. No production code changes. The examples are already in core and compile. The merge-tree is clean after the G-M05 conflict resolution.

## Verdict

**PASS** — CI and spike scripts correctly relocated to core; revit-alike retains only app workflows.
