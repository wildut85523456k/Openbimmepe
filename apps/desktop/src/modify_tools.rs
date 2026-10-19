//! Revit's Modify tools that work on what is already there: Pin and Unpin, Cut, Copy and Paste through a clipboard,
//! Match Type Properties, Select All Instances and Create Similar.
use crate::{Desktop, Tool};
use eframe::egui;
use openbim_document::{Category, ElementId, Geometry};

impl Desktop {
    /// Ctrl+C, Ctrl+X and Ctrl+V reach egui as copy, cut and paste events (Cmd on a Mac).
    pub(crate) fn clipboard_events(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() { return; }
        let has = |f: fn(&egui::Event) -> bool| ctx.input(|i| i.events.iter().any(f));
        if has(|e| matches!(e, egui::Event::Copy)) { self.copy_to_clipboard(false); }
        else if has(|e| matches!(e, egui::Event::Cut)) { self.copy_to_clipboard(true); }
        else if has(|e| matches!(e, egui::Event::Paste(_))) { self.paste_start(); }
    }

    /// Ctrl+C or Ctrl+X: the selection and what stands in it go on the clipboard; a cut also deletes them.
    pub(crate) fn copy_to_clipboard(&mut self, cut: bool) {
        let ids = self.selected_ids();
        let what = if cut { "Cut" } else { "Copy" };
        match self.document.clipboard(&ids) {
            Err(e) => self.message = format!("{what}: {e}"),
            Ok(cb) => {
                let n = cb.elements.len();
                if cut {
                    match self.document.delete_commands(&ids).and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
                        Ok(_) => { self.select_element(None); self.clipboard = Some(cb); self.message = format!("Cut {n} element(s) to the clipboard; Ctrl+V pastes them, undo brings them back"); }
                        Err(e) => self.message = format!("Cut: {e}"),
                    }
                } else {
                    self.clipboard = Some(cb);
                    self.message = format!("Copied {n} element(s) to the clipboard; Ctrl+V pastes them");
                }
            }
        }
    }

    /// Ctrl+V: the clipboard follows the pointer from its centre, and a click puts it down (Esc ends).
    pub(crate) fn paste_start(&mut self) {
        if self.clipboard.is_none() { self.message = "The clipboard is empty: select something and press Ctrl+C".into(); return; }
        if matches!(self.view, crate::View::ThreeD | crate::View::Elevation(_) | crate::View::Section(_) | crate::View::Marked(..)) {
            self.message = "Paste works in a plan view".into(); return;
        }
        self.choose_tool(Tool::Paste);
        self.message = "Paste: move the pointer and click to put the elements down (they can be pasted again; Esc ends)".into();
    }

    /// A click of Paste: the clipboard is put down with its centre at `point`, on the level of the view, as one
    /// undoable step. The pasted elements are selected and the tool stays active.
    pub(crate) fn paste_at(&mut self, point: [f64; 2]) {
        let Some(cb) = self.clipboard.clone() else { return };
        self.paste_with(&cb, [point[0] - cb.base[0], point[1] - cb.base[1]]);
    }

    /// Paste Aligned to Same Place: the clipboard goes down where it came from, on the level of the view.
    pub(crate) fn paste_same_place(&mut self) {
        let Some(cb) = self.clipboard.clone() else { self.message = "The clipboard is empty: select something and press Ctrl+C".into(); return };
        if matches!(self.view, crate::View::ThreeD | crate::View::Elevation(_) | crate::View::Section(_) | crate::View::Marked(..)) { self.message = "Paste works in a plan view".into(); return; }
        self.paste_with(&cb, [0.0, 0.0]);
    }

    fn paste_with(&mut self, cb: &openbim_document::Clipboard, d: [f64; 2]) {
        let level = self.plan_level();
        match self.document.paste_commands(cb, d, Some(level)).and_then(|(ops, new)| self.document.transact(self.document.revision(), &ops).map(|_| new).map_err(|e| format!("{e:?}"))) {
            Ok(new) => { self.select_many(new); self.message = format!("Pasted {} element(s); undo takes them back", cb.elements.len()); }
            Err(e) => self.message = format!("Paste: {e}"),
        }
    }

    /// Pin or Unpin the selection.
    pub(crate) fn pin_selected(&mut self, pin: bool) {
        let ids = self.selected_ids();
        match self.document.pin_commands(&ids, pin).and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
            Ok(_) => self.message = format!("{} {} element(s); undo takes it back", if pin { "Pinned" } else { "Unpinned" }, ids.len()),
            Err(e) => self.message = format!("{}: {e}", if pin { "Pin" } else { "Unpin" }),
        }
    }

    /// Select All Instances: every element of the selected one's type, in the project or only in the open view.
    pub(crate) fn select_all_instances(&mut self, visible_in_view: bool) {
        let Some(id) = self.selected_id() else { self.message = "Select an element first".into(); return };
        let level = visible_in_view.then(|| self.plan_level());
        let ids = self.document.select_similar(id, level);
        let n = ids.len();
        self.select_many(ids);
        self.message = format!("{n} instance(s) of the same type selected{}", if visible_in_view { " in this view" } else { "" });
    }

    /// A click of Match Type Properties: the first pick is the source, then each pick gets its type (Esc ends).
    pub(crate) fn match_type_click(&mut self, hit: Option<ElementId>, at: [f64; 2]) {
        let Some(id) = hit else { self.message = "Match Type Properties: click an element".into(); return };
        match self.edit_ref {
            None => { self.edit_ref = Some((id, at)); self.message = format!("Match Type Properties: now click the elements that should get the type of {} (Esc ends)", self.document.get(id).map_or("it", |e| e.name.as_str())); }
            Some((source, _)) => match self.document.match_type(source, &[id]) {
                Ok(()) => { self.sync_duct_controls(); self.message = "Type matched; undo takes it back (click the next element, Esc ends)".into(); }
                Err(e) => self.message = format!("Match Type Properties: {e}"),
            },
        }
    }

    /// Create Similar: the tool that made the selected element, with its type and sizes.
    pub(crate) fn create_similar(&mut self) {
        let Some(el) = self.selected_id().and_then(|id| self.document.get(id)).cloned() else { self.message = "Select an element first".into(); return };
        self.sync_duct_controls();
        let tool = match el.category {
            Category::Wall => { self.wall_type = el.type_id; Tool::Wall }
            Category::Duct => { if let Some(r) = self.document.mep_run(el.id) { self.system_pick = r.system; } Tool::Duct }
            Category::Pipe => { if let Some(r) = self.document.mep_run(el.id) { self.system_pick = r.system; } Tool::Pipe }
            Category::Family => { self.family_pick = el.family.clone(); self.family_type = el.type_name.clone(); Tool::Family }
            Category::Column => Tool::Column,
            Category::Beam => Tool::Beam,
            Category::BeamSystem => Tool::BeamSystem,
            Category::Truss => Tool::Truss,
            Category::Footing => Tool::Footing,
            Category::WallFooting => Tool::WallFooting,
            Category::Slab => { self.slab_kind_pick = el.slab_kind; Tool::Slab }
            Category::Railing => Tool::Railing,
            Category::Stair => Tool::Stair,
            Category::Room => Tool::Room,
            Category::Grid => Tool::Grid,
            Category::Rebar => Tool::Rebar,
            Category::Level => Tool::Level,
            _ => { self.message = format!("Create Similar does not apply to {}", el.name); return; }
        };
        // choosing the tool clears the options, so the sizes are read again from the element
        let (w, h, e, wall_type, family, family_type, slab, system) = (self.width_mm, self.height_mm, self.elevation_mm, self.wall_type, self.family_pick.clone(), self.family_type.clone(), self.slab_kind_pick, self.system_pick);
        self.choose_tool(tool);
        (self.width_mm, self.height_mm, self.elevation_mm, self.wall_type, self.family_pick, self.family_type, self.slab_kind_pick, self.system_pick) = (w, h, e, wall_type, family, family_type, slab, system);
        if tool == Tool::Wall { self.wall_flip = false; }
        self.message = format!("Create Similar: {} — place the next one like {}; Esc ends", match tool { Tool::Wall => "Wall", Tool::Duct => "Duct", Tool::Pipe => "Pipe", Tool::Family => "Component", _ => "the same tool" }, el.name);
    }

    /// The elements of the plan that are pinned, with the point their pushpin is drawn at (the first point of the element).
    pub(crate) fn pin_marks(&self, level: ElementId) -> Vec<[f64; 2]> {
        self.document.elements().filter(|e| e.pinned && self.document.level_of(e.id).is_none_or(|l| l == level))
            .filter_map(|e| match e.geometry {
                Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) => Some([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0]),
                _ => openbim_document::plan_points(e).first().copied(),
            }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_blocks_the_modify_tools_and_unpin_frees_them() {
        let mut app = Desktop::sample();
        app.select_element(Some(ElementId(20)));
        app.pin_selected(true);
        assert!(app.document.get(ElementId(20)).unwrap().pinned);
        app.delete_selected();
        assert!(app.document.get(ElementId(20)).is_some() && app.message.contains("pinned"), "{}", app.message);
        app.pin_selected(true);
        assert!(app.message.contains("already pinned"), "{}", app.message);
        app.pin_selected(false);
        assert!(!app.document.get(ElementId(20)).unwrap().pinned);
        app.delete_selected();
        assert!(app.document.get(ElementId(20)).is_none());
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;
    use crate::View;

    fn door(app: &mut Desktop, wall: u64, along: f64) -> ElementId {
        let spec = openbim_document::FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        app.document.place_family_in_wall(ElementId(wall), along, &spec).unwrap()
    }
    fn events(app: &mut Desktop, e: egui::Event) {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput { events: vec![e], ..Default::default() }, |ui| app.clipboard_events(ui.ctx()));
        out.textures_delta.clear();
    }

    #[test]
    fn ctrl_x_then_ctrl_v_moves_a_wall_with_its_door_and_the_pasted_ones_are_selected() {
        let mut app = Desktop::sample();
        let dr = door(&mut app, 24, 3.0);
        app.select_element(Some(ElementId(24)));
        events(&mut app, egui::Event::Copy);
        assert!(app.clipboard.is_some() && app.document.get(ElementId(24)).is_some(), "copy leaves the wall");
        events(&mut app, egui::Event::Cut);
        assert!(app.document.get(ElementId(24)).is_none() && app.document.get(dr).is_none(), "{}", app.message);
        // nothing selected, so a second cut has nothing to cut and the clipboard keeps its data
        events(&mut app, egui::Event::Paste(String::new()));
        assert_eq!(app.tool, Tool::Paste, "{}", app.message);
        // Paste Aligned to Same Place puts it back where it was, on this view's level
        app.view = View::Plan;
        app.paste_same_place();
        let n = app.selected_id().expect("the pasted wall is selected");
        assert_eq!(app.document.get(n).unwrap().category, Category::Wall);
        assert!(app.document.elements().any(|e| matches!(e.geometry, Some(Geometry::InWall { host, .. }) if host == n)));
        app.run_command("undo", "Undo");
        assert!(app.document.get(n).is_none(), "one undo takes the whole paste back");
        // redo puts it back
        app.run_command("redo", "Redo");
        assert!(app.document.get(n).is_some(), "{}", app.message);
        // pasting on Level 2
        app.view = View::UpperPlan;
        app.paste_same_place();
        let up = app.selected_id().unwrap();
        assert_eq!(app.document.level_of(up), Some(ElementId(11)));
    }

    #[test]
    fn paste_in_a_section_view_or_with_an_empty_clipboard_says_why() {
        let mut app = Desktop::sample();
        app.paste_start();
        assert!(app.message.contains("clipboard is empty"), "{}", app.message);
        app.select_element(Some(ElementId(24)));
        app.copy_to_clipboard(false);
        app.view = View::ThreeD;
        app.paste_start();
        assert!(app.message.contains("plan view") && app.tool != Tool::Paste, "{}", app.message);
        // a level is not copied
        app.select_element(Some(ElementId(10)));
        app.copy_to_clipboard(false);
        assert!(app.message.contains("cannot be copied"), "{}", app.message);
    }

    #[test]
    fn select_all_instances_match_type_and_create_similar() {
        let mut app = Desktop::sample();
        let t = app.document.create_type(ElementId(20), "Wall 200 x 3000").unwrap();
        app.document.assign_type(&[ElementId(21)], t).unwrap();
        app.select_element(Some(ElementId(21)));
        app.select_all_instances(false);
        let mut got = app.selected_ids();
        got.sort();
        assert_eq!(got, vec![ElementId(20), ElementId(21)], "{}", app.message);
        // Match Type Properties: source, then targets, the tool stays active
        app.choose_tool(Tool::MatchType);
        app.match_type_click(Some(ElementId(20)), [0.0, 0.0]);
        app.match_type_click(Some(ElementId(22)), [0.0, 0.0]);
        assert_eq!(app.document.get(ElementId(22)).unwrap().type_id, Some(t), "{}", app.message);
        assert_eq!(app.tool, Tool::MatchType);
        app.match_type_click(Some(ElementId(1)), [0.0, 0.0]);
        assert!(app.message.contains("not the same kind"), "{}", app.message);
        // Create Similar on a typed wall chooses the Wall tool with that type
        app.select_element(Some(ElementId(21)));
        app.create_similar();
        assert_eq!((app.tool, app.wall_type), (Tool::Wall, Some(t)), "{}", app.message);
        assert_eq!((app.width_mm, app.height_mm), (200.0, 3000.0));
        // on a duct it chooses the Duct tool with the size of the duct
        app.select_element(Some(ElementId(1)));
        app.create_similar();
        assert_eq!((app.tool, app.width_mm, app.height_mm, app.elevation_mm), (Tool::Duct, 200.0, 300.0, 2700.0), "{}", app.message);
        // on a tag or a level-less element it says so
        app.select_element(Some(ElementId(100)));
        app.create_similar();
        assert!(app.message.contains("Create Similar"), "{}", app.message);
    }
}
