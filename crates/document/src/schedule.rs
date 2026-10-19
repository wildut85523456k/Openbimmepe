//! Schedules/Quantities, as Revit's Schedule views have them: a schedule lists the elements of one category, a row for
//! each (or one row for those that look alike), with the fields the designer chose, filtered, sorted and grouped, with
//! footers and grand totals. The definition is saved with the model; the table is worked out from the model whenever
//! it is asked for, so it is always the model as it is now.
//!
//! Lengths are shown in millimetres, areas in square metres and volumes in cubic metres, as in the rest of the app.
use crate::{Category, Command, Document, Element, ElementId, Geometry, InsulationKind, MepNode, SlabKind};
use openbim_family::mep::Category as Fam;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const MAX_SCHEDULES: usize = 200;
pub const MAX_FIELDS: usize = 40;
pub const MAX_FILTERS: usize = 8;
pub const MAX_SORTS: usize = 4;

/// The category a schedule lists, named as Revit names its categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScheduleKind {
    Walls, Floors, Roofs, Ceilings, Foundations, Columns, Beams, Stairs, Railings, Rooms, Doors, Windows, Furniture,
    GenericModels, MechanicalEquipment, AirTerminals, DuctAccessories, PipeAccessories, PlumbingFixtures,
    LightingFixtures, ElectricalEquipment, ElectricalFixtures, Sprinklers, Ducts, Pipes, DuctFittings, PipeFittings,
}

impl ScheduleKind {
    pub const ALL: [ScheduleKind; 27] = {
        use ScheduleKind::*;
        [Walls, Floors, Roofs, Ceilings, Foundations, Columns, Beams, Stairs, Railings, Rooms, Doors, Windows, Furniture,
         GenericModels, MechanicalEquipment, AirTerminals, DuctAccessories, PipeAccessories, PlumbingFixtures,
         LightingFixtures, ElectricalEquipment, ElectricalFixtures, Sprinklers, Ducts, Pipes, DuctFittings, PipeFittings]
    };
    pub fn name(self) -> &'static str {
        use ScheduleKind::*;
        match self {
            Walls => "Walls", Floors => "Floors", Roofs => "Roofs", Ceilings => "Ceilings", Foundations => "Structural Foundations",
            Columns => "Structural Columns", Beams => "Structural Framing", Stairs => "Stairs", Railings => "Railings", Rooms => "Rooms",
            Doors => "Doors", Windows => "Windows", Furniture => "Furniture", GenericModels => "Generic Models",
            MechanicalEquipment => "Mechanical Equipment", AirTerminals => "Air Terminals", DuctAccessories => "Duct Accessories",
            PipeAccessories => "Pipe Accessories", PlumbingFixtures => "Plumbing Fixtures", LightingFixtures => "Lighting Fixtures",
            ElectricalEquipment => "Electrical Equipment", ElectricalFixtures => "Electrical Fixtures", Sprinklers => "Sprinklers",
            Ducts => "Ducts", Pipes => "Pipes", DuctFittings => "Duct Fittings", PipeFittings => "Pipe Fittings",
        }
    }
    /// The code written in the file.
    pub fn code(self) -> &'static str {
        use ScheduleKind::*;
        match self {
            Walls => "wall", Floors => "floor", Roofs => "roof", Ceilings => "ceiling", Foundations => "found", Columns => "column",
            Beams => "beam", Stairs => "stair", Railings => "railing", Rooms => "room", Doors => "door", Windows => "window",
            Furniture => "furn", GenericModels => "generic", MechanicalEquipment => "mech", AirTerminals => "airterm",
            DuctAccessories => "ductacc", PipeAccessories => "pipeacc", PlumbingFixtures => "plumb", LightingFixtures => "light",
            ElectricalEquipment => "elecequ", ElectricalFixtures => "elecfix", Sprinklers => "sprink", Ducts => "duct", Pipes => "pipe",
            DuctFittings => "ductfit", PipeFittings => "pipefit",
        }
    }
    pub fn from_code(code: &str) -> Option<ScheduleKind> { Self::ALL.into_iter().find(|k| k.code() == code) }
    /// A new schedule of this category is called, as in Revit, "Wall Schedule" (the singular of the category).
    fn default_title(self) -> String {
        let n = self.name();
        let singular = match n.strip_suffix('s') { Some(s) if !n.ends_with("ss") => s.to_string(), _ => n.to_string() };
        format!("{singular} Schedule")
    }
    fn is_family(self) -> bool { !matches!(self, Self::Walls | Self::Floors | Self::Roofs | Self::Ceilings | Self::Foundations | Self::Columns | Self::Beams | Self::Stairs | Self::Railings | Self::Rooms | Self::Ducts | Self::Pipes | Self::DuctFittings | Self::PipeFittings) }
}

/// What a field holds, and so how it is shown, sorted, filtered and totalled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType { Text, Id, Count, Length, Area, Volume, Number }

impl FieldType {
    /// Whether a field of this type holds numbers (filters compare them as numbers).
    pub fn numeric(self) -> bool { self != FieldType::Text }
    /// Whether the values of a field of this type can be added up.
    pub fn summable(self) -> bool { matches!(self, FieldType::Count | FieldType::Length | FieldType::Area | FieldType::Volume | FieldType::Number) }
    pub fn unit(self) -> &'static str { match self { FieldType::Length => "mm", FieldType::Area => "m²", FieldType::Volume => "m³", _ => "" } }
    /// The value as the schedule shows it (the model's lengths are in metres).
    pub fn to_display(self, v: f64) -> f64 { if self == FieldType::Length { v * 1000.0 } else { v } }
    fn decimals(self) -> usize { match self { FieldType::Length => 0, FieldType::Area | FieldType::Number => 2, FieldType::Volume => 3, _ => 0 } }
    /// The text of a value of this type.
    pub fn format(self, v: &Value) -> String {
        match v {
            Value::Blank => String::new(),
            Value::Text(t) => t.clone(),
            Value::Num(x) => {
                let s = format!("{:.*}", self.decimals(), self.to_display(*x));
                if s.starts_with("-0") && s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') { s.trim_start_matches('-').to_string() } else { s }
            }
        }
    }
}

/// A cell of a schedule.
#[derive(Clone, Debug, PartialEq)]
pub enum Value { Blank, Text(String), Num(f64) }

/// A field a schedule of some category can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldInfo { pub key: &'static str, pub label: &'static str, pub ty: FieldType }

const fn fi(key: &'static str, label: &'static str, ty: FieldType) -> FieldInfo { FieldInfo { key, label, ty } }

/// The fields a schedule of `kind` can show, in the order the Fields list gives them.
pub fn fields(kind: ScheduleKind) -> Vec<FieldInfo> {
    use FieldType::*;
    use ScheduleKind::*;
    let mut v = match kind {
        Walls => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Base Constraint", Text), fi("base_offset", "Base Offset", Length), fi("length", "Length", Length), fi("height", "Unconnected Height", Length), fi("width", "Width", Length), fi("area", "Area", Area), fi("volume", "Volume", Volume)],
        Floors | Roofs | Ceilings => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Level", Text), fi("offset", "Height Offset From Level", Length), fi("thickness", "Thickness", Length), fi("perimeter", "Perimeter", Length), fi("area", "Area", Area), fi("volume", "Volume", Volume), fi("material", "Structural Material", Text)],
        Foundations => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Level", Text), fi("length", "Length", Length), fi("width", "Width", Length), fi("thickness", "Thickness", Length), fi("area", "Area", Area), fi("volume", "Volume", Volume), fi("material", "Structural Material", Text)],
        Columns => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Base Level", Text), fi("top_level", "Top Level", Text), fi("base_offset", "Base Offset", Length), fi("width", "Width", Length), fi("depth", "Depth", Length), fi("length", "Length", Length), fi("volume", "Volume", Volume), fi("material", "Structural Material", Text)],
        Beams => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Reference Level", Text), fi("offset", "Start Level Offset", Length), fi("width", "Width", Length), fi("depth", "Depth", Length), fi("length", "Length", Length), fi("volume", "Volume", Volume), fi("material", "Structural Material", Text), fi("usage", "Structural Usage", Text)],
        Stairs => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Base Level", Text), fi("width", "Actual Run Width", Length), fi("length", "Run Length", Length), fi("height", "Height", Length), fi("risers", "Actual Number of Risers", Count), fi("treads", "Actual Number of Treads", Count)],
        Railings => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Base Level", Text), fi("length", "Length", Length), fi("height", "Height", Length)],
        Rooms => vec![fi("name", "Name", Text), fi("level", "Level", Text), fi("area", "Area", Area), fi("perimeter", "Perimeter", Length)],
        Doors | Windows => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Level", Text), fi("mark", "Mark", Text), fi("host", "Host", Text), fi("width", "Width", Length), fi("height", "Height", Length), fi("sill", "Sill Height", Length), fi("head", "Head Height", Length), fi("area", "Area", Area)],
        Ducts => vec![fi("system", "System Type", Text), fi("type", "Type", Text), fi("level", "Reference Level", Text), fi("size", "Size", Text), fi("width", "Width", Length), fi("height", "Height", Length), fi("length", "Length", Length), fi("area", "Area", Area), fi("offset", "Offset", Length), fi("slope", "Slope (%)", Number), fi("insulation_type", "Insulation Type", Text), fi("insulation_thickness", "Insulation Thickness", Length), fi("lining_type", "Lining Type", Text), fi("lining_thickness", "Lining Thickness", Length)],
        Pipes => vec![fi("system", "System Type", Text), fi("type", "Type", Text), fi("level", "Reference Level", Text), fi("size", "Size", Text), fi("diameter", "Diameter", Length), fi("length", "Length", Length), fi("area", "Area", Area), fi("offset", "Offset", Length), fi("slope", "Slope (%)", Number), fi("insulation_type", "Insulation Type", Text), fi("insulation_thickness", "Insulation Thickness", Length)],
        DuctFittings | PipeFittings => vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("part", "Part Number", Text), fi("shape", "Fitting", Text), fi("angle", "Angle (°)", Number), fi("system", "System Type", Text), fi("size", "Size", Text)],
        _ => {
            let mut f = vec![fi("family", "Family", Text), fi("type", "Type", Text), fi("level", "Level", Text), fi("mark", "Mark", Text), fi("offset", "Elevation from Level", Length)];
            if !matches!(kind, Furniture | GenericModels) { f.push(fi("system", "System Name", Text)); }
            f
        }
    };
    v.push(fi("count", "Count", Count));
    if !matches!(kind, DuctFittings | PipeFittings) { v.push(fi("id", "Element Id", Id)); }
    v
}

/// The field `key` of `kind`, if there is one.
pub fn field_info(kind: ScheduleKind, key: &str) -> Option<FieldInfo> { fields(kind).into_iter().find(|f| f.key == key) }

/// A field of a schedule: which one, what its column is headed, whether it is shown, and whether it is added up.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleField {
    pub key: String,
    /// the column heading; empty for the field's own name
    pub heading: String,
    /// a hidden field is not shown, but still sorts, filters and groups
    pub hidden: bool,
    /// Calculate totals: a footer and the grand total add the field up, and rows that look alike add up into one
    pub totals: bool,
}

impl ScheduleField {
    pub fn new(key: &str) -> Self { ScheduleField { key: key.into(), heading: String::new(), hidden: false, totals: false } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOp { Equal, NotEqual, Greater, GreaterEqual, Less, LessEqual, Contains, NotContains, BeginsWith, NotBeginsWith, EndsWith, NotEndsWith }

impl FilterOp {
    pub const ALL: [FilterOp; 12] = [FilterOp::Equal, FilterOp::NotEqual, FilterOp::Greater, FilterOp::GreaterEqual, FilterOp::Less, FilterOp::LessEqual, FilterOp::Contains, FilterOp::NotContains, FilterOp::BeginsWith, FilterOp::NotBeginsWith, FilterOp::EndsWith, FilterOp::NotEndsWith];
    pub fn name(self) -> &'static str {
        match self {
            FilterOp::Equal => "equals", FilterOp::NotEqual => "does not equal", FilterOp::Greater => "is greater than", FilterOp::GreaterEqual => "is greater than or equal to",
            FilterOp::Less => "is less than", FilterOp::LessEqual => "is less than or equal to", FilterOp::Contains => "contains", FilterOp::NotContains => "does not contain",
            FilterOp::BeginsWith => "begins with", FilterOp::NotBeginsWith => "does not begin with", FilterOp::EndsWith => "ends with", FilterOp::NotEndsWith => "does not end with",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            FilterOp::Equal => "eq", FilterOp::NotEqual => "ne", FilterOp::Greater => "gt", FilterOp::GreaterEqual => "ge", FilterOp::Less => "lt", FilterOp::LessEqual => "le",
            FilterOp::Contains => "has", FilterOp::NotContains => "nhas", FilterOp::BeginsWith => "beg", FilterOp::NotBeginsWith => "nbeg", FilterOp::EndsWith => "end", FilterOp::NotEndsWith => "nend",
        }
    }
    pub fn from_code(c: &str) -> Option<FilterOp> { Self::ALL.into_iter().find(|o| o.code() == c) }
    /// The operators that apply to a field of type `ty`: numbers compare, text also matches parts.
    pub fn applies(self, ty: FieldType) -> bool {
        !ty.numeric() || matches!(self, FilterOp::Equal | FilterOp::NotEqual | FilterOp::Greater | FilterOp::GreaterEqual | FilterOp::Less | FilterOp::LessEqual)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleFilter {
    pub field: String,
    pub op: FilterOp,
    /// text, or a number in the units the schedule shows (millimetres, square metres...)
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleSort {
    pub field: String,
    pub descending: bool,
    /// a line with the value above each group
    pub header: bool,
    /// a line below each group with its count and totals
    pub footer: bool,
    /// an empty line after each group
    pub blank: bool,
}

impl ScheduleSort {
    pub fn new(field: &str) -> Self { ScheduleSort { field: field.into(), descending: false, header: false, footer: false, blank: false } }
}

/// A schedule: what it lists and how.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleDef {
    /// stable for the life of the model; 1 or more
    pub id: u64,
    pub name: String,
    pub kind: ScheduleKind,
    pub fields: Vec<ScheduleField>,
    pub filters: Vec<ScheduleFilter>,
    pub sorts: Vec<ScheduleSort>,
    /// Itemize every instance: a row for each element. Off, elements that look alike are one row.
    pub itemize: bool,
    /// a last row with the count and the totals of everything listed
    pub grand_totals: bool,
}

impl ScheduleDef {
    /// The fields a new schedule of `kind` starts with: the ones its quantities are taken from.
    pub fn starter(id: u64, name: &str, kind: ScheduleKind) -> ScheduleDef {
        use ScheduleKind::*;
        let (keys, totals): (&[&str], &[&str]) = match kind {
            Walls => (&["type", "level", "length", "height", "area", "volume", "count"], &["length", "area", "volume"]),
            Floors | Roofs | Ceilings => (&["type", "level", "thickness", "area", "volume", "count"], &["area", "volume"]),
            Foundations => (&["type", "level", "length", "width", "thickness", "volume", "count"], &["length", "volume"]),
            Columns => (&["type", "level", "top_level", "length", "volume", "count"], &["length", "volume"]),
            Beams => (&["type", "level", "length", "volume", "count"], &["length", "volume"]),
            Stairs => (&["type", "level", "width", "height", "risers", "count"], &[]),
            Railings => (&["type", "level", "length", "count"], &["length"]),
            Rooms => (&["name", "level", "area", "perimeter", "count"], &["area"]),
            Doors | Windows => (&["type", "level", "mark", "width", "height", "sill", "count"], &[]),
            Ducts => (&["system", "type", "size", "length", "area", "count"], &["length", "area"]),
            Pipes => (&["system", "type", "size", "length", "count"], &["length"]),
            DuctFittings | PipeFittings => (&["type", "shape", "size", "system", "count"], &[]),
            _ => (&["family", "type", "level", "mark", "count"], &[]),
        };
        let fields = keys.iter().map(|k| ScheduleField { totals: totals.contains(k), ..ScheduleField::new(k) }).collect();
        let sorts = match kind { Rooms | Doors | Windows => vec![ScheduleSort::new("level")], DuctFittings | PipeFittings => vec![ScheduleSort::new("type")], _ => vec![ScheduleSort::new("type")] };
        ScheduleDef { id, name: name.into(), kind, fields, filters: Vec::new(), sorts, itemize: matches!(kind, Rooms | Doors | Windows), grand_totals: true }
    }

    /// The heading of the column of a field.
    pub fn heading(&self, f: &ScheduleField) -> String {
        if f.heading.trim().is_empty() { field_info(self.kind, &f.key).map_or(f.key.clone(), |i| i.label.to_string()) } else { f.heading.clone() }
    }

    /// Whether the schedule can be saved: fields, filters and sorting that exist for its category, and sensible text.
    pub fn check(&self) -> Result<(), String> {
        let ok_name = |s: &str| s == s.trim() && !s.is_empty() && s.chars().count() <= 60;
        if self.id == 0 { return Err("a schedule has a number of 1 or more".into()); }
        if !ok_name(&self.name) { return Err("the name of a schedule is 1 to 60 characters without spaces at the ends".into()); }
        if self.fields.is_empty() { return Err("a schedule shows at least one field".into()); }
        if self.fields.len() > MAX_FIELDS { return Err(format!("a schedule has at most {MAX_FIELDS} fields")); }
        let mut seen = std::collections::BTreeSet::new();
        for f in &self.fields {
            let info = field_info(self.kind, &f.key).ok_or_else(|| format!("\"{}\" is not a field of {}", f.key, self.kind.name()))?;
            if !seen.insert(f.key.as_str()) { return Err(format!("the field \"{}\" is in the schedule twice", info.label)); }
            if f.heading.chars().count() > 60 || f.heading != f.heading.trim() { return Err("a heading is up to 60 characters without spaces at the ends".into()); }
            if f.totals && !info.ty.summable() { return Err(format!("\"{}\" cannot be added up", info.label)); }
        }
        if self.fields.iter().all(|f| f.hidden) { return Err("a schedule shows at least one field".into()); }
        if self.filters.len() > MAX_FILTERS { return Err(format!("a schedule has at most {MAX_FILTERS} filters")); }
        for fl in &self.filters {
            let info = self.fields.iter().find(|f| f.key == fl.field).and_then(|f| field_info(self.kind, &f.key)).ok_or_else(|| format!("filter by \"{}\": it is not a field of the schedule", fl.field))?;
            if !fl.op.applies(info.ty) { return Err(format!("\"{}\" is a number: it cannot be tested with \"{}\"", info.label, fl.op.name())); }
            if fl.value.chars().count() > 64 { return Err("a filter value is up to 64 characters".into()); }
            if info.ty.numeric() && !fl.value.trim().parse::<f64>().is_ok_and(f64::is_finite) { return Err(format!("filter by \"{}\": \"{}\" is not a number", info.label, fl.value)); }
        }
        if self.sorts.len() > MAX_SORTS { return Err(format!("a schedule sorts by at most {MAX_SORTS} fields")); }
        let mut sorted = std::collections::BTreeSet::new();
        for s in &self.sorts {
            if !self.fields.iter().any(|f| f.key == s.field) { return Err(format!("sort by \"{}\": it is not a field of the schedule", s.field)); }
            if !sorted.insert(s.field.as_str()) { return Err("a field is sorted by once".into()); }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind { Data, Header, Footer, Blank, Grand }

/// A line of a schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub kind: RowKind,
    /// the title of a header, footer or grand total
    pub label: String,
    /// the elements the row lists (a row of elements that look alike lists them all)
    pub ids: Vec<ElementId>,
    /// the shown fields, in order
    pub cells: Vec<Value>,
}

/// A schedule worked out from the model.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleTable {
    pub headings: Vec<String>,
    pub types: Vec<FieldType>,
    pub rows: Vec<TableRow>,
}

impl ScheduleTable {
    /// The text of a cell.
    pub fn text(&self, row: &TableRow, col: usize) -> String { self.types[col].format(&row.cells[col]) }

    /// The table as comma separated values: the headings (with units), then every line. Numbers are plain, so a
    /// spreadsheet can add them up; text that a spreadsheet would run as a formula is marked as text.
    pub fn to_csv(&self) -> String {
        fn quote(s: &str) -> String {
            let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
            if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
        }
        let mut out = String::new();
        let heads: Vec<String> = self.headings.iter().zip(&self.types).map(|(h, t)| quote(&if t.unit().is_empty() { h.clone() } else { format!("{h} ({})", t.unit()) })).collect();
        out.push_str(&heads.join(","));
        out.push('\n');
        for row in &self.rows {
            let mut cells: Vec<String> = row.cells.iter().zip(&self.types).map(|(v, t)| match v { Value::Text(s) => quote(s), _ => t.format(v) }).collect();
            if matches!(row.kind, RowKind::Header | RowKind::Footer | RowKind::Grand) && !cells.is_empty() {
                // the title goes in the first column the line has nothing of its own in
                if let Some(c) = cells.iter_mut().find(|c| c.is_empty()) { *c = quote(&row.label); }
            }
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the values

enum Item { El(ElementId), Fit(u64) }

struct FamFacts { kind: ScheduleKind, opening: Option<[f64; 3]> }

fn library() -> &'static [openbim_family::Compiled] {
    static LIB: std::sync::OnceLock<Vec<openbim_family::Compiled>> = std::sync::OnceLock::new();
    LIB.get_or_init(|| openbim_family::library::all().unwrap_or_default())
}

/// The category a family instance is scheduled and shown under (Doors, Windows, Mechanical Equipment...), from its family.
pub fn family_kind(e: &Element) -> ScheduleKind {
    let Some(f) = library().iter().find(|f| f.name() == e.family) else { return ScheduleKind::GenericModels };
    match f.category() {
        Fam::Door => ScheduleKind::Doors, Fam::Window => ScheduleKind::Windows, Fam::Furniture => ScheduleKind::Furniture,
        Fam::AirTerminal => ScheduleKind::AirTerminals, Fam::Damper => ScheduleKind::DuctAccessories, Fam::Valve => ScheduleKind::PipeAccessories,
        Fam::Fan | Fam::Pump | Fam::Boiler | Fam::Chiller | Fam::Coil | Fam::UnitaryEquipment | Fam::Tank => ScheduleKind::MechanicalEquipment,
        Fam::SanitaryTerminal => ScheduleKind::PlumbingFixtures, Fam::LightFixture => ScheduleKind::LightingFixtures,
        Fam::DistributionBoard => ScheduleKind::ElectricalEquipment, Fam::Outlet | Fam::SwitchingDevice => ScheduleKind::ElectricalFixtures,
        Fam::Sprinkler => ScheduleKind::Sprinklers,
        Fam::Generic | Fam::DuctFitting | Fam::PipeFitting | Fam::CableFitting => ScheduleKind::GenericModels,
    }
}

fn family_facts(e: &Element) -> FamFacts {
    let Some(f) = library().iter().find(|f| f.name() == e.family) else { return FamFacts { kind: ScheduleKind::GenericModels, opening: None } };
    let kind = family_kind(e);
    let opening = if matches!(kind, ScheduleKind::Doors | ScheduleKind::Windows) {
        f.solve(&openbim_family::Request {
            type_name: (!e.type_name.is_empty()).then_some(e.type_name.as_str()),
            overrides: e.overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect(),
            texts: e.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect(),
        }).ok().and_then(|i| i.opening)
    } else { None };
    FamFacts { kind, opening }
}

/// What the rows of one table are worked out from, found once.
struct Ctx {
    fams: BTreeMap<ElementId, FamFacts>,
    /// wall -> the area its openings take out
    cut: BTreeMap<ElementId, f64>,
    /// room -> (area, perimeter)
    rooms: BTreeMap<ElementId, (f64, f64)>,
}

impl Ctx {
    fn new(doc: &Document, kind: ScheduleKind) -> Ctx {
        let fams: BTreeMap<ElementId, FamFacts> = if kind.is_family() || kind == ScheduleKind::Walls {
            let list: Vec<&Element> = doc.elements().filter(|e| e.category == Category::Family).collect();
            list.par_iter().map(|e| (e.id, family_facts(e))).collect()
        } else { BTreeMap::new() };
        let mut cut: BTreeMap<ElementId, f64> = BTreeMap::new();
        if kind == ScheduleKind::Walls {
            let mut add = |host: ElementId, w: f64, h: f64, sill: f64| {
                let top = doc.get(host).map_or(0.0, |x| x.height_m);
                *cut.entry(host).or_default() += w.max(0.0) * h.min(top - sill).max(0.0);
            };
            for e in doc.elements().filter(|e| e.category == Category::Family) {
                if let (Some(Geometry::InWall { host, .. }), Some([w, h, sill])) = (e.geometry, fams.get(&e.id).and_then(|f| f.opening)) { add(host, w, h, sill + e.elevation_m); }
            }
            for (_, host, [_, w, sill, h]) in doc.wall_openings() { add(host, w, h, sill); }
        }
        let rooms = if kind == ScheduleKind::Rooms {
            let list: Vec<ElementId> = doc.elements().filter(|e| e.category == Category::Room).map(|e| e.id).collect();
            list.par_iter().filter_map(|id| doc.room_shape(*id).ok().map(|s| (*id, (s.area(), s.perimeter())))).collect()
        } else { BTreeMap::new() };
        Ctx { fams, cut, rooms }
    }
}

fn mm(x: f64) -> String { format!("{}", (x * 1000.0).round() as i64) }

fn txt(s: &str) -> Value { if s.is_empty() { Value::Blank } else { Value::Text(s.to_string()) } }

fn level_name(doc: &Document, id: ElementId) -> Value { doc.level_of(id).and_then(|l| doc.get(l)).map_or(Value::Blank, |l| txt(&l.name)) }

fn length(a: [f64; 2], b: [f64; 2]) -> f64 { (b[0] - a[0]).hypot(b[1] - a[1]) }

fn polygon_area(p: &[[f64; 2]]) -> f64 { openbim_geom::profile::polygon_area(p).abs() }

/// The type of an element: its type element, or what its sizes say, as Revit names a type it makes on the fly.
fn type_of(doc: &Document, e: &Element) -> Value {
    if let Some(t) = e.type_id.and_then(|t| doc.get(t)) { return txt(&t.name); }
    match e.category {
        Category::Wall => Value::Text(format!("Generic - {}mm", mm(e.width_m))),
        Category::Slab => Value::Text(format!("Generic {}mm", mm(e.width_m))),
        Category::Beam | Category::Column => Value::Text(if e.section == crate::Section::Circle { format!("{}mm dia", mm(e.width_m)) } else { format!("{} x {}mm", mm(e.width_m), mm(e.height_m)) }),
        Category::Footing | Category::WallFooting => Value::Text(format!("{} x {}mm", mm(e.width_m), mm(e.height_m))),
        Category::Family => Value::Text(if e.type_name.is_empty() { "Default".into() } else { e.type_name.clone() }),
        Category::Stair => Value::Text(format!("Stair {}mm", mm(e.width_m))),
        Category::Railing => Value::Text(format!("Railing {}mm", mm(e.height_m))),
        _ => Value::Blank,
    }
}

type Cells = Vec<(&'static str, Value)>;

fn num(x: f64) -> Value { Value::Num(x) }

fn element_cells(doc: &Document, ctx: &Ctx, kind: ScheduleKind, e: &Element) -> Cells {
    let lvl = level_name(doc, e.id);
    let ty = type_of(doc, e);
    let mut c: Cells = vec![("type", ty), ("level", lvl), ("count", num(1.0)), ("id", num(e.id.0 as f64))];
    let mut push = |k: &'static str, v: Value| c.push((k, v));
    match (kind, e.geometry) {
        (ScheduleKind::Walls, Some(Geometry::Linear { start, end, .. })) => {
            let len = length(start, end);
            let area = (len * e.height_m - ctx.cut.get(&e.id).copied().unwrap_or(0.0)).max(0.0);
            push("family", Value::Text("Basic Wall".into()));
            push("base_offset", num(e.elevation_m)); push("length", num(len)); push("height", num(e.height_m)); push("width", num(e.width_m));
            push("area", num(area)); push("volume", num(area * e.width_m));
        }
        (ScheduleKind::Floors | ScheduleKind::Roofs | ScheduleKind::Ceilings | ScheduleKind::Foundations, Some(Geometry::Area { .. })) => {
            let plan = if e.is_roof() { doc.roof_figures(e.id).map_or_else(|| polygon_area(&e.outline), |f| f.surface_m2) } else {
                // a shaft through the slab takes its area out
                let cuts = doc.cut_shapes(e.id);
                if cuts.is_empty() { polygon_area(&e.outline) } else { cuts.iter().map(|s| s.area()).sum() }
            };
            let per: f64 = (0..e.outline.len()).map(|i| length(e.outline[i], e.outline[(i + 1) % e.outline.len()])).sum();
            push("family", Value::Text(match e.slab_kind { SlabKind::Floor => "Floor", SlabKind::Roof => "Basic Roof", SlabKind::Ceiling => "Compound Ceiling", SlabKind::Foundation => "Foundation Slab" }.into()));
            push("offset", num(e.elevation_m)); push("thickness", num(e.width_m)); push("perimeter", num(per));
            push("area", num(plan)); push("volume", num(plan * e.width_m)); push("material", txt(if e.material == crate::Material::Unspecified { "" } else { e.material.name() }));
        }
        (ScheduleKind::Foundations, Some(Geometry::Pad { thickness_m, .. })) => {
            push("family", Value::Text("Isolated Footing".into())); push("width", num(e.width_m)); push("length", num(e.height_m)); push("thickness", num(thickness_m));
            push("area", num(e.width_m * e.height_m)); push("volume", num(e.width_m * e.height_m * thickness_m)); push("material", txt(if e.material == crate::Material::Unspecified { "" } else { e.material.name() }));
        }
        (ScheduleKind::Foundations, Some(Geometry::OnWall { host })) => {
            let len = match doc.get(host).and_then(|h| h.geometry) { Some(Geometry::Linear { start, end, .. }) => length(start, end), _ => 0.0 };
            push("family", Value::Text("Wall Foundation".into())); push("width", num(e.width_m)); push("length", num(len)); push("thickness", num(e.height_m));
            push("area", num(len * e.width_m)); push("volume", num(len * e.width_m * e.height_m)); push("material", txt(if e.material == crate::Material::Unspecified { "" } else { e.material.name() }));
        }
        (ScheduleKind::Columns, Some(Geometry::Point { level, top, .. })) => {
            let base = doc.get(level).map_or(0.0, |l| l.elevation_m) + e.elevation_m;
            let h = (doc.get(top).map_or(base, |l| l.elevation_m) - base).max(0.0);
            push("family", Value::Text("Structural Column".into())); push("top_level", doc.get(top).map_or(Value::Blank, |l| txt(&l.name)));
            push("base_offset", num(e.elevation_m)); push("width", num(e.width_m)); push("depth", num(e.height_m)); push("length", num(h));
            push("volume", num(e.profile().area() * h)); push("material", txt(if e.material == crate::Material::Unspecified { "" } else { e.material.name() }));
        }
        (ScheduleKind::Beams, Some(Geometry::Linear { start, end, .. })) => {
            let plan = length(start, end);
            let len = plan.hypot(e.slope_ratio.unwrap_or(0.0) * plan);
            push("family", Value::Text("Structural Framing".into())); push("offset", num(e.elevation_m)); push("width", num(e.width_m)); push("depth", num(e.height_m));
            push("length", num(len)); push("volume", num(e.profile().area() * len)); push("material", txt(if e.material == crate::Material::Unspecified { "" } else { e.material.name() })); push("usage", Value::Text(e.usage.name().into()));
        }
        (ScheduleKind::Stairs, Some(Geometry::Linear { start, end, .. })) => {
            push("family", Value::Text("Stair".into())); push("width", num(e.width_m)); push("length", num(length(start, end))); push("height", num(e.height_m));
            push("risers", num(f64::from(e.count))); push("treads", num(f64::from(e.count.saturating_sub(1))));
        }
        (ScheduleKind::Railings, Some(Geometry::Area { .. })) => {
            let len: f64 = e.outline.windows(2).map(|w| length(w[0], w[1])).sum();
            push("family", Value::Text("Railing".into())); push("length", num(len)); push("height", num(e.height_m));
        }
        (ScheduleKind::Rooms, Some(Geometry::Room { .. })) => {
            c.push(("name", txt(&e.name)));
            if let Some((a, p)) = ctx.rooms.get(&e.id) { c.push(("area", num(*a))); c.push(("perimeter", num(*p))); }
        }
        (ScheduleKind::Ducts | ScheduleKind::Pipes, Some(Geometry::Linear { start, end, .. })) => {
            let plan = length(start, end);
            let slope = e.slope_ratio.unwrap_or(0.0);
            let len = plan.hypot(slope * plan);
            let pipe = e.category == Category::Pipe;
            let sys = crate::compute::system_of_run(doc, e.id);
            push("system", Value::Text(sys.name().into()));
            push("family", Value::Text(if pipe { "Pipe Types" } else { "Rectangular Duct" }.into()));
            push("size", Value::Text(if pipe { format!("{}mm", mm(e.width_m)) } else { format!("{}x{}mm", mm(e.width_m), mm(e.height_m)) }));
            push("length", num(len)); push("offset", num(e.elevation_m)); push("slope", num(slope * 100.0));
            if pipe { push("diameter", num(e.width_m)); push("area", num(std::f64::consts::PI * e.width_m * len)); }
            else { push("width", num(e.width_m)); push("height", num(e.height_m)); push("area", num(2.0 * (e.width_m + e.height_m) * len)); }
            for (kind, tk, tt) in [(InsulationKind::Insulation, "insulation_type", "insulation_thickness"), (InsulationKind::Lining, "lining_type", "lining_thickness")] {
                if let Some(i) = doc.mep().insulation_of(MepNode::Run(e.id), kind) { push(tk, txt(&i.type_name)); push(tt, num(i.thickness_m)); }
            }
        }
        (k, g) if k.is_family() && e.category == Category::Family => {
            push("family", txt(&e.family));
            push("mark", txt(e.texts.iter().find(|(n, _)| n == "mark").map_or("", |(_, v)| v.as_str())));
            push("offset", num(e.elevation_m));
            let names: Vec<&str> = doc.mep().systems.iter().filter(|s| s.members.contains(&e.id)).map(|s| s.name.as_str()).collect();
            push("system", txt(&names.join(", ")));
            if let Some(Geometry::InWall { host, .. }) = g { push("host", doc.get(host).map_or(Value::Blank, |h| type_of(doc, h))); }
            if let Some([w, h, sill]) = ctx.fams.get(&e.id).and_then(|f| f.opening) {
                let sill = sill + e.elevation_m;
                push("width", num(w)); push("height", num(h)); push("sill", num(sill)); push("head", num(sill + h)); push("area", num(w * h));
            }
        }
        _ => {}
    }
    c
}

fn fitting_cells(doc: &Document, id: u64) -> Cells {
    use openbim_route::FittingKind;
    let Some(m) = doc.mep().fitting(id) else { return Vec::new() };
    let f = &m.fitting;
    let (shape, angle) = match f.kind { FittingKind::Elbow { angle } => ("Elbow", Some(angle)), FittingKind::Transition => ("Transition", None), FittingKind::Tee { branch_angle } => ("Tee", Some(branch_angle)), FittingKind::Tap { branch_angle } => ("Tap", Some(branch_angle)) };
    let size = |s: &openbim_place::penetrations::RunSize| match *s {
        openbim_place::penetrations::RunSize::Round { diameter } => format!("{}mm", mm(diameter)),
        openbim_place::penetrations::RunSize::Rect { width, height, .. } => format!("{}x{}mm", mm(width), mm(height)),
    };
    let mut sizes: Vec<String> = Vec::new();
    for s in f.port_sizes.iter().map(size) { if !sizes.contains(&s) { sizes.push(s); } }
    let mut c: Cells = vec![
        ("family", Value::Text(if m.domain == openbim_family::mep::Domain::Pipe { "Pipe Fitting" } else { "Duct Fitting" }.into())),
        ("type", txt(&f.type_name)), ("part", txt(&f.part_code)), ("shape", Value::Text(shape.into())),
        ("system", Value::Text(m.system.name().into())), ("size", Value::Text(sizes.join(" / "))), ("count", num(1.0)),
    ];
    if let Some(a) = angle { c.push(("angle", num(a))); }
    c
}

fn items(doc: &Document, ctx: &Ctx, kind: ScheduleKind) -> Vec<Item> {
    use ScheduleKind::*;
    if matches!(kind, DuctFittings | PipeFittings) {
        let domain = if kind == PipeFittings { openbim_family::mep::Domain::Pipe } else { openbim_family::mep::Domain::Duct };
        return doc.mep().fittings.iter().filter(|f| f.domain == domain).map(|f| Item::Fit(f.id)).collect();
    }
    doc.elements().filter(|e| match kind {
        Walls => e.category == Category::Wall,
        Floors => e.category == Category::Slab && e.slab_kind == SlabKind::Floor,
        Roofs => e.category == Category::Slab && e.slab_kind == SlabKind::Roof,
        Ceilings => e.category == Category::Slab && e.slab_kind == SlabKind::Ceiling,
        Foundations => (e.category == Category::Slab && e.slab_kind == SlabKind::Foundation) || matches!(e.category, Category::Footing | Category::WallFooting),
        Columns => e.category == Category::Column,
        Beams => e.category == Category::Beam,
        Stairs => e.category == Category::Stair,
        Railings => e.category == Category::Railing,
        Rooms => e.category == Category::Room,
        Ducts => e.category == Category::Duct,
        Pipes => e.category == Category::Pipe,
        _ => e.category == Category::Family && ctx.fams.get(&e.id).is_some_and(|f| f.kind == kind),
    }).map(|e| Item::El(e.id)).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// the table

struct RowData { ids: Vec<ElementId>, vals: Vec<Value>, n: usize }

/// How the sorted rows become lines: the groups, their headers and footers, and the totals.
struct Lines<'a> {
    sorts: &'a [(usize, &'a ScheduleSort)],
    infos: &'a [FieldInfo],
    /// the fields shown, as indexes of the schedule's fields
    shown: &'a [usize],
    /// whether each field of the schedule is added up
    sum_of: &'a [bool],
}

impl Lines<'_> {
    fn count_of(&self, part: &[RowData]) -> usize { part.iter().map(|r| r.n).sum() }

    fn totals(&self, part: &[RowData]) -> Vec<Value> {
        self.shown.iter().map(|&i| {
            if self.sum_of[i] { Value::Num(part.iter().filter_map(|r| if let Value::Num(x) = r.vals[i] { Some(x) } else { None }).sum()) } else { Value::Blank }
        }).collect()
    }

    fn emit(&self, rows: &[RowData], level: usize, out: &mut Vec<TableRow>) {
        if level == self.sorts.len() {
            for r in rows { out.push(TableRow { kind: RowKind::Data, label: String::new(), ids: r.ids.clone(), cells: self.shown.iter().map(|&i| r.vals[i].clone()).collect() }); }
            return;
        }
        let (fi, s) = self.sorts[level];
        let mut start = 0;
        while start < rows.len() {
            let mut end = start + 1;
            while end < rows.len() && same_value(&rows[end].vals[fi], &rows[start].vals[fi], self.infos[fi].ty) { end += 1; }
            let group = &rows[start..end];
            let title = self.infos[fi].ty.format(&group[0].vals[fi]);
            if s.header { out.push(TableRow { kind: RowKind::Header, label: title.clone(), ids: Vec::new(), cells: vec![Value::Blank; self.shown.len()] }); }
            self.emit(group, level + 1, out);
            if s.footer { out.push(TableRow { kind: RowKind::Footer, label: format!("{}: {}", if title.is_empty() { "(blank)" } else { &title }, self.count_of(group)), ids: Vec::new(), cells: self.totals(group) }); }
            if s.blank { out.push(TableRow { kind: RowKind::Blank, label: String::new(), ids: Vec::new(), cells: vec![Value::Blank; self.shown.len()] }); }
            start = end;
        }
    }
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (si, sj) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() { i += 1; }
            while j < b.len() && b[j].is_ascii_digit() { j += 1; }
            let (x, y): (String, String) = (a[si..i].iter().collect(), b[sj..j].iter().collect());
            let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
            let o = x.len().cmp(&y.len()).then_with(|| x.cmp(y));
            if o != Ordering::Equal { return o; }
        } else {
            let o = a[i].cmp(&b[j]);
            if o != Ordering::Equal { return o; }
            i += 1; j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

fn cmp_value(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Blank, Value::Blank) => Ordering::Equal,
        (Value::Blank, _) => Ordering::Less,
        (_, Value::Blank) => Ordering::Greater,
        (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
        (Value::Text(x), Value::Text(y)) => natural_cmp(x, y),
        (Value::Num(_), Value::Text(_)) => Ordering::Less,
        (Value::Text(_), Value::Num(_)) => Ordering::Greater,
    }
}

fn same_value(a: &Value, b: &Value, ty: FieldType) -> bool {
    match (a, b) { (Value::Num(x), Value::Num(y)) => (ty.to_display(*x) - ty.to_display(*y)).abs() < 1e-9 * (1.0 + x.abs().max(y.abs()) * 1000.0), _ => cmp_value(a, b) == Ordering::Equal }
}

fn passes(f: &ScheduleFilter, v: &Value, ty: FieldType) -> bool {
    if ty.numeric() {
        let Ok(want) = f.value.trim().parse::<f64>() else { return false };
        let Value::Num(x) = v else { return f.op == FilterOp::NotEqual };
        let x = ty.to_display(*x);
        let eq = (x - want).abs() <= 1e-9 * (1.0 + want.abs());
        return match f.op {
            FilterOp::Equal => eq, FilterOp::NotEqual => !eq, FilterOp::Greater => x > want && !eq, FilterOp::GreaterEqual => x > want || eq,
            FilterOp::Less => x < want && !eq, FilterOp::LessEqual => x < want || eq, _ => false,
        };
    }
    let s = match v { Value::Text(t) => t.to_lowercase(), _ => String::new() };
    let w = f.value.to_lowercase();
    match f.op {
        FilterOp::Equal => s == w, FilterOp::NotEqual => s != w,
        FilterOp::Greater => s > w, FilterOp::GreaterEqual => s >= w, FilterOp::Less => s < w, FilterOp::LessEqual => s <= w,
        FilterOp::Contains => s.contains(&w), FilterOp::NotContains => !s.contains(&w),
        FilterOp::BeginsWith => s.starts_with(&w), FilterOp::NotBeginsWith => !s.starts_with(&w),
        FilterOp::EndsWith => s.ends_with(&w), FilterOp::NotEndsWith => !s.ends_with(&w),
    }
}

impl Document {
    /// The schedule with this number.
    pub fn schedule(&self, id: u64) -> Option<&ScheduleDef> { self.mep().schedules.iter().find(|s| s.id == id) }

    /// The table of a schedule definition, worked out from the model as it is now. The elements are looked at on all
    /// cores; the rows come out in the order the sorting says (and the element order where it does not).
    pub fn schedule_table(&self, def: &ScheduleDef) -> ScheduleTable {
        let ctx = Ctx::new(self, def.kind);
        let infos: Vec<FieldInfo> = def.fields.iter().filter_map(|f| field_info(def.kind, &f.key)).collect();
        let list = items(self, &ctx, def.kind);
        let mut rows: Vec<RowData> = list.par_iter().map(|item| {
            let (ids, cells) = match item {
                Item::El(id) => match self.get(*id) { Some(e) => (vec![*id], element_cells(self, &ctx, def.kind, e)), None => (Vec::new(), Vec::new()) },
                Item::Fit(id) => (Vec::new(), fitting_cells(self, *id)),
            };
            let vals = def.fields.iter().map(|f| cells.iter().find(|(k, _)| *k == f.key).map_or(Value::Blank, |(_, v)| v.clone())).collect();
            RowData { ids, vals, n: 1 }
        }).collect();
        // the filters
        let at = |key: &str| def.fields.iter().position(|f| f.key == key);
        for fl in &def.filters {
            if let Some(i) = at(&fl.field) { rows.retain(|r| passes(fl, &r.vals[i], infos[i].ty)); }
        }
        // the sorting
        let sorts: Vec<(usize, &ScheduleSort)> = def.sorts.iter().filter_map(|s| at(&s.field).map(|i| (i, s))).collect();
        let summed = |i: usize| def.fields[i].totals || def.fields[i].key == "count";
        let group_key: Vec<usize> = (0..def.fields.len()).filter(|&i| !summed(i) && def.fields[i].key != "id").collect();
        rows.sort_by(|a, b| {
            for (i, s) in &sorts {
                let o = cmp_value(&a.vals[*i], &b.vals[*i]);
                if o != Ordering::Equal { return if s.descending { o.reverse() } else { o }; }
            }
            if !def.itemize { for &i in &group_key { let o = cmp_value(&a.vals[i], &b.vals[i]); if o != Ordering::Equal { return o; } } }
            Ordering::Equal
        });
        // elements that look alike are one row, their quantities added up
        if !def.itemize {
            let mut merged: Vec<RowData> = Vec::new();
            for r in rows {
                match merged.last_mut() {
                    Some(m) if group_key.iter().all(|&i| same_value(&m.vals[i], &r.vals[i], infos[i].ty)) && sorts.iter().all(|(i, _)| same_value(&m.vals[*i], &r.vals[*i], infos[*i].ty)) => {
                        for i in 0..m.vals.len() {
                            if summed(i) { if let (Value::Num(a), Value::Num(b)) = (&m.vals[i], &r.vals[i]) { m.vals[i] = Value::Num(a + b); } }
                        }
                        m.ids.extend(r.ids);
                        m.n += r.n;
                    }
                    _ => merged.push(r),
                }
            }
            rows = merged;
        }
        // the lines
        let shown: Vec<usize> = (0..def.fields.len()).filter(|&i| !def.fields[i].hidden).collect();
        let sum_of: Vec<bool> = (0..def.fields.len()).map(|i| summed(i) && infos[i].ty.summable()).collect();
        let lines = Lines { sorts: &sorts, infos: &infos, shown: &shown, sum_of: &sum_of };
        let mut out: Vec<TableRow> = Vec::new();
        lines.emit(&rows, 0, &mut out);
        if def.grand_totals && !rows.is_empty() {
            out.push(TableRow { kind: RowKind::Grand, label: format!("Grand total: {}", lines.count_of(&rows)), ids: Vec::new(), cells: lines.totals(&rows) });
        }
        ScheduleTable { headings: shown.iter().map(|&i| def.heading(&def.fields[i])).collect(), types: shown.iter().map(|&i| infos[i].ty).collect(), rows: out }
    }

    /// The number a new schedule gets.
    pub fn next_schedule_id(&self) -> u64 { self.mep().schedules.iter().map(|s| s.id + 1).max().unwrap_or(1) }

    /// The name a new schedule of `kind` would get: "Wall Schedule", then "Wall Schedule 1", "Wall Schedule 2"...
    pub fn default_schedule_name(&self, kind: ScheduleKind) -> String {
        let base = kind.default_title();
        if self.mep().schedules.iter().all(|s| s.name != base) { return base; }
        (1..).map(|n| format!("{base} {n}")).find(|n| self.mep().schedules.iter().all(|s| &s.name != n)).unwrap_or(base)
    }

    /// New Schedule/Quantities: a schedule of `kind` with the fields its quantities come from, called `name` (the
    /// category's own name when empty). Gives the commands of the step and the number of the schedule.
    pub fn new_schedule_commands(&self, kind: ScheduleKind, name: &str) -> Result<(Vec<Command>, u64), String> {
        let name = if name.trim().is_empty() { self.default_schedule_name(kind) } else { name.trim().to_string() };
        let def = ScheduleDef::starter(self.next_schedule_id(), &name, kind);
        let id = def.id;
        Ok((self.set_schedule_commands(&def)?, id))
    }

    /// Saves `def`: a new schedule (its number is not in use) or a changed one. Names are unique.
    pub fn set_schedule_commands(&self, def: &ScheduleDef) -> Result<Vec<Command>, String> {
        def.check()?;
        let mut data = self.mep().clone();
        if data.schedules.iter().any(|s| s.name == def.name && s.id != def.id) { return Err(format!("there is a schedule called \"{}\" already", def.name)); }
        match data.schedules.iter().position(|s| s.id == def.id) {
            Some(i) => { if data.schedules[i] == *def { return Err("nothing changes".into()); } data.schedules[i] = def.clone(); }
            None => { if data.schedules.len() >= MAX_SCHEDULES { return Err(format!("a model has at most {MAX_SCHEDULES} schedules")); } data.schedules.push(def.clone()); }
        }
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// Duplicates a schedule (Revit's Duplicate View > Duplicate): a copy with the next free name. Gives the commands
    /// and the number of the copy.
    pub fn duplicate_schedule_commands(&self, id: u64) -> Result<(Vec<Command>, u64), String> {
        let src = self.schedule(id).ok_or("there is no such schedule")?;
        let name = (1..).map(|n| format!("{} Copy {n}", src.name)).find(|n| n.chars().count() <= 60 && self.mep().schedules.iter().all(|s| &s.name != n)).ok_or("the name of the copy is too long")?;
        let copy = ScheduleDef { id: self.next_schedule_id(), name, ..src.clone() };
        let new_id = copy.id;
        Ok((self.set_schedule_commands(&copy)?, new_id))
    }

    pub fn delete_schedule_commands(&self, id: u64) -> Result<Vec<Command>, String> {
        let mut data = self.mep().clone();
        let at = data.schedules.iter().position(|s| s.id == id).ok_or("there is no such schedule")?;
        data.schedules.remove(at);
        Ok(vec![Command::SetMep(Box::new(data))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FamilySpec, Document};

    fn run(d: &mut Document, c: Vec<Command>) { d.transact(d.revision(), &c).unwrap(); }
    /// a door or window of the library in a wall: "Type A" of the door is 800 x 2100, the window's own size 1200 x 1200
    fn door(d: &mut Document, wall: u64, along: f64, family: &str, type_name: &str) -> ElementId {
        d.place_family_in_wall(ElementId(wall), along, &FamilySpec { family: family.into(), type_name: type_name.into(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 }).unwrap()
    }
    fn table(d: &Document, def: &ScheduleDef) -> ScheduleTable { d.schedule_table(def) }
    fn data(t: &ScheduleTable) -> Vec<&TableRow> { t.rows.iter().filter(|r| r.kind == RowKind::Data).collect() }

    #[test]
    fn every_category_has_fields_a_starter_that_checks_and_a_table_even_on_an_empty_model() {
        let d = Document::spatial_sample();
        for k in ScheduleKind::ALL {
            assert_eq!(ScheduleKind::from_code(k.code()), Some(k));
            let keys: Vec<&str> = fields(k).iter().map(|f| f.key).collect();
            let mut u = keys.clone(); u.sort(); u.dedup();
            assert_eq!(u.len(), keys.len(), "{k:?}: a field twice");
            let def = ScheduleDef::starter(1, "S", k);
            def.check().unwrap_or_else(|e| panic!("{k:?}: {e}"));
            let t = table(&d, &def);
            assert_eq!(t.headings.len(), def.fields.len());
        }
        assert_eq!(ScheduleKind::Walls.default_title(), "Wall Schedule");
        assert_eq!(ScheduleKind::Ceilings.default_title(), "Ceiling Schedule");
        assert_eq!(ScheduleKind::DuctFittings.default_title(), "Duct Fitting Schedule");
        assert_eq!(ScheduleKind::GenericModels.default_title(), "Generic Model Schedule");
    }

    #[test]
    fn a_wall_schedule_gives_lengths_areas_and_volumes_net_of_doors_windows_and_openings() {
        let mut d = Document::spatial_sample();
        let mut def = ScheduleDef::starter(1, "Walls", ScheduleKind::Walls);
        def.itemize = true;
        def.sorts.clear();
        let t = table(&d, &def);
        let rows = data(&t);
        assert_eq!(rows.len(), 6);
        let col = |k: &str| def.fields.iter().filter(|f| !f.hidden).position(|f| f.key == k).unwrap();
        let wall20 = rows.iter().find(|r| r.ids == vec![ElementId(20)]).unwrap();
        assert_eq!(wall20.cells[col("length")], Value::Num(8.0));
        assert_eq!(wall20.cells[col("area")], Value::Num(24.0));
        assert_eq!(t.text(wall20, col("length")), "8000");
        assert_eq!(t.text(wall20, col("area")), "24.00");
        assert_eq!(t.text(wall20, col("volume")), "4.800");
        assert_eq!(t.text(wall20, col("type")), "Generic - 200mm");
        // a door (0.9 x 2.1) and a window take their area out of the wall; so does an opening
        door(&mut d, 20, 2.0, "Door, single flush", "Type A");
        door(&mut d, 20, 5.0, "Window, fixed with mullions", "");
        let t = table(&d, &def);
        let w = data(&t).into_iter().find(|r| r.ids == vec![ElementId(20)]).unwrap();
        let Value::Num(a) = w.cells[col("area")] else { panic!() };
        assert!((a - (24.0 - 0.8 * 2.1 - 1.2 * 1.2)).abs() < 1e-9, "{a}");
        let before = a;
        d.place_wall_opening(ElementId(20), 6.5, 7.5, 0.5, 2.0).unwrap();
        let t = table(&d, &def);
        let w = data(&t).into_iter().find(|r| r.ids == vec![ElementId(20)]).unwrap();
        let Value::Num(a) = w.cells[col("area")] else { panic!() };
        assert!((before - a - 1.5).abs() < 1e-9, "{before} {a}");
        let Value::Num(v) = w.cells[col("volume")] else { panic!() };
        assert!((v - a * 0.2).abs() < 1e-9);
    }

    #[test]
    fn rows_that_look_alike_are_one_row_with_their_quantities_added_up_unless_every_instance_is_itemized() {
        let d = Document::spatial_sample();
        let mut def = ScheduleDef::starter(1, "W", ScheduleKind::Walls);
        def.itemize = false;
        def.sorts.clear();
        // type and level are the same for the walls on level 1 (five) and the upper wall is on level 2
        def.fields = ["type", "level", "length", "count"].iter().map(|k| ScheduleField { totals: *k == "length", ..ScheduleField::new(k) }).collect();
        let t = table(&d, &def);
        let rows = data(&t);
        assert_eq!(rows.len(), 2, "{:?}", t.rows);
        let l1 = rows.iter().find(|r| r.cells[1] == Value::Text("Level 1".into())).unwrap();
        assert_eq!(l1.ids.len(), 5);
        assert_eq!(l1.cells[3], Value::Num(5.0));
        assert_eq!(l1.cells[2], Value::Num(8.0 + 6.0 + 8.0 + 6.0 + 6.0));
        let grand = t.rows.last().unwrap();
        assert_eq!(grand.kind, RowKind::Grand);
        assert_eq!(grand.label, "Grand total: 6");
        assert_eq!(grand.cells[2], Value::Num(34.0 + 8.0));
        def.itemize = true;
        assert_eq!(data(&table(&d, &def)).len(), 6);
    }

    #[test]
    fn filters_sorting_headers_footers_and_blank_lines_work_like_revits() {
        let d = Document::spatial_sample();
        let mut def = ScheduleDef::starter(1, "W", ScheduleKind::Walls);
        def.itemize = true;
        def.fields = ["level", "length", "count"].iter().map(|k| ScheduleField { totals: *k == "length", ..ScheduleField::new(k) }).collect();
        def.sorts = vec![ScheduleSort { field: "level".into(), descending: true, header: true, footer: true, blank: true }, ScheduleSort { field: "length".into(), descending: false, header: false, footer: false, blank: false }];
        let t = table(&d, &def);
        let kinds: Vec<RowKind> = t.rows.iter().map(|r| r.kind).collect();
        use RowKind::*;
        assert_eq!(kinds, vec![Header, Data, Footer, Blank, Header, Data, Data, Data, Data, Data, Footer, Blank, Grand]);
        assert_eq!(t.rows[0].label, "Level 2");
        assert_eq!(t.rows[2].label, "Level 2: 1");
        assert_eq!(t.rows[10].label, "Level 1: 5");
        assert_eq!(t.rows[10].cells[1], Value::Num(34.0));
        // within a level the lengths ascend
        let lens: Vec<f64> = t.rows[5..10].iter().map(|r| match r.cells[1] { Value::Num(x) => x, _ => f64::NAN }).collect();
        assert_eq!(lens, vec![6.0, 6.0, 6.0, 8.0, 8.0]);
        // a filter: walls longer than 7000 mm
        def.filters = vec![ScheduleFilter { field: "length".into(), op: FilterOp::Greater, value: "7000".into() }];
        let t = table(&d, &def);
        assert_eq!(data(&t).len(), 3);
        // a filter on a field that is hidden still works
        def.fields[0].hidden = true;
        def.filters = vec![ScheduleFilter { field: "level".into(), op: FilterOp::Equal, value: "level 2".into() }];
        let t = table(&d, &def);
        assert_eq!(data(&t).len(), 1);
        assert_eq!(t.headings, vec!["Length", "Count"]);
        // text matching
        def.filters = vec![ScheduleFilter { field: "level".into(), op: FilterOp::BeginsWith, value: "LEV".into() }, ScheduleFilter { field: "level".into(), op: FilterOp::NotContains, value: "1".into() }];
        assert_eq!(data(&table(&d, &def)).len(), 1);
    }

    #[test]
    fn sorting_reads_numbers_in_text_as_numbers_and_a_bad_definition_is_refused() {
        assert_eq!(natural_cmp("Level 2", "Level 10"), Ordering::Less);
        assert_eq!(natural_cmp("level 10", "Level 10"), Ordering::Equal);
        assert_eq!(natural_cmp("A", "a1"), Ordering::Less);
        let mut def = ScheduleDef::starter(1, "S", ScheduleKind::Walls);
        let bad = |f: &dyn Fn(&mut ScheduleDef)| { let mut d = def.clone(); f(&mut d); d.check().is_err() };
        assert!(bad(&|d| d.name = " x".into()));
        assert!(bad(&|d| d.fields.clear()));
        assert!(bad(&|d| d.fields.push(ScheduleField::new("length"))));
        assert!(bad(&|d| d.fields.push(ScheduleField::new("mark"))));
        assert!(bad(&|d| d.fields[0].totals = true), "text cannot be added up");
        assert!(bad(&|d| d.filters.push(ScheduleFilter { field: "volume".into(), op: FilterOp::Contains, value: "1".into() })));
        assert!(bad(&|d| d.filters.push(ScheduleFilter { field: "length".into(), op: FilterOp::Equal, value: "long".into() })));
        assert!(bad(&|d| d.filters.push(ScheduleFilter { field: "id".into(), op: FilterOp::Equal, value: "1".into() })), "a filter is on a field of the schedule");
        assert!(bad(&|d| d.sorts.push(ScheduleSort::new("type"))));
        assert!(bad(&|d| d.sorts = vec![ScheduleSort::new("area")] .into_iter().chain((0..4).map(|_| ScheduleSort::new("length"))).collect()));
        assert!(bad(&|d| for f in &mut d.fields { f.hidden = true; }));
        def.sorts.clear();
        def.check().unwrap();
    }

    #[test]
    fn rooms_floors_columns_beams_and_runs_give_their_quantities() {
        use crate::{BeamSpec, ColumnSpec, Material, SlabSpec};
        let mut d = Document::spatial_sample();
        let room = d.place_room(ElementId(10), [2.0, 3.0]).unwrap();
        let slab = d.place_slab(ElementId(10), &SlabSpec { outline: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }).unwrap();
        let col = d.place_column(ElementId(10), ElementId(11), [4.0, 3.0], &ColumnSpec { width_m: 0.4, depth_m: 0.3, base_offset_m: 0.0, section: crate::Section::Rectangle, material: Material::Concrete }).unwrap();
        let beam = d.place_beam(ElementId(11), &BeamSpec::level([0.0, 3.0], [8.0, 3.0], 0.3, 0.5, 0.0)).unwrap();
        let get = |d: &Document, kind, key: &str| -> Vec<(Vec<ElementId>, Value)> {
            let mut def = ScheduleDef::starter(1, "S", kind);
            def.itemize = true; def.sorts.clear();
            def.fields = vec![ScheduleField::new("id"), ScheduleField::new(key)];
            d.schedule_table(&def).rows.into_iter().filter(|r| r.kind == RowKind::Data).map(|r| (r.ids, r.cells[1].clone())).collect()
        };
        let num = |v: &Value| match v { Value::Num(x) => *x, _ => panic!("{v:?}") };
        let r = get(&d, ScheduleKind::Rooms, "area");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, vec![room]);
        assert!((num(&r[0].1) - 3.8 * 5.8).abs() < 1e-6, "{:?}", r[0].1);
        let r = get(&d, ScheduleKind::Floors, "volume");
        assert_eq!(r[0].0, vec![slab]);
        assert!((num(&r[0].1) - 48.0 * 0.2).abs() < 1e-9);
        assert_eq!(get(&d, ScheduleKind::Roofs, "area").len(), 0);
        // a shaft through a floor takes its area out; a roof is measured on its slope
        let upper = d.place_slab(ElementId(11), &SlabSpec { outline: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }).unwrap();
        d.place_shaft(ElementId(10), &crate::opening::ShaftSpec { outline: vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]], top: ElementId(11), base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        let r = get(&d, ScheduleKind::Floors, "area");
        let of = |id: ElementId| num(&r.iter().find(|x| x.0 == vec![id]).unwrap().1);
        assert!((of(upper) - 44.0).abs() < 1e-6, "{}", of(upper));
        assert!((of(slab) - 48.0).abs() < 1e-6, "the floor the shaft starts on is not cut: {}", of(slab));
        d.place_roof(ElementId(11), &crate::roof::RoofSpec { outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]], thickness_m: 0.2, base_offset_m: 3.0, slope_ratio: 1.0, edges: vec![], material: Material::Timber }).unwrap();
        let r = get(&d, ScheduleKind::Roofs, "area");
        assert!((num(&r[0].1) - 60.0 * 2f64.sqrt()).abs() < 1e-6, "{:?}", r);
        let r = get(&d, ScheduleKind::Columns, "volume");
        assert_eq!(r[0].0, vec![col]);
        assert!((num(&r[0].1) - 0.4 * 0.3 * 3.0).abs() < 1e-9);
        let r = get(&d, ScheduleKind::Beams, "volume");
        assert_eq!(r[0].0, vec![beam]);
        assert!((num(&r[0].1) - 0.3 * 0.5 * 8.0).abs() < 1e-9);
        let r = get(&d, ScheduleKind::Ducts, "area");
        assert!((num(&r[0].1) - 2.0 * (0.2 + 0.3) * 6.0).abs() < 1e-9);
        let r = get(&d, ScheduleKind::Pipes, "area");
        assert!((num(&r[0].1) - std::f64::consts::PI * 0.1 * 6.0).abs() < 1e-9);
        let r = get(&d, ScheduleKind::Pipes, "size");
        assert_eq!(r[0].1, Value::Text("100mm".into()));
        let r = get(&d, ScheduleKind::Ducts, "size");
        assert_eq!(r[0].1, Value::Text("200x300mm".into()));
    }

    #[test]
    fn doors_windows_and_equipment_are_listed_by_their_family_category_with_their_marks_and_systems() {
        let mut d = Document::spatial_sample();
        let dr = door(&mut d, 20, 2.0, "Door, single flush", "Type B");
        let win = door(&mut d, 20, 5.0, "Window, fixed with mullions", "");
        d.transact(d.revision(), &[Command::SetFamilyTexts { id: dr, texts: vec![("mark".into(), "D1".into())] }]).unwrap();
        let fc = d.place_family(ElementId(10), [3.0, 3.0], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 }).unwrap();
        let c = d.create_system_commands("", openbim_family::mep::System::SupplyAir, &[fc]).unwrap();
        run(&mut d, c);
        let mut def = ScheduleDef::starter(1, "Doors", ScheduleKind::Doors);
        def.sorts.clear();
        def.fields = ["mark", "host", "width", "height", "sill", "head", "area", "count"].iter().map(|k| ScheduleField::new(k)).collect();
        let t = table(&d, &def);
        let rows = data(&t);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ids, vec![dr]);
        assert_eq!(t.text(rows[0], 0), "D1");
        assert_eq!(t.text(rows[0], 1), "Generic - 200mm");
        assert_eq!(t.text(rows[0], 2), "1200");
        assert_eq!(t.text(rows[0], 3), "2400");
        assert_eq!(t.text(rows[0], 4), "0");
        assert_eq!(t.text(rows[0], 5), "2400");
        assert_eq!(t.text(rows[0], 6), "2.88");
        let mut w = ScheduleDef::starter(2, "Windows", ScheduleKind::Windows);
        w.sorts.clear();
        assert_eq!(data(&table(&d, &w))[0].ids, vec![win]);
        let mut m = ScheduleDef::starter(3, "Equipment", ScheduleKind::MechanicalEquipment);
        m.sorts.clear();
        m.fields = vec![ScheduleField::new("type"), ScheduleField::new("system"), ScheduleField::new("count")];
        let t = table(&d, &m);
        let rows = data(&t);
        assert_eq!(rows[0].ids, vec![fc]);
        assert_eq!(t.text(rows[0], 1), "Supply air 1");
        // the equipment is not a generic model, nor is a door a piece of equipment
        let mut g = ScheduleDef::starter(4, "G", ScheduleKind::GenericModels);
        g.sorts.clear();
        assert!(data(&table(&d, &g)).is_empty());
    }

    #[test]
    fn schedules_are_saved_renamed_duplicated_deleted_undone_and_read_back_from_the_records() {
        let mut d = Document::spatial_sample();
        let (c, id) = d.new_schedule_commands(ScheduleKind::Walls, "").unwrap();
        run(&mut d, c);
        assert_eq!(d.schedule(id).unwrap().name, "Wall Schedule");
        let (c, id2) = d.new_schedule_commands(ScheduleKind::Walls, "").unwrap();
        run(&mut d, c);
        assert_eq!(d.schedule(id2).unwrap().name, "Wall Schedule 1");
        assert!(d.new_schedule_commands(ScheduleKind::Doors, "Wall Schedule").is_err(), "names are unique");
        // edit it
        let mut def = d.schedule(id).unwrap().clone();
        def.name = "Walls, \"A\"; 1".into();
        def.filters.push(ScheduleFilter { field: "length".into(), op: FilterOp::GreaterEqual, value: "6000".into() });
        def.fields[1].heading = "Lvl, \"x\"".into();
        def.fields[2].hidden = true;
        def.sorts[0].footer = true;
        def.grand_totals = false;
        def.itemize = true;
        let c = d.set_schedule_commands(&def).unwrap();
        run(&mut d, c);
        assert!(d.set_schedule_commands(&def).unwrap_err().contains("nothing changes"));
        let mut other = d.schedule(id2).unwrap().clone();
        other.name = def.name.clone();
        assert!(d.set_schedule_commands(&other).is_err());
        // records
        let back = crate::native::parse_mep(&crate::native::mep_records(d.mep())).unwrap();
        assert_eq!(back.schedules, d.mep().schedules);
        // a copy
        let (c, id3) = d.duplicate_schedule_commands(id).unwrap();
        run(&mut d, c);
        assert_eq!(d.schedule(id3).unwrap().name, "Walls, \"A\"; 1 Copy 1");
        assert_eq!(d.schedule(id3).unwrap().fields, def.fields);
        // undo takes the copy away, redo brings it back, delete removes it
        d.undo().unwrap();
        assert!(d.schedule(id3).is_none());
        d.redo().unwrap();
        let c = d.delete_schedule_commands(id3).unwrap();
        run(&mut d, c);
        assert!(d.schedule(id3).is_none() && d.schedule(id).is_some());
        assert!(d.delete_schedule_commands(id3).is_err());
        // the model refuses a bad schedule that does not come through the commands
        let mut bad = d.mep().clone();
        bad.schedules[0].fields.push(ScheduleField::new("nonsense"));
        assert!(d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]).is_err());
        let mut bad = d.mep().clone();
        bad.schedules[1].id = bad.schedules[0].id;
        assert!(d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]).is_err());
    }

    #[test]
    fn csv_has_units_in_the_headings_plain_numbers_and_text_that_cannot_run_as_a_formula() {
        let mut d = Document::spatial_sample();
        let ty = d.add_wall_type("=1+1, \"A\"", 0.2, 3.0).unwrap();
        d.assign_type(&[ElementId(20)], ty).unwrap();
        let mut def = ScheduleDef::starter(1, "W", ScheduleKind::Walls);
        def.sorts.clear();
        def.fields = ["type", "length", "count"].iter().map(|k| ScheduleField { totals: *k == "length", ..ScheduleField::new(k) }).collect();
        def.filters = vec![ScheduleFilter { field: "type".into(), op: FilterOp::Contains, value: "=".into() }];
        let csv = d.schedule_table(&def).to_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "Type,Length (mm),Count");
        assert_eq!(lines[1], "\"'=1+1, \"\"A\"\"\",8000,1");
        assert_eq!(lines[2], "Grand total: 1,8000,1");
    }
}
