# PR #72 verification report — gh-ctl + automation.sh (shell-script review)

| Field | Value |
|---|---|
| Reviewer profile | `bim-review` (independent reviewer) |
| Review type | READ-ONLY shell-script review (no Rust) |
| Repository | `wildut85523456k/revit-alike` |
| PR | #72 `Add gh-ctl and automation.sh (repo-agnostic dev tooling)` |
| Candidate SHA | **`799c1f08283d61365d9c6b37aaad425ff16fb959`** (verified: `git rev-parse HEAD` in worktree) |
| Base / merge-base | `main` @ `271c774` / merge-base `f0dc0b7` |
| Diff reviewed | `git diff f0dc0b7 799c1f0` → 2 files, +312/−0 (`gh-ctl` +227, `automation.sh` +85) |
| Read worktree | `C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr72` (detached @ 799c1f0; removed after the review) |
| Shared checkout | `C:/Users/wilfr/hermes-first-task/revit-alike` @ `feat/auto-008-obg-ds` — **not modified** |
| Host / toolchain | Windows 11, Git-for-Windows bash 5.2.37 (MSYS), GNU coreutils `stat -c` present, `/proc` present, `gh` 2.102.0 |
| Timestamp | 2026-10-23 05:57 (+07) |

## Overall verdict: **FAIL**

Three of eight acceptance items fail. The happy-path locking works, but the `gate`
subcommand never releases its lock, a crashed holder deadlocks all p0/p1 work
silently, and an unrecognised tier silently downgrades a job to the unlocked
parallel path. The `cargo` role in `automation.sh` is unreachable dead code, and
the scripts rely on GNU/Linux-only constructs.

---

## A1 — gh-ctl tiering & locking → **FAIL**

Verified **working** (evidence `raw/e3-lock-queue.txt`):
- `p2` never touches the lock — `raw/e2-p2-parallel.txt` shows `.gh-ctl` stays empty after `gh-ctl p2`.
- `p0`/`p1` acquire the atomic `mkdir` lock; while held, `p1` jobs enqueue (`[gh-ctl p1] lock held -> queued`) and the holder drains them in order (`draining queue (priority order)...` → `QUEUED-A`, `QUEUED-B`).
- Lock released on EXIT — `.gh-ctl after: []`, `status` → `LOCK: free` after the holder exits.

Two defects break the locking contract:

**F1 (HIGH) — stale lock deadlocks p0/p1 and silently no-ops the caller.**
There is no stale-lock reclamation anywhere. A holder killed with SIGKILL (or a
crash) leaves `.gh-ctl/lock`; every later `gh-ctl p0|p1` then hits the
`mkdir`-fails branch (`gh-ctl:187-192`), appends to the queue and **exits 0** —
so the caller believes the job ran, but nothing ever drains it and the command
never executes. Evidence `raw/e4-stale-lock.txt`:

```
$ bash gh-ctl p0 'echo SHOULD-RUN'      # lock dir present, owner PID 999999 (dead)
[gh-ctl p0] lock held -> queued (priority p0)
P0_EXIT=0                                <-- caller sees success
=== queue now ===
0	echo SHOULD-RUN                      <-- never executed
=== a second p0 job ===
[gh-ctl p0] lock held -> queued (priority p0)
P0B_EXIT=0
```

`gh-ctl health` does report `LOCK: STALE` (so `automation.sh` refuses to run),
but nothing in `gh-ctl` itself ever reclaims the lock — the operator must delete
`.gh-ctl/lock` by hand. File/line: `gh-ctl:187` (acquire), `gh-ctl:189` (enqueue).

**F2 (HIGH) — an unrecognised tier silently runs as unlocked p2.**
`rank()` returns `99` for anything that is not `p0|p1|p2` (`gh-ctl:35`), and the
guard `[ "$MY_RANK" -gt 1 ]` (`gh-ctl:179`) sends `99` down the p2 path — i.e.
**no lock, fully parallel**, while a real `p1` job queues behind the lock. A typo
such as `p0x`, `P0` or `p3` therefore runs a critical job unlocked, and the log
line misleadingly says `[gh-ctl p2]`. Evidence `raw/e8-badprio.txt`:

```
$ bash gh-ctl p3 'echo P3-UNLOCKED'     # lock held by a live holder
[gh-ctl p2] running (parallel): echo P3-UNLOCKED
P3-UNLOCKED                              <-- ran unlocked, concurrently
```

**F8 (LOW) — trap installed after the lock is taken.** `mkdir "$LOCK_FILE"` at
`gh-ctl:187` precedes `trap cleanup EXIT INT TERM` at `gh-ctl:205`; a signal in
that window leaves the lock held (feeds F1).

## A2 — gh-ctl subcommands → **FAIL**

**F3 (HIGH) — the `gate` subcommand never releases its lock.**
`gate` reassigns `LOCK_FILE="$REPO_ROOT/$TARGET"` (`gh-ctl:150`) and creates it
with `mkdir` (`gh-ctl:151`), but the release is
`rm -f "$LOCK_FILE.pid" "$LOCK_FILE"` (`gh-ctl:167`) — `rm -f` cannot remove a
directory, so the lock persists forever and every later `gate <same-target>`
fails with `still held — not pushing` (exit 1). Evidence `raw/e6c-gate-release.txt`:

```
[gate target-x] acquired — running: echo first
first
first_exit=0
is target-x a directory? YES
does it still exist after the run? YES
--- can rm -f remove it? ---
rm: cannot remove '.../target-x': Is a directory
rm -f exit=1
--- second gate on same target ---
[gate target-x] lock held — waiting...
[gate target-x] still held — not pushing
second_exit=1
```

(Contrast the p0/p1 path, which correctly uses `rm -rf "$LOCK_FILE"` at `gh-ctl:202`.)

**F4 (MEDIUM) — `status` and `health` use different liveness predicates.**
`status` uses `kill -0 "$OWNER"` (`gh-ctl:45`); `health` uses `[ -d "/proc/$OWNER" ]`
(`gh-ctl:84`). On this single-user host they agreed on both a live PID and a dead
PID (`raw/e5-liveness.txt`), but the predicates are not equivalent: on a
multi-user system `kill -0` fails with EPERM for a live process owned by another
user while `/proc/<pid>` still exists, so `status` would report "stale" while
`health` reports "held". `health` also counts a stale lock as a hard failure
(`FAILURES+1`) while `status` merely reports it.

**F5 (MEDIUM) — the gate lock is created in the repo root, not under `.gh-ctl/`.**
`gh-ctl:150` puts the per-target lock at `$REPO_ROOT/$TARGET`, contradicting the
header comment "Coordination primitives (all inside the repo): `.gh-ctl/lock`"
(`gh-ctl:10-12`) and dropping stray directories into the checkout.

Working subcommands (evidence `raw/e6b-gate-cargo.txt`, `raw/e6-subcommands.txt`):
`gate` with no args → usage + exit 2; `cargo` acquires/releases its own lock and
drains `cargo.queue`; `cargo` honours `CARGO_TIMEOUT`; `status`/`health` exit 0/1
correctly for free/held/stale locks.

## A3 — automation.sh dispatch → **FAIL**

**F6 (HIGH) — the `cargo` role is unreachable dead code.**
The argument parser accepts only `review|impl` (`automation.sh:44`); the
`cargo)` dispatch branch at `automation.sh:66-69` can never be selected. Confirmed
by real invocation in both a Sane and a non-Sane environment
(`raw/e7-automation-roles.txt`, and the earlier throwaway run):

```
$ REPO_ROOT=<sane throwaway repo> bash automation.sh cargo "echo hi"
automation.sh — role-based automation driver (gates on `gh-ctl health`)
...usage text...
CARGO_ROLE_EXIT=2        <-- usage + exit 2, before the health gate even runs
```

The `usage()` text (`automation.sh:24-36`) documents only `review` and `impl`, so
the branch is both unreachable and undocumented — the `cargo` role described in
the task contract cannot be invoked.

Working dispatch (evidence `raw/e7-automation-roles.txt`):
- health gate aborts a non-Sane environment: on a throwaway `REPO_ROOT` the gate
  printed `ABORT — ENVIRONMENT: NOT Sane (gh-ctl health exit 127)` and exited 1.
- `review` → `gh-ctl p2` (exit 0); `impl` on branch `main` → `WARNING: impl running
  on shared branch 'main'` then `gh-ctl p0` (exit 0).
- bad/no role → usage + exit 2; `review` with no command → usage + exit 2.

## A4 — Syntax → **PASS**

```
$ bash -n gh-ctl          exit=0
$ bash -n automation.sh   exit=0
```
Evidence `raw/e1-syntax-and-diff.txt`.

## A5 — Safety → **PASS**

- No credentials/secrets embedded: a scan for
  `token|secret|password|api[_-]?key|ghp_|github_pat_|PRIVATE KEY` over both files
  returned **no matches** (`raw/e6-subcommands.txt`).
- No unguarded destructive command on a path the user did not intend: the only
  `rm -rf` sites are `gh-ctl:129` (`$CARGO_PID`/`$CARGO_LOCK`, fixed paths under
  `.gh-ctl/`) and `gh-ctl:202` (`$LOCK_FILE`, fixed path, guarded by `HELD`). The
  `gate` path uses non-recursive `rm -f` (`gh-ctl:167`).
- The `eval` surface is understood and documented: `eval "$*"` at `gh-ctl:118,165,181,208`
  and `GHCTL_SKIP_LOCK=1 eval "$CMD"` at `gh-ctl:123,216`. This is inherent to a
  command-runner design and is disclosed by the usage text (`gh-ctl:14-18`,
  `automation.sh:15-17`); the caller-supplied string is the intended input. No
  untrusted string was eval'd during this review — every exercised command was an
  authored literal (`echo ...`).
- Residual risk (not a FAIL): because `eval "$*"` runs the caller's string, a
  caller can pass `; rm -rf <path>`; there is no allow-list. This is by design and
  documented, so it is recorded, not counted against the item.

## A6 — Portability → **FAIL**

All three constructs the contract asks about are GNU/Linux-specific where a
portable form exists (read-based; confirmed present on this MSYS host, so they
work here but fail elsewhere):

| Construct | Location | Issue | Portable alternative |
|---|---|---|---|
| `stat -c '%y'` | `gh-ctl:52` | GNU coreutils only; BSD/macOS `stat` needs `-f '%Sm'` | `stat -f '%Sm'` (BSD) / `date -r` |
| `/proc/$OWNER` | `gh-ctl:84-85` | Linux procfs only; absent on macOS and native Windows | `kill -0` (already used by `status`) |
| `sort -t$'\t' -k1,1n` | `gh-ctl:126,219` | `$'...'` is a bashism (fine here — shebang is bash), but `sort -t$'\t'` is GNU `sort` | `sort -t"$(printf '\t')"` |

Because `health` gates every `automation.sh` job, the `/proc` dependency means the
driver refuses to run on any host without procfs. `raw/e6-subcommands.txt` records
the exact source lines.

## A7 — Repo-agnostic → **PASS**

Read of both files: the only coupling to a checkout is `REPO_ROOT`
(`gh-ctl:23`, `automation.sh:21`), defaulting to `$HOME/hermes-first-task/revit-alike`.
No Dynamo, Revit, host-specific path, or Windows-only logic appears. The scripts
call only `gh`/`git`/`cargo` generically.

## A8 — No Rust/Cargo changes → **PASS**

```
$ git diff --name-only f0dc0b7 799c1f0
automation.sh
gh-ctl
$ git diff --stat f0dc0b7 799c1f0
 automation.sh |  85 +++++++++++++++++++++++++++++++++
 gh-ctl        | 227 +++++++++++++++++++++++++++++++++++++++++
 2 files changed, 312 insertions(+)
```
Diff is shell-only (`raw/e1-syntax-and-diff.txt`). The "pre-push gate skips it"
half of the item was **not** exercised — I did not run or read a pre-push hook
(none is part of this diff), so that specific claim is unverified rather than a
pass.

---

## Findings summary

| ID | Severity | File:line | Finding |
|---|---|---|---|
| F1 | HIGH | `gh-ctl:187-192` | Stale lock never reclaimed → p0/p1 enqueue forever, exit 0, job never runs |
| F2 | HIGH | `gh-ctl:35,179` | Unknown tier (`p3`/`P0`/`p0x`) silently runs unlocked as p2 |
| F3 | HIGH | `gh-ctl:150,167` | `gate` lock is a directory; `rm -f` cannot remove it → lock never released |
| F6 | HIGH | `automation.sh:44,66-69` | `cargo` role unreachable; dispatch branch is dead code |
| F4 | MEDIUM | `gh-ctl:45` vs `84` | `status` (`kill -0`) and `health` (`/proc`) use different liveness predicates |
| F5 | MEDIUM | `gh-ctl:150` | Gate lock created in repo root, not under `.gh-ctl/` |
| F7 | MEDIUM | `gh-ctl:52,84,126,219` | GNU/Linux-only constructs (`stat -c`, `/proc`, `$'\t'`) |
| F8 | LOW | `gh-ctl:187,205` | Trap installed after lock acquisition (signal window) |

## Unmet requirements

- **A1** — locking is not robust: no stale-lock recovery (F1) and no rejection of
  an unknown tier (F2); a typo silently drops a critical job to the unlocked path.
- **A2** — `gate` does not release its lock (F3); `status`/`health` liveness
  predicates differ (F4); gate lock placed outside `.gh-ctl/` (F5).
- **A3** — the documented/used `cargo` role is unreachable through the parser (F6).
- **A6** — `stat -c`, `/proc` and `$'\t'` are silently Linux/GNU-only (F7).

## Raw evidence paths

All under `docs/agent-team/evidence/repo-tools-review/bim-review/raw/`:

| File | Contents |
|---|---|
| `e1-syntax-and-diff.txt` | `bash -n` (both exit 0), `git diff --name-only/--stat f0dc0b7 799c1f0`, `git rev-parse HEAD` |
| `e2-p2-parallel.txt` | p2 leaves `.gh-ctl` empty |
| `e3-lock-queue.txt` | p0 holds lock, p1s queue, holder drains in order, p2 parallel |
| `e4-stale-lock.txt` | stale lock → p0/p1 enqueue, exit 0, never run |
| `e5-liveness.txt` | status vs health on live and dead PIDs |
| `e6-subcommands.txt` | secrets/destructive/eval/portability scans + subcommand smoke |
| `e6b-gate-cargo.txt` | gate/cargo on a fresh throwaway (gate lock persists) |
| `e6c-gate-release.txt` | gate lock never removed; `rm -f` fails on a directory |
| `e7-automation-roles.txt` | role reachability, health-gate abort, impl warning |
| `e8-badprio.txt` | unknown tier runs unlocked |
| `pr72-*.sh`, `run-all.sh` | the exact test scripts (authored literals only) |

## Commands actually run (summary)

```
bash -n gh-ctl                     exit 0
bash -n automation.sh              exit 0
git diff --name-only f0dc0b7 799c1f0   exit 0  (automation.sh, gh-ctl)
git diff --stat   f0dc0b7 799c1f0       exit 0  (+312/-0)
REPO_ROOT=<throwaway> bash gh-ctl status|health|p0|p1|p2|cargo|gate ...  (see raw/)
REPO_ROOT=<throwaway> bash automation.sh review|impl|cargo|bogus ...    (see raw/)
```

Nothing was pushed, merged, published or deleted. The judged scripts were not
modified. The detached review worktree was removed after the evidence was captured.
