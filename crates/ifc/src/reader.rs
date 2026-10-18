//! Reads the building out of an IFC file (IFC2X3, IFC4 or IFC4.3): the storeys, and the walls, beams, columns, slabs,
//! footings, ducts and pipes that are an extruded profile, with their property sets, materials and types.
//! Everything else is counted by class in `skipped`, so that an import can say what it left out.
use crate::step_read::{parse, Entity, ReadError, StepFile, Value};
use std::collections::BTreeMap;

pub use crate::step_read::ReadError as Error;

#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Text(String),
    Num(f64),
    Bool(bool),
}

impl Prop {
    pub fn text(&self) -> Option<&str> { if let Prop::Text(s) = self { Some(s) } else { None } }
    pub fn num(&self) -> Option<f64> { if let Prop::Num(x) = self { Some(*x) } else { None } }
}

/// Property sets by name, each a list of properties by name.
pub type Psets = BTreeMap<String, BTreeMap<String, Prop>>;

#[derive(Clone, Debug, PartialEq)]
pub struct Storey {
    pub name: String,
    pub elevation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Section {
    Rect { w: f64, h: f64 },
    Circle { d: f64 },
    I { w: f64, d: f64, web: f64, flange: f64 },
    Polygon,
}

/// An extruded profile in model coordinates (metres).
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub section: Section,
    /// the outline of the profile where it starts (counter-clockwise or clockwise as written)
    pub outline: Vec<[f64; 3]>,
    /// the middle of the profile's start
    pub centre: [f64; 3],
    /// the direction of the extrusion, of length 1
    pub dir: [f64; 3],
    pub depth: f64,
}

impl Body {
    pub fn end(&self) -> [f64; 3] { std::array::from_fn(|i| self.centre[i] + self.dir[i] * self.depth) }
    pub fn vertical(&self) -> bool { self.dir[2].abs() > 0.9999 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub step_id: u32,
    pub guid: String,
    /// upper case, e.g. `IFCBEAM`
    pub class: String,
    /// the predefined type without dots, e.g. `BEAM`, `STRIP_FOOTING`; empty for none
    pub predefined: String,
    pub name: String,
    pub body: Option<Body>,
    /// index into `Imported::storeys`
    pub storey: Option<usize>,
    pub psets: Psets,
    pub material: Option<String>,
    /// index into `Imported::types`
    pub type_obj: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeObj {
    pub step_id: u32,
    pub guid: String,
    pub class: String,
    pub predefined: String,
    pub name: String,
    pub psets: Psets,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    /// e.g. `IFC4X3_ADD2`
    pub schema: String,
    pub storeys: Vec<Storey>,
    pub members: Vec<Member>,
    pub types: Vec<TypeObj>,
    /// the app's own records of the whole model, when the file carries them (`OpenBIM_Document` on the project)
    pub native: Vec<String>,
    /// what was read but is not a member: class name and how many
    pub skipped: BTreeMap<String, usize>,
}

/// The classes that become members.
const MEMBERS: [&str; 10] = ["IFCBEAM", "IFCMEMBER", "IFCCOLUMN", "IFCSLAB", "IFCFOOTING", "IFCWALL", "IFCWALLSTANDARDCASE", "IFCPIPESEGMENT", "IFCDUCTSEGMENT", "IFCCABLECARRIERSEGMENT"];

/// Entities that are not products of the building, or are read in another way.
const NOT_PRODUCTS: [&str; 6] = ["IFCBUILDINGSTOREY", "IFCBUILDING", "IFCSITE", "IFCOPENINGELEMENT", "IFCVIRTUALELEMENT", "IFCPROJECT"];

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dot(a: V3, b: V3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn len(a: V3) -> f64 { dot(a, a).sqrt() }
fn unit(a: V3) -> Option<V3> { let n = len(a); (n > 1e-12 && n.is_finite()).then(|| [a[0] / n, a[1] / n, a[2] / n]) }

/// A placement: an origin and three axes.
#[derive(Clone, Copy, Debug)]
struct Mat { o: V3, x: V3, y: V3, z: V3 }

impl Mat {
    const ID: Mat = Mat { o: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], z: [0.0, 0.0, 1.0] };
    fn rot(&self, v: V3) -> V3 { std::array::from_fn(|i| self.x[i] * v[0] + self.y[i] * v[1] + self.z[i] * v[2]) }
    fn apply(&self, p: V3) -> V3 { let r = self.rot(p); std::array::from_fn(|i| self.o[i] + r[i]) }
    /// `self` is the parent: where a placement given in the parent's frame is in model coordinates
    fn then(&self, child: &Mat) -> Mat { Mat { o: self.apply(child.o), x: self.rot(child.x), y: self.rot(child.y), z: self.rot(child.z) } }
}

struct R<'a> {
    f: &'a StepFile,
}

impl R<'_> {
    fn point(&self, v: &Value) -> Option<Vec<f64>> {
        let e = self.f.deref(v)?;
        (e.class == "IFCCARTESIANPOINT").then(|| e.args.first().map(|c| c.as_list().iter().filter_map(Value::as_f64).collect()))?
    }
    fn direction(&self, v: &Value) -> Option<V3> {
        let e = self.f.deref(v)?;
        let c: Vec<f64> = e.args.first()?.as_list().iter().filter_map(Value::as_f64).collect();
        (c.len() >= 2).then(|| [c[0], c[1], c.get(2).copied().unwrap_or(0.0)])
    }
    /// IfcAxis2Placement3D (or 2D, with z up).
    fn axis(&self, v: &Value) -> Option<Mat> {
        let e = self.f.deref(v)?;
        let loc = self.point(e.args.first()?)?;
        let o = [loc[0], loc[1], loc.get(2).copied().unwrap_or(0.0)];
        match e.class.as_str() {
            "IFCAXIS2PLACEMENT3D" => {
                let z = e.args.get(1).and_then(|a| self.direction(a)).and_then(unit).unwrap_or([0.0, 0.0, 1.0]);
                let xr = e.args.get(2).and_then(|a| self.direction(a)).unwrap_or(if z[0].abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] });
                let xp = sub(xr, [z[0] * dot(xr, z), z[1] * dot(xr, z), z[2] * dot(xr, z)]);
                let x = unit(xp).or_else(|| unit(cross([0.0, 1.0, 0.0], z)))?;
                Some(Mat { o, x, y: cross(z, x), z })
            }
            "IFCAXIS2PLACEMENT2D" => {
                let x = e.args.get(1).and_then(|a| self.direction(a)).and_then(unit).unwrap_or([1.0, 0.0, 0.0]);
                Some(Mat { o, x, y: [-x[1], x[0], 0.0], z: [0.0, 0.0, 1.0] })
            }
            _ => None,
        }
    }
    /// An IfcLocalPlacement in model coordinates.
    fn placement(&self, v: &Value, depth: usize) -> Option<Mat> {
        if depth > 64 { return None; }
        let e = self.f.deref(v)?;
        if e.class != "IFCLOCALPLACEMENT" { return None; }
        let rel = self.axis(e.args.get(1)?)?;
        match e.args.first() {
            Some(Value::Ref(_)) => Some(self.placement(&e.args[0], depth + 1)?.then(&rel)),
            _ => Some(rel),
        }
    }

    /// The profile as (section, outline in the profile's plane with its position applied, centre).
    fn profile(&self, v: &Value) -> Option<(Section, Vec<[f64; 2]>)> {
        let e = self.f.deref(v)?;
        let pos = e.args.get(2).and_then(|a| self.axis(a)).unwrap_or(Mat::ID);
        let place = |pts: Vec<[f64; 2]>| -> Vec<[f64; 2]> { pts.into_iter().map(|p| { let q = pos.apply([p[0], p[1], 0.0]); [q[0], q[1]] }).collect() };
        let num = |i: usize| e.args.get(i).and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0);
        match e.class.as_str() {
            "IFCRECTANGLEPROFILEDEF" => {
                let (w, h) = (num(3)?, num(4)?);
                Some((Section::Rect { w, h }, place(vec![[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [w / 2.0, h / 2.0], [-w / 2.0, h / 2.0]])))
            }
            "IFCCIRCLEPROFILEDEF" => {
                let r = num(3)?;
                Some((Section::Circle { d: 2.0 * r }, place((0..16).map(|i| { let a = i as f64 * std::f64::consts::TAU / 16.0; [r * a.cos(), r * a.sin()] }).collect())))
            }
            "IFCISHAPEPROFILEDEF" => {
                let (w, d, web, flange) = (num(3)?, num(4)?, num(5)?, num(6)?);
                Some((Section::I { w, d, web, flange }, place(vec![[-w / 2.0, -d / 2.0], [w / 2.0, -d / 2.0], [w / 2.0, d / 2.0], [-w / 2.0, d / 2.0]])))
            }
            "IFCARBITRARYCLOSEDPROFILEDEF" => {
                let curve = self.f.deref(e.args.get(2)?)?;
                if curve.class != "IFCPOLYLINE" { return None; }
                let mut pts: Vec<[f64; 2]> = curve.args.first()?.as_list().iter().filter_map(|p| self.point(p)).filter(|c| c.len() >= 2).map(|c| [c[0], c[1]]).collect();
                if pts.len() > 1 && pts.first() == pts.last() { pts.pop(); }
                (pts.len() >= 3).then_some((Section::Polygon, pts))
            }
            _ => None,
        }
    }

    /// The first extruded solid of a product's body.
    fn body(&self, product: &Entity) -> Option<Body> {
        let place = self.placement(product.args.get(5)?, 0)?;
        let shape = self.f.deref(product.args.get(6)?)?;
        if shape.class != "IFCPRODUCTDEFINITIONSHAPE" { return None; }
        for rep in shape.args.get(2)?.as_list() {
            let Some(rep) = self.f.deref(rep) else { continue };
            if rep.class != "IFCSHAPEREPRESENTATION" { continue; }
            if rep.args.get(1).and_then(Value::as_str).is_some_and(|id| id != "Body") { continue; }
            for item in rep.args.get(3)?.as_list() {
                if let Some(b) = self.solid(item, &place, 0) { return Some(b); }
            }
        }
        None
    }

    fn solid(&self, item: &Value, place: &Mat, depth: usize) -> Option<Body> {
        if depth > 8 { return None; }
        let e = self.f.deref(item)?;
        match e.class.as_str() {
            "IFCEXTRUDEDAREASOLID" => {
                let (section, outline) = self.profile(e.args.first()?)?;
                let m = place.then(&e.args.get(1).and_then(|a| self.axis(a)).unwrap_or(Mat::ID));
                let depth_m = e.args.get(3).and_then(Value::as_f64).filter(|d| d.is_finite() && *d > 0.0)?;
                let local = e.args.get(2).and_then(|a| self.direction(a)).and_then(unit)?;
                let dir = unit(m.rot(local))?;
                let pts: Vec<V3> = outline.iter().map(|p| m.apply([p[0], p[1], 0.0])).collect();
                let n = pts.len() as f64;
                let c = pts.iter().fold([0.0; 3], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]);
                // a symmetric profile is centred on its origin; a polygon's middle is the middle of its points
                Some(Body { section, outline: pts, centre: c, dir, depth: depth_m })
            }
            // a solid cut or clipped by another: the first operand is the member
            "IFCBOOLEANCLIPPINGRESULT" | "IFCBOOLEANRESULT" => self.solid(e.args.get(1)?, place, depth + 1),
            _ => None,
        }
    }

    fn prop_value(v: &Value) -> Option<Prop> {
        match v {
            Value::Typed(t, args) => match (t.as_str(), args.first()?) {
                ("IFCBOOLEAN" | "IFCLOGICAL", Value::Enum(b)) => Some(Prop::Bool(b == "T")),
                (_, Value::Str(s)) => Some(Prop::Text(s.clone())),
                (_, x) => x.as_f64().map(Prop::Num),
            },
            _ => None,
        }
    }

    fn pset(&self, id: u32) -> Option<(String, BTreeMap<String, Prop>)> {
        let e = self.f.get(id)?;
        if e.class != "IFCPROPERTYSET" { return None; }
        let name = e.args.get(2)?.as_str()?.to_string();
        let mut props = BTreeMap::new();
        for p in e.args.get(4)?.as_list() {
            let Some(pe) = self.f.deref(p) else { continue };
            if pe.class != "IFCPROPERTYSINGLEVALUE" { continue; }
            if let (Some(n), Some(v)) = (pe.args.first().and_then(Value::as_str), pe.args.get(2).and_then(Self::prop_value)) { props.insert(n.to_string(), v); }
        }
        Some((name, props))
    }
}

fn predefined(e: &Entity) -> String {
    e.args.last().and_then(Value::as_enum).filter(|p| *p != "NOTDEFINED").unwrap_or("").to_string()
}

/// Reads an IFC file.
pub fn read(text: &str) -> Result<Imported, ReadError> {
    let f = parse(text)?;
    let r = R { f: &f };
    let mut out = Imported { schema: f.schema.clone(), ..Default::default() };
    if f.complex > 0 { out.skipped.insert("complex entities".into(), f.complex); }

    // storeys, lowest first
    let mut storeys: Vec<(u32, Storey, f64)> = Vec::new();
    for (&id, e) in &f.entities {
        if e.class != "IFCBUILDINGSTOREY" { continue; }
        let z = e.args.get(5).and_then(|p| r.placement(p, 0)).map_or(0.0, |m| m.o[2]);
        let elevation = e.args.get(9).and_then(Value::as_f64).unwrap_or(z);
        storeys.push((id, Storey { name: e.args.get(2).and_then(Value::as_str).unwrap_or("Level").to_string(), elevation }, z));
    }
    storeys.sort_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation));
    let storey_of: BTreeMap<u32, usize> = storeys.iter().enumerate().map(|(i, s)| (s.0, i)).collect();
    out.storeys = storeys.into_iter().map(|s| s.1).collect();

    // relationships, by the object they relate
    let mut contained: BTreeMap<u32, usize> = BTreeMap::new();
    let mut psets_of: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut type_of: BTreeMap<u32, u32> = BTreeMap::new();
    let mut material_of: BTreeMap<u32, String> = BTreeMap::new();
    for e in f.entities.values() {
        let objects = |i: usize| -> Vec<u32> { e.args.get(i).map_or(Vec::new(), |l| l.as_list().iter().filter_map(Value::as_ref).collect()) };
        match e.class.as_str() {
            "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                if let Some(&s) = e.args.get(5).and_then(Value::as_ref).and_then(|s| storey_of.get(&s)) {
                    for o in objects(4) { contained.insert(o, s); }
                }
            }
            "IFCRELDEFINESBYPROPERTIES" => {
                if let Some(p) = e.args.get(5).and_then(Value::as_ref) { for o in objects(4) { psets_of.entry(o).or_default().push(p); } }
            }
            "IFCRELDEFINESBYTYPE" => {
                if let Some(t) = e.args.get(5).and_then(Value::as_ref) { for o in objects(4) { type_of.insert(o, t); } }
            }
            "IFCRELASSOCIATESMATERIAL" => {
                if let Some(m) = e.args.get(5).and_then(|m| f.deref(m)).filter(|m| m.class == "IFCMATERIAL") {
                    if let Some(name) = m.args.first().and_then(Value::as_str) { for o in objects(4) { material_of.insert(o, name.to_string()); } }
                }
            }
            _ => {}
        }
    }

    // the app's own records, on the project
    if let Some((&project, _)) = f.entities.iter().find(|(_, e)| e.class == "IFCPROJECT") {
        for p in psets_of.get(&project).into_iter().flatten() {
            if let Some((name, props)) = r.pset(*p) {
                if name == "OpenBIM_Document" { out.native = props.into_values().filter_map(|v| v.text().map(str::to_string)).collect(); }
            }
        }
    }

    // types first, so that members can point at them
    let mut type_index: BTreeMap<u32, usize> = BTreeMap::new();
    for (&id, e) in &f.entities {
        if !e.class.ends_with("TYPE") || !e.class.starts_with("IFC") || e.args.len() < 6 { continue; }
        let Some(guid) = e.args.first().and_then(Value::as_str) else { continue };
        let mut psets = Psets::new();
        for p in e.args.get(5).map_or(&[][..], Value::as_list) {
            if let Some((n, props)) = p.as_ref().and_then(|p| r.pset(p)) { psets.insert(n, props); }
        }
        type_index.insert(id, out.types.len());
        out.types.push(TypeObj { step_id: id, guid: guid.to_string(), class: e.class.clone(), predefined: predefined(e), name: e.args.get(2).and_then(Value::as_str).unwrap_or("").to_string(), psets });
    }

    for (&id, e) in &f.entities {
        if MEMBERS.contains(&e.class.as_str()) {
            let mut psets = Psets::new();
            for p in psets_of.get(&id).into_iter().flatten() { if let Some((n, props)) = r.pset(*p) { psets.insert(n, props); } }
            out.members.push(Member {
                step_id: id, guid: e.args.first().and_then(Value::as_str).unwrap_or("").to_string(), class: e.class.clone(), predefined: predefined(e),
                name: e.args.get(2).and_then(Value::as_str).unwrap_or("").to_string(), body: r.body(e),
                storey: contained.get(&id).copied(), psets, material: material_of.get(&id).cloned(),
                type_obj: type_of.get(&id).and_then(|t| type_index.get(t)).copied(),
            });
        } else if e.class == "IFCREINFORCINGBAR" {
            *out.skipped.entry("IFCREINFORCINGBAR".into()).or_default() += 1;
        } else if !NOT_PRODUCTS.contains(&e.class.as_str()) && e.args.len() >= 7 && matches!(e.args.get(5), Some(Value::Ref(p)) if f.get(*p).is_some_and(|p| p.class == "IFCLOCALPLACEMENT")) {
            *out.skipped.entry(e.class.clone()).or_default() += 1;
        }
    }
    Ok(out)
}
