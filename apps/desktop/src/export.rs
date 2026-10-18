//! The document as an IFC 4.3 file (IFC4 on request): levels, walls and beams. The document's other elements are named in the warnings.
use openbim_arch::{Level, Levels};
use openbim_document::{Category, Document, Geometry, Material, Usage};
use openbim_family::mep::{Domain, System};
use openbim_geom::WallChain;
use openbim_route::graph::{NetSegment, Network};
use openbim_ifc::{MemberInfo, TypeClass, TypeItem, TypeProp, BeamItem, ColumnItem, FamilyItem, FamilyRef, FramingUsage, Frame, Scene, SlabItem, SlabRole, WallItem};

pub struct Exported {
    pub text: String,
    pub walls: usize,
    pub beams: usize,
    pub columns: usize,
    pub slabs: usize,
    pub warnings: Vec<String>,
}

/// The beam of a document element: its line at the height of its centre on its level, and its section.
pub fn beam_item(doc: &Document, el: &openbim_document::Element) -> Option<BeamItem> {
    let ext = doc.beam_extent(el)?;
    let usage = match el.usage {
        Usage::Girder => FramingUsage::Girder,
        Usage::Joist => FramingUsage::Joist,
        Usage::Purlin => FramingUsage::Purlin,
        Usage::Brace => FramingUsage::Brace,
        Usage::Other => FramingUsage::Other,
    };
    let bars = doc.rebars_of(el.id).into_iter().filter_map(|r| doc.get(r)).flat_map(|r| {
        let paths = doc.rebar_bars(r).unwrap_or_default();
        paths.into_iter().enumerate().map(|(i, p)| openbim_ifc::BarItem { id: r.id.0, name: format!("{} {}", r.name, i + 1), path: p.path, diameter: p.diameter, shear: p.shear }).collect::<Vec<_>>()
    }).collect();
    Some(BeamItem { id: el.id.0, name: el.name.clone(), a: ext.a, b: ext.b, profile: el.profile(), usage, bars })
}

/// The column of a document element: its section stood on its point from the foot to the top level.
pub fn column_item(doc: &Document, el: &openbim_document::Element) -> Option<ColumnItem> {
    let Some(Geometry::Point { at, level, top }) = el.geometry else { return None };
    let base = doc.get(level)?.elevation_m + el.elevation_m;
    let height = doc.get(top)?.elevation_m - base;
    // an I-shaped footprint is not convex, so such a column cannot bound rooms
    ColumnItem::from_profile(el.id.0, &el.name, &el.profile(), at, base, height, !matches!(el.section, openbim_document::Section::IShape { .. })).ok()
}

/// The compiled sample library, once: families are looked up by name.
pub fn library() -> &'static [openbim_family::Compiled] {
    static LIB: std::sync::OnceLock<Vec<openbim_family::Compiled>> = std::sync::OnceLock::new();
    LIB.get_or_init(|| openbim_family::library::all().unwrap_or_default())
}

/// The instance values of a family (its numbers and yes/no inputs of instance scope, `wall_t` left out because the
/// wall sets it) as solved for `type_name` with `overrides`; None for an unknown family or values that do not solve.
pub fn instance_values(family: &str, type_name: &str, overrides: &[(String, f64)]) -> Option<Vec<(String, f64)>> {
    instance_values_with(family, type_name, overrides, &[])
}

/// `instance_values` with choices and texts set as well.
pub fn instance_values_with(family: &str, type_name: &str, overrides: &[(String, f64)], texts: &[(String, String)]) -> Option<Vec<(String, f64)>> {
    let f = library().iter().find(|f| f.name() == family)?;
    let inst = f.solve(&openbim_family::Request {
        type_name: (!type_name.is_empty()).then_some(type_name),
        overrides: overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect(), texts: texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() }).ok()?;
    Some(f.input_values(&inst).into_iter()
        .filter(|(n, _)| *n != "wall_t" && f.input_scope(n) == Some(openbim_family::Scope::Instance))
        .map(|(n, v)| (n.to_string(), v)).collect())
}

/// An instance-scope choice, text or material input: its name, current value and, for a choice, the options.
pub type TextInput = (String, String, Vec<String>);

/// The choices, texts and materials of a family instance that its instance can set, as solved for `type_name`,
/// `overrides` and `texts`.
pub fn instance_texts(family: &str, type_name: &str, overrides: &[(String, f64)], texts: &[(String, String)]) -> Option<Vec<TextInput>> {
    let f = library().iter().find(|f| f.name() == family)?;
    let inst = f.solve(&openbim_family::Request {
        type_name: (!type_name.is_empty()).then_some(type_name),
        overrides: overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect(), texts: texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() }).ok()?;
    Some(f.input_texts(&inst).into_iter().filter(|(n, _)| f.input_scope(n) == Some(openbim_family::Scope::Instance))
        .map(|(n, v)| (n.to_string(), v.to_string(), f.choice_options(n).map(<[String]>::to_vec).unwrap_or_default())).collect())
}

/// The scene of the document, and what could not be put in it. `timestamp` is written into the header of a file,
/// `project_key` makes the GlobalIds of this project its own.
pub fn scene_of(doc: &Document, project_name: &str, project_key: u64, timestamp: &str) -> Result<(Scene, std::collections::BTreeMap<&'static str, usize>), String> {
    let level_of = |id| doc.get(id).filter(|l| matches!(l.geometry, Some(Geometry::Level))).map(|l| l.elevation_m);
    let levels: Vec<Level> = doc.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level)))
        .map(|e| Level { name: e.name.clone(), elevation: e.elevation_m }).collect();
    let (mut walls, mut beams, mut columns, mut slabs) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut materials = std::collections::BTreeMap::new();
    let mut skipped = std::collections::BTreeMap::<&'static str, usize>::new();
    let mut instances = Vec::new();
    let mut mep = Network::new(1);
    for el in doc.elements() {
        match (el.category, el.geometry) {
            (Category::Wall, Some(Geometry::Linear { start, end, level })) => {
                let mut chain = WallChain::new(vec![start, end], false, el.width_m, el.height_m);
                chain.base_z = level_of(level).ok_or("a wall stands on a level that is gone")? + el.elevation_m;
                walls.push(WallItem { id: el.id.0, name: el.name.clone(), chain, wall_type: None, room_bounding: true });
            }
            (Category::Beam, Some(Geometry::Linear { .. })) => {
                beams.push(beam_item(doc, el).ok_or("a beam lies on a level that is gone")?);
                if el.material != Material::Unspecified { materials.insert(el.id.0, el.material.name().to_string()); }
            }
            (Category::Column, Some(Geometry::Point { .. })) => {
                columns.push(column_item(doc, el).ok_or_else(|| format!("column {} has no height or a section that does not check", el.name))?);
                if el.material != Material::Unspecified { materials.insert(el.id.0, el.material.name().to_string()); }
            }
            (Category::Slab, Some(Geometry::Area { level })) => {
                let top = level_of(level).ok_or("a slab lies on a level that is gone")? + el.elevation_m;
                let slab = openbim_place::floors::Slab { name: el.name.clone(), top, thickness: el.height_m, outline: el.outline.clone(), fire_rating_min: None };
                let role = if el.slab_kind == openbim_document::SlabKind::Foundation { SlabRole::Foundation } else { SlabRole::Structure };
                slabs.push(SlabItem { id: el.id.0, slab, role, room_bounding: true });
                if el.material != Material::Unspecified { materials.insert(el.id.0, el.material.name().to_string()); }
            }
            (Category::Footing, Some(Geometry::Pad { at, level, thickness_m })) => {
                let top = level_of(level).ok_or("a footing lies on a level that is gone")? + el.elevation_m;
                let (hx, hy) = (el.width_m / 2.0, el.height_m / 2.0);
                let outline = vec![[at[0] - hx, at[1] - hy], [at[0] + hx, at[1] - hy], [at[0] + hx, at[1] + hy], [at[0] - hx, at[1] + hy]];
                let slab = openbim_place::floors::Slab { name: el.name.clone(), top, thickness: thickness_m, outline, fire_rating_min: None };
                slabs.push(SlabItem { id: el.id.0, slab, role: SlabRole::PadFooting, room_bounding: false });
                if el.material != Material::Unspecified { materials.insert(el.id.0, el.material.name().to_string()); }
            }
            (Category::WallFooting, Some(Geometry::OnWall { host })) => {
                let wall = doc.get(host).ok_or("a wall footing's wall is gone")?;
                let Some(Geometry::Linear { start, end, level }) = wall.geometry else { return Err("a wall footing's host has no axis".into()) };
                let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
                let len = dx.hypot(dy);
                let (nx, ny) = (-dy / len * el.width_m / 2.0, dx / len * el.width_m / 2.0);
                let top = level_of(level).ok_or("a wall footing lies on a level that is gone")? + wall.elevation_m + el.elevation_m;
                let outline = vec![[start[0] - nx, start[1] - ny], [end[0] - nx, end[1] - ny], [end[0] + nx, end[1] + ny], [start[0] + nx, start[1] + ny]];
                let slab = openbim_place::floors::Slab { name: el.name.clone(), top, thickness: el.height_m, outline, fire_rating_min: None };
                slabs.push(SlabItem { id: el.id.0, slab, role: SlabRole::StripFooting, room_bounding: false });
                if el.material != Material::Unspecified { materials.insert(el.id.0, el.material.name().to_string()); }
            }
            // the beams, members or bars they make are exported in their place
            (Category::Family, Some(Geometry::Placed { .. } | Geometry::InWall { .. })) => instances.push(el),
            (Category::Level | Category::Grid | Category::BeamSystem | Category::Truss | Category::Rebar, _) => {}
            (Category::Duct | Category::Pipe, Some(Geometry::Linear { start, end, level })) => {
                let z = level_of(level).ok_or("a duct or pipe lies on a level that is gone")? + el.elevation_m;
                let rise = (end[0] - start[0]).hypot(end[1] - start[1]) * el.slope_ratio.unwrap_or(0.0);
                let (a, b) = ([start[0], start[1], z], [end[0], end[1], z + rise]);
                // the document does not say which system a run belongs to yet
                let run = if el.category == Category::Pipe {
                    openbim_place::penetrations::Run::round(&el.name, Domain::Pipe, System::Other, a, b, el.width_m)
                } else {
                    openbim_place::penetrations::Run::rect(&el.name, Domain::Duct, System::Other, a, b, el.width_m, el.height_m)
                };
                mep.segments.push(NetSegment { id: el.id.0, run });
            }
            (Category::Duct, _) => *skipped.entry("ducts without a line").or_default() += 1,
            (Category::Pipe, _) => *skipped.entry("pipes without a line").or_default() += 1,
            (Category::Equipment, _) => *skipped.entry("equipment").or_default() += 1,
            _ => *skipped.entry("elements without geometry").or_default() += 1,
        }
    }
    // family instances, after the walls they may stand in
    let wall_index: std::collections::BTreeMap<u64, usize> = walls.iter().enumerate().map(|(i, w): (usize, &WallItem)| (w.id, i)).collect();
    let lib = library();
    let mut items = Vec::new();
    let mut families = Vec::new();
    for el in instances {
        let Some(fi) = lib.iter().position(|f| f.name() == el.family) else { *skipped.entry("family instances of unknown families").or_default() += 1; continue };
        if !families.iter().any(|f: &FamilyRef| f.id == fi as u32 + 1) { families.push(FamilyRef { id: fi as u32 + 1, family: lib[fi].clone() }); }
        let fam_index = families.iter().position(|f| f.id == fi as u32 + 1).unwrap_or(0);
        let angle = el.angle_deg.to_radians();
        let (frame, host, wall_t) = match el.geometry {
            Some(Geometry::Placed { at, level }) => {
                let z = level_of(level).ok_or("a family instance stands on a level that is gone")? + el.elevation_m;
                (Frame { origin: [at[0], at[1], z], x: [angle.cos(), angle.sin()] }, None, None)
            }
            Some(Geometry::InWall { host, along_m }) => {
                let Some(&wi) = wall_index.get(&host.0) else { *skipped.entry("family instances in missing walls").or_default() += 1; continue };
                let mut frame = Frame::on_wall(&walls[wi].chain, 0, along_m);
                frame.origin[2] += el.elevation_m;
                (frame, Some((wi, 0, along_m)), Some(walls[wi].chain.thickness))
            }
            _ => continue,
        };
        let mut overrides: Vec<(&str, f64)> = el.overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        let type_name = (!el.type_name.is_empty()).then_some(el.type_name.as_str());
        // a door or window fills the wall it stands in, whatever the wall's thickness is now
        let instance = match wall_t {
            Some(t) if !overrides.iter().any(|(n, _)| *n == "wall_t") => {
                overrides.push(("wall_t", t));
                lib[fi].solve(&openbim_family::Request { type_name, overrides: overrides.clone(), texts: el.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() })
                    .or_else(|_| { overrides.pop(); lib[fi].solve(&openbim_family::Request { type_name, overrides: overrides.clone(), texts: el.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() }) })
            }
            _ => lib[fi].solve(&openbim_family::Request { type_name, overrides, texts: el.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() }),
        }.map_err(|e| format!("{}: {e:?}", el.name))?;
        // a door or window opens its wall
        if let (Some((wi, _, along)), Some([w, h, sill])) = (host, instance.opening) {
            let c = &mut walls[wi].chain;
            if instance.opening_rise != 0.0 {
                if c.poly_openings.is_empty() { c.poly_openings.resize(1, Vec::new()); }
                if let Some(outline) = instance.opening_outline(along - w / 2.0) { c.poly_openings[0].push(outline); }
            } else {
                if c.openings.is_empty() { c.openings.resize(1, Vec::new()); }
                c.openings[0].push(openbim_geom::Opening { offset: along - w / 2.0, width: w, sill, height: h });
            }
        }
        items.push(FamilyItem { id: el.id.0, name: el.name.clone(), family: fam_index, type_name: type_name.map(str::to_string), frame, instance, host });
    }
    let mep_ids: Vec<u64> = mep.segments.iter().map(|s| s.id).collect();
    let (types, member_info) = types_of(doc, &mep_ids, &beams, &columns, &slabs);
    let scene = Scene {
        project_name: project_name.into(), project_key, beams: beams.clone(), materials, timestamp: timestamp.into(),
        levels: Levels::new(levels).map_err(|e| format!("levels: {e:?}"))?,
        walls, wall_penetrations: vec![], firestops: Default::default(), slabs, columns: columns.clone(), shafts: vec![],
        floor_penetrations: vec![], networks: if mep.segments.is_empty() { vec![] } else { vec![mep] }, families, items, rooms: vec![], wall_types: vec![],
        wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![],
        room_lighting: Default::default(), pset_map: Default::default(), types, member_info,
    };
    Ok((scene, skipped))
}

/// 16 hex digits over the values of an element that the file carries (FNV-1a, so the same on every machine).
fn fingerprint(el: &openbim_document::Element) -> String {
    let text = format!("{:?}", (&el.name, el.width_m, el.height_m, el.elevation_m, el.section, el.material, el.usage, el.geometry, &el.outline));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() { h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3); }
    format!("{h:016x}")
}

/// The IFC type objects of the document's types, and the `OpenBIM_Element` data of the members that were written.
fn types_of(doc: &Document, mep_ids: &[u64], beams: &[BeamItem], columns: &[ColumnItem], slabs: &[SlabItem]) -> (Vec<TypeItem>, std::collections::BTreeMap<u64, MemberInfo>) {
    let written: std::collections::BTreeSet<u64> = mep_ids.iter().copied().chain(beams.iter().map(|b| b.id)).chain(columns.iter().map(|c| c.id)).chain(slabs.iter().map(|s| s.id)).collect();
    let mut info = std::collections::BTreeMap::new();
    for el in doc.elements().filter(|e| written.contains(&e.id.0)) {
        let type_name = el.type_id.and_then(|t| doc.get(t)).map(|t| t.name.clone()).unwrap_or_default();
        info.insert(el.id.0, MemberInfo { category: format!("{:?}", el.category), type_name, fingerprint: fingerprint(el) });
    }
    let mut types = Vec::new();
    for t in doc.elements().filter(|e| e.category == Category::Type) {
        let Some(kind) = t.type_for else { continue };
        let class = match kind {
            Category::Beam => TypeClass::Beam(match t.usage { Usage::Joist => "JOIST", Usage::Girder => "BEAM", _ => "NOTDEFINED" }),
            Category::Column => TypeClass::Column,
            Category::Slab => TypeClass::Slab("FLOOR"),
            Category::WallFooting => TypeClass::StripFooting,
            _ => continue, // rebar types have no IFC type object
        };
        let instances: Vec<u64> = doc.elements().filter(|e| e.type_id == Some(t.id)).map(|e| e.id.0).collect();
        let v = openbim_document::TypeValues::of(t);
        let mut props = vec![("Category".to_string(), TypeProp::Text(format!("{kind:?}"))), ("Width".into(), TypeProp::Len(v.width_m)), ("Height".into(), TypeProp::Len(v.height_m))];
        if matches!(kind, Category::Beam | Category::Column) {
            props.push(("Section".into(), TypeProp::Text(match v.section {
                openbim_document::Section::Rectangle => "Rectangle".into(),
                openbim_document::Section::Circle => "Circle".into(),
                openbim_document::Section::IShape { web_m, flange_m } => format!("I web {web_m} flange {flange_m}"),
            })));
        }
        if v.material != Material::Unspecified { props.push(("Material".into(), TypeProp::Text(v.material.name().into()))); }
        if kind == Category::Beam { props.push(("Usage".into(), TypeProp::Text(format!("{:?}", v.usage)))); }
        types.push(TypeItem { id: t.id.0, name: t.name.clone(), class, props, instances });
    }
    (types, info)
}

/// `timestamp` is written into the header, `project_key` makes the GlobalIds of this project its own.
/// The newest schema, IFC 4.3, which is what the app writes unless told otherwise.
#[cfg(test)]
pub fn export_ifc(doc: &Document, project_name: &str, project_key: u64, timestamp: &str) -> Result<Exported, String> {
    export_ifc_as(doc, project_name, project_key, timestamp, Schema::Ifc4x3)
}

/// The schema of an exported file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schema { Ifc4x3, Ifc4, Ifc2x3 }

impl Schema {
    /// The default first: the newest.
    pub const ALL: [Schema; 3] = [Schema::Ifc4x3, Schema::Ifc4, Schema::Ifc2x3];
    pub fn name(self) -> &'static str { match self { Schema::Ifc4x3 => "IFC 4.3", Schema::Ifc4 => "IFC4", Schema::Ifc2x3 => "IFC2X3" } }
}

pub fn export_ifc_as(doc: &Document, project_name: &str, project_key: u64, timestamp: &str, schema: Schema) -> Result<Exported, String> {
    let (scene, skipped) = scene_of(doc, project_name, project_key, timestamp)?;
    let out = match schema { Schema::Ifc4x3 => openbim_ifc::export_ifc4x3(&scene), Schema::Ifc4 => openbim_ifc::export(&scene), Schema::Ifc2x3 => openbim_ifc::export_ifc2x3(&scene) }.map_err(|e| format!("{e:?}"))?;
    let mut warnings = out.report.warnings;
    warnings.extend(skipped.into_iter().map(|(what, n)| format!("{n} {what} not exported yet")));
    if doc.site().is_some() { warnings.push("the project site is not exported: it needs a coordinate reference system".into()); }
    Ok(Exported { text: out.text, walls: scene.walls.len(), beams: scene.beams.len(), columns: scene.columns.len(), slabs: scene.slabs.len(), warnings })
}

/// `2026-10-17T19:34:56` for a number of seconds since 1970 (UTC).
pub fn iso_time(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::{BeamSpec, ElementId, Section};

    #[test]
    fn the_document_exports_its_walls_and_beams_and_names_what_it_leaves_out() {
        let mut d = Document::spatial_sample();
        let level = BeamSpec::level;
        d.place_beam(ElementId(10), &BeamSpec { material: Material::Concrete, ..level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8) }).unwrap();
        d.place_beam(ElementId(11), &BeamSpec { section: Section::Circle, material: Material::Steel, ..level([1.0, 4.0], [5.0, 4.0], 0.3, 0.3, 0.2) }).unwrap();
        d.place_beam(ElementId(11), &BeamSpec { section: Section::IShape { web_m: 0.01, flange_m: 0.02 }, material: Material::Steel, ..level([1.0, 5.0], [5.0, 5.0], 0.2, 0.4, 0.2) }).unwrap();
        // a diagonal brace over 3 m of run rising 3 m
        d.place_beam(ElementId(10), &BeamSpec { usage: Usage::Brace, material: Material::Steel, slope_ratio: 1.0, ..level([6.0, 1.0], [6.0, 4.0], 0.1, 0.1, 0.1) }).unwrap();
        let out = export_ifc(&d, "Sample", 1, "2026-10-17T00:00:00").unwrap();
        assert_eq!((out.walls, out.beams), (6, 4));
        assert_eq!(out.text.matches("IFCWALL(").count(), 6);
        assert_eq!(out.text.matches("IFCBEAM(").count(), 3);
        assert_eq!(out.text.matches("IFCMEMBER(").count(), 1, "the brace is a member");
        assert!(out.text.contains(".BRACE."));
        // concrete on one beam, steel on a beam and the brace: one IfcMaterial of each
        assert_eq!(out.text.matches("IFCMATERIAL('Steel')").count(), 1);
        assert_eq!(out.text.matches("IFCMATERIAL('Concrete')").count(), 1);
        assert!(out.text.contains("IFCRELASSOCIATESMATERIAL"));
        // the brace rises 3 m along its 3 m run: a 4.24 m extrusion
        assert!(out.text.contains("4.242640"), "{}", out.text.lines().filter(|l| l.contains("IFCEXTRUDEDAREASOLID")).collect::<Vec<_>>().join("\n"));
        assert!(out.text.contains("IFCCIRCLEPROFILEDEF") && out.text.contains("IFCISHAPEPROFILEDEF"));
        assert_eq!(out.text.matches("IFCBUILDINGSTOREY(").count(), 2);
        // the sample's duct and pipe leave as MEP segments, each with its element set
        assert_eq!(out.text.matches("IFCDUCTSEGMENT(").count(), 1);
        assert_eq!(out.text.matches("IFCPIPESEGMENT(").count(), 1);
        assert!(out.text.contains("IFCCIRCLEPROFILEDEF") && out.text.contains("'Supply Air Duct'") && out.text.contains("'Drainage Pipe'"));
        assert!(!out.warnings.iter().any(|w| w.contains("ducts") || w.contains("pipes")), "{:?}", out.warnings);
        // the upper wall stands on Level 2 (3 m): the export is stable and the same twice
        assert_eq!(out.text, export_ifc(&d, "Sample", 1, "2026-10-17T00:00:00").unwrap().text);
    }

    #[test]
    fn types_and_the_element_set_are_in_the_file_and_the_file_is_repeatable() {
        let mut d = Document::spatial_sample();
        let level = BeamSpec::level;
        let a = d.place_beam(ElementId(10), &BeamSpec { material: Material::Concrete, ..level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8) }).unwrap();
        let b = d.place_beam(ElementId(10), &BeamSpec { material: Material::Concrete, ..level([1.0, 2.0], [5.0, 2.0], 0.3, 0.5, 2.8) }).unwrap();
        let free = d.place_beam(ElementId(10), &level([1.0, 3.0], [5.0, 3.0], 0.2, 0.4, 2.8)).unwrap();
        let t = d.create_type(a, "Beam 300 x 500").unwrap();
        d.assign_type(&[b], t).unwrap();
        let out = export_ifc(&d, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCBEAMTYPE(").count(), 1);
        let line = |needle: &str| out.text.lines().find(|l| l.contains(needle)).unwrap().to_string();
        assert!(line("IFCBEAMTYPE(").contains("'Beam 300 x 500'") && line("IFCBEAMTYPE(").ends_with(".BEAM.);"));
        // one relationship names both typed beams and not the free one
        assert_eq!(out.text.matches("IFCRELDEFINESBYTYPE(").count(), 1);
        assert_eq!(line("IFCRELDEFINESBYTYPE(").matches('#').count(), 4, "its own number, two beams and the type");
        // every beam carries the element set, with the type name only on the typed ones
        assert_eq!(out.text.matches("'OpenBIM_Element'").count(), 5, "three beams, the sample's duct and pipe");
        assert_eq!(out.text.matches("IFCLABEL('Beam 300 x 500')").count(), 2, "on the two typed beams");
        assert!(out.text.contains(&format!("IFCINTEGER({})", free.0)));
        assert!(out.text.contains("'OpenBIM_Type'") && out.text.contains("IFCLABEL('Concrete')"));
        assert_eq!(out.text, export_ifc(&d, "P", 1, "2026-10-18T00:00:00").unwrap().text);
        // a value changed outside the type changes the fingerprint of that element only
        let before = fingerprint(d.get(free).unwrap());
        d.edit_type(t, &openbim_document::TypeValues { width_m: 0.4, ..openbim_document::TypeValues::of(d.get(t).unwrap()) }).unwrap();
        assert_eq!(before, fingerprint(d.get(free).unwrap()));
        assert_ne!(fingerprint(d.get(a).unwrap()), fingerprint(d.get(free).unwrap()));
    }

    #[test]
    fn a_sloping_pipe_and_a_duct_are_exported_with_their_height_size_and_element_set() {
        let mut d = Document::spatial_sample();
        d.transact(d.revision(), &[openbim_document::Command::SetSlope { id: ElementId(2), slope_ratio: -0.02 }]).unwrap();
        let out = export_ifc(&d, "P", 1, "2026-10-18T00:00:00").unwrap();
        // a 6 m pipe falling 2%: the extrusion is 6.0 * sqrt(1 + 0.02^2) = 6.0012 m long
        assert!(out.text.contains("6.001199"), "{}", out.text.lines().filter(|l| l.contains("IFCEXTRUDEDAREASOLID")).collect::<Vec<_>>().join("\n"));
        assert_eq!(out.text.matches("'OpenBIM_Element'").count(), 2, "one for the duct, one for the pipe");
        assert!(out.text.contains("IFCLABEL('Pipe')") && out.text.contains("IFCLABEL('Duct')"));
        assert!(out.text.contains("IFCDISTRIBUTIONPORT("));
        assert_eq!(out.text, export_ifc(&d, "P", 1, "2026-10-18T00:00:00").unwrap().text);
    }

    #[test]
    fn the_clock_is_written_as_iso() {
        assert_eq!(iso_time(0), "1970-01-01T00:00:00");
        assert_eq!(iso_time(1_781_998_496), "2026-06-20T23:34:56");
        assert_eq!(iso_time(951_782_400), "2000-02-29T00:00:00");
    }
}
