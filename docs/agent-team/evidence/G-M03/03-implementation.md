# G-M03 — Implementation

## Branch

`test/cross-repo-e2e-round-trip`

## Base SHA (failing)

`92587cb` — `cargo test --workspace --no-run` fails with:
```
error[E0063]: missing field `source` in initializer of `Event`
  --> apps/electrical/tests/canvas.rs:626:14
error[E0277]: the trait bound `Id: From<&str>` is not satisfied
  --> apps/electrical/tests/editor.rs:1275:20
error: could not compile `openbim-electrical-app` (test "canvas")
error: could not compile `openbim-electrical-app` (test "editor")
```

## Candidate SHA (fixed)

`34499a2` — commit: `fix: migrate egui 0.37 (Id::from->unique, Id::new, too_many_arguments)`

## Diff (2 files, 3 insertions, 1 deletion)

```diff
--- a/apps/electrical/tests/canvas.rs
+++ b/apps/electrical/tests/canvas.rs
@@ -628,6 +628,7 @@
             delta: egui::vec2(0.0, 100.0),
             phase: egui::TouchPhase::Move,
             modifiers: egui::Modifiers::default(),
+            source: egui::MouseWheelSource::Unknown,
         }],

--- a/apps/electrical/tests/editor.rs
+++ b/apps/electrical/tests/editor.rs
@@ -797,6 +797,7 @@
 /// field (with a stable `field_id`) above the canvas. Used to verify that the
 /// canvas yields Escape to a widget that owns the keyboard focus.
+#[allow(clippy::too_many_arguments)]
 fn canvas_frame_with_field(

@@ -1272,7 +1273,7 @@
-    let field_id = egui::Id::from("properties_label_field");
+    let field_id = egui::Id::unique("properties_label_field");
```

## Machine / toolchain

- OS: Windows 11 (bash/git-bash)
- rustc: 1.99.1, edition 2021, resolver 2
- Commit `34499a2` pushed? No (migration branch not yet merged to `main`)
