//! An IFC file into the document: levels, walls, beams, columns, slabs, footings, ducts and pipes that are an
//! extruded profile, and the types they share. All of it is one transaction, so one undo takes it back. What the file
//! has and the document cannot hold is counted in the report.
use openbim_document::{Category, Command, Document, ElementId, Geometry, Material, Section, SlabKind, TypeValues, Usage};
use openbim_ifc::reader::{Imported, Member, Section as Shape};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default, PartialEq)]
pub struct ImportReport {
    pub schema: String,
    /// elements added, by category name
    pub added: BTreeMap<&'static str, usize>,
    pub levels: usize,
    pub types: usize,
    /// elements put back exactly as the app wrote them (from the file's own records)
    pub restored: usize,
    /// things to tell the user: edited elsewhere, deleted elsewhere, own data not used
    pub notes: Vec<String>,
    /// what was left out and why, with how many
    pub skipped: BTreeMap<String, usize>,
}

impl ImportReport {
    pub fn text(&self) -> String {
        let added: Vec<String> = self.added.iter().map(|(k, n)| format!("{n} {k}")).collect();
        let mut s = if self.restored > 0 { format!("Restored {} elements exactly as saved. ", self.restored) } else { String::new() };
        s.push_str(&format!("Imported {} ({}): {}", if added.is_empty() { "nothing".to_string() } else { added.join(", ") }, self.schema, if self.levels > 0 { format!("{} new levels, {} types", self.levels, self.types) } else { format!("{} types", self.types) }));
        for n in &self.notes { s.push_str(&format!(". {n}")); }
        if !self.skipped.is_empty() {
            let left: Vec<String> = self.skipped.iter().map(|(k, n)| format!("{n} {k}")).collect();
            s.push_str(&format!(". Left out: {}", left.join("; ")));
        }
        s
    }
}

fn r6(x: f64) -> f64 { (x * 1e6).round() / 1e6 }

/// What one member of the file becomes.
struct Planned {
    category: Category,
    name: String,
    width: f64,
    height: f64,
    elevation: f64,
    section: Section,
    usage: Usage,
    material: Material,
    slope: Option<f64>,
    slab_kind: SlabKind,
    outline: Vec<[f64; 2]>,
    geometry: Geometry,
    /// strip footings: the wall they lie under, once it is known
    wall_footing_at: Option<([f64; 2], f64)>,
    number: Option<u64>,
    type_obj: Option<usize>,
}

fn material_of(name: Option<&str>) -> Material {
    let n = name.unwrap_or("").to_lowercase();
    if n.contains("concrete") || n.contains("béton") || n.contains("beton") { Material::Concrete }
    else if n.contains("steel") || n.contains("acier") { Material::Steel }
    else if n.contains("timber") || n.contains("wood") || n.contains("bois") { Material::Timber }
    else { Material::Unspecified }
}

fn bbox(pts: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    pts.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]))
}

fn area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| pts[i][0] * pts[(i + 1) % n][1] - pts[(i + 1) % n][0] * pts[i][1]).sum::<f64>().abs() / 2.0
}

/// The section of a column from its footprint: a rectangle along the axes, a circle, or a symmetric I; None for
/// anything else (a turned column, an L shape).
fn column_section(pts: &[[f64; 2]]) -> Option<(Section, f64, f64, [f64; 2])> {
    let (lo, hi) = bbox(pts);
    let (w, d) = (hi[0] - lo[0], hi[1] - lo[1]);
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let tol = 1e-4 * w.max(d).max(1e-3);
    if (area(pts) - w * d).abs() <= 0.002 * w * d { return Some((Section::Rectangle, w, d, centre)); }
    // a circle: all corners the same distance from the middle, and enough of them
    let r: Vec<f64> = pts.iter().map(|p| (p[0] - centre[0]).hypot(p[1] - centre[1])).collect();
    if pts.len() >= 12 && r.iter().all(|x| (x - r[0]).abs() <= 0.01 * r[0]) && (w - d).abs() <= 0.05 * w { return Some((Section::Circle, 2.0 * r[0], 2.0 * r[0], centre)); }
    // an I: twelve corners, symmetric
    if pts.len() == 12 {
        let mut ys: Vec<f64> = pts.iter().map(|p| p[1]).collect();
        let mut xs: Vec<f64> = pts.iter().map(|p| p[0]).collect();
        let dedupe = |v: &mut Vec<f64>| { v.sort_by(f64::total_cmp); v.dedup_by(|a, b| (*a - *b).abs() <= tol); };
        dedupe(&mut ys);
        dedupe(&mut xs);
        if ys.len() == 4 && xs.len() == 4 {
            let (flange, web) = (ys[1] - ys[0], xs[2] - xs[1]);
            if flange > tol && web > tol && ((ys[3] - ys[2]) - flange).abs() <= tol * 10.0 { return Some((Section::IShape { web_m: r6(web), flange_m: r6(flange) }, w, d, centre)); }
        }
    }
    None
}

/// The centre line and thickness of a wall from its footprint: along the longest edge, with both long sides averaged
/// so that mitred ends give the true length.
fn wall_axis(pts: &[[f64; 2]]) -> Option<([f64; 2], [f64; 2], f64)> {
    let n = pts.len();
    let edge = |i: usize| { let (a, b) = (pts[i], pts[(i + 1) % n]); ((b[0] - a[0]).hypot(b[1] - a[1]), a, b) };
    let best = (0..n).map(|i| edge(i).0).fold(0.0, f64::max);
    if best < 1e-6 { return None; }
    // the file lists the left edge going forward first (as this app writes it), so among the long edges the first
    // one gives the direction of the wall: a mitred wall has a long and a short side
    let first = (0..n).map(edge).find(|e| e.0 >= 0.9 * best).unwrap_or_else(|| edge(0));
    let u = [(first.2[0] - first.1[0]) / first.0, (first.2[1] - first.1[1]) / first.0];
    let v = [-u[1], u[0]];
    let proj: Vec<(f64, f64)> = pts.iter().map(|p| (p[0] * u[0] + p[1] * u[1], p[0] * v[0] + p[1] * v[1])).collect();
    let (vmin, vmax) = proj.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let vc = (vmin + vmax) / 2.0;
    let range = |side: bool| -> Option<(f64, f64)> {
        let us: Vec<f64> = proj.iter().filter(|p| (p.1 > vc) == side).map(|p| p.0).collect();
        (!us.is_empty()).then(|| (us.iter().copied().fold(f64::MAX, f64::min), us.iter().copied().fold(f64::MIN, f64::max)))
    };
    let (a, b) = match (range(true), range(false)) {
        (Some(p), Some(m)) => ((p.0 + m.0) / 2.0, (p.1 + m.1) / 2.0),
        (Some(p), None) | (None, Some(p)) => p,
        _ => return None,
    };
    let at = |s: f64| [u[0] * s + v[0] * vc, u[1] * s + v[1] * vc];
    ((b - a) > 1e-6 && vmax - vmin > 1e-6).then(|| (at(a), at(b), vmax - vmin))
}

/// Plans the import of `imp` into `doc`: the commands, and the report.
pub fn plan(doc: &Document, imp: &Imported) -> (Vec<Command>, ImportReport) {
    let mut report = ImportReport { schema: imp.schema.clone(), skipped: imp.skipped.iter().map(|(k, n)| (format!("{k} (not an extruded profile this app reads)"), *n)).collect(), ..Default::default() };
    let mut ops: Vec<Command> = Vec::new();
    let mut used: BTreeSet<u64> = doc.elements().map(|e| e.id.0).collect();
    let mut next = doc.next_id().0;
    // the numbers the file asks for are kept for the elements that ask, so levels and types do not take them
    let wanted: BTreeSet<u64> = imp.members.iter().filter_map(|m| m.psets.get("OpenBIM_Element").and_then(|p| p.get("ElementNumber")).and_then(|p| p.num())).map(|n| n as u64).collect();
    let mut take = |want: Option<u64>, used: &mut BTreeSet<u64>| -> ElementId {
        if let Some(n) = want.filter(|n| *n > 0 && !used.contains(n)) { used.insert(n); return ElementId(n); }
        while used.contains(&next) || wanted.contains(&next) { next += 1; }
        used.insert(next);
        ElementId(next)
    };
    let skip = |report: &mut ImportReport, why: &str| *report.skipped.entry(why.to_string()).or_default() += 1;

    // levels: the document's, and those of the file that it does not have yet
    let mut levels: Vec<(ElementId, f64)> = doc.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level))).map(|e| (e.id, e.elevation_m)).collect();
    let mut storey_level: Vec<ElementId> = Vec::new();
    for s in &imp.storeys {
        let elevation = r6(s.elevation);
        let found = doc.elements().find(|e| matches!(e.geometry, Some(Geometry::Level)) && (e.name.eq_ignore_ascii_case(&s.name) || (e.elevation_m - elevation).abs() < 1e-3))
            .map(|e| e.id).or_else(|| levels.iter().find(|l| (l.1 - elevation).abs() < 1e-3).map(|l| l.0));
        let id = found.unwrap_or_else(|| {
            let id = take(None, &mut used);
            ops.push(Command::Create { id, category: Category::Level, name: if s.name.trim().is_empty() { "Level".into() } else { s.name.clone() }, width_m: 1.0, height_m: 1.0, elevation_m: elevation });
            ops.push(Command::SetGeometry { id, geometry: Geometry::Level });
            levels.push((id, elevation));
            report.levels += 1;
            id
        });
        storey_level.push(id);
    }
    if levels.is_empty() {
        let id = take(None, &mut used);
        ops.push(Command::Create { id, category: Category::Level, name: "Level 1".into(), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 });
        ops.push(Command::SetGeometry { id, geometry: Geometry::Level });
        levels.push((id, 0.0));
        report.levels += 1;
    }
    levels.sort_by(|a, b| a.1.total_cmp(&b.1));
    // the level a member stands on: its storey, else the highest level at or below `z`, else the lowest
    let level_for = |m: &Member, z: f64| -> (ElementId, f64) {
        if let Some(l) = m.storey.and_then(|s| storey_level.get(s)).and_then(|id| levels.iter().find(|l| l.0 == *id)) { return *l; }
        levels.iter().rev().find(|l| l.1 <= z + 1e-3).copied().unwrap_or(levels[0])
    };

    let mut planned: Vec<Planned> = Vec::new();
    for m in imp.members.iter() {
        let number = m.psets.get("OpenBIM_Element").and_then(|p| p.get("ElementNumber")).and_then(|p| p.num()).map(|n| n as u64);
        let class = m.class.as_str();
        let category = match class {
            "IFCBEAM" | "IFCMEMBER" => Category::Beam,
            "IFCCOLUMN" => Category::Column,
            "IFCSLAB" if m.predefined == "LANDING" => { skip(&mut report, "IFCSLAB LANDING (the landing of a stair written by another program is not read yet)"); continue }
            "IFCSLAB" => Category::Slab,
            "IFCCOVERING" if m.predefined == "CEILING" => Category::Slab,
            "IFCCOVERING" => { skip(&mut report, "IFCCOVERING that is not a ceiling (no such element in the app)"); continue }
            "IFCSTAIR" => { skip(&mut report, "IFCSTAIR (a stair written by another program is not read yet)"); continue }
            "IFCROOF" => { skip(&mut report, "IFCROOF (a roof written by another program is not read yet)"); continue }
            "IFCFOOTING" => if m.predefined == "STRIP_FOOTING" { Category::WallFooting } else { Category::Footing },
            "IFCWALL" | "IFCWALLSTANDARDCASE" => Category::Wall,
            "IFCPIPESEGMENT" => Category::Pipe,
            "IFCDUCTSEGMENT" => Category::Duct,
            _ => { skip(&mut report, &format!("{class} (no such element in the app)")); continue }
        };
        if let Some(n) = number {
            if doc.get(ElementId(n)).is_some_and(|e| e.category == category) { skip(&mut report, "elements already in the model (same element number)"); continue; }
        }
        let Some(body) = &m.body else { skip(&mut report, &format!("{class} without an extruded profile")); continue };
        let mut p = Planned {
            category, name: m.name.clone(), width: 0.0, height: 0.0, elevation: 0.0, section: Section::Rectangle, usage: Usage::Girder,
            material: material_of(m.material.as_deref()), slope: None, slab_kind: SlabKind::Floor, outline: Vec::new(),
            geometry: Geometry::Level, wall_footing_at: None, number, type_obj: m.type_obj,
        };
        let pts2: Vec<[f64; 2]> = body.outline.iter().map(|q| [r6(q[0]), r6(q[1])]).collect();
        let (z_lo, z_hi) = { let e = body.end(); (body.centre[2].min(e[2]), body.centre[2].max(e[2])) };
        let (z_lo, z_hi) = (r6(z_lo), r6(z_hi));
        match category {
            Category::Beam | Category::Pipe | Category::Duct => {
                let (a, b) = (body.centre, body.end());
                let run = (b[0] - a[0]).hypot(b[1] - a[1]);
                if run < 1e-6 || body.vertical() { skip(&mut report, &format!("{class} that runs straight up or down")); continue; }
                let (lid, lz) = level_for(m, a[2].min(b[2]));
                p.elevation = r6(a[2] - lz);
                p.slope = Some(((b[2] - a[2]) / run).clamp(-100.0, 100.0)).map(|s| if s.abs() < 1e-9 { 0.0 } else { s });
                p.geometry = Geometry::Linear { start: [r6(a[0]), r6(a[1])], end: [r6(b[0]), r6(b[1])], level: lid };
                match (category, body.section) {
                    (Category::Pipe, Shape::Circle { d }) => { (p.width, p.height) = (r6(d), r6(d)); }
                    (Category::Pipe, _) => { skip(&mut report, "pipes that are not round"); continue }
                    (Category::Duct, Shape::Rect { w, h }) => { (p.width, p.height) = (r6(w), r6(h)); }
                    (Category::Duct, Shape::Circle { d }) => { (p.width, p.height) = (r6(d), r6(d)); }
                    (Category::Duct, _) => { skip(&mut report, "ducts with a section that is not a rectangle or a circle"); continue }
                    (_, Shape::Rect { w, h }) => { (p.width, p.height) = (r6(w), r6(h)); }
                    (_, Shape::Circle { d }) => { (p.width, p.height, p.section) = (r6(d), r6(d), Section::Circle); }
                    (_, Shape::I { w, d, web, flange }) => { (p.width, p.height, p.section) = (r6(w), r6(d), Section::IShape { web_m: r6(web), flange_m: r6(flange) }); }
                    (_, Shape::Polygon) => { skip(&mut report, &format!("{class} with a profile that is not a rectangle, circle or I")); continue }
                }
                if category == Category::Beam {
                    p.usage = match (class, m.predefined.as_str()) {
                        ("IFCMEMBER", "BRACE") => Usage::Brace,
                        ("IFCMEMBER", "PURLIN") => Usage::Purlin,
                        ("IFCMEMBER", _) => Usage::Brace,
                        (_, "JOIST") => Usage::Joist,
                        (_, "BEAM") | (_, "GIRDER") => Usage::Girder,
                        (_, "") => Usage::Girder,
                        _ => Usage::Other,
                    };
                } else { p.material = Material::Unspecified; }
            }
            Category::Column => {
                if !body.vertical() { skip(&mut report, "columns that are not upright"); continue; }
                let Some((section, w, d, at)) = column_section(&pts2) else { skip(&mut report, "columns with a section that is not a rectangle, circle or I along the axes"); continue };
                let (lid, lz) = level_for(m, z_lo);
                let Some(top) = levels.iter().find(|l| l.1 > lz + 1e-3 && (l.1 - z_hi).abs() <= 0.005) else { skip(&mut report, "columns whose top is not on a level"); continue };
                (p.width, p.height, p.section, p.elevation) = (r6(w), r6(d), section, r6(z_lo - lz));
                p.geometry = Geometry::Point { at: [r6(at[0]), r6(at[1])], level: lid, top: top.0 };
            }
            Category::Slab | Category::Footing | Category::WallFooting | Category::Wall => {
                if !body.vertical() || pts2.len() < 3 { skip(&mut report, &format!("{class} that is not a prism standing on its profile")); continue; }
                let (lid, lz) = level_for(m, z_hi);
                let thickness = r6(z_hi - z_lo);
                let (lo, hi) = bbox(&pts2);
                match category {
                    Category::Slab => {
                        (p.width, p.height, p.elevation) = (thickness, thickness, r6(z_hi - lz));
                        p.outline = pts2.clone();
                        p.slab_kind = if class == "IFCCOVERING" { SlabKind::Ceiling } else if m.predefined == "BASESLAB" { SlabKind::Foundation } else { SlabKind::Floor };
                        // a ceiling is placed by its underside
                        if p.slab_kind == SlabKind::Ceiling { p.elevation = r6(z_lo - lz); }
                        p.geometry = Geometry::Area { level: lid };
                    }
                    Category::Footing => {
                        (p.width, p.height, p.elevation) = (r6(hi[0] - lo[0]), r6(hi[1] - lo[1]), r6(z_hi - lz));
                        p.geometry = Geometry::Pad { at: [r6((lo[0] + hi[0]) / 2.0), r6((lo[1] + hi[1]) / 2.0)], level: lid, thickness_m: thickness };
                    }
                    Category::WallFooting => {
                        let Some((s, e, w)) = wall_axis(&pts2) else { skip(&mut report, "strip footings without a length"); continue };
                        (p.width, p.height) = (r6(w), thickness);
                        p.wall_footing_at = Some(([(s[0] + e[0]) / 2.0, (s[1] + e[1]) / 2.0], z_hi));
                        p.elevation = z_hi; // made relative to the wall below
                    }
                    _ => {
                        let Some((s, e, t)) = wall_axis(&pts2) else { skip(&mut report, "walls without a length"); continue };
                        (p.width, p.height, p.elevation) = (r6(t), thickness, r6(z_lo - lz));
                        p.geometry = Geometry::Linear { start: [r6(s[0]), r6(s[1])], end: [r6(e[0]), r6(e[1])], level: lid };
                        p.material = Material::Unspecified;
                    }
                }
            }
            _ => continue,
        }
        if p.name.trim().is_empty() { p.name = format!("{} {}", class.trim_start_matches("IFC").to_lowercase(), m.step_id); }
        planned.push(p);
    }

    // types: instances of one type that agree on the values a type governs follow it; the rest keep their values
    let governed = |p: &Planned| TypeValues {
        width_m: p.width, height_m: p.height, section: p.section, material: p.material, usage: p.usage,
        thickness_m: if let Geometry::Pad { thickness_m, .. } = p.geometry { thickness_m } else { 0.0 }, ..Default::default()
    }.governed(p.category);
    let mut taken_names: BTreeSet<String> = doc.elements().filter(|e| e.category == Category::Type).map(|e| e.name.clone()).collect();
    let mut type_ids: BTreeMap<usize, ElementId> = BTreeMap::new();
    let mut follows: BTreeMap<usize, usize> = BTreeMap::new(); // planned index -> type object
    let mut by_type: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, p) in planned.iter().enumerate() {
        if let (Some(t), true) = (p.type_obj, openbim_document::Element::can_have_type(p.category)) { by_type.entry(t).or_default().push(i); }
    }
    for (t, members) in &by_type {
        let kind = planned[members[0]].category;
        let gov = governed(&planned[members[0]]);
        let same: Vec<usize> = members.iter().copied().filter(|i| planned[*i].category == kind && governed(&planned[*i]) == gov).collect();
        for i in members { if !same.contains(i) { skip(&mut report, "elements whose size differs from their type (imported without it)"); } }
        let base = if imp.types[*t].name.trim().is_empty() { "Type".to_string() } else { imp.types[*t].name.trim().to_string() };
        let mut name = base.clone();
        let mut k = 2;
        while taken_names.contains(&name) { name = format!("{base} ({k})"); k += 1; }
        taken_names.insert(name.clone());
        let tid = take(None, &mut used);
        ops.push(Command::Create { id: tid, category: Category::Type, name, width_m: gov.width_m, height_m: gov.height_m, elevation_m: 0.0 });
        ops.push(Command::SetTypeFor { id: tid, category: kind });
        ops.extend(gov.commands(tid, kind));
        type_ids.insert(*t, tid);
        report.types += 1;
        for i in same { follows.insert(i, *t); }
    }

    // walls first (strip footings need them), then the rest in the order of the file
    let mut wall_axes: Vec<(ElementId, [f64; 2], [f64; 2], f64)> = doc.elements().filter_map(|e| match (e.category, e.geometry) {
        (Category::Wall, Some(Geometry::Linear { start, end, level })) => Some((e.id, start, end, doc.get(level).map_or(0.0, |l| l.elevation_m) + e.elevation_m)),
        _ => None,
    }).collect();
    let order: Vec<usize> = (0..planned.len()).filter(|i| planned[*i].category == Category::Wall).chain((0..planned.len()).filter(|i| planned[*i].category != Category::Wall)).collect();
    for i in order {
        let p = &planned[i];
        let id = take(p.number, &mut used);
        let mut geometry = p.geometry;
        let mut elevation = p.elevation;
        if p.category == Category::WallFooting {
            let (c, z_top) = p.wall_footing_at.unwrap_or(([0.0; 2], 0.0));
            let host = wall_axes.iter().find(|(_, s, e, _)| {
                let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
                let l2 = dx * dx + dy * dy;
                let t = (((c[0] - s[0]) * dx + (c[1] - s[1]) * dy) / l2).clamp(0.0, 1.0);
                (c[0] - (s[0] + t * dx)).hypot(c[1] - (s[1] + t * dy)) < 0.02
            });
            let Some((hid, _, _, wall_base)) = host.copied() else { skip(&mut report, "strip footings with no wall above them"); continue };
            geometry = Geometry::OnWall { host: hid };
            elevation = r6(z_top - wall_base);
        }
        ops.push(Command::Create { id, category: p.category, name: p.name.clone(), width_m: p.width, height_m: p.height, elevation_m: elevation });
        match p.category {
            Category::Beam => ops.extend([Command::SetSection { id, section: p.section }, Command::SetUsage { id, usage: p.usage }, Command::SetMaterial { id, material: p.material }, Command::SetSlope { id, slope_ratio: p.slope.unwrap_or(0.0) }]),
            Category::Column => ops.extend([Command::SetSection { id, section: p.section }, Command::SetMaterial { id, material: p.material }]),
            Category::Slab => ops.extend([Command::SetSlabKind { id, kind: p.slab_kind }, Command::SetMaterial { id, material: p.material }, Command::SetOutline { id, outline: p.outline.clone() }]),
            Category::Footing | Category::WallFooting => ops.push(Command::SetMaterial { id, material: p.material }),
            Category::Pipe | Category::Duct => { if let Some(s) = p.slope.filter(|s| *s != 0.0) { ops.push(Command::SetSlope { id, slope_ratio: s }); } }
            _ => {}
        }
        ops.push(Command::SetGeometry { id, geometry });
        if let Some(t) = follows.get(&i).and_then(|t| type_ids.get(t)) { ops.push(Command::SetType { id, type_id: Some(*t) }); }
        if let (Category::Wall, Geometry::Linear { start, end, level }) = (p.category, geometry) {
            wall_axes.push((id, start, end, levels.iter().find(|l| l.0 == level).map_or(0.0, |l| l.1) + elevation));
        }
        let name = match p.category { Category::Wall => "walls", Category::Beam => "beams", Category::Column => "columns", Category::Slab => "slabs", Category::Footing => "pad footings", Category::WallFooting => "strip footings", Category::Pipe => "pipes", _ => "ducts" };
        *report.added.entry(name).or_default() += 1;
    }
    (ops, report)
}

/// One line about the file at `path` for the import window: its schema, and whether it carries this app's own data.
pub fn describe_file(path: &str) -> String {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return "The file cannot be opened.".into() };
    let mut head = vec![0u8; 4096];
    let n = f.read(&mut head).unwrap_or(0);
    let head = String::from_utf8_lossy(&head[..n]).to_ascii_uppercase();
    if !head.starts_with("ISO-10303-21") { return "This is not an IFC (STEP) file.".into(); }
    let schema = head.split("FILE_SCHEMA((").nth(1).and_then(|s| s.split('\'').nth(1)).unwrap_or("unknown schema");
    format!("Schema {schema}")
}

/// The element numbers an element points at.
fn refs(e: &openbim_document::Element) -> Vec<ElementId> {
    let mut v: Vec<ElementId> = e.parent.into_iter().chain(e.type_id).collect();
    match e.geometry {
        Some(Geometry::Linear { level, .. } | Geometry::Placed { level, .. } | Geometry::Area { level } | Geometry::Pad { level, .. }) => v.push(level),
        Some(Geometry::Point { level, top, .. }) => v.extend([level, top]),
        Some(Geometry::InWall { host, .. } | Geometry::OnMember { host } | Geometry::OnWall { host }) => v.push(host),
        _ => {}
    }
    v
}

fn same(a: &openbim_document::Element, b: &openbim_document::Element) -> bool {
    *a == openbim_document::Element { revision: a.revision, ..b.clone() }
}

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-5 }
fn close2(a: [f64; 2], b: [f64; 2]) -> bool { close(a[0], b[0]) && close(a[1], b[1]) }

/// Whether the file still shows `native` (an element of the saved model, in `nd`) as it wrote it: `read` is the same
/// element as the file's geometry gives it (in `gd`, a model made from the file alone).
fn unchanged_in_file(native: &openbim_document::Element, nd: &Document, read: &openbim_document::Element, gd: &Document, v2: bool) -> bool {
    let lev = |d: &Document, id: ElementId| d.get(id).map(|l| l.elevation_m);
    let same_level = |a: ElementId, b: ElementId| match (lev(nd, a), lev(gd, b)) { (Some(x), Some(y)) => close(x, y), _ => false };
    // a height is the same height whichever level it is counted from
    let base = |d: &Document, el: &openbim_document::Element| match el.geometry {
        Some(Geometry::Linear { level, .. } | Geometry::Point { level, .. } | Geometry::Pad { level, .. } | Geometry::Area { level }) => lev(d, level).map(|l| l + el.elevation_m),
        _ => Some(el.elevation_m),
    };
    let geometry = match (native.geometry, read.geometry) {
        (Some(Geometry::Linear { start: a, end: b, .. }), Some(Geometry::Linear { start: c, end: d, .. })) => {
            // a wall is written with its joins (mitred corners, ends stopped at the face of the wall they meet), so the
            // axis the file's outline gives is the axis of the joined wall, not the drawn one
            let joined = if native.category == Category::Wall { nd.wall_plan(native.id).and_then(|r| wall_axis(&r)).map(|(s, e, _)| (s, e)) } else { None };
            match joined { Some((s, e)) => close2(s, c) && close2(e, d), None => close2(a, c) && close2(b, d) }
        }
        (Some(Geometry::Point { at: a, top: t, .. }), Some(Geometry::Point { at: b, top: u, .. })) => close2(a, b) && same_level(t, u),
        (Some(Geometry::Pad { at: a, thickness_m: t, .. }), Some(Geometry::Pad { at: b, thickness_m: u, .. })) => close2(a, b) && close(t, u),
        (Some(Geometry::Area { .. }), Some(Geometry::Area { .. })) => true,
        (Some(Geometry::OnWall { host: a }), Some(Geometry::OnWall { host: b })) => a == b,
        _ => false,
    };
    let section = match (native.section, read.section) {
        (Section::IShape { web_m: a, flange_m: b }, Section::IShape { web_m: c, flange_m: d }) => close(a, c) && close(b, d),
        (a, b) => a == b,
    };
    geometry && section && native.category == read.category && native.name == read.name
        && close(native.width_m, read.width_m) && close(native.height_m, read.height_m) && matches!((base(nd, native), base(gd, read)), (Some(a), Some(b)) if close(a, b))
        && match (native.slope_ratio, read.slope_ratio) { (Some(a), Some(b)) => close(a, b), (a, b) => a == b }
        && (v2 || native.usage == read.usage) && native.material == read.material && native.slab_kind == read.slab_kind
        && native.outline.len() == read.outline.len() && native.outline.iter().zip(&read.outline).all(|(a, b)| close2(*a, *b))
}

/// The fittings, links and taps already in the model, and those of the file that do not clash with them. A link whose
/// run is gone is dropped by the model itself when the import is applied.
fn merge_mep(have: &openbim_document::MepData, incoming: openbim_document::MepData) -> openbim_document::MepData {
    let mut out = have.clone();
    let mut refused: BTreeSet<u64> = BTreeSet::new();
    for f in incoming.fittings {
        match have.fitting(f.id) { None => out.fittings.push(f), Some(h) if *h == f => {}, Some(_) => { refused.insert(f.id); } }
    }
    let involves = |p: &openbim_document::MepPort| matches!(p.node, openbim_document::MepNode::Fitting(id) if refused.contains(&id));
    for (a, b) in incoming.links {
        if involves(&a) || involves(&b) || out.links.contains(&(a, b)) || out.links.contains(&(b, a)) { continue; }
        if out.partner(a).is_none() && out.partner(b).is_none() { out.links.push((a, b)); }
    }
    for t in incoming.taps {
        if !refused.contains(&t.fitting) && !out.taps.iter().any(|x| x.fitting == t.fitting) { out.taps.push(t); }
    }
    for (r, s) in incoming.run_systems {
        if out.run_system(r).is_none() { out.run_systems.push((r, s)); }
    }
    for e in incoming.endpoints {
        if !involves(&e.port) && out.endpoint_at(e.port).is_none() && out.partner(e.port).is_none() { out.endpoints.push(e); }
    }
    for q in incoming.equip_links {
        if !out.equip_links.iter().any(|x| x.port == q.port) && out.partner(q.port).is_none() { out.equip_links.push(q); }
    }
    for n in incoming.elec_nodes {
        if !out.elec_nodes.iter().any(|x| x.id == n.id) { out.elec_nodes.push(n); }
    }
    for c in incoming.elec_cables {
        if out.elec_nodes.iter().any(|n| n.id == c.from) && out.elec_nodes.iter().any(|n| n.id == c.to) && !out.elec_cables.iter().any(|x| x.to == c.to) { out.elec_cables.push(c); }
    }
    out
}

/// Puts back the model from the file's own records. Every element that the file still shows with the values the app
/// wrote comes back exactly as it was; one that was changed in another program is read from the file's geometry
/// (with the same number), one deleted there stays deleted, one added there is imported as a new element.
fn restore(doc: &mut Document, imp: &Imported) -> Result<ImportReport, String> {
    let (elements, site) = openbim_document::native::parse_records(&imp.native)?;
    let mep_in = openbim_document::native::parse_mep(&imp.native)?;
    let merged_fittings = mep_in.fittings.len();
    let number_of = |m: &Member| m.psets.get("OpenBIM_Element").and_then(|p| p.get("ElementNumber")).and_then(|p| p.num()).map(|n| n as u64);
    let by_number: BTreeMap<u64, &Member> = imp.members.iter().filter_map(|m| number_of(m).map(|n| (n, m))).collect();
    // IFC2X3 has no pipes or ducts here, and no predefined type to say what a beam is for
    let v2 = imp.schema == "IFC2X3";
    let exported = |c: Category| matches!(c, Category::Wall | Category::Beam | Category::Column | Category::Slab | Category::Footing | Category::WallFooting) || (!v2 && matches!(c, Category::Duct | Category::Pipe));
    let (mut keep, mut changed, mut deleted, mut already) = (Vec::new(), BTreeSet::new(), 0usize, 0usize);
    let native_numbers: BTreeSet<u64> = elements.iter().map(|e| e.id.0).collect();
    // the file's geometry alone, to see what another program has changed
    let mut saved = Document::new();
    saved.transact(0, &openbim_document::native::restore_commands(elements.clone(), None)).map_err(|e| format!("the records do not make a model: {e:?}"))?;
    let mut read = Document::new();
    {
        let only_members = Imported { skipped: BTreeMap::new(), native: Vec::new(), ..imp.clone() };
        let (ops, _) = plan(&read, &only_members);
        if !ops.is_empty() { let _ = read.transact(0, &ops); }
    }
    for e in elements {
        if let Some(existing) = doc.get(e.id) {
            if same(existing, &e) { already += 1; continue; }
            return Err(format!("element number {} is already used by another element of the model", e.id.0));
        }
        if exported(e.category) {
            match by_number.get(&e.id.0) {
                None => { deleted += 1; continue }
                Some(m) => {
                    let fp = m.psets.get("OpenBIM_Element").and_then(|p| p.get("Fingerprint")).and_then(|p| p.text());
                    // a roof is a solid of several planes, which is not read back: it stays as the records have it
                    let same_values = e.is_roof() || e.category == Category::Stair || read.get(e.id).is_some_and(|g| unchanged_in_file(&e, &saved, g, &read, v2));
                    if !e.is_roof() && e.category != Category::Stair && (fp != Some(crate::export::fingerprint(&e).as_str()) || !same_values) { changed.insert(e.id.0); continue; }
                }
            }
        }
        keep.push(e);
    }
    // what points at something that is gone goes with it
    let mut dropped = 0;
    loop {
        let present: BTreeSet<u64> = keep.iter().map(|e| e.id.0).chain(doc.elements().map(|e| e.id.0)).chain(changed.iter().copied()).collect();
        let before = keep.len();
        keep.retain(|e| refs(e).iter().all(|r| present.contains(&r.0)));
        if keep.len() == before { break; }
        dropped += before - keep.len();
    }
    let restored = keep.len();
    let site = site.filter(|_| doc.site().is_none());
    let restore_ops = openbim_document::native::restore_commands(keep, site);
    let mut scratch = doc.clone();
    if !restore_ops.is_empty() { scratch.transact(scratch.revision(), &restore_ops).map_err(|e| format!("{e:?}"))?; }
    // members that are new, or that were changed elsewhere, are read from their geometry
    let rest = Imported {
        members: imp.members.iter().filter(|m| number_of(m).is_none_or(|n| changed.contains(&n) || !native_numbers.contains(&n))).cloned().collect(),
        skipped: BTreeMap::new(), native: Vec::new(), ..imp.clone()
    };
    let (ops2, mut report) = plan(&scratch, &rest);
    // products the records account for (doors, windows, equipment, railings, and the ports and bars of what was exported) are not news
    let explained = saved.elements().filter(|e| matches!(e.category, Category::Family | Category::Equipment | Category::Railing)).count() + merged_fittings;
    let others: usize = imp.skipped.iter().filter(|(k, _)| !matches!(k.as_str(), "IFCDISTRIBUTIONPORT" | "IFCREINFORCINGBAR")).map(|(_, n)| *n).sum();
    report.skipped = if others <= explained { BTreeMap::new() } else {
        imp.skipped.iter().filter(|(k, _)| !matches!(k.as_str(), "IFCDISTRIBUTIONPORT" | "IFCREINFORCINGBAR")).map(|(k, n)| (format!("{k} (not an extruded profile this app reads)"), *n)).collect()
    };
    let mut all: Vec<Command> = restore_ops.into_iter().chain(ops2).collect();
    // the fittings and what is joined to what come back with the runs they were joined to
    let merged = merge_mep(doc.mep(), mep_in);
    if merged != *doc.mep() { all.push(Command::SetMep(Box::new(merged))); }
    report.schema = imp.schema.clone();
    if all.is_empty() {
        report.notes.push(format!("The model already holds everything in this file ({already} elements were already in the model)"));
        return Ok(report);
    }
    doc.transact(doc.revision(), &all).map_err(|e| format!("{e:?}"))?;
    report.restored = restored;
    if already > 0 { report.notes.push(format!("{already} elements were already in the model")); }
    if !changed.is_empty() { report.notes.push(format!("{} elements were changed in another program: read from the file's geometry, their other values are not kept", changed.len())); }
    if deleted > 0 { report.notes.push(format!("{deleted} elements were deleted in another program and stay deleted")); }
    if dropped > 0 { report.notes.push(format!("{dropped} elements were left out because what they sit on is gone")); }
    Ok(report)
}

/// Reads `text` and adds what it holds to the document, as one transaction.
pub fn import_ifc(doc: &mut Document, text: &str) -> Result<ImportReport, String> {
    let imp = openbim_ifc::reader::read(text).map_err(|e| e.to_string())?;
    let mut note = None;
    if !imp.native.is_empty() {
        match restore(doc, &imp) {
            Ok(report) => return Ok(report),
            Err(why) => note = Some(format!("The app's own data in the file could not be used ({why}); the file's geometry was read instead")),
        }
    }
    let (ops, mut report) = plan(doc, &imp);
    report.notes.extend(note);
    if report.added.is_empty() && report.levels == 0 { return Ok(report); }
    doc.transact(doc.revision(), &ops).map_err(|e| format!("the model does not accept what the file holds: {e:?}"))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::{BeamSpec, ColumnSpec, FootingSpec, SlabSpec, WallFootingSpec};

    fn model() -> Document {
        let mut d = Document::spatial_sample();
        let a = d.place_beam(ElementId(10), &BeamSpec { material: Material::Concrete, ..BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8) }).unwrap();
        let b = d.place_beam(ElementId(10), &BeamSpec { material: Material::Concrete, ..BeamSpec::level([1.0, 2.0], [5.0, 2.0], 0.3, 0.5, 2.8) }).unwrap();
        d.place_beam(ElementId(11), &BeamSpec { section: Section::Circle, material: Material::Steel, ..BeamSpec::level([1.0, 4.0], [5.0, 4.0], 0.3, 0.3, 0.2) }).unwrap();
        d.place_beam(ElementId(10), &BeamSpec { usage: Usage::Brace, material: Material::Steel, slope_ratio: 1.0, ..BeamSpec::level([6.0, 1.0], [6.0, 4.0], 0.1, 0.1, 0.1) }).unwrap();
        let t = d.create_type(a, "Beam 300 x 500").unwrap();
        d.assign_type(&[b], t).unwrap();
        d.place_column(ElementId(10), ElementId(11), [4.0, 3.0], &ColumnSpec { width_m: 0.4, depth_m: 0.3, base_offset_m: 0.0, section: Section::Rectangle, material: Material::Concrete }).unwrap();
        d.place_column(ElementId(10), ElementId(11), [6.0, 5.0], &ColumnSpec { width_m: 0.35, depth_m: 0.35, base_offset_m: 0.0, section: Section::Circle, material: Material::Steel }).unwrap();
        d.place_slab(ElementId(11), &SlabSpec { outline: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }).unwrap();
        let pad = FootingSpec { width_m: 1.2, length_m: 1.0, thickness_m: 0.4, top_offset_m: -0.1, material: Material::Concrete };
        let p1 = d.place_footing(ElementId(10), [2.0, 2.0], &pad).unwrap();
        let p2 = d.place_footing(ElementId(10), [6.0, 2.0], &pad).unwrap();
        let pt = d.create_type(p1, "Pad 1200 x 1000 x 400").unwrap();
        d.assign_type(&[p2], pt).unwrap();
        let wt = d.create_type(ElementId(20), "Wall 200 x 3000").unwrap();
        d.assign_type(&[ElementId(21)], wt).unwrap();
        d.place_wall_footing(ElementId(20), &WallFootingSpec { width_m: 0.6, depth_m: 0.3, top_offset_m: -0.05, material: Material::Concrete }).unwrap();
        d
    }

    /// The values that an import has to give back, by category and sorted: the same model comes back.
    fn digest(d: &Document) -> Vec<String> {
        let mut v: Vec<String> = d.elements().filter(|e| !matches!(e.category, Category::Grid | Category::Level)).map(|e| {
            let g = match e.geometry {
                Some(Geometry::Linear { start, end, .. }) => format!("{start:.3?}{end:.3?}"),
                Some(Geometry::Point { at, .. }) => format!("{at:.3?}"),
                Some(Geometry::Pad { at, thickness_m, .. }) => format!("{at:.3?}{thickness_m:.3}"),
                Some(Geometry::OnWall { .. }) => "on a wall".into(),
                _ => String::new(),
            };
            let t = e.type_id.and_then(|t| d.get(t)).map_or(String::new(), |t| t.name.clone());
            format!("{:?}|{:.3}|{:.3}|{:.3}|{:?}|{:?}|{:?}|{:.3?}|{g}|{t}|{:.3?}", e.category, e.width_m, e.height_m, e.elevation_m, e.section, e.usage, e.material, e.slope_ratio.unwrap_or(0.0), e.outline)
        }).collect();
        v.sort();
        v
    }

    /// Every value of every element, with the revision (which is the document's bookkeeping) left out.
    fn all(d: &Document) -> Vec<openbim_document::Element> {
        d.elements().map(|e| openbim_document::Element { revision: 0, ..e.clone() }).collect()
    }

    /// A model with one of everything the app can hold.
    fn everything() -> Document {
        use openbim_document::{BeamSystemSpec, FamilySpec, RebarSpec, TrussSpec};
        let mut d = model();
        let beam = d.place_beam(ElementId(10), &BeamSpec { material: Material::Concrete, ..BeamSpec::level([0.5, 5.0], [7.5, 5.0], 0.25, 0.6, 2.4) }).unwrap();
        d.place_rebar(beam, &RebarSpec { count: 3, bar_m: 0.016, stirrup_m: 0.008, cover_m: 0.04, stirrup_spacing_m: 0.2 }).unwrap();
        d.place_beam_system(ElementId(11), &BeamSystemSpec { outline: vec![[0.5, 0.5], [7.5, 0.5], [7.5, 5.5], [0.5, 5.5]], spacing_m: 1.5, angle_deg: 0.0, width_m: 0.15, depth_m: 0.3, elevation_m: -0.2, section: Section::Rectangle, usage: Usage::Joist, material: Material::Timber }).unwrap();
        d.place_truss(ElementId(11), &TrussSpec { start: [1.0, 5.8], end: [7.0, 5.8], spacing_m: 1.5, depth_m: 1.0, member_m: 0.1, elevation_m: 0.1, material: Material::Steel }).unwrap();
        let door = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![("rise".into(), 0.0)], angle_deg: 0.0, elevation_m: 0.0 };
        let roof = d.place_roof(ElementId(11), &openbim_document::RoofSpec { outline: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]], thickness_m: 0.2, base_offset_m: 0.0, slope_ratio: 0.5, edges: vec![true, false, true, false], material: Material::Timber }).unwrap();
        d.place_stair(ElementId(10), &openbim_document::StairSpec { start: [1.0, 1.0], toward: [1.0, 9.0], width_m: 1.0, rise_m: 3.0, risers: 18, tread_m: 0.28, base_offset_m: 0.0 }).unwrap();
        d.place_railing(ElementId(10), &openbim_document::RailingSpec::new(vec![[1.5, 1.0], [1.5, 5.0], [2.5, 5.0]])).unwrap();
        d.place_stair_with_railing(ElementId(10), &openbim_document::StairSpec { start: [5.0, 1.0], toward: [5.0, 9.0], width_m: 1.0, rise_m: 3.0, risers: 17, tread_m: 0.28, base_offset_m: 0.0 }, openbim_document::StairShape::U { left: false }, openbim_document::RailingSides::Both).unwrap();
        d.place_wall_opening(ElementId(22), 5.0, 6.0, 0.5, 2.0).unwrap();
        // vertical openings: one in the roof (cut in its shape) and one in a floor (a void of the slab)
        d.place_vertical_opening(roof, &[[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]]).unwrap();
        let floor = d.elements().find(|e| e.category == Category::Slab && !e.is_roof() && e.slab_kind == openbim_document::SlabKind::Floor).expect("the model has a floor").id;
        d.place_vertical_opening(floor, &[[0.5, 4.5], [1.5, 4.5], [1.5, 5.5], [0.5, 5.5]]).unwrap();
        d.place_shaft(ElementId(10), &openbim_document::ShaftSpec { outline: vec![[6.5, 0.5], [7.1, 0.5], [7.1, 1.1], [6.5, 1.1]], top: ElementId(11), base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        d.place_ceiling(ElementId(10), vec![[0.2, 0.2], [3.8, 0.2], [3.8, 5.8], [0.2, 5.8]], 2.7, 0.02, Material::Unspecified).unwrap();
        d.place_family_in_wall(ElementId(20), 3.0, &FamilySpec { overrides: vec![], ..door.clone() }).unwrap();
        d.place_family_in_wall(ElementId(21), 2.0, &FamilySpec { family: "Window, fixed with mullions".into(), overrides: vec![], elevation_m: 0.9, ..door.clone() }).unwrap();
        d.place_family(ElementId(10), [2.0, 4.5], &FamilySpec { family: "Fan coil unit, ducted".into(), overrides: vec![], angle_deg: 90.0, elevation_m: 2.2, ..door }).unwrap();
        let b = d.place_beam(ElementId(10), &BeamSpec::level([1.0, 5.5], [5.0, 5.5], 0.2, 0.3, 2.5)).unwrap();
        // a duct with an elbow and a pipe with an elbow, joined to their fittings
        {
            use openbim_family::mep::{Domain, System};
            use openbim_place::penetrations::RunSize;
            let off = openbim_route::snaps::Snaps::new(openbim_route::snaps::SnapSettings::off());
            let rect = RunSize::Rect { width: 0.4, height: 0.3, roll: 0.0 };
            let duct = openbim_route::route_path(&[[1.0, 0.8, 2.7], [5.0, 0.8, 2.7], [5.0, 1.8, 2.7]], &[rect, rect], &crate::export::tests_support::prefs(Domain::Duct, System::SupplyAir), &off).unwrap();
            d.place_mep_route(ElementId(10), &duct).unwrap();
            let round = RunSize::Round { diameter: 0.1 };
            let pipe = openbim_route::route_path(&[[1.0, 3.5, 0.6], [6.0, 3.5, 0.6], [6.0, 3.0, 0.4]], &[round, round], &crate::export::tests_support::prefs(Domain::Pipe, System::ChilledWaterSupply), &off).unwrap();
            d.place_mep_route(ElementId(10), &pipe).unwrap();
        }
        d.transact(d.revision(), &[Command::SetJoins { id: b, start: false, end: true }, Command::SetJoins { id: ElementId(20), start: true, end: false }, Command::SetSlope { id: ElementId(2), slope_ratio: -0.02 }, Command::Create { id: ElementId(900), category: Category::Equipment, name: "Pump \u{b5} | 1; two=2".into(), width_m: 0.6, height_m: 0.5, elevation_m: 0.1 }]).unwrap();
        d
    }

    #[test]
    fn exported_fittings_are_joined_to_their_ducts_and_pipes_by_ports_and_the_network_is_in_the_file() {
        let d = everything();
        assert_eq!((d.mep().fittings.len(), d.mep().links.len()), (2, 4));
        let out = crate::export::export_ifc_as(&d, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4x3).unwrap();
        let count = |what: &str| out.text.matches(what).count();
        assert_eq!(count("IFCDUCTFITTING("), 1, "the duct elbow");
        assert_eq!(count("IFCPIPEFITTING("), 1, "the pipe elbow");
        assert_eq!(count("IFCRELCONNECTSPORTS("), 4, "each elbow leg is joined to the end of its run");
        assert_eq!(count("IFCDISTRIBUTIONSYSTEM("), 3, "the air, the chilled water, and the sample's own duct and pipe whose system is not set");
        assert!(!out.warnings.iter().any(|w| w.contains("no longer meet")), "{:?}", out.warnings);
        // a run moved after it was joined no longer meets its fitting: the link is left out, said so, and nothing else changes
        let mut moved = d.clone();
        let run = d.mep().links.iter().find_map(|(a, b)| match (a.node, b.node) { (openbim_document::MepNode::Run(r), _) | (_, openbim_document::MepNode::Run(r)) => Some(r), _ => None }).unwrap();
        let el = moved.get(run).unwrap().clone();
        let Some(Geometry::Linear { start, end, level }) = el.geometry else { panic!() };
        moved.transact(moved.revision(), &[Command::SetGeometry { id: run, geometry: Geometry::Linear { start: [start[0] + 0.5, start[1]], end: [end[0] + 0.5, end[1]], level } }]).unwrap();
        let out = crate::export::export_ifc_as(&moved, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4x3).unwrap();
        assert!(out.warnings.iter().any(|w| w.contains("no longer meet")), "{:?}", out.warnings);
        assert!(out.text.matches("IFCRELCONNECTSPORTS(").count() < 4);
    }

    #[test]
    fn a_branched_network_with_a_tee_and_a_tap_keeps_its_joins_end_points_and_systems_through_the_file() {
        use openbim_document::{EndDemand, EndRole, MepEndpoint, MepNode, MepPort};
        use openbim_family::mep::{Domain, System};
        use openbim_place::penetrations::{Run, RunSize};
        use openbim_route::snaps::{Snaps, SnapSettings};
        let mut d = model();
        let off = Snaps::new(SnapSettings::off());
        let rect = RunSize::Rect { width: 0.5, height: 0.3, roll: 0.0 };
        let duct = openbim_route::route_path(&[[0.5, 0.8, 2.7], [7.0, 0.8, 2.7]], &[rect], &crate::export::tests_support::prefs(Domain::Duct, System::SupplyAir), &off).unwrap();
        let main = d.place_mep_route(ElementId(10), &duct).unwrap()[0];
        let small = RunSize::Rect { width: 0.3, height: 0.2, roll: 0.0 };
        let mut on = main;
        for (x, tee) in [(2.5, true), (5.5, false)] {
            let mut p = crate::export::tests_support::prefs(Domain::Duct, System::SupplyAir);
            p.junction = if tee { openbim_route::Junction::Tee } else { openbim_route::Junction::Tap };
            let run: Run = d.mep_run(on).unwrap();
            let br = openbim_route::branch_from_run(&run, &[[x, 0.8, 2.7], [x, 3.8, 2.7]], &[small], &p, &off).unwrap();
            let ids = d.place_mep_branch(ElementId(10), on, &br).unwrap();
            if tee { on = ids[0]; }
        }
        // a source at the start and a terminal at the end of the branch
        let mut data = d.mep().clone();
        data.endpoints.push(MepEndpoint { port: MepPort { node: MepNode::Run(main), port: 0 }, role: EndRole::Source, demand: EndDemand::None, required_pressure_pa: 0.0 });
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        assert_eq!((d.mep().fittings.len(), d.mep().taps.len()), (2, 1));
        for schema in crate::export::Schema::ALL {
            let out = crate::export::export_ifc_as(&d, "P", 1, "2026-10-18T00:00:00", schema).unwrap();
            assert!(!out.warnings.iter().any(|w| w.contains("no longer")), "{schema:?} {:?}", out.warnings);
            let mut back = Document::new();
            import_ifc(&mut back, &out.text).unwrap();
            assert_eq!(back.mep(), d.mep(), "{schema:?}: joins, taps, systems and end points come back");
        }
    }

    #[test]
    fn the_electrical_network_comes_back_from_a_file_in_every_schema() {
        use openbim_document::{ElecCable, ElecKind, ElecNode};
        let mut d = model();
        let lv = ElementId(10);
        let mut data = d.mep().clone();
        let base = data.next_node_id();
        let mut load = ElecNode::new(base + 2, "Pump \u{b5} | 1; x=2", ElecKind::Load, [6.5, 2.25], lv);
        (load.watts, load.power_factor, load.efficiency, load.motor) = (7500.0, 0.82, 0.9, Some((6.5, 0.3)));
        data.elec_nodes = vec![ElecNode::new(base, "TX", ElecKind::Source, [0.0, 0.0], lv), ElecNode::new(base + 1, "MSB", ElecKind::Board, [3.0, 0.0], lv), load];
        let mut c = ElecCable::new(base + 1, base + 2, false);
        (c.via, c.extra_m, c.grouped_circuits, c.ambient_c, c.demand_factor, c.rcd_ma) = (vec![[3.0, 2.25]], 2.5, 4, Some(40.0), 0.7, Some(30.0));
        data.elec_cables = vec![ElecCable::new(base, base + 1, true), c];
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        for schema in crate::export::Schema::ALL {
            let out = crate::export::export_ifc_as(&d, "P", 1, "2026-10-18T00:00:00", schema).unwrap();
            let mut back = Document::new();
            import_ifc(&mut back, &out.text).unwrap();
            assert_eq!(back.mep(), d.mep(), "{schema:?}: nodes and cables come back");
        }
    }

    #[test]
    fn a_file_the_app_wrote_comes_back_with_nothing_lost_in_every_schema() {
        let d = everything();
        let kinds: BTreeSet<String> = d.elements().map(|e| format!("{:?}", e.category)).collect();
        assert!(kinds.len() >= 15, "the model should use every kind: {kinds:?}");
        for schema in crate::export::Schema::ALL {
            let out = crate::export::export_ifc_as(&d, "P", 1, "2026-10-18T00:00:00", schema).unwrap();
            assert_eq!(out.text.matches("IFCCOVERING(").count(), 1, "{schema:?}: the ceiling");
            assert!(out.text.contains(".CEILING."), "{schema:?}");
            // the straight stair, and the two runs of the U stair with their landing slab
            assert_eq!(out.text.matches("IFCSTAIR(").count(), 3, "{schema:?}: the stairs");
            assert_eq!(out.text.matches(".LANDING.").count(), 1, "{schema:?}: the landing");
            // the free railing and the two of the U stair, which follow its runs and landing
            assert_eq!(out.text.matches("IFCRAILING(").count(), 3, "{schema:?}: the railings");
            // its top rails, posts and balusters are solids of their own: 2 rails, 3 posts, 25 + 6 balusters
            assert!(out.text.matches("IFCFACETEDBREP(").count() >= 36, "{schema:?}: the parts of the railing");
            // the shaft voids the floor above it; the wall opening voids its wall; the door and the window void theirs
            assert_eq!(out.text.lines().filter(|l| l.contains("IFCOPENINGELEMENT(") && l.contains("Shaft Opening 1")).count(), 1, "{schema:?}: the shaft is an opening of the floor");
            assert_eq!(out.text.matches("IFCOPENINGELEMENT(").count(), 5, "{schema:?}: door, window, wall opening, shaft and the vertical opening in the floor");
            // the one in the roof is cut in the roof's shape, not a void of its own
            assert_eq!(out.text.lines().filter(|l| l.contains("IFCOPENINGELEMENT(") && l.contains("Vertical Opening")).count(), 1, "{schema:?}");
            assert!(out.text.contains("Pset_StairCommon") && out.text.contains(".STRAIGHT_RUN_STAIR."), "{schema:?}");
            assert_eq!(out.text.matches("IFCROOF(").count(), 1, "{schema:?}");
            assert!(out.text.contains(".GABLE_ROOF.") && out.text.contains("IFCFACETEDBREP"), "{schema:?}: the roof is a gable solid");
            let mut back = Document::new();
            let report = import_ifc(&mut back, &out.text).unwrap();
            assert_eq!(back.site(), d.site());
            assert_eq!(back.mep(), d.mep(), "{schema:?}: the fittings and what is joined to what come back");
            let (want, got) = (all(&d), all(&back));
            for (w, g) in want.iter().zip(&got) { assert_eq!(w, g, "{schema:?}: {}", report.text()); }
            assert_eq!(want.len(), got.len(), "{schema:?}: {}", report.text());
            assert_eq!(report.restored, want.len());
            assert!(report.skipped.is_empty() && report.notes.is_empty(), "{schema:?}: {}", report.text());
        }
    }

    #[test]
    fn the_file_without_the_apps_own_data_still_gives_the_geometry_and_types() {
        let d = model();
        let out = crate::export::export_ifc_with(&d, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4x3, false).unwrap();
        assert!(!out.text.contains("OpenBIM_Document"));
        let mut back = Document::new();
        let report = import_ifc(&mut back, &out.text).unwrap();
        assert_eq!(report.restored, 0);
        // a wall that stops at the face of another has only that length in the file: without the app's own data
        // the file's geometry is all there is
        let want: Vec<String> = digest(&d).into_iter().map(|l| l.replace("[4.000, 0.000][4.000, 6.000]", "[4.000, 0.100][4.000, 5.900]")).collect();
        let got = digest(&back);
        assert_eq!(want, got, "{}", report.text());
    }

    #[test]
    fn changes_made_in_another_program_are_taken_and_the_rest_is_kept() {
        let d = everything();
        let text = crate::export::export_ifc_as(&d, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4x3).unwrap().text;
        // another program makes the first beam's section 0.35 wide, removes a column and adds a wall
        let mut imp = openbim_ifc::reader::read(&text).unwrap();
        let col = imp.members.iter().position(|m| m.class == "IFCCOLUMN").unwrap();
        imp.members.remove(col);
        let beam = imp.members.iter().position(|m| m.class == "IFCBEAM" && m.psets.contains_key("OpenBIM_Element")).unwrap();
        let number = imp.members[beam].psets["OpenBIM_Element"]["ElementNumber"].num().unwrap() as u64;
        if let Some(b) = imp.members[beam].body.as_mut() { if let openbim_ifc::reader::Section::Rect { w, .. } = &mut b.section { *w += 0.05; } }
        let mut wall = imp.members.iter().find(|m| m.class == "IFCWALL").unwrap().clone();
        wall.psets.clear();
        wall.name = "New wall".into();
        if let Some(b) = wall.body.as_mut() { for p in &mut b.outline { p[1] += 20.0; } b.centre[1] += 20.0; }
        imp.members.push(wall);
        let mut back = Document::new();
        let report = restore(&mut back, &imp).unwrap();
        assert!(report.restored > 0);
        assert!(report.notes.iter().any(|n| n.contains("deleted in another program")), "{}", report.text());
        assert!(report.notes.iter().any(|n| n.contains("changed in another program")), "{}", report.text());
        assert!(back.elements().any(|e| e.name == "New wall"), "{}", report.text());
        assert_eq!(back.get(ElementId(number)).unwrap().width_m, d.get(ElementId(number)).unwrap().width_m + 0.05);
        // what was not touched is exactly as it was
        let norm = |e: Option<&openbim_document::Element>| e.map(|e| openbim_document::Element { revision: 0, ..e.clone() });
        assert_eq!(norm(back.get(ElementId(21))), norm(d.get(ElementId(21))));
        assert!(back.get(ElementId(900)).is_some());
    }

    #[test]
    fn an_import_is_one_undo_and_does_not_double_what_is_already_there() {
        let d = everything();
        let text = crate::export::export_ifc_as(&d, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4x3).unwrap().text;
        let mut back = Document::new();
        import_ifc(&mut back, &text).unwrap();
        back.undo().unwrap();
        assert_eq!(back.elements().count(), 0, "one undo takes the whole import back");
        import_ifc(&mut back, &text).unwrap();
        let n = back.elements().count();
        let again = import_ifc(&mut back, &text).unwrap();
        assert_eq!(back.elements().count(), n, "{}", again.text());
        assert!(again.notes.iter().any(|n| n.contains("already in the model")), "{}", again.text());
    }
}
