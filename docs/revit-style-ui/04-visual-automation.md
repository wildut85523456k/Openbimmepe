# 04 — Automation: Dynamo-like graph app and MCP server

Status: **design agreed 16 Oct 2026; not implemented.** It replaces the 15 Oct study. The full record is the Claude Doc "Automation: Dynamo-like Graph App and MCP Server".

## Decisions

- **One API, three clients.** One Rust command and query API inside the app is shared by three clients:
  - the ribbon and shortcuts;
  - **OpenBIM Graph**, the Dynamo-like app;
  - an **MCP server** for AI automation runs.
- **egui only, Rust only.** Python exists only inside OpenBIM Graph, which is the external automation software, as Dynamo is to Revit. The app embeds no Python.
- **OpenBIM Graph is as close to Dynamo as possible.**
  - It is a separate egui program, launched from the app's Manage tab and linked live to the open model.
  - Its screen matches Dynamo's: node library on the left, canvas in the middle, and a Run bar with Automatic and Manual modes.
  - It has preview bubbles under nodes, Watch nodes, a Code Block node and a Python Script node.
  - Geometry previews in the main app's 3D view.
  - A Player equivalent runs saved graphs from the app without opening the canvas.
- **The MCP server is built into the app.**
  - Its tools are the same commands and queries as the ribbon. Elements, schedules and parameters can be read.
  - It connects two ways:
    - **stdio**, when the AI client starts the app headless on a project file;
    - **local HTTP with a token**, when the AI client attaches to a running app.
  - It is read-only by default. AI edits arrive as a proposal that the user previews and accepts. Every accepted change is logged under the AI client's name.
- **No compatibility promises.** Dynamo `.dyn` files and Revit API scripts are not compatible and are not promised.

## Architecture

```text
Ribbon and shortcuts --direct calls--+
OpenBIM Graph (egui, Python) --local link, scoped token--+--> openbim-commands (registry + typed API)
AI agents --> MCP server in the app ----------------------+        |          |            |
                                                          writes own model  reads links  calls calculation modules
```

## Execution contract

1. A run reads a fixed revision of the model and records what it read: each element and its revision.
2. The graph is checked before it runs: topology, dimensions and cycles.
3. Computation runs in parallel on all cores, produces results in a fixed order, and returns a **proposed change set**.
4. The proposal is shown in 2D/3D with the affected elements and any rule violations.
5. The user confirms, and the change set is committed through the per-element rules below. Undo works as for any edit.

## Rules the API enforces

| # | Topic | Rule |
|---|---|---|
| 5 | Bulk edits | Applied per element, matching the collaboration decisions of 15 Oct. <br>• Changes that don't clash apply. <br>• An unlocked element: the last writer wins, and the losing edit is kept and flagged. <br>• A locked element: the change becomes a change request, recorded with the clash results. <br>The run reports applied, flagged and requested elements. Edits that must stay whole (a route with its fittings) are marked all-or-nothing. |
| 6 | Stale edits | Each element carries the revision of its last change. The commit checks only the elements the run read; there is no whole-document revision check. |
| 7 | Units | Every value is a quantity: an SI number plus its dimension. <br>• Python gets quantity objects. <br>• MCP returns `{"value": 6.0, "unit": "m/s"}` and accepts "6 m/s". <br>A dimension mismatch is an error. |
| 8 | Parameters | Stable IDs (`duct.width`), mapped to IFC property sets, with English and French labels. A label is accepted with a warning, like Dynamo's get parameter by name. |
| 9 | Commands | Crate `openbim-commands`. Each command has: <br>• an ID, a label and an icon; <br>• typed inputs and the conditions under which it can run; <br>• a preview step and an execute step that produces log operations. <br>The ribbon, keymap, graph nodes and MCP tools are all generated from it. |
| 10 | Graph engine | Crate `openbim-graph`, shared by OpenBIM Graph and headless runs. <br>• Independent nodes run in parallel. <br>• Results come out in a fixed order and are cached per model revision. <br>• Dynamo lacing (shortest, longest, cross product) and replication guides are supported. <br>• Python nodes run in a pool of worker processes. |
| 11 | Trust | A graph file declares its permissions: read only, write own model, files, network. <br>• Python from a shared folder shows "this graph contains code"; trusted folders and signed graphs skip the warning. <br>• The app enforces permissions with a token scoped to them. <br>• MCP clients get the same treatment. |
| 12 | First use cases | 1. Firestop records from a site-inspection CSV. <br>2. Room schedule values. <br>3. Luminaire data for DIALux. <br>4. Duct sizing once M6 exists. |
| 13 | Calculations | Graph nodes and MCP tools call the app's calculation modules (M4 to M9); nothing is reimplemented. Each result records the standard and edition. |

**Graph file format.**
- **Structure.** The same as Dynamo's `.dyn` (JSON): nodes and connections are kept separate from the canvas layout.
- **Schema.** Own versioned schema. Node types are named by library and library version, so old graphs can be upgraded.
- **Storage.** In the project, as a file kept in the operation log with a checksum, or next to the project file.

## Milestones

1. **A0:** command registry and typed Rust API, used by the ribbon.
2. **A1:** MCP server, read-only first, then proposals the user accepts.
3. **A2:** headless graph engine (lacing, multicore, caching) and the graph file format.
4. **A3:** OpenBIM Graph app: canvas, library, Watch nodes, 3D preview in the main app.
5. **A4:** Python Script node with permissions.
6. **A5:** discipline node libraries as the calculations arrive, and a Player in the app.

## Acceptance test (first)

1. Read a site-inspection CSV of firestop records, through OpenBIM Graph and through an MCP client.
2. Match each row to its wall penetration by stable ID.
3. Preview the changed records.
4. Commit.
5. See the records in the IFC export.
6. Undo.

A locked wall turns its rows into change requests instead of edits.
