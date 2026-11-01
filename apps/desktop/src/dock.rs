//! Lightweight dock for the side panels (Properties, Browser, Warnings), built on egui's native
//! [`egui::Panel`]. Each panel can be pinned to any screen edge, shown or hidden, resized by dragging
//! its splitter, and moved to another edge from its title-bar context menu. The layout is persisted in
//! `app_settings`. This is not a full floating docking system; it gives every side panel a home on any
//! edge with a saved, resizable arrangement, using only egui's own panels.

use crate::app_settings::DockEdge;
use crate::panels;
use eframe::egui;

/// The three dockable side panels, in a fixed order shared with the settings arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SidePanel {
    Properties,
    Browser,
    Warnings,
}

impl SidePanel {
    pub(crate) const ALL: [SidePanel; 3] = [SidePanel::Properties, SidePanel::Browser, SidePanel::Warnings];

    pub(crate) fn title(self) -> &'static str {
        match self {
            SidePanel::Properties => "Properties",
            SidePanel::Browser => "Browser",
            SidePanel::Warnings => "Warnings",
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            SidePanel::Properties => 0,
            SidePanel::Browser => 1,
            SidePanel::Warnings => 2,
        }
    }

    /// The edges offered in the panel's context menu, excluding the one it is already on.
    fn edge_options(self, current: DockEdge) -> Vec<DockEdge> {
        [DockEdge::Left, DockEdge::Right, DockEdge::Top, DockEdge::Bottom]
            .into_iter()
            .filter(|e| *e != current)
            .collect()
    }

    fn edge_menu(self, app: &mut crate::Desktop, ui: &mut egui::Ui) {
        let current = app.settings.panel_edges[self.index()];
        ui.label("Dock to:");
        ui.separator();
        for edge in self.edge_options(current) {
            let label = match edge {
                DockEdge::Left => "Left",
                DockEdge::Right => "Right",
                DockEdge::Top => "Top",
                DockEdge::Bottom => "Bottom",
            };
            if ui.selectable_label(false, label).clicked() {
                app.settings.panel_edges[self.index()] = edge;
                let _ = app.settings.save();
            }
        }
    }

    fn id(self) -> egui::Id { egui::Id::new(("dock_panel", self.title())) }
}

/// Render one panel's content. The dock supplies the title bar, so the body variants are used.
fn panel_content(app: &mut crate::Desktop, ui: &mut egui::Ui, panel: SidePanel) {
    match panel {
        SidePanel::Properties => panels::properties_body(app, ui),
        SidePanel::Browser => panels::browser_body(app, ui),
        SidePanel::Warnings => panels::warnings(app, ui),
    }
}

/// The whole dock: group the visible panels by edge, then render one egui panel per occupied edge.
/// Inside each edge panel the panels are stacked with resizable splitters (vertically for Left/Right,
/// horizontally for Top/Bottom).
pub(crate) fn render(app: &mut crate::Desktop, ui: &mut egui::Ui) {
    let mut by_edge: [Vec<SidePanel>; 4] = Default::default();
    for panel in SidePanel::ALL {
        if !app.settings.panel_visible[panel.index()] { continue; }
        by_edge[edge_index(app.settings.panel_edges[panel.index()])].push(panel);
    }
    // Left and Right take full height; render them before Top/Bottom so the latter fit in the gap.
    render_edge(app, ui, DockEdge::Left, &by_edge[0]);
    render_edge(app, ui, DockEdge::Right, &by_edge[1]);
    render_edge(app, ui, DockEdge::Top, &by_edge[2]);
    render_edge(app, ui, DockEdge::Bottom, &by_edge[3]);
}

fn edge_index(edge: DockEdge) -> usize {
    match edge { DockEdge::Left => 0, DockEdge::Right => 1, DockEdge::Top => 2, DockEdge::Bottom => 3 }
}

/// The egui panel constructor for an edge.
fn panel_for_edge(edge: DockEdge, id: impl egui::AsIdSalt) -> egui::Panel {
    match edge {
        DockEdge::Left => egui::Panel::left(id),
        DockEdge::Right => egui::Panel::right(id),
        DockEdge::Top => egui::Panel::top(id),
        DockEdge::Bottom => egui::Panel::bottom(id),
    }
}

/// Render the panels pinned to one edge as a stacked, resizable group inside a single outer egui panel.
fn render_edge(app: &mut crate::Desktop, ui: &mut egui::Ui, edge: DockEdge, list: &[SidePanel]) {
    if list.is_empty() { return; }
    let size = app.settings.edge_sizes[edge_index(edge)];
    let outer = panel_for_edge(edge, "dock_edge").default_size(size).resizable(list.len() == 1).min_size(160.0);
    outer.show(ui, |ui| {
        // Within an edge panel, stack the groups along the edge's length: vertically for Left/Right,
        // horizontally for Top/Bottom.
        let vertical = matches!(edge, DockEdge::Left | DockEdge::Right);
        for (i, &panel) in list.iter().enumerate() {
            let last = i + 1 == list.len();
            // A slim title bar with the panel name; right-click opens the edge menu.
            let title = ui.horizontal(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(panel.title()).strong()).selectable(false));
            }).response;
            title.context_menu(|ui| panel.edge_menu(app, ui));
            // The content fills the rest of this group's slice of the edge.
            if last {
                egui::CentralPanel::default().frame(egui::Frame::group(ui.style()))
                    .show(ui, |ui| panel_content(app, ui, panel));
            } else if vertical {
                egui::Panel::top(panel.id()).resizable(true).min_size(80.0)
                    .show(ui, |ui| panel_content(app, ui, panel));
            } else {
                egui::Panel::left(panel.id()).resizable(true).min_size(80.0)
                    .show(ui, |ui| panel_content(app, ui, panel));
            }
        }
    });
}

/// Toggles for the three panels, shown in the status bar so a panel can be restored if hidden everywhere.
pub(crate) fn status_toggles(app: &mut crate::Desktop, ui: &mut egui::Ui) {
    for panel in SidePanel::ALL {
        let mut shown = app.settings.panel_visible[panel.index()];
        if ui.checkbox(&mut shown, panel.title()).changed() {
            app.settings.panel_visible[panel.index()] = shown;
            let _ = app.settings.save();
        }
    }
}
