# P0 office acceptance contract

Scope approved 25 October 2026: start P0 from the architecture-book review.
Reference: Architecture 2026 chapters 2-8 and 11-13. This is an original test fixture, not copied book content.

## Evidence levels

1. Model/API fixture: `office.rs` builds two storeys and checks persistence/exchange.
2. Window event regression: `model_view.rs::tests::the_small_office_is_built_from_a_new_project_then_saved_reopened_and_exported`.
   The P0 candidate clicks New and tool buttons and places grid/walls/floor/room/section with pointer events through
   `Desktop::window`. Tabs and wall location are still fixture setup. Save/Open/IFC still use direct methods.
3. Complete office interaction: OPEN. Level/parameter fields, tabs, dialogs and documentation actions must be operated.
4. Native window/file dialogs and visual output: OPEN. Headless shape counts do not close this gate.

## Complete journey

| Step | User interaction | Independent observable result |
|---|---|---|
| New | Click New; confirm unsaved changes when needed | Two real levels, no sample elements; default plan accepts input |
| Datums | Rename levels, set 3.6 m spacing, draw grids | Browser labels, elevations and model levels agree |
| Envelope | Draw two-storey walls and floors, door/window, stair, roof and ceiling | Expected category counts, hosted cuts and closed geometry |
| Rooms | Place two rooms and edit number/name | Closed boundaries; correct area and live room schedule |
| Change | Move a host wall, edit a door size, undo and redo | Host/opening follows; unrelated elements unchanged; final state reproducible |
| Annotation | Place tag and supported dimension | Correct view ownership and value; explicitly expose non-associative limitation |
| Schedule | Create room/door schedules using dialog fields | Independent counts and quantities match model |
| Sheet | Create sheet, place plan/section/schedule | View scale and title fields retained; no unintended clipping |
| Persistence | Save, close and reopen through controls | Full native records and stable IDs match |
| Output | Export PDF and IFC through dialogs | Independent PDF page/scale check and IFC entities/host relationships |
| Recovery | Edit, allow journal write, terminate/restart | Committed recovery state offered; saved project preserved |

Do not mark the journey complete if it calls geometry creation methods instead of driving the corresponding controls.
File paths can be injected at the platform-dialog boundary in automated tests, but that substitution must be disclosed.
Run the event journey in debug and release; record candidate SHA, OS, toolchain, results and exclusions.
Perform native visual/file-dialog review on Windows and macOS, plus a Linux startup smoke check.

## Regression quality

- Derive pointer locations from actual widget rectangles and the current viewport transform.
- Recompute after geometry changes; assert resulting geometry/counts, not only the selected tool.
- Break one pointer handler deliberately in a disposable candidate and prove the relevant test fails before restoring it.
- Include Esc step-back and undo/redo. Do not use global sleep-based coordinate scripts.
- Use isolated temporary filenames, preserve user files, and test non-ASCII project/element text.
- Compilation is necessary, but neither compilation nor this checklist is evidence that the journey passed.

## Candidate boundary

This P0 slice improves the existing small-office event regression and creates the native build matrix.
It does not implement missing dimension, annotation, family, collaboration or printing features to make the test appear complete.
The complete two-storey drawing-package journey remains an explicit next P0 deliverable.
