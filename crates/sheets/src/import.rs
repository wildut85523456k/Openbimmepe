//! Sheet-set generation from an Excel file. The first row names the columns; a column whose name matches
//! (ignoring case, spaces and punctuation) a sheet property or a title block field fills it. Each further
//! row is one sheet. The sheet number is the key: an existing number is updated, a new one is created.
//!
//! Safe by default: nothing is applied unless every row is accepted (`atomic`), cells are length- and
//! character-checked, and the file size, rows and columns are capped.

use std::io::Cursor;

use calamine::{Data, Reader};

use crate::model::{is_builtin, same, valid_number, Params, Sheet, SheetSet, Viewport};
use crate::visibility::ViewLibrary;
use crate::{SheetError, MAX_TEXT};

pub const MAX_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_ROWS: usize = 10_000;
pub const MAX_COLUMNS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// a number already in the set is an error
    CreateOnly,
    /// a number already in the set is updated from the cells that are filled
    CreateOrUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unmatched {
    Ignore,
    /// keep the column as a sheet parameter named as its header
    AsParameters,
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub mode: Mode,
    pub unmatched: Unmatched,
    /// name of the worksheet to read; the first one when empty
    pub worksheet: Option<String>,
    /// title block for rows that name neither a title block nor a size
    pub default_titleblock: Option<String>,
    /// apply nothing when any row is refused
    pub atomic: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions { mode: Mode::CreateOrUpdate, unmatched: Unmatched::Ignore, worksheet: None, default_titleblock: None, atomic: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Number,
    Name,
    Titleblock,
    Size,
    Scale,
    View,
    /// a title block field, under its exact name
    Field(String),
    /// a column kept as a sheet parameter under its own header (`Unmatched::AsParameters`)
    Param(String),
    Ignored,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnMap {
    pub column: usize,
    pub header: String,
    pub target: Target,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowError {
    /// the row as numbered in the spreadsheet (header is row 1)
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub columns: Vec<ColumnMap>,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub errors: Vec<RowError>,
    pub warnings: Vec<String>,
    /// false when nothing was applied because of errors
    pub applied: bool,
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn intrinsic(header: &str) -> Option<Target> {
    match norm(header).as_str() {
        "sheetnumber" | "number" | "sheetno" | "sheetnum" | "drawingnumber" | "drawingno" | "dwgno" => Some(Target::Number),
        "sheetname" | "name" | "title" | "drawingtitle" | "sheettitle" => Some(Target::Name),
        "titleblock" | "titleblockname" | "template" => Some(Target::Titleblock),
        "sheetsize" | "size" | "papersize" | "paper" => Some(Target::Size),
        "scale" => Some(Target::Scale),
        "view" | "viewname" | "placeview" => Some(Target::View),
        _ => None,
    }
}

/// The column names an import file should have: the sheet properties and every title block field that is
/// not computed by the program.
pub fn expected_columns(set: &SheetSet) -> Vec<String> {
    let mut cols: Vec<String> = ["Sheet Number", "Sheet Name", "Sheet Size", "Scale", "View"].iter().map(|s| s.to_string()).collect();
    for tb in set.titleblocks() {
        for f in tb.fields() {
            if !is_builtin(&f) && !cols.iter().any(|c| same(c, &f)) {
                cols.push(f);
            }
        }
    }
    cols
}

fn civil(days_since_1970: i64) -> (i64, u32, u32) {
    let z = days_since_1970 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn cell_text(c: &Data) -> Result<String, String> {
    let s = match c {
        Data::Empty => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.trim().to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            if !f.is_finite() {
                return Err("a number that is not finite".into());
            }
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                format!("{f}")
            }
        }
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => {
            let v = d.as_f64();
            if !v.is_finite() || !(1.0..2_958_466.0).contains(&v) {
                return Err("a date outside the supported range".into());
            }
            let (y, m, dd) = civil(v.floor() as i64 - 25_569);
            format!("{y:04}-{m:02}-{dd:02}")
        }
        Data::Error(e) => return Err(format!("a spreadsheet error value ({e:?})")),
    };
    if s.chars().count() > MAX_TEXT || s.chars().any(|c| c.is_control()) {
        return Err("text that is too long or has control characters".into());
    }
    Ok(s)
}

fn parse_scale(t: &str) -> Option<u32> {
    let t = t.trim();
    let n = t.strip_prefix("1:").or_else(|| t.strip_prefix("1 :")).unwrap_or(t).trim();
    n.parse::<u32>().ok().filter(|v| *v >= 1 && *v <= crate::model::MAX_SCALE)
}

/// Read an Excel file (xlsx, xlsm, xls, ods) and create or update sheets in `set`.
pub fn import_xlsx(set: &mut SheetSet, lib: &ViewLibrary, bytes: &[u8], opts: &ImportOptions) -> Result<ImportReport, SheetError> {
    if bytes.len() > MAX_BYTES {
        return Err(SheetError::Import(format!("file is larger than {} MB", MAX_BYTES / 1024 / 1024)));
    }
    let mut wb = calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| SheetError::Import(format!("cannot open the workbook: {e}")))?;
    let names = wb.sheet_names();
    let ws = match &opts.worksheet {
        Some(n) => {
            if !names.iter().any(|x| x == n) {
                return Err(SheetError::Unknown { what: "worksheet", value: n.clone() });
            }
            n.clone()
        }
        None => names.first().cloned().ok_or_else(|| SheetError::Import("the workbook has no worksheets".into()))?,
    };
    let range = wb.worksheet_range(&ws).map_err(|e| SheetError::Import(format!("cannot read worksheet '{ws}': {e}")))?;
    let (h, w) = range.get_size();
    if h == 0 || w == 0 {
        return Err(SheetError::Import(format!("worksheet '{ws}' is empty")));
    }
    if h > MAX_ROWS + 1 || w > MAX_COLUMNS {
        return Err(SheetError::Import(format!("worksheet '{ws}' has more than {MAX_ROWS} rows or {MAX_COLUMNS} columns")));
    }
    let origin_row = range.start().map_or(0, |s| s.0 as usize);
    let rows: Vec<&[Data]> = range.rows().collect();

    // columns
    let known: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        for tb in set.titleblocks() {
            for f in tb.fields() {
                if !v.iter().any(|x| same(x, &f)) {
                    v.push(f);
                }
            }
        }
        v
    };
    let mut rep = ImportReport::default();
    for (i, c) in rows[0].iter().enumerate() {
        let header = cell_text(c).map_err(|m| SheetError::Import(format!("header cell {}: {m}", i + 1)))?;
        if header.is_empty() {
            continue;
        }
        let target = if let Some(t) = intrinsic(&header) {
            t
        } else if let Some(f) = known.iter().find(|f| norm(f) == norm(&header)) {
            if is_builtin(f) {
                rep.warnings.push(format!("column '{header}' is computed by the program and is ignored"));
                Target::Ignored
            } else {
                Target::Field(f.clone())
            }
        } else if opts.unmatched == Unmatched::AsParameters && !is_builtin(&header) {
            Target::Param(header.clone())
        } else {
            rep.warnings.push(format!("column '{header}' matches no sheet property or title block field and is ignored"));
            Target::Ignored
        };
        rep.columns.push(ColumnMap { column: i, header, target });
    }
    // two columns for one target is ambiguous
    for (i, a) in rep.columns.iter().enumerate() {
        if a.target == Target::Ignored {
            continue;
        }
        if let Some(b) = rep.columns[i + 1..].iter().find(|b| b.target == a.target) {
            return Err(SheetError::Import(format!("columns '{}' and '{}' both fill the same field", a.header, b.header)));
        }
    }
    if !rep.columns.iter().any(|c| c.target == Target::Number) {
        return Err(SheetError::Import("no column holds the sheet number (expected a header such as 'Sheet Number')".into()));
    }

    let mut work = set.clone();
    for (ri, row) in rows.iter().enumerate().skip(1) {
        let rownum = origin_row + ri + 1;
        let mut err = |m: String| rep.errors.push(RowError { row: rownum, message: m });
        let mut cells: Vec<(&ColumnMap, String)> = Vec::new();
        let mut bad = false;
        for c in &rep.columns {
            let raw = row.get(c.column).cloned().unwrap_or(Data::Empty);
            match cell_text(&raw) {
                Ok(t) => cells.push((c, t)),
                Err(m) => {
                    err(format!("column '{}': {m}", c.header));
                    bad = true;
                }
            }
        }
        if bad {
            continue;
        }
        if cells.iter().all(|(_, t)| t.is_empty()) {
            continue;
        }
        let get = |t: &Target| cells.iter().find(|(c, _)| &c.target == t).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty());
        let Some(number) = get(&Target::Number) else {
            err("the sheet number is empty".into());
            continue;
        };
        if !valid_number(number) {
            err(format!("sheet number '{number}' is empty, too long or has forbidden characters (\\ / : * ? \" < > |)"));
            continue;
        }
        let existing = work.sheet(number).cloned();
        if existing.is_some() && opts.mode == Mode::CreateOnly {
            err(format!("sheet {number} already exists"));
            continue;
        }
        // title block
        let tb_name: Result<Option<String>, String> = if let Some(n) = get(&Target::Titleblock) {
            if work.titleblock(n).is_some() {
                Ok(Some(n.to_string()))
            } else {
                Err(format!("no title block called '{n}'"))
            }
        } else if let Some(sz) = get(&Target::Size) {
            let parsed = crate::paper::PaperSize::parse(sz);
            let found: Vec<&str> = work
                .titleblocks()
                .iter()
                .filter(|t| parsed.as_ref().is_some_and(|p| (p.width_mm - t.size.width_mm).abs() < 0.5 && (p.height_mm - t.size.height_mm).abs() < 0.5) || t.size.name.eq_ignore_ascii_case(sz))
                .map(|t| t.name.as_str())
                .collect();
            match found.first() {
                Some(n) => Ok(Some((*n).to_string())),
                None => Err(format!("no title block for size '{sz}'")),
            }
        } else {
            Ok(None)
        };
        let tb_name = match tb_name {
            Ok(v) => v,
            Err(m) => {
                err(m);
                continue;
            }
        };
        let mut sheet = match existing {
            Some(s) => s,
            None => {
                let tb = tb_name.clone().or_else(|| opts.default_titleblock.clone()).or_else(|| (work.titleblocks().len() == 1).then(|| work.titleblocks()[0].name.clone()));
                let Some(tb) = tb else {
                    err("no title block, size or default title block given".into());
                    continue;
                };
                let Some(name) = get(&Target::Name) else {
                    err("the sheet name is empty".into());
                    continue;
                };
                Sheet::new(number, name, &tb)
            }
        };
        if let Some(n) = get(&Target::Name) {
            sheet.name = n.to_string();
        }
        if let Some(t) = tb_name {
            sheet.titleblock = t;
        }
        let mut params: Params = sheet.params.clone();
        for (c, v) in &cells {
            if v.is_empty() {
                continue;
            }
            match &c.target {
                Target::Field(f) | Target::Param(f) => {
                    params.retain(|k, _| !same(k, f));
                    params.insert(f.clone(), v.clone());
                }
                _ => {}
            }
        }
        sheet.params = params;
        if let Some(vn) = get(&Target::View) {
            if lib.view(vn).is_none() {
                rep.warnings.push(format!("row {rownum}: view '{vn}' is not defined; sheet {number} has no viewport"));
            } else if let Some(tb) = work.titleblock(&sheet.titleblock) {
                let scale = match get(&Target::Scale) {
                    Some(t) => match parse_scale(t) {
                        Some(s) => s,
                        None => {
                            rep.errors.push(RowError { row: rownum, message: format!("scale '{t}' is not like 1:100") });
                            continue;
                        }
                    },
                    None => 100,
                };
                let f = tb.free_rect;
                sheet.viewports = vec![Viewport {
                    view: vn.to_string(),
                    centre: f.centre(),
                    size: [f.width() - 10.0, f.height() - 18.0],
                    scale,
                    model_centre: [0.0, 0.0],
                    title: None,
                }];
            }
        } else if let Some(t) = get(&Target::Scale) {
            match parse_scale(t) {
                Some(s) => sheet.viewports.iter_mut().for_each(|v| v.scale = s),
                None => {
                    rep.errors.push(RowError { row: rownum, message: format!("scale '{t}' is not like 1:100") });
                    continue;
                }
            }
        }
        let was_there = work.sheet(number).is_some();
        let r = if was_there { work.replace_sheet(sheet) } else { work.add_sheet(sheet) };
        match r {
            Ok(()) => {
                if was_there {
                    rep.updated.push(number.to_string());
                } else {
                    rep.created.push(number.to_string());
                }
            }
            Err(e) => rep.errors.push(RowError { row: rownum, message: e.to_string() }),
        }
    }
    if rep.errors.is_empty() || !opts.atomic {
        *set = work;
        rep.applied = true;
    } else {
        rep.created.clear();
        rep.updated.clear();
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::norm;

    #[test]
    fn digits_distinguish_headers() {
        assert_ne!(norm("Zone 1"), norm("Zone 2"));
        assert_eq!(norm("Zone 1"), norm("zone-1"));
        assert_eq!(norm("  Sheet_Number "), "sheetnumber");
    }
}
