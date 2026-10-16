use crate::{Desktop, ElementId, SampleElement};
use openbim_document::{Category, Command, Geometry};

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
            if let Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) = el.geometry {
                self.axis_mm = [start[0], start[1], end[0], end[1]].map(|n| n * 1000.0);
            }
        } else { self.selected = None; }
    }

    pub(crate) fn has_pending_edit(&self) -> bool {
        let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) else { return false; };
        let differs = |a: f64, b: f64| !b.is_finite() || (a - b).abs() > 1e-6;
        let size = matches!(el.category, Category::Wall | Category::Duct | Category::Pipe)
            && (differs(el.width_m * 1000.0, self.width_mm)
                || (el.category != Category::Pipe && differs(el.height_m * 1000.0, self.height_mm)));
        let elevation = el.category != Category::Grid && differs(el.elevation_m * 1000.0, self.elevation_mm);
        let slope = el.slope_ratio.is_some_and(|s| differs(s * 100.0, self.slope_percent));
        let axis = match el.geometry {
            Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) =>
                [start[0], start[1], end[0], end[1]].iter().zip(self.axis_mm)
                    .any(|(a, b)| differs(a * 1000.0, b)),
            _ => false,
        };
        size || elevation || slope || axis
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
        if matches!(el.category, Category::Wall | Category::Duct | Category::Pipe) {
            commands.push(Command::Resize { id, width_m: self.width_mm / 1000.0,
                height_m: if el.category == Category::Pipe { self.width_mm } else { self.height_mm } / 1000.0 });
        }
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
