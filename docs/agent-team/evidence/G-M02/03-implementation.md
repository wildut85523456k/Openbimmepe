# G-M02 — Implementation

## Branch

PR #111 `update-core-deps`

## Root cause

`click_at` in `apps/desktop/src/model_view.rs` sent a `PointerMoved` event before the button-press/release events, inside the same frame. Under egui 0.37, the scroll-area layout of the Properties panel was re-evaluated mid-click, displacing the Apply button from the coordinates recorded by the preceding `widget()` lookup. The button click therefore hit the scroll area's empty interior instead of the button — `apply.clicked()` returned `false` for all 23 frames.

Secondary: the schedule dialog's `new_schedule_kinds` `ScrollArea` had `max_height(220)`, clipping the "Doors" option and hiding a reachable button.

## Fix (at `d8cd1c3`, on top of G-M01 `d867ab1`)

Three changes in `apps/desktop/src/`:

1. **`model_view.rs` `click_at`** — Removed the standalone `PointerMoved` event; merged `PointerMoved(at)`, `button(at, true)`, `button(at, false)` into a single `window_frame` call so the pointer position and button events are delivered atomically within the same input frame.
2. **`model_view.rs` `click_widget`** — Added a trailing `window_frame(ctx, screen, app, vec![])` after the click to let combo boxes and scroll areas settle before the next assertion.
3. **`schedule_ui.rs`** — Increased `new_schedule_kinds` scroll area `max_height` from 220 to 400 so all schedule kinds (including Doors) are visible.

## Diff summary

```
apps/desktop/src/model_view.rs    | 5 +++--
apps/desktop/src/schedule_ui.rs   | 2 +-1
2 files changed, 4 insertions(+), 2 deletions(-)
```

## Candidate SHA

`d8cd1c3` — "G-M02 fix: egui 0.37 pointer click dispatch in click_at; G-M03 evidence" (includes G-M01 fix `d867ab1` as ancestor)

## Machine / toolchain

- OS: Windows 11 (git-bash)
- rustc: 1.99.1, edition 2021
