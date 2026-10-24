# Grid lines come back from IFC (24 Oct 2026)

- The reader (`crates/ifc/src/reader.rs`) reads every `IfcGrid`: the polyline of each axis (U, V and W), placed by the
  grid's own placement, in metres, as `Imported::grids`. A grid is no longer counted as left out.
- The import (`apps/desktop/src/import.rs`) makes a grid line of each axis with its tag as the name. A line the model has
  already (same name and ends) is not made twice, so the app's own files and a second import add nothing.

Not done: curved axes (only the first and last point of a polyline are used) and the grid's own name.
Still left out of a file without the app's own data: the stair and equipment.

Test: `the_grid_lines_of_a_file_from_another_program_come_back_by_name_and_are_not_doubled` (mutation-checked: swapping
the ends fails it). The round-trip expectation list no longer lists IFCGRID.
Run: ifc, view and desktop suites 397 passed, 0 failed; clippy clean on ifc and desktop.
