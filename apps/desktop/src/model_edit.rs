use crate::{Desktop, ElementId, SampleElement, Tool};
use openbim_document::{Category, Command, Geometry, Section};

impl Desktop {
    pub(crate) fn selected_id(&self) -> Option<ElementId> {
        match self.selected { Some(SampleElement::Duct) => Some(ElementId(1)),
            Some(SampleElement::Model(id)) => Some(id), _ => None }
    }

    pub(crate) fn select_element(&mut self, id: Option<ElementId>) {
        if id == self.selected_id() { return; }
        let pending = self.has_pending_edit();
        self.selected = id.map(|id| if id == ElementId(1) { SampleElement::Duct } else { SampleElement::Model(id) });
        self.sync_duct_controls();
        if pending { self.message = "Selection changed; unapplied values reset".into(); }
    }

    pub(crate) fn sync_duct_controls(&mut self) {
        if let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) {
            self.width_mm = el.width_m * 1000.0;
            self.height_mm = el.height_m * 1000.0;
            self.elevation_mm = el.elevation_m * 1000.0;
            self.slope_percent = el.slope_ratio.unwrap_or(0.0) * 100.0;
            if el.category == Category::Beam {
                (self.section_pick, self.web_mm, self.flange_mm) = match el.section {
                    Section::Rectangle => (0, self.web_mm, self.flange_mm),
                    Section::Circle => (1, self.web_mm, self.flange_mm),
                    Section::IShape { web_m, flange_m } => (2, web_m * 1000.0, flange_m * 1000.0),
                };
            }
            if let Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) = el.geometry {
                self.axis_mm = [start[0], start[1], end[0], end[1]].map(|n| n * 1000.0);
            }
        } else { self.selected = None; }
    }

    /// The section the options bar or the properties stand for.
    pub(crate) fn section_from_controls(&self) -> Section {
        match self.section_pick {
            1 => Section::Circle,
            2 => Section::IShape { web_m: self.web_mm / 1000.0, flange_m: self.flange_mm / 1000.0 },
            _ => Section::Rectangle,
        }
    }

    pub(crate) fn has_pending_edit(&self) -> bool {
        let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) else { return false; };
        let differs = |a: f64, b: f64| !b.is_finite() || (a - b).abs() > 1e-6;
        let size = matches!(el.category, Category::Wall | Category::Beam | Category::Duct | Category::Pipe)
            && (differs(el.width_m * 1000.0, self.width_mm)
                || (!el.is_round() && self.section_pick != 1 && differs(el.height_m * 1000.0, self.height_mm)));
        let section = el.category == Category::Beam && {
            let want = self.section_from_controls();
            match (el.section, want) {
                (Section::IShape { web_m: a, flange_m: b }, Section::IShape { web_m: c, flange_m: d }) => differs(a * 1000.0, c * 1000.0) || differs(b * 1000.0, d * 1000.0),
                (a, b) => a != b,
            }
        };
        let elevation = el.category != Category::Grid && differs(el.elevation_m * 1000.0, self.elevation_mm);
        let slope = el.slope_ratio.is_some_and(|s| differs(s * 100.0, self.slope_percent));
        let axis = match el.geometry {
            Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) =>
                [start[0], start[1], end[0], end[1]].iter().zip(self.axis_mm)
                    .any(|(a, b)| differs(a * 1000.0, b)),
            _ => false,
        };
        size || section || elevation || slope || axis
    }

    /// Writes the document as `openbim-export.ifc` in the working directory and says what was and was not written.
    pub(crate) fn export_ifc_file(&mut self) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        match crate::export::export_ifc(&self.document, "Project1", 1, &crate::export::iso_time(secs)) {
            Ok(out) => match std::fs::write("openbim-export.ifc", &out.text) {
                Ok(()) => self.message = format!("Exported openbim-export.ifc: {} walls, {} beams. {}", out.walls, out.beams, out.warnings.join("; ")),
                Err(e) => self.message = format!("Could not write openbim-export.ifc: {e}"),
            },
            Err(e) => self.message = format!("IFC export failed: {e}"),
        }
    }

    /// The levels of the document, lowest first: (id, name, elevation).
    pub(crate) fn levels(&self) -> Vec<(ElementId, String, f64)> {
        let mut l: Vec<_> = self.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level)))
            .map(|e| (e.id, e.name.clone(), e.elevation_m)).collect();
        l.sort_by(|a, b| a.2.total_cmp(&b.2));
        l
    }

    /// Places a beam with the sizes of the options bar on `level`.
    fn place_beam_on(&mut self, level: ElementId, start: [f64; 2], end: [f64; 2]) -> Option<ElementId> {
        let m = |v: f64| v / 1000.0;
        let depth = if self.section_pick == 1 { self.width_mm } else { self.height_mm };
        match self.document.place_beam(level, start, end, m(self.width_mm), m(depth), m(self.elevation_mm), self.section_from_controls()) {
            Ok(id) => { self.message = format!("Beam {} placed; undo removes it", id.0); Some(id) }
            Err(error) => { self.message = format!("Beam rejected: {error:?}"); None }
        }
    }

    /// Places a beam from the options bar on its level, selects it and goes back to Select.
    pub(crate) fn place_beam_from_controls(&mut self) {
        let levels = self.levels();
        let level = self.beam_level.filter(|l| levels.iter().any(|x| x.0 == *l)).or(levels.first().map(|l| l.0));
        let Some(level) = level else { self.message = "Add a level before placing a beam".into(); return; };
        let m = |v: f64| v / 1000.0;
        let (start, end) = ([m(self.axis_mm[0]), m(self.axis_mm[1])], [m(self.axis_mm[2]), m(self.axis_mm[3])]);
        if let Some(id) = self.place_beam_on(level, start, end) {
            self.tool = Tool::Select;
            self.beam_start = None;
            self.selected = Some(SampleElement::Model(id));
            self.sync_duct_controls();
        }
    }

    /// A click of the Beam tool in the plan of `level`: the first sets the start, the second places the beam and the
    /// tool stays on for the next one.
    pub(crate) fn beam_click(&mut self, level: ElementId, point: [f64; 2]) {
        match self.beam_start {
            None => { self.beam_start = Some(point); self.message = "Beam: click the end".into(); }
            Some(start) => if self.place_beam_on(level, start, point).is_some() { self.beam_start = None; },
        }
    }

    pub(crate) fn apply_duct_edit(&mut self) {
        if self.selected_id() != Some(ElementId(1)) {
            self.message = "Select the duct to edit its parameters".into();
            return;
        }
        self.apply_selected_edit();
    }

    pub(crate) fn apply_selected_edit(&mut self) {
        let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) else { return; };
        if !self.has_pending_edit() { return; }
        let id = el.id;
        let mut commands = Vec::new();
        if matches!(el.category, Category::Wall | Category::Beam | Category::Duct | Category::Pipe) {
            let round = el.category == Category::Pipe || (el.category == Category::Beam && self.section_pick == 1);
            commands.push(Command::Resize { id, width_m: self.width_mm / 1000.0,
                height_m: if round { self.width_mm } else { self.height_mm } / 1000.0 });
        }
        if el.category == Category::Beam { commands.push(Command::SetSection { id, section: self.section_from_controls() }); }
        if el.category != Category::Grid { commands.push(Command::MoveElevation { id, elevation_m: self.elevation_mm / 1000.0 }); }
        if el.category.supports_slope() { commands.push(Command::SetSlope { id, slope_ratio: self.slope_percent / 100.0 }); }
        let start = [self.axis_mm[0] / 1000.0, self.axis_mm[1] / 1000.0];
        let end = [self.axis_mm[2] / 1000.0, self.axis_mm[3] / 1000.0];
        let geometry = match el.geometry {
            Some(Geometry::Linear { level, .. }) => Some(Geometry::Linear { start, end, level }),
            Some(Geometry::Grid { .. }) => Some(Geometry::Grid { start, end }), _ => None,
        };
        if let Some(geometry) = geometry { commands.push(Command::SetGeometry { id, geometry }); }
        match self.document.transact(self.document.revision(), &commands) {
            Ok(revision) => {
                self.sync_duct_controls();
                self.message = format!("Element {} updated — revision {revision}", id.0);
            }
            Err(error) => self.message = format!("Edit rejected: {error:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_and_i_beams_are_placed_edited_drawn_closed_in_3d_and_undone() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Beam);
        (app.width_mm, app.height_mm, app.elevation_mm) = (200.0, 400.0, 2800.0);
        app.axis_mm = [1000.0, 1000.0, 5000.0, 1000.0];
        app.section_pick = 2;
        (app.web_mm, app.flange_mm) = (10.0, 20.0);
        app.place_beam_from_controls();
        let id = app.selected_id().unwrap();
        assert_eq!(app.document.get(id).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
        assert_eq!(app.section_pick, 2);
        assert!(!app.has_pending_edit());
        // its 3D mesh is closed: each edge is used twice, with opposite directions, and the volume is area x length
        let batch = crate::view3d::batch(&app.document);
        let ei = batch.elements.iter().position(|e| e.id == id.0).unwrap() as u32;
        let tris: Vec<[u32; 3]> = batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| *t).collect();
        let mut edges = std::collections::HashMap::<(u32, u32), i32>::new();
        for t in &tris { for k in 0..3 { *edges.entry((t[k], t[(k + 1) % 3])).or_default() += 1; } }
        assert!(edges.iter().all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1)), "closed and consistently wound");
        let vol: f64 = tris.iter().map(|t| {
            let p = t.map(|i| batch.verts[i as usize].map(f64::from));
            (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
        }).sum();
        assert!((vol - 0.0116 * 4.0).abs() < 1e-3, "{vol}");
        // the properties turn it into a round beam of 300 mm: one transaction
        app.section_pick = 1;
        app.width_mm = 300.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        let el = app.document.get(id).unwrap();
        assert_eq!((el.section, el.width_m, el.height_m), (Section::Circle, 0.3, 0.3));
        assert!(!app.has_pending_edit());
        app.undo_document();
        assert_eq!(app.document.get(id).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
        // an I-section whose web is wider than its flanges is rejected and nothing changes
        app.sync_duct_controls();
        app.web_mm = 250.0;
        app.apply_selected_edit();
        assert!(app.message.contains("rejected"), "{}", app.message);
        assert_eq!(app.document.get(id).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
    }

    #[test]
    fn two_clicks_in_a_plan_place_a_beam_on_that_level_and_the_tool_stays_on() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Beam);
        (app.width_mm, app.height_mm, app.elevation_mm) = (300.0, 500.0, 2500.0);
        let n = app.document.elements().count();
        app.beam_click(ElementId(11), [1.0, 1.0]);
        assert_eq!(app.document.elements().count(), n, "one click places nothing");
        app.beam_click(ElementId(11), [1.0, 1.0]);
        assert_eq!(app.document.elements().count(), n, "a zero-length beam is refused");
        assert_eq!(app.beam_start, Some([1.0, 1.0]), "and the start is kept");
        app.beam_click(ElementId(11), [4.0, 1.0]);
        assert_eq!(app.document.elements().count(), n + 1);
        assert!(app.tool == Tool::Beam && app.beam_start.is_none());
        let beam = app.document.elements().find(|e| e.category == Category::Beam).unwrap();
        assert!(matches!(beam.geometry, Some(Geometry::Linear { level, start: [1.0, 1.0], end: [4.0, 1.0] }) if level == ElementId(11)));
        // its centre is 2.5 m above Level 2 (3 m)
        let z = app.document.mesh(beam.id).unwrap().vertices.iter().map(|v| v[2]).fold(0.0, f64::max);
        assert!((z - (3.0 + 2.5 + 0.25)).abs() < 1e-9, "{z}");
        // the options bar places on the level it was given, not the lowest
        app.beam_level = Some(ElementId(11));
        app.axis_mm = [0.0, 2000.0, 3000.0, 2000.0];
        app.place_beam_from_controls();
        let last = app.selected_id().unwrap();
        assert!(matches!(app.document.get(last).unwrap().geometry, Some(Geometry::Linear { level, .. }) if level == ElementId(11)));
        // choosing the tool again forgets a half-placed beam
        app.choose_tool(Tool::Beam);
        app.beam_click(ElementId(10), [0.0, 0.0]);
        app.choose_tool(Tool::Beam);
        assert!(app.beam_start.is_none());
    }

    #[test]
    fn a_beam_is_placed_from_the_options_bar_shown_in_3d_and_undone() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Beam);
        (app.width_mm, app.height_mm, app.elevation_mm) = (200.0, 400.0, 2800.0);
        app.axis_mm = [1000.0, 1000.0, 5000.0, 1000.0];
        let before = crate::view3d::batch(&app.document).elements.len();
        app.place_beam_from_controls();
        let id = app.selected_id().expect("the new beam is selected");
        assert!(app.tool == Tool::Select);
        let el = app.document.get(id).unwrap();
        assert_eq!(el.category, Category::Beam);
        assert!(!app.has_pending_edit(), "the controls show the beam");
        let batch = crate::view3d::batch(&app.document);
        assert_eq!(batch.elements.len(), before + 1);
        assert!(batch.elements.iter().any(|e| e.kind == openbim_view::Kind::Beam && e.id == id.0));
        // 4 m long, 0.2 x 0.4: the mesh is that box centred at 2.8 m
        let v = app.document.mesh(id).unwrap().vertices;
        let (lo, hi) = (v.iter().map(|p| p[2]).fold(f64::MAX, f64::min), v.iter().map(|p| p[2]).fold(f64::MIN, f64::max));
        assert!((lo - 2.6).abs() < 1e-9 && (hi - 3.0).abs() < 1e-9, "{lo} {hi}");
        // a zero-length beam is refused and nothing is added
        app.choose_tool(Tool::Beam);
        app.axis_mm = [1000.0, 1000.0, 1000.0, 1000.0];
        let n = app.document.elements().count();
        app.place_beam_from_controls();
        assert_eq!(app.document.elements().count(), n);
        assert!(app.message.contains("rejected"));
        app.undo_document();
        assert!(app.document.get(id).is_none(), "undo removes the beam");
    }
    #[test]
    fn category_controls_update_the_selected_geometry_and_undo() {
        let mut app = Desktop::sample();
        app.select_element(Some(ElementId(20)));
        assert!(!app.has_pending_edit());
        let original = app.document.mesh(ElementId(20)).unwrap();
        app.width_mm = 400.0;
        app.height_mm = 4000.0;
        app.apply_selected_edit();
        assert_ne!(app.document.mesh(ElementId(20)).unwrap(), original);
        app.undo_document();
        assert_eq!(app.document.mesh(ElementId(20)).unwrap(), original);
        app.select_element(Some(ElementId(100)));
        app.axis_mm[2] = 1000.0;
        app.apply_selected_edit();
        assert!(matches!(app.document.get(ElementId(100)).unwrap().geometry,
            Some(Geometry::Grid { end: [1.0, 7.0], .. })));
        app.select_element(Some(ElementId(10)));
        app.elevation_mm = 1000.0;
        app.apply_selected_edit();
        assert_eq!(app.document.mesh(ElementId(20)).unwrap().vertices[0][2], 1.0);
        app.select_element(Some(ElementId(2)));
        app.width_mm = 150.0;
        app.slope_percent = -2.0;
        app.apply_selected_edit();
        let pipe = app.document.get(ElementId(2)).unwrap();
        assert_eq!(pipe.width_m, 0.15);
        assert_eq!(pipe.height_m, 0.15);
        assert_eq!(pipe.slope_ratio, Some(-0.02));
    }
}
