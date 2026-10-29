# G-M11 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M11 |
| **Gap** | `cargo fmt --check` on core reports ~3,855 hunks; no fmt gate anywhere | 1 | core | owner OK for one format-only commit | single format-only commit, then fmt gate in core CI |
| **Status** | ✅ closed @ `4d8a1fa` (core) |
| **Wave** | 0 — migration blockers |

## What was done

1. **Format-only commit**: Ran `cargo fmt` on core master at `bb3aca8` (G-M06). Changed 210 files (48,586+ insertions, 9,957− deletions). Commit: `102a720` (later squashed into `4d8a1fa`).
2. **CI fmt gate**: Added `cargo fmt --check` step to the `rust` job in `.github/workflows/ci.yml`. Added `rustfmt` to the toolchain components.

## Verification

- `cargo fmt --check` passes on `4d8a1fa` (no formatting differences)
- CI workflow includes `cargo fmt --check` before `cargo test --workspace --release`

## Candidate SHA

`4d8a1fa` — "style: cargo fmt + ci: add cargo fmt --check gate" (core master)
