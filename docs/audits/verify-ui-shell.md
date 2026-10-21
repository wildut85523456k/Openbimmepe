# UI/shell blocker verification

> Scope: six claimed blocker defects in the OpenBIM desktop shell, checked by reading code only.
> Worktree: `C:/Users/wilfr/hermes-first-task/revit-book-refs` — detached at `6ac8addc9550fcbed3d2b5cce868a540fe0a0780` (based on main `dcf8382`).
> Method: code reading only. No `cargo`, no edits, no checkouts. Line numbers are as of the working tree, which matches the pinned SHA (`git rev-parse HEAD` = `6ac8addc…`).

---

## A1 — Early `return` on non-finite bounds starves every tool branch

**Verdict: CONFIRMED**

**Real lines:** early return at **model_view.rs:143**; first tool branch at **model_view.rs:482**.

**Quoted text**

model_view.rs:120 (bounds start), :132 and :140 (the only two writers), :143 (the early return):

```rust
    let mut bounds = Rect::NOTHING;
```
```rust
        for vertex in &pm.mesh.vertices { bounds.extend_with(Pos2::ZERO + proj(*vertex).0); }
```
```rust
            for [x, y] in [start, end] { bounds.extend_with(Pos2::ZERO + proj([x, y, level_z]).0); }
```
```rust
    if !bounds.is_finite() { return; }
```

model_view.rs:482 (first tool branch, i.e. everything after the return):

```rust
    if matches!(app.tool, crate::Tool::ElecNode | crate::Tool::Cable) && !three_d {
```

**What it means**

`bounds` is extended in exactly two places, both before line 143: plan meshes that survive the view-range class test (:132) and grid datum extents (:138-141, `Geometry::Grid` only). If neither contributes a point, `bounds` stays `Rect::NOTHING` and `!bounds.is_finite()` is true, so `show` returns at :143 — before `let fit = …` (:144), before `let screen = …` (:150), before the pointer/drawing block (:451-467), and before the tool dispatch chain that starts at :482 (`ElecNode | Cable`, then `Wall` :489, `Stair` :580, `Room` :634, `Section` :653, `Elevation` :663, `WallJoins` :673, `Dimension|Measure` :699, `Move|Copy|Rotate|Mirror` :741, `Paste` :788, `Align|Trim|Offset|MatchType` :803, `Tag` :816, `Grid` :824, `Level` :845, `EndPoint` :848, `Family` :853, `WallOpening` :863, `Rebar` :882, `WallFooting` :890, `Footing` :898, `Slab|BeamSystem|Shaft|Railing|VerticalOpening` :903, `Column` :946, `Duct|Pipe` :988, and the Select/pick fallthrough :1008). In an empty or level-only project the plan therefore accepts no pointer input at all.

**Levels do not contribute to `bounds` — CONFIRMED.** `Geometry::Level` appears in the plan path only at model_view.rs:220 (`Some(Geometry::Level) if three_d => {`), which is *after* the return and is additionally gated on `three_d`. No other `Geometry::Level` arm exists in `model_view::show`. The only level-derived geometry in the plan is the `view3d::datums()` helper (view3d.rs:105-127), which `model_view.rs` never calls (`grep datums apps/desktop/src/*.rs` → only view3d.rs:105, 194, 315).

Two supporting facts:
- `all_meshes` (view_ui.rs:141-169) never emits a `PlanMesh` for a level: it keeps only `Geometry::OnWall | Linear | Point | Area | Pad` (:152-154) and, from the scene mesher, `Placed | Linear | Point | Area | InWall` (:163). `Geometry::Level` matches none of these.
- `Document::mesh` has no level arm (crates/document/src/spatial.rs:501-660) — the level element produces no mesh.
- The 3D view is unaffected: `if app.view == View::ThreeD { return crate::view3d::show(app, ui); }` is at model_view.rs:88, i.e. **before** :143.

**Confidence: high.** The early return, the ordering, and the absence of any level contribution to `bounds` are all directly quoted.

---

## A2 — Hard-coded `ElementId(10)/ElementId(11)` and a sample-only Mechanical tab

**Verdict: CONFIRMED**

**Real lines:** model_view.rs:68, :72, :74; model_edit.rs:316; panels.rs:931, :943; export.rs:123; plan_export.rs:76-78.

**Quoted text**

model_view.rs:68 — new levels are *filtered out* of the tab list by id:

```rust
        let added: Vec<(View, String)> = app.levels().into_iter().filter(|l| l.0 != ElementId(10) && l.0 != ElementId(11)).map(|l| (View::Level(l.0.0), l.1)).collect();
```

model_view.rs:72 and :74 — the two fixed tabs and the fixed Mechanical tab:

```rust
        let tabs = [("Level 1".to_string(), View::Plan), ("Level 2".to_string(), View::UpperPlan)].into_iter()
```
```rust
            .chain([("{3D}".to_string(), View::ThreeD), ("M-101 - Mechanical".to_string(), View::Mechanical)])
```

model_edit.rs:316 — `plan_level()` resolves every plan view to a hard-coded id:

```rust
        match self.view { crate::View::Level(id) if self.document.get(ElementId(id)).is_some() => ElementId(id), crate::View::UpperPlan => ElementId(11), _ => ElementId(10) }
```

panels.rs:931 — the same names for the status/title:

```rust
    match view { View::Plan => "Level 1", View::UpperPlan => "Level 2", View::ThreeD => "{3D}", View::Mechanical => "M-101 - Mechanical", View::Level(_) => "Level plan", …
```

panels.rs:943 — the project browser's `TREE` hard-codes the same three:

```rust
        folder("Floor Plans", &[view("Level 1", View::Plan), view("Level 2", View::UpperPlan), view("M-101 - Mechanical", View::Mechanical)]),
```

export.rs:123 — the claimed failure text, verbatim:

```rust
                chain.base_z = level_of(level).ok_or("a wall stands on a level that is gone")? + el.elevation_m;
```

`level_of` is export.rs:109: `let level_of = |id| doc.get(id).filter(|l| matches!(l.geometry, Some(Geometry::Level))).map(|l| l.elevation_m);`

**What it means**

`ElementId(10)` and `ElementId(11)` are not "the first two levels"; they are the ids minted by `Document::spatial_sample()` (crates/document/src/spatial.rs:654-655, `add(10, Category::Level, "Level 1", …)`, `add(11, Category::Level, "Level 2", …)`). A `Document::new()` (crates/document/src/lib.rs:564) has zero elements, so `plan_level()` returns an id that `document.get(...)` cannot resolve, the "Level 1"/"Level 2" tabs show an empty plan, and `View::Mechanical` ("M-101 - Mechanical", the app's *default* view — main.rs:287 `view: View::Mechanical`) is a view of a level that does not exist. `plan_export::plan_content` has the same hard-coding (plan_export.rs:76-78: `View::Plan => (ElementId(10), "PLAN-L1")`, `View::UpperPlan => (ElementId(11), "PLAN-L2")`, `View::Mechanical => (ElementId(10), "M-101")`), and export.rs:123 is the wall path that fails when a wall's level id is missing.

**Caveat (does not change the verdict):** the Mechanical tab is not sample-*data* in the sense of duct/pipe elements — it is a plan range over `ElementId(10)` and renders whatever is on that level. It is nonetheless hard-coded to a sample-only id, which is the claim.

**Confidence: high** for the ids, the tab labels, and the export failure string. The "~68-75 / ~315 / ~941-948" hints in the claim are all within ±2 lines of the real text.

---

## A3 — Unconditional global Escape handler at the top of `Desktop::ui`

**Verdict: CONFIRMED**

**Real line:** main.rs:567.

**Quoted text**

```rust
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.choose_tool(Tool::Select); }
```

It is the first statement of the app body, at main.rs:565-567:

```rust
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.choose_tool(Tool::Select); }
```

**What it means**

Unlike the handlers below it (Tab :568, Ctrl+Y/Z :578-582, Ctrl+P :584), this one has **no** `!ctx.egui_wants_keyboard_input()` guard, so it fires even while a text field has focus, and it runs before `self.ribbon(ui)` (:589) and `self.viewport(ui)` (:590 → `model_view::show`). Tools were written for two-stage Esc; the tool-side handlers show the intended contract, e.g. model_view.rs:563:

```rust
            if !app.wall_pts.is_empty() { app.wall_pts.clear(); app.wall_ids.clear(); app.wall_len.clear(); app.message = "Wall: chain ended; Esc again leaves the tool".into(); } else { app.choose_tool(crate::Tool::Select); }
```

Because `choose_tool(Tool::Select)` already runs first, the tool's own branch is never reached with Esc — the "Esc ends the chain" stage is unreachable through the real app. This is a genuine interaction between :567 and the tool code, not just a redundant line.

**No test exercises `Desktop::ui` — CONFIRMED.** `impl eframe::App for Desktop` is main.rs:564; `fn ui` is main.rs:565. `apps/desktop/src/main.rs` contains **no** `mod tests` and no `#[test]` (grep returns nothing). The only callers of `Desktop::ui` anywhere are production: main.rs:589 (`self.ribbon(ui)`), main.rs:646 (`self.automation.ui(...)` — that is `automation::Automation::ui`, a different type), and main.rs:668 (`eframe::run_native`). Every UI test drives a sub-function directly instead — `model_view::show` (view_ui.rs:526,534,538,577,613,644,680; model_view.rs:1221,1245,1301,1983), `panels::properties` (model_edit.rs:1814), `panels::options`, `automation::ui` (automation_canvas.rs:463), etc. So the global Esc handler, the toolbar, the ribbon and the workspace switch are all untested.

**Confidence: high.**

---

## A4 — Ctrl+S / Ctrl+N / Ctrl+O absent; `save_as` unreachable; keymap never consulted

**Verdict: CONFIRMED** (with one precision note on "no Save As button")

**Real lines:** no `Key::S`/`Key::N`/`Key::O` handler exists; `save_as` arm at main.rs:476; keymap consulted at main.rs:268 (build) and main.rs:657 (print only).

**Quoted text**

The only keyboard shortcuts handled in the whole desktop shell (main.rs:567-586):

```rust
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.choose_tool(Tool::Select); }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
```
```rust
        let redo = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))));
        else if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
```
```rust
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P)) {
```

`grep -rn "Key::S|Key::N|Key::O" apps/desktop/src/` returns **nothing** — so Ctrl+S, Ctrl+N and Ctrl+O have no handler anywhere in the app (the only `Key::*` hits are Space, Escape, Tab, Delete, Backspace, Enter, Y, Z, P, arrows, Home, End, G).

main.rs:476 — the dead arm:

```rust
            "save_as" => self.save_file(true),
```

`save_as` is the only reference to that id in the whole repo (`grep -rn "save_as" apps/ crates/` → just this line). It is not in `ribbon::QUICK_ACCESS` (ribbon.rs:46-54: `new`, `open`, `save`, `undo`, `redo`, `measure`, `settings`) and not in any ribbon group (`grep -n "level" apps/desktop/src/ribbon.rs` shows the only `level` command is the Datum one at :117; no `save_as` string exists in ribbon.rs). So `save_as` is unreachable from the UI.

The `save`/`new`/`open` commands that *are* reachable are only reachable as icon buttons (QUICK_ACCESS, ribbon.rs:47-49, wired at main.rs:376-381) — no key chord.

main.rs:268 (keymap built, never used for input):

```rust
        keymap.set_preset(&[
```

main.rs:656-657 (the only consultation — a Help-window print):

```rust
                ui.label("ESC: cancel. TAB: cycle sample selection. Mouse wheel: zoom. Middle-drag: pan. Right-drag: orbit in 3D.");
                ui.label(format!("Existing keymap crate: Duct shortcut = {:?}",
                    self.keymap.shortcuts("Duct")));
```

`grep -rn "keymap|Keymap" apps/desktop/src/` returns only main.rs:254 (field), :266, :268, :299 (init), :657 (print). The keymap's input path — `Matcher::feed` (crates/keymap/src/lib.rs:366), `command_for` (:251), `matcher` (:271) — is never called from the app.

**Precision note:** "no Save As button exists" is accurate for the shell as shipped. There is a `save` icon button (QUICK_ACCESS, ribbon.rs:49, `("save", "Save")`), so Save itself is reachable — just not by keyboard and not as "Save As".

**Confidence: high.**

---

## A5 — `slab_offset_mm` / `slab_thickness_mm` shared across unrelated tools

**Verdict: CONFIRMED**

**Real lines:** main.rs:458 (Ceiling), :459 (Roof), :462 (Door), :463 (Window), :464 (Component), :460 (Wall Footing), :467 (Footing); model_edit.rs:568 (Family), :465/:470/:526 (Ceiling), :533 (Roof), :541 (Slab), :379 (Footing), :390 (Wall Footing); panels.rs:456, :468, :483, :641, :645.

**Quoted text**

main.rs:458 — the Ceiling command sets the shared offset to 2700:

```rust
            "ceiling" => { (self.slab_kind_pick, self.slab_thickness_mm, self.slab_offset_mm, self.material_pick) = (openbim_document::SlabKind::Ceiling, 20.0, 2700.0, openbim_document::Material::Unspecified); self.choose_tool(Tool::Slab) }
```

main.rs:459 — Roof sets the same pair:

```rust
            "roof" => { (self.slab_kind_pick, self.slab_thickness_mm, self.slab_offset_mm) = (openbim_document::SlabKind::Roof, 200.0, 0.0); self.choose_tool(Tool::Slab) }
```

main.rs:462 — Door sets **only** the family; it never resets `slab_offset_mm`:

```rust
            "door" => { self.family_pick = "Door, single flush".into(); self.family_type.clear(); self.family_kind = Some(openbim_family::Category::Door); self.choose_tool(Tool::Family) }
```

main.rs:463 and :464 (Window, Component) are the same shape — `family_pick`, `family_type`, `family_kind` only.

model_edit.rs:568 — the Family tool consumes the shared offset as its elevation:

```rust
        let spec = FamilySpec { family: self.family_pick.clone(), type_name: self.family_type.clone(), overrides: Vec::new(), angle_deg: self.angle_deg, elevation_m: self.slab_offset_mm / 1000.0 };
```

model_edit.rs:465, :470, :526 (Ceiling), :533 (Roof), :541 (Slab) — the other consumers of the same two fields:

```rust
        let height = self.slab_offset_mm / 1000.0;
```
```rust
        match self.document.place_ceiling(level, shape.outline, height, self.slab_thickness_mm / 1000.0, self.material_pick) {
```
```rust
        match self.document.place_ceiling(level, self.slab_points.clone(), m(self.slab_offset_mm), m(self.slab_thickness_mm), self.material_pick) {
```
```rust
        let spec = openbim_document::RoofSpec { outline: self.slab_points.clone(), thickness_m: m(self.slab_thickness_mm), base_offset_m: m(self.slab_offset_mm),
```
```rust
        let spec = SlabSpec { outline: self.slab_points.clone(), thickness_m: m(self.slab_thickness_mm), top_offset_m: m(self.slab_offset_mm),
```

model_edit.rs:379 and :390 — the footings read the same offset:

```rust
        let spec = FootingSpec { width_m: m(self.width_mm), length_m: m(self.height_mm), thickness_m: m(self.footing_thickness_mm), top_offset_m: m(self.slab_offset_mm), material: self.material_pick };
```
```rust
        let spec = WallFootingSpec { width_m: m(self.width_mm), depth_m: m(self.height_mm), top_offset_m: m(self.slab_offset_mm), material: self.material_pick };
```

panels.rs:456 and :468 — the options bar exposes the *same* field for Wall Footing and isolated Footing as "Base offset (mm)"; panels.rs:483 exposes it for Family as "Offset (mm)"; panels.rs:641/:645 expose the pair for Slab:

```rust
                if matches!(field, Field::Elevation) { ui.add(egui::DragValue::new(&mut app.slab_offset_mm).speed(1.0)); } else { editor(app, ui, field); }
```
```rust
            ui.add(egui::DragValue::new(&mut app.slab_thickness_mm).speed(1.0).range(1.0..=5000.0));
```
```rust
            ui.add(egui::DragValue::new(&mut app.slab_offset_mm).speed(1.0));
```

**What it means**

The two fields are one piece of shared shell state, and the tool commands are asymmetric: Floor (:450), Roof (:459), Ceiling (:458), Slab (:468) and Foundation (:467) *set* them, while Door (:462), Window (:463) and Component (:464) do not. `choose_tool` (main.rs:303-318) clears `beam_start`, `wall_pts`, `edit_pts`, `slab_points`, `opening_from`, … but **not** `slab_offset_mm` or `slab_thickness_mm`. So the exact sequence claimed reproduces: Ceiling sets `slab_offset_mm = 2700.0`, then Door leaves it at 2700.0, and `family_click` reads `elevation_m: 2700.0 / 1000.0 = 2.7` — a door placed 2.7 m up. The same leak reaches Wall Footing, Footing and the free-standing Component. `choose_tool` does reset `family_kind` for non-Family tools (:304), which is why the Door command still has to set it explicitly.

**Confidence: high.** Note this is a *default-value* leak, not a bug in the Ceiling placement itself — the Ceiling command's own placement is correct (model_edit.rs:465/:470 use the value it just set).

---

## A6 — `place_level` accepts duplicates and <100 mm gaps that export later rejects; no rename/delete

**Verdict: CONFIRMED** (one correction: the "no rename" half is not a missing capability — rename exists at the model level and is simply never surfaced for levels)

**Real lines:** place_level at crates/document/src/lib.rs:689-702; the rejection at crates/arch/src/lib.rs:82-92 (`MIN_LEVEL_GAP` at :18); the export call site at apps/desktop/src/export.rs:289.

**Quoted text**

crates/document/src/lib.rs:689-702 — the only check is on *elevation*, by a 1 mm tolerance:

```rust
    pub fn place_level(&mut self, name: &str, elevation_m: f64) -> Result<ElementId, EditError> {
        let id = self.next_id();
        if !elevation_m.is_finite() || self.elements.values().any(|e| e.geometry == Some(Geometry::Level) && (e.elevation_m - elevation_m).abs() < 1e-3) { return Err(EditError::InvalidGeometry(id)); }
        let name = if name.trim().is_empty() {
            let mut n = self.elements.values().filter(|e| e.geometry == Some(Geometry::Level)).count() + 1;
            while self.elements.values().any(|e| e.geometry == Some(Geometry::Level) && e.name == format!("Level {n}")) { n += 1; }
            format!("Level {n}")
        } else { name.trim().to_string() };
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Level, name, width_m: 1.0, height_m: 1.0, elevation_m },
            Command::SetGeometry { id, geometry: Geometry::Level },
        ])?;
        Ok(id)
    }
```

Note the two gaps in that code: (a) an **explicit** name is taken verbatim with no uniqueness check — `place_level("Level 1", 3.0)` on a document that already has "Level 1" succeeds; (b) the tolerance is `1e-3` (1 mm), so `place_level("", 0.05)` beside an existing 0.0 m level succeeds, and even `place_level("", 0.09)` succeeds.

The validator that would catch a malformed level element does not cover names: crates/document/src/spatial.rs:219,

```rust
            Some(Geometry::Level) => el.category == Category::Level && el.elevation_m.abs() <= 1e6,
```

crates/arch/src/lib.rs:18 and :82-92 — the rules export enforces:

```rust
pub const MIN_LEVEL_GAP: f64 = 0.1;
```
```rust
    pub fn new(mut list: Vec<Level>) -> Result<Levels, ArchError> {
        if list.len() > MAX_LEVELS { return Err(ArchError::TooMany("levels")); }
        for l in &list { check_name(&l.name)?; fin(l.elevation, "level elevation")?; }
        list.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        for w in list.windows(2) {
            if w[1].elevation - w[0].elevation < MIN_LEVEL_GAP { return Err(ArchError::LevelsTooClose { a: w[0].name.clone(), b: w[1].name.clone() }); }
        }
        let mut names: Vec<&str> = list.iter().map(|l| l.name.as_str()).collect();
        names.sort_unstable();
        if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) { return Err(ArchError::DuplicateName(w[0].to_string())); }
        Ok(Levels { list })
```

apps/desktop/src/export.rs:289 — where that rejection surfaces to the user:

```rust
        levels: Levels::new(levels).map_err(|e| format!("levels: {e:?}"))?,
```

`levels` is built at export.rs:109-110 from *every* `Geometry::Level` element:

```rust
    let levels: Vec<Level> = doc.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level)))
        .map(|e| Level { name: e.name.clone(), elevation: e.elevation_m }).collect();
```

**What it means**

The document accepts a state the exporter refuses: two levels 1-99 mm apart, or two levels with the same name. The app then reports `levels: LevelsTooClose { … }` / `levels: DuplicateName("…")` as an IFC export failure, with no in-app affordance to fix it — which is the third half of the claim:

- **Levels cannot be deleted:** `delete_selected` hard-refuses a level (model_edit.rs:870):
  ```rust
          if ids.iter().any(|id| self.document.get(*id).is_some_and(|e| e.category == Category::Level)) { self.message = "A level cannot be deleted here".into(); return; }
  ```
- **Levels cannot be renamed:** `Command::Rename { id, name }` exists (crates/document/src/lib.rs:524, applied at :1240) and the Properties panel has a Rename path for *types* only (model_edit.rs:42, :60). No `Command::Rename` for a level is ever issued from the UI (`grep -rn "Command::Rename" apps/desktop/src/` → no hits), and there is no "Rename Level"/"Delete Level" label anywhere in `apps/desktop/src` (grep returns nothing).

**Correction to the claim's framing:** rename is *not* missing from the model — `Command::Rename` is a general element command. The accurate statement is: the desktop UI never offers rename or delete for a level, and `delete_selected` actively blocks the generic delete path. The claim's conclusion ("levels cannot be renamed or deleted") is still correct as a UI statement.

**Confidence: high** on the accept/reject divergence and the export failure; **medium-high** on "cannot be renamed" (it is true of the shipped UI, but a `Command::Rename` exists that a future UI could use).

---

## Bonus

**`#[test]` count**

| scope | count |
| --- | --- |
| `crates/` | 973 |
| `apps/` | 169 |
| `apps/desktop/` | 169 (all of the app-side tests) |
| **total** | **1142** |

Command: `grep -rn "#\[test\]" crates/ apps/ --include=*.rs | wc -l` → `1142`.

**Does any test start from an empty document (`Document::new()`) rather than `Desktop::sample()`?**

**No test that renders or drives the UI from an empty document.** The `Document::new()` uses are all in model/file round-trip helpers, not in a rendered frame:

- `crates/document/src/lib.rs` — 12 tests construct `Document::new()` and transact ops directly (e.g. :1313, :1332, :1357, :1370, :1382, :1391, :1401, :1411, :1424, :1433), plus `crates/document/src/edit.rs:395`, `layers.rs:405`, `native.rs:466`, `opening.rs:168/:236/:317`. These never touch `Desktop`, `model_view::show`, or any egui context.
- `apps/desktop/src` — `Document::new()` appears at elec_ui.rs:656, :705; file.rs:20 (a `document_from` helper); import.rs:554, :556, :781, :803, :833, :879, :905, :934, :951; insulation_ui.rs:261; mep_settings_ui.rs:217; model_edit.rs:2063; schedule_ui.rs:544; systems_ui.rs:439. Every one is a persistence/round-trip assertion (`back` / `saved` / `read` / `again`) — none renders a frame or asserts on plan input.
- `apps/desktop/src/file.rs:200-207` (`opening_or_starting_over_with_unsaved_changes_asks_first`) does drive `app.new_model()` and asserts `app.document.elements().count() == 0` — the only test that reaches an empty model — but it asserts on the document, never calls `model_view::show`, and never sends a pointer event.

Correspondingly, **no test covers the empty-project plan**: every rendering/plan test in `apps/desktop` builds `Desktop::sample()` — `Desktop::sample()` appears 119 times across `apps/desktop/src`, and the `frames()`/`frame()` helpers used by the model-view tests (model_view.rs:1245, 1301, 1983; view_ui.rs:464) all take a `Desktop` the test built from `sample()`. So A1's empty-plan input starvation has no test, and the `bounds`-is-finite early return at model_view.rs:143 is never exercised by a test.

---

## Summary table

| claim | verdict | real line |
| --- | --- | --- |
| A1 — early `if !bounds.is_finite() { return; }` before all tool branches; levels never in `bounds` | **CONFIRMED** | model_view.rs:**143** (return), :**482** (first tool branch), :120/:132/:140 (bounds writers), :220 (level arm, `three_d`-gated, after return) |
| A2 — hard-coded `ElementId(10)/ElementId(11)` and sample-only Mechanical tab; `Document::new()` has neither | **CONFIRMED** | model_view.rs:**68**, :**72**, :**74**; model_edit.rs:**316**; panels.rs:**931**, :**943**; export.rs:**123**; plan_export.rs:76-78; sample ids at crates/document/src/spatial.rs:654-655 |
| A3 — unconditional global Escape → Select at the top of `Desktop::ui`, before the viewport | **CONFIRMED** | main.rs:**567** (`impl eframe::App` :564, `fn ui` :565); tool contract at model_view.rs:563 |
| A3b — no test exercises `Desktop::ui` | **CONFIRMED** | main.rs:564-565; no `mod tests`/`#[test]` in main.rs; all UI tests call `model_view::show`/`panels` directly |
| A4 — Ctrl+S/N/O unhandled; `save_as` arm unreachable; keymap never consulted for key events | **CONFIRMED** | no `Key::S/N/O` in `apps/desktop/src`; main.rs:**476** (`save_as`); main.rs:**268** (build), :**657** (print only) |
| A5 — `slab_offset_mm`/`slab_thickness_mm` shared; Ceiling 2700/20 then Door places 2.7 m up | **CONFIRMED** | main.rs:**458** (Ceiling 2700), :**459** (Roof), :**462** (Door, no reset); model_edit.rs:**568** (`elevation_m: self.slab_offset_mm / 1000.0`); `choose_tool` main.rs:303-318 does not clear it |
| A6 — `place_level` accepts duplicate names and <100 mm gaps that export rejects; no level rename/delete | **CONFIRMED** | crates/document/src/lib.rs:**689-702**; crates/arch/src/lib.rs:**18**, :**82-92**; export.rs:**289**; delete blocked at model_edit.rs:**870**; `Command::Rename` exists (lib.rs:524) but is never issued for a level |
| Bonus — `#[test]` count | — | **1142** total (crates 973 + apps 169) |
| Bonus — any UI test from `Document::new()` | **No** | all rendering/plan tests use `Desktop::sample()` (119 uses); `Document::new()` uses are round-trip only |
