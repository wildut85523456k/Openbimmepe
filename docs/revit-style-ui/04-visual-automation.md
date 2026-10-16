# 04 — Dynamo-like visual automation study

Status: **native foundation implemented; full workflow remains a target**. See
[implementation status](../automation.md) and [interaction contract](../automation-wire-interactions.md).

Decision: use an independent Rust graph/runtime and egui desktop editor, with
external Python processes. Dynamo, DesignScript, Revit APIs and .NET are not
runtime dependencies. The candidate approaches below record the original study;
embedded Python and compatibility importers are not commitments.

Current scope: typed JSON graphs, snapshot previews, atomic apply/undo, inline
block editors, connector drag/release, manually adjusted curves, a restricted
numeric Python worker and a separate read-only snapshot MCP server. Docking,
search, graph debugger, general scripting, live MCP and full BIM node families
remain incomplete. The proposed Python API below is still illustrative.

## Outcome
Users can build reusable node graphs or Python scripts that query and manipulate open BIM models while remaining transactionally safe in standalone or central-project mode.

## Candidate approaches
1. **Dynamo open-source project (DynamoDS/Dynamo)**: inspect engine / graph ideas and licensing; primarily .NET integration, official desktop UI is Windows-oriented, and direct binding to our Rust document requires significant work.
2. **Rust-native egui graph editor** (e.g. egui node-editor ecosystem): build UI nodes on a native graph executor, with typed BIM ports; design flexibility and Rust ownership, at the cost of implementing graph runtime and libraries.
3. **Independent graph model + swappable UI**: preferred architecture—serialize nodes/edges and typed values independently of GUI so future Qt/Slint/egui frontends can use it.

Do not assume Dynamo .dyn compatibility or Revit API scripts are portable.

## Proposed API
```python
from openbim import current_document

doc = current_document()
with doc.transaction("Resize ducts"):
    for duct in doc.elements.by_category("Duct"):
        if duct.get("Velocity") > 6.0:
            duct.set("Width", duct.get("Width") * 1.2)
```
Illustrative syntax **only**. Establish stable Rust typed commands and Python bindings first (consider PyO3 for embedded/bound Python, subject to sandbox review).

## Node families
- Document / query: current document, by type/category/system/level, element properties, IFC metadata.
- Geometry: vectors, points, curves, intersections, bounding boxes, transforms.
- BIM operations: create/place, set type, set parameters, move/rotate, host, connect routes.
- MEP: systems, port graph, route, size, pressure drop and warnings (calculation kernels are separate work).
- Data: lists, map/filter, conditional, units/conversions, file import/export with permissions.
- Execution: Python node, graph inputs/outputs, debug, preview, commit and rollback.

## Execution contract
1. Graph reads a specific document snapshot/revision.
2. Validate graph topology and dimensions; detect cycles unless explicit supported feedback node.
3. Execute pure computation and generate a **proposed diff**.
4. Show 3D/2D preview with affected IDs and rule violations.
5. Explicit user confirmation triggers transaction with expected revisions.
6. Server validates and accepts/rejects; UI displays outcome and supports undo.

## Milestones
- A0: Rust command/query API plus transaction-safe Python console.
- A1: Headless typed graph runtime; JSON or another versioned, validated graph representation.
- A2: Dockable node canvas and graph debugger.
- A3: Python nodes with controlled runtime, caching and permissions.
- A4: Library of architecture/structure/MEP nodes, examples and unit tests.
- A5: Investigate subset graph importers only after APIs stabilize.

## Test graph
Get Supply Air network → select ducts → inspect computed flow/velocity → propose duct sizes → preview changed 3D segments → commit transaction → undo. No arbitrary mutation of central project files.
