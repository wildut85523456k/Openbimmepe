# Openbimmepe — independent clean rebuild candidate

This is an **independent** Rust/eGUI openBIM authoring repo — a clean rebuild
alternative to `revit-alike`. It is the buildable, verifiable candidate for the
`OpenBIM-REA-2026-10-31` gap-register refresh (START-HERMES pack), kept separate
from in-progress corrections landing on `revit-alike`.

## Provenance

- Forked from `revit-alike` at commit `76963c0`
  (`feat(viewport): family tool preview handled by the application, not viewport overlays`)
- Baseline tag: `v0.0.1-candidate-76963c0`.
- Candidate core pin: `openbim-core` @ `544ee0d7c035cb0dffa57f5981094f28fe7ce591`
  — the **PR#111 actual** core pin audited by START-HERMES.md. `master` (`eda121c`)
  is deliberately **not** used (it lacks crates the app needs).
- egui / eframe / egui_extras `0.37`.
- Local-only: the `origin` remote is stripped; nothing auto-pushes or merges.
- Intentionally **does not** carry `revit-alike`'s uncommitted `c3bad7a` bump +
  compile fixes — that is the other agent's in-progress work. Openbimmepe builds
  the committed PR#111 candidate independently; the `c3bad7a` follow-up is a
  documented next step (see `docs/openbimmepe/S0-INVENTORY.md`).

## Layout (post core-dep migration)

Shared library crates now live in the `openbim-core` monorepo and are consumed as
git-SHA-pinned dependencies (all `@ 544ee0d7`): `openbim-keymap`,
`openbim-commands`, `openbim-document`, `openbim-oplog`, `openbim-arch`,
`openbim-geom`, `openbim-ifc`, `openbim-place`, `openbim-calc`, `openbim-family`,
`openbim-route`, `openbim-view`, `openbim-viewport-egui`, `openbim-sheets`,
`openbim-clash`, `openbim-automation`.

`apps/desktop` → `openbim-desktop` is the desktop application.

## Build / verify

```sh
cargo build    -p openbim-desktop
cargo test     -p openbim-desktop
cargo test     --workspace --release
cargo clippy   --workspace --all-targets -- -D warnings
cargo audit
```

Windows requires MSVC Build Tools + Windows SDK; Linux needs X11/Wayland/graphics
dev libs. The workspace builds with no private access. The electrical solver is
the separate private `edesign-alike` repo, wired only by `full/`; without it the
app stamps calculations as "not calculated".

Example/test sizes and engineering values are illustrative, not design recommendations.
