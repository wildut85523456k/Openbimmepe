//! The document as lines of text, exactly: every element with every value, and the site. This is the app's own record
//! format (no JSON): one line for each element, `key=value` fields between `|`, numbers written so that they read back
//! to the same bits, text with every character that is not a letter, digit, space, `.`, `-`, `_` or `+` written as
//! `~` and six hex digits. The same lines go in a file saved by the app and in the property set that an IFC export
//! carries for the app's own re-import.
use super::{Category, Document, Element, ElementId, Geometry, Material, Section, Site, SlabKind, Usage, Command};

/// Written first in every set of records.
pub const FORMAT: &str = "obim-records-1";

const CATEGORIES: [(Category, &str); 24] = [
    (Category::Wall, "Wall"), (Category::Tag, "Tag"), (Category::Dimension, "Dimension"), (Category::Room, "Room"), (Category::Stair, "Stair"), (Category::Opening, "Opening"), (Category::Section, "Section"), (Category::Elevation, "Elevation"), (Category::Railing, "Railing"), (Category::Beam, "Beam"), (Category::Column, "Column"), (Category::Slab, "Slab"), (Category::Footing, "Footing"),
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
        Some(Geometry::Room { at, level }) => format!("R,{},{:?},{:?}", level.0, at[0], at[1]),
        Some(Geometry::Shaft { level, top, base_offset_m, top_offset_m }) => format!("H,{},{},{:?},{:?}", level.0, top.0, base_offset_m, top_offset_m),
        Some(Geometry::Dimension { a, b, at, level }) => format!("N,{},{:?},{:?},{:?},{:?},{:?},{:?}", level.0, a[0], a[1], b[0], b[1], at[0], at[1]),
        Some(Geometry::Tag { host, at, anchor }) => format!("K,{},{:?},{:?},{:?},{:?}", host.0, at[0], at[1], anchor[0], anchor[1]),
        Some(Geometry::ViewTag { host, view, at, anchor }) => { let (k, id, d) = view.parts(); format!("T,{},{k},{id},{d},{:?},{:?},{:?},{:?}", host.0, at[0], at[1], anchor[0], anchor[1]) }
        Some(Geometry::ViewDimension { view, a, b, at }) => { let (k, id, d) = view.parts(); format!("X,{k},{id},{d},{:?},{:?},{:?},{:?},{:?},{:?}", a[0], a[1], b[0], b[1], at[0], at[1]) }
    }
}

fn num(s: &str) -> Result<f64, String> { s.parse::<f64>().ok().filter(|x| x.is_finite()).ok_or_else(|| format!("'{s}' is not a number")) }
fn int(s: &str) -> Result<u64, String> { s.parse::<u64>().map_err(|_| format!("'{s}' is not a whole number")) }
fn view_ref(kind: &str, id: &str, k: &str) -> Result<crate::ViewRef, String> {
    let small = |s: &str| int(s).and_then(|n| u8::try_from(n).map_err(|_| format!("'{s}' is too big")));
    crate::ViewRef::from_parts(small(kind)?, int(id)?, small(k)?).ok_or_else(|| format!("'{kind}' is not a kind of view"))
}
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
        "H" => { need(5)?; Geometry::Shaft { level: ElementId(int(p[1])?), top: ElementId(int(p[2])?), base_offset_m: num(p[3])?, top_offset_m: num(p[4])? } }
        "R" => { need(4)?; Geometry::Room { level: ElementId(int(p[1])?), at: [num(p[2])?, num(p[3])?] } }
        "N" => { need(8)?; Geometry::Dimension { level: ElementId(int(p[1])?), a: [num(p[2])?, num(p[3])?], b: [num(p[4])?, num(p[5])?], at: [num(p[6])?, num(p[7])?] } }
        "K" => { need(6)?; Geometry::Tag { host: ElementId(int(p[1])?), at: [num(p[2])?, num(p[3])?], anchor: [num(p[4])?, num(p[5])?] } }
        "T" => { need(9)?; Geometry::ViewTag { host: ElementId(int(p[1])?), view: view_ref(p[2], p[3], p[4])?, at: [num(p[5])?, num(p[6])?], anchor: [num(p[7])?, num(p[8])?] } }
        "X" => { need(10)?; Geometry::ViewDimension { view: view_ref(p[1], p[2], p[3])?, a: [num(p[4])?, num(p[5])?], b: [num(p[6])?, num(p[7])?], at: [num(p[8])?, num(p[9])?] } }
        other => return Err(format!("unknown geometry '{other}'")),
    }))
}

/// The layers of a wall type: `material:thickness:function:wraps` for each, between `;`. Records written before wall
/// types had layers have none.
fn parse_layers(s: &str) -> Result<Vec<crate::WallLayer>, String> {
    if s.is_empty() { return Ok(Vec::new()); }
    s.split(';').map(|l| {
        let p: Vec<&str> = l.split(':').collect();
        if p.len() != 4 { return Err("a layer without four parts".to_string()); }
        Ok(crate::WallLayer {
            material: unesc(p[0])?, thickness_m: num(p[1])?,
            function: crate::LayerFunction::from_code(p[2]).ok_or("an unknown layer function")?,
            wraps: p[3] == "T",
        })
    }).collect()
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
    let ly: Vec<String> = e.layers.iter().map(|l| format!("{}:{:?}:{}:{}", esc(&l.material), l.thickness_m, l.function.code(), if l.wraps { 'T' } else { 'F' })).collect();
    let lc = e.core.map_or(String::new(), |(a, b)| format!("{a},{b}"));
    format!(
        "E|id={}|cat={cat}|name={}|w={:?}|h={:?}|el={:?}|sl={}|geo={}|sec={section}|use={usage}|mat={}|out={}|sk={}|se={}|je={}{}|sp={:?}|an={:?}|par={}|cnt={}|fam={}|tn={}|ov={}|tx={}|tid={}|tf={tf}|ly={}|lc={lc}|pn={}",
        e.id.0, esc(&e.name), e.width_m, e.height_m, e.elevation_m, e.slope_ratio.map_or(String::new(), |s| format!("{s:?}")), geometry_text(e.geometry),
        e.material.name(), outline.join(";"), e.slab_kind.name(), e.slope_edges.iter().map(|b| if *b { 'T' } else { 'F' }).collect::<String>(), if e.join_ends[0] { 'T' } else { 'F' }, if e.join_ends[1] { 'T' } else { 'F' },
        e.spacing_m, e.angle_deg, id(e.parent), e.count, esc(&e.family), esc(&e.type_name), ov.join(";"), tx.join(";"), id(e.type_id), ly.join(";"), if e.pinned { 'T' } else { 'F' },
    )
}

/// All the lines of a document: the format, the site, then the elements by number.
pub fn records(doc: &Document) -> Vec<String> {
    let mut out = vec![format!("F|{FORMAT}")];
    if let Some(s) = doc.site() { out.push(format!("S|lat={:?}|lon={:?}|ang={:?}|utc={:?}", s.latitude, s.longitude, s.angle_to_true_north_deg, s.utc_offset_hours)); }
    out.extend(doc.elements().map(element_record));
    out.extend(mep_records(doc.mep()));
    out
}

fn size_text(s: openbim_place::penetrations::RunSize) -> String {
    use openbim_place::penetrations::RunSize;
    match s { RunSize::Round { diameter } => format!("R:{diameter:?}"), RunSize::Rect { width, height, roll } => format!("Q:{width:?}:{height:?}:{roll:?}") }
}

fn parse_size(s: &str) -> Result<openbim_place::penetrations::RunSize, String> {
    use openbim_place::penetrations::RunSize;
    let p: Vec<&str> = s.split(':').collect();
    match (p[0], p.len()) {
        ("R", 2) => Ok(RunSize::Round { diameter: num(p[1])? }),
        ("Q", 4) => Ok(RunSize::Rect { width: num(p[1])?, height: num(p[2])?, roll: num(p[3])? }),
        _ => Err("an unknown pipe or duct size".into()),
    }
}

fn v3_text(v: [f64; 3]) -> String { format!("{:?},{:?},{:?}", v[0], v[1], v[2]) }

fn parse_v3(s: &str) -> Result<[f64; 3], String> {
    let p: Vec<&str> = s.split(',').collect();
    if p.len() != 3 { return Err("a point without three numbers".into()); }
    Ok([num(p[0])?, num(p[1])?, num(p[2])?])
}

fn port_text(p: &super::MepPort) -> String {
    match p.node { super::MepNode::Run(id) => format!("R{}.{}", id.0, p.port), super::MepNode::Fitting(id) => format!("F{id}.{}", p.port) }
}

fn parse_port(s: &str) -> Result<super::MepPort, String> {
    let (node, port) = s.split_once('.').ok_or("a port without '.'")?;
    let n = int(node.get(1..).unwrap_or(""))?;
    let node = match node.as_bytes().first() { Some(b'R') => super::MepNode::Run(ElementId(n)), Some(b'F') => super::MepNode::Fitting(n), _ => return Err("a port of an unknown kind".into()) };
    Ok(super::MepPort { node, port: int(port)? as usize })
}

/// The lines of the fittings (`T`), the links (`L`) and the taps (`P`) of the ducts and pipes.
pub fn mep_records(m: &super::MepData) -> Vec<String> {
    use openbim_route::FittingKind;
    let mut out = Vec::new();
    for f in &m.fittings {
        let (k, a) = match f.fitting.kind { FittingKind::Elbow { angle } => ("E", angle), FittingKind::Transition => ("T", 0.0), FittingKind::Tee { branch_angle } => ("Y", branch_angle), FittingKind::Tap { branch_angle } => ("P", branch_angle) };
        let legs: Vec<String> = (0..f.fitting.ports.len()).map(|i| format!("{}/{}/{:?}", v3_text(f.fitting.ports[i]), size_text(f.fitting.port_sizes[i]), f.fitting.trim[i])).collect();
        out.push(format!("T|id={}|dom={}|sys={}|kind={k}|ang={a:?}|tn={}|pc={}|at={}|rad={:?}|legs={}", f.id, f.domain.code(), f.system.code(), esc(&f.fitting.type_name), esc(&f.fitting.part_code), v3_text(f.fitting.at), f.fitting.radius, legs.join(";")));
    }
    for (a, b) in &m.links { out.push(format!("L|a={}|b={}", port_text(a), port_text(b))); }
    for t in &m.taps { out.push(format!("P|f={}|r={}|d={:?}", t.fitting, t.run.0, t.distance)); }
    for (r, s) in &m.run_systems { out.push(format!("Z|r={}|sys={}", r.0, s.code())); }
    for e in &m.endpoints {
        let dem = match e.demand {
            super::EndDemand::None => "N".to_string(),
            super::EndDemand::Flow(q) => format!("F:{q:?}"),
            super::EndDemand::Power { watts, delta_t } => format!("W:{watts:?}:{delta_t:?}"),
            super::EndDemand::LoadingUnits(u) => format!("U:{u:?}"),
            super::EndDemand::Sprinkler { k_l_min_bar, min_flow_l_min } => format!("K:{k_l_min_bar:?}:{min_flow_l_min:?}"),
        };
        out.push(format!("N|p={}|role={}|dem={dem}|rp={:?}", port_text(&e.port), if e.role == super::EndRole::Source { "S" } else { "T" }, e.required_pressure_pa));
    }
    if m.settings != super::MepSettings::default() {
        let list = |v: &[f64]| v.iter().map(|x| format!("{x:?}")).collect::<Vec<_>>().join(",");
        let s = &m.settings;
        out.push(format!("M|sl={}|dn={}|dw={}|dh={}", list(&s.pipe_slopes_pct), list(&s.pipe_diameters_mm), list(&s.duct_widths_mm), list(&s.duct_heights_mm)));
    }
    for q in &m.equip_links { out.push(format!("Q|p={}|el={}|cn={}", port_text(&q.port), q.element.0, esc(&q.connector))); }
    for n in &m.elec_nodes {
        let kind = match n.kind { super::ElecKind::Source => "S", super::ElecKind::Board => "B", super::ElecKind::Load => "L" };
        let motor = n.motor.map_or("N".to_string(), |(m, p)| format!("{m:?}:{p:?}"));
        out.push(format!("B|id={}|name={}|kind={kind}|at={:?},{:?}|lv={}|w={:?}|pf={:?}|eff={:?}|mot={motor}|el={}|cn={}", n.id, esc(&n.name), n.at[0], n.at[1], n.level.0, n.watts, n.power_factor, n.efficiency, n.element.map_or("N".to_string(), |e| e.0.to_string()), esc(&n.connector)));
    }
    for c in &m.elec_cables {
        let via: Vec<String> = c.via.iter().map(|p| format!("{:?},{:?}", p[0], p[1])).collect();
        out.push(format!("C|a={}|b={}|via={}|ph={}|ex={:?}|gc={}|amb={}|df={:?}|rcd={}", c.from, c.to, via.join(";"), if c.three_phase { 3 } else { 1 }, c.extra_m, c.grouped_circuits, c.ambient_c.map_or("N".into(), |t| format!("{t:?}")), c.demand_factor, c.rcd_ma.map_or("N".into(), |m| format!("{m:?}"))));
    }
    out
}

/// Reads the fittings, links and taps out of the lines `records` wrote (the other lines are not looked at).
pub fn parse_mep(lines: &[String]) -> Result<super::MepData, String> {
    use openbim_family::mep::{Domain, System};
    use openbim_route::{Fitting, FittingKind};
    let mut m = super::MepData::default();
    for line in lines {
        if !["T|", "L|", "P|", "Z|", "N|", "B|", "C|", "Q|", "M|"].iter().any(|p| line.starts_with(p)) { continue; }
        let f = fields(line)?;
        let get = |k: &str| f.get(k).copied().ok_or_else(|| format!("a record without '{k}'"));
        let pt = |t: &str| -> Result<[f64; 2], String> { let (a, b) = t.split_once(',').ok_or("a point without ','")?; Ok([num(a)?, num(b)?]) };
        let opt = |t: &str| -> Result<Option<f64>, String> { if t == "N" { Ok(None) } else { num(t).map(Some) } };
        if line.starts_with("M|") {
            let list = |t: &str| -> Result<Vec<f64>, String> { if t.is_empty() { Ok(Vec::new()) } else { t.split(',').map(num).collect() } };
            m.settings = super::MepSettings { pipe_slopes_pct: list(get("sl")?)?, pipe_diameters_mm: list(get("dn")?)?, duct_widths_mm: list(get("dw")?)?, duct_heights_mm: list(get("dh")?)? };
            continue;
        }
        if line.starts_with("Q|") { m.equip_links.push(super::EquipLink { port: parse_port(get("p")?)?, element: ElementId(int(get("el")?)?), connector: unesc(get("cn")?)? }); continue; }
        if line.starts_with("B|") {
            let kind = match get("kind")? { "S" => super::ElecKind::Source, "B" => super::ElecKind::Board, "L" => super::ElecKind::Load, _ => return Err("an unknown electrical node kind".into()) };
            let motor = match get("mot")? { "N" => None, t => { let (a, b) = t.split_once(':').ok_or("a motor without ':'")?; Some((num(a)?, num(b)?)) } };
            m.elec_nodes.push(super::ElecNode { id: int(get("id")?)?, name: unesc(get("name")?)?, kind, at: pt(get("at")?)?, level: ElementId(int(get("lv")?)?), watts: num(get("w")?)?, power_factor: num(get("pf")?)?, efficiency: num(get("eff")?)?, motor, element: match f.get("el").copied() { None | Some("N") => None, Some(t) => Some(ElementId(int(t)?)) }, connector: f.get("cn").map_or(Ok(String::new()), |t| unesc(t))? });
            continue;
        }
        if line.starts_with("C|") {
            let via = if get("via")?.is_empty() { Vec::new() } else { get("via")?.split(';').map(pt).collect::<Result<Vec<_>, _>>()? };
            m.elec_cables.push(super::ElecCable { from: int(get("a")?)?, to: int(get("b")?)?, via, three_phase: get("ph")? == "3", extra_m: num(get("ex")?)?, grouped_circuits: int(get("gc")?)? as u32, ambient_c: opt(get("amb")?)?, demand_factor: num(get("df")?)?, rcd_ma: opt(get("rcd")?)? });
            continue;
        }
        if line.starts_with("L|") { m.links.push((parse_port(get("a")?)?, parse_port(get("b")?)?)); continue; }
        if line.starts_with("Z|") {
            m.run_systems.push((ElementId(int(get("r")?)?), System::from_code(int(get("sys")?)? as u8).ok_or("an unknown system")?));
            continue;
        }
        if line.starts_with("N|") {
            let d: Vec<&str> = get("dem")?.split(':').collect();
            let demand = match (d[0], d.len()) {
                ("N", 1) => super::EndDemand::None,
                ("F", 2) => super::EndDemand::Flow(num(d[1])?),
                ("W", 3) => super::EndDemand::Power { watts: num(d[1])?, delta_t: num(d[2])? },
                ("U", 2) => super::EndDemand::LoadingUnits(num(d[1])?),
                ("K", 3) => super::EndDemand::Sprinkler { k_l_min_bar: num(d[1])?, min_flow_l_min: num(d[2])? },
                _ => return Err("an unknown end point demand".into()),
            };
            let role = match get("role")? { "S" => super::EndRole::Source, "T" => super::EndRole::Terminal, _ => return Err("an unknown end point role".into()) };
            m.endpoints.push(super::MepEndpoint { port: parse_port(get("p")?)?, role, demand, required_pressure_pa: num(get("rp")?)? });
            continue;
        }
        if line.starts_with("P|") { m.taps.push(super::MepTap { fitting: int(get("f")?)?, run: ElementId(int(get("r")?)?), distance: num(get("d")?)? }); continue; }
        let angle = num(get("ang")?)?;
        let kind = match get("kind")? { "E" => FittingKind::Elbow { angle }, "T" => FittingKind::Transition, "Y" => FittingKind::Tee { branch_angle: angle }, "P" => FittingKind::Tap { branch_angle: angle }, _ => return Err("an unknown fitting kind".into()) };
        let (mut ports, mut port_sizes, mut trim) = (Vec::new(), Vec::new(), Vec::new());
        for leg in get("legs")?.split(';') {
            let p: Vec<&str> = leg.split('/').collect();
            if p.len() != 3 { return Err("a fitting leg without three parts".into()); }
            ports.push(parse_v3(p[0])?);
            port_sizes.push(parse_size(p[1])?);
            trim.push(num(p[2])?);
        }
        let dom = int(get("dom")?)?;
        let sys = int(get("sys")?)?;
        m.fittings.push(super::MepFitting {
            id: int(get("id")?)?,
            fitting: Fitting { type_name: unesc(get("tn")?)?, part_code: unesc(get("pc")?)?, kind, at: parse_v3(get("at")?)?, ports, port_sizes, radius: num(get("rad")?)?, trim },
            domain: Domain::from_code(dom as u8).ok_or("an unknown domain")?,
            system: System::from_code(sys as u8).ok_or("an unknown system")?,
        });
    }
    Ok(m)
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
        if ["T|", "L|", "P|", "Z|", "N|", "B|", "C|", "Q|", "M|"].iter().any(|p| line.starts_with(p)) { continue; } // see `parse_mep`
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
            slope_edges: get("se").unwrap_or("").chars().map(|c| c == 'T').collect(),
            join_ends: [je.starts_with('T'), je.ends_with('T')],
            spacing_m: num(get("sp")?)?, angle_deg: num(get("an")?)?, parent: opt_id(get("par")?)?, count: int(get("cnt")?)? as u32,
            family: unesc(get("fam")?)?, type_name: unesc(get("tn")?)?,
            overrides: pairs(get("ov")?)?.into_iter().map(|(n, v)| Ok((n, num(&v)?))).collect::<Result<_, String>>()?,
            texts: pairs(get("tx")?)?.into_iter().map(|(n, v)| Ok((n, unesc(&v)?))).collect::<Result<_, String>>()?,
            type_id: opt_id(get("tid")?)?,
            type_for: if get("tf")?.is_empty() { None } else { Some(CATEGORIES.iter().find(|c| c.1 == get("tf").unwrap_or("")).map(|c| c.0).ok_or("an unknown type category")?) },
            layers: parse_layers(f.get("ly").copied().unwrap_or(""))?,
            core: match f.get("lc").copied().unwrap_or("") {
                "" => None,
                s => { let (a, b) = s.split_once(',').ok_or("a core without ','")?; Some((int(a)? as usize, int(b)? as usize)) }
            },
            pinned: f.get("pn").copied() == Some("T"),
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
