# G-P10 Native window review on Windows + macOS; installer/packaging

- **Gap**: Native window review Windows + macOS; installer/packaging
- **Class**: 3 (review + new packaging on existing app)
- **Repos**: revit-alike `apps/desktop`; core dep pins
- **Acceptance**: review notes per OS; installable build on Windows 11 and Mac mini M4
- **Owner**: Wilfrid

## INTAKE

### Current state
- `apps/desktop` is an `eframe`/`egui` 0.37 desktop app (`src/main.rs` → `openbim-desktop`).
- Native dialogs via `rfd 0.17.2`; no packaging toolchain wired (no `cargo-bundle`, no Tauri,
  no WiX/NSIS). The repo ships none of: `tauri.conf.json`, `cargo-bundle` `[package.metadata.bundle]`,
  `.app`/`.dmg`/`.msi`/`.exe` build scripts.
- The desktop app pins **all** `openbim-*` core deps to `544ee0d` (the 28-Oct-2026 baseline).
  Current core `master` is `2a67204` (post G-P09). revit-alike has not been rebuilt against it.
- CI: `ci.yml` `desktop-p0` job exists on revit-alike; core has **no packaging jobs** (per G-M06).
- Host here is Windows 11. No Mac mini M4 available in this session (per the rule: a single
  OS result never closes a gap that names another OS).

### Requirement
1. Review native window behaviour on Windows 11 (eframe native window creation, sizing,
   HiDPI, menu/shortcut parity with the design doc V7/V8 notes).
2. Review on macOS (window, dock, Cmd+ shortcuts) on Mac mini M4 — **cannot be done here**.
3. Produce an installer per OS: `.msi`/`.exe` (Windows), `.dmg`/`.app` (macOS).
4. Bump revit-alike core dep pins to `2a67204` and rebuild the desktop app clean on Windows.

### DESIGN (contract)
- **Dependency bump**: rewrite every `rev = "544ee0d"` line in `apps/desktop/Cargo.toml` to
  `rev = "2a67204"` (all `openbim-*` crates).
- **Packaging**: add `[package.metadata.bundle]` to `apps/desktop/Cargo.toml` using
  `cargo-bundle` (cross-platform, produces `.app`/`.dmg` on macOS and `.msi`/`.exe` on Windows)
  — preferred over Tauri (native Rust, no JS bridge) per the "native Rust, NO JavaScript"
  constraint.
- **Windows review**: `cargo build -p openbim-desktop --release`, launch the native window,
  verify HiDPI, menu bar, ribbon. Record notes.
- **macOS review**: deferred — needs Mac mini M4 host; will be done there and the result
  appended to the evidence.
- **Acceptance**: `cargo bundle` produces an installer on Windows 11; review notes recorded.

### Status
- INTAKE done. DESIGN done (draft). Blocked on: macOS host (not available here).
- Windows part is doable in this session. Awaiting owner approval to (a) bump deps and
  (b) add `cargo-bundle` packaging.

## RESULTS

### Packaging approach
- **cargo-bundle `msi` format (REJECTED)**: cargo-bundle v0.12.0 uses the Rust `msi` crate (v0.10.0)
  for native MSI generation. The `msi` crate produces a **corrupt CFB compound file** — directory
  entries contain garbage data, making the resulting `.msi` unreadable by Windows Installer
  ("this installation package could not be opened"). Root cause: CFB directory sector corruption.
- **cargo-bundle `exe` format (REJECTED)**: fails with `WIN32_ERROR(1812)` (`ERROR_RESOURCE_NAME_NOT_FOUND`)
  because `winres_edit::Resources::load()` fails on a freshly built Rust binary with no resource section.
  Patched `winres_edit` and `exe_bundle.rs` to tolerate this, but the exe-bundle format only produces a
  repackaged binary with embedded resources, not a true installer.
- **WiX toolset v3.14.1 (ADOPTED)**: downloaded `wix314-binaries.zip` (candle.exe + light.exe), wrote a
  `.wxs` source file, compiled with `candle.exe -arch x64` and linked with `light.exe -out ...` producing
  a valid MSI with embedded CAB.

### Dependency bump
- **Not performed**: core deps remain pinned to `544ee0d` (the 28-Oct baseline). The release binary
  `target/release/openbim-desktop.exe` (27 MB) was built against this pin and verified to launch.

### Windows review
- Not performed: the release binary (`openbim-desktop.exe`) starts and stays running (GUI window opens).
  Full native window behaviour review (HiDPI, menu bar, ribbon) was not conducted in this session.

### MSI deliverables
- `apps/desktop/installer.wxs` — WiX source (committed)
- `apps/desktop/OpenBIM.msi` — 11.8 MB installer with embedded CAB (built artifact)
- `apps/assets/icon.png`, `apps/assets/icon.ico` — generated logo assets
- `apps/desktop/.gitignore` — excludes WiX build artifacts (*.wixobj, *.wixpdb, cab*.cab)

### MSI verification
- **Admin install** (`/a`): extracts `openbim-desktop.exe` (27 MB) + `icon.ico` correctly
- **Install** (`/i /qb`): exit 0, files installed to `C:\Program Files\OpenBIM\`, product registered
  in Add/Remove Programs (DisplayName: "OpenBIM", Version: 0.0.1)
- **Uninstall** (`/x /qb`): exit 0, all files removed from `C:\Program Files\OpenBIM\`, ARP entry removed
- **Executable**: `openbim-desktop.exe` launches successfully (GUI app)

### macOS
- Deferred to user (no Mac host in this session).
