//! Shared parameters: input values defined once, with a fixed identity, and used by several families.
//! A shared parameter has the same name, kind, range, default and scope everywhere it is used, so schedules and
//! IFC property sets can treat it as one thing. The set is kept as a plain tab-separated text file (no JSON).
//! No parameter is shipped: the set is typed in or imported by the user.
use crate::{clean_text, valid_name, Family, FamilyError, Scope, ValueDef, ValueKind};

pub const MAX_SHARED: usize = 10_000;
pub const MAX_GROUP: usize = 64;
pub const MAX_DESCRIPTION: usize = 256;

#[derive(Debug, Clone)]
pub struct SharedParam {
    /// stable identity that survives a rename; never 0
    pub id: u128,
    /// heading in the properties list, for example the IFC property set it is exported to
    pub group: String,
    pub description: String,
    /// an input: number, whole number, yes/no, choice, text or material (never a formula)
    pub def: ValueDef,
}

#[derive(Debug, Clone, Default)]
pub struct SharedSet {
    pub params: Vec<SharedParam>,
}

fn bad(name: &str, why: &'static str) -> FamilyError { FamilyError::Shared { name: name.into(), why } }

impl SharedSet {
    pub fn get(&self, name: &str) -> Option<&SharedParam> { self.params.iter().find(|p| p.def.name == name) }

    /// Every parameter must be valid on its own, and names and identities must be unique.
    pub fn check(&self) -> Result<(), FamilyError> {
        if self.params.len() > MAX_SHARED { return Err(FamilyError::TooMany("shared parameters")); }
        let mut names = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for p in &self.params {
            let n = &p.def.name;
            if !names.insert(n.clone()) { return Err(FamilyError::DuplicateName(n.clone())); }
            if p.id == 0 { return Err(bad(n, "the identity cannot be 0")); }
            if !ids.insert(p.id) { return Err(bad(n, "the identity is used by another shared parameter")); }
            if matches!(p.def.kind, ValueKind::Formula(_)) { return Err(bad(n, "a shared parameter must be an input, not a formula")); }
            if p.def.dim.is_some_and(|d| d.code().is_none()) { return Err(bad(n, "this dimension cannot be written in a shared parameter file")); }
            clean_text(n, &p.group, MAX_GROUP)?;
            clean_text(n, &p.description, MAX_DESCRIPTION)?;
            // compile it alone: range, default, options and name rules
            Family { name: "check".into(), values: vec![p.def.clone()], ..Default::default() }.compile()?;
        }
        Ok(())
    }

    /// The text form, one parameter per line:
    /// id (32 hex digits), name, kind (length, angle, area, volume, ratio, count, flag, choice, text, material; number means length), scope, default, min, max, options (separated by |), group, description.
    pub fn to_text(&self) -> String {
        let mut s = String::from("# id\tname\tkind\tscope\tdefault\tmin\tmax\toptions\tgroup\tdescription\n");
        for p in &self.params {
            let (kind, scope, default, min, max, options) = match &p.def.kind {
                ValueKind::Input { default, min, max, integer, scope } => {
                    let kind = if *integer { "count" } else { match p.def.dim { None => "length", Some(d) if d == crate::NUMBER => "ratio", Some(d) => d.code().unwrap_or("length") } };
                    (kind, *scope, default.to_string(), min.to_string(), max.to_string(), String::new())
                }
                ValueKind::Flag { default, scope } => ("flag", *scope, (*default as u8).to_string(), String::new(), String::new(), String::new()),
                ValueKind::Choice { options, default, scope } => ("choice", *scope, default.clone(), String::new(), String::new(), options.join("|")),
                ValueKind::Text { default, scope } => ("text", *scope, default.clone(), String::new(), String::new(), String::new()),
                ValueKind::Material { default, scope } => ("material", *scope, default.clone(), String::new(), String::new(), String::new()),
                ValueKind::Formula(_) => continue,
            };
            let sc = if scope == Scope::Type { "type" } else { "instance" };
            s.push_str(&format!("{:032x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", p.id, p.def.name, kind, sc, default, min, max, options, p.group, p.description));
        }
        s
    }

    /// Read the text form. Blank lines and lines starting with # are skipped. The error carries the 1-based line number.
    pub fn from_text(text: &str) -> Result<SharedSet, (usize, FamilyError)> {
        let mut set = SharedSet::default();
        for (i, raw) in text.lines().enumerate() {
            let ln = i + 1;
            let line = raw; // `lines()` already drops a trailing \r
            if line.trim().is_empty() || line.starts_with('#') { continue; }
            let f: Vec<&str> = line.split('\t').collect();
            let name = f.get(1).copied().unwrap_or("");
            let e = |why: &'static str| (ln, bad(name, why));
            if f.len() != 10 { return Err(e("a line needs 10 tab-separated fields")); }
            if f[0].len() != 32 || !f[0].chars().all(|c| c.is_ascii_hexdigit()) { return Err(e("the identity must be 32 hex digits")); }
            let id = u128::from_str_radix(f[0], 16).map_err(|_| e("the identity must be 32 hex digits"))?;
            if !valid_name(name) { return Err((ln, FamilyError::BadName(name.into()))); }
            let scope = match f[3] { "type" => Scope::Type, "instance" => Scope::Instance, _ => return Err(e("scope must be type or instance")) };
            let num = |s: &str, why: &'static str| -> Result<f64, (usize, FamilyError)> { s.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| e(why)) };
            let def = match f[2] {
                // number is the older word for length
                "number" | "length" | "ratio" | "angle" | "area" | "volume" | "count" | "time" | "mass" | "temperature" | "flow" | "force" | "pressure" | "energy" | "power" | "u_value" => {
                    let d = ValueDef::input(name, num(f[4], "the default is not a number")?, num(f[5], "the minimum is not a number")?, num(f[6], "the maximum is not a number")?, scope);
                    match f[2] { "ratio" => d.with_dim(crate::NUMBER), "angle" => d.with_dim(crate::ANGLE), "area" => d.with_dim(crate::AREA), "volume" => d.with_dim(crate::VOLUME), k => match crate::Dim::from_code(k) { Some(dim) if k != "number" && k != "length" && k != "count" => d.with_dim(dim), _ => d } }
                }
                "flag" => match f[4] { "0" | "false" => ValueDef::flag(name, false, scope), "1" | "true" => ValueDef::flag(name, true, scope), _ => return Err(e("a flag default is 0 or 1")) },
                "choice" => { let o: Vec<&str> = f[7].split('|').collect(); ValueDef::choice(name, &o, f[4], scope) }
                "text" => ValueDef::text(name, f[4], scope),
                "material" => ValueDef::material(name, f[4], scope),
                _ => return Err(e("kind must be length, angle, area, volume, time, mass, temperature, flow, force, pressure, energy, power, u_value, ratio, count, flag, choice, text or material")),
            };
            let def = if f[2] == "count" { match def.kind { ValueKind::Input { default, min, max, scope, .. } => ValueDef::count(name, default, min, max, scope), _ => def } } else { def };
            set.params.push(SharedParam { id, group: f[8].into(), description: f[9].into(), def });
            if set.params.len() > MAX_SHARED { return Err((ln, FamilyError::TooMany("shared parameters"))); }
        }
        set.check().map_err(|err| (0, err))?;
        Ok(set)
    }
}
