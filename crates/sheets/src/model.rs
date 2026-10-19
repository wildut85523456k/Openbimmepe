//! Sheets and the sheet set: numbers, names, parameters, viewports, revisions and revision clouds.
//! Field values for title block text come from here (`SheetSet::field`).

use std::collections::BTreeMap;

use crate::paper::Rect;
use crate::titleblock::TitleBlockDef;
use crate::{clean_name, clean_text, finite, SheetError, MAX_TEXT};

pub type Params = BTreeMap<String, String>;

pub const MAX_SHEETS: usize = 10_000;
pub const MAX_VIEWPORTS: usize = 200;
pub const MAX_PARAMS: usize = 200;
pub const MAX_REVISIONS: usize = 1_000;
pub const MAX_CLOUD_POINTS: usize = 500;
pub const MAX_NUMBER: usize = 64;
pub const MAX_SCALE: u32 = 1_000_000;

/// Fields computed by the program. Everything else is read from sheet or project parameters.
pub const BUILTIN_FIELDS: [&str; 11] = [
    "Sheet Number",
    "Sheet Name",
    "Sheet Index",
    "Sheet Count",
    "Sheet Size",
    "Scale",
    "Current Revision",
    "Current Revision Date",
    "Current Revision Description",
    "Plot Date",
    "File Path",
];

pub(crate) fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_FIELDS.iter().any(|b| same(b, name))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Revision {
    pub seq: u32,
    pub date: String,
    pub description: String,
    pub issued: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    /// name of the view placed here
    pub view: String,
    /// where the middle of the frame is on the sheet, millimetres
    pub centre: [f64; 2],
    /// frame width and height on the sheet, millimetres
    pub size: [f64; 2],
    /// scale 1:n
    pub scale: u32,
    /// the model point (metres, view coordinates) shown at the middle of the frame
    pub model_centre: [f64; 2],
    /// view title; the view name when empty
    pub title: Option<String>,
}

impl Viewport {
    pub fn frame(&self) -> Rect {
        Rect::new(self.centre[0] - self.size[0] / 2.0, self.centre[1] - self.size[1] / 2.0, self.size[0], self.size[1])
    }
}

/// A revision cloud: a closed outline on the sheet, in sheet millimetres, tied to a revision.
#[derive(Debug, Clone, PartialEq)]
pub struct Cloud {
    pub revision: u32,
    pub outline: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub number: String,
    pub name: String,
    pub titleblock: String,
    pub params: Params,
    pub viewports: Vec<Viewport>,
    pub clouds: Vec<Cloud>,
}

impl Sheet {
    pub fn new(number: &str, name: &str, titleblock: &str) -> Self {
        Sheet {
            number: number.to_string(),
            name: name.to_string(),
            titleblock: titleblock.to_string(),
            params: Params::new(),
            viewports: Vec::new(),
            clouds: Vec::new(),
        }
    }
}

/// What the printer knows that the model does not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrintContext {
    pub plot_date: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct SheetSet {
    pub project: Params,
    titleblocks: Vec<TitleBlockDef>,
    revisions: Vec<Revision>,
    sheets: Vec<Sheet>,
}

pub fn valid_number(n: &str) -> bool {
    clean_text(n, MAX_NUMBER) && !n.chars().any(|c| "\\/:*?\"<>|".contains(c)) && n == n.trim()
}

pub(crate) fn check_params(p: &Params) -> Result<(), SheetError> {
    if p.len() > MAX_PARAMS {
        return Err(SheetError::TooMany("parameters"));
    }
    for (k, v) in p {
        clean_name(k, "parameter name")?;
        if v.chars().count() > MAX_TEXT || v.chars().any(|c| c.is_control()) {
            return Err(SheetError::BadValue { what: "parameter value", value: format!("{k} = {v}") });
        }
    }
    Ok(())
}

impl SheetSet {
    pub fn new(project: Params) -> Result<Self, SheetError> {
        check_params(&project)?;
        Ok(SheetSet { project, ..Default::default() })
    }

    pub fn sheets(&self) -> &[Sheet] {
        &self.sheets
    }
    pub fn titleblocks(&self) -> &[TitleBlockDef] {
        &self.titleblocks
    }
    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }
    pub fn sheet(&self, number: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.number == number)
    }
    pub fn titleblock(&self, name: &str) -> Option<&TitleBlockDef> {
        self.titleblocks.iter().find(|t| t.name == name)
    }

    pub fn add_titleblock(&mut self, def: TitleBlockDef) -> Result<(), SheetError> {
        def.validate()?;
        if self.titleblock(&def.name).is_some() {
            return Err(SheetError::Duplicate { what: "title block", value: def.name });
        }
        if self.titleblocks.len() >= 200 {
            return Err(SheetError::TooMany("title blocks"));
        }
        self.titleblocks.push(def);
        Ok(())
    }

    pub fn add_revision(&mut self, date: &str, description: &str) -> Result<u32, SheetError> {
        if self.revisions.len() >= MAX_REVISIONS {
            return Err(SheetError::TooMany("revisions"));
        }
        for (v, w) in [(date, "revision date"), (description, "revision description")] {
            if v.chars().count() > MAX_TEXT || v.chars().any(|c| c.is_control()) {
                return Err(SheetError::BadValue { what: w, value: v.to_string() });
            }
        }
        let seq = self.revisions.last().map_or(1, |r| r.seq + 1);
        self.revisions.push(Revision { seq, date: date.to_string(), description: description.to_string(), issued: false });
        Ok(seq)
    }

    pub fn issue_revision(&mut self, seq: u32) -> Result<(), SheetError> {
        match self.revisions.iter_mut().find(|r| r.seq == seq) {
            Some(r) => {
                r.issued = true;
                Ok(())
            }
            None => Err(SheetError::Unknown { what: "revision", value: seq.to_string() }),
        }
    }

    fn check_sheet(&self, s: &Sheet) -> Result<(), SheetError> {
        if !valid_number(&s.number) {
            return Err(SheetError::BadName { what: "sheet number", value: s.number.clone() });
        }
        clean_name(&s.name, "sheet name")?;
        let tb = self.titleblock(&s.titleblock).ok_or_else(|| SheetError::Unknown { what: "title block", value: s.titleblock.clone() })?;
        check_params(&s.params)?;
        if s.viewports.len() > MAX_VIEWPORTS {
            return Err(SheetError::TooMany("viewports"));
        }
        for v in &s.viewports {
            clean_name(&v.view, "view name")?;
            if let Some(t) = &v.title {
                if t.chars().count() > MAX_TEXT || t.chars().any(|c| c.is_control()) {
                    return Err(SheetError::BadValue { what: "view title", value: t.clone() });
                }
            }
            if v.scale == 0 || v.scale > MAX_SCALE {
                return Err(SheetError::BadScale(v.scale));
            }
            for x in [v.centre[0], v.centre[1], v.size[0], v.size[1], v.model_centre[0], v.model_centre[1]] {
                finite(x, "viewport")?;
            }
            if v.size[0] <= 0.0 || v.size[1] <= 0.0 || !v.frame().inside(&tb.free_rect) {
                return Err(SheetError::OutOfBounds(format!("viewport of view '{}' on sheet {}", v.view, s.number)));
            }
        }
        if s.clouds.len() > 1_000 {
            return Err(SheetError::TooMany("revision clouds"));
        }
        let border = tb.border();
        for c in &s.clouds {
            if !self.revisions.iter().any(|r| r.seq == c.revision) {
                return Err(SheetError::Unknown { what: "revision", value: c.revision.to_string() });
            }
            if c.outline.len() < 3 || c.outline.len() > MAX_CLOUD_POINTS {
                return Err(SheetError::BadValue { what: "revision cloud", value: format!("{} points", c.outline.len()) });
            }
            for p in &c.outline {
                finite(p[0], "cloud")?;
                finite(p[1], "cloud")?;
                if !border.contains(*p) {
                    return Err(SheetError::OutOfBounds(format!("a revision cloud on sheet {}", s.number)));
                }
            }
        }
        Ok(())
    }

    pub fn add_sheet(&mut self, s: Sheet) -> Result<(), SheetError> {
        if self.sheets.len() >= MAX_SHEETS {
            return Err(SheetError::TooMany("sheets"));
        }
        self.check_sheet(&s)?;
        if self.sheet(&s.number).is_some() {
            return Err(SheetError::Duplicate { what: "sheet number", value: s.number });
        }
        self.sheets.push(s);
        Ok(())
    }

    /// Replace the sheet with this number (kept in place), checking the new content first.
    pub fn replace_sheet(&mut self, s: Sheet) -> Result<(), SheetError> {
        self.check_sheet(&s)?;
        match self.sheets.iter().position(|o| o.number == s.number) {
            Some(i) => {
                self.sheets[i] = s;
                Ok(())
            }
            None => Err(SheetError::Unknown { what: "sheet", value: s.number }),
        }
    }

    /// Revisions that have a cloud on this sheet, oldest first. These fill the sheet's revision table.
    pub fn revisions_on(&self, s: &Sheet) -> Vec<&Revision> {
        let mut seqs: Vec<u32> = s.clouds.iter().map(|c| c.revision).collect();
        seqs.sort_unstable();
        seqs.dedup();
        seqs.iter().filter_map(|q| self.revisions.iter().find(|r| r.seq == *q)).collect()
    }

    /// The latest issued revision with a cloud on this sheet.
    pub fn current_revision(&self, s: &Sheet) -> Option<&Revision> {
        self.revisions_on(s).into_iter().rfind(|r| r.issued)
    }

    /// "1:100" when every viewport has the same scale, "As indicated" when they differ, nothing without views.
    pub fn scale_text(&self, s: &Sheet) -> String {
        let mut it = s.viewports.iter().map(|v| v.scale);
        match it.next() {
            None => String::new(),
            Some(first) => {
                if it.all(|x| x == first) {
                    format!("1:{first}")
                } else {
                    "As indicated".to_string()
                }
            }
        }
    }

    /// The value of a title block field for a sheet: computed fields first, then sheet parameters, then
    /// project parameters. Names are compared ignoring case and surrounding spaces.
    pub fn field(&self, s: &Sheet, ctx: &PrintContext, name: &str) -> Option<String> {
        let n = name.trim().to_ascii_lowercase();
        let cur = self.current_revision(s);
        match n.as_str() {
            "sheet number" => return Some(s.number.clone()),
            "sheet name" => return Some(s.name.clone()),
            "sheet index" => {
                return self.sheets.iter().position(|o| o.number == s.number).map(|i| (i + 1).to_string());
            }
            "sheet count" => return Some(self.sheets.len().to_string()),
            "sheet size" => return self.titleblock(&s.titleblock).map(|t| t.size.name.clone()),
            "scale" => return Some(self.scale_text(s)),
            "current revision" => return Some(cur.map_or(String::new(), |r| r.seq.to_string())),
            "current revision date" => return Some(cur.map_or(String::new(), |r| r.date.clone())),
            "current revision description" => return Some(cur.map_or(String::new(), |r| r.description.clone())),
            "plot date" => return Some(ctx.plot_date.clone()),
            "file path" => return Some(ctx.file_path.clone()),
            _ => {}
        }
        for map in [&s.params, &self.project] {
            if let Some((_, v)) = map.iter().find(|(k, _)| same(k, name)) {
                return Some(v.clone());
            }
        }
        None
    }
}
