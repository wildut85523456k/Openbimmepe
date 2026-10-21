# Revit-alike development team

Seven persistent Hermes profiles are configured for this checkout. They run on demand; no scheduled jobs or automatic merging is enabled. Each retains its own identity and session history. They use the inherited GPT configuration initially; model diversity is optional, not a substitute for independent review.

| Profile | Responsibility | Deliverable |
|---|---|---|
| `bim-research` | Study Revit behavior and current implementation | Cited gap register and workflow acceptance contract |
| `bim-rust` | Implement approved scope in Rust | Tests, isolated worktree, candidate diff, build instructions |
| `bim-review` | Independently inspect and exercise code | Severity-ranked findings and correctness gate |
| `bim-ux` | Exercise the real app against reference workflows | Interaction evidence, screenshots, parity gaps |
| `bim-perf` | Measure compilation and runtime multicore behavior | Reproducible raw measurements and scaling report |
| `bim-reference` | Own the book-to-reference extraction and the citation re-check against a pinned SHA | Reference docs, extraction manifest, citation re-check report |
| `bim-gap` | Own the gap register: one row per gap, each with a citation, a remediation and an acceptance test | The current register and its change log |

## Working sequence

Research -> approve one task -> Rust implementation -> independent review + UX + performance -> coordinator decision -> user-approved merge/push.

The parent assistant acts as coordinator. A researcher proposal is not permission to begin an unlimited rewrite. Gates attach to the exact same candidate SHA and patch identity. A new candidate invalidates affected prior results. A FAIL returns to the coder; a BLOCKED gate cannot be waived by claiming success. Retry scope/cost must stay bounded; ask the user before broadening it. Roles coordinate through explicit task/evidence files, not shared mutable memories.

The current task is to review all declared feature gaps and branch-integration needs before selecting code work. `docs/revit-style-ui/README.md` and its linked specifications 01–05 on main are the governing requirements; this team guide and gap reports do not supersede them. Their settled decisions include Rust/egui, native operation-log/snapshot project storage, IFC4 import/export in Rust (plus IFC2X3 for DIALux), stable GlobalIds, and no proprietary RVT dependency. Review GUI, sheets and automation branch code against those decisions, recording divergence rather than inventing a replacement specification. Do not begin feature implementation or merge branches until a bounded task is approved.

## Open a specialist

```sh
hermes -p bim-research chat --in <repo-root>
hermes -p bim-rust chat --in <repo-root> --worktree
hermes -p bim-review chat --in <repo-root>
hermes -p bim-ux chat --in <repo-root>
hermes -p bim-perf chat --in <repo-root>
```

The Rust role must verify its worktree/branch before editing. To inspect a candidate, the other profiles should use their own detached worktrees, or inspect the shared checkout read-only. Use distinct CARGO_TARGET_DIR values if running different build configurations concurrently. Profile working directories are set to the project, but `--in` makes the intended scope explicit.

A bounded one-shot task can use `chat --oneshot --query-file <task.md> --max-turns 20 --run-budget 300`. Do not use `--yolo`, unattended recurring loops, shared-profile concurrent processes, or skip approval/security prompts. The five profiles were cloned without Telegram channels, so they cannot compete for the bot's polling connection.

## Acceptance/evidence format

Copy `task-template.md` for each approved task. Save reports outside production source (for example `docs/agent-team/evidence/<task-id>/`), with a subdirectory per reviewing role. Keep large binary captures and machine-specific logs out of commits unless explicitly needed and approved. Each report records actual hardware/OS/toolchain, exact SHA, commands and exit codes, and distinguishes PASS, FAIL and BLOCKED. Agent policy is not an enforced filesystem/permission sandbox.

## Revit reference

The target Revit version/edition and direct comparison access have not yet been chosen. Research uses official Autodesk documentation and explicitly supplied, licensed reference material. User-observed comparisons must identify their source. Screenshot resemblance is insufficient: command state, keyboard and mouse behavior, parameter editing, selection, cancel/undo, errors and save/reopen all matter.

## Performance reference

Build throughput: clean independent output directories for each `cargo build --release -j N`, record dependency-cache state and repeated samples. Runtime: compile once, then run identical validated fixtures with supported runtime thread controls. Report speedup `T1/Tn`, efficiency `(T1/Tn)/n`, memory and latency variability; verify equivalent results. RAYON_NUM_THREADS does not control custom std::thread pools. Hardware-specific budgets remain to be agreed; a benchmark number without an agreed budget is a measurement, not a performance PASS.

## Current scope

Creating the team does not mean application features are implemented or that UX/performance gates have passed. No production source has been changed as part of team setup, and nothing has been pushed to GitHub.
