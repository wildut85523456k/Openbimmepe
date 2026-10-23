# Doors and windows come back from IFC (23 Oct 2026)

Found by the office end-to-end fixture (PR #82): a file from another program lost its doors and windows on import.

- The reader (`crates/ifc/src/reader.rs`) reads `IFCDOOR` and `IFCWINDOW` as members with their place, x axis and overall
  height and width (attributes 8 and 9, scaled to metres).
- The import (`plan_fillers` in `apps/desktop/src/import.rs`) puts each in the wall whose axis passes by its place at the
  right height, with the library door or window family at the file's size (window sill from the height of its place).
- A door or window in no wall, or of a size the family does not allow, is left out and said. One already in the model
  (same wall, within 0.1 m along) is not made twice, so the app's own files and a second import add nothing.

Not done: the stair, equipment and the grid of a foreign file are still left out; the family is always the library's
single flush door and fixed window, whatever the file's door type.

Tests: `a_door_and_a_window_of_a_file_from_another_program_stand_in_the_wall_they_were_in` (mutation-checked: shifting the
place by 0.5 m fails it). The expectation list of `a_file_the_app_wrote_comes_back_with_nothing_lost_in_every_schema` no
longer lists IFCDOOR/IFCWINDOW as left out.
