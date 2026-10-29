# Verification Report — PR #58: AUTO-008 (Dynamo ProtoCore replication port) + AUTO-009 (Apache-2.0 compliance)

- **Reviewer role / profile:** `bim-review` (independent reviewer; read-only on the judged artefacts)
- **Repository:** `wildut85523456k/revit-alike`, local clone `C:/Users/wilfr/hermes-first-task/revit-alike`
- **Candidate branch/SHA:** `feat/auto-008-obg-ds` @ **`6dad47e8be391eab236b3d0a9a6ccaced9683907`**
- **Base branch/SHA:** `main` @ `480927122d4b4300611fdfd37442b1f910676586`
- **Exact diff:** `git diff 4809271 6dad47e` — **28 files, +4912 / -4** (brief said ~+4911/-3; +1 insertion / +1 deletion counting drift only, see F1)
- **Uncommitted patch:** none — reviewed against the committed SHA only; the candidate worktree was left clean
- **Timestamp:** 2026-10-22T19:26:27+07:00 (Asia/Phnom_Penh)
- **Host / toolchain:** Windows 11, git-bash/MSYS. `cargo 1.99.1 (5f94df478 2026-08-27)`, `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo-audit 0.22.2`, `clippy 0.1.99`. Python 3 for JSON/grep cross-checks.
- **Review worktree (detached, read-only):** `C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr58` @ `6dad47e` (created with `git worktree add --detach`; removed at the end of the review).
- **Isolated build dir:** `CARGO_TARGET_DIR=C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr58-review-target` — the shared checkout's build was never touched.
- **Pinned upstream sources fetched for independent cross-checking** (not part of the repo): DynamoDS/Dynamo @ `8a211c5f306e5b54541fa57081aa1ffa32a65e90` — `LICENSE.txt`, `ABOUT.txt`, `src/Engine/ProtoCore/Lang/Replication/{Replicator,ArgumentAtLevel,ReplicationInstruction}.cs`, `src/Engine/ProtoCore/Lang/CallSite.cs`, `test/Engine/ProtoTest/TD/Associative/Replication.cs`, `test/Engine/ProtoTest/AtLevel/AtLevelTest.cs`, plus the repo tree listing.

> Note on the brief: `AGENTS.md` does not exist in this repository (only `docs/agent-team/README.md`, which was read and followed). The pinned Dynamo tree *does* contain an `AGENTS.md`; that is upstream, not this repo.

## Commands actually run (all in the detached worktree, isolated target dir)

| # | Command | Exit |
|---|---|---|
| 1 | `cargo check --workspace --release --all-targets` | **0** (0 warnings) |
| 2 | `cargo test -p obg-ds` | **0** (12 tests) |
| 3 | `cargo test -p openbim-commands` | **0** (16 tests) |
| 4 | `cargo clippy --workspace --all-targets -- -D warnings` | **0** (0 warnings) |
| 5 | `cargo test --workspace --release --no-fail-fast` | **0** (115 test binaries, 1235 passed, 0 failed) |
| 6 | `cargo audit` | **0** (4 pre-existing unmaintained-crate warnings, 0 vulnerabilities) |
| 7 | `bash -n gh-ctl && bash -n automation.sh` | **0** |

Raw logs: `raw-gate-check.log`, `raw-gates.log` (this directory).

---

## A1 — Rebase conflict resolutions correct — **PASS**

All three resolutions were verified structurally *and* by a clean, warning-free build.

**(1) `Cargo.toml` workspace `members`.** The single changed line is a strict union:
`… "crates/document", "crates/schematic", "crates/obg-ds", "crates/commands", "apps/desktop"`.
`crates/schematic` (main) is retained; `crates/obg-ds` and `crates/commands` (PR) are added. No member was dropped.
`Cargo.lock` adds exactly two packages — `obg-ds` and `openbim-commands` (`git diff 4809271 6dad47e -- Cargo.lock | grep '^+name'` → 2 lines) — and `openbim-desktop` gains the `openbim-commands` dependency. No third-party crate was newly introduced.

**(2) `apps/desktop/src/main.rs` struct init.** Field-set diff of `struct Desktop` between base and candidate:
`base=133 fields`, `new=134 fields`; `comm -23 base new` → **empty** (nothing dropped); `comm -13 base new` → **`show_about`** (the PR's field). The init expression carries both sides: `… show_help: false, show_about: false, dark: false,` and the `#[cfg(test)] ribbon_hits: Vec::new()` field. Evidence: `fields_base.txt`, `fields_new.txt`.

**(3) `apps/desktop/src/main.rs` ribbon render.** `main.rs:670` reads
`let response = ribbon::big_button(ui, cmd.id, ribbon::label(cmd.id, cmd.label), self.dark, active, enabled);`
— main's newer `big_button(ui, id, label, dark, active, enabled)` signature (`ribbon.rs:304`) **and** main's `#[cfg(test)] hits.push((cmd.id, response.rect));` / `response.clicked()` logic, **combined with** the PR's `ribbon::label(cmd.id, cmd.label)` registry lookup (`ribbon.rs:21-28`, backed by `openbim_commands::lookup`). `ribbon::label` is genuinely used (1 call site) — a dropped `ribbon::label` would have produced a `dead_code` warning, and command 1 reported **0 warnings**.

**Build:** `cargo check --workspace --release --all-targets` → exit **0**, `Finished release profile … in 5m 56s`, **0 warning lines** in the log.

## A2 — obg-ds replication phases — **PASS**

All four phases are present in `crates/obg-ds/src/` and the structure was cross-checked against the pinned Dynamo sources fetched at `8a211c5f…`:

| Phase | Port | Dynamo source (pinned) | Checked |
|---|---|---|---|
| COMPILE | `replication.rs`: `build_instructions`, `build_replication_combinations`, `build_reductions`, `reduction_to_instructions`, `max_reduction_depths` | `Replicator.cs` `BuildPartialReplicationInstructions` / `BuildReplicationCombinations` / `BuildReductions` / `ReductionToInstructions` / `GetMaxReductionDepth` | guide-grouping by level, ascending guide sort, `guide <= 0` stub skip, Longest wins within a group, zip-first then Cartesian — all match |
| RESOLVE | `replication.rs`: `compute_all_reduced_params`, `estimate_reduced_params`; `engine.rs`: `select_plan` | `Replicator.cs` `ComputeAllReducedParams` / `EstimateReducedParams`; `CallSite.cs` endpoint matching | rank view + trial selection present (see F2 for a doc-accuracy caveat) |
| RUNTIME | `engine.rs`: `execute` (mirrors `CallSite.Execute`/`ExecWithRISlowPath`), `restore_dominant` | `CallSite.cs:1405-1440, 1455-1565`; `ArgumentAtLevel.cs:219-278` | scalar == length-1 array; `hasEmptyArg → retSize = 0`; Shortest=min / Longest=max; dominant restoration only when the outermost instruction involves the dominant argument — all match `CallSite.cs:1481-1544` |
| AT-LEVEL | `replication.rs`: `argument_at_level`, `apply_at_levels`, `get_elements_at_level` | `ArgumentAtLevel.cs:75-114, 116-160, 184-204` | `level >= 0` = no extraction; negative = `maxDepth + level` with promotion when the reduction exceeds the depth — matches |

`operations.rs` supplies the operator layer (`scalar_add/sub/mul` with list broadcasting, `sum_rank_one`, `identity_any_rank`). `value.rs` supplies the `Value` model and `max_depth`.

## A3 — `replication-v1.json` conformance — **PASS**

`cargo test -p obg-ds` → exit **0**; the fixture test binary reports:

```
running 2 tests
test gated_cases_refuse_with_unresolved ... ok
test non_gated_cases_byte_equal ... ok
test result: ok. 2 passed; 0 failed
```

- The fixture (`docs/revit-style-ui/fixtures/replication-v1.json`, **unchanged by this PR** — present at base `4809271` since commit `9813646`/PR #55) holds **9 `cases`** and **6 `gated_cases`**; `Case::from_json` marks a case gated iff it carries a `settings` block, and all 6 gated entries do.
- `non_gated_cases_byte_equal` iterates all 9 non-gated cases and asserts `evaluate(case) == expected` (byte-equal `Value` equality) — all 9 pass.
- `gated_cases_refuse_with_unresolved` asserts `gated.len() == 6` and that each gated case returns exactly `Err("UNRESOLVED_REPLICATION_PROFILE")` — all 6 pass.

**Independent cross-check of the fixture's expected values against the pinned Dynamo source** (not just trusting the fixture): the 9 non-gated expectations match `Replication.cs` T01/T04 and the `@L` semantics of `AtLevelTest.cs` at `8a211c5f…` (e.g. `auto-flat` = T01 `list3 = {6,12,10,8}`; `auto-scalar` = T04 `list2 = {6,7,8,9,10}`; `level-keep` = `TestDominantList01Case1` `{{3,7},{11,15}}`). The fixture itself declares `"expected_output_status": "source-derived; not locally executed Dynamo results"`, which is honest; the reviewer verified the derivation against the pinned tree. See F4 for the tautology caveat on the gated half.

## A4 — Dynamo manifest — **PASS**

`crates/obg-ds/tests/dynamo_manifest.rs` (new in this PR) is a pinned manifest of transcribed Dynamo input→output cases. `dynamo_manifest_conformance` → **ok** (1 test).

Every entry records `source` (Dynamo test method + line span at the pinned SHA), `inputs`, `operation`, `lacing`, `at_level`, `keep`, `expected`, `encoding`. The reviewer verified the transcriptions against the pinned Dynamo test files fetched from `8a211c5f…`:

| Manifest entry | Pinned Dynamo source | Verified |
|---|---|---|
| `T01_Arithmatic_List_And_List_Different_Length` (L12-25) — list3/4/5 | `Replication.cs:12` | `{6,12,10,8}`, `{-4,-4,4,-4}`, `{5,32,21,12}` — **match** |
| `T02_…_Same_Length` (L38-54) | `Replication.cs:38` | **match** |
| `T04_…_Single_List_And_Integer` (L70-91) | `Replication.cs:70` | `list2 = {6,7,8,9,10}` — **match** |
| `T09_Defect_1456568_…_2` (L1576-1586) | `Replication.cs:1576` | `[1,2]*[3,4,5] = {3,8}` — **match** |
| `T09_Defect_1456568_…_3` (L1590-1600) | `Replication.cs:1590` | `[1,2,5,7]-[3,4] = {-2,-2}` — **match** |
| `T58_Defect_1456115_…_3` (L1859-1878) | `Replication.cs:1859` | `foo([1],[1,2,3]) = {2}` — **match** |
| `TestAtLevel01` (L8-105) — 5 levels | `AtLevelTest.cs:8` | `@-1`→flat 16, `@-2`→8 pairs, `@-3`→4 quads, `@-4`→`xs`, `@-5`→`{xs}` — **match** |
| `TestDominantList01Case1` (L122-134) | `AtLevelTest.cs:122` | `Sum(xs@@-2) = {{3,7},{11,15}}` — **match** |
| `TestDominantList01Case2` (L137-151) | `AtLevelTest.cs:138` | `Sum(xs@@-1) = {{{36}}}` — **match** |

The manifest also records the one case Dynamo's own test marks as divergent (T01 `list6 = list2 / list1`, division) as out of the ported subset. 16 entries total (`assert!(entries.len() >= 15)`).

## A5 — commands typed API — **PASS**

`cargo test -p openbim-commands` → exit **0**:
- unit tests: **12 passed; 0 failed** (`command`, `parameter`, `quantity`, `unit` modules);
- `tests/acceptance.rs`: **4 passed; 0 failed** (`a_dimension_mismatch_is_an_error`, `a_label_looks_up_the_stable_id`, `the_ribbon_commands_are_registered`, `a_quantity_travels_as_a_value_and_a_unit`).

`crates/commands` compiles as workspace member `openbim-commands`; `apps/desktop` depends on it (`apps/desktop/Cargo.toml`) and consumes it via `ribbon::label`. The API is genuinely typed: `Quantity` = SI value + `Dimension` (mismatch is a `DimensionMismatch` value error), `ParameterId` = stable ID with EN/FR labels, `CommandDef` = the registry the ribbon/keymap/graph/MCP are generated from.

## A6 — Apache-2.0 compliance — **PASS**

Verified against the pinned Dynamo tree at `8a211c5f306e5b54541fa57081aa1ffa32a65e90` (fetched live) and `04c §8`:

1. **`LICENSE.txt`** — 201 lines, CRLF. Byte-identical to `DynamoDS/Dynamo@8a211c5f…/LICENSE.txt` after CRLF→LF normalisation:
   `repo-LICENSE.normalized.txt` sha256 `ba665b8054e15c1a960e307832b521780a6bcce3ea505beaa090c15083555b19`
   = `pinned-dynamo-LICENSE.normalized.txt` sha256 `ba665b8054e15c1a960e307832b521780a6bcce3ea505beaa090c15083555b19` ✓ **IDENTICAL**.
   It is the Apache-2.0 text; the only delta vs the canonical `apache.org/licenses/LICENSE-2.0.txt` is the APPENDIX copyright line, which Dynamo has filled in as `Copyright [2020] [Autodesk, Inc.]` (see F3).
2. **`THIRD_PARTY_NOTICES`** — 939 lines, CRLF. Byte-identical to `DynamoDS/Dynamo@8a211c5f…/ABOUT.txt` after CRLF→LF normalisation:
   `repo-THIRD_PARTY_NOTICES.normalized.txt` sha256 `9b4be5bb6dc3c27ed95e4b682d5f1aad6932913ea5b8b7234608881205035423`
   = `pinned-dynamo-ABOUT.normalized.txt` sha256 `9b4be5bb6dc3c27ed95e4b682d5f1aad6932913ea5b8b7234608881205035423` ✓ **IDENTICAL** (939 = 939 lines).
3. **No `NOTICE.txt` at the pinned SHA** — verified against the upstream tree listing at `8a211c5f…` (root blobs: `ABOUT.txt`, `LICENSE.txt`, `README.md`, … — no `NOTICE.txt`); matches `PORTING.md`'s claim, so §4(c) applies to the notices actually carried.
4. **Per-file headers** — all **7** ported `obg-ds` files carry all required elements (grep count = 1 each for every element, every file):
   `crates/obg-ds/src/{value,operations,replication,engine,lib}.rs`, `crates/obg-ds/tests/{dynamo_manifest,replication_fixtures}.rs`
   — `DynamoDS/Dynamo@8a211c5f…` + source path ✓, `Copyright 2026 Autodesk, Inc.` ✓, `Apache License, Version 2.0` reference ✓, `Ported to Rust; modified from the original.` ✓, plus `SPDX-License-Identifier: Apache-2.0` ✓.
5. **`PORTING.md`** — indexes each ported file with its Dynamo source path @ SHA and the nature of the change (§8.3/§8.5), carries the dependency audit (§8.4: `Newtonsoft.Json` MIT → replaced by `serde_json`; `RestSharp`/`Lucene.Net` Apache-2.0 not ported; `LibG`/`ProtoGeometry`/`ForgeUnits`/`Analytics`/`IDSDK` closed-source **not ported**; `PythonNet3` unverified/out of scope), the trademark/compatibility statements (§8.6) and the patent-grant note (§8.8).
6. **`apps/desktop/src/about.rs`** — ships both texts in-app: `LICENSE_TEXT = include_str!("../../../LICENSE.txt")`, `THIRD_PARTY_NOTICES = include_str!("../../../THIRD_PARTY_NOTICES")`, rendered in the "About OpenBIM" window (`about::window`, opened from the Help menu at `main.rs:525`); unit tests assert the licence text and the Autodesk/Ian Keough credits and the closed-source statement are present. This satisfies §8.1's "deliver the licence text with every redistributed artifact", not only in the source tree.
7. **Test data (§8.7)** — `test/core/packageDependencyTests/*` is **not** reused (see A8); the transcribed cases are input→output evidence only.

## A7 — Tests + gates — **PASS**

| Gate | Result |
|---|---|
| `cargo check --workspace --release --all-targets` | exit **0**, **0 warnings** |
| `cargo test -p obg-ds` | exit **0** — 9 unit + 1 manifest + 2 fixture = **12 tests**, 0 failed (matches the expected 12) |
| `cargo test --workspace --release --no-fail-fast` | exit **0** — 115 test binaries, **1235 passed, 0 failed**, 0 ignored-failures |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit **0**, **0 warnings** |
| `cargo audit` | exit **0** — 4 **pre-existing** `unmaintained` warnings, **0 vulnerabilities** |

Audit detail (all four crates are present at base `4809271` too, i.e. **not introduced by this PR**; the PR adds only `obg-ds` + `openbim-commands` to `Cargo.lock`): `ttf-parser 0.25.1` (RUSTSEC-2026-0192), `zbus_names 4.3.4` (RUSTSEC-2026-0371), `zvariant 5.15.0` (RUSTSEC-2026-0370), `zvariant_derive 5.15.0` (RUSTSEC-2026-0372) — all transitive (egui/zbus), all `unmaintained` (warning class, not vulnerabilities).

## A8 — Clean-room / no copied fixtures — **PASS**

- `git ls-tree -r --name-only 6dad47e | grep -c '\.dyn$'` → **0**. No Dynamo `.dyn` or test fixtures are copied.
- `packageDependencyTests` appears only in the *exclusion* statements (`04c-dynamo-engine-port.md:341,443`, `PORTING.md:75`); no fixture from that tree is reused.
- `unsafe` in the diff appears only as `unsafe_code = "forbid"` / `#![forbid(unsafe_code)]` (8 occurrences, all forbidding); **no `unsafe` block** in any added code. `crates/commands` and `apps/desktop` both forbid it.
- No verbatim C# in the diff (`grep -c 'namespace ProtoCore|using System|public static List<ReplicationInstruction>'` on added lines → **0**); the port is ~1300 lines of hand-written Rust. The Dynamo algorithm was re-implemented, and the reviewer confirmed the re-implementation matches the pinned sources structurally (A2).

---

## Findings

- **F1 Low — brief vs actual diff size.** The brief says "28 files, ~+4911 / -3"; the exact `git diff --numstat 4809271 6dad47e` is **28 files, +4912 / -4**. Counting drift only; no gate is affected.
- **F2 Low — `compute_all_reduced_params` doc mis-attributes the zip semantics.** `crates/obg-ds/src/replication.rs:234-238` claims `expand_instruction` "Mirrors the one-phase unbox in `ComputeAllReducedParams` (Replicator.cs lines 218-270)" including "an empty collection drives the zip length to 0 (CallSite.cs lines 1543-1544)". In the pinned Dynamo source, `ComputeAllReducedParams` does **not** implement zip truncation/longest/empty→0 — it expands each index independently and `continue`s on an empty array (leaving the original value in place); the zip-length semantics live in `CallSite.ExecWithRISlowPath` (`CallSite.cs:1481-1544`). The port's `execute` mirrors the *CallSite* behaviour correctly; only the comment on the RESOLVE helper is inaccurate. **No functional impact**: `compute_all_reduced_params` is not on the engine's resolve/execute path (only `estimate_reduced_params` + `execute` are) and is exercised solely by the `reduced_params_flat_and_ordered` unit test.
- **F3 Low — `LICENSE.txt` appendix is filled in.** `LICENSE.txt` is byte-identical to the pinned Dynamo `LICENSE.txt`, whose APPENDIX line reads `Copyright [2020] [Autodesk, Inc.]` rather than the canonical `Copyright [yyyy] [name of copyright owner]` (and the file has no leading blank line). It is still the Apache-2.0 text verbatim *as distributed by the pinned source*, which is what §8.1 asks for; noted so the coordinator knows it is not the unmodified `apache.org` text.
- **F4 Info — the gated half of A3 is refused by construction.** `engine.rs:246-249` (`is_refused`) returns `true` for any `case.gated` before the resolver runs, so the 6 gated cases are refused unconditionally rather than by exercising the resolver. This is the intended AUTO-008 policy ("refuse rather than silently approximate") and the fixture marks them `evidence_status: "unresolved; no Dynamo result asserted"`, so nothing is overclaimed — but the gated gate does not demonstrate the engine *detects* those shapes.
- **F5 Low — `PORTING.md` header claim overstates the JSON row.** `PORTING.md:24-25` says "Every file below carries its own header in-file", but the table's `docs/revit-style-ui/fixtures/replication-v1.json` row is JSON and carries no header (and is pre-existing, not added by this PR). Cosmetic documentation inaccuracy.
- **F6 Info — `Encoding::Refused` is unreachable in the manifest.** `dynamo_manifest.rs` defines `Encoding::Refused` and a `matches()` arm for it, but all 16 entries use `Encoding::Exact`; the `Refused` path is never taken (and `matches()` would return `false` for it). Harmless, but the variant/arm are effectively dead.
- **F7 Low — `select_plan` silently degrades when no trial matches.** `engine.rs:237-242` falls back to `ReplicationPlan { instructions: vec![] }` when no trial's reduced ranks match the operator rank, i.e. it executes with *no* replication instead of refusing. This is contrary to the stated "refuse rather than silently approximate" policy for shapes outside the pinned evidence, and no fixture case exercises it (all 9 non-gated cases find a matching trial). Worth an explicit `Err(UNRESOLVED)` if the corpus ever grows.

No correctness, security, concurrency, determinism or numerical defects were found in the ported algorithm or the typed command API.

---

## Per-item verdict

| Item | Verdict |
|---|---|
| A1 Rebase resolutions correct (union + 0-warning build) | **PASS** |
| A2 obg-ds replication phases (COMPILE/RESOLVE/RUNTIME/AT-LEVEL) | **PASS** |
| A3 `replication-v1.json` conformance (9 byte-equal, 6 refuse) | **PASS** |
| A4 Dynamo manifest pins transcribed cases | **PASS** |
| A5 commands typed API compiles + acceptance passes | **PASS** |
| A6 Apache-2.0 compliance artefacts + per-file headers | **PASS** |
| A7 Tests + gates (check/test/clippy/audit) | **PASS** |
| A8 Clean-room / no copied fixtures / no `unsafe` | **PASS** |

**Overall verdict: PASS** — candidate SHA `6dad47e8be391eab236b3d0a9a6ccaced9683907`.

All eight acceptance items are satisfied on the rebased candidate. Unmet requirements: **none**. Findings F1–F7 are Low/Info and none blocks the merge; F2, F5, F7 are documentation/robustness nits the implementer may address in a follow-up.

Evidence in this directory: `raw-gate-check.log`, `raw-gates.log`, `repo-LICENSE.normalized.txt`, `repo-THIRD_PARTY_NOTICES.normalized.txt`, `pinned-dynamo-LICENSE.normalized.txt`, `pinned-dynamo-ABOUT.normalized.txt`, `canonical-apache-2.0.normalized.txt`, `dynamo-license.txt`, `dynamo-about.txt`, `apache-license.txt`, `fields_base.txt`, `fields_new.txt`. Machine-specific build logs also remain at `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr58-review-target/{gate-check.log,gates.log}`.
