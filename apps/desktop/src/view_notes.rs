//! Tags and dimensions in an elevation or section view. As in Revit they belong to the view they are drawn in: another
//! view of the same building does not show them. A tag says what its element is and goes with it when the element is
//! moved in the view; a dimension measures between two vertical (or two horizontal) edges of what is drawn.
use crate::section_ui::{pick_at, Xf};
use crate::view_tools::{orientation, pick_edge, Edge, ViewEdit};
use crate::{Desktop, Tool, View};
use eframe::egui::{self, Pos2, Rect, Stroke, Vec2};
use openbim_document::{Category, ElementId, Geometry, ViewEdge, ViewRef};
use openbim_view::SectionView;

/// The place of a view in a file: which view of the building a tag or a dimension belongs to.
pub fn view_ref(view: View) -> Option<ViewRef> {
    match view {
        View::Elevation(k) => Some(ViewRef::Elevation(k)),
        View::Section(id) => Some(ViewRef::Section(ElementId(id))),
        View::Marked(id, k) => Some(ViewRef::Marked(ElementId(id), k)),
        _ => None,
    }
}

/// Where a drawn tag or dimension can be picked.
pub enum Hit { Box(Rect), Line(Pos2, Pos2) }

impl Hit {
    pub fn near(&self, p: Pos2, reach: f32) -> bool {
        match self {
            Hit::Box(r) => r.expand(reach).contains(p),
            Hit::Line(a, b) => {
                let ab = *b - *a;
                let t = if ab.length_sq() > 0.0 { ((p - *a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                (*a + ab * t).distance(p) <= reach
            }
        }
    }
}

/// Draws the tags and dimensions of the view `view`; the places they can be picked at are returned.
pub fn draw(app: &Desktop, ui: &egui::Ui, painter: &egui::Painter, xf: &Xf, view: ViewRef) -> Vec<(ElementId, Hit)> {
    let ink = ui.visuals().text_color();
    let blue = ui.visuals().hyperlink_color;
    let chosen = app.selected_ids();
    let mut hits = Vec::new();
    for el in app.document.elements() {
        let colour = if chosen.contains(&el.id) { blue } else { ink };
        match el.geometry {
            Some(Geometry::ViewTag { host, view: v, at, anchor }) if v == view && app.document.get(host).is_some() => {
                let (head, tip) = (xf.screen(at), xf.screen(anchor));
                if head.distance(tip) > 2.0 { painter.line_segment([head, tip], Stroke::new(1.0, colour)); painter.circle_filled(tip, 2.5, colour); }
                let galley = painter.layout_no_wrap(app.document.tag_text(el.id), egui::FontId::proportional(12.0), colour);
                let r = Rect::from_center_size(head, galley.size() + Vec2::new(10.0, 6.0));
                painter.rect_filled(r, 2.0, ui.visuals().panel_fill);
                painter.rect_stroke(r, 2.0, Stroke::new(1.0, colour), egui::StrokeKind::Middle);
                painter.galley(r.center() - galley.size() / 2.0, galley, colour);
                hits.push((el.id, Hit::Box(r)));
            }
            Some(Geometry::ViewDimension { view: v, a, b, at }) if v == view => {
                let horizontal = a[1] == b[1];
                let (qa, qb) = if horizontal { ([a[0], at[1]], [b[0], at[1]]) } else { ([at[0], a[1]], [at[0], b[1]]) };
                let length = if horizontal { (b[0] - a[0]).abs() } else { (b[1] - a[1]).abs() };
                let (q1, q2) = (xf.screen(qa), xf.screen(qb));
                crate::model_view::draw_dimension(painter, xf.screen(a), xf.screen(b), q1, q2, &format!("{:.0}", length * 1000.0), colour);
                hits.push((el.id, Hit::Line(q1, q2)));
            }
            _ => {}
        }
    }
    hits
}

/// The tag or dimension under `p`, if any.
pub fn pick(hits: &[(ElementId, Hit)], p: Pos2) -> Option<ElementId> {
    hits.iter().rev().find(|(_, h)| h.near(p, 5.0)).map(|(id, _)| *id)
}

impl Desktop {
    /// A click of the Tag tool in a view: the tag of the element under the pointer, with a leader when that is on.
    pub(crate) fn view_tag_click(&mut self, view: ViewRef, host: ElementId, at: [f64; 2]) {
        let head = if self.tag_leader { [at[0] + 0.6, at[1] + 0.6] } else { at };
        match self.document.place_view_tag(host, view, head, at) {
            Ok(id) => self.message = format!("Tagged {} in this view only; undo removes the tag", self.document.tag_text(id)),
            Err(_) => self.message = "Tag: click a building element (walls, doors, windows, ducts, ...)".into(),
        }
    }

    /// A pick of the Dimension tool in a view. The first two picks are edges of the same direction (two vertical edges
    /// give a horizontal dimension, two horizontal ones a vertical one); the third is where the dimension line goes.
    pub(crate) fn view_dimension_click(&mut self, view: ViewRef, edge: Option<(ViewEdge, [f64; 2])>, at: [f64; 2]) {
        let Some(st) = self.view_edit.clone() else { return };
        match (st.pts.len(), edge) {
            (0, Some((e, p))) => {
                let z_or_u = match e { ViewEdge::Vertical(_) => p[1], ViewEdge::Horizontal(_) => p[0] };
                let v = match e { ViewEdge::Vertical(u) => u, ViewEdge::Horizontal(z) => z };
                if let Some(s) = self.view_edit.as_mut() { s.pts = vec![[v, z_or_u]]; s.edge = Some(e); }
                self.message = "Dimension: pick the second edge, parallel to the first".into();
            }
            (1, Some((e, _))) => {
                let same = matches!((st.edge, e), (Some(ViewEdge::Vertical(_)), ViewEdge::Vertical(_)) | (Some(ViewEdge::Horizontal(_)), ViewEdge::Horizontal(_)));
                let v = match e { ViewEdge::Vertical(u) => u, ViewEdge::Horizontal(z) => z };
                if !same { self.message = "Dimension: the second edge must run the same way as the first".into(); }
                else if (v - st.pts[0][0]).abs() < 0.001 { self.message = "Dimension: pick a different edge".into(); }
                else {
                    if let Some(s) = self.view_edit.as_mut() { s.pts.push([v, 0.0]); }
                    self.message = "Dimension: pick where the dimension line goes".into();
                }
            }
            (0 | 1, None) => self.message = "Dimension: pick a vertical or a horizontal edge".into(),
            (2, _) => {
                let (first, second) = (st.pts[0], st.pts[1]);
                let (a, b) = match st.edge {
                    Some(ViewEdge::Vertical(_)) => ([first[0], first[1]], [second[0], first[1]]),
                    _ => ([first[1], first[0]], [first[1], second[0]]),
                };
                match self.document.place_view_dimension(view, a, b, at) {
                    Ok(_) => {
                        let mm = ((b[0] - a[0]).abs() + (b[1] - a[1]).abs()) * 1000.0;
                        if let Some(s) = self.view_edit.as_mut() { s.pts.clear(); s.edge = None; }
                        self.message = format!("Dimension {mm:.0} mm placed in this view only; undo removes it");
                    }
                    Err(e) => self.message = format!("Dimension rejected: {e:?}"),
                }
            }
            _ => {}
        }
    }
}

/// The Tag and Dimension tools in a view.
pub fn tool(app: &mut Desktop, ui: &egui::Ui, painter: &egui::Painter, response: &egui::Response, view: &SectionView, xf: Xf, edges: &[Edge]) {
    let Some(vr) = view_ref(app.view) else { return };
    let blue = ui.visuals().hyperlink_color;
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if app.view_edit.as_ref().is_none_or(|e| e.view != app.view) { app.view_edit = Some(ViewEdit::new(app.view)); }
    let click = response.clicked().then(|| response.interact_pointer_pos()).flatten();
    if app.tool == Tool::Tag {
        if esc { app.choose_tool(Tool::Select); return; }
        if let Some(click) = click {
            let at = xf.model(click);
            match pick_at(&app.document, view, at) {
                Some(host) if app.document.get(host).is_some_and(|e| !matches!(e.category, Category::Level | Category::Grid)) => app.view_tag_click(vr, host, at),
                _ => app.message = "Tag: click an element in this view".into(),
            }
        }
        return;
    }
    // Dimension: edges of the same direction, the second one only of the direction of the first
    let st = app.view_edit.clone().unwrap_or_else(|| ViewEdit::new(app.view));
    let want = |e: &Edge| match (st.edge, orientation(e)) {
        (_, None) => false,
        (None, _) => true,
        (Some(ViewEdge::Vertical(_)), Some(ViewEdge::Vertical(_))) | (Some(ViewEdge::Horizontal(_)), Some(ViewEdge::Horizontal(_))) => true,
        _ => false,
    };
    if esc {
        if st.pts.is_empty() { app.choose_tool(Tool::Select); } else if let Some(s) = app.view_edit.as_mut() { s.pts.clear(); s.edge = None; }
        return;
    }
    if st.pts.len() < 2 {
        if let Some(p) = response.hover_pos() {
            if let Some((e, _)) = pick_edge(edges, &xf, p, 7.0, want) { painter.line_segment([xf.screen(e.a), xf.screen(e.b)], Stroke::new(3.0, blue.gamma_multiply(0.7))); }
        }
    }
    // the references already picked, and the line that follows the pointer before it is placed
    if let (Some(first), Some(e)) = (st.pts.first(), st.edge) {
        let (p, q) = match e { ViewEdge::Vertical(_) => ([first[0], -1e3], [first[0], 1e3]), ViewEdge::Horizontal(_) => ([-1e3, first[0]], [1e3, first[0]]) };
        painter.line_segment([xf.screen(p), xf.screen(q)], Stroke::new(1.0, blue.gamma_multiply(0.5)));
    }
    if st.pts.len() == 2 {
        if let (Some(h), Some(e)) = (response.hover_pos(), st.edge) {
            let m = xf.model(h);
            let (first, second) = (st.pts[0], st.pts[1]);
            let (a, b, qa, qb) = match e {
                ViewEdge::Vertical(_) => ([first[0], first[1]], [second[0], first[1]], [first[0], m[1]], [second[0], m[1]]),
                ViewEdge::Horizontal(_) => ([first[1], first[0]], [first[1], second[0]], [m[0], first[0]], [m[0], second[0]]),
            };
            crate::model_view::draw_dimension(painter, xf.screen(a), xf.screen(b), xf.screen(qa), xf.screen(qb), &format!("{:.0}", ((b[0] - a[0]).abs() + (b[1] - a[1]).abs()) * 1000.0), blue);
        }
    }
    if let Some(click) = click {
        let at = xf.model(click);
        if st.pts.len() == 2 { app.view_dimension_click(vr, None, [(at[0] / 0.005).round() * 0.005, (at[1] / 0.005).round() * 0.005]); }
        else {
            let picked = pick_edge(edges, &xf, click, 7.0, want).map(|(e, o)| {
                // where along the edge it was clicked, kept within the edge, so that the extension line starts on it
                let p = match o {
                    ViewEdge::Vertical(_) => [e.a[0], at[1].clamp(e.a[1].min(e.b[1]), e.a[1].max(e.b[1]))],
                    ViewEdge::Horizontal(_) => [at[0].clamp(e.a[0].min(e.b[0]), e.a[0].max(e.b[0])), e.a[1]],
                };
                (o, p)
            });
            app.view_dimension_click(vr, picked, at);
        }
    }
}
