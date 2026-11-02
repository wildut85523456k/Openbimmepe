//! First executable desktop prototype. The viewport is a clearly labelled SAMPLE scene:
//! no model persistence, IFC import, live routing, or central collaboration is implied.
#![forbid(unsafe_code)]

/// Records, in a test build, where a control was drawn this frame so a window test can click it by name:
/// `test_hit!(app, response, "tab:{}", name)` keeps the part a pointer can reach (clipped by its scroll area) and the
/// whole rectangle. It compiles to nothing in the program.
macro_rules! test_hit {
    ($app:expr, $response:expr, $($name:tt)+) => {
        #[cfg(test)]
        { let r = &$response; $app.widget_hits.push((format!($($name)+), r.interact_rect, r.rect)); }
        #[cfg(not(test))]
        { let _ = &$response; }
    };
}

mod icons;
mod ribbon;
mod panels;
mod dock;
mod dwg_export;
mod dwg_import;
mod step_import;
mod step_params_ui;
mod grid_tools;
mod model_view;
mod view3d;
mod section_ui;
mod view_tools;
mod view_notes;
mod model_edit;
mod family_types_ui;
mod snap_place;
mod type_editor;
mod wall_shapes;
mod modify_tools;
mod export;
#[cfg(test)]
mod office;
#[cfg(test)]
mod data_hall;
mod import;
mod class_map;
mod app_settings;
mod georef_ui;
mod journal;
mod file;
mod dialog;
mod clock;
mod mep_tool;
mod insulation_ui;
mod systems_ui;
mod schedule_ui;
mod view_ui;
mod pdf;
mod sheet_draw;
mod sheet_ui;
mod plan_export;
mod print_dialog;
mod mep_settings_ui;
mod compute_ui;
mod elec_ui;
mod automation;
mod automation_canvas;
mod automation_groups;
mod automation_slider;
mod automation_icons;

use openbim_document::{Document, ElementId};
use eframe::egui::{self, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace { Design, Coordination, Automation }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Plan, UpperPlan, ThreeD, Mechanical,
    /// one of the four building elevations: East, North, South, West
    Elevation(u8),
    /// the view of a section line (its element number)
    Section(u64),
    /// one view of an elevation marker: the marker's element number and the direction (0..4 as the four elevations)
    Marked(u64, u8),
    /// the plan of a level added with the Level tool
    Level(u64),
    /// a schedule/quantities view (its number)
    Schedule(u64),
    Sheet(u64) }

impl View {
    /// A stable identifier for a view, independent of its position in an open-views list (REA-006).
    /// Two equal views share an id; the id is stable across restarts so a view can be re-opened
    /// exactly after close/reorder. Ranges are disjoint per variant so ids never collide.
    pub(crate) fn id(&self) -> u64 {
        match self {
            View::Plan => 0, View::UpperPlan => 1, View::ThreeD => 2, View::Mechanical => 3,
            View::Elevation(k) => 4 + *k as u64,
            View::Level(n) => 100 + n,
            View::Section(n) => 1_000 + n,
            View::Marked(n, k) => 10_000 + n * 8 + *k as u64,
            View::Schedule(n) => 20_000 + n,
            View::Sheet(n) => 30_000 + n,
        }
    }
}

/// Cap on the number of views open as tabs at once (REA-006). A seventh open view
/// prompts the user to replace an existing tab or cancel; 10 is the Revit-like default.
pub(crate) const MAX_OPEN_VIEWS: usize = 10;
/// What to do when opening a view while the tab cap is reached.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViewOpenPolicy { Continue, Replace }
/// Returns `Replace` once `open_count` has reached `MAX_OPEN_VIEWS`, else `Continue`.
pub(crate) fn view_open_policy(open_count: usize) -> ViewOpenPolicy {
    if open_count >= crate::MAX_OPEN_VIEWS { ViewOpenPolicy::Replace } else { ViewOpenPolicy::Continue }
}

/// Per-view state each open tab remembers (REA-006): the plan camera (pan/zoom), the 3D orbit
/// intent (yaw/pitch) and the section/elevation model-to-screen transform. The 3D viewport
/// widget keeps its own keyed camera. Snapshotted on leave, restored on switch.
#[derive(Clone, Debug)]
pub(crate) struct ViewState { camera_yaw: f32, camera_pitch: f32, zoom: f32, pan: Vec2, section_xf: Option<crate::section_ui::Xf> }
impl Default for ViewState {
    fn default() -> Self { Self { camera_yaw: 0.7, camera_pitch: 0.6, zoom: 1.0, pan: Vec2::ZERO, section_xf: None } }
}

impl Desktop {
    /// Open `v` as a tab if it is not already open, making it active. Returns the cap policy
    /// (REA-006): `Replace` when the Revit-like tab limit is reached so the caller can prompt
    /// to replace an existing tab; the tab is NOT added until the user chooses one.
    pub(crate) fn add_view(&mut self, v: View) -> ViewOpenPolicy {
        if let Some(i) = self.open_views.iter().position(|o| *o == v) {
            self.activate_view_at(i);
            return ViewOpenPolicy::Continue;
        }
        let policy = view_open_policy(self.open_views.len());
        if policy == ViewOpenPolicy::Replace {
            self.view_replace_pending = Some(v);
            return policy;
        }
        self.open_views.push(v);
        self.view_states.push(None);
        self.activate_view_at(self.open_views.len() - 1);
        self.view_replace_pending = None;
        policy
    }
    /// Make the tab at index `i` active; `view` follows it so the viewport and the rest of
    /// the UI show that view (per-view state comes in Increment 2).
    pub(crate) fn activate_view_at(&mut self, i: usize) {
        let i = i.min(self.open_views.len().saturating_sub(1));
        if let Some(v) = self.open_views.get(i).copied() {
            // snapshot the outgoing view's live camera before it is left behind
            if self.active_view < self.view_states.len() {
                let s = self.save_view_state();
                self.view_states[self.active_view] = Some(s);
            }
            self.active_view = i;
            self.view = v;
            // restore an incoming view only if it has saved state; a tab with none inherits the
            // live camera, so pointing at a view keeps the picture continuous (Revit)
            if let Some(st) = self.view_states.get(i).and_then(|o| o.clone()) {
                self.restore_view_state(st);
            }
        }
    }
    /// Close the tab at index `i`, keeping `active_view` and `view` valid (REA-006).
    pub(crate) fn close_view_at(&mut self, i: usize) {
        if i >= self.open_views.len() || self.open_views.len() <= 1 { return; }
        // snapshot the active view's live camera before the parallel lists shift
        if self.active_view < self.view_states.len() {
            let s = self.save_view_state();
            self.view_states[self.active_view] = Some(s);
        }
        self.open_views.remove(i);
        self.view_states.remove(i);
        self.view_replace_pending = None;
        self.active_view = match self.active_view.cmp(&(self.open_views.len())) {
            std::cmp::Ordering::Greater if self.active_view > 0 => self.active_view - 1,
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Less => self.active_view.min(self.open_views.len().saturating_sub(1)),
            std::cmp::Ordering::Equal => self.active_view.min(self.open_views.len().saturating_sub(1)),
        };
        self.view = self.open_views[self.active_view];
        if let Some(st) = self.view_states.get(self.active_view).and_then(|o| o.clone()) {
            self.restore_view_state(st);
        }
    }
    /// Stable id of the active view (REA-006).
    pub(crate) fn active_view_id(&self) -> u64 {
        self.open_views.get(self.active_view).map_or(self.view.id(), |v| v.id())
    }
    fn save_view_state(&self) -> ViewState {
        ViewState { camera_yaw: self.camera_yaw, camera_pitch: self.camera_pitch, zoom: self.zoom, pan: self.pan, section_xf: self.section_xf }
    }
    fn restore_view_state(&mut self, s: ViewState) {
        self.camera_yaw = s.camera_yaw; self.camera_pitch = s.camera_pitch;
        self.zoom = s.zoom; self.pan = s.pan; self.section_xf = s.section_xf;
    }
    /// Open-or-switch to `v` (as in Revit, pointing at a view makes it a tab).
    pub(crate) fn set_view(&mut self, v: View) -> ViewOpenPolicy { self.add_view(v) }
    /// Reset the tab set to a single view (new document / reset).
    pub(crate) fn reset_view(&mut self, v: View) {
        self.open_views = vec![v];
        self.view_states = vec![Some(ViewState::default())];
        self.active_view = 0;
        self.view = v;
        self.restore_view_state(ViewState::default());
    }
    /// Confirm the pending replace: evict tab `i` then add the view the user tried to open.
    pub(crate) fn replace_pending_with(&mut self, i: usize) {
        if let Some(v) = self.view_replace_pending.take() {
            self.close_view_at(i);
            let _ = self.add_view(v);
        }
    }
    /// Dismiss the replace/cancel prompt; the attempted view is dropped (as in Revit).
    pub(crate) fn cancel_replace(&mut self) { self.view_replace_pending = None; }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool { Select, Paste, MatchType, Railing, Stair, VerticalOpening, Room, Section, Elevation, Shaft, WallOpening, WallJoins, Dimension, Measure, Move, Copy, Rotate, Mirror, Align, Trim, Offset, Duct, Pipe, EndPoint, ElecNode, Cable, Wall, Grid, Level, Tag, Beam, Column, Slab, Footing, WallFooting, BeamSystem, Truss, Rebar, Family }
/// What the options bar starts from when a command is chosen. A field left `None` is not touched (A5).
#[derive(Clone, Copy, Default)]
struct ToolDefaults {
    width_mm: Option<f64>,
    height_mm: Option<f64>,
    elevation_mm: Option<f64>,
    slab_thickness_mm: Option<f64>,
    slab_offset_mm: Option<f64>,
    footing_thickness_mm: Option<f64>,
    spacing_mm: Option<f64>,
    bar_count: Option<f64>,
    material_pick: Option<openbim_document::Material>,
}

/// A temporary dimension of the selected wall that is being retyped: its length, or its distance to a neighbour.
#[derive(Clone, Copy, PartialEq, Debug)]
enum TempDim { Length(ElementId), Distance(ElementId, ElementId) }
impl TempDim {
    /// A number that tells one temporary dimension from another.
    fn key(self) -> (u8, u64, u64) { match self { TempDim::Length(a) => (0, a.0, 0), TempDim::Distance(a, b) => (1, a.0, b.0) } }
}
/// The elevation marker being placed: its number and the views (facing, from, to) it will hold.
type ElevMarks = (u64, Vec<(u8, [f64; 2], [f64; 2])>);
/// What is being dragged in Select mode: an element to move, or one end of a wall.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag { Move { id: ElementId, from: [f64; 2] }, Grip { id: ElementId, at_start: bool }, Label { id: ElementId, at_start: bool, end: [f64; 2] } }
#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleElement { Duct, Model(ElementId) }

struct PdfOverlay {
    png_bytes: Vec<u8>,
    w: u32,
    h: u32,
    scale: f64,
    texture: Option<egui::TextureHandle>,
}

struct Desktop {
    automation: automation::Automation,
    document: Document,
    workspace: Workspace,
    view: View,
    /// The open view tabs (REA-006). `view` is always `open_views[active_view]`.
    open_views: Vec<View>,
    active_view: usize,
    /// per-open-view state, kept parallel to `open_views` (REA-006). `None` means the tab has no
    /// saved state yet: opening such a view inherits the live camera (a continuous picture, as in
    /// Revit) until the user leaves it and returns, at which point its state is snapshotted and
    /// restored on re-entry.
    view_states: Vec<Option<ViewState>>,
    /// a view the user tried to open at the tab cap, awaiting a replace/cancel (REA-006)
    view_replace_pending: Option<View>,
    /// a view the user chose to rename in the Project Browser, with the name typed so far (REA-009)
    rename_view_pending: Option<View>,
    rename_view_text: String,
    view_names: std::collections::HashMap<u64, String>,
    tool: Tool,
    selected: Option<SampleElement>,
    ribbon: &'static str,
    systems_sub: &'static str,
    width_mm: f64,
    height_mm: f64,
    elevation_mm: f64,
    slope_percent: f64,
    /// the section chosen for a beam: 0 rectangle, 1 circle, 2 I-shape; with the web and flange thickness of an I
    section_pick: u8,
    usage_pick: openbim_document::Usage,
    material_pick: openbim_document::Material,
    /// how far a brace rises over its run when it is placed by clicking
    brace_rise_mm: f64,
    web_mm: f64,
    flange_mm: f64,
    axis_mm: [f64; 4],
    /// the first point of a beam being placed in a plan view
    beam_start: Option<[f64; 2]>,
    /// the Wall tool's chain: the points picked so far, the walls drawn from them, the Location Line, whether it chains,
    /// and a length typed in mm
    wall_pts: Vec<[f64; 2]>,
    wall_ids: Vec<ElementId>,
    wall_loc: usize,
    wall_chain_on: bool,
    /// the Wall tool: how it draws (0 line, 1 rectangle, 2 pick lines), the wall type chosen (none: custom), Spacebar flipped, and the name typed for a new type
    wall_mode: usize,
    /// the Wall tool's polygon modes: the number of sides
    wall_sides: usize,
    wall_type: Option<ElementId>,
    wall_flip: bool,
    wall_type_name: String,
    wall_len: String,
    /// the Tag tool: a leader from the tag to the element
    tag_leader: bool,
    /// the Move, Copy, Rotate and Mirror tools: the points picked so far, the number typed, and whether a mirror leaves the original
    edit_pts: Vec<[f64; 2]>,
    edit_num: String,
    drag: Option<Drag>,
    /// the temporary dimension being retyped, the text typed, and where the labels of the shown ones are
    temp_edit: Option<TempDim>,
    temp_text: String,
    temp_labels: Vec<(eframe::egui::Rect, TempDim, f64)>,
    /// the last Measure: the two points
    measured: Option<([f64; 2], [f64; 2])>,
    /// Align, Trim/Extend and Offset: the element picked first (and where), and the offset distance and whether it copies
    edit_ref: Option<(ElementId, [f64; 2])>,
    /// What Copy or Cut put on the clipboard (Ctrl+C, Ctrl+X), pasted with Ctrl+V.
    clipboard: Option<openbim_document::Clipboard>,
    offset_mm: f64,
    offset_copy: bool,
    /// the element the points picked so far are about: a Copy goes on making copies of it
    edit_src: Vec<ElementId>,
    /// the elements selected besides the primary one (Ctrl or Cmd click adds, Shift click takes away, a box selects)
    multi: Vec<ElementId>,
    /// where a selection box was started in the plan
    marquee: Option<eframe::egui::Pos2>,
    mirror_copy: bool,
    /// the Level tool: the elevation (mm) and the name of the next level, empty for the next number
    level_elev_mm: f64,
    level_name: String,
    /// the level the options bar places beams on; the lowest when None
    beam_level: Option<ElementId>,
    /// the corners of the slab being drawn in a plan view
    slab_points: Vec<[f64; 2]>,
    slab_pick_walls: bool,
    slab_rings: Vec<Vec<[f64; 2]>>,
    /// the Duct and Pipe tools: the points clicked so far, the size of the leg that ends at each from the second on, and the system
    run_points: Vec<[f64; 2]>,
    /// the duct or pipe the run being drawn branches from, when its first click was on one
    branch_main: Option<ElementId>,
    run_opts: mep_tool::RunOpts,
    run_sizes: Vec<openbim_place::penetrations::RunSize>,
    system_pick: openbim_family::mep::System,
    /// beam system: the distance between its beams and their direction
    spacing_mm: f64,
    angle_deg: f64,
    /// rebar: the number of bars along the bottom of the beam
    bar_count: f64,
    /// family tool: the family and type to place, and the selected instance's type
    family_pick: String,
    family_type: String,
    /// the kind of family the picker offers while placing: a door command offers doors only, none offers every family
    family_kind: Option<openbim_family::Category>,
    /// the instance values of the selected family instance, as edited
    family_values: Vec<(String, f64)>,
    family_texts: Vec<crate::export::TextInput>,
    slab_thickness_mm: f64,
    /// how far above its level the top of a new slab is
    slab_offset_mm: f64,
    /// the slope of the roof to draw, in degrees from the horizontal
    roof_slope_deg: f64,
    /// Automatic Ceiling: a click in a closed space hangs a ceiling in it; off, the ceiling is sketched by its corners
    ceiling_auto: bool,
    /// Stair: width, the highest riser allowed, the depth of a tread, and the climb when no level is above
    stair_width_mm: f64,
    stair_riser_mm: f64,
    stair_tread_mm: f64,
    stair_rise_mm: f64,
    /// Stair: straight, or two runs and a landing that turn left or right (L) or come back (U)
    stair_shape: openbim_document::StairShape,
    /// Stair: which sides get a railing, as Revit puts one on a stair when it is made
    stair_railing: openbim_document::RailingSides,
    /// Shaft: how far above the base level it starts, the level it goes up to (None: the level above) and how far above that it ends
    /// Vertical Opening: the floor, roof or ceiling picked to cut, and the others under the same click for Tab
    opening_host: Option<ElementId>,
    opening_cands: Vec<ElementId>,
    shaft_base_mm: f64,
    shaft_top: Option<ElementId>,
    shaft_top_mm: f64,
    /// Wall Opening: the sill and the head above the base of the wall, and the first end picked (wall and distance along it)
    opening_sill_mm: f64,
    opening_head_mm: f64,
    opening_from: Option<(ElementId, f64)>,
    /// the first corner of a wall opening sketched in an elevation or section: the wall and the (u, z) of the corner
    view_corner: Option<(ElementId, [f64; 2])>,
    /// where a drag in an elevation or section began, in (u, z) of that view
    view_drag: Option<[f64; 2]>,
    /// the points and the reference edge of a Modify tool in an elevation or section view
    view_edit: Option<view_tools::ViewEdit>,
    /// how the elevation or section view last drawn maps (u, z) to the screen, for picking and for the tests
    section_xf: Option<section_ui::Xf>,
    /// where the four elevations look from, for the markers in plans: worked out again when the document changes
    elev_marks: Option<ElevMarks>,
    slab_kind_pick: openbim_document::SlabKind,
    /// footing: the thickness of a new pad, or of the selected one
    footing_thickness_mm: f64,
    /// the beam being edited: whether its start and end are joined to the columns they end in
    join_pick: [bool; 2],
    camera_yaw: f32,
    camera_pitch: f32,
    view3d: openbim_viewport_egui::Viewport3d,
    view3d_revision: Option<u64>,
    /// a site edit in progress in the sun controls, committed when the value is let go
    site_edit: Option<openbim_document::Site>,
    property_query: String,
    browser_query: String,
    ribbon_query: String,
    /// active discipline filter for the ribbon (REA-008): Architecture, Structure, MEP or All
    discipline: &'static str,
    browser_selected: String,
    message: String,
    show_properties: bool,
    show_browser: bool,
    /// the warnings window is open; the list is worked out again only when the document has a new revision
    show_warnings: bool,
    /// Imported PDF overlay: PNG bytes, pixel dims, scale (model units per pixel), and lazily-created texture.
    pdf_overlay: Option<PdfOverlay>,
    /// Imported CAD overlay (DWG/DXF): name, drawing, scale (model units per DWG unit).
    cad_overlay: Option<(String, crate::dwg_import::CadDrawing, f64)>,
    /// Imported STEP mesh (cadrum tessellation), mm units, awaiting placement as a family display mesh.
    imported_step: Option<openbim_document::Mesh>,
    /// The parameter table of the imported STEP, shown after import.
    step_params_dialog: Option<step_params_ui::StepParamsDialog>,
    /// Worksets: a Revit-style grouping for collaborate/edit permissions. Not yet wired to per-element
    /// ownership, but the bottom bar exposes a dropdown so the user can pick the active workset.
    worksets: Vec<String>,
    current_workset: String,
    /// per-grid bubble (label) offsets from each end, in model metres: (start offset, end offset). Revit lets
    /// the user drag a grid's label off its end; these are view-state offsets, not stored in the IFC model.
    grid_label_offsets: std::collections::HashMap<ElementId, ([f64; 2], [f64; 2])>,
    /// the file the model is saved in, and the questions about unsaved changes
    file: file::FileState,
    /// the types window, while it is open
    types_dialog: Option<model_edit::TypesDialog>,
    mep_settings_dialog: Option<mep_settings_ui::MepSettingsDialog>,
    insulation_dialog: Option<insulation_ui::InsulationDialog>,
    /// systems of equipment, the System Browser and Generate Layout
    systems: systems_ui::SystemsUi,
    schedules: schedule_ui::ScheduleUi,
    views: view_ui::ViewUi,
    sheets: sheet_ui::SheetUi,
    /// Edit Type of a wall type: its layers
    type_editor: Option<type_editor::TypeEditor>,
    /// the End Point tool and the Compute window
    end_draft: compute_ui::EndDraft,
    compute: compute_ui::ComputeDialog,
    elec_draft: elec_ui::ElecDraft,
    elec: elec_ui::ElecDialog,
    /// the cable being drawn: the node it leaves and the plan points so far
    cable_from: Option<u64>,
    cable_via: Vec<[f64; 2]>,
    /// the export window, while it is open
    export_dialog: Option<model_edit::ExportDialog>,
    import_dialog: Option<model_edit::ImportDialog>,
    load_family_dialog: Option<model_edit::LoadFamilyDialog>,
    create_family_dialog: Option<model_edit::CreateFamilyDialog>,
    /// Family Editor mode: ribbon switches to Family tabs; document treated as a family definition.
    family_editor_mode: bool,
    /// the active work plane (a reference plane / grid / level the user sketches on); None = default level plane
    work_plane: Option<ElementId>,
    family_types_dialog: Option<family_types_ui::FamilyTypesDialog>,
    /// which of the family's connectors meets the open connector it snaps to (Tab takes the next)
    family_snap_k: usize,
    /// Move with Revit's Disjoin option
    move_disjoin: bool,
    ghost_cache: Option<(snap_place::GhostKey, Option<snap_place::Ghost>)>,
    /// P2: the type of a family the selected instance takes, and the type-scoped parameters being edited on it.
    family_type_editing: Option<openbim_document::ElementId>,
    family_type_values: Vec<(String, f64)>,
    family_type_texts: Vec<crate::export::TextInput>,
    /// the print / export plan dialog (PRT-006): current plan view to PDF/DXF with a print profile
    print_dialog: print_dialog::PrintDialog,
    /// Named page setups (Revit's Page Setup): profile + paper + scale + colour mode under a name.
    page_setups: Vec<print_dialog::PageSetup>,
    /// the name for a new type
    type_name_input: String,
    warnings_cache: (u64, Vec<openbim_document::Warning>),
    show_help: bool,
    /// the settings of the application, kept between sessions
    settings: app_settings::AppSettings,
    show_options: bool,
    georef: georef_ui::GeorefInput,
    show_georef: bool,
    dark: bool,
    zoom: f32,
    pan: Vec2,
    keymap: openbim_keymap::Keymap,
    /// the keys of a shortcut being typed, replayed through a fresh matcher each key (A4)
    key_typed: Vec<openbim_keymap::Chord>,
    /// the level whose name is being edited in the options bar, and the draft: it is kept here so that typing
    /// survives the frames between keystrokes (A6)
    level_rename_id: Option<ElementId>,
    level_rename_name: String,
    /// the level whose elevation is being typed, and the text in mm (A6)
    level_elev_edit: Option<(ElementId, String)>,
    /// where the last frame drew each ribbon and quick-access button: the whole-window test clicks them (A8)
    #[cfg(test)]
    ribbon_hits: Vec<(&'static str, egui::Rect)>,
    /// where the last frame drew the other controls a window test operates (tabs, browser views, options-bar
    /// fields, dialog fields and buttons), keyed by a readable name; see `test_hit!`
    #[cfg(test)]
    widget_hits: Vec<(String, egui::Rect, egui::Rect)>,
    /// the plan's own model-to-screen transform of the last frame: (centre + pan, pixels per metre, the centre of the
    /// fitted box), so a test can aim a click at a model point (A8)
    #[cfg(test)]
    plan_transform: Option<(egui::Pos2, f32, egui::Vec2)>,
}

/// The ribbon commands that are wired. Every other button the ribbon shows is rendered disabled (decision of 21 Oct
/// 2026, A7); `run_command` gates on this list, so a command cannot run while its button is disabled.
const IMPLEMENTED: &[&str] = &[
    "automation_editor", "automation_player", "automation_mcp", "automation_snapshot", "duct", "pipe", "end_point",
    "elec_node", "elec_cable", "elec_compute", "compute", "grid", "tag", "move", "dimension", "room", "wall_joins",
    "measure", "align", "trim", "offset", "copy", "copy_clipboard", "cut", "paste", "paste_same_place", "match_type",
    "pin", "unpin", "select_instances", "create_similar", "rotate", "mirror", "delete", "level", "wall", "beam",
    "structural_column", "column", "structural_floor", "floor", "elevation", "railing", "section",
    "opening_vertical", "shaft", "opening_wall", "stair", "ceiling", "roof", "footing", "truss", "door", "window",
    "ahu", "component", "rebar", "beam_system", "foundation", "slab", "brace", "select", "clash", "ifc_export", "ifc_import",
    "new", "open", "save", "save_as", "mep_settings", "duct_insulation", "pipe_insulation", "create_mech_system",
    "create_pipe_system", "generate_layout", "system_browser", "schedule", "sheet", "add_view", "print",
    "visibility", "view_range", "project_parameters", "undo", "redo", "zoom_in", "zoom_out",
    "load_family",
];

impl Desktop {
    /// Whether a ribbon command is wired (A7).
    pub(crate) fn ribbon_command_implemented(id: &str) -> bool { IMPLEMENTED.contains(&id) }

    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        if let Some(rs) = cc.wgpu_render_state.as_ref() { openbim_viewport_egui::Viewport3d::init(rs); }
        let mut app = Self::sample();
        app.settings = app_settings::AppSettings::load();
        // restore the persisted Light/Dark theme before the first frame is shown
        app.dark = app.settings.theme.is_dark();
        app.apply_theme(&cc.egui_ctx);
        // apply persisted keyboard-shortcut overrides on top of the Revit defaults
        let _ = app.keymap.load_overrides(&app.settings.keymap_overrides);
        // a model file named on the command line (or by the system, when a .obim file is opened) is opened at start
        match std::env::args_os().nth(1).map(|a| a.to_string_lossy().into_owned()).filter(|a| !a.starts_with('-')) {
            Some(path) => { app.new_model(); app.load_model(&path); }
            None => app.check_recovery(None),
        }
        app
    }

    /// Applies the in-memory theme (`self.dark`) to the egui visuals. Centralised so `new`, the
    /// toolbar toggle and the Options window all restore/apply the persisted theme consistently.
    pub(crate) fn apply_theme(&self, ctx: &egui::Context) {
        ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
    }

    fn sample() -> Self {
        let mut keymap = openbim_keymap::Keymap::new(&["Select", "Duct", "Pipe", "Wall", "Beam", "Column", "Slab", "Footing", "Wall Footing", "Beam System", "Truss", "Rebar", "Family"])
            .expect("static shortcut commands");
        keymap.set_preset(&[
            ("Duct", &["D T"]),
            ("Pipe", &["P I"]),
            ("Wall", &["W A"]),
            ("Beam", &["B M"]),
            ("Column", &["C L"]),
            ("Slab", &["S B"]),
            ("Footing", &["F T"]),
            ("Wall Footing", &["W F"]),
            ("Beam System", &["B S"]),
            ("Truss", &["T R"]),
            ("Rebar", &["R B"]),
            ("Family", &["C M"]),
        ]).expect("static shortcuts");
        let document = Document::spatial_sample();
        let document_revision = document.revision();
        Self {
            automation: automation::Automation::default(),
            document,
            workspace: Workspace::Design, view: View::Mechanical, open_views: vec![View::Mechanical], active_view: 0, view_states: vec![Some(ViewState::default())], view_replace_pending: None, rename_view_pending: None, rename_view_text: String::new(), view_names: std::collections::HashMap::new(), tool: Tool::Select,
            selected: Some(SampleElement::Duct), ribbon: "Systems", systems_sub: "HVAC",
            width_mm: 200.0, height_mm: 300.0, elevation_mm: 2700.0,
            slope_percent: 0.0, section_pick: 0, usage_pick: openbim_document::Usage::Girder, material_pick: openbim_document::Material::Unspecified, brace_rise_mm: 3000.0, web_mm: 10.0, flange_mm: 20.0,
            axis_mm: [1000.0, 2000.0, 7000.0, 2000.0], beam_start: None, wall_pts: Vec::new(), wall_ids: Vec::new(), wall_loc: 0, wall_chain_on: true, wall_mode: 0, wall_sides: 6, wall_type: None, wall_flip: false, wall_type_name: String::new(), wall_len: String::new(), tag_leader: true, edit_pts: Vec::new(), edit_num: String::new(), drag: None, temp_edit: None, temp_text: String::new(), temp_labels: Vec::new(), measured: None, edit_ref: None, clipboard: None, offset_mm: 1000.0, offset_copy: false, edit_src: Vec::new(), multi: Vec::new(), marquee: None, mirror_copy: true, level_elev_mm: 6000.0, level_name: String::new(), beam_level: None,
            slab_points: Vec::new(), slab_pick_walls: false, slab_rings: Vec::new(), run_points: Vec::new(), branch_main: None, run_opts: mep_tool::RunOpts::default(), run_sizes: Vec::new(), system_pick: openbim_family::mep::System::SupplyAir, spacing_mm: 600.0, angle_deg: 0.0, bar_count: 3.0, family_pick: String::new(), family_type: String::new(), family_kind: None, family_values: Vec::new(), family_texts: Vec::new(), slab_thickness_mm: 200.0, slab_offset_mm: 0.0, roof_slope_deg: 30.0, ceiling_auto: true, stair_width_mm: 1000.0, stair_riser_mm: 175.0, stair_tread_mm: 280.0, stair_rise_mm: 3000.0, stair_shape: openbim_document::StairShape::Straight, stair_railing: openbim_document::RailingSides::Both, opening_host: None, opening_cands: Vec::new(), shaft_base_mm: 0.0, shaft_top: None, shaft_top_mm: 0.0, opening_sill_mm: 900.0, opening_head_mm: 2100.0, opening_from: None, view_corner: None, view_drag: None, view_edit: None, section_xf: None, elev_marks: None, slab_kind_pick: openbim_document::SlabKind::Floor, footing_thickness_mm: 500.0, join_pick: [true; 2],
            camera_yaw: 0.7, camera_pitch: 0.6,
            view3d: openbim_viewport_egui::Viewport3d::new(), view3d_revision: None, site_edit: None,
            property_query: String::new(), browser_query: String::new(), ribbon_query: String::new(), discipline: "All",
            browser_selected: String::new(),
            message: "Sample scene - use Save to keep your edits".into(),
            show_properties: true, show_browser: true, show_warnings: false, pdf_overlay: None, cad_overlay: None, imported_step: None, step_params_dialog: None, worksets: vec!["Mechanical".into(), "Electrical".into(), "Plumbing".into(), "Structural".into()], current_workset: "Mechanical".into(), grid_label_offsets: Default::default(), file: file::FileState { saved_revision: document_revision, ..Default::default() }, types_dialog: None, mep_settings_dialog: None, insulation_dialog: None, systems: Default::default(), schedules: Default::default(), views: Default::default(), sheets: Default::default(), type_editor: None, end_draft: Default::default(), compute: Default::default(), elec_draft: Default::default(), elec: Default::default(), cable_from: None, cable_via: Vec::new(), export_dialog: None, import_dialog: None, load_family_dialog: None, create_family_dialog: None, family_editor_mode: false, work_plane: None, family_types_dialog: None, family_snap_k: 0, move_disjoin: false, ghost_cache: None, family_type_editing: None, family_type_values: Vec::new(), family_type_texts: Vec::new(), print_dialog: print_dialog::PrintDialog::default(), page_setups: Vec::new(), type_name_input: String::new(), warnings_cache: (u64::MAX, Vec::new()), show_help: false, settings: if cfg!(test) { app_settings::AppSettings { autosave_minutes: 0, ..Default::default() } } else { Default::default() }, show_options: false, georef: Default::default(), show_georef: false, dark: false,
            zoom: 1.0, pan: Vec2::ZERO, keymap, key_typed: Vec::new(), level_rename_id: None, level_rename_name: String::new(), level_elev_edit: None,
            #[cfg(test)]
            ribbon_hits: Vec::new(),
            #[cfg(test)]
            widget_hits: Vec::new(),
            #[cfg(test)]
            plan_transform: None,
        }
    }

    /// What the options bar starts from when a command is chosen. Without it a tool inherits whatever the previous
    /// one left there: a door placed after a ceiling stood 2.7 m up, and the fan coil took its elevation from the same
    /// box (A5). These are the values the commands used to set inline.
    fn command_defaults(id: &str) -> ToolDefaults {
        let d = ToolDefaults::default();
        match id {
            "duct" => ToolDefaults { width_mm: Some(400.0), height_mm: Some(300.0), elevation_mm: Some(2700.0), ..d },
            "pipe" => ToolDefaults { width_mm: Some(100.0), height_mm: Some(100.0), elevation_mm: Some(600.0), ..d },
            "wall" => ToolDefaults { width_mm: Some(200.0), height_mm: Some(3000.0), elevation_mm: Some(0.0), ..d },
            "railing" => ToolDefaults { width_mm: Some(50.0), height_mm: Some(1100.0), elevation_mm: Some(0.0), spacing_mm: Some(150.0), ..d },
            "rebar" => ToolDefaults { width_mm: Some(16.0), height_mm: Some(8.0), elevation_mm: Some(40.0), spacing_mm: Some(200.0), bar_count: Some(3.0), ..d },
            "stair" => ToolDefaults { elevation_mm: Some(0.0), ..d },
            // the framing tools read the same width/height/elevation box as the wall and the duct: a beam chosen after
            // a rebar must not come out 16 x 8 mm (R3)
            "beam" | "brace" | "beam_system" | "structural_column" | "column" => ToolDefaults { width_mm: Some(200.0), height_mm: Some(300.0), elevation_mm: Some(2700.0), ..d },
            "truss" => ToolDefaults { width_mm: Some(200.0), height_mm: Some(300.0), elevation_mm: Some(2700.0), spacing_mm: Some(600.0), ..d },
            "ceiling" => ToolDefaults { slab_thickness_mm: Some(20.0), slab_offset_mm: Some(2700.0), material_pick: Some(openbim_document::Material::Unspecified), ..d },
            "roof" => ToolDefaults { slab_thickness_mm: Some(200.0), slab_offset_mm: Some(0.0), ..d },
            "floor" | "structural_floor" | "slab" => ToolDefaults { slab_thickness_mm: Some(200.0), slab_offset_mm: Some(0.0), ..d },
            // the two footing commands consume the whole box: a wall footing, and the isolated pad the Foundation
            // button places
            "footing" => ToolDefaults { width_mm: Some(200.0), height_mm: Some(300.0), slab_offset_mm: Some(0.0), material_pick: Some(openbim_document::Material::Unspecified), ..d },
            "foundation" => ToolDefaults { width_mm: Some(200.0), height_mm: Some(300.0), footing_thickness_mm: Some(500.0), slab_offset_mm: Some(0.0), material_pick: Some(openbim_document::Material::Unspecified), ..d },
            // a family stands on its level: the elevation of the ceiling or slab drawn before it must not follow it
            "door" | "window" | "ahu" | "component" => ToolDefaults { slab_offset_mm: Some(0.0), ..d },
            _ => d,
        }
    }

    /// Puts the chosen command's own numbers in the options bar before its arm runs (A5).
    fn apply_command_defaults(&mut self, id: &str) {
        let d = Self::command_defaults(id);
        if let Some(v) = d.width_mm { self.width_mm = v; }
        if let Some(v) = d.height_mm { self.height_mm = v; }
        if let Some(v) = d.elevation_mm { self.elevation_mm = v; }
        if let Some(v) = d.slab_thickness_mm { self.slab_thickness_mm = v; }
        if let Some(v) = d.slab_offset_mm { self.slab_offset_mm = v; }
        if let Some(v) = d.footing_thickness_mm { self.footing_thickness_mm = v; }
        if let Some(v) = d.spacing_mm { self.spacing_mm = v; }
        if let Some(v) = d.bar_count { self.bar_count = v; }
        if let Some(v) = d.material_pick { self.material_pick = v; }
    }

    /// Whether the tool on show has work in progress that Esc steps back from before the tool is left (A3).
    fn tool_has_pending_points(&self) -> bool {
        !self.wall_pts.is_empty() || !self.slab_points.is_empty() || !self.run_points.is_empty() || !self.edit_pts.is_empty()
            || !self.wall_ids.is_empty() || !self.cable_via.is_empty() || self.beam_start.is_some() || self.cable_from.is_some()
            || self.opening_from.is_some()
            // the reference of Align, Trim and Match Type; the host of Vertical Opening; the corner and the edges of a
            // view edit in an elevation or a section (R2)
            || self.edit_ref.is_some() || self.opening_host.is_some() || self.view_corner.is_some() || self.view_edit.is_some()
    }

    /// The tools that put an element on a level: without one they cannot start (A2).
    fn tool_needs_level(tool: Tool) -> bool {
        matches!(tool, Tool::Wall | Tool::Grid | Tool::Beam | Tool::Column | Tool::Slab | Tool::Footing | Tool::WallFooting
            | Tool::BeamSystem | Tool::Truss | Tool::Rebar | Tool::Stair | Tool::Railing | Tool::Room | Tool::Family
            | Tool::Shaft | Tool::VerticalOpening | Tool::WallOpening | Tool::Duct | Tool::Pipe | Tool::EndPoint
            | Tool::ElecNode | Tool::Cable | Tool::Section | Tool::Elevation | Tool::Tag | Tool::Dimension)
    }

    fn choose_tool(&mut self, tool: Tool) {
        if Self::tool_needs_level(tool) && self.plan_level().0 == 0 {
            self.message = "Add a level first: Architecture > Datum > Level".into();
            return;
        }
        if tool != Tool::Family { self.family_kind = None; }
        self.tool = tool;
        self.beam_start = None;
        self.wall_pts.clear();
        self.wall_ids.clear();
        self.wall_len.clear();
        self.edit_pts.clear();
        self.edit_num.clear();
        self.edit_src.clear();
        self.marquee = None;
        self.temp_edit = None;
        self.edit_ref = None;
        self.drag = None;
        self.slab_points.clear();
        if tool == Tool::Slab { self.slab_edit_begin(); }
        self.slab_rings.clear();
        self.opening_from = None;
        self.view_corner = None;
        self.view_edit = None;
        self.run_points.clear();
        self.branch_main = None;
        self.cable_from = None;
        self.cable_via.clear();
        self.run_sizes.clear();
        self.message = match tool {
            Tool::Select => "Select a sample element. Tab cycles; Esc cancels.".into(),
            Tool::Paste => "Paste: move the pointer and click to put the elements down (Esc ends).".into(),
            Tool::MatchType => "Match Type Properties: click the element whose type to copy, then the elements that should get it (Esc ends).".into(),
            Tool::Duct => { self.fit_run_system(); "Duct: click the points of the run in a plan view; elbows and reducers are placed for you. Enter or Finish places it (Backspace takes a point back, Esc ends).".into() }
            Tool::Pipe => { self.fit_run_system(); "Pipe: click the points of the run in a plan view; elbows and reducers are placed for you. Enter or Finish places it (Backspace takes a point back, Esc ends).".into() }
            Tool::EndPoint => "End point: click an open end of a duct or pipe to mark the source (pump, fan) or a terminal (coil, diffuser, fixture, sprinkler) and what it takes. Esc ends.".into(),
            Tool::ElecNode => "Electrical node: click in a plan view to place a source, a board or a load (set in the options bar). Esc ends.".into(),
            Tool::Cable => "Cable: click the source or a board, then the points on the way, then the node it feeds. Esc cancels.".into(),
            Tool::Beam => "Beam: click the start and the end in a plan view (Esc ends), or type the ends in the options bar and press Place Beam.".into(),
            Tool::Column => "Column: click its place in a plan view; it stands from that level to the next one up (Esc ends).".into(),
            Tool::Slab if self.slab_kind_pick == openbim_document::SlabKind::Ceiling => "Ceiling: Automatic Ceiling hangs a ceiling in the closed space you click (its outline shows as you move); untick Automatic in the options bar to sketch the corners instead (Enter, a click on the first corner or Finish closes it). Esc ends.".into(),
            Tool::Slab if self.slab_kind_pick == openbim_document::SlabKind::Roof => "Roof by Footprint: click the corners of the roof at the eaves in a plan view, then Enter, a click on the first corner or Finish closes it. Every edge slopes by the Slope in the options bar; select the roof afterwards to choose which edges define a slope (two opposite ones make a gable, one a shed). Backspace takes the last corner back, Esc cancels.".into(),
            Tool::Railing => "Railing: click the points of the path in a plan view (a post stands at each), then Enter or Finish; Backspace takes a point back, Esc cancels. Height, rail size, baluster spacing and base offset are in the options bar; the top rail is at the height.".into(),
            Tool::Slab => "Slab: click the corners in a plan view, then Enter, a click on the first corner or Finish closes it (Backspace takes the last corner back, Esc cancels).".into(),
            Tool::Elevation => "Elevation: click in a plan view to put a marker with its four views (East, North, South, West); select it to switch views on or off. Double click a pointer to open that view. Esc ends.".into(),
            Tool::Section => "Section: pick the two ends of the line in a plan view (or press, drag and release); the view looks to the left of the line. Open it from the Project Browser. Esc ends.".into(),
            Tool::VerticalOpening => "Vertical Opening: click the floor, roof or ceiling to cut (Tab takes the next one under the click), then click the corners of the opening, then Enter, a click on the first corner or Finish closes it. It cuts that one element straight through. Esc steps back.".into(),
            Tool::Shaft => "Shaft Opening: click the corners of the shaft in a plan view, then Enter, a click on the first corner or Finish closes it. It cuts every floor, foundation slab and ceiling between its base and its top level (set in the options bar). Esc cancels.".into(),
            Tool::WallOpening => "Wall Opening: click a wall at one end of the opening, then at the other end; the sill and head heights are in the options bar. Esc ends.".into(),
            Tool::Footing => "Isolated footing: click its place in a plan view; it is as wide and long as the options bar says, under the level (Esc ends).".into(),
            Tool::WallFooting => "Wall footing: click a wall in a plan view; the footing runs under it and follows it (Esc ends).".into(),
            Tool::BeamSystem => "Beam system: click the corners of the area in a plan view, then Enter, a click on the first corner or Finish lays parallel beams at the spacing and angle of the options bar (Esc cancels).".into(),
            Tool::Truss => "Truss: click the start and the end of the truss in a plan view; it is built with the panel length, depth and member size of the options bar (Esc ends).".into(),
            Tool::Rebar => "Rebar: click a beam in a plan view; bars and stirrups are placed inside it and follow it (Esc ends).".into(),
            Tool::Family => format!("{}: click in a plan view (a door or window: click the wall it goes in); Esc ends.", self.family_pick),
            Tool::Move => "Move: pick the base point, then the point to move to (or press, drag and release); type a distance in mm and Enter. Shift keeps it square. With nothing selected, click an element first. Esc ends.".into(),
            Tool::Copy => "Copy: pick the base point, then each point a copy goes to (or press, drag and release); type a distance in mm and Enter. Esc ends.".into(),
            Tool::Rotate => "Rotate: pick the centre, then the start and the end of the turn; or type an angle in degrees (counter-clockwise) and Enter. Esc ends.".into(),
            Tool::Mirror => "Mirror: draw the axis by two points (or press, drag and release); Copy in the options bar keeps the original. Esc ends.".into(),
            Tool::Stair => "Stair: click the foot of the first riser, then a point in the direction the stair climbs (Shift keeps it square); the number of risers follows from the height between the levels and the highest riser allowed. Esc ends.".into(),
            Tool::Room => "Room: move over a closed space (its walls shown) and click to place a room there; it follows the walls and shows its name and area. Esc ends.".into(),
            Tool::WallJoins => "Wall Joins: click the end of a wall (a marker shows the ends) to disallow its join, so that it stays square, or to allow it again. Esc ends.".into(),
            Tool::Dimension => "Aligned Dimension: click the first point, the second point, then where the dimension line goes (corners of walls, ends of grids and 50 mm positions snap; Shift keeps it square). In an elevation or section: click two vertical (or two horizontal) edges, then where the line goes; it stays in that view. Esc ends.".into(),
            Tool::Measure => "Measure between two points: click both; the distance, its x and y parts and the angle are shown. Esc ends.".into(),
            Tool::Align => "Align: click the wall, beam or grid to align to, then the one to move onto its line (it keeps the reference until Esc).".into(),
            Tool::Trim => "Trim/Extend to Corner: click the first wall on the part to keep, then the second wall on the part to keep; both meet at the corner of their lines (Esc ends).".into(),
            Tool::Offset => "Offset: set the distance in the options bar and click a wall, beam or grid on the side to offset to; Copy keeps the original (Esc ends).".into(),
            Tool::Tag => "Tag: click an element in a plan view, an elevation or a section; the tag shows its type (or its name), follows the element and belongs to the view it is placed in. With Leader on, the label stands off the element on a leader line. Esc ends.".into(),
            Tool::Grid => "Grid: click the start and the end of the grid line in a plan view, or press, drag and release; each is named after the last (Shift keeps it square, Esc ends).".into(),
            Tool::Level => "Level: set the elevation (and a name, if you want one) in the options bar and click in a plan view. Revit draws a level in an elevation view; there is none here yet, so the click only confirms. The level gets its own plan view.".into(),
            Tool::Wall => "Wall: (Draw: Line, Rectangle or Pick Lines in the options bar; Spacebar flips the wall) click the start and then each next point in a plan view, or press, drag and release for one wall. The line you draw is the Location Line of the options bar (centreline, or the exterior or interior face, exterior on the left when you draw clockwise). Shift keeps it square, a typed length (mm) and Enter sets it, clicking the first point closes the loop; Esc ends the chain, Esc again the tool.".into(),
        };
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("OpenBIM").strong());
            ui.separator();
            // Quick access toolbar: common commands with the 3D icons.
            for (id, label) in ribbon::QUICK_ACCESS {
                let img = ribbon::icon_image(ui, id, self.dark, Vec2::splat(24.0));
                let enabled = Self::ribbon_command_implemented(id);
                let button = ui.add_enabled(enabled, egui::Button::image(img).frame(false));
                let button = if enabled { button.on_hover_text(*label) } else { button.on_hover_text(format!("{label} - planned, not implemented yet")) };
                #[cfg(test)]
                self.ribbon_hits.push((id, button.rect));
                if button.clicked() { self.run_command(id, label); }
            }
            ui.separator();
            ui.label("Project1 (SAMPLE)");
            if ui.button("Print / Export").clicked() { self.print_dialog.open = true; }
            ui.separator();
            for (label, ws) in [
                ("Design", Workspace::Design), ("Coordination", Workspace::Coordination),
                ("Automation", Workspace::Automation),
            ] {
                if ui.selectable_label(self.workspace == ws, label).clicked() {
                    self.workspace = ws;
                }
            }
            ui.separator();
            // Discipline filter (REA-008) — hides ribbon tabs not relevant to the active discipline.
            let disciplines: &[&str] = &["Architecture", "Structure", "MEP", "All"];
            for d in disciplines {
                if ui.selectable_label(self.discipline == *d, *d).clicked() {
                    self.discipline = *d;
                }
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut self.ribbon_query).hint_text("Search commands...").desired_width(160.0));
            // Show matching commands when searching
            if !self.ribbon_query.trim().is_empty() {
                let q = self.ribbon_query.trim().to_lowercase();
                let matches: Vec<_> = ribbon::all_commands().into_iter().filter(|(_, label, _)| label.to_lowercase().contains(&q)).collect();
                if !matches.is_empty() {
                    ui.separator();
                    for (id, label, tab) in matches.iter().take(8) {
                        let enabled = Self::ribbon_command_implemented(id);
                        if ui.add_enabled(enabled, egui::Button::new(format!("{} ({})", label, tab)).small()).clicked() {
                            self.run_command(id, label);
                            self.ribbon_query.clear();
                        }
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if self.dark { "Light" } else { "Dark" }).clicked() {
                    self.dark = !self.dark;
                    self.settings.theme = if self.dark { app_settings::Theme::Dark } else { app_settings::Theme::Light };
                    self.apply_theme(ui.ctx());
                    if !self.settings.save() { self.message = "Theme could not be saved; it holds for this session".into(); }
                }
                if ui.button("Help").clicked() { self.show_help = !self.show_help; }
                let r = ui.button("Options");
                test_hit!(self, r, "toolbar:options");
                if r.clicked() { self.show_options = !self.show_options; }
                if ui.button("Georeferencing").clicked() { self.show_georef = !self.show_georef; }
            });
        });
    }

    /// Commands without an implementation yet report it in the status bar.
    fn run_command(&mut self, id: &str, label: &str) {
        if !Self::ribbon_command_implemented(id) {
            self.message = format!("{} is not implemented yet - the button is disabled (see {})", label, ribbon::backlog_row(id));
            return;
        }
        self.apply_command_defaults(id);
        match id {
            "automation_editor" | "automation_player" => {
                self.workspace = Workspace::Automation;
                self.automation.player = id == "automation_player";
            }
            "automation_mcp" | "automation_snapshot" => {
                self.workspace = Workspace::Automation;
                self.automation.show_mcp = true;
                if id == "automation_snapshot" { self.automation.export_snapshot(&self.document); }
            }
            "duct" => self.choose_tool(Tool::Duct),
            "pipe" => { self.system_pick = openbim_family::mep::System::ChilledWaterSupply; self.choose_tool(Tool::Pipe) }
            "end_point" => self.choose_tool(Tool::EndPoint),
            "elec_node" => self.choose_tool(Tool::ElecNode),
            "elec_cable" => self.choose_tool(Tool::Cable),
            "elec_compute" => self.elec.open = true,
            "compute" => self.compute.open = true,
            "grid" => self.choose_tool(Tool::Grid),
            "tag" => self.choose_tool(Tool::Tag),
            "move" => self.choose_tool(Tool::Move),
            "dimension" => self.choose_tool(Tool::Dimension),
            "room" => self.choose_tool(Tool::Room),
            "wall_joins" => self.choose_tool(Tool::WallJoins),
            "measure" => { self.measured = None; self.choose_tool(Tool::Measure) }
            "align" => self.choose_tool(Tool::Align),
            "trim" => self.choose_tool(Tool::Trim),
            "offset" => self.choose_tool(Tool::Offset),
            "copy" => self.choose_tool(Tool::Copy),
            "copy_clipboard" => self.copy_to_clipboard(false),
            "cut" => self.copy_to_clipboard(true),
            "paste" => self.paste_start(),
            "paste_same_place" => self.paste_same_place(),
            "match_type" => self.choose_tool(Tool::MatchType),
            "pin" => self.pin_selected(true),
            "unpin" => self.pin_selected(false),
            "select_instances" => self.select_all_instances(false),
            "create_similar" => self.create_similar(),
            "rotate" => self.choose_tool(Tool::Rotate),
            "mirror" => self.choose_tool(Tool::Mirror),
            "delete" => self.delete_selected(),
            "level" => { self.level_elev_mm = self.levels().last().map_or(0.0, |l| l.2 * 1000.0 + 3000.0); self.level_name.clear(); self.choose_tool(Tool::Level) }
            "wall" => { self.wall_type = None; self.wall_flip = false; self.choose_tool(Tool::Wall) }
            "beam" => { self.usage_pick = openbim_document::Usage::Girder; self.choose_tool(Tool::Beam) }
            "structural_column" | "column" => self.choose_tool(Tool::Column),
            "structural_floor" | "floor" => { self.slab_kind_pick = openbim_document::SlabKind::Floor; self.choose_tool(Tool::Slab) }
            "elevation" => { self.edit_pts.clear(); self.choose_tool(Tool::Elevation) }
            "railing" => self.choose_tool(Tool::Railing),
            "section" => { self.edit_pts.clear(); self.choose_tool(Tool::Section) }
            "opening_vertical" => { self.opening_host = None; self.opening_cands.clear(); self.slab_points.clear(); self.choose_tool(Tool::VerticalOpening) }
            "shaft" => { (self.shaft_base_mm, self.shaft_top_mm, self.shaft_top) = (0.0, 0.0, None); self.choose_tool(Tool::Shaft) }
            "opening_wall" => self.choose_tool(Tool::WallOpening),
            "stair" => { self.edit_pts.clear(); self.choose_tool(Tool::Stair) }
            "ceiling" => { self.slab_kind_pick = openbim_document::SlabKind::Ceiling; self.choose_tool(Tool::Slab) }
            "roof" => { self.slab_kind_pick = openbim_document::SlabKind::Roof; self.choose_tool(Tool::Slab) }
            "footing" => self.choose_tool(Tool::WallFooting),
            "truss" => self.choose_tool(Tool::Truss),
            "door" => { self.family_pick = "Door, single flush".into(); self.family_type.clear(); self.family_kind = Some(openbim_family::Category::Door); self.choose_tool(Tool::Family) }
            "window" => { self.family_pick = "Window, fixed with mullions".into(); self.family_type.clear(); self.family_kind = Some(openbim_family::Category::Window); self.choose_tool(Tool::Family) }
            "ahu" | "component" => { self.family_pick = "Fan coil unit, ducted".into(); self.family_type.clear(); self.family_kind = Some(openbim_family::Category::UnitaryEquipment); self.choose_tool(Tool::Family) }
            "rebar" => self.choose_tool(Tool::Rebar),
            "beam_system" => { self.usage_pick = openbim_document::Usage::Joist; self.choose_tool(Tool::BeamSystem) }
            "foundation" => self.choose_tool(Tool::Footing),
            "slab" => { self.slab_kind_pick = openbim_document::SlabKind::Foundation; self.choose_tool(Tool::Slab) }
            "brace" => { self.usage_pick = openbim_document::Usage::Brace; self.choose_tool(Tool::Beam) }
            "select" => self.choose_tool(Tool::Select),
            "clash" => {
                let clashes = openbim_clash::detect_clashes(&self.document);
                self.message = format!("{} clash(es) detected", clashes.len());
            }
            "ifc_import" => self.import_ifc_file(),
            "ifc_export" => self.export_ifc_file(),
            "load_family" => self.load_family_file(),
            "create_family" => self.create_family_file(),
            "family_editor_toggle" => { self.family_editor_mode = !self.family_editor_mode; if self.family_editor_mode { self.ribbon = "Family"; } self.message = if self.family_editor_mode { "Family Editor mode on".into() } else { "Family Editor mode off".into() }; }
            "import_dwg" => self.import_dwg_file(),
            "import_step" => self.import_step_file(),
            "import_pdf" => self.import_pdf_file(),
            "new" => self.new_model(),
            "open" => self.open_file(),
            "save" => self.save_file(false),
            "save_as" => self.save_file(true),
            "mep_settings" => self.open_mep_settings(),
            "duct_insulation" => self.open_insulation(false),
            "pipe_insulation" => self.open_insulation(true),
            "create_mech_system" => self.open_create_system(false),
            "create_pipe_system" => self.open_create_system(true),
            "generate_layout" => self.open_generate_layout(),
            "system_browser" => self.systems.browser = !self.systems.browser,
            "schedule" => self.open_new_schedule(),
            "sheet" => self.open_new_sheet(),
            "add_view" => { let key = crate::view_ui::view_key(self.view).filter(|k| self.placeable_views().iter().any(|v| &v.0 == k)); self.open_add_view(None, key) }
            "print" => self.open_print(),
            "visibility" => self.open_vis_graphics(),
            "view_range" => self.open_view_range(),
            "project_parameters" => self.types_dialog = Some(model_edit::TypesDialog::default()),
            "undo" => self.undo_document(),
            "redo" => self.redo_document(),
            "zoom_in" => self.zoom = (self.zoom * 1.25).min(4.0),
            "zoom_out" => self.zoom = (self.zoom / 1.25).max(0.3),
            other => {
                // the list above and this match have to agree: a listed command with no arm is a bug, not a message
                debug_assert!(false, "ribbon command {other} is listed as implemented but has no arm");
                self.message = format!("{} is not implemented in this UI prototype (see {})", label, ribbon::backlog_row(other));
            }
        }
    }

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        // Ribbon tabs, each with a small icon.
        ui.horizontal(|ui| {
            for (tab, icon) in ribbon::TABS {
                if *tab == "Family" && !self.family_editor_mode { continue; }
                if !ribbon::tab_visible(tab, self.discipline) { continue; }
                let img = ribbon::icon_image(ui, icon, self.dark, Vec2::splat(16.0));
                let response = ui.add(egui::Button::image_and_text(img, *tab).selected(self.ribbon == *tab).frame(false));
                test_hit!(self, response, "tab:{tab}");
                if response.clicked() {
                    self.ribbon = tab;
                }
            }
            // If the active tab was filtered out by the discipline switch, fall back to the first visible tab.
            if !ribbon::tab_visible(self.ribbon, self.discipline) {
                self.ribbon = ribbon::TABS.iter().find(|(t, _)| ribbon::tab_visible(t, self.discipline)).map(|(t, _)| *t).unwrap_or("Modify");
            }
        });
        if self.ribbon == "Systems" {
            ui.horizontal(|ui| {
                for (sub, icon) in ribbon::SYSTEMS_SUBTABS {
                    let img = ribbon::icon_image(ui, icon, self.dark, Vec2::splat(14.0));
                    if ui.add(egui::Button::image_and_text(img, *sub).selected(self.systems_sub == *sub).small()).clicked() {
                        self.systems_sub = sub;
                    }
                }
            });
        }
        ui.separator();
        let groups = ribbon::groups(self.ribbon, self.systems_sub);
        let mut clicked = None;
        #[cfg(test)]
        let mut hits: Vec<(&'static str, egui::Rect)> = Vec::new();
        egui::ScrollArea::horizontal().id_salt("ribbon").show(ui, |ui| {
            ui.horizontal(|ui| {
                for group in groups {
                    ribbon::panel(ui, group.title, |ui| {
                        for cmd in group.commands {
                            let roofing = self.tool == Tool::Slab && matches!(self.slab_kind_pick, openbim_document::SlabKind::Roof | openbim_document::SlabKind::Ceiling);
                            let active = (cmd.id == "roof" && roofing && self.slab_kind_pick == openbim_document::SlabKind::Roof) || (cmd.id == "ceiling" && roofing && self.slab_kind_pick == openbim_document::SlabKind::Ceiling) || (!matches!(cmd.id, "roof" | "ceiling") && !(roofing && matches!(cmd.id, "structural_floor" | "floor" | "slab")) && matches!((cmd.id, self.tool),
                                ("duct", Tool::Duct) | ("pipe", Tool::Pipe) | ("end_point", Tool::EndPoint) | ("elec_node", Tool::ElecNode) | ("elec_cable", Tool::Cable) | ("wall", Tool::Wall) | ("grid", Tool::Grid) | ("tag", Tool::Tag) | ("move", Tool::Move) | ("dimension", Tool::Dimension) | ("wall_joins", Tool::WallJoins) | ("room", Tool::Room) | ("stair", Tool::Stair) | ("railing", Tool::Railing) | ("section", Tool::Section) | ("elevation", Tool::Elevation) | ("shaft", Tool::Shaft) | ("opening_vertical", Tool::VerticalOpening) | ("opening_wall", Tool::WallOpening) | ("measure", Tool::Measure) | ("align", Tool::Align) | ("trim", Tool::Trim) | ("offset", Tool::Offset) | ("copy", Tool::Copy) | ("paste", Tool::Paste) | ("match_type", Tool::MatchType) | ("rotate", Tool::Rotate) | ("mirror", Tool::Mirror) | ("level", Tool::Level) | ("beam" | "brace", Tool::Beam) | ("structural_column", Tool::Column) | ("structural_floor" | "floor" | "slab", Tool::Slab) | ("foundation", Tool::Footing) | ("footing", Tool::WallFooting) | ("beam_system", Tool::BeamSystem) | ("truss", Tool::Truss) | ("rebar", Tool::Rebar) | ("door" | "window" | "ahu" | "component", Tool::Family)));
                            let enabled = Self::ribbon_command_implemented(cmd.id);
                            let response = ribbon::big_button(ui, cmd.id, ribbon::label(cmd), self.dark, active, enabled);
                            #[cfg(test)]
                            hits.push((cmd.id, response.rect));
                            if response.clicked() {
                                clicked = Some((cmd.id, ribbon::label(cmd)));
                            }
                        }
                    });
                }
            });
        });
        #[cfg(test)]
        self.ribbon_hits.extend(hits);
        if let Some((id, label)) = clicked { self.run_command(id, label); }
        ui.separator();
        panels::options(self, ui);
    }

    fn viewport(&mut self, ui: &mut egui::Ui) { model_view::show(self, ui); }
}

impl Desktop {
    fn redo_document(&mut self) {
        match self.document.redo() {
            Ok(true) => { self.sync_duct_controls(); self.message = "Redid the transaction".into(); },
            Ok(false) => self.message = "Nothing to redo".into(),
            Err(err) => self.message = format!("Redo rejected: {err:?}"),
        }
    }

    fn undo_document(&mut self) {
        match self.document.undo() {
            Ok(true) => { self.sync_duct_controls(); self.message = "Undid last document transaction".into(); },
            Ok(false) => self.message = "Nothing to undo".into(),
            Err(err) => self.message = format!("Undo rejected: {err:?}"),
        }
    }
}

impl Desktop {
    /// The keys the whole window answers to, before the ribbon and the viewport are drawn: Esc, Tab, undo/redo,
    /// Ctrl+P, Ctrl+S/N/O and the keymap's typed shortcuts (A3, A4).
    fn global_keys(&mut self, ctx: &egui::Context) {
        // Esc steps back inside the tool first: the viewport clears what is pending, and the tool is left only when
        // nothing is pending, as in Revit.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.tool_has_pending_points() { self.choose_tool(Tool::Select); }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
            let ids: Vec<_> = self.document.elements().map(|el| el.id).collect();
            if !ids.is_empty() {
                let next = self.selected_id().and_then(|id| ids.iter().position(|candidate| *candidate == id))
                    .map_or(0, |index| (index + 1) % ids.len());
                self.select_element(Some(ids[next]));
            }
        }
        // the clipboard keys keep the place they had before the window was split into this method (R5)
        self.clipboard_events(ctx);
        // Ctrl+Y (also Ctrl+Shift+Z) redoes, Ctrl+Z undoes, as in Revit
        let redo = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))));
        if redo { self.redo_document(); }
        else if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            self.undo_document();
        }
        // Ctrl+P opens Print / Export globally, even while a toolbar button or text field owns focus.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P)) {
            self.print_dialog.open = true;
        }
        // Ctrl+S, Ctrl+N and Ctrl+O, as in Revit. They wait for the keyboard, so typing in a field is never stolen.
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) { self.save_file(false); }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) { self.new_model(); }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O)) { self.open_file(); }
            self.keymap_keys(ctx);
        }
    }

    /// The keymap's typed shortcuts, one key at a time ("W A" for Wall), as Revit does (A4).
    fn keymap_keys(&mut self, ctx: &egui::Context) {
        let mut pressed: Vec<char> = Vec::new();
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } = event {
                    if !modifiers.any() { if let Some(c) = key_char(*key) { pressed.push(c); } }
                }
            }
        });
        for c in pressed { self.keymap_key(&c.to_string()); }
    }

    /// One key of a typed shortcut. The keys typed so far are replayed through a fresh matcher, because a matcher
    /// borrows the keymap and cannot be kept from one frame to the next.
    fn keymap_key(&mut self, name: &str) {
        let Ok(chord) = openbim_keymap::Chord::new(openbim_keymap::Mods::NONE, name) else { return };
        self.key_typed.push(chord);
        let mut matcher = self.keymap.matcher();
        let mut result = openbim_keymap::Match::Pending;
        for chord in self.key_typed.clone() { result = matcher.feed(chord); }
        match result {
            // the start of a longer shortcut: keep typing
            openbim_keymap::Match::Pending => {}
            openbim_keymap::Match::None => self.key_typed.clear(),
            openbim_keymap::Match::Command(command) => {
                self.key_typed.clear();
                match Self::keymap_command_id(&command) {
                    Some(id) => self.run_command(id, id),
                    None => self.message = format!("{command} has a shortcut but no command here yet"),
                }
            }
            // several commands share the shortcut: Revit offers them, here they are named
            openbim_keymap::Match::Choice(commands) => {
                self.key_typed.clear();
                self.message = format!("{name} is the shortcut of several commands: {}", commands.join(", "));
            }
        }
    }

    /// The ribbon command a keymap command name stands for (A4).
    fn keymap_command_id(name: &str) -> Option<&'static str> {
        Some(match name {
            "Select" => "select", "Duct" => "duct", "Pipe" => "pipe", "Wall" => "wall", "Beam" => "beam",
            "Column" => "structural_column", "Slab" => "floor", "Footing" => "foundation", "Wall Footing" => "footing",
            "Beam System" => "beam_system", "Truss" => "truss", "Rebar" => "rebar", "Family" => "component",
            _ => return None,
        })
    }
}

/// The letter or digit a key stands for, for the keymap's typed shortcuts (A4).
fn key_char(key: egui::Key) -> Option<char> {
    let name = key.name();
    let mut chars = name.chars();
    let c = chars.next()?;
    (chars.next().is_none() && c.is_ascii_alphanumeric()).then(|| c.to_ascii_uppercase())
}

impl Desktop {
    /// The whole window: the keys, the toolbar, the ribbon, the options bar, the panels and the viewport. It is separate
    /// from the `eframe::App` entry point so that a test can drive the whole window without an `eframe::Frame` (A8).
    fn window(&mut self, ui: &mut egui::Ui) {
        #[cfg(test)]
        { self.ribbon_hits.clear(); self.widget_hits.clear(); }
        let ctx = ui.ctx().clone();
        self.global_keys(&ctx);
        egui::Panel::top("header").show(ui, |ui| {
            self.toolbar(ui);
            self.ribbon(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.message);
                ui.separator();
                // Detail Level dropdown (Coarse / Medium / Fine) for the active view.
                ui.label("Detail");
                let detail_key = self.current_view_key();
                let current_detail = detail_key.as_ref().map(|k| self.document.view_settings(k).detail).unwrap_or(openbim_document::Detail::Medium);
                let mut next_detail = current_detail;
                egui::ComboBox::from_id_salt("bottom_detail").selected_text(current_detail.name()).width(70.0).show_ui(ui, |ui| {
                    for d in openbim_document::Detail::ALL { ui.selectable_value(&mut next_detail, d, d.name()); }
                });
                if next_detail != current_detail {
                    if let Some(k) = detail_key {
                        let mut vs = self.document.view_settings(&k);
                        vs.detail = next_detail;
                        self.apply_view_settings(&vs, "Detail level changed");
                    }
                }
                ui.separator();
                ui.label("Orthographic / fit to view");
                ui.separator();
                // Workset dropdown.
                ui.label("Workset");
                egui::ComboBox::from_id_salt("bottom_workset").selected_text(&self.current_workset).width(110.0).show_ui(ui, |ui| {
                    for w in self.worksets.clone() { ui.selectable_value(&mut self.current_workset, w.clone(), w); }
                });
                ui.separator();
                ui.separator();
                let name = self.file.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).and_then(|n| n.to_str()).unwrap_or("not saved yet");
                ui.label(format!("BIM revision {} | {name}{}", self.document.revision(), if self.is_dirty() { " | unsaved changes" } else { "" }));
                ui.separator();
                let n = self.warnings().len();
                let label = if n == 0 { "No warnings".to_string() } else { format!("⚠ {n} warning{}", if n == 1 { "" } else { "s" }) };
                if ui.selectable_label(self.show_warnings, label).clicked() { self.show_warnings = !self.show_warnings; }
                ui.separator();
                dock::status_toggles(self, ui);
            });
        });
        self.warnings_window(&ctx);
        self.export_window(&ctx);
        self.import_window(&ctx);
        self.load_family_window(&ctx);
        self.create_family_window(&ctx);
        self.family_types_window(&ctx);
        self.types_window(&ctx);
        self.mep_settings_window(&ctx);
        self.insulation_window(&ctx);
        self.create_system_window(&ctx);
        self.system_browser_window(&ctx);
        self.new_schedule_window(&ctx);
        self.schedule_props_window(&ctx);
        self.rename_schedule_window(&ctx);
        self.view_range_window(&ctx);
        self.new_sheet_window(&ctx);
        self.sheet_props_window(&ctx);
        self.add_view_window(&ctx);
        self.print_window(&ctx);
        self.print_dialog.show(&ctx, &self.document, self.view, self.workspace == Workspace::Design, &mut self.page_setups);
        self.step_params_window(&ctx);
        self.vis_graphics_window(&ctx);
        self.layout_window(&ctx);
        self.type_editor_window(&ctx);
        self.compute_window(&ctx);
        self.elec_window(&ctx);
        self.file_window(&ctx);
        // the timer of the automatic save; a test drives it by moving the app's clock on (`clock::advance`)
        self.autosave();
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
        self.options_window(&ctx);
        self.georef_window(&ctx);
        // Dockable side panels (Properties / Browser / Warnings): each pinned to any edge, resizable,
        // and saved to settings. Replaces the old single left sidebar.
        dock::render(self, ui);
        egui::CentralPanel::default().show(ui, |ui| {
            match self.workspace {
                Workspace::Design => self.viewport(ui),
                Workspace::Coordination => {
                    ui.heading("Coordination - planned");
                    ui.label("Federated IFC models, clash detection and issue tracking require the shared BIM document core.");
                }
                Workspace::Automation => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.automation.ui(ui, &mut self.document) { self.sync_duct_controls(); }
                    });
                }
            }
        });
        if self.show_help {
            egui::Window::new("Prototype scope").open(&mut self.show_help).show(&ctx, |ui| {
                ui.label("Plan and orbitable 3D views derive geometry from the same in-memory document.");
                ui.label("Model files, server, IFC editing, routing and persistence are not connected yet.");
                ui.label("ESC: cancel. TAB: cycle sample selection. Mouse wheel: zoom. Middle-drag: pan. Shift+Middle-drag: orbit in 3D.");
                ui.label(format!("Existing keymap crate: Duct shortcut = {:?}",
                    self.keymap.shortcuts("Duct")));
            });
        }
    }
}

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) { self.window(ui); }
}

/// Open the desktop window and run until it closes. The binary of this package calls it; the full
/// build (`full/desktop-full`) installs the electrical solver first and then calls it.
pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default()
        .with_title("OpenBIM - Revit-style Desktop Prototype")
        .with_inner_size([1440.0, 850.0])
        .with_min_inner_size([900.0, 550.0]), ..Default::default() };
    eframe::run_native("OpenBIM Desktop Prototype", options,
        Box::new(|cc| Ok(Box::new(Desktop::new(cc)))))
}
