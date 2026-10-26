# P0 office acceptance contract

Scope approved 25 October 2026: start P0 from the architecture-book review.
Reference: Architecture 2026 chapters 2-8 and 11-13. This is an original test fixture, not copied book content.

## Evidence levels

1. Model/API fixture: `office.rs` builds two storeys and checks persistence/exchange.
2. Window event regression: `model_view.rs::tests::the_small_office_is_built_from_a_new_project_then_saved_reopened_and_exported`.
   The P0 candidate clicks New and tool buttons and places grid/walls/floor/room/section with pointer events through
   `Desktop::window`. Tabs and wall location are still fixture setup. Save/Open/IFC still use direct methods.
3. Complete office interaction: PASS headless (26 Oct, candidate branch `p0/office-journey-ui`).
   `model_view.rs::tests::the_two_storey_office_is_drawn_saved_reopened_and_exported_through_the_window` operates only
   controls the window drew, located from that frame by name (`test_hit!`), and covers every step of the journey
   below. The disclosed substitutions are: the platform file dialog behind Save, Open and Print (`dialog::answer`),
   and the autosave timer, called directly once (`autosave_tick`) because the window's 30 s timer does not run in a
   test. A scroll-clipped control is reached with the mouse wheel; a control out of sight fails the test.
   Known limitations the test pins rather than hides: a dimension stores two points and does not follow the wall
   (D-DIM), and a U stair exports as one straight IfcStair per run (X-IFC).
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

## Journey evidence (26 Oct)

| Step | Controls operated | Checked independently |
|---|---|---|
| New | quick-access New | two levels; an untouched new model is clean |
| Datums | Level options bar: Rename x2, elevation x2, Place Level; Grid; View > Zoom Out | names; 0 / 3.6 / 7.2 m; IFC storey elevations |
| Envelope | Location Line list; Wall, Door, Window, Floor, Stair (U shape list), Ceiling, Roof; browser double clicks | walls per storey, hosts, slab kinds, roof level, 21 risers |
| Rooms | Room tool; Select click on the label; Properties Number/Name/Apply | 2 x 11.6 m²; 101 Office, 102 Meeting |
| Change | Properties Type list + Apply (Type B); Move tool on the host wall; quick-access Undo/Redo | joined walls stretch, door stays hosted, rooms grow by 1.45 m²; exact records after undo and redo |
| Annotation | Annotate > Tag on the door; Annotate > Dimension | tag host and type; 6.2 m; coordinates only (limitation) |
| Schedule | View > Schedule, category, OK, Properties OK | room numbers and areas equal the model; one Type B door, 1200 mm |
| Sheet | View > Sheet: number, name, OK; Place View x2 | A-101 with the plan and the room schedule |
| Output | Print (PDF); Insert > Export IFC with a typed path | pdfinfo: 1 page at the sheet's paper; IFC storeys, 9 walls, 2 spaces, door/window fillings, stair, roof, ceiling |
| Persistence | quick-access Save, New, Open | every native record equal after reopen |
| Recovery | Wall after the save; journal written; new session Open; Recover | recovered records equal the unsaved ones; the file on disk is unchanged |

Mutation checks (each made the journey fail, then was reverted): room-label pick, level Rename, Recover button,
Print button.

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
