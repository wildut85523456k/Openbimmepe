//! Type catalogue: a table of named family types kept as plain tab-separated text (no JSON), so a manufacturer's
//! range or a company standard can be edited in a spreadsheet and loaded into families.
//!
//! ```text
//! # comment
//! family<TAB>Door, single flush
//! type<TAB>width<TAB>height<TAB>swing
//! D09<TAB>0.9<TAB>2.1<TAB>left
//! D12<TAB>1.2<TAB>2.1<TAB>
//! ```
//! An empty cell leaves the family's own default in place. Numbers and yes/no values (0, 1, true, false, yes, no)
//! go to number and yes/no inputs; anything else goes to choice, text and material inputs.
//! No type is shipped: the catalogue is typed in or imported by the user.
use crate::{Family, FamilyError, SharedSet, TypeDef, ValueDef, ValueKind};
use std::fmt;

pub const MAX_COLUMNS: usize = 500;
pub const MAX_ROWS: usize = 10_000;
pub const MAX_TYPE_NAME: usize = 128;
pub const MAX_CELL: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub family: String,
    pub columns: Vec<String>,
    /// (type name, one cell per column; empty = not set)
    pub rows: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Catalogue {
    pub tables: Vec<Table>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CatError {
    Syntax { line: usize, why: &'static str },
    Text { line: usize, err: FamilyError },
    UnknownFamily(String),
    UnknownColumn { family: String, column: String },
    NotAnInput { family: String, column: String },
    BadNumber { family: String, type_name: String, column: String },
    DuplicateType { family: String, type_name: String },
    TooMany(&'static str),
}

impl fmt::Display for CatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CatError::*;
        match self {
            Syntax { line, why } => write!(f, "line {line}: {why}"),
            Text { line, err } => write!(f, "line {line}: {err}"),
            UnknownFamily(n) => write!(f, "the catalogue lists family '{n}', which is not loaded"),
            UnknownColumn { family, column } => write!(f, "family '{family}' has no input '{column}'"),
            NotAnInput { family, column } => write!(f, "'{column}' of family '{family}' is calculated, not an input"),
            BadNumber { family, type_name, column } => write!(f, "type '{type_name}' of '{family}': '{column}' needs a number or yes/no"),
            DuplicateType { family, type_name } => write!(f, "family '{family}' already has a type named '{type_name}'"),
            TooMany(w) => write!(f, "too many {w}"),
        }
    }
}
impl std::error::Error for CatError {}

fn cell_ok(line: usize, s: &str, max: usize) -> Result<(), CatError> {
    crate::clean_text("catalogue", s, max).map_err(|err| CatError::Text { line, err })
}

impl Catalogue {
    pub fn from_text(text: &str) -> Result<Catalogue, CatError> {
        let mut cat = Catalogue::default();
        let mut want_header = false;
        for (i, line) in text.lines().enumerate() {
            let ln = i + 1;
            if line.trim().is_empty() || line.starts_with('#') { continue; }
            let f: Vec<&str> = line.split('\t').collect();
            if f[0] == "family" {
                if f.len() != 2 || f[1].is_empty() { return Err(CatError::Syntax { line: ln, why: "a family line is 'family', a tab and the family name" }); }
                cell_ok(ln, f[1], MAX_TYPE_NAME)?;
                if cat.tables.iter().any(|t| t.family == f[1]) { return Err(CatError::Syntax { line: ln, why: "this family is listed twice" }); }
                cat.tables.push(Table { family: f[1].to_string(), columns: vec![], rows: vec![] });
                want_header = true;
                continue;
            }
            let Some(t) = cat.tables.last_mut() else { return Err(CatError::Syntax { line: ln, why: "a 'family' line must come first" }) };
            if want_header {
                if f[0] != "type" { return Err(CatError::Syntax { line: ln, why: "the line after 'family' must start with 'type' and list the input names" }); }
                if f.len() < 2 { return Err(CatError::Syntax { line: ln, why: "list at least one input name" }); }
                if f.len() - 1 > MAX_COLUMNS { return Err(CatError::TooMany("columns")); }
                for (k, c) in f[1..].iter().enumerate() {
                    if !crate::valid_name(c) { return Err(CatError::Syntax { line: ln, why: "an input name is not valid" }); }
                    if f[1..k + 1].contains(c) { return Err(CatError::Syntax { line: ln, why: "an input name is listed twice" }); }
                }
                t.columns = f[1..].iter().map(|s| s.to_string()).collect();
                want_header = false;
                continue;
            }
            if f.len() != t.columns.len() + 1 { return Err(CatError::Syntax { line: ln, why: "a row needs a type name and one cell per input" }); }
            if f[0].is_empty() { return Err(CatError::Syntax { line: ln, why: "a type needs a name" }); }
            cell_ok(ln, f[0], MAX_TYPE_NAME)?;
            for c in &f[1..] { cell_ok(ln, c, MAX_CELL)?; }
            if t.rows.iter().any(|(n, _)| n == f[0]) { return Err(CatError::Syntax { line: ln, why: "this type name is used twice" }); }
            if t.rows.len() >= MAX_ROWS { return Err(CatError::TooMany("types in one family")); }
            t.rows.push((f[0].to_string(), f[1..].iter().map(|s| s.to_string()).collect()));
        }
        if want_header { return Err(CatError::Syntax { line: text.lines().count().max(1), why: "a family has no header line" }); }
        Ok(cat)
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for t in &self.tables {
            s.push_str(&format!("family\t{}\ntype\t{}\n", t.family, t.columns.join("\t")));
            for (n, cells) in &t.rows { s.push_str(&format!("{}\t{}\n", n, cells.join("\t"))); }
        }
        s
    }

    /// The catalogue of the types the families already have.
    pub fn export(families: &[Family]) -> Catalogue {
        let mut cat = Catalogue::default();
        for f in families {
            if f.types.is_empty() { continue; }
            let mut columns: Vec<String> = Vec::new();
            for t in &f.types {
                let names = t.values.iter().map(|(n, _)| n).chain(t.texts.iter().map(|(n, _)| n));
                for n in names { if !columns.contains(n) { columns.push(n.clone()); } }
            }
            let rows = f.types.iter().map(|t| {
                let cells = columns.iter().map(|c| {
                    if let Some((_, v)) = t.values.iter().find(|(n, _)| n == c) { v.to_string() }
                    else if let Some((_, v)) = t.texts.iter().find(|(n, _)| n == c) { v.clone() }
                    else { String::new() }
                }).collect();
                (t.name.clone(), cells)
            }).collect();
            cat.tables.push(Table { family: f.name.clone(), columns, rows });
        }
        cat
    }

    /// Add the catalogue's types to the families (matched by name). Nothing changes if any table is refused.
    pub fn apply(&self, families: &mut [Family], shared: Option<&SharedSet>) -> Result<(), CatError> {
        let mut staged: Vec<(usize, Vec<TypeDef>)> = Vec::new();
        for t in &self.tables {
            let k = families.iter().position(|f| f.name == t.family).ok_or_else(|| CatError::UnknownFamily(t.family.clone()))?;
            let fam = &families[k];
            let def_of = |col: &str| -> Option<&ValueDef> {
                fam.values.iter().find(|v| v.name == col).or_else(|| fam.shared.iter().find(|n| *n == col).and_then(|n| shared?.get(n)).map(|p| &p.def))
            };
            let mut kinds = Vec::new();
            for c in &t.columns {
                let d = def_of(c).ok_or_else(|| CatError::UnknownColumn { family: t.family.clone(), column: c.clone() })?;
                kinds.push(match d.kind {
                    ValueKind::Input { .. } | ValueKind::Flag { .. } => true,
                    ValueKind::Choice { .. } | ValueKind::Text { .. } | ValueKind::Material { .. } => false,
                    ValueKind::Formula(_) => return Err(CatError::NotAnInput { family: t.family.clone(), column: c.clone() }),
                });
            }
            let mut types = Vec::new();
            for (name, cells) in &t.rows {
                if fam.types.iter().any(|x| x.name == *name) { return Err(CatError::DuplicateType { family: t.family.clone(), type_name: name.clone() }); }
                let mut td = TypeDef { name: name.clone(), values: vec![], texts: vec![], lighting: None };
                for ((c, cell), numeric) in t.columns.iter().zip(cells).zip(&kinds) {
                    if cell.is_empty() { continue; }
                    if *numeric {
                        let v = match cell.as_str() {
                            "true" | "yes" => 1.0,
                            "false" | "no" => 0.0,
                            s => s.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| CatError::BadNumber { family: t.family.clone(), type_name: name.clone(), column: c.clone() })?,
                        };
                        td.values.push((c.clone(), v));
                    } else {
                        td.texts.push((c.clone(), cell.clone()));
                    }
                }
                types.push(td);
            }
            staged.push((k, types));
        }
        for (k, types) in staged { families[k].types.extend(types); }
        Ok(())
    }
}
