//! Placed families as IFC: one type entity for each family type, the occurrences linked to it with
//! `IfcRelDefinesByType`, and one shared `IfcRepresentationMap` for the occurrences whose geometry is the
//! type's own. An occurrence whose instance values change the shape (a door open at another angle, a window with
//! more mullions) keeps its own extruded bodies and is still an occurrence of the type.
//!
//! Boxes and extruded or revolved forms are written as `IfcExtrudedAreaSolid` and `IfcRevolvedAreaSolid`, never
//! as meshes (blends have no such solid in IFC and are written as an `IfcFacetedBrep`; voids are cut out with
//! `IfcBooleanResult`). Swept forms are written as `IfcFixedReferenceSweptAreaSolid` along the path with the profile's up fixed. Doors and windows fill their wall opening
//! (`IfcRelFillsElement`); connectors become ports nested in the occurrence, and a network joins them.
use super::*;
use openbim_family::forms::{Form, FormGeom, Loop};
use openbim_family::mep::Category;
use openbim_family::Axis;

fn upper(s: &str) -> String {
    s.to_ascii_uppercase()
}

fn rot(f: &Form, v: V3) -> V3 {
    let (ax, ay) = f.xf.axes();
    [v[0] * ax[0] + v[1] * ay[0], v[0] * ax[1] + v[1] * ay[1], v[2]]
}

/// The shapes of an instance, in the family frame, and the kind of representation they make: swept solids; clipping
/// when voids are cut out of them by `IfcBooleanResult`; a boundary representation when a blend is among them.
fn solids(o: &mut Out, inst: &Instance, name: &str) -> (Vec<u32>, &'static str) {
    let mut out = Vec::new();
    let mut voids = Vec::new();
    let mut kind = "SweptSolid";
    for (n, (_, b)) in inst.solids.iter().enumerate() {
        let c = [(b.min[0] + b.max[0]) / 2.0, (b.min[1] + b.max[1]) / 2.0];
        let (centre, ax) = match inst.xf[n] {
            Some(x) => (x.apply(c), x.axes().0),
            None => (c, [1.0, 0.0]),
        };
        let cp = o.p2(centre[0], centre[1]);
        let dr = o.s.add(format!("IFCDIRECTION(({},{}))", real(ax[0]), real(ax[1])));
        let a2 = o.s.add(format!("IFCAXIS2PLACEMENT2D(#{cp},#{dr})"));
        let (w, h) = (o.r(b.max[0] - b.min[0], "size"), o.r(b.max[1] - b.min[1], "size"));
        let prof = o.s.add(format!("IFCRECTANGLEPROFILEDEF(.AREA.,$,#{a2},{w},{h})"));
        out.push(o.extrusion(prof, [0.0, 0.0, b.min[2]], None, None, b.max[2] - b.min[2]));
    }
    for f in &inst.forms {
        let written = match f.geom {
            FormGeom::Blend { .. } | FormGeom::SweptBlend { .. } => { kind = "Brep"; write_brep(o, f) }
            _ => write_form(o, f),
        };
        match written {
            Some(s) if f.void => voids.push(s),
            Some(s) => out.push(s),
            None => o.report.warnings.push(format!("{name}: a form was not written")),
        }
    }
    if !voids.is_empty() && !out.is_empty() {
        // every solid with every void cut out of it, one `IfcBooleanResult` after the other
        for item in out.iter_mut() {
            for v in &voids {
                *item = o.s.add(format!("IFCBOOLEANRESULT(.DIFFERENCE.,#{item},#{v})"));
            }
        }
        if kind != "Brep" { kind = "Clipping"; }
    }
    (out, kind)
}

/// A blend as a closed `IfcFacetedBrep` of triangles (IFC has no solid that changes its section).
fn write_brep(o: &mut Out, f: &Form) -> Option<u32> {
    let m = f.mesh()?;
    let pts: Vec<u32> = m.verts.iter().map(|&v| o.p3(v)).collect();
    let mut faces = Vec::with_capacity(m.tris.len());
    for t in &m.tris {
        let ring = o.s.add(format!("IFCPOLYLOOP((#{},#{},#{}))", pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]));
        let bound = o.s.add(format!("IFCFACEOUTERBOUND(#{ring},.T.)"));
        faces.push(format!("#{}", o.s.add(format!("IFCFACE((#{bound}))"))));
    }
    let shell = o.s.add(format!("IFCCLOSEDSHELL(({}))", faces.join(",")));
    Some(o.s.add(format!("IFCFACETEDBREP(#{shell})")))
}

fn curve(o: &mut Out, l: &Loop) -> u32 {
    match l {
        Loop::Poly(pts) => {
            let mut ids = Vec::new();
            for p in pts.iter().chain(pts.first()) {
                ids.push(format!("#{}", o.p2(p[0], p[1])));
            }
            o.s.add(format!("IFCPOLYLINE(({}))", ids.join(",")))
        }
        Loop::Circle { c, r } => {
            let cp = o.p2(c[0], c[1]);
            let a = o.s.add(format!("IFCAXIS2PLACEMENT2D(#{cp},$)"));
            let r = o.r(*r, "size");
            o.s.add(format!("IFCCIRCLE(#{a},{r})"))
        }
    }
}

fn form_profile(o: &mut Out, f: &Form) -> u32 {
    if let (Loop::Circle { c, r }, [Loop::Circle { c: c2, r: r2 }]) = (&f.outer, f.holes.as_slice()) {
        if c == c2 {
            let cp = o.p2(c[0], c[1]);
            let a = o.s.add(format!("IFCAXIS2PLACEMENT2D(#{cp},$)"));
            let (r1, t) = (o.r(*r, "size"), o.r(r - r2, "size"));
            return o.s.add(format!("IFCCIRCLEHOLLOWPROFILEDEF(.AREA.,$,#{a},{r1},{t})"));
        }
    }
    let outer = curve(o, &f.outer);
    if f.holes.is_empty() {
        return o.s.add(format!("IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{outer})"));
    }
    let inner: Vec<String> = f.holes.iter().map(|h| format!("#{}", curve(o, h))).collect();
    o.s.add(format!("IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,$,#{outer},({}))", inner.join(",")))
}

fn write_form(o: &mut Out, f: &Form) -> Option<u32> {
    let (plane, offset) = match &f.geom {
        FormGeom::Extrusion { plane, from, to } => (*plane, from.min(*to)),
        FormGeom::Revolve { plane, offset, .. } => (*plane, *offset),
        FormGeom::Blend { .. } | FormGeom::SweptBlend { .. } => return None,
        FormGeom::Sweep { z, path } => {
            // `IfcFixedReferenceSweptAreaSolid` takes the profile's x along the fixed reference ("up" here) and its y
            // along (direction of travel x up), which is to the right; our profile has u to the left and v up, so
            // the profile is written turned a quarter: x = v, y = -u. The corners are mitred.
            let turn = |l: &Loop| match l {
                Loop::Poly(p) => Loop::Poly(p.iter().map(|q| [q[1], -q[0]]).collect()),
                Loop::Circle { c, r } => Loop::Circle { c: [c[1], -c[0]], r: *r },
            };
            let t = Form { outer: turn(&f.outer), holes: f.holes.iter().map(turn).collect(), ..f.clone() };
            let prof = form_profile(o, &t);
            let pts: Vec<String> = path
                .iter()
                .map(|q| {
                    let xy = f.xf.apply(*q);
                    format!("#{}", o.p3([xy[0], xy[1], z + f.dz]))
                })
                .collect();
            let dx = o.s.add(format!("IFCPOLYLINE(({}))", pts.join(",")));
            let up = o.dir([0.0, 0.0, 1.0]);
            return Some(o.s.add(format!("IFCFIXEDREFERENCESWEPTAREASOLID(#{prof},$,#{dx},$,$,#{up})")));
        }
    };
    let (axis, refd, org): (V3, V3, V3) = match plane {
        Axis::Z => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, offset]),
        Axis::X => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [offset, 0.0, 0.0]),
        Axis::Y => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, offset, 0.0]),
    };
    // the form's own turn and shift move it into the family frame
    let xy = f.xf.apply([org[0], org[1]]);
    let loc = [xy[0], xy[1], org[2] + f.dz];
    let prof = form_profile(o, f);
    let pos = o.axis(loc, Some(rot(f, axis)), Some(rot(f, refd)));
    Some(match &f.geom {
        FormGeom::Extrusion { from, to, .. } => {
            let up = o.dir([0.0, 0.0, 1.0]);
            let d = o.r((to - from).abs(), "depth");
            o.s.add(format!("IFCEXTRUDEDAREASOLID(#{prof},#{pos},#{up},{d})"))
        }
        FormGeom::Revolve { about_v, at, angle, .. } => {
            let (l, d) = if *about_v { ([*at, 0.0, 0.0], [0.0, 1.0, 0.0]) } else { ([0.0, *at, 0.0], [1.0, 0.0, 0.0]) };
            let lp = o.p3(l);
            let dd = o.dir(d);
            let ax = o.s.add(format!("IFCAXIS1PLACEMENT(#{lp},#{dd})"));
            let a = o.r(angle.to_radians(), "angle");
            o.s.add(format!("IFCREVOLVEDAREASOLID(#{prof},#{pos},#{ax},{a})"))
        }
        FormGeom::Sweep { .. } | FormGeom::Blend { .. } | FormGeom::SweptBlend { .. } => return None,
    })
}

fn same_geometry(a: &Instance, b: &Instance) -> bool {
    a.solids == b.solids && a.xf == b.xf && a.forms == b.forms && a.opening == b.opening && a.opening_rise == b.opening_rise
}

fn type_tail(cat: Category) -> &'static str {
    match cat {
        Category::Door => "$,$,.DOOR.,.NOTDEFINED.,.F.,$",
        Category::Window => "$,$,.WINDOW.,.NOTDEFINED.,.F.,$",
        Category::Furniture => "$,$,.NOTDEFINED.,.NOTDEFINED.",
        _ => "$,$,.NOTDEFINED.",
    }
}

struct Group {
    family: usize,
    type_name: Option<String>,
    reference: usize,
    map: Option<u32>,
    type_entity: u32,
    occurrences: Vec<u32>,
}

fn props_of<'a>(fam: &'a Compiled, inst: &'a Instance, scope: openbim_family::Scope) -> Vec<(&'a str, Val)> {
    let mut v: Vec<(&str, Val)> = Vec::new();
    for (n, x) in fam.input_values(inst) {
        if fam.input_scope(n) == Some(scope) {
            // what the value measures decides how it is written: a length as a length, an angle as an angle in radians
            v.push((n, match fam.value_dim(n) {
                Some(d) if d == openbim_family::LENGTH => Val::LenAny(x),
                Some(d) if d == openbim_family::AREA => Val::Area(x),
                Some(d) if d == openbim_family::VOLUME => Val::Vol(x),
                Some(d) if d == openbim_family::ANGLE => Val::Angle(x),
                _ => Val::Real(x),
            }));
        }
    }
    for (n, t) in fam.input_texts(inst) {
        if fam.input_scope(n) == Some(scope) && !t.is_empty() {
            v.push((n, Val::Label(t.to_string())));
        }
    }
    v
}

pub(crate) fn write_families(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    let mut groups: Vec<Group> = Vec::new();
    for (k, it) in scene.items.iter().enumerate() {
        let fr = scene.families.get(it.family).ok_or_else(|| ExportError::Unsupported(format!("{}: no such family", it.name)))?;
        let fam = &fr.family;
        let gi = match groups.iter().position(|g| g.family == it.family && g.type_name == it.type_name) {
            Some(i) => i,
            None => {
                // the type: its shared shape comes from the first occurrence
                let slot = it.type_name.as_deref().and_then(|t| fam.type_names().iter().position(|n| *n == t)).map_or(0, |i| i + 1);
                let tname = it.type_name.clone().unwrap_or_else(|| fam.name().to_string());
                let (body, kind) = solids(o, &it.instance, &it.name);
                let map = if body.is_empty() {
                    None
                } else {
                    let rep = o.shape_rep(kind, &body);
                    let org = o.axis([0.0; 3], None, None);
                    Some(o.s.add(format!("IFCREPRESENTATIONMAP(#{org},#{rep})")))
                };
                let mut psets = Vec::new();
                let mut props = vec![("Family", Val::Label(fam.name().to_string())), ("Category", Val::Label(fam.category().ifc_class().to_string()))];
                if !fam.code().is_empty() {
                    props.push(("Code", Val::Label(fam.code().to_string())));
                }
                props.extend(props_of(fam, &it.instance, openbim_family::Scope::Type));
                for (sys, code) in fam.classification() {
                    props.push(("Classification", Val::Label(format!("{sys}: {code}"))));
                }
                let mut report = std::mem::take(&mut o.report);
                let split = crate::split_props(&scene.pset_map, fam.category().ifc_class(), props, &tname, &mut report);
                o.report = report;
                if !split.custom.is_empty() {
                    psets.push(format!("#{}", o.pset_entity("OpenBIM_Family", split.custom)));
                }
                for (name, list) in split.sets {
                    psets.push(format!("#{}", o.pset_entity(name, list)));
                }
                if let Some(l) = fam.lighting_for(it.type_name.as_deref()) {
                    // what a lighting program needs; standard sets where IFC has the field, the rest in our own
                    let mut common = vec![("LightFixtureMountingType", Val::Label(l.mounting.ifc().to_string()))];
                    if let Some(w) = l.power_w {
                        common.push(("TotalWattage", Val::Real(w)));
                    }
                    psets.push(format!("#{}", o.pset_entity("Pset_LightFixtureTypeCommon", common)));
                    let mut maker = Vec::new();
                    if let Some(m) = &l.manufacturer {
                        maker.push(("Manufacturer", Val::Label(m.clone())));
                    }
                    if let Some(a) = &l.article {
                        maker.push(("ArticleNumber", Val::Label(a.clone())));
                    }
                    if !maker.is_empty() {
                        psets.push(format!("#{}", o.pset_entity("Pset_ManufacturerTypeInformation", maker)));
                    }
                    let mut own = Vec::new();
                    if let Some(f) = &l.photometry {
                        own.push(("PhotometryFile", Val::Label(f.clone())));
                    }
                    for (n, f) in [("PictureFile", &l.picture), ("DatasheetFile", &l.datasheet)] {
                        if let Some(f) = f {
                            own.push((n, Val::Label(f.clone())));
                        }
                    }
                    for (n, v) in [("LuminousFlux", l.flux_lm), ("ColourTemperature", l.cct_k), ("ColourRenderingIndex", l.cri), ("DimmingLevel", l.dimming)] {
                        if let Some(v) = v {
                            own.push((n, Val::Real(v)));
                        }
                    }
                    if let Some(ip) = &l.ip {
                        own.push(("IngressProtection", Val::Label(ip.clone())));
                    }
                    psets.push(format!("#{}", o.pset_entity("OpenBIM_Lighting", own)));
                }
                let tid = (0xFFFF_FFFEu64 << 32) | (u64::from(fr.id) << 16) | slot as u64;
                let g = o.gid(tid, 0);
                let cls = upper(&format!("{}Type", fam.category().ifc_class()));
                let maps = map.map_or("$".to_string(), |m| format!("(#{m})"));
                let te = o.s.add(format!("{cls}({g},$,{},$,$,({}),{maps},{})", string(&tname), psets.join(","), type_tail(fam.category())));
                o.report.types += 1;
                for (sys, code) in fam.classification() {
                    o.classify(te, sys, code);
                }
                groups.push(Group { family: it.family, type_name: it.type_name.clone(), reference: k, map, type_entity: te, occurrences: Vec::new() });
                groups.len() - 1
            }
        };
        // the occurrence
        let st = o.storey_for(it.frame.origin[2]);
        let elev = o.storeys[st].2;
        let xdir = [it.frame.x[0], it.frame.x[1], 0.0];
        let pl = o.place(Some(o.storeys[st].1), [it.frame.origin[0], it.frame.origin[1], it.frame.origin[2] - elev], Some([0.0, 0.0, 1.0]), Some(xdir));
        let ref_inst = &scene.items[groups[gi].reference].instance;
        let shape = match groups[gi].map {
            Some(m) if same_geometry(&it.instance, ref_inst) => {
                let org = o.p3([0.0; 3]);
                let op = o.s.add(format!("IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#{org},1.,$)"));
                let mi = o.s.add(format!("IFCMAPPEDITEM(#{m},#{op})"));
                let rep = o.shape_rep("MappedRepresentation", &[mi]);
                o.report.mapped += 1;
                Some(o.s.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))")))
            }
            _ => {
                let (body, kind) = solids(o, &it.instance, &it.name);
                if body.is_empty() {
                    None
                } else {
                    let rep = o.shape_rep(kind, &body);
                    Some(o.s.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))")))
                }
            }
        };
        let rep = shape.map_or("$".to_string(), |s| format!("#{s}"));
        let cat = fam.category();
        let (h, w) = it.instance.opening.map_or(("$".to_string(), "$".to_string()), |[w, h, _]| (real(h), real(w)));
        let tail = match cat {
            Category::Door => {
                let op = match fam.text(&it.instance, "swing") {
                    Some("left") => ".SINGLE_SWING_LEFT.",
                    Some("right") => ".SINGLE_SWING_RIGHT.",
                    _ => ".NOTDEFINED.",
                };
                format!("$,{h},{w},.DOOR.,{op},$")
            }
            Category::Window => format!("$,{h},{w},.WINDOW.,.NOTDEFINED.,$"),
            _ => "$,.NOTDEFINED.".to_string(),
        };
        let g = o.gid(it.id, 0);
        let occ = o.s.add(format!("{}({g},$,{},$,$,#{pl},{rep},{tail})", upper(cat.ifc_class()), string(&it.name)));
        o.contain(st, occ);
        o.item_entities.push(occ);
        o.report.items += 1;
        let props = props_of(fam, &it.instance, openbim_family::Scope::Instance);
        if !props.is_empty() {
            o.pset_mapped(occ, cat.ifc_class(), "OpenBIM_Instance", props, &it.name);
        }
        if let Some([ow, oh, _]) = it.instance.opening {
            match cat {
                Category::Door => o.qto(occ, "Qto_DoorBaseQuantities", &crate::quantities::opening_item(ow, oh)),
                Category::Window => o.qto(occ, "Qto_WindowBaseQuantities", &crate::quantities::opening_item(ow, oh)),
                _ => {}
            }
        }
        groups[gi].occurrences.push(occ);
        // fills its wall opening
        if let (Some((wi, seg, centre)), Some([ow, oh, sill])) = (it.host, it.instance.opening) {
            let want = [centre - ow / 2.0, ow, sill, oh];
            let found = o.opening_ids.iter().find(|(w, s, b, _)| *w == wi && *s == seg && (0..4).all(|i| (b[i] - want[i]).abs() < 1e-9)).map(|x| x.3);
            match found {
                Some(op) => {
                    let g = o.gen_gid();
                    o.s.add(format!("IFCRELFILLSELEMENT({g},$,$,$,#{op},#{occ})"));
                }
                None => o.report.warnings.push(format!("{}: no wall opening of its size was found to fill", it.name)),
            }
        }
        // connectors: ports nested in the occurrence
        let mut ports = Vec::new();
        for (c, conn) in it.instance.connectors.iter().enumerate() {
            let p = write_port(o, (it.id, 0x2000 + c as u16), &conn.name, pl, conn.pos, unit(conn.dir), (conn.domain, conn.system, conn.flow_dir));
            o.item_ports.insert((it.id, conn.name.clone()), p);
            ports.push(p);
        }
        nest(o, occ, &ports);
    }
    for g in &groups {
        let gid = o.gen_gid();
        let occs: Vec<String> = g.occurrences.iter().map(|i| format!("#{i}")).collect();
        o.s.add(format!("IFCRELDEFINESBYTYPE({gid},$,$,$,({}),#{})", occs.join(","), g.type_entity));
    }
    Ok(())
}

/// IFC2X3: the doors and windows only, as plain boxes in their wall openings. Luminaires and other families have no
/// place in a 2x3 shell file (a 2x3 light fixture is a flow terminal, which DIALux ignores); they are counted in the warnings.
pub(crate) fn write_openings_2x3(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    let mut skipped = 0usize;
    for it in &scene.items {
        let Some(fam) = scene.families.get(it.family).map(|f| &f.family) else {
            return Err(ExportError::Unsupported(format!("{}: no family {}", it.name, it.family)));
        };
        let cat = fam.category();
        let (Category::Door | Category::Window, Some([ow, oh_, sill])) = (cat, it.instance.opening) else {
            skipped += 1;
            continue;
        };
        let st = o.storey_for(it.frame.origin[2]);
        let elev = o.storeys[st].2;
        let thick = it.host.and_then(|(wi, _, _)| scene.walls.get(wi)).map_or(0.1, |w| w.chain.thickness.min(0.1));
        let [x, y] = it.frame.x;
        let origin = [it.frame.origin[0], it.frame.origin[1], it.frame.origin[2] - elev + sill];
        let pl = o.place(Some(o.storeys[st].1), origin, None, Some([x, y, 0.0]));
        let prof = o.rect_profile(0.0, 0.0, ow, thick);
        let solid = o.extrusion(prof, [0.0; 3], None, None, oh_);
        let shape = o.shape(&[solid]);
        let g = o.gid(it.id, 0);
        let (hh, ww) = (o.r(oh_, "height"), o.r(ow, "width"));
        let e = o.s.add(format!("{}({g},$,{},$,$,#{pl},#{shape},$,{hh},{ww})", upper(cat.ifc_class()), string(&it.name)));
        o.contain(st, e);
        o.item_entities.push(e);
        o.report.items += 1;
        match cat {
            Category::Door => o.qto(e, "Qto_DoorBaseQuantities", &crate::quantities::opening_item(ow, oh_)),
            _ => o.qto(e, "Qto_WindowBaseQuantities", &crate::quantities::opening_item(ow, oh_)),
        }
        if let Some((wi, seg, centre)) = it.host {
            let want = [centre - ow / 2.0, ow, sill, oh_];
            match o.opening_ids.iter().find(|(w, s, b, _)| *w == wi && *s == seg && (0..4).all(|i| (b[i] - want[i]).abs() < 1e-9)).map(|x| x.3) {
                Some(op) => {
                    let g = o.gen_gid();
                    o.s.add(format!("IFCRELFILLSELEMENT({g},$,$,$,#{op},#{e})"));
                }
                None => o.report.warnings.push(format!("{}: no wall opening of its size was found to fill", it.name)),
            }
        }
    }
    if skipped > 0 {
        o.report.warnings.push(format!("IFC2X3: {skipped} placed families (luminaires, furniture, equipment) are not written; use the luminaire schedule"));
    }
    Ok(())
}
