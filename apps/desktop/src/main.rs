//! First executable desktop prototype. The viewport is a clearly labelled SAMPLE scene:
//! no model persistence, IFC import, live routing, or central collaboration is implied.
#![forbid(unsafe_code)]

mod icons;
mod ribbon;
mod panels;
mod model_view;
mod view3d;
mod section_ui;
mod view_tools;
mod view_notes;
mod model_edit;
mod export;
mod import;
mod file;
mod mep_tool;
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Plan, UpperPlan, ThreeD, Mechanical,
    /// one of the four building elevations: East, North, South, West
    Elevation(u8),
    /// the view of a section line (its element number)
    Section(u64),
    /// one view of an elevation marker: the marker's element number and the direction (0..4 as the four elevations)
    Marked(u64, u8),
    /// the plan of a level added with the Level tool
    Level(u64) }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool { Select, Railing, Stair, Room, Section, Elevation, Shaft, WallOpening, WallJoins, Dimension, Measure, Move, Copy, Rotate, Mirror, Align, Trim, Offset, Duct, Pipe, EndPoint, ElecNode, Cable, Wall, Grid, Level, Tag, Beam, Column, Slab, Footing, WallFooting, BeamSystem, Truss, Rebar, Family }
/// A temporary dimension of the selected wall that is being retyped: its length, or its distance to a neighbour.
#[derive(Clone, Copy, PartialEq, Debug)]
enum TempDim { Length(ElementId), Distance(ElementId, ElementId) }
impl TempDim {
    /// A number that tells one temporary dimension from another.
    fn key(self) -> (u8, u64, u64) { match self { TempDim::Length(a) => (0, a.0, 0), TempDim::Distance(a, b) => (1, a.0, b.0) } }
}
/// What is being dragged in Select mode: an element to move, or one end of a wall.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag { Move { id: ElementId, from: [f64; 2] }, Grip { id: ElementId, at_start: bool } }
#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleElement { Duct, Model(ElementId) }

struct Desktop {
    automation: automation::Automation,
    document: Document,
    workspace: Workspace,
    view: View,
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
    /// the Duct and Pipe tools: the points clicked so far, the size of the leg that ends at each from the second on, and the system
    run_points: Vec<[f64; 2]>,
    /// the duct or pipe the run being drawn branches from, when its first click was on one
    branch_main: Option<ElementId>,
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
    /// Shaft: how far above the base level it starts, the level it goes up to (None: the level above) and how far above that it ends
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
    elev_marks: Option<(u64, Vec<(u8, [f64; 2], [f64; 2])>)>,
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
    browser_selected: String,
    message: String,
    show_properties: bool,
    show_browser: bool,
    /// the warnings window is open; the list is worked out again only when the document has a new revision
    show_warnings: bool,
    /// the file the model is saved in, and the questions about unsaved changes
    file: file::FileState,
    /// the types window, while it is open
    types_dialog: Option<model_edit::TypesDialog>,
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
    /// the name for a new type
    type_name_input: String,
    warnings_cache: (u64, Vec<openbim_document::Warning>),
    show_help: bool,
    dark: bool,
    zoom: f32,
    pan: Vec2,
    keymap: openbim_keymap::Keymap,
}

impl Desktop {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        if let Some(rs) = cc.wgpu_render_state.as_ref() { openbim_viewport_egui::Viewport3d::init(rs); }
        Self::sample()
    }

    fn sample() -> Self {
        let mut keymap = openbim_keymap::Keymap::new(&["Select", "Duct", "Pipe", "Wall", "Beam", "Column", "Slab", "Footing", "Wall Footing", "Beam System", "Truss", "Rebar", "Family"])
            .expect("static shortcut commands");
        keymap.set_preset(&[
            ("Duct", &["D D"]),
            ("Pipe", &["P I"]),
            ("Wall", &["W A"]),
            ("Beam", &["B M"]),
            ("Column", &["C L"]),
            ("Slab", &["S L"]),
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
            workspace: Workspace::Design, view: View::Mechanical, tool: Tool::Select,
            selected: Some(SampleElement::Duct), ribbon: "Systems", systems_sub: "HVAC",
            width_mm: 200.0, height_mm: 300.0, elevation_mm: 2700.0,
            slope_percent: 0.0, section_pick: 0, usage_pick: openbim_document::Usage::Girder, material_pick: openbim_document::Material::Unspecified, brace_rise_mm: 3000.0, web_mm: 10.0, flange_mm: 20.0,
            axis_mm: [1000.0, 2000.0, 7000.0, 2000.0], beam_start: None, wall_pts: Vec::new(), wall_ids: Vec::new(), wall_loc: 0, wall_chain_on: true, wall_mode: 0, wall_type: None, wall_flip: false, wall_type_name: String::new(), wall_len: String::new(), tag_leader: true, edit_pts: Vec::new(), edit_num: String::new(), drag: None, temp_edit: None, temp_text: String::new(), temp_labels: Vec::new(), measured: None, edit_ref: None, offset_mm: 1000.0, offset_copy: false, edit_src: Vec::new(), multi: Vec::new(), marquee: None, mirror_copy: true, level_elev_mm: 6000.0, level_name: String::new(), beam_level: None,
            slab_points: Vec::new(), run_points: Vec::new(), branch_main: None, run_sizes: Vec::new(), system_pick: openbim_family::mep::System::SupplyAir, spacing_mm: 600.0, angle_deg: 0.0, bar_count: 3.0, family_pick: String::new(), family_type: String::new(), family_values: Vec::new(), family_texts: Vec::new(), slab_thickness_mm: 200.0, slab_offset_mm: 0.0, roof_slope_deg: 30.0, ceiling_auto: true, stair_width_mm: 1000.0, stair_riser_mm: 175.0, stair_tread_mm: 280.0, stair_rise_mm: 3000.0, shaft_base_mm: 0.0, shaft_top: None, shaft_top_mm: 0.0, opening_sill_mm: 900.0, opening_head_mm: 2100.0, opening_from: None, view_corner: None, view_drag: None, view_edit: None, section_xf: None, elev_marks: None, slab_kind_pick: openbim_document::SlabKind::Floor, footing_thickness_mm: 500.0, join_pick: [true; 2],
            camera_yaw: 0.7, camera_pitch: 0.6,
            view3d: openbim_viewport_egui::Viewport3d::new(), view3d_revision: None, site_edit: None,
            property_query: String::new(), browser_query: String::new(),
            browser_selected: String::new(),
            message: "Sample scene - use Save to keep your edits".into(),
            show_properties: true, show_browser: true, show_warnings: false, file: file::FileState { saved_revision: document_revision, ..Default::default() }, types_dialog: None, end_draft: Default::default(), compute: Default::default(), elec_draft: Default::default(), elec: Default::default(), cable_from: None, cable_via: Vec::new(), export_dialog: None, import_dialog: None, type_name_input: String::new(), warnings_cache: (u64::MAX, Vec::new()), show_help: false, dark: false,
            zoom: 1.0, pan: Vec2::ZERO, keymap,
        }
    }

    fn choose_tool(&mut self, tool: Tool) {
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
                if ui.add(egui::Button::image(img).frame(false)).on_hover_text(*label).clicked() {
                    self.run_command(id, label);
                }
            }
            ui.separator();
            ui.label("Project1 (SAMPLE)");
            ui.separator();
            for (label, ws) in [
                ("Design", Workspace::Design), ("Coordination", Workspace::Coordination),
                ("Automation", Workspace::Automation),
            ] {
                if ui.selectable_label(self.workspace == ws, label).clicked() {
                    self.workspace = ws;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if self.dark { "Light" } else { "Dark" }).clicked() {
                    self.dark = !self.dark;
                    ui.ctx().set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                }
                if ui.button("Help").clicked() { self.show_help = !self.show_help; }
            });
        });
    }

    /// Commands without an implementation yet report it in the status bar.
    fn run_command(&mut self, id: &str, label: &str) {
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
            "duct" => { (self.width_mm, self.height_mm, self.elevation_mm) = (400.0, 300.0, 2700.0); self.choose_tool(Tool::Duct) }
            "pipe" => { (self.width_mm, self.height_mm, self.elevation_mm) = (100.0, 100.0, 600.0); self.system_pick = openbim_family::mep::System::ChilledWaterSupply; self.choose_tool(Tool::Pipe) }
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
            "rotate" => self.choose_tool(Tool::Rotate),
            "mirror" => self.choose_tool(Tool::Mirror),
            "delete" => self.delete_selected(),
            "level" => { self.level_elev_mm = self.levels().last().map_or(0.0, |l| l.2 * 1000.0 + 3000.0); self.level_name.clear(); self.choose_tool(Tool::Level) }
            "wall" => { (self.width_mm, self.height_mm, self.elevation_mm) = (200.0, 3000.0, 0.0); self.wall_type = None; self.wall_flip = false; self.choose_tool(Tool::Wall) }
            "beam" => { self.usage_pick = openbim_document::Usage::Girder; self.choose_tool(Tool::Beam) }
            "structural_column" | "column" => self.choose_tool(Tool::Column),
            "structural_floor" | "floor" => { self.slab_kind_pick = openbim_document::SlabKind::Floor; self.choose_tool(Tool::Slab) }
            "elevation" => { self.edit_pts.clear(); self.choose_tool(Tool::Elevation) }
            "railing" => { (self.height_mm, self.width_mm, self.spacing_mm, self.elevation_mm) = (1100.0, 50.0, 150.0, 0.0); self.choose_tool(Tool::Railing) }
            "section" => { self.edit_pts.clear(); self.choose_tool(Tool::Section) }
            "shaft" => { (self.shaft_base_mm, self.shaft_top_mm, self.shaft_top) = (0.0, 0.0, None); self.choose_tool(Tool::Shaft) }
            "opening_wall" => self.choose_tool(Tool::WallOpening),
            "stair" => { self.elevation_mm = 0.0; self.edit_pts.clear(); self.choose_tool(Tool::Stair) }
            "ceiling" => { (self.slab_kind_pick, self.slab_thickness_mm, self.slab_offset_mm, self.material_pick) = (openbim_document::SlabKind::Ceiling, 20.0, 2700.0, openbim_document::Material::Unspecified); self.choose_tool(Tool::Slab) }
            "roof" => { (self.slab_kind_pick, self.slab_thickness_mm, self.slab_offset_mm) = (openbim_document::SlabKind::Roof, 200.0, 0.0); self.choose_tool(Tool::Slab) }
            "footing" => self.choose_tool(Tool::WallFooting),
            "truss" => self.choose_tool(Tool::Truss),
            "door" => { self.family_pick = "Door, single flush".into(); self.family_type.clear(); self.choose_tool(Tool::Family) }
            "window" => { self.family_pick = "Window, fixed with mullions".into(); self.family_type.clear(); self.choose_tool(Tool::Family) }
            "ahu" | "component" => { self.family_pick = "Fan coil unit, ducted".into(); self.family_type.clear(); self.choose_tool(Tool::Family) }
            "rebar" => { (self.width_mm, self.height_mm, self.elevation_mm, self.spacing_mm, self.bar_count) = (16.0, 8.0, 40.0, 200.0, 3.0); self.choose_tool(Tool::Rebar) }
            "beam_system" => { self.usage_pick = openbim_document::Usage::Joist; self.choose_tool(Tool::BeamSystem) }
            "foundation" => self.choose_tool(Tool::Footing),
            "slab" => { self.slab_kind_pick = openbim_document::SlabKind::Foundation; self.choose_tool(Tool::Slab) }
            "brace" => { self.usage_pick = openbim_document::Usage::Brace; self.choose_tool(Tool::Beam) }
            "select" => self.choose_tool(Tool::Select),
            "ifc_export" => self.export_ifc_file(),
            "ifc_import" => self.import_ifc_file(),
            "new" => self.new_model(),
            "open" => self.open_file(),
            "save" => self.save_file(false),
            "save_as" => self.save_file(true),
            "project_parameters" => self.types_dialog = Some(model_edit::TypesDialog::default()),
            "undo" => self.undo_document(),
            "zoom_in" => self.zoom = (self.zoom * 1.25).min(4.0),
            "zoom_out" => self.zoom = (self.zoom / 1.25).max(0.3),
            _ => self.message = format!("{label} is not implemented in this UI prototype"),
        }
    }

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        // Ribbon tabs, each with a small icon.
        ui.horizontal(|ui| {
            for (tab, icon) in ribbon::TABS {
                let img = ribbon::icon_image(ui, icon, self.dark, Vec2::splat(16.0));
                if ui.add(egui::Button::image_and_text(img, *tab).selected(self.ribbon == *tab).frame(false)).clicked() {
                    self.ribbon = tab;
                }
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
        egui::ScrollArea::horizontal().id_salt("ribbon").show(ui, |ui| {
            ui.horizontal(|ui| {
                for group in groups {
                    ribbon::panel(ui, group.title, |ui| {
                        for cmd in group.commands {
                            let roofing = self.tool == Tool::Slab && matches!(self.slab_kind_pick, openbim_document::SlabKind::Roof | openbim_document::SlabKind::Ceiling);
                            let active = (cmd.id == "roof" && roofing && self.slab_kind_pick == openbim_document::SlabKind::Roof) || (cmd.id == "ceiling" && roofing && self.slab_kind_pick == openbim_document::SlabKind::Ceiling) || (!matches!(cmd.id, "roof" | "ceiling") && !(roofing && matches!(cmd.id, "structural_floor" | "floor" | "slab")) && matches!((cmd.id, self.tool),
                                ("duct", Tool::Duct) | ("pipe", Tool::Pipe) | ("end_point", Tool::EndPoint) | ("elec_node", Tool::ElecNode) | ("elec_cable", Tool::Cable) | ("wall", Tool::Wall) | ("grid", Tool::Grid) | ("tag", Tool::Tag) | ("move", Tool::Move) | ("dimension", Tool::Dimension) | ("wall_joins", Tool::WallJoins) | ("room", Tool::Room) | ("stair", Tool::Stair) | ("railing", Tool::Railing) | ("section", Tool::Section) | ("elevation", Tool::Elevation) | ("shaft", Tool::Shaft) | ("opening_wall", Tool::WallOpening) | ("measure", Tool::Measure) | ("align", Tool::Align) | ("trim", Tool::Trim) | ("offset", Tool::Offset) | ("copy", Tool::Copy) | ("rotate", Tool::Rotate) | ("mirror", Tool::Mirror) | ("level", Tool::Level) | ("beam" | "brace", Tool::Beam) | ("structural_column", Tool::Column) | ("structural_floor" | "floor" | "slab", Tool::Slab) | ("foundation", Tool::Footing) | ("footing", Tool::WallFooting) | ("beam_system", Tool::BeamSystem) | ("truss", Tool::Truss) | ("rebar", Tool::Rebar) | ("door" | "window" | "ahu" | "component", Tool::Family)));
                            if ribbon::big_button(ui, cmd.id, cmd.label, self.dark, active).clicked() {
                                clicked = Some((cmd.id, cmd.label));
                            }
                        }
                    });
                }
            });
        });
        if let Some((id, label)) = clicked { self.run_command(id, label); }
        ui.separator();
        panels::options(self, ui);
    }

    fn viewport(&mut self, ui: &mut egui::Ui) { model_view::show(self, ui); }
}

impl Desktop {
    fn undo_document(&mut self) {
        match self.document.undo() {
            Ok(true) => { self.sync_duct_controls(); self.message = "Undid last document transaction".into(); },
            Ok(false) => self.message = "Nothing to undo".into(),
            Err(err) => self.message = format!("Undo rejected: {err:?}"),
        }
    }
}

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.choose_tool(Tool::Select); }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
            let ids: Vec<_> = self.document.elements().map(|el| el.id).collect();
            if !ids.is_empty() {
                let next = self.selected_id().and_then(|id| ids.iter().position(|candidate| *candidate == id))
                    .map_or(0, |index| (index + 1) % ids.len());
                self.select_element(Some(ids[next]));
            }
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            self.undo_document();
        }
        egui::Panel::top("header").show(ui, |ui| {
            self.toolbar(ui);
            self.ribbon(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.message);
                ui.separator();
                ui.label("Orthographic / fit to view");
                ui.separator();
                ui.label("Workset: Mechanical (sample)");
                ui.separator();
                let name = self.file.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).and_then(|n| n.to_str()).unwrap_or("not saved yet");
                ui.label(format!("BIM revision {} | {name}{}", self.document.revision(), if self.is_dirty() { " | unsaved changes" } else { "" }));
                ui.separator();
                let n = self.warnings().len();
                let label = if n == 0 { "No warnings".to_string() } else { format!("⚠ {n} warning{}", if n == 1 { "" } else { "s" }) };
                if ui.selectable_label(self.show_warnings, label).clicked() { self.show_warnings = !self.show_warnings; }
                ui.separator();
                ui.checkbox(&mut self.show_properties, "Properties");
                ui.checkbox(&mut self.show_browser, "Browser");
            });
        });
        self.warnings_window(&ctx);
        self.export_window(&ctx);
        self.import_window(&ctx);
        self.types_window(&ctx);
        self.compute_window(&ctx);
        self.elec_window(&ctx);
        self.file_window(&ctx);
        if self.show_properties || self.show_browser {
            egui::Panel::left("side").default_size(310.0).min_size(260.0)
                .max_size(520.0).resizable(true).show(ui, |ui| panels::sidebar(self, ui));
        }
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
                ui.label("ESC: cancel. TAB: cycle sample selection. Mouse wheel: zoom. Middle-drag: pan. Right-drag: orbit in 3D.");
                ui.label(format!("Existing keymap crate: Duct shortcut = {:?}",
                    self.keymap.shortcuts("Duct")));
            });
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default()
        .with_title("OpenBIM - Revit-style Desktop Prototype")
        .with_inner_size([1440.0, 850.0])
        .with_min_inner_size([900.0, 550.0]), ..Default::default() };
    eframe::run_native("OpenBIM Desktop Prototype", options,
        Box::new(|cc| Ok(Box::new(Desktop::new(cc)))))
}
