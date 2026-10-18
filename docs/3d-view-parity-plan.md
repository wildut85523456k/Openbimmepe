# Revit alike: the 3D view against Revit (separate topic)

Opened 2026-10-19 at the owner's request ("a point to clear at a later stage in the 3D view"): editing of elements in 3D, and an orbit control at the top right. Status is that of commit cae7598. This is a plan, not built work: nothing here has been started.

## Sources and how far they can be trusted

- **Project rule:** files shared on Gmail are the standard references. A search of the Gmail account for Revit 3D-view material found nothing, so **no client standard has been used**. The lines about Revit below are marked **(K)** (general knowledge of Revit, not checked in a running Revit) as in `claude/structural-elements-plan.md`. Revit was not run. Before building each item, check it against Revit by hand or against a reference the owner supplies.
- **What the app has now** was read in the code (`apps/desktop/src/view3d.rs`, `crates/viewport`, `crates/viewport-egui`) and is marked **(V)** (verified in code).
- Tests of the 3D view use simulated mouse events and, for the GPU path, an offscreen wgpu target. In the container where they were run an adapter was present (the tests were not skipped), most likely a software Vulkan driver; no real GPU has been used, so nothing below about speed is measured.

## 1. What the 3D view has today (V)

| Part | State |
|---|---|
| Drawing | wgpu widget inside egui: Wireframe, Hidden Line, Shaded, Realistic (sun and a 2048-texel shadow map; sun from date, time and the project site). A software renderer in `crates/viewport` is what the tests compare against |
| Mouse | Middle-drag pans, Shift + middle-drag orbits, wheel zooms toward the cursor, double middle-click fits (all as Revit). Perspective only, field of view fixed at 45 degrees. Right button does nothing in this view |
| ViewCube | Top right, about 70 px wide, 18 px from the corner. 26 places (6 faces, 12 edges, 8 corners); a click turns the camera there with a short animation; faces named TOP / FRONT... (English or French). No other part: see section 3 |
| Section Box | On or off from a tick box; six sliders (min and max on X, Y, Z) and a button per level that boxes that storey. Not draggable in the view |
| Selection | A click selects the front face under the cursor (ray against every triangle, in parallel); Ctrl or Shift adds or removes. It is the same selection as in the plan: Properties edits the selected element, Delete removes it |
| Datums | Levels and grids drawn over the picture |
| Not available in 3D | Every drawing and modify tool (Wall, Move, Rotate, Mirror, Copy, Align, Trim, Offset, Dimension, Tag, Room, Stair, Shaft...) is plan-only; picking in 3D selects but cannot move |

## 2. Point 1: editing of elements in the 3D view

Revit lets you place and modify in a 3D view: most Modify tools work on what you pick, walls and beams can be drawn on a work plane, selected elements show drag controls, and temporary dimensions appear. What a 3D view cannot do in Revit is also worth keeping (K): it has no level to draw on, so a work plane is set first.

| Revit behaviour (K) | In this app | Proposed step |
|---|---|---|
| Select: click, Ctrl/Shift add, drag a window (left to right inside, right to left touching), Tab cycles through what lies under the cursor (chain of walls, element behind), Select All Instances, Filter | click and Ctrl/Shift only | Window and crossing rectangle; Tab to cycle through the hits along the ray (the pick code already gets all hits, it keeps the first) |
| Move / Rotate / Mirror / Copy / Align / Trim / Offset on a picked element | plan only | Run the same commands in 3D with picks on the work plane; the commands themselves are in the document layer and need no change |
| Work plane: set by level, by face, by picking a plane; shown as a grid; used for drawing in 3D | none (no work plane at all, even in the plan) | A work plane object (level, face, named plane) in the viewport; every pick in 3D intersects it. This also gives Reference Planes (listed as a gap in the structural plan) |
| Drawing in 3D: Wall, Beam, Column, Duct, Pipe, Floor picked on the work plane; snapping to the cube, edge, face | none | After the work plane. Snapping in 3D needs a spatial index (see section 4) |
| Drag controls on a selected element (wall ends, beam ends, flip arrows), temporary dimensions | in plan only for walls | Controls drawn as screen-space handles on the selected element, dragging constrained to the work plane or to the element's own axis |
| Hide Element / Hide Category / Isolate Element or Category (temporary, then Reset), Reveal Hidden Elements | none | Hidden set kept in the view, not the document; the batch skips those elements |
| Per-view Visibility/Graphics: category on/off, transparency, line weights, halftone; View Properties: detail level, discipline, shadows, depth cue, crop region | styles only, a fixed colour per category | Start with category visibility and transparency; the rest after view templates exist (also missing in plans) |
| Undo / Redo | The document's undo works from 3D; there is no redo (known) | Redo is a separate missing item for the whole app |

Remark: because the document layer already applies edits as atomic commands and checks them, 3D editing is mostly **picking and gizmo work in the viewport**, not new model rules. The risk is not the model; it is picking precision and snapping in a perspective view.

## 3. Point 2: the orbit control at the top right

In Revit the top right of a view has the **ViewCube** and, under it, the **Navigation Bar** (K).

| Revit part (K) | In this app |
|---|---|
| ViewCube: click a face, edge or corner to look from there | done (26 places, animated) |
| ViewCube: **drag the cube to orbit** the model | missing: the cube only reacts to a click; a drag on it starts an ordinary drag of the view, which does nothing without the middle button |
| Arrows at the cube's edges: turn the view by 90 degrees; curved arrows: roll when looking straight at a face | missing |
| **Compass ring** below the cube (N, E, S, W): click a letter to look from that side, drag the ring to spin about the vertical axis; True North vs Project North | missing (the site's north angle exists in the document and could drive it) |
| **Home** button (house icon) and the cube's right-click menu: Go Home, Save View, Lock to Selection, Orient to a View, Perspective / Orthographic, Set Current View as Home, Show Compass | missing |
| **Navigation Bar**: Steering Wheels (full navigation, view object, tour building), Zoom (Zoom in Region, Zoom Out 2x, Zoom to Fit, Zoom to Sheet Size, Previous and Next Pan/Zoom), Orbit, Pan | missing as a bar. Zoom to fit exists (button and double middle-click) |
| Orbit about the selected element (Shift + middle orbits about the centre of the view in Revit; with an element selected, about its centre in recent versions) | orbits about the camera target only |
| Perspective camera with eye and target (View > 3D View > Camera), Walk, Fly | missing; the camera is a fixed-field orbit camera |
| **Orthographic 3D view** (the Revit default for a new {3D} is orthographic) | missing: perspective only. This matters for measuring and for sections |

Proposed first step (small, no GPU work): in `crates/viewport` and `viewport-egui`, make the cube draggable, add the compass ring, the Home button and the four arrows, and add a small Navigation Bar under the cube (Zoom to Fit, Zoom in Region, Previous/Next, Orbit). Add an orthographic projection to the camera (view-projection matrix and the picking ray). All of that is testable with the existing simulated-event tests and the software renderer.

## 4. GPU and hardware (why this is a separate topic)

What the code does today (V):

- **The whole app needs wgpu.** eframe is built with its wgpu renderer; a machine with no usable Vulkan, Metal or DirectX 12 driver cannot open the window at all. The software renderer in `crates/viewport` is used by tests only; it is **not** a fallback for the window. Not tested on a machine without a GPU.
- **Whole-scene upload.** Any change of the document rebuilds one `Batch` for the whole model and uploads it again (`view3d.rs`: batch rebuilt when the revision changes). Fine for the sample; for a building with tens of thousands of elements, an edit would stall the view.
- **Picking walks every triangle** of the batch on the CPU in parallel (rayon). No spatial index. Adequate for small models; snapping in 3D (every move needs a hit test) would not be.
- **No instancing, level of detail, frustum or occlusion culling.** Families (doors, windows, equipment) are duplicated as plain triangles.
- **No anti-aliasing** (sample count 1; lines are drawn by the line pipeline), no transparency blending (blend is off in the pipelines), so a ghosted element, glass or a faint ceiling in 3D cannot be shown yet. Realistic uses one shadow map of 2048 texels over the whole scene, which will look coarse on a large site.
- Line styles are drawn as 1 pixel lines by the line pipeline; thick or dashed lines (needed for section lines and hidden-line quality in 3D) need a quad-based line shader.

What Revit does that needs more than this (K): ray-traced Realistic (cloud or local), ambient occlusion, high-quality anti-aliasing, graphics-card-driven large-model navigation. These are **not** proposed now. What is proposed is the part that decides whether the app is usable on an engineer's laptop.

## 5. Proposed order (for the owner to confirm)

0. **View-only mesh import of IFC** (faceted brep, triangulated face set, mapped items), so that the owner's project can be loaded and measured (section 6). Without it step 4 has nothing real to measure.
1. **Navigation, no GPU change** (section 3): draggable cube, compass, Home, arrows, Navigation Bar with zoom tools, orthographic camera. Then Steering Wheels (rewind history, orbit, pan, zoom, look, walk) after the eye-height camera exists. About the size of one earlier audit item for the first part.
2. **Selection in 3D**: window / crossing, Tab cycling, Select All Instances; Hide / Isolate (temporary) and Reveal.
3. **Work plane** (levels, faces, reference planes) and **Move / Rotate / Copy / Mirror / Align** from picks on it. Then **drawing in 3D** (Wall, Beam, Column, Duct, Pipe).
4. **Performance, needs real hardware to measure**: incremental batch update per element, BVH for picking, instancing for families, frustum culling, MSAA, blend for transparency, thicker lines. Do this once a model of realistic size and a reference laptop are chosen.
5. **View properties** (Visibility/Graphics, detail level, crop region, view templates), shared with plans, elevations and sections.

## 6. Decisions (owner, 2026-10-19)

| Question | Answer | What it means |
|---|---|---|
| Minimum hardware, and must a machine without a GPU be supported? | "Same as Revit." | See the table below. Revit lists a DirectX 11 graphics card even for its entry level (it lets a user switch hardware acceleration off, but Autodesk does not list that as a supported setup), so **a GPU is required; no software fallback in the window**. The software renderer stays as a test tool |
| Realistic project to measure with | The owner will supply one on 2026-10-19 and will look for past `.rvt` files | See "The test project" below |
| Navigation Bar | "Follow Revit for user experience": copy Revit's layout **including Steering Wheels** | Section 3 is in scope in full, Steering Wheels included (see below) |

### Hardware target: Revit's published requirements

Source: Autodesk's Revit product-details page, **Revit 2027** (read 2026-10-19; the page's table was summarised by a tool and its columns are ambiguous, so confirm the figures on the page before they go into a specification): 64-bit Windows 11; processor of 2 GHz or more; memory 16 GB for entry level, 32 GB or more for the middle level, 64 GB or more for large complex models; a DirectX 11 graphics card with Shader Model 5 (video memory 4 GB entry, 6 GB, 8 GB for large models; 8 GB or more and OpenGL 4.5 for "accelerated graphics"); 30 GB of disk for the installation and 100 GB free for temporary files recommended.

What this means for the app (proposal, K until tried on real machines):
- **Windows 11 with a DirectX 11-class card is Revit's floor.** The app draws through wgpu, which on Windows uses DirectX 12 or Vulkan, and on Linux Vulkan, on macOS Metal. A DirectX 11-only card from before about 2012 may not have a DirectX 12 or Vulkan driver; every card sold since then that Revit accepts is expected to have one, but this is **not yet checked**. The first job of step 4 is to run the app on one older reference laptop and on one workstation card.
- Revit runs on Windows only. The app is not tied to Windows; the owner has so far not asked for Linux or macOS support, so the three systems are kept as targets of wgpu, not as promises.
- Memory and video memory: the figures above become the **sizing targets** for the large-model test (a model must open within 16 GB, and the 3D view within 4 GB of video memory, for the entry class).

### Navigation Bar and Steering Wheels

Revit's Navigation Bar sits at the right of the view, under the ViewCube. It has: the Steering Wheels button (with the wheels: Full Navigation, Tour Building, View Object, each in a full and a mini size, Basic wheels; the wheel carries Zoom, Rewind, Pan, Orbit, Center, Look, Walk, Up/Down, Forward, and Rewind shows a film strip of earlier views), the Zoom button with its menu (Zoom in Region, Zoom Out (2x), Zoom To Fit, Zoom Sheet Size, Previous Pan/Zoom, Next Pan/Zoom), and the Orbit tool. (K: layout from Revit's help; the wheels' exact look, size and pointer shapes to be checked against a running Revit by the owner, since Revit was not run for this plan.) Steering Wheels are a separate piece of work: a pop-up wheel that follows the pointer, a rewind history of cameras (the app already blends between two cameras, so the history is a list of cameras), and Walk / Look / Up-Down tools that need an eye-height camera (section 3, "Perspective camera with eye and target, Walk, Fly"). They are placed after the cube, compass and zoom tools in the order of section 5.

### The test project

- **What the app can read.** A Revit `.rvt` cannot be opened by this app or by me (closed format; the route to it is the later add-in). The owner would export it from Revit as **IFC** (IFC4 Reference View, or IFC2x3 Coordination View). Checked in the code today: the IFC reader takes only bodies made of **extruded profiles** (`IfcExtrudedAreaSolid`, and boolean results through their first operand). It has **no reader for `IfcFacetedBrep`, `IfcTriangulatedFaceSet`, `IfcPolygonalFaceSet` or `IfcMappedItem`**, which is what Revit writes for most elements. So a Revit IFC would today mostly be listed as skipped. **A new first step is needed for measuring: a view-only import of any IFC geometry as meshes** (Revit's "link model" in effect), which the 3D view and picking can then be tested against. It is added as step 0 in section 5.
- **Size to provide.** This working environment has 2 processor cores, 7 GB of memory and about 3.6 GB of free disk. Ask: one IFC of **about 20 MB** for a first check, and one of **up to about 200 MB** (a building with architecture, structure and MEP) for the real test; above that the text parsing of the file may not fit in memory here. Send as a Google Drive link rather than an attachment: the Drive connector failed on very large files in earlier work (see `claude/structural-elements-plan.md`), and Gmail attachments stop at 25 MB. Also say, for each file, the Revit version, the number of elements if known, and whether it is architecture, structure or MEP.
- **What can and cannot be measured here.** This environment has no real GPU and only 2 cores, so the figures for drawing speed, video memory and the multicore gain must be taken on the owner's machine; here I can measure the CPU side (import time, batch build, picking) and make the program print the figures for the owner to run himself.
