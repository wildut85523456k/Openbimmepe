# P0 validation and native builds

Baseline: b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be (25 October 2026).
No native build/test PASS is claimed for this candidate until the checks execute.

## Native matrix

`.github/workflows/desktop-p0.yml` runs on Ubuntu 24.04 and Windows Server 2022 for pull requests, and additionally on macOS 14
when dispatched manually (`include_macos`, default on). It does not run on push to main: the PR run already tests the merged tree.
Each job records git HEAD and rustc host/version, resolves Cargo.lock with --locked, runs the office pointer
regression in debug, desktop debug tests, workspace release tests and Clippy, then builds/uploads the executable.
Artifacts are named by OS and event SHA. On pull_request the tested checkout is GitHub's merge candidate;
the printed HEAD is authoritative. This is not an installer or native-window startup check.

Existing CI, independent IFC checks and printing-regressions remain in place. The new matrix does not silently
replace those gates. macOS Intel, native startup, rendering, file dialogs and performance are separate pending checks.

## Private dependency (full build only)

Since 25 October 2026 the main workspace does not depend on edesign-alike: `openbim-document` defines the
`ElectricalCalc` interface and the open build installs no engine. The solver is plugged in by `full/`
(`openbim-electrical-adapter`, pinned at 57ef01848860771d29960dcf2e8af2f831fe1e6c), a separate workspace
excluded from the main one, so the open workflows need no secret and fork PRs build.

Only the manual `full-build.yml` workflow needs the repository Actions secret `P0_DEPENDENCY_READ_TOKEN` with
**read-only contents** access to edesign-alike. Never put a token in code or a report. The job uses an
environment-reading credential helper and removes it at job end; checkout credentials are not persisted.
Do not use pull_request_target to expose credentials to untrusted PR code.

## Evidence at preparation

- GitHub source inspection: baseline above.
- Host toolchain detected: cargo 1.99.1; rustfmt 1.10.0-stable.
- HTTPS git clone: blocked by host DNS resolution.
- Local checkout clone: blocked by filesystem/MSYS access.
- Therefore this workspace cannot claim Cargo compilation or test execution from a full checkout.
- Previously inspected GitHub runs failed before useful step evidence; runner/account failures remain distinct from Rust failures.
- Workflow configuration alone does not fix unavailable runners or account-level Actions restrictions.

## Completion gates

Keep the candidate in draft until all native jobs and existing checks pass on the exact candidate.
Record run URLs, SHA, target, commands, pass/fail/ignored counts and remaining exclusions.
A failed infrastructure/authentication preflight is BLOCKED validation; a compiler/test failure is FAIL and must be fixed.
The [capability register](capability-register.md) must be updated only with actual results.
