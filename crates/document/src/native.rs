//! The document as lines of text, exactly: every element with every value, and the site. This is the app's own record
//! format (no JSON): one line for each element, `key=value` fields between `|`, numbers written so that they read back
//! to the same bits, text with every character that is not a letter, digit, space, `.`, `-`, `_` or `+` written as
//! `~` and six hex digits. The same lines go in a file saved by the app and in the property set that an IFC export
//! carries for the app's own re-import.
use super::{Category, Document, Element, ElementId, Geometry, Material, Section, Site, SlabKind, Usage, Command};

/// Written first in every set of records.
pub const FORMAT: &str = "obim-records-1";

const CATEGORIES: [(Category, &str); 16] = [
    (Category::Wall, "Wall"), (Category::Beam, "Beam"), (Category::Column, "Column"), (Category::Slab, "Slab"), (Category::Footing, "Footing"),
    (Category::WallFooting, "WallFooting"), (Category::BeamSystem, "BeamSystem"), (Category::Truss, "Truss"), (Category::Rebar, "Rebar"),
    (Category::Family, "Family"), (Category::Type, "Type"), (Category::Duct, "Duct"), (Category::Pipe, "Pipe"), (Category::Equipment, "Equipment"),
    (Category::Level, "Level"), (Category::Grid, "Grid"),
];

fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_' | '+') { o.push(c); } else { o.push_str(&format!("~{:06x}", c as u32)); }
    }
    o
}

fn unesc(s: &str) -> Result<String, String> {
    let mut o = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '~' { o.push(c); continue; }
        let hex: String = it.by_ref().take(6).collect();
        let ch = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).ok_or("a bad escape in a record")?;
        o.push(ch);
    }
    Ok(o)
}

fn id(x: Option<ElementId>) -> String { x.map_or(String::new(), |i| i.0.to_string()) }

fn geometry_text(g: Option<Geometry>) -> String {
    match g {
        None => String::new(),
        Some(Geometry::Level) => "V".into(),
        Some(Geometry::Grid { start, end }) => format!("G,{:?},{:?},{:?},{:?}", start[0], start[1], end[0], end[1]),
        Some(Geometry::Linear { start, end, level }) => format!("L,{:?},{:?},{:?},{:?},{}", start[0], start[1], end[0], end[1], level.0),
        Some(Geometry::Point { at, level, top }) => format!("P,{:?},{:?},{},{}", at[0], at[1], level.0, top.0),
        Some(Geometry::Placed { at, level }) => format!("D,{:?},{:?},{}", at[0], at[1], level.0),
        Some(Geometry::InWall { host, along_m }) => format!("W,{},{:?}", host.0, along_m),
        Some(Geometry::OnMember { host }) => format!("M,{}", host.0),
        Some(Geometry::Area { level }) => format!("A,{}", level.0),
        Some(Geometry::Pad { at, level, thickness_m }) => format!("F,{:?},{:?},{},{:?}", at[0], at[1], level.0, thickness_m),
        Some(Geometry::OnWall { host }) => format!("O,{}", host.0),
    }
}

fn num(s: &str) -> Result<f64, String> { s.parse::<f64>().ok().filter(|x| x.is_finite()).ok_or_else(|| format!("'{s}' is not a number")) }
fn int(s: &str) -> Result<u64, String> { s.parse::<u64>().map_err(|_| format!("'{s}' is not a whole number")) }
fn opt_id(s: &str) -> Result<Option<ElementId>, String> { if s.is_empty() { Ok(None) } else { int(s).map(|n| Some(ElementId(n))) } }

fn parse_geometry(s: &str) -> Result<Option<Geometry>, String> {
    if s.is_empty() { return Ok(None); }
    let p: Vec<&str> = s.split(',').collect();
    let need = |n: usize| if p.len() == n { Ok(()) } else { Err(format!("a geometry record has {} fields, not {n}", p.len())) };
    Ok(Some(match p[0] {
        "V" => { need(1)?; Geometry::Level }
        "G" => { need(5)?; Geometry::Grid { start: [num(p[1])?, num(p[2])?], end: [num(p[3])?, num(p[4])?] } }
        "L" => { need(6)?; Geometry::Linear { start: [num(p[1])?, num(p[2])?], end: [num(p[3])?, num(p[4])?], level: ElementId(int(p[5])?) } }
        "P" => { need(5)?; Geometry::Point { at: [num(p[1])?, num(p[2])?], level: ElementId(int(p[3])?), top: ElementId(int(p[4])?) } }
        "D" => { need(4)?; Geometry::Placed { at: [num(p[1])?, num(p[2])?], level: ElementId(int(p[3])?) } }
        "W" => { need(3)?; Geometry::InWall { host: ElementId(int(p[1])?), along_m: num(p[2])? } }
        "M" => { need(2)?; Geometry::OnMember { host: ElementId(int(p[1])?) } }
        "A" => { need(2)?; Geometry::Area { level: ElementId(int(p[1])?) } }
        "F" => { need(5)?; Geometry::Pad { at: [num(p[1])?, num(p[2])?], level: ElementId(int(p[3])?), thickness_m: num(p[4])? } }
        "O" => { need(2)?; Geometry::OnWall { host: ElementId(int(p[1])?) } }
        other => return Err(format!("unknown geometry '{other}'")),
    }))
}

/// The line of one element.
pub fn element_record(e: &Element) -> String {
    let cat = CATEGORIES.iter().find(|c| c.0 == e.category).map_or("", |c| c.1);
    let section = match e.section { Section::Rectangle => "R".to_string(), Section::Circle => "C".into(), Section::IShape { web_m, flange_m } => format!("I,{web_m:?},{flange_m:?}") };
    let usage = Usage::ALL.iter().find(|u| **u == e.usage).map_or("", |u| u.name());
    let outline: Vec<String> = e.outline.iter().map(|p| format!("{:?},{:?}", p[0], p[1])).collect();
    let ov: Vec<String> = e.overrides.iter().map(|(n, v)| format!("{},{v:?}", esc(n))).collect();
    let tx: Vec<String> = e.texts.iter().map(|(n, v)| format!("{},{}", esc(n), esc(v))).collect();
    let tf = e.type_for.and_then(|t| CATEGORIES.iter().find(|c| c.0 == t)).map_or("", |c| c.1);
    format!(
        "E|id={}|cat={cat}|name={}|w={:?}|h={:?}|el={:?}|sl={}|geo={}|sec={section}|use={usage}|mat={}|out={}|sk={}|je={}{}|sp={:?}|an={:?}|par={}|cnt={}|fam={}|tn={}|ov={}|tx={}|tid={}|tf={tf}",
        e.id.0, esc(&e.name), e.width_m, e.height_m, e.elevation_m, e.slope_ratio.map_or(String::new(), |s| format!("{s:?}")), geometry_text(e.geometry),
        e.material.name(), outline.join(";"), e.slab_kind.name(), if e.join_ends[0] { 'T' } else { 'F' }, if e.join_ends[1] { 'T' } else { 'F' },
        e.spacing_m, e.angle_deg, id(e.parent), e.count, esc(&e.family), esc(&e.type_name), ov.join(";"), tx.join(";"), id(e.type_id),
    )
}

/// All the lines of a document: the format, the site, then the elements by number.
pub fn records(doc: &Document) -> Vec<String> {
    let mut out = vec![format!("F|{FORMAT}")];
    if let Some(s) = doc.site() { out.push(format!("S|lat={:?}|lon={:?}|ang={:?}|utc={:?}", s.latitude, s.longitude, s.angle_to_true_north_deg, s.utc_offset_hours)); }
    out.extend(doc.elements().map(element_record));
    out
}

fn fields(line: &str) -> Result<std::collections::BTreeMap<&str, &str>, String> {
    line.split('|').skip(1).map(|f| f.split_once('=').ok_or_else(|| "a field without '='".to_string())).collect()
}

/// Reads the lines written by `records`: the elements, and the site if there is one.
pub fn parse_records(lines: &[String]) -> Result<(Vec<Element>, Option<Site>), String> {
    let mut it = lines.iter();
    if it.next().map(String::as_str) != Some(&format!("F|{FORMAT}")) { return Err("these records are not in a format this version reads".into()); }
    let (mut elements, mut site) = (Vec::new(), None);
    for line in it {
        let f = fields(line)?;
        let get = |k: &str| f.get(k).copied().ok_or_else(|| format!("a record without '{k}'"));
        if line.starts_with("S|") {
            site = Some(Site { latitude: num(get("lat")?)?, longitude: num(get("lon")?)?, angle_to_true_north_deg: num(get("ang")?)?, utc_offset_hours: num(get("utc")?)? });
            continue;
        }
        if !line.starts_with("E|") { return Err("a record of an unknown kind".into()); }
        let cat_name = get("cat")?;
        let category = CATEGORIES.iter().find(|c| c.1 == cat_name).map(|c| c.0).ok_or("an unknown category")?;
        let sec: Vec<&str> = get("sec")?.split(',').collect();
        let section = match sec[0] {
            "R" => Section::Rectangle,
            "C" => Section::Circle,
            "I" if sec.len() == 3 => Section::IShape { web_m: num(sec[1])?, flange_m: num(sec[2])? },
            _ => return Err("an unknown section".into()),
        };
        let pairs = |s: &str| -> Result<Vec<(String, String)>, String> {
            if s.is_empty() { return Ok(Vec::new()); }
            s.split(';').map(|p| p.split_once(',').ok_or("a pair without ','".to_string()).and_then(|(a, b)| Ok((unesc(a)?, b.to_string())))).collect()
        };
        let je = get("je")?;
        elements.push(Element {
            id: ElementId(int(get("id")?)?), category, name: unesc(get("name")?)?, width_m: num(get("w")?)?, height_m: num(get("h")?)?, elevation_m: num(get("el")?)?,
            slope_ratio: if get("sl")?.is_empty() { None } else { Some(num(get("sl")?)?) },
            geometry: parse_geometry(get("geo")?)?, section,
            usage: *Usage::ALL.iter().find(|u| u.name() == get("use").unwrap_or("")).ok_or("an unknown usage")?,
            material: *Material::ALL.iter().find(|m| m.name() == get("mat").unwrap_or("")).ok_or("an unknown material")?,
            outline: if get("out")?.is_empty() { Vec::new() } else { get("out")?.split(';').map(|p| { let (a, b) = p.split_once(',').ok_or("a corner without ','")?; Ok([num(a)?, num(b)?]) }).collect::<Result<_, String>>()? },
            slab_kind: *SlabKind::ALL.iter().find(|k| k.name() == get("sk").unwrap_or("")).ok_or("an unknown slab kind")?,
            join_ends: [je.starts_with('T'), je.ends_with('T')],
            spacing_m: num(get("sp")?)?, angle_deg: num(get("an")?)?, parent: opt_id(get("par")?)?, count: int(get("cnt")?)? as u32,
            family: unesc(get("fam")?)?, type_name: unesc(get("tn")?)?,
            overrides: pairs(get("ov")?)?.into_iter().map(|(n, v)| Ok((n, num(&v)?))).collect::<Result<_, String>>()?,
            texts: pairs(get("tx")?)?.into_iter().map(|(n, v)| Ok((n, unesc(&v)?))).collect::<Result<_, String>>()?,
            type_id: opt_id(get("tid")?)?,
            type_for: if get("tf")?.is_empty() { None } else { Some(CATEGORIES.iter().find(|c| c.1 == get("tf").unwrap_or("")).map(|c| c.0).ok_or("an unknown type category")?) },
            revision: 0,
        });
    }
    Ok((elements, site))
}

/// The commands that put these elements into a document, as they were.
pub fn restore_commands(elements: Vec<Element>, site: Option<Site>) -> Vec<Command> {
    let mut ops: Vec<Command> = elements.into_iter().map(|e| Command::Restore(Box::new(e))).collect();
    if let Some(s) = site { ops.push(Command::SetSite(s)); }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_with_every_kind_of_character_and_numbers_come_back_exactly() {
        for s in ["", "plain 1-2_3+4.5", "pipe|and=equals,comma;semi:colon~tilde", "tab\tnew\nline\\back'quote", "Café – 日本 😀"] {
            assert_eq!(unesc(&esc(s)).unwrap(), s);
            assert!(esc(s).chars().all(|c| c.is_ascii() && c != '|' && c != ',' && c != ';' && c != '=' && c != '\n'));
        }
        let mut e = Document::spatial_sample().get(ElementId(20)).unwrap().clone();
        e.width_m = 0.1 + 0.2;
        e.name = "Wall | 1; ü".into();
        let (back, _) = parse_records(&["F|obim-records-1".to_string(), element_record(&e)]).unwrap();
        assert_eq!(back[0], Element { revision: 0, ..e });
    }

    #[test]
    fn a_document_survives_its_records_and_bad_records_are_refused() {
        let d = Document::spatial_sample();
        let lines = records(&d);
        let (els, site) = parse_records(&lines).unwrap();
        let mut back = Document::new();
        back.transact(0, &restore_commands(els, site)).unwrap();
        let norm = |d: &Document| d.elements().map(|e| Element { revision: 0, ..e.clone() }).collect::<Vec<_>>();
        assert_eq!(norm(&d), norm(&back));
        assert_eq!(d.site(), back.site());
        assert!(parse_records(&["F|other".to_string()]).is_err());
        assert!(parse_records(&[lines[0].clone(), "E|id=x".to_string()]).is_err());
        assert!(parse_records(&[lines[0].clone(), "E|id=1|cat=Nope".to_string()]).is_err());
    }
}
