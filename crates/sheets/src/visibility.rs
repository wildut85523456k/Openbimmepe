//! What a view shows, the Revit way.
//!
//! * every category belongs to a discipline (or is shared: grids, levels);
//! * a view has a discipline and, for every other discipline, a mode: show, halftone or hide;
//! * visibility/graphics overrides per category, then rule-based filters (a filter higher in the list
//!   wins), then the discipline mode — but only where nothing said explicitly whether to show;
//! * elements of a linked model follow the host view, a named view of the linked model, or a view template;
//! * a view template replaces a view's own settings.
//!
//! The defaults for the disciplines are editable starting points, not standards.

use std::collections::BTreeMap;

use crate::model::Params;
use crate::{clean_name, SheetError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Discipline {
    Architectural,
    Structural,
    Mechanical,
    Plumbing,
    Electrical,
    FireProtection,
    /// shows everything; as a category discipline it means shared by all (grids, levels)
    Coordination,
}

impl Discipline {
    pub const ALL: [Discipline; 7] = [
        Discipline::Architectural,
        Discipline::Structural,
        Discipline::Mechanical,
        Discipline::Plumbing,
        Discipline::Electrical,
        Discipline::FireProtection,
        Discipline::Coordination,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Discipline::Architectural => "Architectural",
            Discipline::Structural => "Structural",
            Discipline::Mechanical => "Mechanical",
            Discipline::Plumbing => "Plumbing",
            Discipline::Electrical => "Electrical",
            Discipline::FireProtection => "Fire Protection",
            Discipline::Coordination => "Coordination",
        }
    }

    /// one-letter code used in layer names and drawing numbers
    pub fn code(&self) -> char {
        match self {
            Discipline::Architectural => 'A',
            Discipline::Structural => 'S',
            Discipline::Mechanical => 'M',
            Discipline::Plumbing => 'P',
            Discipline::Electrical => 'E',
            Discipline::FireProtection => 'F',
            Discipline::Coordination => 'C',
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        Self::ALL.into_iter().find(|d| d.name().eq_ignore_ascii_case(t) || (t.len() == 1 && d.code().to_string().eq_ignore_ascii_case(t)) || (*d == Discipline::FireProtection && t.eq_ignore_ascii_case("fire")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryKind {
    Model,
    Annotation,
    Imported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub name: String,
    pub discipline: Discipline,
    pub kind: CategoryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Coarse,
    Medium,
    Fine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Show,
    Halftone,
    Hide,
}

/// Graphic overrides. `None` means "no opinion".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Override {
    pub visible: Option<bool>,
    pub halftone: Option<bool>,
    /// line weight number (1 to 16)
    pub weight: Option<u8>,
    pub colour: Option<[u8; 3]>,
}

impl Override {
    /// This override where it has an opinion, `below` elsewhere.
    pub fn over(&self, below: &Override) -> Override {
        Override {
            visible: self.visible.or(below.visible),
            halftone: self.halftone.or(below.halftone),
            weight: self.weight.or(below.weight),
            colour: self.colour.or(below.colour),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Equals,
    NotEquals,
    Contains,
    BeginsWith,
    Greater,
    Less,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub param: String,
    pub op: Op,
    pub value: String,
}

impl Rule {
    /// A missing parameter never matches, whatever the operator. Text is compared ignoring case;
    /// Greater and Less compare numbers and do not match text that is not a number.
    pub fn matches(&self, params: &Params) -> bool {
        let Some((_, have)) = params.iter().find(|(k, _)| crate::model::same(k, &self.param)) else {
            return false;
        };
        let (h, w) = (have.trim().to_lowercase(), self.value.trim().to_lowercase());
        match self.op {
            Op::Equals => h == w,
            Op::NotEquals => h != w,
            Op::Contains => h.contains(&w),
            Op::BeginsWith => h.starts_with(&w),
            Op::Greater | Op::Less => match (h.parse::<f64>(), w.parse::<f64>()) {
                (Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => {
                    if self.op == Op::Greater {
                        a > b
                    } else {
                        a < b
                    }
                }
                _ => false,
            },
        }
    }
}

/// Selects elements of some categories (all when the list is empty) where every rule holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub name: String,
    pub categories: Vec<String>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkDisplay {
    /// follow the host view
    ByHost,
    /// use the named view of the linked model (its own settings)
    ByLinked(String),
    /// use the named view template
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewSettings {
    pub name: String,
    pub discipline: Discipline,
    pub detail: DetailLevel,
    /// how the other disciplines look; a discipline not listed is shown
    pub others: BTreeMap<Discipline, Mode>,
    pub categories: BTreeMap<String, Override>,
    /// filters in order; the first has priority
    pub filters: Vec<(String, Override)>,
    /// display setting per linked model (not listed: by host)
    pub links: BTreeMap<String, LinkDisplay>,
    /// a view template that replaces this view's discipline, detail, others, categories and filters
    pub template: Option<String>,
}

impl ViewSettings {
    /// A view with the starting discipline defaults: its own discipline shown, architecture and
    /// structure halftoned behind the services, other services hidden; Coordination shows everything.
    pub fn new(name: &str, discipline: Discipline) -> Self {
        use Discipline::*;
        let mut others = BTreeMap::new();
        for d in Discipline::ALL {
            let m = if d == discipline || discipline == Coordination || d == Coordination {
                Mode::Show
            } else {
                match (discipline, d) {
                    (Architectural, Structural) => Mode::Halftone,
                    (Structural, Architectural) => Mode::Halftone,
                    (Mechanical | Plumbing | Electrical | FireProtection, Architectural | Structural) => Mode::Halftone,
                    _ => Mode::Hide,
                }
            };
            others.insert(d, m);
        }
        ViewSettings {
            name: name.to_string(),
            discipline,
            detail: DetailLevel::Medium,
            others,
            categories: BTreeMap::new(),
            filters: Vec::new(),
            links: BTreeMap::new(),
            template: None,
        }
    }

    pub fn mode_for(&self, d: Discipline) -> Mode {
        if d == Discipline::Coordination || d == self.discipline || self.discipline == Discipline::Coordination {
            Mode::Show
        } else {
            self.others.get(&d).copied().unwrap_or(Mode::Show)
        }
    }
}

/// Everything views refer to by name.
#[derive(Debug, Clone, Default)]
pub struct ViewLibrary {
    categories: BTreeMap<String, Category>,
    filters: BTreeMap<String, Filter>,
    templates: BTreeMap<String, ViewSettings>,
    views: BTreeMap<String, ViewSettings>,
}

/// One element as the view sees it.
#[derive(Debug, Clone)]
pub struct ElementRef<'a> {
    pub category: &'a str,
    pub params: &'a Params,
    /// name of the linked model the element comes from, if any
    pub link: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Display {
    Hidden,
    Shown { halftone: bool, weight: Option<u8>, colour: Option<[u8; 3]> },
}

impl ViewLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_category(&mut self, c: Category) -> Result<(), SheetError> {
        clean_name(&c.name, "category name")?;
        if self.categories.len() >= 2_000 {
            return Err(SheetError::TooMany("categories"));
        }
        if self.categories.insert(c.name.clone(), c.clone()).is_some() {
            return Err(SheetError::Duplicate { what: "category", value: c.name });
        }
        Ok(())
    }

    pub fn category(&self, name: &str) -> Option<&Category> {
        self.categories.get(name)
    }

    pub fn add_filter(&mut self, f: Filter) -> Result<(), SheetError> {
        clean_name(&f.name, "filter name")?;
        if f.rules.len() > 50 || f.rules.is_empty() {
            return Err(SheetError::BadValue { what: "filter rules", value: f.rules.len().to_string() });
        }
        for c in &f.categories {
            if !self.categories.contains_key(c) {
                return Err(SheetError::Unknown { what: "category", value: c.clone() });
            }
        }
        if self.filters.insert(f.name.clone(), f.clone()).is_some() {
            return Err(SheetError::Duplicate { what: "filter", value: f.name });
        }
        Ok(())
    }

    fn check_view(&self, v: &ViewSettings) -> Result<(), SheetError> {
        clean_name(&v.name, "view name")?;
        for c in v.categories.keys() {
            if !self.categories.contains_key(c) {
                return Err(SheetError::Unknown { what: "category", value: c.clone() });
            }
        }
        for (f, o) in &v.filters {
            if !self.filters.contains_key(f) {
                return Err(SheetError::Unknown { what: "filter", value: f.clone() });
            }
            check_override(o)?;
        }
        for o in v.categories.values() {
            check_override(o)?;
        }
        Ok(())
    }

    pub fn add_template(&mut self, t: ViewSettings) -> Result<(), SheetError> {
        self.check_view(&t)?;
        if t.template.is_some() {
            return Err(SheetError::BadValue { what: "template", value: "a template cannot use a template".into() });
        }
        if self.templates.insert(t.name.clone(), t.clone()).is_some() {
            return Err(SheetError::Duplicate { what: "view template", value: t.name });
        }
        Ok(())
    }

    pub fn add_view(&mut self, v: ViewSettings) -> Result<(), SheetError> {
        self.check_view(&v)?;
        if let Some(t) = &v.template {
            if !self.templates.contains_key(t) {
                return Err(SheetError::Unknown { what: "view template", value: t.clone() });
            }
        }
        if self.views.insert(v.name.clone(), v.clone()).is_some() {
            return Err(SheetError::Duplicate { what: "view", value: v.name });
        }
        Ok(())
    }

    pub fn view(&self, name: &str) -> Option<&ViewSettings> {
        self.views.get(name)
    }

    /// Settings in force: the template's where the view has one, under the view's own name and links.
    pub fn effective(&self, v: &ViewSettings) -> ViewSettings {
        match v.template.as_ref().and_then(|t| self.templates.get(t)) {
            Some(t) => ViewSettings { name: v.name.clone(), links: v.links.clone(), template: v.template.clone(), ..t.clone() },
            None => v.clone(),
        }
    }

    /// Categories the view's override list offers (Revit's "filter list"): shared ones, its own
    /// discipline's and those of other disciplines it does not hide.
    pub fn listed_categories(&self, v: &ViewSettings) -> Vec<&Category> {
        let e = self.effective(v);
        self.categories.values().filter(|c| e.mode_for(c.discipline) != Mode::Hide).collect()
    }

    /// How one element looks in a view.
    pub fn display(&self, view: &ViewSettings, el: &ElementRef) -> Result<Display, SheetError> {
        let host = self.effective(view);
        let using: ViewSettings = match el.link {
            None => host,
            Some(l) => match host.links.get(l).unwrap_or(&LinkDisplay::ByHost) {
                LinkDisplay::ByHost => host,
                LinkDisplay::ByLinked(vn) => {
                    let lv = self.views.get(vn).ok_or_else(|| SheetError::Unknown { what: "view", value: vn.clone() })?;
                    self.effective(lv)
                }
                LinkDisplay::Custom(tn) => self.templates.get(tn).cloned().ok_or_else(|| SheetError::Unknown { what: "view template", value: tn.clone() })?,
            },
        };
        let cat = self.categories.get(el.category).ok_or_else(|| SheetError::Unknown { what: "category", value: el.category.to_string() })?;
        let base = using.categories.get(&cat.name).cloned().unwrap_or_default();
        let mut by_filters = Override::default();
        for (fname, ov) in &using.filters {
            let Some(f) = self.filters.get(fname) else { continue };
            let in_cat = f.categories.is_empty() || f.categories.iter().any(|c| c == &cat.name);
            if in_cat && f.rules.iter().all(|r| r.matches(el.params)) {
                by_filters = by_filters.over(ov);
            }
        }
        let mut fin = by_filters.over(&base);
        if fin.visible.is_none() {
            match using.mode_for(cat.discipline) {
                Mode::Show => {}
                Mode::Halftone => {
                    if fin.halftone.is_none() {
                        fin.halftone = Some(true);
                    }
                }
                Mode::Hide => fin.visible = Some(false),
            }
        }
        if fin.visible == Some(false) {
            return Ok(Display::Hidden);
        }
        Ok(Display::Shown { halftone: fin.halftone.unwrap_or(false), weight: fin.weight, colour: fin.colour })
    }
}

fn check_override(o: &Override) -> Result<(), SheetError> {
    if let Some(w) = o.weight {
        if !(1..=16).contains(&w) {
            return Err(SheetError::BadValue { what: "line weight number", value: w.to_string() });
        }
    }
    Ok(())
}
