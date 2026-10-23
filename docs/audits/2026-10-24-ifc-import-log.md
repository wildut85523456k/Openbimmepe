# IFC import log (24 Oct 2026)

Revit writes `<file>.ifc.log` beside every IFC it opens. This app now writes `<file>.ifc.log` too (the path with `.log`
added) each time a file is imported, and the message line says where it is.

The log holds: the file, the time, the schema, the summary line, what was added (by kind, levels, elements restored from
the file's own records), what was left out with counts and reasons, the notes (renumbered rooms, edited elsewhere ...), and
the reader's line for each entity it could not use (`#12 IFCSTAIR: unsupported product ...`). A failed import is logged too.
If the folder cannot be written, the message says so and the import stands.

Tests: `the_import_window_reads_a_file_the_app_wrote_back_into_an_empty_model` now reads the logs of the app's own file, of
a file without the app's data and of a bad file (mutation-checked: a wrong log path fails it). Desktop suite 217 passed.

This is step 1 of Revit's approach (the next steps are a generic carrier for unsupported classes, a class mapping table
and curved grids). The carrier touches the category, validity rules, native records, 3D, plan and IFC export, so it is
proposed as its own change.
