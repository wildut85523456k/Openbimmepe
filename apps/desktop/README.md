# OpenBIM desktop (Rust / egui / wgpu)

```sh
cargo run --locked -p openbim-desktop
```

Windows needs MSVC Build Tools and the Windows SDK. Linux needs X11/Wayland and graphics libraries.
Use the committed Cargo.lock. No private repository access is needed: this binary has no electrical solver, so
single-line schematics show placeholders. `full/desktop-full` is the same app with the edesign-alike solver installed.

## Current implementation

The desktop contains a ribbon, contextual controls, searchable Properties/Project Browser, light/dark themes,
level-derived plan views, sections/elevations and a 3D viewport. Tools cover basic architectural placement and
editing, MEP runs/systems, family catalogue/type editing, schedules, sheets and PDF/IFC exchange.

Save/Open persists the model in its native format. Unsaved work is journaled for recovery according to Options;
normal Save clears that recovery journal. The initial sample is a starting scene, not a persistence limitation.

These are partial workflows. The [capability register](../../docs/capability-register.md) records remaining work,
including associative dimensions, crop/templates, graphical family authoring and collaboration.
Old renderer/UI milestone reports are historical evidence, not an up-to-date missing-feature list.

## Validation

See [P0 validation](../../docs/p0-validation.md). Headless event tests run the egui window body; they do not launch
a native window or validate OS file dialogs. Native builds, native startup, visual review and performance must
each have their own evidence. The [full office acceptance contract](../../docs/p0-office-acceptance.md) stays open.
