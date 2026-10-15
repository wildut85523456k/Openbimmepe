//! Where family and shared parameters go in the IFC file. A value with a row in the table is written in the
//! property set and under the property name the row names, with the value type the row names; everything else
//! stays in the custom sets (`OpenBIM_Family` on the type, `OpenBIM_Instance` on the occurrence).
//!
//! The table is plain tab-separated text, one row per line: IFC class (`*` for any), the parameter's name, the
//! property set, the property name, and the value kind. It can be made from the groups of a shared parameter set
//! (`from_shared`: a parameter whose group names a property set goes there under its own name) and edited by
//! hand. Which sets and property names exist is the IFC schema's business and the author's: nothing is shipped.
use openbim_family::SharedSet;
use openbim_family::{ValueKind};

pub const MAX_ROWS: usize = 100_000;
pub const MAX_NAME: usize = 64;

/// The kind of value a property holds, and so the IFC measure it is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Label,
    Text,
    Identifier,
    Boolean,
    Integer,
    Real,
    /// metres, any sign
    Length,
    /// metres, above zero
    PositiveLength,
    /// square metres
    Area,
    /// cubic metres
    Volume,
    /// a plane angle (the file holds radians)
    Angle,
}

impl PropKind {
    pub const ALL: [PropKind; 11] = [PropKind::Label, PropKind::Text, PropKind::Identifier, PropKind::Boolean, PropKind::Integer, PropKind::Real, PropKind::Length, PropKind::PositiveLength, PropKind::Area, PropKind::Volume, PropKind::Angle];
    pub fn code(self) -> &'static str {
        match self {
            PropKind::Label => "label",
            PropKind::Text => "text",
            PropKind::Identifier => "identifier",
            PropKind::Boolean => "boolean",
            PropKind::Integer => "integer",
            PropKind::Real => "real",
            PropKind::Length => "length",
            PropKind::PositiveLength => "positive_length",
            PropKind::Area => "area",
            PropKind::Volume => "volume",
            PropKind::Angle => "angle",
        }
    }
    pub fn from_code(s: &str) -> Option<PropKind> {
        PropKind::ALL.into_iter().find(|k| k.code() == s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapRow {
    /// IFC class the row applies to (`IfcDoor`), or `*`
    pub class: String,
    /// the family or shared parameter's name
    pub source: String,
    pub pset: String,
    pub property: String,
    pub kind: PropKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    TooMany,
    /// (line, what is wrong); line 0 when the table was not read from text
    Bad(usize, &'static str),
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for MapError {}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PsetMap {
    pub rows: Vec<MapRow>,
}

fn name_ok(s: &str) -> bool {
    !s.is_empty() && s.chars().count() <= MAX_NAME && s.trim() == s && !s.chars().any(|c| c.is_control() || c == '\'' || c == '\\')
}

fn class_ok(s: &str) -> bool {
    s == "*" || (s.len() > 3 && s.starts_with("Ifc") && s.chars().all(|c| c.is_ascii_alphanumeric()))
}

impl PsetMap {
    /// The row for a value on an element of `class`: a row for that class first, then a row for any class.
    pub fn find(&self, class: &str, source: &str) -> Option<&MapRow> {
        self.rows.iter().find(|r| r.source == source && r.class.eq_ignore_ascii_case(class)).or_else(|| self.rows.iter().find(|r| r.source == source && r.class == "*"))
    }

    pub fn check(&self) -> Result<(), MapError> {
        if self.rows.len() > MAX_ROWS {
            return Err(MapError::TooMany);
        }
        let mut seen = std::collections::HashSet::new();
        for r in &self.rows {
            if !class_ok(&r.class) {
                return Err(MapError::Bad(0, "the class is * or an IFC class name such as IfcDoor"));
            }
            if !name_ok(&r.source) || !name_ok(&r.pset) || !name_ok(&r.property) {
                return Err(MapError::Bad(0, "a name is empty, too long, padded with spaces, or has a quote, backslash or control character"));
            }
            if !seen.insert((r.class.to_ascii_lowercase(), r.source.clone())) {
                return Err(MapError::Bad(0, "two rows for the same class and parameter"));
            }
            // two parameters may not land on one property
        }
        let mut targets = std::collections::HashSet::new();
        for r in &self.rows {
            if !targets.insert((r.class.to_ascii_lowercase(), r.pset.clone(), r.property.clone())) {
                return Err(MapError::Bad(0, "two parameters go to the same property of the same set"));
            }
        }
        Ok(())
    }

    /// Rows from the groups of a shared parameter set: a parameter with a group goes to the property set of that
    /// name, under its own name, as the kind of its input (a length as a length, an angle as an angle, whole numbers as integers, other numbers as reals).
    pub fn from_shared(set: &SharedSet) -> PsetMap {
        let mut rows = Vec::new();
        for p in &set.params {
            if p.group.is_empty() {
                continue;
            }
            let kind = match &p.def.kind {
                ValueKind::Input { integer: true, .. } => PropKind::Integer,
                ValueKind::Input { .. } => match p.def.dim {
                    None => PropKind::Length,
                    Some(d) if d == openbim_family::LENGTH => PropKind::Length,
                    Some(d) if d == openbim_family::ANGLE => PropKind::Angle,
                    Some(d) if d == openbim_family::AREA => PropKind::Area,
                    Some(d) if d == openbim_family::VOLUME => PropKind::Volume,
                    Some(_) => PropKind::Real,
                },
                ValueKind::Flag { .. } => PropKind::Boolean,
                ValueKind::Choice { .. } | ValueKind::Text { .. } | ValueKind::Material { .. } => PropKind::Label,
                ValueKind::Formula(_) => continue,
            };
            rows.push(MapRow { class: "*".into(), source: p.def.name.clone(), pset: p.group.clone(), property: p.def.name.clone(), kind });
        }
        PsetMap { rows }
    }

    /// The rows of `self`, then the rows of `over` that do not repeat a class and parameter already there are
    /// replaced by it: `over` wins.
    pub fn overlaid(&self, over: &PsetMap) -> PsetMap {
        let mut rows: Vec<MapRow> = self.rows.iter().filter(|r| !over.rows.iter().any(|o| o.source == r.source && o.class.eq_ignore_ascii_case(&r.class))).cloned().collect();
        rows.extend(over.rows.iter().cloned());
        PsetMap { rows }
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from("# class\tparameter\tproperty set\tproperty\tkind\n");
        for r in &self.rows {
            s.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", r.class, r.source, r.pset, r.property, r.kind.code()));
        }
        s
    }

    /// Read the text form. Blank lines and lines starting with # are skipped; the error carries the 1-based line.
    pub fn from_text(text: &str) -> Result<PsetMap, MapError> {
        let mut m = PsetMap::default();
        for (i, line) in text.lines().enumerate() {
            let ln = i + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 5 {
                return Err(MapError::Bad(ln, "a row needs 5 tab-separated fields"));
            }
            let kind = PropKind::from_code(f[4]).ok_or(MapError::Bad(ln, "unknown kind"))?;
            m.rows.push(MapRow { class: f[0].into(), source: f[1].into(), pset: f[2].into(), property: f[3].into(), kind });
            if m.rows.len() > MAX_ROWS {
                return Err(MapError::TooMany);
            }
        }
        m.check().map_err(|e| match e {
            MapError::Bad(_, w) => MapError::Bad(0, w),
            o => o,
        })?;
        Ok(m)
    }
}
