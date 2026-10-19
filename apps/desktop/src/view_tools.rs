//! Snaps and the Modify tools in an elevation or section view, as Revit has them there: the pointer snaps to the
//! endpoints, intersections, midpoints and nearest points of what is drawn (else to an increment that follows the
//! zoom); Move and Copy take a base point and a second point or a typed distance (Shift keeps the move horizontal or
//! vertical); Rotate turns about the middle of the element by two rays or a typed angle; Mirror takes a vertical axis;
//! Align takes a reference edge and then the edge to move onto it.
use crate::section_ui::{opening_rects, Xf};
use crate::{Desktop, Tool, View};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use openbim_document::{ElementId, ViewEdge, ViewFrame};
use openbim_view::{Class, Kind, SectionView, Shape};

/// What a snap caught, in the order Revit prefers them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapKind { Endpoint, Intersection, Midpoint, Nearest, Grid }

impl SnapKind {
    pub fn name(self) -> &'static str {
        match self { SnapKind::Endpoint => "Endpoint", SnapKind::Intersection => "Intersection", SnapKind::Midpoint => "Midpoint", SnapKind::Nearest => "Nearest", SnapKind::Grid => "" }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Snap { pub at: [f64; 2], pub kind: SnapKind }

/// A straight edge drawn in the view: its two ends, the element it belongs to, and whether it is a level line.
#[derive(Clone, Copy, Debug)]
pub struct Edge { pub a: [f64; 2], pub b: [f64; 2], pub id: Option<ElementId>, pub datum: bool }

/// The state of a Modify tool in a view: the points picked so far, the reference edge of Align, and the elements the
/// tool works on (kept while Copy goes on).
#[derive(Clone)]
pub struct ViewEdit { pub view: View, pub pts: Vec<[f64; 2]>, pub edge: Option<ViewEdge>, pub src: Vec<ElementId> }

impl ViewEdit {
    pub fn new(view: View) -> ViewEdit { ViewEdit { view, pts: Vec::new(), edge: None, src: Vec::new() } }
}

/// Every straight edge of what is drawn: the openings first (so that they win a tie with the hole they make in a wall).
pub fn edges(doc: &openbim_document::Document, view: &SectionView) -> Vec<Edge> {
    let mut out = Vec::new();
    for (id, _, r) in opening_rects(doc, view) {
        let c = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]];
        for i in 0..4 { out.push(Edge { a: c[i], b: c[(i + 1) % 4], id: Some(id), datum: false }); }
    }
    let ring = |out: &mut Vec<Edge>, r: &[[f64; 2]], id: Option<ElementId>, datum: bool| {
        for i in 0..r.len() { out.push(Edge { a: r[i], b: r[(i + 1) % r.len()], id, datum }); }
    };
    for item in &view.items {
        let datum = item.class == Class::Datum || item.element.kind == Kind::Level;
        let id = (!datum).then_some(ElementId(item.element.id)).filter(|i| doc.get(*i).is_some());
        if !datum && id.is_none() { continue; }
        match &item.shape {
            Shape::Region { outer, holes } => { ring(&mut out, outer, id, datum); for h in holes { ring(&mut out, h, id, datum); } }
            Shape::Fill(r) => ring(&mut out, r, id, datum),
            Shape::Line(a, b) => out.push(Edge { a: *a, b: *b, id, datum }),
            _ => {}
        }
    }
    out
}

/// The increment the pointer snaps to when nothing is caught: smaller as the view is zoomed in, as in Revit.
pub fn grid_step(px_per_m: f32) -> f64 {
    [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0, 10.0].into_iter().find(|s| *s as f32 * px_per_m >= 6.0).unwrap_or(10.0)
}

fn seg_point(p: Pos2, a: Pos2, b: Pos2) -> (Pos2, f32) {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-10)).clamp(0.0, 1.0);
    let q = a + t * ab;
    (q, (p - q).length())
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 { a[0] * b[1] - a[1] * b[0] }

/// The point where two edges cross, if they do.
fn meet(e: &Edge, f: &Edge) -> Option<[f64; 2]> {
    let (r, s) = ([e.b[0] - e.a[0], e.b[1] - e.a[1]], [f.b[0] - f.a[0], f.b[1] - f.a[1]]);
    let d = cross(r, s);
    if d.abs() < 1e-12 { return None; }
    let q = [f.a[0] - e.a[0], f.a[1] - e.a[1]];
    let (t, u) = (cross(q, s) / d, cross(q, r) / d);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| [e.a[0] + r[0] * t, e.a[1] + r[1] * t])
}

/// Where the pointer at `cursor` snaps: an endpoint, an intersection, a midpoint or the nearest point of an edge within
/// `reach` pixels, else the increment of the zoom. `base` with `ortho` keeps the point level or plumb with it (Shift).
pub fn snap(edges: &[Edge], xf: &Xf, cursor: Pos2, reach: f32, base: Option<[f64; 2]>, ortho: bool) -> Snap {
    let step = grid_step(xf.scale);
    let mut m = xf.model(cursor);
    m = [(m[0] / step).round() * step, (m[1] / step).round() * step];
    if let (true, Some(b)) = (ortho, base) {
        if (m[0] - b[0]).abs() >= (m[1] - b[1]).abs() { m[1] = b[1]; } else { m[0] = b[0]; }
        return Snap { at: m, kind: SnapKind::Grid };
    }
    let mut best: Option<(u8, f32, Snap)> = None;
    let mut offer = |rank: u8, d: f32, at: [f64; 2], kind: SnapKind| {
        if d <= reach && best.as_ref().is_none_or(|b| (rank, d) < (b.0, b.1)) { best = Some((rank, d, Snap { at, kind })); }
    };
    let mut near: Vec<&Edge> = Vec::new();
    for e in edges {
        let (pa, pb) = (xf.screen(e.a), xf.screen(e.b));
        offer(0, (cursor - pa).length(), e.a, SnapKind::Endpoint);
        offer(0, (cursor - pb).length(), e.b, SnapKind::Endpoint);
        let mid = [(e.a[0] + e.b[0]) / 2.0, (e.a[1] + e.b[1]) / 2.0];
        offer(2, (cursor - xf.screen(mid)).length(), mid, SnapKind::Midpoint);
        let (q, d) = seg_point(cursor, pa, pb);
        if d <= 6.0 { offer(3, d, xf.model(q), SnapKind::Nearest); }
        if d <= reach * 3.0 && near.len() < 40 { near.push(e); }
    }
    for i in 0..near.len() {
        for j in i + 1..near.len() {
            if let Some(p) = meet(near[i], near[j]) { offer(1, (cursor - xf.screen(p)).length(), p, SnapKind::Intersection); }
        }
    }
    best.map(|b| b.2).unwrap_or(Snap { at: m, kind: SnapKind::Grid })
}

/// The marker of a snap at the screen point `p` and its name, as Revit draws them.
pub fn draw_snap(painter: &egui::Painter, p: Pos2, kind: SnapKind, colour: Color32) {
    let st = Stroke::new(1.5, colour);
    match kind {
        SnapKind::Endpoint => { painter.rect_stroke(egui::Rect::from_center_size(p, Vec2::splat(9.0)), 0.0, st, egui::StrokeKind::Middle); }
        SnapKind::Midpoint => { painter.add(egui::Shape::closed_line(vec![p + Vec2::new(0.0, -5.0), p + Vec2::new(5.0, 4.0), p + Vec2::new(-5.0, 4.0)], st)); }
        SnapKind::Intersection => { painter.line_segment([p + Vec2::new(-5.0, -5.0), p + Vec2::new(5.0, 5.0)], st); painter.line_segment([p + Vec2::new(-5.0, 5.0), p + Vec2::new(5.0, -5.0)], st); }
        SnapKind::Nearest => { painter.line_segment([p + Vec2::new(-5.0, -5.0), p + Vec2::new(5.0, -5.0)], st); painter.line_segment([p + Vec2::new(5.0, -5.0), p + Vec2::new(-5.0, 5.0)], st); painter.line_segment([p + Vec2::new(-5.0, 5.0), p + Vec2::new(5.0, 5.0)], st); }
        SnapKind::Grid => return,
    }
    painter.text(p + Vec2::new(10.0, 10.0), egui::Align2::LEFT_TOP, kind.name(), egui::FontId::proportional(11.0), colour);
}

/// The ghost of `ids` moved by (`du`, `dz`): their outlines in orange.
pub fn draw_ghost(painter: &egui::Painter, view: &SectionView, doc: &openbim_document::Document, xf: &Xf, ids: &[ElementId], du: f64, dz: f64) {
    let ghost = Stroke::new(1.5, Color32::from_rgb(220, 140, 0));
    let shift = |p: &[f64; 2]| xf.screen([p[0] + du, p[1] + dz]);
    for item in view.items.iter().filter(|i| i.class != Class::Datum && ids.contains(&ElementId(i.element.id)) && i.element.kind != Kind::Level) {
        match &item.shape {
            Shape::Region { outer, holes } => for r in std::iter::once(outer).chain(holes.iter()) { painter.add(egui::Shape::closed_line(r.iter().map(shift).collect(), ghost)); },
            Shape::Fill(r) => { painter.add(egui::Shape::closed_line(r.iter().map(shift).collect(), ghost)); }
            Shape::Line(a, b) => { painter.line_segment([shift(a), shift(b)], ghost); }
            _ => {}
        }
    }
    for (_, _, r) in opening_rects(doc, view).iter().filter(|(id, _, _)| ids.contains(id)) {
        painter.rect_stroke(egui::Rect::from_two_pos(shift(&[r[0], r[1]]), shift(&[r[2], r[3]])), 0.0, ghost, egui::StrokeKind::Middle);
    }
}

/// The edge of the view nearest to `p` within `reach` pixels, if it is vertical or horizontal; the edges to leave out
/// (level lines when picking what to move) are dropped by `want`.
pub fn pick_edge(edges: &[Edge], xf: &Xf, p: Pos2, reach: f32, want: impl Fn(&Edge) -> bool) -> Option<(Edge, ViewEdge)> {
    let mut best: Option<(f32, Edge)> = None;
    for e in edges.iter().filter(|e| want(e)) {
        let d = seg_point(p, xf.screen(e.a), xf.screen(e.b)).1;
        if d <= reach && best.as_ref().is_none_or(|b| d < b.0) { best = Some((d, *e)); }
    }
    let (_, e) = best?;
    orientation(&e).map(|o| (e, o))
}

/// Whether an edge is vertical or horizontal, and where.
pub fn orientation(e: &Edge) -> Option<ViewEdge> {
    let (dx, dy) = ((e.b[0] - e.a[0]).abs(), (e.b[1] - e.a[1]).abs());
    if dx <= 1e-6 + 0.01 * dy { Some(ViewEdge::Vertical(e.a[0])) }
    else if dy <= 1e-6 + 0.01 * dx { Some(ViewEdge::Horizontal(e.a[1])) }
    else { None }
}

impl Desktop {
    /// The commands of a Move or Copy of the selection by (`du`, `dz`) in a view, applied as one undoable step.
    pub(crate) fn view_move(&mut self, frame: ViewFrame, ids: &[ElementId], du: f64, dz: f64, copy: bool) -> bool {
        let Some(view) = crate::view_notes::view_ref(self.view) else { self.message = "Cannot do that: this is not an elevation or a section".into(); return false };
        match self.document.view_move_in(ids, frame, view, du, dz, copy)
            .and_then(|(ops, new)| self.document.transact(self.document.revision(), &ops).map(|_| new).map_err(|e| format!("{e:?}"))) {
            Ok(new) => {
                if copy && !new.is_empty() { self.select_many(new); }
                self.message = format!("{} done; undo takes it back", if copy { "Copy" } else { "Move" });
                true
            }
            Err(e) => { self.message = format!("Cannot do that: {e}"); false }
        }
    }
}

/// The Move, Copy, Rotate, Mirror and Align tools in a view.
#[allow(clippy::too_many_arguments)]
pub fn modify(app: &mut Desktop, ui: &egui::Ui, painter: &egui::Painter, response: &egui::Response, view: &SectionView, xf: Xf, edges: &[Edge]) {
    let blue = ui.visuals().hyperlink_color;
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let shift = ui.input(|i| i.modifiers.shift);
    let Some(frame) = ViewFrame::from_line(view.section.a, view.section.b) else { return };
    if app.view_edit.as_ref().is_none_or(|e| e.view != app.view) { app.view_edit = Some(ViewEdit::new(app.view)); }
    let tool = app.tool;
    let st = app.view_edit.clone().unwrap_or_else(|| ViewEdit::new(app.view));
    let base = st.pts.first().copied();
    let at = |p: Pos2| snap(edges, &xf, p, 10.0, base, shift && base.is_some());
    let hover = response.hover_pos().map(at);
    if let Some(h) = hover { draw_snap(painter, xf.screen(h.at), h.kind, blue); }

    if tool == Tool::Align {
        // Align: the reference edge first (any edge, a level line too), then the edge of what moves; the reference stays until Esc
        if esc { if st.edge.is_some() { if let Some(e) = app.view_edit.as_mut() { e.edge = None; } app.message = "Align: pick the reference edge".into(); } else { app.choose_tool(Tool::Select); } return; }
        if let Some(p) = response.hover_pos() {
            if let Some((e, _)) = pick_edge(edges, &xf, p, 7.0, |e| st.edge.is_none() || !e.datum) {
                painter.line_segment([xf.screen(e.a), xf.screen(e.b)], Stroke::new(3.0, blue.gamma_multiply(0.7)));
            }
        }
        if response.clicked() {
            let Some(p) = response.interact_pointer_pos() else { return };
            match st.edge {
                None => match pick_edge(edges, &xf, p, 7.0, |_| true) {
                    Some((_, edge)) => { if let Some(e) = app.view_edit.as_mut() { e.edge = Some(edge); } app.message = "Align: now pick the edge of the element to move onto it".into(); }
                    None => app.message = "Align: pick a vertical or a horizontal edge as the reference".into(),
                },
                Some(reference) => match pick_edge(edges, &xf, p, 7.0, |e| !e.datum && e.id.is_some()) {
                    Some((e, picked)) => {
                        let id = e.id.unwrap_or(ElementId(0));
                        match app.document.view_align_commands(id, frame, picked, reference)
                            .and_then(|ops| app.document.transact(app.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
                            Ok(_) => app.message = "Aligned; undo takes it back (pick the next element, Esc ends)".into(),
                            Err(e) => app.message = format!("Align: {e}"),
                        }
                    }
                    None => app.message = "Align: pick a vertical or a horizontal edge of an element".into(),
                },
            }
        }
        return;
    }

    // the others work on the selection: a click on an element picks one when nothing is selected
    let ids: Vec<ElementId> = if st.src.is_empty() { app.selected_ids() } else { st.src.clone() };
    if ids.is_empty() {
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let hit = crate::section_ui::pick_at(&app.document, view, xf.model(p));
                app.select_element(hit);
                app.message = if hit.is_some() { "Now pick the points".into() } else { "Click an element to work on".into() };
            }
        }
        if esc { app.choose_tool(Tool::Select); }
        return;
    }
    let typed: String = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
    if !st.pts.is_empty() { for c in typed.chars() { if c.is_ascii_digit() || c == '.' || (c == '-' && app.edit_num.is_empty()) { app.edit_num.push(c); } } }
    // Rotate turns about the middle of what is selected; Move and Copy start at a base point
    let centre = {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for item in view.items.iter().filter(|i| ids.contains(&ElementId(i.element.id)) && i.class != Class::Datum) {
            let pts: Vec<[f64; 2]> = match &item.shape { Shape::Region { outer, .. } => outer.clone(), Shape::Fill(r) => r.clone(), Shape::Line(a, b) => vec![*a, *b], _ => vec![] };
            for p in pts { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
        }
        [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]
    };
    if esc {
        if st.pts.is_empty() && app.edit_num.is_empty() { app.choose_tool(Tool::Select); } else { app.edit_num.clear(); if let Some(e) = app.view_edit.as_mut() { e.pts.clear(); e.src.clear(); } }
        return;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Backspace)) && !app.edit_num.is_empty() { app.edit_num.pop(); }
    let typed_value = app.edit_num.parse::<f64>().ok();
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    let click = response.clicked().then(|| response.interact_pointer_pos()).flatten();
    let finish = |app: &mut Desktop, keep_copying: bool| {
        app.edit_num.clear();
        if keep_copying { if let Some(e) = app.view_edit.as_mut() { e.pts.truncate(1); e.src = ids.clone(); } }
        else { app.choose_tool(Tool::Select); }
    };
    match tool {
        Tool::Move | Tool::Copy => {
            let copy = tool == Tool::Copy;
            let Some(b) = base else {
                if let Some(p) = click { let s = at(p); if let Some(e) = app.view_edit.as_mut() { e.pts = vec![s.at]; e.src = ids.clone(); } app.message = "Pick the point to move to (type a distance in mm and Enter, Shift keeps it level or plumb, Esc cancels)".into(); }
                return;
            };
            if let Some(h) = hover {
                let (mut du, mut dz) = (h.at[0] - b[0], h.at[1] - b[1]);
                if let Some(v) = typed_value { let l = du.hypot(dz); if l > 1e-9 { (du, dz) = (du / l * v / 1000.0, dz / l * v / 1000.0); } }
                painter.line_segment([xf.screen(b), xf.screen(h.at)], Stroke::new(1.0, blue));
                draw_ghost(painter, view, &app.document, &xf, &ids, du, dz);
                if !app.edit_num.is_empty() { painter.text(xf.screen(h.at) + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{}_", app.edit_num), egui::FontId::proportional(13.0), blue); }
                else { painter.text(xf.screen(h.at) + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{:+.0} mm along, {:+.0} mm up", du * 1000.0, dz * 1000.0), egui::FontId::proportional(13.0), blue); }
                let go = if enter && typed_value.is_some() { Some((du, dz)) } else { click.map(|p| { let s = at(p); (s.at[0] - b[0], s.at[1] - b[1]) }) };
                if let Some((du, dz)) = go {
                    if du.hypot(dz) < 1e-9 { app.message = "Pick a different point".into(); }
                    else if app.view_move(frame, &ids, du, dz, copy) { finish(app, copy); } else { app.edit_num.clear(); if let Some(e) = app.view_edit.as_mut() { e.pts.clear(); e.src.clear(); } }
                }
            }
        }
        Tool::Rotate => {
            painter.circle_stroke(xf.screen(centre), 6.0, Stroke::new(1.5, blue));
            painter.line_segment([xf.screen(centre) + Vec2::new(-9.0, 0.0), xf.screen(centre) + Vec2::new(9.0, 0.0)], Stroke::new(1.0, blue));
            let turn = |app: &mut Desktop, angle: f64| {
                match app.document.view_rotate_commands(&ids, frame, angle)
                    .and_then(|ops| app.document.transact(app.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
                    Ok(_) => { app.message = "Rotate done; undo takes it back".into(); finish(app, false); }
                    Err(e) => { app.message = format!("Cannot do that: {e}"); app.edit_num.clear(); if let Some(e) = app.view_edit.as_mut() { e.pts.clear(); } }
                }
            };
            if enter { if let Some(v) = typed_value { turn(app, v.to_radians()); return; } }
            match (base, hover) {
                (None, _) => if let Some(p) = click { let s = at(p); if let Some(e) = app.view_edit.as_mut() { e.pts = vec![s.at]; } app.message = "Pick the end of the turn, or type an angle in degrees and Enter".into(); },
                (Some(start), Some(h)) => {
                    painter.line_segment([xf.screen(centre), xf.screen(start)], Stroke::new(1.0, blue));
                    painter.line_segment([xf.screen(centre), xf.screen(h.at)], Stroke::new(1.0, blue));
                    let ang = |p: [f64; 2]| (p[1] - centre[1]).atan2(p[0] - centre[0]);
                    let delta = ang(h.at) - ang(start);
                    painter.text(xf.screen(h.at) + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, if app.edit_num.is_empty() { format!("{:.1}°", delta.to_degrees()) } else { format!("{}_", app.edit_num) }, egui::FontId::proportional(13.0), blue);
                    if let Some(p) = click { let s = at(p); if (s.at[0] - centre[0]).hypot(s.at[1] - centre[1]) > 1e-6 { turn(app, ang(s.at) - ang(start)); } }
                }
                _ => {}
            }
        }
        Tool::Mirror => {
            let copy = app.mirror_copy;
            match (base, hover) {
                (None, _) => if let Some(p) = click { let s = at(p); if let Some(e) = app.view_edit.as_mut() { e.pts = vec![s.at]; e.src = ids.clone(); } app.message = "Pick the second point of the vertical axis (Shift keeps it plumb, Esc cancels)".into(); },
                (Some(b), Some(h)) => {
                    painter.line_segment([xf.screen(b), xf.screen(h.at)], Stroke::new(1.0, blue));
                    if let Some(p) = click {
                        let s = at(p);
                        if (s.at[0] - b[0]).abs() > 0.02 { app.message = "In a view the mirror axis must be vertical (hold Shift for the second point)".into(); }
                        else {
                            match app.document.view_mirror_commands(&ids, frame, b[0], copy)
                                .and_then(|(ops, new)| app.document.transact(app.document.revision(), &ops).map(|_| new).map_err(|e| format!("{e:?}"))) {
                                Ok(new) => { if copy && !new.is_empty() { app.select_many(new); } app.message = "Mirror done; undo takes it back".into(); finish(app, false); }
                                Err(e) => { app.message = format!("Cannot do that: {e}"); if let Some(e) = app.view_edit.as_mut() { e.pts.clear(); } }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xf() -> Xf { Xf { centre: Pos2::ZERO, scale: 100.0, cu: 0.0, cz: 0.0 } }
    fn edge(a: [f64; 2], b: [f64; 2]) -> Edge { Edge { a, b, id: Some(ElementId(1)), datum: false } }
    fn near(a: [f64; 2], b: [f64; 2]) -> bool { (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6 }

    #[test]
    fn the_increment_follows_the_zoom_and_the_pointer_snaps_in_revits_order() {
        // zoomed out the increment is large, zoomed in it is small
        assert!(grid_step(5.0) >= 1.0 && grid_step(2000.0) <= 0.01 + 1e-12);
        let x = xf();
        let s = |e: &[Edge], p: [f64; 2], d: Vec2| snap(e, &x, x.screen(p) + d, 10.0, None, false);
        let line = [edge([0.0, 0.0], [1.0, 0.0])];
        // near an end: the endpoint; near the middle: the midpoint; elsewhere on the line: the nearest point
        let r = s(&line, [1.0, 0.0], Vec2::new(3.0, 2.0));
        assert!(r.kind == SnapKind::Endpoint && near(r.at, [1.0, 0.0]));
        let r = s(&line, [0.5, 0.0], Vec2::new(-2.0, 3.0));
        assert!(r.kind == SnapKind::Midpoint && near(r.at, [0.5, 0.0]));
        let r = s(&line, [0.3, 0.0], Vec2::new(0.0, -4.0));
        assert!(r.kind == SnapKind::Nearest && (r.at[0] - 0.3).abs() < 1e-6 && r.at[1].abs() < 1e-9, "{r:?}");
        // a crossing wins over the midpoints that coincide with it, an endpoint over a crossing
        let cross = [edge([0.0, 0.0], [1.0, 1.0]), edge([0.0, 1.0], [1.0, 0.0])];
        assert_eq!(s(&cross, [0.5, 0.5], Vec2::new(2.0, 2.0)).kind, SnapKind::Intersection);
        let corner = [edge([0.0, 0.0], [1.0, 0.0]), edge([1.0, 0.0], [1.0, 1.0]), edge([0.5, -1.0], [1.0, 1.0])];
        assert_eq!(s(&corner, [1.0, 0.0], Vec2::new(1.0, 1.0)).kind, SnapKind::Endpoint);
        // far from everything: the increment (0.1 m at 100 px/m)
        let r = s(&line, [0.234, 0.0], Vec2::new(0.0, -60.0));
        assert!(r.kind == SnapKind::Grid && (r.at[0] - 0.2).abs() < 1e-9, "{r:?}");
        // Shift keeps it level with the base point
        let r = snap(&line, &x, x.screen([1.0, 0.3]), 10.0, Some([0.0, 0.0]), true);
        assert!(near(r.at, [1.0, 0.0]), "{r:?}");
        // an edge must be vertical or horizontal to be aligned to
        let e = [edge([0.0, 0.0], [1.0, 1.0]), edge([2.0, 0.0], [2.0, 1.0])];
        assert!(pick_edge(&e, &x, x.screen([0.5, 0.5]), 7.0, |_| true).is_none());
        assert!(matches!(pick_edge(&e, &x, x.screen([2.0, 0.5]), 7.0, |_| true), Some((_, ViewEdge::Vertical(u))) if (u - 2.0).abs() < 1e-9));
    }
}
