//! What each view of the model is set to, as Revit's View Properties, Visibility/Graphics and View Range have it: the
//! view scale, the detail level, the view range of a plan (top, cut plane, bottom, view depth), the categories that are
//! turned off and the elements that are hidden in the view. A view that has its default settings holds nothing in the
//! model. The settings are saved with the model and are one undo step each.
//!
//! A view is named by a short key the application chooses ("p10" for the plan of level 10, "3d", "s25" for a section...).
use crate::schedule::{family_kind, ScheduleKind};
use crate::{Category, Command, Document, Element, ElementId, SlabKind};
use std::collections::BTreeMap;

pub const MAX_HIDDEN: usize = 20_000;
pub const MIN_SCALE: u32 = 1;
pub const MAX_SCALE: u32 = 10_000;
/// The scales of Revit's list (1:1 ... 1:1000); any other is a custom scale.
pub const STANDARD_SCALES: [u32; 14] = [1, 2, 5, 10, 20, 25, 50, 75, 100, 125, 150, 200, 500, 1000];

/// A category of the Visibility/Graphics window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewCategory {
    Model(ScheduleKind),
    Openings,
    Rebar,
    Grids,
    Levels,
    Tags,
    Dimensions,
    Sections,
    Elevations,
}

impl ViewCategory {
    /// Every category, model categories first, as the two tabs of the window list them.
    pub fn all() -> Vec<ViewCategory> {
        let mut v: Vec<ViewCategory> = ScheduleKind::ALL.into_iter().filter(|k| !matches!(k, ScheduleKind::DuctFittings | ScheduleKind::PipeFittings)).map(ViewCategory::Model).collect();
        v.extend([ViewCategory::Openings, ViewCategory::Rebar, ViewCategory::Grids, ViewCategory::Levels, ViewCategory::Tags, ViewCategory::Dimensions, ViewCategory::Sections, ViewCategory::Elevations]);
        v
    }
    pub fn annotation(self) -> bool { matches!(self, ViewCategory::Grids | ViewCategory::Levels | ViewCategory::Tags | ViewCategory::Dimensions | ViewCategory::Sections | ViewCategory::Elevations) }
    pub fn name(self) -> &'static str {
        match self {
            ViewCategory::Model(k) => k.name(), ViewCategory::Openings => "Openings", ViewCategory::Rebar => "Structural Rebar", ViewCategory::Grids => "Grids",
            ViewCategory::Levels => "Levels", ViewCategory::Tags => "Tags", ViewCategory::Dimensions => "Dimensions", ViewCategory::Sections => "Sections", ViewCategory::Elevations => "Elevations",
        }
    }
    pub fn code(self) -> String {
        match self {
            ViewCategory::Model(k) => format!("m-{}", k.code()), ViewCategory::Openings => "openings".into(), ViewCategory::Rebar => "rebar".into(), ViewCategory::Grids => "grids".into(),
            ViewCategory::Levels => "levels".into(), ViewCategory::Tags => "tags".into(), ViewCategory::Dimensions => "dims".into(), ViewCategory::Sections => "sections".into(), ViewCategory::Elevations => "elevs".into(),
        }
    }
    pub fn from_code(c: &str) -> Option<ViewCategory> { Self::all().into_iter().find(|v| v.code() == c) }

    /// The category of an element; None for a type, which is nothing in a view.
    pub fn of(e: &Element) -> Option<ViewCategory> {
        use ScheduleKind as K;
        Some(match e.category {
            Category::Wall => ViewCategory::Model(K::Walls),
            Category::Slab => ViewCategory::Model(match e.slab_kind { SlabKind::Floor => K::Floors, SlabKind::Foundation => K::Foundations, SlabKind::Roof => K::Roofs, SlabKind::Ceiling => K::Ceilings }),
            Category::Footing | Category::WallFooting => ViewCategory::Model(K::Foundations),
            Category::Column => ViewCategory::Model(K::Columns),
            Category::Beam | Category::BeamSystem | Category::Truss => ViewCategory::Model(K::Beams),
            Category::Stair => ViewCategory::Model(K::Stairs),
            Category::Railing => ViewCategory::Model(K::Railings),
            Category::Room => ViewCategory::Model(K::Rooms),
            Category::Duct => ViewCategory::Model(K::Ducts),
            Category::Pipe => ViewCategory::Model(K::Pipes),
            Category::Equipment => ViewCategory::Model(K::MechanicalEquipment),
            Category::Family => ViewCategory::Model(family_kind(e)),
            Category::Opening => ViewCategory::Openings,
            Category::Rebar => ViewCategory::Rebar,
            Category::Grid => ViewCategory::Grids,
            Category::Level => ViewCategory::Levels,
            Category::Tag => ViewCategory::Tags,
            Category::Dimension => ViewCategory::Dimensions,
            Category::Section => ViewCategory::Sections,
            Category::Elevation => ViewCategory::Elevations,
            Category::Type => return None,
        })
    }
}

/// How much detail a view shows (Revit's Detail Level). Coarse draws a layered wall as one solid shape; medium and fine
/// draw its layers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Detail { Coarse, #[default] Medium, Fine }

impl Detail {
    pub const ALL: [Detail; 3] = [Detail::Coarse, Detail::Medium, Detail::Fine];
    pub fn name(self) -> &'static str { match self { Detail::Coarse => "Coarse", Detail::Medium => "Medium", Detail::Fine => "Fine" } }
    pub fn code(self) -> &'static str { match self { Detail::Coarse => "C", Detail::Medium => "M", Detail::Fine => "F" } }
    pub fn from_code(c: &str) -> Option<Detail> { Self::ALL.into_iter().find(|d| d.code() == c) }
}

/// The view range of a plan, as heights above its level (metres): the top of what is seen, the cut plane (what it
/// cuts is drawn as cut), the bottom, and the view depth (how far below the bottom things are still seen, shown as
/// beyond the range). `depth_m <= bottom_m <= cut_m <= top_m`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewRange {
    pub top_m: f64,
    pub cut_m: f64,
    pub bottom_m: f64,
    pub depth_m: f64,
}

impl ViewRange {
    /// Revit's default for a plan: top 2300 mm, cut plane 1200 mm, bottom and view depth at the level.
    pub const ARCHITECTURE: ViewRange = ViewRange { top_m: 2.3, cut_m: 1.2, bottom_m: 0.0, depth_m: 0.0 };
    /// The default of the mechanical plan: a top of 3000 mm, to see ducts under a ceiling.
    pub const MECHANICAL: ViewRange = ViewRange { top_m: 3.0, cut_m: 1.2, bottom_m: 0.0, depth_m: 0.0 };
    pub fn valid(&self) -> bool {
        [self.top_m, self.cut_m, self.bottom_m, self.depth_m].iter().all(|x| x.is_finite() && x.abs() <= 1000.0)
            && self.depth_m <= self.bottom_m && self.bottom_m <= self.cut_m && self.cut_m <= self.top_m
    }
    /// Why the range is not valid, in Revit's terms.
    pub fn problem(&self) -> Option<&'static str> {
        if ![self.top_m, self.cut_m, self.bottom_m, self.depth_m].iter().all(|x| x.is_finite() && x.abs() <= 1000.0) { Some("the heights are numbers within 1000 m") }
        else if self.cut_m > self.top_m { Some("the cut plane is at or below the top") }
        else if self.bottom_m > self.cut_m { Some("the bottom is at or below the cut plane") }
        else if self.depth_m > self.bottom_m { Some("the view depth is at or below the bottom") }
        else { None }
    }
}

/// Where a thing of height `z0..z1` is in a plan whose level is at `level_z`, with this range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeClass {
    /// not in the range at all: not drawn
    Out,
    /// crossed by the cut plane: drawn as cut
    Cut,
    /// between the cut plane and the top, not crossed: seen from the cut plane upward
    Above,
    /// between the bottom and the cut plane: seen from the cut plane downward
    Below,
    /// under the bottom but within the view depth: beyond the range
    Beyond,
}

impl ViewRange {
    pub fn classify(&self, level_z: f64, z0: f64, z1: f64) -> RangeClass {
        const EPS: f64 = 1e-6;
        let (lo, hi) = (z0.min(z1), z0.max(z1));
        let (top, cut, bottom, depth) = (level_z + self.top_m, level_z + self.cut_m, level_z + self.bottom_m, level_z + self.depth_m);
        if hi <= depth + EPS || lo >= top - EPS { RangeClass::Out }
        else if lo < cut - EPS && hi > cut + EPS { RangeClass::Cut }
        else if lo >= cut - EPS { RangeClass::Above }
        else if hi > bottom + EPS { RangeClass::Below }
        else { RangeClass::Beyond }
    }
}

/// The settings of one view.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewSettings {
    pub key: String,
    /// 1:`scale`
    pub scale: u32,
    pub detail: Detail,
    /// a plan's own view range; None: the default of its kind
    pub range: Option<ViewRange>,
    /// the categories turned off
    pub hidden_categories: Vec<ViewCategory>,
    /// the elements hidden in this view (Hide in View > Elements)
    pub hidden_elements: Vec<ElementId>,
}

impl ViewSettings {
    pub fn default_for(key: &str) -> ViewSettings {
        ViewSettings { key: key.into(), scale: 100, detail: Detail::Medium, range: None, hidden_categories: Vec::new(), hidden_elements: Vec::new() }
    }
    pub fn is_default(&self) -> bool { *self == Self::default_for(&self.key) }
    /// Whether the key is that of a plan (the plan of a level or the mechanical plan), which has a view range.
    pub fn is_plan_key(key: &str) -> bool { key == "mech" || (key.starts_with('p') && key.len() > 1 && key[1..].chars().all(|c| c.is_ascii_digit())) }
    /// The view range in force: its own, or the default of its kind.
    pub fn effective_range(&self) -> Option<ViewRange> {
        if !Self::is_plan_key(&self.key) { return None; }
        Some(self.range.unwrap_or(if self.key == "mech" { ViewRange::MECHANICAL } else { ViewRange::ARCHITECTURE }))
    }
    /// Whether an element is hidden by these settings (its category is off, or it is hidden by itself).
    pub fn hides(&self, e: &Element) -> bool {
        self.hidden_elements.contains(&e.id) || ViewCategory::of(e).is_some_and(|c| self.hidden_categories.contains(&c))
    }
    pub fn check(&self, elements: &BTreeMap<ElementId, Element>) -> Result<(), String> {
        if self.key.is_empty() || self.key.len() > 24 || !self.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { return Err("a view key is 1 to 24 letters, digits or _".into()); }
        if !(MIN_SCALE..=MAX_SCALE).contains(&self.scale) { return Err(format!("the scale is 1:{MIN_SCALE} to 1:{MAX_SCALE}")); }
        if let Some(r) = &self.range {
            if !Self::is_plan_key(&self.key) { return Err("only a plan has a view range".into()); }
            if let Some(p) = r.problem() { return Err(format!("view range: {p}")); }
        }
        let mut cats = std::collections::BTreeSet::new();
        if !self.hidden_categories.iter().all(|c| cats.insert(c.code())) { return Err("a category is turned off twice".into()); }
        let mut seen = std::collections::BTreeSet::new();
        if self.hidden_elements.len() > MAX_HIDDEN || !self.hidden_elements.iter().all(|id| elements.contains_key(id) && seen.insert(*id)) { return Err("the hidden elements are not all there, once each".into()); }
        Ok(())
    }
}

impl Document {
    /// The settings of a view (the defaults for a view that has none).
    pub fn view_settings(&self, key: &str) -> ViewSettings {
        self.mep().view_settings.iter().find(|v| v.key == key).cloned().unwrap_or_else(|| ViewSettings::default_for(key))
    }

    /// Saves the settings of a view: one undo step. A view set back to its defaults holds nothing.
    pub fn set_view_settings_commands(&self, vs: &ViewSettings) -> Result<Vec<Command>, String> {
        let elements: BTreeMap<ElementId, Element> = self.elements().map(|e| (e.id, e.clone())).collect();
        vs.check(&elements)?;
        if self.view_settings(&vs.key) == *vs { return Err("nothing changes".into()); }
        let mut data = self.mep().clone();
        data.view_settings.retain(|v| v.key != vs.key);
        if !vs.is_default() { data.view_settings.push(vs.clone()); }
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// The document as a view with these settings sees it: without what `keep` turns away, and without the ducts and
    /// pipes' joins to it. It has no undo history and is for drawing and for making views, not for editing.
    pub fn filtered(&self, keep: &dyn Fn(&Element) -> bool) -> Document {
        let elements: BTreeMap<ElementId, Element> = self.elements().filter(|e| keep(e)).map(|e| (e.id, e.clone())).collect();
        let mep = self.mep().pruned(&elements);
        Document { elements, site: self.site(), mep, project_key: self.project_key(), app_records: Vec::new(), revision: self.revision(), history: Vec::new(), future: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(d: &mut Document, c: Vec<Command>) { d.transact(d.revision(), &c).unwrap(); }

    #[test]
    fn the_view_range_puts_a_thing_in_its_class_and_must_be_in_order() {
        let r = ViewRange::ARCHITECTURE;
        // a wall 0 to 3 m on a level at 0 is cut by the cut plane at 1.2 m
        assert_eq!(r.classify(0.0, 0.0, 3.0), RangeClass::Cut);
        // a floor under the level is beyond only if the view depth reaches it
        assert_eq!(r.classify(0.0, -0.3, 0.0), RangeClass::Out);
        let deep = ViewRange { depth_m: -0.5, ..r };
        assert_eq!(deep.classify(0.0, -0.3, 0.0), RangeClass::Beyond);
        // a table top 0.7 to 0.8 is below the cut plane; a shelf at 1.5 to 1.6 is above it; a duct at 2.7 is over the top
        assert_eq!(r.classify(0.0, 0.7, 0.8), RangeClass::Below);
        assert_eq!(r.classify(0.0, 1.5, 1.6), RangeClass::Above);
        assert_eq!(r.classify(0.0, 2.7, 3.0), RangeClass::Out);
        assert_eq!(ViewRange::MECHANICAL.classify(0.0, 2.7, 3.0), RangeClass::Above);
        // the level above: a slab whose top is at the level of the next plan is not in that plan's range
        assert_eq!(r.classify(3.0, 2.8, 3.0), RangeClass::Out);
        // the order
        assert!(r.valid());
        assert_eq!(ViewRange { cut_m: 2.5, ..r }.problem(), Some("the cut plane is at or below the top"));
        assert_eq!(ViewRange { bottom_m: 1.5, ..r }.problem(), Some("the bottom is at or below the cut plane"));
        assert_eq!(ViewRange { depth_m: 0.5, ..r }.problem(), Some("the view depth is at or below the bottom"));
        assert!(!ViewRange { top_m: f64::NAN, ..r }.valid());
    }

    #[test]
    fn every_element_has_a_category_a_code_that_reads_back_and_a_hidden_one_is_found() {
        let d = Document::spatial_sample();
        for c in ViewCategory::all() {
            assert_eq!(ViewCategory::from_code(&c.code()), Some(c), "{c:?}");
            assert!(!c.name().is_empty());
        }
        let cats = |id: u64| ViewCategory::of(d.get(ElementId(id)).unwrap());
        assert_eq!(cats(20), Some(ViewCategory::Model(ScheduleKind::Walls)));
        assert_eq!(cats(10), Some(ViewCategory::Levels));
        assert_eq!(cats(100), Some(ViewCategory::Grids));
        assert_eq!(cats(1), Some(ViewCategory::Model(ScheduleKind::Ducts)));
        let mut vs = ViewSettings::default_for("p10");
        vs.hidden_categories.push(ViewCategory::Model(ScheduleKind::Walls));
        vs.hidden_elements.push(ElementId(1));
        assert!(vs.hides(d.get(ElementId(20)).unwrap()) && vs.hides(d.get(ElementId(1)).unwrap()) && !vs.hides(d.get(ElementId(2)).unwrap()));
        assert!(ViewSettings::is_plan_key("p10") && ViewSettings::is_plan_key("mech") && !ViewSettings::is_plan_key("3d") && !ViewSettings::is_plan_key("p"));
        assert_eq!(ViewSettings::default_for("mech").effective_range(), Some(ViewRange::MECHANICAL));
        assert_eq!(ViewSettings::default_for("p11").effective_range(), Some(ViewRange::ARCHITECTURE));
        assert_eq!(ViewSettings::default_for("3d").effective_range(), None);
    }

    #[test]
    fn settings_are_saved_one_undo_step_each_default_ones_hold_nothing_and_they_read_back() {
        let mut d = Document::spatial_sample();
        assert_eq!(d.view_settings("p10"), ViewSettings::default_for("p10"));
        let mut vs = d.view_settings("p10");
        vs.scale = 50;
        vs.detail = Detail::Coarse;
        vs.range = Some(ViewRange { top_m: 3.5, cut_m: 1.0, bottom_m: -0.2, depth_m: -1.0 });
        vs.hidden_categories = vec![ViewCategory::Grids, ViewCategory::Model(ScheduleKind::Ducts)];
        vs.hidden_elements = vec![ElementId(20), ElementId(21)];
        let steps = d.history_mark();
        let c = d.set_view_settings_commands(&vs).unwrap();
        run(&mut d, c);
        assert_eq!(d.history_mark(), steps + 1);
        assert_eq!(d.view_settings("p10"), vs);
        assert!(d.set_view_settings_commands(&vs).unwrap_err().contains("nothing changes"));
        // a second view
        let mut s3 = ViewSettings::default_for("3d");
        s3.detail = Detail::Fine;
        let c = d.set_view_settings_commands(&s3).unwrap();
        run(&mut d, c);
        assert_eq!(d.mep().view_settings.len(), 2);
        // the records
        let back = crate::native::parse_mep(&crate::native::mep_records(d.mep())).unwrap();
        assert_eq!(back.view_settings, d.mep().view_settings);
        // back to the defaults: nothing is held
        let c = d.set_view_settings_commands(&ViewSettings::default_for("p10")).unwrap();
        run(&mut d, c);
        assert_eq!(d.mep().view_settings.len(), 1);
        d.undo().unwrap();
        assert_eq!(d.view_settings("p10"), vs);
        // an element that goes is no longer hidden in the view
        d.transact(d.revision(), &[Command::Delete { id: ElementId(21) }]).unwrap();
        assert_eq!(d.view_settings("p10").hidden_elements, vec![ElementId(20)]);
    }

    #[test]
    fn bad_settings_are_refused_by_the_commands_and_by_the_model() {
        let d = Document::spatial_sample();
        let bad = |f: &dyn Fn(&mut ViewSettings)| { let mut v = ViewSettings::default_for("p10"); f(&mut v); d.set_view_settings_commands(&v).is_err() };
        assert!(bad(&|v| v.scale = 0));
        assert!(bad(&|v| v.scale = 20_000));
        assert!(bad(&|v| v.key = "bad key".into()));
        assert!(bad(&|v| v.key = String::new()));
        assert!(bad(&|v| v.range = Some(ViewRange { top_m: 1.0, ..ViewRange::ARCHITECTURE })));
        assert!(bad(&|v| { v.key = "3d".into(); v.range = Some(ViewRange::ARCHITECTURE); }), "only a plan has a view range");
        assert!(bad(&|v| v.hidden_categories = vec![ViewCategory::Grids, ViewCategory::Grids]));
        assert!(bad(&|v| v.hidden_elements = vec![ElementId(9999)]));
        assert!(bad(&|v| v.hidden_elements = vec![ElementId(20), ElementId(20)]));
        let mut data = d.mep().clone();
        let mut v = ViewSettings::default_for("p10");
        v.scale = 0;
        data.view_settings.push(v);
        let mut d2 = d.clone();
        assert!(d2.transact(d2.revision(), &[Command::SetMep(Box::new(data))]).is_err());
    }

    #[test]
    fn a_filtered_document_leaves_out_what_is_turned_away_and_changes_nothing_of_the_original() {
        let d = Document::spatial_sample();
        let n = d.elements().count();
        let f = d.filtered(&|e| e.category != Category::Wall);
        assert_eq!(f.elements().count(), n - 6);
        assert_eq!(d.elements().count(), n);
        assert!(!f.can_undo());
        assert!(f.get(ElementId(10)).is_some() && f.get(ElementId(20)).is_none());
        assert_eq!(f.revision(), d.revision());
    }
}
