# Requirement: Purge Unused (proposed backlog item DOC-PURGE)

Requested by the user on 2026-10-17: "We shall be able to purge the files of non-used items at some stage."

## Current state
No purge or unused-item cleanup exists in the app (checked `origin/main`, `feature/rust-gui` and `m1c-sheets-printing` for purge/unused/unreferenced). The team's own purge exists only inside Dynamo graphs (`Purge Unused` in four graphs, including `Remove mark.dyn`). Those graphs are quarantined because the purge runs together with deletions.

## What it should do (Revit "Purge Unused" behaviour, made safe)
1. **Scan first, delete nothing.** List every item no element or view references: family types and families, pipe/duct segments and sizes, materials, line styles and patterns, fill patterns, text and dimension types, view templates, filters, title blocks, unplaced views/schedules, empty levels and worksets, unused shared-parameter bindings and imported layers.
2. **Show the reference chain.** For each item: why it counts as unused, and what would become unused after it is removed. Purge runs in passes, like Revit's repeat purge.
3. **Opt in per item.** Check boxes grouped by category, with "check all" per group. Protected items are never offered: project-standard defaults, items used by templates, title blocks set as default, linked-model content, and IFC-referenced types with stable GlobalIds.
4. **One undoable operation.** It is written to the operation log as a single entry, with a summary of counts and names. Undo restores everything.
5. **Same typed command everywhere.** The `purge_unused(scan|apply)` command is shared by the UI, the OpenBIM Graph and MCP. `apply` requires the scan result ID, so an automation cannot purge blind. It never runs at startup or inside another command.
6. **Team catalogue cleanup.** The same scan applies to imported team content (pipe segments from the catalogue workbook, layers, families). A large reference catalogue can be loaded and then trimmed to what the project uses.

## Acceptance
- A project with known unused items: the scan lists exactly those items, verified by an independent reference count.
- Apply and then undo gives a byte-identical document snapshot.
- Purging never changes model geometry, schedules, IFC export of used elements or GlobalIds.
- An automation calling `apply` without a matching scan ID is refused.
- Runs in multiple passes until nothing is left. Reference scanning is parallel, and its performance on a 100k-element model is measured by the perf agent.
