//! IFC4 writer. Takes the model as the app holds it (wall chains, floor slabs, openings, MEP networks with
//! their connectivity) and writes a STEP file.
//!
//! * Every element's GlobalId comes from its element number (see `guid`), so re-exporting an unchanged or
//!   edited model keeps the ids.
//! * A drawn wall chain is one `IfcWall`, with one extruded body per mitred segment; every opening in it is
//!   an `IfcOpeningElement` linked with `IfcRelVoidsElement`. Openings that are service penetrations carry
//!   their register id, systems and fire rating.
//! * Floor slabs are `IfcSlab`; riser openings and shaft voids cut them the same way.
//! * Runs are `IfcPipeSegment`, `IfcDuctSegment`, `IfcCableCarrierSegment` or `IfcCableSegment`, fittings the
//!   matching fitting class. Every end of a run and every fitting leg is an `IfcDistributionPort` nested in
//!   its element, and every link in the connectivity graph is an `IfcRelConnectsPorts`.
//! * Fitting bodies are straight stubs along each leg (no swept bend yet). Equipment is written as a bare
//!   `IfcFlowTerminal` that owns the connector ports, until families are exported as types.
//!
//! The output is the same bytes for the same input: nothing in it depends on the clock or on hash order.
mod families;
pub mod guid;
pub mod pset_map;
pub mod quantities;
pub mod step;

use openbim_arch::{Levels, Room};
use openbim_family::mep::{up_for, Domain, System};
use openbim_family::{Compiled, Instance};
use openbim_geom::WallChain;
use openbim_place::floors::{FloorOpeningKind, FloorPenetration, ShaftVoid, Slab};
use openbim_place::penetrations::{Penetration, RunSize};
use openbim_route::graph::{Network, NodeRef, PortRef};
use openbim_route::{Fitting, FittingKind};
use std::collections::BTreeMap;
use pset_map::{PropKind, PsetMap};
use step::{real, string, Step};

pub struct WallItem {
    /// element number of the wall in the model
    pub id: u64,
    pub name: String,
    /// the built chain, every opening included
    pub chain: WallChain,
}

pub struct SlabItem {
    pub id: u64,
    pub slab: Slab,
}

/// A compiled family with the number it keeps for life (it is part of the GlobalId of each of its types).
pub struct FamilyRef {
    pub id: u32,
    pub family: Compiled,
}

/// Where the family frame sits in the model: origin, and the plan direction of its x axis (y is to the left of x).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub x: [f64; 2],
}

impl Frame {
    /// The frame of a family placed on wall segment `segment` of `chain`, `centre` along it.
    pub fn on_wall(chain: &WallChain, segment: usize, centre: f64) -> Frame {
        let n = chain.points.len();
        let (a, b) = (chain.points[segment], chain.points[(segment + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        Frame { origin: [a[0] + u[0] * centre, a[1] + u[1] * centre, chain.base_z], x: u }
    }
    pub fn point(&self, p: [f64; 3]) -> [f64; 3] {
        let (u, v) = (self.x, [-self.x[1], self.x[0]]);
        [self.origin[0] + u[0] * p[0] + v[0] * p[1], self.origin[1] + u[1] * p[0] + v[1] * p[1], self.origin[2] + p[2]]
    }
    pub fn vector(&self, p: [f64; 3]) -> [f64; 3] {
        let (u, v) = (self.x, [-self.x[1], self.x[0]]);
        [u[0] * p[0] + v[0] * p[1], u[1] * p[0] + v[1] * p[1], p[2]]
    }
    /// A connector given in the family frame, moved into the model.
    pub fn connector(&self, c: &openbim_family::mep::Connector) -> openbim_family::mep::Connector {
        let mut w = c.clone();
        w.pos = self.point(c.pos);
        w.dir = self.vector(c.dir);
        w.up = self.vector(c.up);
        w
    }
}

/// One placed family: its element number, which family and type, the solved instance, where it is.
pub struct FamilyItem {
    pub id: u64,
    pub name: String,
    /// index into `Scene::families`
    pub family: usize,
    pub type_name: Option<String>,
    pub frame: Frame,
    pub instance: Instance,
    /// the wall it sits in: (index into `Scene::walls`, segment, distance of its origin along the segment)
    pub host: Option<(usize, usize, f64)>,
}

/// A room, written as an `IfcSpace` with its base quantities.
pub struct RoomItem {
    pub id: u64,
    pub room: Room,
}

pub struct Scene {
    pub project_name: String,
    /// 48-bit key that makes the GlobalIds of this project different from every other project's
    pub project_key: u64,
    /// written in the file header; fixed by the caller so that exports are byte for byte repeatable
    pub timestamp: String,
    pub levels: Levels,
    pub walls: Vec<WallItem>,
    /// register of service openings in walls, by chain index into `walls`
    pub wall_penetrations: Vec<Penetration>,
    pub slabs: Vec<SlabItem>,
    pub shafts: Vec<ShaftVoid>,
    /// register of openings in slabs, by slab index into `slabs`
    pub floor_penetrations: Vec<FloorPenetration>,
    pub networks: Vec<Network>,
    pub families: Vec<FamilyRef>,
    pub items: Vec<FamilyItem>,
    pub rooms: Vec<RoomItem>,
    /// where family and shared parameters go; what it does not name goes to the custom sets
    pub pset_map: PsetMap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    NoLevels,
    NotFinite(String),
    /// the model has something this writer does not write yet
    Unsupported(String),
    /// an element number does not fit (only the lower 64 bits exist, but ids must be distinct)
    DuplicateId(u64),
    BadKey,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ExportError {}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub warnings: Vec<String>,
    pub walls: usize,
    pub openings: usize,
    pub slabs: usize,
    pub segments: usize,
    pub fittings: usize,
    pub ports: usize,
    pub connections: usize,
    pub items: usize,
    pub types: usize,
    /// occurrences whose shape is a mapped item of their type's shared shape
    pub mapped: usize,
    pub spaces: usize,
    /// base quantity sets written
    pub quantity_sets: usize,
    /// parameters written in the property set the mapping table names
    pub mapped_properties: usize,
}

pub struct Exported {
    pub text: String,
    pub report: Report,
}

type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: V3) -> V3 {
    let n = dot(a, a).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
/// Any unit vector square to `d`.
fn perp(d: V3) -> V3 {
    let h = if d[0].abs() <= d[1].abs() && d[0].abs() <= d[2].abs() {
        [1.0, 0.0, 0.0]
    } else if d[1].abs() <= d[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    unit(cross(d, h))
}

enum Val {
    Label(String),
    Text(String),
    Ident(String),
    Bool(bool),
    /// a positive length
    Len(f64),
    /// a length of any sign
    LenAny(f64),
    Area(f64),
    Vol(f64),
    Int(i64),
    Real(f64),
}

/// A value written as the kind a mapping row asks for; the reason when it cannot be.
fn convert(v: &Val, kind: PropKind) -> Result<Val, &'static str> {
    Ok(match (v, kind) {
        (Val::Real(x), PropKind::Real) => Val::Real(*x),
        (Val::Real(x), PropKind::Length) => Val::LenAny(*x),
        (Val::Real(x), PropKind::PositiveLength) if *x > 0.0 => Val::Len(*x),
        (Val::Real(_), PropKind::PositiveLength) => return Err("a positive length must be above 0"),
        (Val::Real(x), PropKind::Area) => Val::Area(*x),
        (Val::Real(x), PropKind::Volume) => Val::Vol(*x),
        (Val::Real(x), PropKind::Integer) if x.fract() == 0.0 && x.abs() < 1e15 => Val::Int(*x as i64),
        (Val::Real(_), PropKind::Integer) => return Err("an integer must be a whole number"),
        (Val::Real(x), PropKind::Boolean) if *x == 0.0 || *x == 1.0 => Val::Bool(*x == 1.0),
        (Val::Real(_), PropKind::Boolean) => return Err("a boolean must be 0 or 1"),
        (Val::Bool(b), PropKind::Boolean) => Val::Bool(*b),
        (Val::Int(i), PropKind::Integer) => Val::Int(*i),
        (Val::Label(t), PropKind::Label) => Val::Label(t.clone()),
        (Val::Label(t), PropKind::Text) => Val::Text(t.clone()),
        (Val::Label(t), PropKind::Identifier) => Val::Ident(t.clone()),
        _ => return Err("the value is not of a kind that can be written like that"),
    })
}

/// Values split by where they go: the custom set, and the property sets the mapping table names.
pub(crate) struct Split<'a> {
    pub custom: Vec<(&'a str, Val)>,
    pub sets: Vec<(&'a str, Vec<(&'a str, Val)>)>,
}

/// Sort `props` of an element of `class` by the mapping table. A value that cannot be written as its row asks
/// stays in the custom set, with a warning.
pub(crate) fn split_props<'a>(map: &'a PsetMap, class: &str, props: Vec<(&'a str, Val)>, who: &str, report: &mut Report) -> Split<'a> {
    let mut out = Split { custom: Vec::new(), sets: Vec::new() };
    for (n, v) in props {
        let Some(row) = map.find(class, n) else {
            out.custom.push((n, v));
            continue;
        };
        match convert(&v, row.kind) {
            Ok(cv) => {
                report.mapped_properties += 1;
                match out.sets.iter_mut().find(|(s, _)| *s == row.pset) {
                    Some((_, list)) => list.push((row.property.as_str(), cv)),
                    None => out.sets.push((row.pset.as_str(), vec![(row.property.as_str(), cv)])),
                }
            }
            Err(why) => {
                report.warnings.push(format!("{who}: '{n}' could not go to {}.{}: {why}", row.pset, row.property));
                out.custom.push((n, v));
            }
        }
    }
    out
}

struct Out<'a> {
    s: Step,
    key: u64,
    scene: &'a Scene,
    bad: Option<String>,
    aux: u64,
    used: std::collections::BTreeSet<(u64, u16)>,
    dup: Option<u64>,
    body_ctx: u32,
    storeys: Vec<(u32, u32, f64)>, // (storey entity, its placement, elevation)
    contained: BTreeMap<usize, Vec<u32>>,
    report: Report,
    /// every wall opening written: (wall, segment, [offset, width, sill, height], entity)
    opening_ids: Vec<(usize, usize, [f64; 4], u32)>,
    /// connector ports of placed families: (item, connector name) to port entity
    item_ports: BTreeMap<(u64, String), u32>,
}

impl<'a> Out<'a> {
    fn r(&mut self, x: f64, what: &str) -> String {
        if !x.is_finite() {
            self.bad.get_or_insert_with(|| what.to_string());
            return "0.".into();
        }
        real(x)
    }
    fn gid(&mut self, element: u64, part: u16) -> String {
        if !self.used.insert((element, part)) {
            self.dup.get_or_insert(element);
        }
        string(&guid::global_id(self.key, part, element))
    }
    /// ids for things that are not model elements (the site, relationships, property sets)
    fn gen_gid(&mut self) -> String {
        self.aux += 1;
        string(&guid::global_id(self.key, 0, (0xFFFF_FFFFu64 << 32) | self.aux))
    }
    fn p3(&mut self, p: V3) -> u32 {
        let (a, b, c) = (self.r(p[0], "coordinate"), self.r(p[1], "coordinate"), self.r(p[2], "coordinate"));
        self.s.add(format!("IFCCARTESIANPOINT(({a},{b},{c}))"))
    }
    fn p2(&mut self, x: f64, y: f64) -> u32 {
        let (a, b) = (self.r(x, "coordinate"), self.r(y, "coordinate"));
        self.s.add(format!("IFCCARTESIANPOINT(({a},{b}))"))
    }
    fn dir(&mut self, d: V3) -> u32 {
        let (a, b, c) = (self.r(d[0], "direction"), self.r(d[1], "direction"), self.r(d[2], "direction"));
        self.s.add(format!("IFCDIRECTION(({a},{b},{c}))"))
    }
    fn axis(&mut self, loc: V3, z: Option<V3>, x: Option<V3>) -> u32 {
        let l = self.p3(loc);
        let (zs, xs) = match (z, x) {
            (Some(z), Some(x)) => (format!("#{}", self.dir(z)), format!("#{}", self.dir(x))),
            _ => ("$".to_string(), "$".to_string()),
        };
        self.s.add(format!("IFCAXIS2PLACEMENT3D(#{l},{zs},{xs})"))
    }
    fn place(&mut self, rel: Option<u32>, loc: V3, z: Option<V3>, x: Option<V3>) -> u32 {
        let a = self.axis(loc, z, x);
        let r = rel.map_or("$".to_string(), |r| format!("#{r}"));
        self.s.add(format!("IFCLOCALPLACEMENT({r},#{a})"))
    }
    fn poly_ring(&mut self, pts: &[[f64; 2]]) -> u32 {
        let mut ids: Vec<String> = pts.iter().map(|p| format!("#{}", self.p2(p[0], p[1]))).collect();
        ids.push(ids[0].clone());
        self.s.add(format!("IFCPOLYLINE(({}))", ids.join(",")))
    }
    fn poly_profile(&mut self, pts: &[[f64; 2]]) -> u32 {
        let mut ids: Vec<String> = Vec::new();
        for p in pts.iter().chain(pts.first()) {
            let id = self.p2(p[0], p[1]);
            ids.push(format!("#{id}"));
        }
        let pl = self.s.add(format!("IFCPOLYLINE(({}))", ids.join(",")));
        self.s.add(format!("IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{pl})"))
    }
    fn rect_profile(&mut self, cx: f64, cy: f64, w: f64, h: f64) -> u32 {
        let c = self.p2(cx, cy);
        let a = self.s.add(format!("IFCAXIS2PLACEMENT2D(#{c},$)"));
        let (w, h) = (self.r(w, "size"), self.r(h, "size"));
        self.s.add(format!("IFCRECTANGLEPROFILEDEF(.AREA.,$,#{a},{w},{h})"))
    }
    fn circle_profile(&mut self, radius: f64) -> u32 {
        let c = self.p2(0.0, 0.0);
        let a = self.s.add(format!("IFCAXIS2PLACEMENT2D(#{c},$)"));
        let r = self.r(radius, "size");
        self.s.add(format!("IFCCIRCLEPROFILEDEF(.AREA.,$,#{a},{r})"))
    }
    fn extrusion(&mut self, profile: u32, loc: V3, z: Option<V3>, x: Option<V3>, depth: f64) -> u32 {
        let pos = self.axis(loc, z, x);
        let up = self.dir([0.0, 0.0, 1.0]);
        let d = self.r(depth, "depth");
        self.s.add(format!("IFCEXTRUDEDAREASOLID(#{profile},#{pos},#{up},{d})"))
    }
    fn shape(&mut self, solids: &[u32]) -> u32 {
        let rep = self.shape_rep("SweptSolid", solids);
        self.s.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"))
    }
    fn shape_rep(&mut self, kind: &str, items: &[u32]) -> u32 {
        let items: Vec<String> = items.iter().map(|i| format!("#{i}")).collect();
        self.s.add(format!("IFCSHAPEREPRESENTATION(#{},'Body','{kind}',({}))", self.body_ctx, items.join(",")))
    }
    fn size_profile(&mut self, size: RunSize) -> u32 {
        match size {
            RunSize::Round { diameter } => self.circle_profile(diameter / 2.0),
            RunSize::Rect { width, height, .. } => self.rect_profile(0.0, 0.0, width, height),
        }
    }
    fn pset_entity(&mut self, name: &str, props: Vec<(&str, Val)>) -> u32 {
        let mut ids = Vec::new();
        for (n, v) in props {
            let val = match v {
                Val::Label(s) => format!("IFCLABEL({})", string(&s)),
                Val::Text(s) => format!("IFCTEXT({})", string(&s)),
                Val::Ident(s) => format!("IFCIDENTIFIER({})", string(&s)),
                Val::Bool(b) => format!("IFCBOOLEAN({})", if b { ".T." } else { ".F." }),
                Val::Len(x) => format!("IFCPOSITIVELENGTHMEASURE({})", self.r(x, "property")),
                Val::LenAny(x) => format!("IFCLENGTHMEASURE({})", self.r(x, "property")),
                Val::Area(x) => format!("IFCAREAMEASURE({})", self.r(x, "property")),
                Val::Vol(x) => format!("IFCVOLUMEMEASURE({})", self.r(x, "property")),
                Val::Int(i) => format!("IFCINTEGER({i})"),
                Val::Real(x) => format!("IFCREAL({})", self.r(x, "property")),
            };
            ids.push(format!("#{}", self.s.add(format!("IFCPROPERTYSINGLEVALUE({},$,{val},$)", string(n)))));
        }
        let g = self.gen_gid();
        self.s.add(format!("IFCPROPERTYSET({g},$,{},$,({}))", string(name), ids.join(",")))
    }
    fn pset(&mut self, owner: u32, name: &str, props: Vec<(&str, Val)>) {
        let ps = self.pset_entity(name, props);
        let g = self.gen_gid();
        self.s.add(format!("IFCRELDEFINESBYPROPERTIES({g},$,$,$,(#{owner}),#{ps})"));
    }
    /// A base quantity set on an element.
    fn qto(&mut self, owner: u32, name: &str, qs: &[quantities::Qty]) {
        let mut ids = Vec::new();
        for q in qs {
            let v = self.r(q.value.max(0.0), "quantity");
            let (class, _) = match q.kind {
                quantities::Kind::Length => ("IFCQUANTITYLENGTH", 0),
                quantities::Kind::Area => ("IFCQUANTITYAREA", 0),
                quantities::Kind::Volume => ("IFCQUANTITYVOLUME", 0),
            };
            ids.push(format!("#{}", self.s.add(format!("{class}({},$,$,{v},$)", string(q.name)))));
        }
        let g = self.gen_gid();
        let eq = self.s.add(format!("IFCELEMENTQUANTITY({g},$,{},$,$,({}))", string(name), ids.join(",")));
        let g = self.gen_gid();
        self.s.add(format!("IFCRELDEFINESBYPROPERTIES({g},$,$,$,(#{owner}),#{eq})"));
        self.report.quantity_sets += 1;
    }
    /// Property sets for the values of an element, by the mapping table; the rest to the custom set `custom`.
    fn pset_mapped(&mut self, owner: u32, class: &str, custom: &str, props: Vec<(&str, Val)>, who: &str) {
        let scene = self.scene;
        let mut report = std::mem::take(&mut self.report);
        let split = split_props(&scene.pset_map, class, props, who, &mut report);
        self.report = report;
        if !split.custom.is_empty() {
            self.pset(owner, custom, split.custom);
        }
        for (name, list) in split.sets {
            self.pset(owner, name, list);
        }
    }
    fn storey_for(&self, z: f64) -> usize {
        let mut k = 0;
        for (i, (_, _, e)) in self.storeys.iter().enumerate() {
            if *e <= z + 1e-6 {
                k = i;
            }
        }
        k
    }
    fn contain(&mut self, storey: usize, e: u32) {
        self.contained.entry(storey).or_default().push(e);
    }
}

fn fire_label(m: u32) -> String {
    format!("{m} min")
}

fn segment_class(d: Domain) -> (&'static str, &'static str) {
    match d {
        Domain::Pipe => ("IFCPIPESEGMENT", ".RIGIDSEGMENT."),
        Domain::Duct => ("IFCDUCTSEGMENT", ".RIGIDSEGMENT."),
        Domain::CableTray => ("IFCCABLECARRIERSEGMENT", ".CABLETRAYSEGMENT."),
        Domain::Conduit => ("IFCCABLECARRIERSEGMENT", ".CONDUITSEGMENT."),
        Domain::Electrical => ("IFCCABLESEGMENT", ".CABLESEGMENT."),
    }
}

fn fitting_class(d: Domain, k: &FittingKind) -> (&'static str, &'static str) {
    use FittingKind::*;
    match d {
        Domain::Pipe | Domain::Duct => {
            let c = if d == Domain::Pipe { "IFCPIPEFITTING" } else { "IFCDUCTFITTING" };
            (c, match k { Elbow { .. } => ".BEND.", Transition => ".TRANSITION.", Tee { .. } | Tap { .. } => ".JUNCTION." })
        }
        Domain::CableTray | Domain::Conduit => ("IFCCABLECARRIERFITTING", match k { Elbow { .. } => ".BEND.", Transition => ".REDUCER.", Tee { .. } | Tap { .. } => ".TEE." }),
        Domain::Electrical => ("IFCCABLEFITTING", match k { Elbow { .. } => ".NOTDEFINED.", Transition => ".TRANSITION.", Tee { .. } | Tap { .. } => ".JUNCTION." }),
    }
}

fn size_props(size: RunSize) -> Vec<(&'static str, Val)> {
    match size {
        RunSize::Round { diameter } => vec![("Shape", Val::Label("round".into())), ("Diameter", Val::Len(diameter))],
        RunSize::Rect { width, height, .. } => vec![("Shape", Val::Label("rectangular".into())), ("Width", Val::Len(width)), ("Height", Val::Len(height))],
    }
}

/// Write the scene as IFC4.
pub fn export(scene: &Scene) -> Result<Exported, ExportError> {
    if scene.project_key > guid::MAX_PROJECT_KEY {
        return Err(ExportError::BadKey);
    }
    if scene.levels.all().is_empty() {
        return Err(ExportError::NoLevels);
    }
    let mut o = Out { s: Step::default(), key: scene.project_key, scene, bad: None, aux: 0, used: Default::default(), dup: None, body_ctx: 0, storeys: Vec::new(), contained: BTreeMap::new(), report: Report::default(), opening_ids: Vec::new(), item_ports: BTreeMap::new() };
    // context and units
    let wcs = o.axis([0.0; 3], None, None);
    let ctx = o.s.add(format!("IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-6,#{wcs},$)"));
    o.body_ctx = o.s.add(format!("IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{ctx},$,.MODEL_VIEW.,$)"));
    let u_len = o.s.add("IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)".into());
    let u_area = o.s.add("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)".into());
    let u_vol = o.s.add("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)".into());
    let u_ang = o.s.add("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)".into());
    let units = o.s.add(format!("IFCUNITASSIGNMENT((#{u_len},#{u_area},#{u_vol},#{u_ang}))"));
    let g = o.gen_gid();
    let project = o.s.add(format!("IFCPROJECT({g},$,{},$,$,$,$,(#{ctx}),#{units})", string(&scene.project_name)));
    // spatial structure
    let site_pl = o.place(None, [0.0; 3], None, None);
    let g = o.gen_gid();
    let site = o.s.add(format!("IFCSITE({g},$,'Site',$,$,#{site_pl},$,$,.ELEMENT.,$,$,$,$,$)"));
    let bld_pl = o.place(Some(site_pl), [0.0; 3], None, None);
    let g = o.gen_gid();
    let bld = o.s.add(format!("IFCBUILDING({g},$,'Building',$,$,#{bld_pl},$,$,.ELEMENT.,$,$,$)"));
    let g = o.gen_gid();
    o.s.add(format!("IFCRELAGGREGATES({g},$,$,$,#{project},(#{site}))"));
    let g = o.gen_gid();
    o.s.add(format!("IFCRELAGGREGATES({g},$,$,$,#{site},(#{bld}))"));
    let mut storey_ids = Vec::new();
    for l in scene.levels.all() {
        let pl = o.place(Some(bld_pl), [0.0, 0.0, l.elevation], None, None);
        let g = o.gen_gid();
        let e = o.r(l.elevation, "level elevation");
        let st = o.s.add(format!("IFCBUILDINGSTOREY({g},$,{},$,$,#{pl},$,$,.ELEMENT.,{e})", string(&l.name)));
        storey_ids.push(st);
        o.storeys.push((st, pl, l.elevation));
    }
    let g = o.gen_gid();
    o.s.add(format!("IFCRELAGGREGATES({g},$,$,$,#{bld},({}))", storey_ids.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));

    write_walls(&mut o)?;
    families::write_families(&mut o)?;
    write_slabs(&mut o)?;
    write_spaces(&mut o)?;
    write_networks(&mut o)?;

    // containment, per storey
    let contained = std::mem::take(&mut o.contained);
    for (si, elems) in contained {
        let g = o.gen_gid();
        let st = o.storeys[si].0;
        o.s.add(format!("IFCRELCONTAINEDINSPATIALSTRUCTURE({g},$,$,$,({}),#{st})", elems.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
    }
    if let Some(w) = o.bad {
        return Err(ExportError::NotFinite(w));
    }
    if let Some(id) = o.dup {
        return Err(ExportError::DuplicateId(id));
    }
    let text = o.s.finish(&scene.project_name, &scene.timestamp);
    Ok(Exported { text, report: o.report })
}

fn write_walls(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for (wi, w) in scene.walls.iter().enumerate() {
        let c = &w.chain;
        if c.tops.iter().any(|t| t.is_some()) {
            return Err(ExportError::Unsupported(format!("wall {} has a sloped top", w.name)));
        }
        if c.poly_openings.iter().any(|v| !v.is_empty()) {
            return Err(ExportError::Unsupported(format!("wall {} has polygonal openings", w.name)));
        }
        let plans = c.plans().map_err(|e| ExportError::Unsupported(format!("wall {}: {e:?}", w.name)))?;
        let st = o.storey_for(c.base_z);
        let elev = o.storeys[st].2;
        let pl = o.place(Some(o.storeys[st].1), [0.0; 3], None, None);
        let mut solids = Vec::new();
        for p in &plans {
            let prof = o.poly_profile(&[p.l0, p.l1, p.r1, p.r0]);
            solids.push(o.extrusion(prof, [0.0, 0.0, c.base_z - elev], None, None, c.height));
        }
        let shape = o.shape(&solids);
        let g = o.gid(w.id, 0);
        let wall = o.s.add(format!("IFCWALL({g},$,{},$,$,#{pl},#{shape},$,.STANDARD.)", string(&w.name)));
        o.contain(st, wall);
        o.report.walls += 1;
        match quantities::wall(c) {
            Ok(q) => o.qto(wall, "Qto_WallBaseQuantities", &q),
            Err(e) => o.report.warnings.push(format!("wall {}: no base quantities: {e}", w.name)),
        }
        if let Some(m) = c.fire_rating_min {
            o.pset(wall, "Pset_WallCommon", vec![("FireRating", Val::Label(fire_label(m)))]);
        }
        // openings
        let n = c.points.len();
        let mut k = 0u16;
        for (si, ops) in c.openings.iter().enumerate() {
            let (p0, p1) = (c.points[si], c.points[(si + 1) % n]);
            let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
            let l = dx.hypot(dy);
            let d = [dx / l, dy / l, 0.0];
            let right = [d[1], -d[0], 0.0];
            for op in ops {
                k += 1;
                let ov = 0.05;
                let t = c.thickness;
                let cs = op.offset + op.width / 2.0;
                let centre = [p0[0] + d[0] * cs, p0[1] + d[1] * cs, c.base_z - elev + op.sill + op.height / 2.0];
                let loc = [centre[0] - right[0] * (t / 2.0 + ov), centre[1] - right[1] * (t / 2.0 + ov), centre[2]];
                let prof = o.rect_profile(0.0, 0.0, op.width, op.height);
                let solid = o.extrusion(prof, [0.0; 3], Some([0.0, 0.0, 1.0]), Some([1.0, 0.0, 0.0]), t + 2.0 * ov);
                // the profile plane sits at the opening: a placement of its own, relative to the wall
                let opl = o.place(Some(pl), loc, Some(right), Some(d));
                let shape = o.shape(&[solid]);
                let pen = scene.wall_penetrations.iter().find(|p| {
                    p.id.chain == wi && p.id.segment == si && p.cut && (p.offset - op.offset).abs() < 1e-9 && (p.width - op.width).abs() < 1e-9 && (p.sill - op.sill).abs() < 1e-9 && (p.height - op.height).abs() < 1e-9
                });
                let name = pen.map_or_else(|| format!("Opening {k}"), |p| p.id.to_string());
                let g = o.gid(w.id, k);
                let oe = o.s.add(format!("IFCOPENINGELEMENT({g},$,{},$,$,#{opl},#{shape},$,.OPENING.)", string(&name)));
                let g = o.gen_gid();
                o.s.add(format!("IFCRELVOIDSELEMENT({g},$,$,$,#{wall},#{oe})"));
                o.report.openings += 1;
                o.opening_ids.push((wi, si, [op.offset, op.width, op.sill, op.height], oe));
                if let Some(p) = pen {
                    let systems: Vec<&str> = p.systems.iter().map(|s| s.ifc_system()).collect();
                    let mut props = vec![("Register", Val::Label(p.id.to_string())), ("Systems", Val::Label(systems.join(","))), ("InFirePartition", Val::Bool(p.in_fire_partition()))];
                    if let Some(m) = p.fire_rating_min {
                        props.push(("FireRating", Val::Label(fire_label(m))));
                    }
                    o.pset(oe, "OpenBIM_Penetration", props);
                }
            }
        }
    }
    Ok(())
}

fn write_slabs(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for (si, item) in scene.slabs.iter().enumerate() {
        let s = &item.slab;
        s.check().map_err(|e| ExportError::Unsupported(format!("slab {}: {e:?}", s.name)))?;
        let st = o.storey_for(s.top);
        let elev = o.storeys[st].2;
        let bottom = s.top - s.thickness - elev;
        let pl = o.place(Some(o.storeys[st].1), [0.0; 3], None, None);
        let prof = o.poly_profile(&s.outline);
        let solid = o.extrusion(prof, [0.0, 0.0, bottom], None, None, s.thickness);
        let shape = o.shape(&[solid]);
        let g = o.gid(item.id, 0);
        let slab = o.s.add(format!("IFCSLAB({g},$,{},$,$,#{pl},#{shape},$,.FLOOR.)", string(&s.name)));
        o.contain(st, slab);
        o.report.slabs += 1;
        let mut void_area = 0.0;
        for sh in scene.shafts.iter().filter(|sh| s.top > sh.z_bottom + 1e-6 && s.top < sh.z_top - 1e-6) {
            match quantities::overlap_area(&s.outline, &sh.outline).or_else(|| quantities::overlap_area(&sh.outline, &s.outline)) {
                Some(a) => void_area += a,
                None => o.report.warnings.push(format!("slab {}: shaft {} area not found (neither outline is convex)", s.name, sh.name)),
            }
        }
        for p in scene.floor_penetrations.iter().filter(|p| p.id.slab == si && p.kind == FloorOpeningKind::Cut && p.fits) {
            void_area += p.size_x() * p.size_y();
        }
        o.qto(slab, "Qto_SlabBaseQuantities", &quantities::slab(s, void_area));
        if let Some(m) = s.fire_rating_min {
            o.pset(slab, "Pset_SlabCommon", vec![("FireRating", Val::Label(fire_label(m)))]);
        }
        let ov = 0.05;
        let mut k = 0u16;
        // shaft voids that cut this slab
        for sh in scene.shafts.iter() {
            if s.top > sh.z_bottom + 1e-6 && s.top < sh.z_top - 1e-6 {
                k += 1;
                let prof = o.poly_profile(&sh.outline);
                let solid = o.extrusion(prof, [0.0, 0.0, bottom - ov], None, None, s.thickness + 2.0 * ov);
                let opl = o.place(Some(pl), [0.0; 3], None, None);
                let shape = o.shape(&[solid]);
                let g = o.gid(item.id, k);
                let oe = o.s.add(format!("IFCOPENINGELEMENT({g},$,{},$,$,#{opl},#{shape},$,.OPENING.)", string(&format!("Shaft {}", sh.name))));
                let g = o.gen_gid();
                o.s.add(format!("IFCRELVOIDSELEMENT({g},$,$,$,#{slab},#{oe})"));
                o.report.openings += 1;
            }
        }
        for p in scene.floor_penetrations.iter().filter(|p| p.id.slab == si) {
            if p.kind != FloorOpeningKind::Cut {
                continue; // inside a shaft, which is already a void
            }
            if !p.fits {
                o.report.warnings.push(format!("floor opening {} does not fit inside slab {} and was not cut", p.id, s.name));
                continue;
            }
            k += 1;
            let prof = o.rect_profile((p.x0 + p.x1) / 2.0, (p.y0 + p.y1) / 2.0, p.size_x(), p.size_y());
            let solid = o.extrusion(prof, [0.0, 0.0, bottom - ov], None, None, s.thickness + 2.0 * ov);
            let opl = o.place(Some(pl), [0.0; 3], None, None);
            let shape = o.shape(&[solid]);
            let g = o.gid(item.id, k);
            let oe = o.s.add(format!("IFCOPENINGELEMENT({g},$,{},$,$,#{opl},#{shape},$,.OPENING.)", string(&p.id.to_string())));
            let g = o.gen_gid();
            o.s.add(format!("IFCRELVOIDSELEMENT({g},$,$,$,#{slab},#{oe})"));
            o.report.openings += 1;
            let systems: Vec<&str> = p.systems.iter().map(|s| s.ifc_system()).collect();
            let mut props = vec![("Register", Val::Label(p.id.to_string())), ("Systems", Val::Label(systems.join(","))), ("InFirePartition", Val::Bool(p.in_fire_partition()))];
            if let Some(m) = p.fire_rating_min {
                props.push(("FireRating", Val::Label(fire_label(m))));
            }
            o.pset(oe, "OpenBIM_Penetration", props);
        }
    }
    Ok(())
}

fn write_spaces(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for r in &scene.rooms {
        let room = &r.room;
        if room.level >= scene.levels.all().len() {
            return Err(ExportError::Unsupported(format!("room {}: no level {}", room.name, room.level)));
        }
        if room.outline.len() < 3 {
            return Err(ExportError::Unsupported(format!("room {}: outline has fewer than 3 points", room.name)));
        }
        let st = room.level;
        let pl = o.place(Some(o.storeys[st].1), [0.0, 0.0, 0.0], None, None);
        let outer = o.poly_ring(&room.outline);
        let prof = if room.holes.is_empty() {
            o.s.add(format!("IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{outer})"))
        } else {
            let holes: Vec<String> = room.holes.iter().map(|h| format!("#{}", o.poly_ring(h))).collect();
            o.s.add(format!("IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,$,#{outer},({}))", holes.join(",")))
        };
        let solid = o.extrusion(prof, [0.0; 3], None, None, room.height);
        let shape = o.shape(&[solid]);
        let g = o.gid(r.id, 0);
        let sp = o.s.add(format!("IFCSPACE({g},$,{},$,$,#{pl},#{shape},$,.ELEMENT.,.INTERNAL.,$)", string(&room.name)));
        let g = o.gen_gid();
        o.s.add(format!("IFCRELAGGREGATES({g},$,$,$,#{},(#{sp}))", o.storeys[st].0));
        o.report.spaces += 1;
        let props = vec![("Usage", Val::Label(room.usage.clone())), ("Occupancy", Val::Real(room.occupancy))];
        o.pset_mapped(sp, "IfcSpace", "OpenBIM_Space", props, &room.name);
        o.qto(sp, "Qto_SpaceBaseQuantities", &quantities::space(room));
    }
    Ok(())
}

/// Port entities of one piece, in port order.
struct PieceOut {
    ports: Vec<u32>,
}

fn write_port(o: &mut Out, key: (u64, u16), name: &str, pl_rel: u32, loc: V3, z: V3, ds: (Domain, System)) -> u32 {
    let ((id, part), (domain, system)) = (key, ds);
    let x = perp(z);
    let pl = o.place(Some(pl_rel), loc, Some(z), Some(x));
    let g = o.gid(id, part);
    o.report.ports += 1;
    o.s.add(format!("IFCDISTRIBUTIONPORT({g},$,{},$,$,#{pl},$,.SOURCEANDSINK.,.{}.,.{}.)", string(name), domain.ifc_port_type(), system.ifc_system()))
}

fn nest(o: &mut Out, parent: u32, ports: &[u32]) {
    if ports.is_empty() {
        return;
    }
    let g = o.gen_gid();
    o.s.add(format!("IFCRELNESTS({g},$,$,$,#{parent},({}))", ports.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
}

fn write_networks(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for (ni, net) in scene.networks.iter().enumerate() {
        let mut segs: Vec<PieceOut> = Vec::new();
        let mut fits: Vec<PieceOut> = Vec::new();
        let mut by_system: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
        for sg in &net.segments {
            let r = &sg.run;
            let d = sub(r.b, r.a);
            let len = dot(d, d).sqrt();
            if len.is_nan() || len <= 1e-9 {
                return Err(ExportError::Unsupported(format!("run {} has no length", r.label)));
            }
            let d = unit(d);
            let xdir = match r.size {
                RunSize::Round { .. } => perp(d),
                RunSize::Rect { roll, .. } => unit(cross(up_for(d, roll), d)),
            };
            let st = o.storey_for(r.a[2].min(r.b[2]));
            let elev = o.storeys[st].2;
            let loc = [r.a[0], r.a[1], r.a[2] - elev];
            let pl = o.place(Some(o.storeys[st].1), loc, Some(d), Some(xdir));
            let prof = o.size_profile(r.size);
            let solid = o.extrusion(prof, [0.0; 3], None, None, len);
            let shape = o.shape(&[solid]);
            let (class, pre) = segment_class(r.domain);
            let g = o.gid(sg.id, 0);
            let e = o.s.add(format!("{class}({g},$,{},$,$,#{pl},#{shape},$,{pre})", string(&r.label)));
            o.contain(st, e);
            o.report.segments += 1;
            let mut props = size_props(r.size);
            props.push(("System", Val::Label(r.system.ifc_system().into())));
            o.pset(e, "OpenBIM_Mep", props);
            let p0 = write_port(o, (sg.id, 0x1000), "Start", pl, [0.0; 3], [0.0, 0.0, -1.0], (r.domain, r.system));
            let p1 = write_port(o, (sg.id, 0x1001), "End", pl, [0.0, 0.0, len], [0.0, 0.0, 1.0], (r.domain, r.system));
            nest(o, e, &[p0, p1]);
            by_system.entry(r.system.code()).or_default().push(e);
            segs.push(PieceOut { ports: vec![p0, p1] });
        }
        for nf in &net.fittings {
            let f: &Fitting = &nf.fitting;
            let st = o.storey_for(f.at[2]);
            let elev = o.storeys[st].2;
            let pl = o.place(Some(o.storeys[st].1), [f.at[0], f.at[1], f.at[2] - elev], None, None);
            let mut solids = Vec::new();
            for k in 0..f.ports.len() {
                if f.trim[k] > 1e-9 {
                    let prof = o.size_profile(f.port_sizes[k]);
                    let z = unit(f.ports[k]);
                    let x = match f.port_sizes[k] {
                        RunSize::Rect { roll, .. } => unit(cross(up_for(z, roll), z)),
                        _ => perp(z),
                    };
                    solids.push(o.extrusion(prof, [0.0; 3], Some(z), Some(x), f.trim[k]));
                }
            }
            let shape = if solids.is_empty() { None } else { Some(o.shape(&solids)) };
            let (class, pre) = fitting_class(nf.domain, &f.kind);
            let g = o.gid(nf.id, 0);
            let rep = shape.map_or("$".to_string(), |s| format!("#{s}"));
            let e = o.s.add(format!("{class}({g},$,{},$,$,#{pl},{rep},$,{pre})", string(&f.type_name)));
            o.contain(st, e);
            o.report.fittings += 1;
            let mut props = vec![("PartCode", Val::Label(f.part_code.clone())), ("System", Val::Label(nf.system.ifc_system().into()))];
            if f.radius > 0.0 {
                props.push(("BendRadius", Val::Len(f.radius)));
            }
            if let FittingKind::Elbow { angle } | FittingKind::Tee { branch_angle: angle } | FittingKind::Tap { branch_angle: angle } = f.kind {
                props.push(("AngleDegrees", Val::Len(angle)));
            }
            props.push(("Legs", Val::Int(f.ports.len() as i64)));
            o.pset(e, "OpenBIM_Mep", props);
            let mut ports = Vec::new();
            for k in 0..f.ports.len() {
                let loc = [f.ports[k][0] * f.trim[k], f.ports[k][1] * f.trim[k], f.ports[k][2] * f.trim[k]];
                ports.push(write_port(o, (nf.id, 0x1000 + k as u16), &format!("Leg {}", k + 1), pl, loc, unit(f.ports[k]), (nf.domain, nf.system)));
            }
            nest(o, e, &ports);
            by_system.entry(nf.system.code()).or_default().push(e);
            fits.push(PieceOut { ports });
        }
        // equipment: one bare placeholder per equipment element, owning all that element's connector ports
        let mut holder: BTreeMap<u64, (u32, u32, V3, Vec<u32>)> = BTreeMap::new();
        let mut eq_ports: Vec<u32> = Vec::new();
        for eq in &net.equipment {
            if let Some(&p) = o.item_ports.get(&(eq.element, eq.connector.clone())) {
                eq_ports.push(p); // the equipment is a placed family: its own port is used
                continue;
            }
            let (pl, base) = match holder.get(&eq.element).map(|h| (h.1, h.2)) {
                Some(x) => x,
                None => {
                    let st = o.storey_for(eq.pos[2]);
                    let elev = o.storeys[st].2;
                    let pl = o.place(Some(o.storeys[st].1), [eq.pos[0], eq.pos[1], eq.pos[2] - elev], None, None);
                    let g = o.gid(eq.element, 0);
                    let e = o.s.add(format!("IFCFLOWTERMINAL({g},$,{},$,$,#{pl},$,$)", string(&format!("Equipment {}", eq.element))));
                    o.contain(st, e);
                    o.pset(e, "OpenBIM_Equipment", vec![("Placeholder", Val::Bool(true))]);
                    holder.insert(eq.element, (e, pl, eq.pos, Vec::new()));
                    (pl, eq.pos)
                }
            };
            let part = 0x1000 + holder.get(&eq.element).map_or(0, |h| h.3.len()) as u16;
            let loc = sub(eq.pos, base);
            let p = write_port(o, (eq.element, part), &eq.connector, pl, loc, unit(eq.dir), (eq.domain, eq.system));
            if let Some(h) = holder.get_mut(&eq.element) {
                h.3.push(p);
            }
            eq_ports.push(p);
        }
        for (e, _, _, ports) in holder.values() {
            nest(o, *e, ports);
        }
        // connections
        let port_entity = |p: &PortRef| -> Option<u32> {
            match p.node {
                NodeRef::Segment(i) => segs.get(i)?.ports.get(p.port).copied(),
                NodeRef::Fitting(i) => fits.get(i)?.ports.get(p.port).copied(),
                NodeRef::Equipment(i) => eq_ports.get(i).copied(),
            }
        };
        for (a, b) in net.links() {
            let (ea, eb) = (port_entity(a).ok_or_else(|| ExportError::Unsupported("a link points at a missing port".into()))?, port_entity(b).ok_or_else(|| ExportError::Unsupported("a link points at a missing port".into()))?);
            let g = o.gen_gid();
            o.s.add(format!("IFCRELCONNECTSPORTS({g},$,$,$,#{ea},#{eb},$)"));
            o.report.connections += 1;
        }
        // systems
        for (code, elems) in by_system {
            let sys = System::from_code(code).unwrap_or(System::Other);
            let g = o.gen_gid();
            let grp = o.s.add(format!("IFCDISTRIBUTIONSYSTEM({g},$,{},$,$,$,.{}.)", string(&format!("{} {}", sys.ifc_system(), ni + 1)), sys.ifc_system()));
            let g = o.gen_gid();
            o.s.add(format!("IFCRELASSIGNSTOGROUP({g},$,$,$,({}),$,#{grp})", elems.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
        }
    }
    Ok(())
}
