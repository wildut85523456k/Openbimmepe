# Visual review in a real window, 26 October 2026 (P0 gate 4, Linux part)

Candidate: branch `p0/office-journey-ui`. The release build of `openbim-desktop` ran in a real X11 window
(Xvfb 1600 x 1000, Mesa software OpenGL through wgpu's GL backend) on Linux, rustc 1.97.0. It opened the model
that the office journey saved (`OPENBIM_JOURNEY_OUT`), given on the command line (new: a `.obim` path as the
first argument is opened at start). Views were opened with real pointer events (`xdotool`), and the screens were
captured and reviewed by eye. The printed PDF was rasterised with `pdftoppm` and reviewed.

**Scope.** Linux only. Windows and macOS native windows, file dialogs, HiDPI scaling and GPU back ends are
still to be reviewed (see the end). This is a review of what is drawn, not a performance test.

## Screens

| Screen | File | What it shows |
|---|---|---|
| Ground floor plan at start | `visual-2026-10-26/01-start.png` | level heads, grids, walls, both rooms with names and areas, the associative 4800 dimension, the Type B door tag, the U stair with UP arrows |
| 3D | `visual-2026-10-26/02-3d.png` | two storeys, the door and window, the U stair with railings, the roof, level lines |
| First floor plan | `visual-2026-10-26/03-first.png` | the first-floor walls; the stair below shown faint |
| Sheet A-101 in the app | `visual-2026-10-26/06-sheet.png` | A1 title block, the plan at 1:100 and the room schedule |
| After the fixes | `visual-2026-10-26/07-browser-fixed.png` | Roof in Floor Plans; the status bar names the opened plan |
| Printed A-101 (PDF) | `visual-2026-10-26/a101-pdf.png` | title block with name and number, the plan with room tags, the schedule with numbers, areas and grand total |

## Findings

| # | Finding | Severity | Status |
|---|---|---|---|
| V1 | The status bar said "Opened sample view: Level 2" after opening the First Floor plan (an old label, and the template name rather than the level's) | Low | Fixed: "Opened First Floor" |
| V2 | The plan of a third level ("Roof (plan)") was listed after every folder at the bottom of the Project Browser, not in Floor Plans | Medium | Fixed: added levels are listed in Floor Plans, open by double click and are found by search |
| V3 | Plans stroke the triangles of faces: a diagonal line crosses each floor and the corners of walls show mitre diagonals | Medium | Open: draw only the outer edges of a face in plan |
| V4 | A door or window has no plan symbol (leaf and swing) where the wall is cut; only its tag shows it | High (drawings) | Open: a plan symbol for hosted families at the cut plane |
| V5 | Walls keep a fixed 3 m height with 3.6 m storeys: a band shows between storeys in 3D, and the roof floats 600 mm above the first-floor walls | High (model) | Open: register A-WALL, top constraint "Up to level" |
| V6 | The floor did not follow the two moved walls: strips of the rooms are not covered by the floor | Medium | Open: floors sketched on walls should keep to them (Revit's Pick Walls) |
| V7 | A view placed on a sheet goes to the top-left corner, not the middle the code comment says; the plan is small on A1 at 1:100 | Low | Open: place at the middle, or where the user clicks |
| V8 | Two grids that start at the same point draw their bubbles on top of each other | Low | Fixture choice; Revit offsets bubbles too, so not changed |

Nothing in the review contradicted the headless journey: the counts, names, areas, the dimension value and the
schedule values on screen and on paper match what the test asserts.

## Still to review on Windows and macOS

The same model and steps, on each OS: start-up, the native Save/Open/Print dialogs (titles, file types, offered
names, as `dialog::Question` records them), HiDPI scaling, the GPU back end (DirectX 12 / Metal) and the 3D view,
and the printed PDF. The release executables for both are built by `desktop-p0.yml` (macOS on manual dispatch).
