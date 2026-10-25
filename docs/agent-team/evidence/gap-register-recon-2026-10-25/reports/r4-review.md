# R4 independent review

Verdict: FAIL
Candidate: ebb043d3e13e9a20916337863e50ab84f56ed674
Requested base: b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be
Immediate parent: 452ea41d6d1d8cb15a98c79ebf54cbe072b6f926
Branch: docs/gap-register-reconciliation-2026-10-25
Inspected worktree: C:/Users/wilfr/hermes-first-task/wt-gap-register-recon
Reviewer: independent bim-review session, 2026-10-25, UTC+07:00.

The accuracy acceptance criterion fails on README.md:17. Completeness and table structure pass. This is not an implementation fix, Revit-equivalence approval, or approval of unrelated reconciliation claims inherited from earlier branch commits.

## Candidate and patch identity

HEAD was checked repeatedly and remained the exact candidate SHA above. Both `git diff HEAD --binary` and `git diff --cached --binary` were empty; their SHA-256 values, calculated with sha256sum, were e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.

At review entry and immediately before report creation, the only untracked candidate-worktree file was docs/agent-team/evidence/gap-register-recon-2026-10-25/r4-review-contract.md. Its Git blob identity from `git hash-object` was 673d423f64944842a2656c4d2b1c889597c50ccb. It is not part of the candidate commit or a reviewed patch. This report is a separate, uncommitted evidence artifact, created as the explicit deliverable exception to the no-edit instruction. No existing candidate file was edited.

Read project team README and reviewer role. No tracked AGENTS.md was present in the candidate tree; root AGENTS.md in the primary checkout was also absent. No domain-specific installed skill was needed for this Markdown/Cargo-manifest review. The office-P0 worktree was not inspected or touched.

## Per-claim findings

### Claim 1 — Completeness: PASS

Cargo.toml:3 explicitly lists the same member paths, in the same order, as README.md:8-27. The table has 20 workspace-member rows, including 19 crates and apps/desktop, followed by the retained spikes/* row at line 28. Search-tool counts independently returned 20 member rows, 19 crate rows and 21 total path rows. Direct comparison of the manifest and table found no missing, extra or duplicated member paths.

The original geom, family, place, keymap and arch rows and the spikes/* row are unchanged. README's header and separator remain at lines 6-7.

`git diff --exit-code b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be ebb043d3e13e9a20916337863e50ab84f56ed674 -- Cargo.toml Cargo.lock crates apps` returned exit 0 and no output. In particular, Cargo.toml has no change from the requested base; the change documents existing members rather than adding any.

### Claim 2 — Description accuracy: FAIL

All added rows were read against the candidate's own crate-root documentation, not a coder summary. Working-tree source reads are tied to the candidate by the verified empty tracked working-tree diff.

- README.md:13, oplog: PASS; crates/oplog/src/lib.rs:1-3 explicitly documents S8 binary records, per-record CRC-32 and torn-tail handling.
- README.md:14, sketch: PASS; crates/sketch/src/lib.rs:1-7 documents the small 2D sketch constraint solver.
- README.md:15, route: PASS; crates/route/src/lib.rs:1-12 documents point-by-point paths, straight runs and fittings.
- README.md:16, ifc: PASS; crates/ifc/src/lib.rs:1-2 documents that IFC4 STEP-writing input. Import support also exists in the documentation at lines 18-20; a short non-exhaustive writer summary does not deny it.
- README.md:17, view: FAIL; see finding R4-01 below.
- README.md:18, viewport: PASS; crates/viewport/src/lib.rs:1-5 documents window/GPU-independent camera logic, mouse interaction, ViewCube and software rendering.
- README.md:19, viewport-egui: PASS; crates/viewport-egui/src/lib.rs:1-2 states the same egui/wgpu widget purpose.
- README.md:20, photo: PASS; crates/photo/src/lib.rs:1-12 documents IES/LM-63 and Eulumdat photometry. The summary does not add unsupported format claims.
- README.md:21, sheets: PASS against crates/sheets/src/lib.rs:1-12, including its exact opening phrase. This is documentation agreement, not an independent certification of Revit parity.
- README.md:22, automation: PASS; crates/automation/src/lib.rs:1-2 states read-only evaluation and validated-plan document modification through the transaction API.
- README.md:23, calc: PASS; crates/calc/src/lib.rs:1-17 documents branched pipe/duct sizing and losses.
- README.md:24, document: PASS; crates/document/src/lib.rs:1-5 uses this foundation label. README adds no persistence or central-concurrency guarantee.
- README.md:25, schematic: PASS; crates/schematic/src/lib.rs:1-6 describes this pure grid-layout engine; README does not claim complete document wiring.
- README.md:26, commands: PASS; crates/commands/src/lib.rs:1-12 documents the typed command/query API and AUTO-001.
- README.md:27, apps/desktop: PASS as a generic purpose description; apps/desktop/src/main.rs:1-2 identifies an executable desktop prototype. README does not call it production-ready or claim persistence, IFC import or collaboration.

R4-01 — Low severity, acceptance-blocking documentation inaccuracy
Location: README.md:17.

The new description says "cut, projected and hidden elements." The crate-root comment instead describes cut geometry, below-cut geometry seen from above, overhead geometry and beyond geometry (crates/view/src/lib.rs:1-11). Class at lines 92-109 has Cut, Below, Overhead, Beyond, Symbol, Room and Datum, not a hidden-element class. classify at lines 336-356 returns None for geometry outside the view range; it does not produce a hidden-element classification. "Hidden" is not a supported summary of the documented overhead/beyond distinctions and could be mistaken for hidden-line or hidden-element handling.

Reproduction: inspect `git show ebb043d3e13e9a20916337863e50ab84f56ed674:README.md`, then `git show ebb043d3e13e9a20916337863e50ab84f56ed674:crates/view/src/lib.rs`, especially lines 1-11, 92-109 and 336-356.
Required remedy: replace the unsupported "hidden elements" wording with the documented classifications, for example "cut, below-cut, overhead and beyond geometry." Re-review the corrected row at a newly pinned candidate SHA. No production-code change is requested.

### Claim 3 — R4 closed annotation: PASS for presence and completeness evidence

docs/gap-remediation-plan-2026-10-21.md:69 contains "Closed 2026-10-25: the README crate table now lists all 20 members" and identifies README.md as the evidence. README.md:8-27 supplies the completeness evidence. This verifies the annotation and member coverage, not overall closure under the separate description-accuracy acceptance criterion; R4-01 prevents unconditional acceptance.

### Claim 4 — Structure / docs-only: PASS, with comparison-scope disclosure

The complete requested base-to-candidate diff changes documentation only. No production source, test, manifest or lockfile changed. `git diff --check <base> <candidate>` returned exit 0 with no output. README's diff is additions only (15 added rows, no deletions); headers, old rows and surrounding text are preserved. The R4 register row retains its table columns.

The contract's two-file list describes only the immediate commit: `git diff --numstat HEAD^ HEAD` reports README.md +15/-0 and the register +1/-1. It does NOT describe the entire requested base-to-candidate range. `git diff --name-status <base> <candidate>` also includes:

- docs/gap-progress-2026-10-23.md
- docs/agent-team/evidence/gap-register-recon-2026-10-25/coder-contract.md
- docs/agent-team/evidence/gap-register-recon-2026-10-25/coder-log.md
- docs/agent-team/evidence/gap-register-recon-2026-10-25/re-review-contract.md
- docs/agent-team/evidence/gap-register-recon-2026-10-25/reports/independent-review.md
- docs/agent-team/evidence/gap-register-recon-2026-10-25/reports/re-review.md
- docs/agent-team/evidence/gap-register-recon-2026-10-25/review-contract.md

The register itself also has inherited non-R4 edits over that base. These do not violate docs-only acceptance, but this bounded R4 review does not approve their substantive historical/feature claims. Required handoff correction: distinguish the immediate R4 commit diff from the full branch comparison rather than presenting the two-file list as the latter.

## Separate execution gates

Environment actually queried: MSYS_NT-10.0-26300 x86_64 on Windows; PROCESSOR_IDENTIFIER = Intel64 Family 6 Model 186 Stepping 2, GenuineIntel; NUMBER_OF_PROCESSORS = 16. rustc 1.99.1 (58cf8f9c3 2026-10-13), host x86_64-pc-windows-msvc, LLVM 23.1.1. cargo 1.99.1 (5f94df478 2026-08-27). No performance measurement or Revit comparison was performed.

1. Diff/Markdown whitespace: PASS.
   Command: git diff --check b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be ebb043d3e13e9a20916337863e50ab84f56ed674
   Exit 0; no output.

2. Rust formatting: FAIL, unchanged baseline code.
   Command: cargo fmt --all -- --check
   Exit 1. Reports formatting differences beginning at apps/desktop/src/app_settings.rs:14 and across the workspace. No formatter write was performed. The code-tree comparison to the requested base is empty, so this failure is not introduced by R4. Full tool-captured output: C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/terminal-output/out-1792914400-39988-1480.log.

3. Workspace release tests: BLOCKED; no completed suite result obtained.
   Command: CARGO_TARGET_DIR=C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/scratch/r4-review-target cargo test --workspace --release --locked --offline
   First foreground attempt returned exit 124 after its 180-second limit, during compilation. A 600-second-limit retry hit the tool transport's 420-second timeout without a usable result. A final tracked retry (proc_6f465d30060f) still reported compilation after a 180-second wait and was explicitly killed to bound review cost. It did not produce a passing or failing suite result. A later process listing still showed cargo PID 60268 and a rustc process; the timed-out foreground invocation could not be conclusively reconciled through process_manage (which listed no tracked processes before the final retry). Do not infer that all foreground work had stopped. No unrelated process was killed. Process-command inspection via PowerShell was approval-blocked; wmic.exe was unavailable (exit 127).

4. Clippy with warnings denied: PASS.
   Command: CARGO_TARGET_DIR=C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/scratch/r4-clippy-target cargo clippy --workspace --all-targets --locked --offline -- -D warnings
   Exit 0; finished dev profile in 2m 03s. Independent build directory; no claimed release-test coverage.

5. Dependency audit: BLOCKED for the complete online/yanked check.
   Command: cargo audit
   Exit 0, but output includes three registry request-timeout errors while checking yanked status. It loaded 1352 advisories and scanned 489 dependencies; it emitted unmaintained warnings for ttf-parser, zbus_names, zvariant and zvariant_derive and a yanked warning for clipboard-win. A zero exit code does not establish a complete audit when the tool reports retrieval failures.
   Alternative command (after reading cargo audit --help): cargo audit --no-fetch --no-yanked
   Exit 0; advisory vulnerability scan completed with four allowed unmaintained warnings. This narrower result excludes yanked checks and is NOT a substitute for the blocked full audit. No dependency or security configuration changed.

Two proposed extraction helpers were also blocked before execution by unattended-session approval policy (terminal Python -c and execute_code). They provided no evidence; normal read/search/git tools supplied the document inspection instead. No permission bypass or security-setting change was attempted.

## Handoff

Overall R4 verdict: FAIL at ebb043d3e13e9a20916337863e50ab84f56ed674 because R4-01 violates the requirement that every added description be accurate. Full engineering gates additionally contain the formatting FAIL and release-test/audit BLOCKED results above; none is silently waived by docs-only scope. No numerical, units, panic, persistence/IFC identity, undo/transaction, ownership, determinism or security implementation change is present in this diff, so this review makes no new implementation-equivalence claim.

Only this requested report was authored. No candidate documentation or production source was fixed; no commit, push, PR, merge, scheduling or office-P0 access occurred. A corrected candidate requires rechecking the affected description and diff/identity gates; this report does not approve a changed SHA.
