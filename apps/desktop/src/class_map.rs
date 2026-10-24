//! The class map: which element of the app an IFC class becomes on import, as a plain tab-separated text the user
//! edits (Revit's IFC class mapping file). One row per line: the IFC class, the app's category, and optionally the
//! predefined type the row is for (`*` or nothing for any). `#` starts a comment.
//!
//! ```text
//! IFCSTAIR                 Generic
//! IFCBUILDINGELEMENTPROXY  Ignore
//! IFCMEMBER                Column
//! IFCSLAB                  Generic   LANDING
//! ```
//!
//! A row only changes what the importer does by default; a class with no row is read as before. The categories are the
//! ones an extruded body can become (`Wall`, `Beam`, `Column`, `Slab`, `Footing`, `WallFooting`, `Pipe`, `Duct`),
//! `Generic` (kept as a generic model with its class and properties) and `Ignore` (left out, and the log says the map did).
//!
//! A row can also say `family:<name of a library family>` (separate the columns with tabs when the name has spaces):
//! the products of the class become instances of that family, placed where the product is and turned as it is.
//! `IFCUNITARYEQUIPMENT<TAB>family:Air handling unit<TAB>AIRHANDLER` is one. A file this app wrote names its families
//! itself, so it needs no row. A product whose family cannot be used is kept as a generic model.
use openbim_document::Category;

pub const MAX_ROWS: usize = 10_000;
pub const MAX_BYTES: usize = 1 << 20;

/// What a class becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Category(Category),
    Ignore,
}

#[derive(Clone, Debug, PartialEq)]
struct Row {
    class: String,
    /// upper case, `None` for any
    predefined: Option<String>,
    target: Target,
    /// the library family the class is read as (the row said `family:<name>`); the target is then `Generic`, which is
    /// what the product becomes if the family cannot be used
    family: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassMap {
    rows: Vec<Row>,
}

/// The names a row can give for a target, as the file writes them.
pub const TARGETS: [(&str, Target); 10] = [
    ("Wall", Target::Category(Category::Wall)),
    ("Beam", Target::Category(Category::Beam)),
    ("Column", Target::Category(Category::Column)),
    ("Slab", Target::Category(Category::Slab)),
    ("Footing", Target::Category(Category::Footing)),
    ("WallFooting", Target::Category(Category::WallFooting)),
    ("Pipe", Target::Category(Category::Pipe)),
    ("Duct", Target::Category(Category::Duct)),
    ("Generic", Target::Category(Category::Generic)),
    ("Ignore", Target::Ignore),
];

impl ClassMap {
    /// Reads the text of a map. A bad row refuses the whole file and names its line.
    pub fn parse(text: &str) -> Result<ClassMap, String> {
        if text.len() > MAX_BYTES { return Err(format!("the class map is larger than {MAX_BYTES} bytes")); }
        let mut rows: Vec<Row> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim_end();
            if line.trim().is_empty() { continue; }
            let at = |why: &str| format!("line {}: {why}", i + 1);
            let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
            // a map typed with spaces instead of tabs reads the same
            let cols: Vec<&str> = if cols.len() == 1 { line.split_whitespace().collect() } else { cols };
            if cols.len() < 2 || cols.len() > 3 { return Err(at("a row is the class, the category and optionally the predefined type")); }
            let class = cols[0].to_ascii_uppercase();
            if !class.starts_with("IFC") || class.len() <= 3 || !class.bytes().all(|b| b.is_ascii_alphanumeric()) { return Err(at(&format!("'{}' is not an IFC class", cols[0]))); }
            let family = cols[1].get(..7).filter(|p| p.eq_ignore_ascii_case("family:")).map(|_| cols[1][7..].trim().to_string());
            if family.as_deref() == Some("") { return Err(at("'family:' needs the name of a library family")); }
            let target = if family.is_some() { Target::Category(Category::Generic) } else {
                TARGETS.iter().find(|(n, _)| n.eq_ignore_ascii_case(cols[1])).map(|t| t.1)
                    .ok_or_else(|| at(&format!("'{}' is not a category (use {}, or family:<name of a library family>)", cols[1], TARGETS.map(|t| t.0).join(", "))))?
            };
            let predefined = cols.get(2).copied().filter(|p| !p.is_empty() && *p != "*").map(|p| p.trim_matches('.').to_ascii_uppercase());
            if rows.iter().any(|r| r.class == class && r.predefined == predefined) { return Err(at(&format!("{class} is given twice"))); }
            if rows.len() >= MAX_ROWS { return Err(at("too many rows")); }
            rows.push(Row { class, predefined, target, family });
        }
        Ok(ClassMap { rows })
    }

    /// What the map says the class (with its predefined type, without dots) becomes; the row for the predefined type wins
    /// over the row for any.
    pub fn target(&self, class: &str, predefined: &str) -> Option<Target> {
        let class = class.to_ascii_uppercase();
        let predefined = predefined.trim_matches('.').to_ascii_uppercase();
        self.rows.iter().find(|r| r.class == class && r.predefined.as_deref() == Some(predefined.as_str()) && !predefined.is_empty())
            .or_else(|| self.rows.iter().find(|r| r.class == class && r.predefined.is_none()))
            .map(|r| r.target)
    }

    /// The library family the map says the class (with its predefined type) is read as, if it says one.
    pub fn family(&self, class: &str, predefined: &str) -> Option<&str> {
        let class = class.to_ascii_uppercase();
        let predefined = predefined.trim_matches('.').to_ascii_uppercase();
        self.rows.iter().find(|r| r.class == class && r.predefined.as_deref() == Some(predefined.as_str()) && !predefined.is_empty())
            .or_else(|| self.rows.iter().find(|r| r.class == class && r.predefined.is_none()))
            .and_then(|r| r.family.as_deref())
    }

    pub fn is_empty(&self) -> bool { self.rows.is_empty() }
    pub fn len(&self) -> usize { self.rows.len() }

    /// The map as text, to be saved and edited.
    #[cfg(test)]
    pub fn to_text(&self) -> String {
        let mut s = String::from("# IFC class\tapp category\tpredefined type (optional)\n");
        for r in &self.rows {
            let name = r.family.as_ref().map_or_else(|| TARGETS.iter().find(|t| t.1 == r.target).map_or("Generic", |t| t.0).to_string(), |f| format!("family:{f}"));
            s.push_str(&format!("{}\t{name}{}\n", r.class, r.predefined.as_ref().map_or(String::new(), |p| format!("\t{p}"))));
        }
        s
    }
}

/// A map to start from: what an importer would do for the classes people most often meet, written out so that it can be edited.
pub const EXAMPLE: &str = "# IFC class\tapp category\tpredefined type (optional)\n\
# Categories: Wall, Beam, Column, Slab, Footing, WallFooting, Pipe, Duct, Generic, Ignore\n\
IFCSTAIR\tGeneric\n\
IFCROOF\tGeneric\n\
IFCSLAB\tGeneric\tLANDING\n\
IFCCOVERING\tGeneric\n\
IFCFLOWTERMINAL\tGeneric\n\
IFCFURNISHINGELEMENT\tGeneric\n\
IFCDISTRIBUTIONCHAMBERELEMENT\tGeneric\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_map_is_read_from_tabs_or_spaces_with_comments_and_the_predefined_type_wins_over_any() {
        let m = ClassMap::parse("# comment\nifcstair\tgeneric\nIFCSLAB  Generic  LANDING # a landing\nIFCSLAB\tSlab\n\nIFCMEMBER\tColumn\t*\nIFCPUMP\tIgnore\n").unwrap();
        assert_eq!(m.len(), 5);
        assert_eq!(m.target("IFCSTAIR", ""), Some(Target::Category(Category::Generic)));
        assert_eq!(m.target("IfcStair", "STRAIGHT"), Some(Target::Category(Category::Generic)));
        assert_eq!(m.target("IFCSLAB", "LANDING"), Some(Target::Category(Category::Generic)));
        assert_eq!(m.target("IFCSLAB", ".LANDING."), Some(Target::Category(Category::Generic)));
        assert_eq!(m.target("IFCSLAB", "FLOOR"), Some(Target::Category(Category::Slab)));
        assert_eq!(m.target("IFCSLAB", ""), Some(Target::Category(Category::Slab)));
        assert_eq!(m.target("IFCMEMBER", "BRACE"), Some(Target::Category(Category::Column)));
        assert_eq!(m.target("IFCPUMP", ""), Some(Target::Ignore));
        assert_eq!(m.target("IFCBEAM", ""), None);
    }

    #[test]
    fn a_bad_row_refuses_the_map_and_names_its_line() {
        for (text, line) in [("IFCSTAIR\n", 1), ("IFCWALL\tWall\nSTAIR\tGeneric\n", 2), ("IFCWALL\tDoor\n", 1), ("IFCWALL\tWall\nIFCWALL\tBeam\n", 2), ("IFCWALL\tWall\tA\tB\n", 1), ("IFC\tWall\n", 1), ("IFC WALL\tWall\n", 1)] {
            let e = ClassMap::parse(text).unwrap_err();
            assert!(e.starts_with(&format!("line {line}:")), "{text:?}: {e}");
        }
        assert!(ClassMap::parse(&"x".repeat(MAX_BYTES + 1)).is_err());
    }

    #[test]
    fn a_map_written_out_reads_back_the_same_and_the_example_reads() {
        let m = ClassMap::parse(EXAMPLE).unwrap();
        assert!(m.len() >= 5);
        assert_eq!(ClassMap::parse(&m.to_text()).unwrap(), m);
    }
}
