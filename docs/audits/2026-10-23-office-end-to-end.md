# Office end-to-end fixture (TEST-001), 23 Oct 2026

`apps/desktop/src/office.rs` (test only) builds the office the plan chose on 14 Oct: two storeys of 20 x 12 m, grids, walls
(14), a door, two windows, floors and a ceiling, a stair, two numbered rooms (G01 Store, G02 Meeting room), a fan coil
unit with a duct, and an electrical network (utility, main board, distribution board, the fan coil unit as a load).

One test takes it through: save, reopen (records equal line for line), IFC export (counts of spaces, stair, door,
windows, storeys), read back from the geometry alone (rooms keep number and name; same wall count), and the electrical
exchange (network whole, the fan coil unit carries its IFC GlobalId found in the file, the cable length is the drawn route).

A mutation check was run: removing the number from the room name on import makes the test fail.

## What the read-back showed
A file from another program lost, on import: doors, windows, the stair, equipment and the grid (reported as left out).
Doors and windows are read back since PR #83 (`2026-10-23-ifc-door-window-import.md`); the stair, equipment and grid are
still left out of a file without the app's own data.

## Not covered
The data hall for the MEP chain (MEP-004), pointer-driven steps through the ribbon (the harness in TEST-001 of the backlog),
and any look at the running window.
