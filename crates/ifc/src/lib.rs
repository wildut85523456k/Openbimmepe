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
pub mod reader;
pub mod step_read;
mod families;
pub mod guid;
pub mod boundaries;
pub mod georef;
pub mod lighting;
pub mod pset_map;
pub mod walls;
pub mod quantities;
pub mod step;

use openbim_arch::{Levels, Room};
use openbim_family::mep::{up_for, Domain, FlowDir, System};
use openbim_family::{Compiled, Instance};
use openbim_geom::{profile::Profile, WallChain};
use openbim_place::floors::{FloorOpeningKind, FloorPenetration, ShaftVoid, Slab};
use openbim_place::penetrations::{Firestops, OpeningShape, Penetration, RunSize};
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
    /// index into `Scene::wall_types`; its layers must add up to the thickness of the chain
    pub wall_type: Option<usize>,
    /// Revit's Room Bounding: whether rooms are bounded by this wall
    pub room_bounding: bool,
}

/// A column: a straight prism on a convex plan footprint (counter-clockwise or clockwise).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnItem {
    pub id: u64,
    pub name: String,
    pub footprint: Vec<[f64; 2]>,
    pub base_z: f64,
    pub height: f64,
    /// Revit's Room Bounding: whether rooms are bounded by this column. It bounds a room where a wall is attached to it.
    pub room_bounding: bool,
}

impl ColumnItem {
    /// A column standing on a section centred at `centre`, from `base_z` up `height`. An I-section has no convex
    /// footprint, so it is refused for a column that bounds rooms.
    pub fn from_profile(id: u64, name: &str, profile: &Profile, centre: [f64; 2], base_z: f64, height: f64, room_bounding: bool) -> Result<ColumnItem, String> {
        profile.check().map_err(str::to_string)?;
        if room_bounding && matches!(profile, Profile::I { .. }) {
            return Err("an I-section column cannot bound rooms: its footprint is not convex".into());
        }
        let footprint = profile.outline().into_iter().map(|p| [centre[0] + p[0], centre[1] + p[1]]).collect();
        Ok(ColumnItem { id, name: name.into(), footprint, base_z, height, room_bounding })
    }
}

/// What a framing member is for, as Revit's Structural Usage. Bracing and purlins export as `IfcMember`, the rest as
/// `IfcBeam`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FramingUsage {
    #[default]
    Girder,
    Joist,
    Purlin,
    Brace,
    Other,
}

impl FramingUsage {
    pub const ALL: [FramingUsage; 5] = [FramingUsage::Girder, FramingUsage::Joist, FramingUsage::Purlin, FramingUsage::Brace, FramingUsage::Other];
    pub fn name(self) -> &'static str {
        match self {
            FramingUsage::Girder => "Girder",
            FramingUsage::Joist => "Joist",
            FramingUsage::Purlin => "Purlin",
            FramingUsage::Brace => "Brace",
            FramingUsage::Other => "Other",
        }
    }
    /// The IFC class and its predefined type.
    fn ifc(self) -> (&'static str, &'static str) {
        match self {
            FramingUsage::Girder => ("IFCBEAM", ".BEAM."),
            FramingUsage::Joist => ("IFCBEAM", ".JOIST."),
            FramingUsage::Purlin => ("IFCMEMBER", ".PURLIN."),
            FramingUsage::Brace => ("IFCMEMBER", ".BRACE."),
            FramingUsage::Other => ("IFCBEAM", ".NOTDEFINED."),
        }
    }
}

/// A beam: a straight member between two points of the model with a section centred on that line. The section's
/// x lies horizontal across the beam, its y at right angles to both (upright for a level beam).
#[derive(Debug, Clone, PartialEq)]
pub struct BeamItem {
    pub id: u64,
    pub name: String,
    /// the ends of the centre line (model coordinates, metres)
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub profile: Profile,
    pub usage: FramingUsage,
    /// reinforcing bars inside the beam
    pub bars: Vec<BarItem>,
}

/// A reinforcing bar: a round bar of `diameter` along a polyline of the model, a stirrup when `shear` is set.
#[derive(Debug, Clone, PartialEq)]
pub struct BarItem {
    pub id: u64,
    pub name: String,
    pub path: Vec<[f64; 3]>,
    pub diameter: f64,
    pub shear: bool,
}

impl BeamItem {
    /// A beam of rectangular section.
    pub fn rect(id: u64, name: &str, a: [f64; 3], b: [f64; 3], width: f64, depth: f64) -> BeamItem {
        BeamItem { id, name: name.into(), a, b, profile: Profile::Rect { width, depth }, usage: FramingUsage::Girder, bars: Vec::new() }
    }

    /// The unit vectors along the beam, across it (horizontal) and up out of it; None for a member that is vertical
    /// (a column), has no length, or has a section that does not check.
    pub fn axes(&self) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        let d = [self.b[0] - self.a[0], self.b[1] - self.a[1], self.b[2] - self.a[2]];
        let len = self.length();
        let finite = self.a.iter().chain(&self.b).all(|v| v.is_finite());
        if !finite || len < 1e-9 || self.profile.check().is_err() {
            return None;
        }
        let u = [d[0] / len, d[1] / len, d[2] / len];
        // across = up x u, which is horizontal
        let (sx, sy) = (-u[1], u[0]);
        let sl = (sx * sx + sy * sy).sqrt();
        if sl < 1e-9 {
            return None;
        }
        let side = [sx / sl, sy / sl, 0.0];
        let up = [u[1] * side[2] - u[2] * side[1], u[2] * side[0] - u[0] * side[2], u[0] * side[1] - u[1] * side[0]];
        Some((u, side, up))
    }
    pub fn length(&self) -> f64 {
        let d = [self.b[0] - self.a[0], self.b[1] - self.a[1], self.b[2] - self.a[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }
}

/// What a slab item is. Rooms are slab to slab by default; a floor finish or a false ceiling takes over as the
/// room's floor or ceiling where there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlabRole {
    #[default]
    Structure,
    /// a finish laid on a floor slab: written as an IfcCovering (flooring)
    FloorFinish,
    /// a false ceiling hung under a slab: written as an IfcCovering (ceiling)
    Ceiling,
    /// a structural slab on the ground: an IfcSlab BASESLAB, a floor for the rooms above it like Structure
    Foundation,
    /// an isolated footing under a column: an IfcFooting PAD_FOOTING, which bounds no room
    PadFooting,
    /// a strip footing under a wall: an IfcFooting STRIP_FOOTING, which bounds no room
    StripFooting,
}

pub struct SlabItem {
    pub id: u64,
    pub slab: Slab,
    /// a structural slab, or a covering laid on one
    pub role: SlabRole,
    /// Revit's Room Bounding: whether rooms are bounded by this slab (floor and ceiling)
    pub room_bounding: bool,
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

/// A value of a type, written as a property of the type's property set.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeProp {
    Text(String),
    /// a length in metres
    Len(f64),
    Int(i64),
}

/// A type of beams, columns, slabs or footings: a name, the values it governs and the elements that follow it.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeItem {
    /// the element number of the type in the document (its GlobalId comes from it)
    pub id: u64,
    pub name: String,
    pub class: TypeClass,
    pub props: Vec<(String, TypeProp)>,
    /// element numbers of the instances; instances that are not in the file are left out
    pub instances: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeClass {
    /// IfcBeamType with its IFC4 predefined type, e.g. `BEAM`
    Beam(&'static str),
    Column,
    /// IfcSlabType with its predefined type, e.g. `FLOOR`
    Slab(&'static str),
    StripFooting,
    PadFooting,
    Wall,
}

impl TypeClass {
    fn entity(self) -> (&'static str, &'static str) {
        match self {
            TypeClass::Beam(p) => ("IFCBEAMTYPE", p),
            TypeClass::Column => ("IFCCOLUMNTYPE", "COLUMN"),
            TypeClass::Slab(p) => ("IFCSLABTYPE", p),
            TypeClass::StripFooting => ("IFCFOOTINGTYPE", "STRIP_FOOTING"),
            TypeClass::PadFooting => ("IFCFOOTINGTYPE", "PAD_FOOTING"),
            TypeClass::Wall => ("IFCWALLTYPE", "STANDARD"),
        }
    }
}

/// The `OpenBIM_Element` property set of a member: what a companion file or a later import matches on.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberInfo {
    pub category: String,
    /// the name of its type, empty for none
    pub type_name: String,
    /// 16 hex digits over the values exported for it; an import that finds another value knows the file was edited elsewhere
    pub fingerprint: String,
}

/// A roof by footprint: its solid as faces (each a ring of points, wound outward) and the shape it is.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofItem {
    pub id: u64,
    pub name: String,
    /// the height of the eaves, which puts the roof on a storey
    pub eaves_z: f64,
    pub verts: Vec<[f64; 3]>,
    pub faces: Vec<Vec<usize>>,
    /// IfcRoofTypeEnum: FLAT_ROOF, SHED_ROOF, GABLE_ROOF, HIP_ROOF or NOTDEFINED
    pub shape: &'static str,
}

/// A straight stair: a stepped block `width` wide that climbs `risers` risers over `risers - 1` treads along `dir`.
#[derive(Clone, Debug, PartialEq)]
pub struct StairItem {
    pub id: u64,
    pub name: String,
    /// the foot of the first riser, at the middle of the width
    pub start: [f64; 3],
    /// the unit direction of the run in plan
    pub dir: [f64; 2],
    pub width: f64,
    pub risers: u32,
    pub riser_height: f64,
    pub tread_length: f64,
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
    /// firestop records of the wall penetrations (product, installed, inspected), entered by hand or by external software
    pub firestops: Firestops,
    pub slabs: Vec<SlabItem>,
    /// roofs by footprint, each written as an IfcRoof with a faceted solid
    pub roofs: Vec<RoofItem>,
    /// straight stairs, each an IfcStair with its side profile extruded across its width
    pub stairs: Vec<StairItem>,
    /// structural columns, each a straight prism on a convex footprint
    pub columns: Vec<ColumnItem>,
    /// beams, each a straight member with a rectangular section
    pub beams: Vec<BeamItem>,
    /// the structural material of beams and columns by element number; each name is an `IfcMaterial`
    pub materials: BTreeMap<u64, String>,
    pub shafts: Vec<ShaftVoid>,
    /// register of openings in slabs, by slab index into `slabs`
    pub floor_penetrations: Vec<FloorPenetration>,
    pub networks: Vec<Network>,
    pub families: Vec<FamilyRef>,
    pub items: Vec<FamilyItem>,
    pub rooms: Vec<RoomItem>,
    /// lighting inputs of rooms, by the room's element number
    pub room_lighting: std::collections::BTreeMap<u64, lighting::RoomLighting>,
    pub wall_types: Vec<walls::WallType>,
    /// types of beams, columns, slabs and footings: one IFC type object each, linked to its instances
    pub types: Vec<TypeItem>,
    /// what the exchange needs to know of each beam, column, slab and footing, by element number (see `MemberInfo`)
    pub member_info: BTreeMap<u64, MemberInfo>,
    /// the app's own records of the whole model (see `openbim_document::native`), carried in the project's `OpenBIM_Document` set so that the app can read its own file back with nothing lost; empty for none
    pub native: Vec<String>,
    /// where one wall element meets another (see `walls::find_joins`)
    pub wall_joins: Vec<walls::WallJoin>,
    /// where the project is on the earth; without it the file has no map and the site has no position
    pub georef: Option<georef::Georef>,
    /// how rooms are bounded and measured, for the whole project (Revit's Area and Volume Computations)
    pub room_settings: boundaries::RoomSettings,
    /// lines that bound rooms where there is no wall
    pub separation_lines: Vec<boundaries::SeparationLine>,
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
    BadGeoref(georef::GeorefError),
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
    pub roofs: usize,
    pub stairs: usize,
    pub bars: usize,
    pub segments: usize,
    pub fittings: usize,
    pub ports: usize,
    pub connections: usize,
    pub items: usize,
    pub types: usize,
    /// occurrences whose shape is a mapped item of their type's shared shape
    pub mapped: usize,
    pub spaces: usize,
    pub space_boundaries: usize,
    pub wall_types: usize,
    pub wall_joins: usize,
    /// 1 when the file carries a map conversion
    pub map_conversions: usize,
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

#[derive(Debug, Clone, PartialEq)]
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
    /// degrees; written in radians
    Angle(f64),
    Int(i64),
    Real(f64),
}

/// A value written as the kind a mapping row asks for; the reason when it cannot be.
fn convert(v: &Val, kind: PropKind) -> Result<Val, &'static str> {
    // what the value measures: none (a plain number), a length, an area, a volume or an angle
    #[derive(PartialEq)]
    enum Of { Plain, Length, Area, Volume, Angle }
    let (x, of) = match v {
        Val::Real(x) => (*x, Of::Plain),
        Val::Len(x) | Val::LenAny(x) => (*x, Of::Length),
        Val::Area(x) => (*x, Of::Area),
        Val::Vol(x) => (*x, Of::Volume),
        Val::Angle(x) => (*x, Of::Angle),
        Val::Int(i) => (*i as f64, Of::Plain),
        Val::Bool(b) => (f64::from(u8::from(*b)), Of::Plain),
        Val::Label(t) | Val::Text(t) | Val::Ident(t) => {
            return Ok(match kind {
                PropKind::Label => Val::Label(t.clone()),
                PropKind::Text => Val::Text(t.clone()),
                PropKind::Identifier => Val::Ident(t.clone()),
                _ => return Err("the value is text"),
            });
        }
    };
    let want = match kind {
        PropKind::Length | PropKind::PositiveLength => Of::Length,
        PropKind::Area => Of::Area,
        PropKind::Volume => Of::Volume,
        PropKind::Angle => Of::Angle,
        PropKind::Real | PropKind::Integer | PropKind::Boolean => Of::Plain,
        PropKind::Label | PropKind::Text | PropKind::Identifier => return Err("the value is a number"),
    };
    if of != want {
        return Err(match of {
            Of::Plain => "the value is a plain number, not a quantity with a unit",
            Of::Length => "the value is a length",
            Of::Area => "the value is an area",
            Of::Volume => "the value is a volume",
            Of::Angle => "the value is an angle",
        });
    }
    Ok(match kind {
        PropKind::Real => Val::Real(x),
        PropKind::Length => Val::LenAny(x),
        PropKind::PositiveLength if x > 0.0 => Val::Len(x),
        PropKind::PositiveLength => return Err("a positive length must be above 0"),
        PropKind::Area => Val::Area(x),
        PropKind::Volume => Val::Vol(x),
        PropKind::Angle => Val::Angle(x),
        PropKind::Integer if x.fract() == 0.0 && x.abs() < 1e15 => Val::Int(x as i64),
        PropKind::Integer => return Err("an integer must be a whole number"),
        PropKind::Boolean if x == 0.0 || x == 1.0 => Val::Bool(x == 1.0),
        PropKind::Boolean => return Err("a boolean must be 0 or 1"),
        PropKind::Label | PropKind::Text | PropKind::Identifier => unreachable!(),
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
    /// entity of each wall, slab and placed family, in the order of the scene
    wall_entities: Vec<u32>,
    slab_entities: Vec<u32>,
    column_entities: Vec<u32>,
    /// every beam, brace and column written: (element number, entity)
    member_entities: Vec<(u64, u32)>,
    /// the virtual element of each separation line that a boundary used
    virtual_entities: BTreeMap<usize, u32>,
    item_entities: Vec<u32>,
    /// writing IFC2X3 (for DIALux and other readers that stop at 2x3) instead of IFC4
    v2: bool,
}

impl<'a> Out<'a> {
    /// The attribute list tail of an element that has a predefined type in IFC4 only: `$` then the type.
    fn tail4(&self, t: &str) -> String {
        if self.v2 { "$".into() } else { format!("$,{t}") }
    }
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
    /// The exact profile of a section: a rectangle, a circle or an I-shape.
    fn section_profile(&mut self, p: &Profile) -> u32 {
        match *p {
            Profile::Rect { width, depth } => self.rect_profile(0.0, 0.0, width, depth),
            Profile::Circle { diameter } => self.circle_profile(diameter / 2.0),
            Profile::I { width, depth, web, flange } => {
                let c = self.p2(0.0, 0.0);
                let a = self.s.add(format!("IFCAXIS2PLACEMENT2D(#{c},$)"));
                let v = [width, depth, web, flange].map(|x| self.r(x, "size"));
                self.s.add(format!("IFCISHAPEPROFILEDEF(.AREA.,$,#{a},{},{},{},{},$,$,$)", v[0], v[1], v[2], v[3]))
            }
        }
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
                Val::Angle(x) => format!("IFCPLANEANGLEMEASURE({})", self.r(x.to_radians(), "property")),
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
            let ent = if self.v2 { format!("{class}({},$,$,{v})", string(q.name)) } else { format!("{class}({},$,$,{v},$)", string(q.name)) };
            ids.push(format!("#{}", self.s.add(ent)));
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
    export_as(scene, false, false)
}

/// Write the scene as IFC 4.3 (`IFC4X3_ADD2`), the newest schema. The entities written are the same as in IFC4; the
/// files of the tests are checked against both schemas with IfcOpenShell.
pub fn export_ifc4x3(scene: &Scene) -> Result<Exported, ExportError> {
    export_as(scene, false, true)
}

/// Write the scene as IFC2X3, the schema DIALux evo reads best: the building shell, rooms, doors and windows.
/// What 2x3 cannot carry (service networks, wall types, joins, space boundaries, map conversion, luminaires)
/// is left out and named in the report's warnings.
pub fn export_ifc2x3(scene: &Scene) -> Result<Exported, ExportError> {
    export_as(scene, true, false)
}

fn export_as(scene: &Scene, v2: bool, x43: bool) -> Result<Exported, ExportError> {
    if scene.project_key > guid::MAX_PROJECT_KEY {
        return Err(ExportError::BadKey);
    }
    if scene.levels.all().is_empty() {
        return Err(ExportError::NoLevels);
    }
    let mut o = Out { s: Step::default(), key: scene.project_key, scene, bad: None, aux: 0, used: Default::default(), dup: None, body_ctx: 0, storeys: Vec::new(), contained: BTreeMap::new(), report: Report::default(), opening_ids: Vec::new(), item_ports: BTreeMap::new(), wall_entities: Vec::new(), slab_entities: Vec::new(), column_entities: Vec::new(), member_entities: Vec::new(), virtual_entities: BTreeMap::new(), item_entities: Vec::new(), v2 };
    // context and units
    let wcs = o.axis([0.0; 3], None, None);
    if let Some(gr) = &scene.georef {
        gr.check().map_err(ExportError::BadGeoref)?;
    }
    let north = match &scene.georef {
        Some(gr) => {
            let (x, y) = gr.file_true_north();
            let (x, y) = (o.r(x, "true north"), o.r(y, "true north"));
            format!("#{}", o.s.add(format!("IFCDIRECTION(({x},{y}))")))
        }
        None => "$".to_string(),
    };
    let ctx = o.s.add(format!("IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-6,#{wcs},{north})"));
    o.body_ctx = o.s.add(format!("IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{ctx},$,.MODEL_VIEW.,$)"));
    let u_len = o.s.add("IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)".into());
    let u_area = o.s.add("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)".into());
    let u_vol = o.s.add("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)".into());
    let u_ang = o.s.add("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)".into());
    let units = o.s.add(format!("IFCUNITASSIGNMENT((#{u_len},#{u_area},#{u_vol},#{u_ang}))"));
    if let (false, Some(gr)) = (v2, &scene.georef) {
        let opt = |v: &Option<String>| v.as_ref().map_or("$".to_string(), |t| string(t));
        let crs = o.s.add(format!("IFCPROJECTEDCRS({},$,{},{},{},{},#{u_len})", string(&gr.crs.name), opt(&gr.crs.geodetic_datum), opt(&gr.crs.vertical_datum), opt(&gr.crs.map_projection), opt(&gr.crs.map_zone)));
        let (origin, (a, b)) = gr.map_conversion();
        let vals = [origin[0], origin[1], origin[2], a, b, gr.scale].map(|v| o.r(v, "map conversion"));
        o.s.add(format!("IFCMAPCONVERSION(#{ctx},#{crs},{})", vals.join(",")));
        o.report.map_conversions = 1;
    }
    // IFC2X3 insists on an owner history for every rooted entity; `Step::finish_2x3` points them all at this one
    let oh = if v2 {
        let org = o.s.add("IFCORGANIZATION($,'openbim',$,$,$)".into());
        let per = o.s.add("IFCPERSON($,'openbim',$,$,$,$,$,$)".into());
        let pao = o.s.add(format!("IFCPERSONANDORGANIZATION(#{per},#{org},$)"));
        let app = o.s.add(format!("IFCAPPLICATION(#{org},'0.1','openbim','openbim')"));
        o.s.add(format!("IFCOWNERHISTORY(#{pao},#{app},$,.ADDED.,$,$,$,0)"))
    } else {
        0
    };
    let g = o.gen_gid();
    let project = o.s.add(format!("IFCPROJECT({g},$,{},$,$,$,$,(#{ctx}),#{units})", string(&scene.project_name)));
    if !scene.native.is_empty() {
        let names: Vec<String> = (0..scene.native.len()).map(|i| format!("R{i:07}")).collect();
        let props: Vec<(&str, Val)> = names.iter().zip(&scene.native).map(|(n, l)| (n.as_str(), Val::Text(l.clone()))).collect();
        o.pset(project, "OpenBIM_Document", props);
    }
    // spatial structure
    // the site carries the whole model, so it is where the file's origin is moved to
    let site_pl = match &scene.georef {
        _ if v2 => o.place(None, [0.0; 3], None, None),
        Some(gr) if gr.coordinate_base == georef::CoordinateBase::SharedCoordinates => {
            let (a, b) = gr.x_axis();
            o.place(None, gr.origin_shared(), Some([0.0, 0.0, 1.0]), Some([a, b, 0.0]))
        }
        Some(gr) => {
            let b = gr.base().0;
            o.place(None, [0.0 - b[0], 0.0 - b[1], 0.0 - b[2]], None, None)
        }
        None => o.place(None, [0.0; 3], None, None),
    };
    let g = o.gen_gid();
    let (lat, lon, el) = match &scene.georef {
        Some(gr) => {
            let ang = |d: f64| format!("({})", georef::compound_angle(d).map(|v| v.to_string()).join(","));
            (ang(gr.latitude), ang(gr.longitude), o.r(gr.elevation, "site elevation"))
        }
        None => ("$".to_string(), "$".to_string(), "$".to_string()),
    };
    let site = o.s.add(format!("IFCSITE({g},$,'Site',$,$,#{site_pl},$,$,.ELEMENT.,{lat},{lon},{el},$,$)"));
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
    if v2 {
        families::write_openings_2x3(&mut o)?;
    } else {
        families::write_families(&mut o)?;
    }
    write_slabs(&mut o)?;
    write_roofs(&mut o)?;
    write_stairs(&mut o)?;
    write_columns(&mut o)?;
    write_beams(&mut o)?;
    write_member_materials(&mut o);
    write_spaces(&mut o)?;
    if v2 {
        if !scene.networks.is_empty() {
            o.report.warnings.push("IFC2X3: service networks are not written".into());
        }
    } else {
        write_networks(&mut o)?;
    }
    write_member_types(&mut o)?;

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
    let text = if v2 { o.s.finish_2x3(&scene.project_name, &scene.timestamp, oh) } else { o.s.finish(&scene.project_name, &scene.timestamp, x43) };
    Ok(Exported { text, report: o.report })
}

fn write_walls(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    let v2 = o.v2;
    // wall types: materials, layer sets and the type entities, before the walls that use them
    let mut materials: BTreeMap<&str, u32> = BTreeMap::new();
    let mut type_ids: Vec<(u32, u32)> = Vec::new(); // (type entity, layer set)
    if o.v2 && !scene.wall_types.is_empty() {
        o.report.warnings.push("IFC2X3: wall types and layer sets are not written (the walls keep their thickness)".into());
    }
    for (ti, t) in scene.wall_types.iter().enumerate().filter(|_| !v2) {
        t.check().map_err(|e| ExportError::Unsupported(format!("wall type {}: {e}", t.name)))?;
        // IFC counts the layers across the wall from the right of travel to the left; the type lists them from the exterior, on the left
        let (core_a, core_b) = t.core_range();
        let mut layers = Vec::new();
        for (li, l) in t.layers.iter().enumerate().rev() {
            let m = match materials.get(l.material.as_str()) {
                Some(m) => *m,
                None => {
                    let m = o.s.add(format!("IFCMATERIAL({})", string(&l.material)));
                    materials.insert(l.material.as_str(), m);
                    m
                }
            };
            let th = o.r(l.thickness, "layer thickness");
            let core = if (core_a..=core_b).contains(&li) { "'Core'" } else { "$" };
            layers.push(format!("#{}", o.s.add(format!("IFCMATERIALLAYER(#{m},{th},$,{},{core},{},{})", string(&l.material), string(l.function.name()), l.function.priority()))));
        }
        let ls = o.s.add(format!("IFCMATERIALLAYERSET(({}),{},$)", layers.join(","), string(&t.name)));
        let wraps: Vec<String> = t.layers.iter().enumerate().filter(|(_, l)| l.wraps).map(|(i, _)| (i + 1).to_string()).collect();
        let tps = o.pset_entity("OpenBIM_WallType", vec![("CoreFirstLayer", Val::Int(core_a as i64 + 1)), ("CoreLastLayer", Val::Int(core_b as i64 + 1)), ("WrappingLayers", Val::Text(wraps.join(","))), ("LayersRunFrom", Val::Label("exterior, on the left of travel".into()))]);
        let g = o.gid((0xFFFF_FFFDu64 << 32) | ti as u64, 0);
        let te = o.s.add(format!("IFCWALLTYPE({g},$,{},$,$,(#{tps}),$,$,$,.STANDARD.)", string(&t.name)));
        let g = o.gen_gid();
        o.s.add(format!("IFCRELASSOCIATESMATERIAL({g},$,$,$,(#{te}),#{ls})"));
        type_ids.push((te, ls));
        o.report.wall_types += 1;
    }
    let mut typed: Vec<Vec<u32>> = vec![Vec::new(); scene.wall_types.len()];
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
        let wall = o.s.add(format!("IFCWALL({g},$,{},$,$,#{pl},#{shape},{})", string(&w.name), o.tail4(".STANDARD.")));
        o.contain(st, wall);
        o.report.walls += 1;
        o.wall_entities.push(wall);
        o.member_entities.push((w.id, wall));
        if let Some(ti) = w.wall_type.filter(|_| !v2) {
            let (t, (_, ls)) = (scene.wall_types.get(ti).ok_or_else(|| ExportError::Unsupported(format!("wall {}: no wall type {ti}", w.name)))?, type_ids[ti]);
            if (t.thickness() - c.thickness).abs() > 1e-9 {
                return Err(ExportError::Unsupported(format!("wall {}: the layers of type {} add up to {} m but the wall is {} m thick", w.name, t.name, t.thickness(), c.thickness)));
            }
            // IFC counts the layers from the face on the right of travel, which is where the centre line is half the thickness from
            let off = o.r(-c.thickness / 2.0, "layer offset");
            let u = o.s.add(format!("IFCMATERIALLAYERSETUSAGE(#{ls},.AXIS2.,.POSITIVE.,{off},$)"));
            let g = o.gen_gid();
            o.s.add(format!("IFCRELASSOCIATESMATERIAL({g},$,$,$,(#{wall}),#{u})"));
            typed[ti].push(wall);
        }
        let credit: f64 = scene.wall_penetrations.iter().filter(|p| p.id.chain == wi && p.cut && p.shape == OpeningShape::Round).map(|p| p.width * p.height - p.area()).sum();
        match quantities::wall(c, credit) {
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
                let pen = scene.wall_penetrations.iter().find(|p| {
                    p.id.chain == wi && p.id.segment == si && p.cut && (p.offset - op.offset).abs() < 1e-9 && (p.width - op.width).abs() < 1e-9 && (p.sill - op.sill).abs() < 1e-9 && (p.height - op.height).abs() < 1e-9
                });
                // one round service gets a round hole, grouped services a rectangle
                let prof = if pen.is_some_and(|p| p.shape == OpeningShape::Round) { o.circle_profile(op.width / 2.0) } else { o.rect_profile(0.0, 0.0, op.width, op.height) };
                let solid = o.extrusion(prof, [0.0; 3], Some([0.0, 0.0, 1.0]), Some([1.0, 0.0, 0.0]), t + 2.0 * ov);
                // the profile plane sits at the opening: a placement of its own, relative to the wall
                let opl = o.place(Some(pl), loc, Some(right), Some(d));
                let shape = o.shape(&[solid]);
                let name = pen.map_or_else(|| format!("Opening {k}"), |p| p.id.to_string());
                let g = o.gid(w.id, k);
                let oe = o.s.add(format!("IFCOPENINGELEMENT({g},$,{},$,$,#{opl},#{shape},{})", string(&name), o.tail4(".OPENING.")));
                let g = o.gen_gid();
                o.s.add(format!("IFCRELVOIDSELEMENT({g},$,$,$,#{wall},#{oe})"));
                o.report.openings += 1;
                o.opening_ids.push((wi, si, [op.offset, op.width, op.sill, op.height], oe));
                if let Some(p) = pen {
                    let systems: Vec<&str> = p.systems.iter().map(|s| s.ifc_system()).collect();
                    let mut props = vec![("Register", Val::Label(p.id.to_string())), ("Shape", Val::Label(if p.shape == OpeningShape::Round { "ROUND" } else { "RECTANGULAR" }.to_string())), ("Systems", Val::Label(systems.join(","))), ("InFirePartition", Val::Bool(p.in_fire_partition()))];
                    if let Some(m) = p.fire_rating_min {
                        // the opening must hold the rating of the wall it is in
                        props.push(("FireRating", Val::Label(fire_label(m))));
                        props.push(("FireOpeningRating", Val::Label(fire_label(m))));
                    }
                    if let Some(f) = scene.firestops.get(&p.id) {
                        for (k, v) in [("FirestopProduct", &f.product), ("FirestopInstalled", &f.installed), ("FirestopInspected", &f.inspected)] {
                            if let Some(v) = v {
                                props.push((k, Val::Label(v.clone())));
                            }
                        }
                    }
                    o.pset(oe, "OpenBIM_Penetration", props);
                }
            }
        }
    }
    for (ti, ws) in typed.iter().enumerate() {
        if !ws.is_empty() {
            let g = o.gen_gid();
            o.s.add(format!("IFCRELDEFINESBYTYPE({g},$,$,$,({}),#{})", ws.iter().map(|w| format!("#{w}")).collect::<Vec<_>>().join(","), type_ids[ti].0));
        }
    }
    // joins between wall elements
    if o.v2 && !scene.wall_joins.is_empty() {
        o.report.warnings.push("IFC2X3: wall joins are not written".into());
    }
    for j in scene.wall_joins.iter().filter(|_| !v2) {
        let (Some(a), Some(b)) = (o.wall_entities.get(j.a).copied(), o.wall_entities.get(j.b).copied()) else {
            return Err(ExportError::Unsupported(format!("wall join {}-{}: no such wall", j.a, j.b)));
        };
        if j.a == j.b {
            return Err(ExportError::Unsupported(format!("wall join {}: a wall cannot be joined to itself", j.a)));
        }
        // where both walls have layers, each layer's priority (Revit's: structure first) says which runs through which
        let prios = |wi: usize| -> String {
            match scene.walls[wi].wall_type.and_then(|ti| scene.wall_types.get(ti)) {
                Some(t) => format!("({})", t.layers.iter().rev().map(|l| l.function.priority().to_string()).collect::<Vec<_>>().join(",")),
                None => "()".to_string(),
            }
        };
        let (pa, pb) = if scene.walls[j.a].wall_type.is_some() && scene.walls[j.b].wall_type.is_some() { (prios(j.a), prios(j.b)) } else { ("()".to_string(), "()".to_string()) };
        let g = o.gen_gid();
        o.s.add(format!("IFCRELCONNECTSPATHELEMENTS({g},$,$,$,$,#{a},#{b},{pa},{pb},{},{})", j.b_end.ifc(), j.a_end.ifc()));
        o.report.wall_joins += 1;
    }
    Ok(())
}

fn write_columns(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for c in &scene.columns {
        if c.footprint.len() < 3 || !c.footprint.iter().all(|p| p[0].is_finite() && p[1].is_finite()) || !(c.height.is_finite() && c.height > 0.0 && c.base_z.is_finite()) {
            return Err(ExportError::Unsupported(format!("column {}: needs a footprint of 3 or more points and a positive height", c.name)));
        }
        let st = o.storey_for(c.base_z);
        let elev = o.storeys[st].2;
        let pl = o.place(Some(o.storeys[st].1), [0.0; 3], None, None);
        let prof = o.poly_profile(&c.footprint);
        let solid = o.extrusion(prof, [0.0, 0.0, c.base_z - elev], None, None, c.height);
        let shape = o.shape(&[solid]);
        let g = o.gid(c.id, 0);
        let e = o.s.add(format!("IFCCOLUMN({g},$,{},$,$,#{pl},#{shape},{})", string(&c.name), o.tail4(".COLUMN.")));
        o.contain(st, e);
        o.column_entities.push(e);
        o.member_entities.push((c.id, e));
    }
    Ok(())
}

fn write_beams(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for b in &scene.beams {
        let Some((u, side, _)) = b.axes() else {
            return Err(ExportError::Unsupported(format!("beam {}: needs two different points that are not one above the other, and a section with positive sizes", b.name)));
        };
        let st = o.storey_for(b.a[2].min(b.b[2]));
        let elev = o.storeys[st].2;
        let at = [b.a[0], b.a[1], b.a[2] - elev];
        // the beam is placed at its start, its local z along the beam and its local x across it
        let pl = o.place(Some(o.storeys[st].1), at, Some(u), Some(side));
        let prof = o.section_profile(&b.profile);
        let solid = o.extrusion(prof, [0.0; 3], None, None, b.length());
        let shape = o.shape(&[solid]);
        let g = o.gid(b.id, 0);
        let (class, predefined) = b.usage.ifc();
        let e = o.s.add(format!("{class}({g},$,{},$,$,#{pl},#{shape},{})", string(&b.name), o.tail4(predefined)));
        o.contain(st, e);
        o.member_entities.push((b.id, e));
        for (k, bar) in b.bars.iter().enumerate() {
            if bar.path.len() < 2 || bar.diameter.is_nan() || bar.diameter <= 0.0 { return Err(ExportError::Unsupported(format!("bar {}: needs two points and a positive diameter", bar.name))); }
            let pl = o.place(Some(o.storeys[st].1), [0.0; 3], None, None);
            let pts: Vec<String> = bar.path.iter().map(|p| { let id = o.p3([p[0], p[1], p[2] - elev]); format!("#{id}") }).collect();
            let curve = o.s.add(format!("IFCPOLYLINE(({}))", pts.join(",")));
            let r = o.r(bar.diameter / 2.0, "size");
            // IFC2X3 wants the range of the sweep (a polyline counts one for each corner); IFC4 may leave it out
            let range = if o.v2 { format!("0.,{}.", bar.path.len() - 1) } else { "$,$".to_string() };
            let solid = o.s.add(format!("IFCSWEPTDISKSOLID(#{curve},{r},$,{range})"));
            let shape = o.shape(&[solid]);
            let g = o.gid(bar.id, k as u16);
            let d = o.r(bar.diameter, "size");
            let kind = if bar.shear { ".SHEAR." } else { ".MAIN." };
            // IFC2X3 requires the cross-section area and the role; IFC4 does not
            let area = if o.v2 { o.r(std::f64::consts::PI * bar.diameter * bar.diameter / 4.0, "size") } else { "$".into() };
            let rb = o.s.add(format!("IFCREINFORCINGBAR({g},$,{},$,$,#{pl},#{shape},$,$,{d},{area},$,{kind},$)", string(&bar.name)));
            o.contain(st, rb);
            o.report.bars += 1;
        }
    }
    Ok(())
}

/// One `IfcMaterial` for each name used, associated with the beams and columns that have it.
fn write_member_materials(o: &mut Out) {
    let scene = o.scene;
    let mut by_name: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (id, e) in o.member_entities.clone() {
        if let Some(name) = scene.materials.get(&id) {
            by_name.entry(name.as_str()).or_default().push(e);
        }
    }
    for (name, elems) in by_name {
        let m = o.s.add(format!("IFCMATERIAL({})", string(name)));
        let g = o.gen_gid();
        o.s.add(format!("IFCRELASSOCIATESMATERIAL({g},$,$,$,({}),#{m})", elems.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
    }
}

/// The type objects and the `OpenBIM_Element` set of every beam, column, slab and footing written.
fn write_member_types(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    let entity_of: BTreeMap<u64, u32> = o.member_entities.iter().copied().collect();
    for (id, e) in &entity_of {
        let Some(info) = scene.member_info.get(id) else { continue };
        o.pset(*e, "OpenBIM_Element", vec![
            ("ElementNumber", Val::Int(*id as i64)), ("Category", Val::Label(info.category.clone())),
            ("TypeName", Val::Label(info.type_name.clone())), ("Fingerprint", Val::Ident(info.fingerprint.clone())),
        ]);
    }
    if scene.types.is_empty() { return Ok(()); }
    if o.v2 {
        o.report.warnings.push("IFC2X3: member types are not written (the elements keep their values and the OpenBIM_Element set names the type)".into());
        return Ok(());
    }
    for t in &scene.types {
        let props: Vec<(&str, Val)> = t.props.iter().map(|(n, v)| (n.as_str(), match v {
            TypeProp::Text(s) => Val::Label(s.clone()),
            TypeProp::Len(x) => Val::LenAny(*x),
            TypeProp::Int(i) => Val::Int(*i),
        })).collect();
        let ps = o.pset_entity("OpenBIM_Type", props);
        let (class, predefined) = t.class.entity();
        let g = o.gid(t.id, 0);
        let ty = o.s.add(format!("{class}({g},$,{},$,$,(#{ps}),$,$,$,.{predefined}.)", string(&t.name)));
        let users: Vec<String> = t.instances.iter().filter_map(|i| entity_of.get(i)).map(|e| format!("#{e}")).collect();
        if !users.is_empty() {
            let g = o.gen_gid();
            o.s.add(format!("IFCRELDEFINESBYTYPE({g},$,$,$,({}),#{ty})", users.join(",")));
        }
        o.report.types += 1;
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
        if matches!(item.role, SlabRole::PadFooting | SlabRole::StripFooting) {
            let kind = if item.role == SlabRole::PadFooting { "PAD_FOOTING" } else { "STRIP_FOOTING" };
            let f = o.s.add(format!("IFCFOOTING({g},$,{},$,$,#{pl},#{shape},$,.{kind}.)", string(&s.name)));
            o.contain(st, f);
            // keeps the slab numbering that room boundaries use
            o.slab_entities.push(f);
            o.member_entities.push((item.id, f));
            o.report.slabs += 1;
            continue;
        }
        if matches!(item.role, SlabRole::FloorFinish | SlabRole::Ceiling) {
            let kind = if item.role == SlabRole::FloorFinish { "FLOORING" } else { "CEILING" };
            let cov = o.s.add(format!("IFCCOVERING({g},$,{},$,$,#{pl},#{shape},$,.{kind}.)", string(&s.name)));
            o.contain(st, cov);
            o.slab_entities.push(cov);
            o.member_entities.push((item.id, cov));
            o.report.slabs += 1;
            continue;
        }
        let slab_type = if item.role == SlabRole::Foundation { ".BASESLAB." } else { ".FLOOR." };
        let slab = o.s.add(format!("IFCSLAB({g},$,{},$,$,#{pl},#{shape},$,{slab_type})", string(&s.name)));
        o.contain(st, slab);
        o.slab_entities.push(slab);
        o.member_entities.push((item.id, slab));
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
                let oe = o.s.add(format!("IFCOPENINGELEMENT({g},$,{},$,$,#{opl},#{shape},{})", string(&format!("Shaft {}", sh.name)), o.tail4(".OPENING.")));
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
            let oe = o.s.add(format!("IFCOPENINGELEMENT({g},$,{},$,$,#{opl},#{shape},{})", string(&p.id.to_string()), o.tail4(".OPENING.")));
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

/// Roofs by footprint: an IfcRoof whose body is a closed faceted solid, since a roof of several planes is not an extrusion.
fn write_roofs(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for item in &scene.roofs {
        if item.faces.is_empty() || item.faces.iter().any(|f| f.len() < 3 || f.iter().any(|&i| i >= item.verts.len())) {
            return Err(ExportError::Unsupported(format!("roof {}: a solid without faces", item.name)));
        }
        let st = o.storey_for(item.eaves_z);
        let elev = o.storeys[st].2;
        let pl = o.place(Some(o.storeys[st].1), [0.0; 3], None, None);
        let pts: Vec<u32> = item.verts.iter().map(|v| o.p3([v[0], v[1], v[2] - elev])).collect();
        let mut faces = Vec::new();
        for f in &item.faces {
            let ring: Vec<String> = f.iter().map(|&i| format!("#{}", pts[i])).collect();
            let lp = o.s.add(format!("IFCPOLYLOOP(({}))", ring.join(",")));
            let bound = o.s.add(format!("IFCFACEOUTERBOUND(#{lp},.T.)"));
            faces.push(format!("#{}", o.s.add(format!("IFCFACE((#{bound}))"))));
        }
        let shell = o.s.add(format!("IFCCLOSEDSHELL(({}))", faces.join(",")));
        let brep = o.s.add(format!("IFCFACETEDBREP(#{shell})"));
        let rep = o.shape_rep("Brep", &[brep]);
        let shape = o.s.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"));
        let g = o.gid(item.id, 0);
        let roof = o.s.add(format!("IFCROOF({g},$,{},$,$,#{pl},#{shape},$,.{}.)", string(&item.name), item.shape));
        o.contain(st, roof);
        o.member_entities.push((item.id, roof));
        o.report.roofs += 1;
    }
    Ok(())
}

/// Stairs: an IfcStair whose body is the side profile of the steps extruded across the width, with the figures of
/// Pset_StairCommon.
fn write_stairs(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for s in &scene.stairs {
        if s.risers < 2 || !(s.riser_height > 0.0) || !(s.tread_length > 0.0) || !(s.width > 0.0) || (s.dir[0].hypot(s.dir[1]) - 1.0).abs() > 1e-6 {
            return Err(ExportError::Unsupported(format!("stair {}: a run with no steps or no direction", s.name)));
        }
        let st = o.storey_for(s.start[2]);
        let elev = o.storeys[st].2;
        let pl = o.place(Some(o.storeys[st].1), [0.0; 3], None, None);
        // the profile in (along, up), extruded to the right of the run
        let mut pts: Vec<[f64; 2]> = vec![[0.0, 0.0]];
        for i in 1..s.risers {
            pts.push([f64::from(i - 1) * s.tread_length, f64::from(i) * s.riser_height]);
            pts.push([f64::from(i) * s.tread_length, f64::from(i) * s.riser_height]);
        }
        pts.push([f64::from(s.risers - 1) * s.tread_length, 0.0]);
        let prof = o.poly_profile(&pts);
        let right = [s.dir[1], -s.dir[0], 0.0];
        let origin = [s.start[0] - right[0] * s.width / 2.0, s.start[1] - right[1] * s.width / 2.0, s.start[2] - elev];
        let solid = o.extrusion(prof, origin, Some(right), Some([s.dir[0], s.dir[1], 0.0]), s.width);
        let shape = o.shape(&[solid]);
        let g = o.gid(s.id, 0);
        let stair = o.s.add(format!("IFCSTAIR({g},$,{},$,$,#{pl},#{shape},$,.STRAIGHT_RUN_STAIR.)", string(&s.name)));
        o.contain(st, stair);
        o.member_entities.push((s.id, stair));
        o.pset(stair, "Pset_StairCommon", vec![
            ("NumberOfRiser", Val::Int(i64::from(s.risers))), ("NumberOfTreads", Val::Int(i64::from(s.risers - 1))),
            ("RiserHeight", Val::Len(s.riser_height)), ("TreadLength", Val::Len(s.tread_length)),
        ]);
        o.report.stairs += 1;
    }
    Ok(())
}

fn write_spaces(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for (ri, r) in scene.rooms.iter().enumerate() {
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
        // the room's volume lies between its floor and its ceiling, which are finishes where it has them
        let ext = boundaries::room_extent(scene, r);
        let elev = scene.levels.all().get(room.level).map_or(0.0, |l| l.elevation);
        let solid = o.extrusion(prof, [0.0, 0.0, ext.floor_z - elev], None, None, ext.ceiling_z - ext.floor_z);
        let shape = o.shape(&[solid]);
        let g = o.gid(r.id, 0);
        let sp = o.s.add(format!("IFCSPACE({g},$,{},$,$,#{pl},#{shape},$,.ELEMENT.,.INTERNAL.,$)", string(&room.name)));
        let g = o.gen_gid();
        o.s.add(format!("IFCRELAGGREGATES({g},$,$,$,#{},(#{sp}))", o.storeys[st].0));
        o.report.spaces += 1;
        let props = vec![("Usage", Val::Label(room.usage.clone())), ("Occupancy", Val::Real(room.occupancy))];
        o.pset_mapped(sp, "IfcSpace", "OpenBIM_Space", props, &room.name);
        let vols = boundaries::room_volumes(scene, r);
        let mut clear = room.clone();
        clear.height = ext.ceiling_z - ext.floor_z;
        o.qto(sp, "Qto_SpaceBaseQuantities", &quantities::space(&clear));
        o.pset(sp, "OpenBIM_SpaceVolumes", vec![("SlabToSlabVolume", Val::Vol(vols.slab_to_slab)), ("CeilingVoidVolume", Val::Vol(vols.ceiling_void)), ("FloorBuildUpVolume", Val::Vol(vols.floor_build_up))]);
        if let Some(rl) = scene.room_lighting.get(&r.id) {
            let n = scene.items.iter().filter(|it| scene.families.get(it.family).is_some_and(|f| f.family.lighting_for(it.type_name.as_deref()).is_some()) && lighting::room_at(scene, [it.frame.origin[0], it.frame.origin[1]], it.frame.origin[2]) == Some(ri)).count();
            let mut common = vec![("ArtificialLighting", Val::Bool(n > 0))];
            if let Some(v) = rl.required_lux {
                common.push(("Illuminance", Val::Real(v)));
            }
            o.pset(sp, "Pset_SpaceLightingRequirements", common);
            let mut own = Vec::new();
            for (n, v) in [("WorkPlaneHeight", rl.work_plane_height_m), ("BoundaryZone", rl.boundary_zone_m)] {
                if let Some(v) = v {
                    own.push((n, Val::LenAny(v)));
                }
            }
            for (n, v) in [("MaintenanceFactor", rl.maintenance_factor), ("MinimumUniformity", rl.min_uniformity), ("MaximumUGR", rl.max_ugr), ("MinimumCRI", rl.min_cri), ("ReflectanceCeiling", rl.reflectance_ceiling), ("ReflectanceWalls", rl.reflectance_walls), ("ReflectanceFloor", rl.reflectance_floor)] {
                if let Some(v) = v {
                    own.push((n, Val::Real(v)));
                }
            }
            o.pset(sp, "OpenBIM_RoomLighting", own);
        }
        if !o.v2 {
            write_boundaries(o, r, sp);
        }
    }
    Ok(())
}

/// The element a virtual boundary is related to (IFC4 wants one): an IfcVirtualElement for the separation line, made once.
fn virtual_element(o: &mut Out, line: usize) -> u32 {
    if !o.virtual_entities.contains_key(&line) {
        let sl = &o.scene.separation_lines[line];
        let (id, level) = (sl.id, sl.level);
        let g = o.gid(id, 0);
        let e = o.s.add(format!("IFCVIRTUALELEMENT({g},$,'Room separation line',$,$,$,$,$)"));
        let elev = o.scene.levels.all().get(level).map_or(0.0, |l| l.elevation);
        let st = o.storey_for(elev);
        o.contain(st, e);
        o.virtual_entities.insert(line, e);
    }
    o.virtual_entities[&line]
}

/// The second-level space boundaries of one room: for each bounding face its own surface in the room's frame.
fn write_boundaries(o: &mut Out, r: &RoomItem, space: u32) {
    let scene = o.scene;
    let (found, warnings) = boundaries::room_boundaries(scene, r);
    o.report.warnings.extend(warnings);
    let mut ents: Vec<u32> = Vec::new();
    for b in &found {
        let virt = matches!(b.target, boundaries::Target::Virtual(_));
        let element = match b.target {
            boundaries::Target::Wall(i) => o.wall_entities.get(i),
            boundaries::Target::Slab(i) => o.slab_entities.get(i),
            boundaries::Target::Column(i) => o.column_entities.get(i),
            boundaries::Target::Item(i) => o.item_entities.get(i),
            boundaries::Target::Virtual(i) => Some(&virtual_element(o, i)),
        };
        let Some(&element) = element else {
            o.report.warnings.push(format!("room {}: a boundary element was not exported, so its boundary was left out", r.room.name));
            ents.push(0);
            continue;
        };
        let sf = &b.surface;
        let pos = o.axis(sf.origin, Some(sf.axis), Some(sf.refdir));
        let plane = o.s.add(format!("IFCPLANE(#{pos})"));
        let outer = o.poly_ring(&sf.outer);
        let inner: Vec<String> = sf.inner.iter().map(|h| format!("#{}", o.poly_ring(h))).collect();
        let face = o.s.add(format!("IFCCURVEBOUNDEDPLANE(#{plane},#{outer},({}))", inner.join(",")));
        let geom = o.s.add(format!("IFCCONNECTIONSURFACEGEOMETRY(#{face},$)"));
        let parent = match b.parent.and_then(|p| ents.get(p)) {
            Some(&e) if e != 0 => format!("#{e}"),
            _ => "$".to_string(),
        };
        let g = o.gen_gid();
        let kind = if virt { "VIRTUAL" } else { "PHYSICAL" };
        let related = format!("#{element}");
        let rel = o.s.add(format!("IFCRELSPACEBOUNDARY2NDLEVEL({g},$,$,$,#{space},{related},#{geom},.{kind}.,.NOTDEFINED.,{parent},$)"));
        ents.push(rel);
        o.report.space_boundaries += 1;
    }
}

/// Port entities of one piece, in port order.
struct PieceOut {
    ports: Vec<u32>,
}

fn write_port(o: &mut Out, key: (u64, u16), name: &str, pl_rel: u32, loc: V3, z: V3, ds: (Domain, System, FlowDir)) -> u32 {
    let ((id, part), (domain, system, flow)) = (key, ds);
    let x = perp(z);
    let pl = o.place(Some(pl_rel), loc, Some(z), Some(x));
    let g = o.gid(id, part);
    o.report.ports += 1;
    o.s.add(format!("IFCDISTRIBUTIONPORT({g},$,{},$,$,#{pl},$,.{}.,.{}.,.{}.)", string(name), flow.ifc(), domain.ifc_port_type(), system.ifc_system()))
}

fn nest(o: &mut Out, parent: u32, ports: &[u32]) {
    if ports.is_empty() {
        return;
    }
    let g = o.gen_gid();
    o.s.add(format!("IFCRELNESTS({g},$,$,$,#{parent},({}))", ports.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
}

/// The flow direction of every port of the pieces of a network, from the way the medium goes through the equipment
/// connectors it is linked to: a run or fitting port that receives the flow is a SINK, one it passes on is a SOURCE.
/// The direction spreads from each equipment connector through the pieces it reaches (a piece receiving at one port
/// passes on at its others, and the port linked to a SINK is a SOURCE). A port the flow never reaches, or one reached
/// both ways (a loop, or two flows meeting), is left as SOURCEANDSINK: the file does not claim more than the network says.
fn piece_flow(net: &openbim_route::graph::Network) -> (Vec<Vec<FlowDir>>, Vec<Vec<FlowDir>>) {
    let mut segs: Vec<Vec<FlowDir>> = net.segments.iter().map(|_| vec![FlowDir::Both; 2]).collect();
    let mut fits: Vec<Vec<FlowDir>> = net.fittings.iter().map(|f| vec![FlowDir::Both; f.fitting.ports.len()]).collect();
    let mut partner: BTreeMap<PortRef, PortRef> = BTreeMap::new();
    for (a, b) in net.links() {
        partner.insert(*a, *b);
        partner.insert(*b, *a);
    }
    let opposite = |d: FlowDir| if d == FlowDir::In { FlowDir::Out } else { FlowDir::In };
    // every (port, direction) the flow can give a port, found from all the equipment at once: a port that gets both
    // directions is where two flows meet, whatever the order they are followed in
    let mut reached: std::collections::BTreeSet<(PortRef, bool)> = std::collections::BTreeSet::new();
    let mut work: Vec<(PortRef, FlowDir)> = Vec::new();
    for (a, b) in net.links() {
        for (e, q) in [(a, b), (b, a)] {
            if let NodeRef::Equipment(i) = e.node {
                if let (Some(eq), false) = (net.equipment.get(i), matches!(q.node, NodeRef::Equipment(_))) {
                    match eq.flow_dir {
                        FlowDir::Out => work.push((*q, FlowDir::In)),
                        FlowDir::In => work.push((*q, FlowDir::Out)),
                        FlowDir::Both => {}
                    }
                }
            }
        }
    }
    while let Some((p, d)) = work.pop() {
        if matches!(p.node, NodeRef::Equipment(_)) || !reached.insert((p, d == FlowDir::In)) {
            continue;
        }
        let n = match p.node {
            NodeRef::Segment(_) => 2,
            NodeRef::Fitting(i) => fits.get(i).map_or(0, Vec::len),
            NodeRef::Equipment(_) => 0,
        };
        if let Some(s) = partner.get(&p) {
            work.push((*s, opposite(d)));
        }
        for k in (0..n).filter(|&k| k != p.port) {
            work.push((PortRef { node: p.node, port: k }, opposite(d)));
        }
    }
    for &(p, is_in) in &reached {
        if reached.contains(&(p, !is_in)) {
            continue; // reached both ways: stays SOURCEANDSINK
        }
        let d = if is_in { FlowDir::In } else { FlowDir::Out };
        let slot = match p.node {
            NodeRef::Segment(i) => segs.get_mut(i).and_then(|v| v.get_mut(p.port)),
            NodeRef::Fitting(i) => fits.get_mut(i).and_then(|v| v.get_mut(p.port)),
            NodeRef::Equipment(_) => None,
        };
        if let Some(x) = slot {
            *x = d;
        }
    }
    (segs, fits)
}

fn write_networks(o: &mut Out) -> Result<(), ExportError> {
    let scene = o.scene;
    for (ni, net) in scene.networks.iter().enumerate() {
        let mut segs: Vec<PieceOut> = Vec::new();
        let mut fits: Vec<PieceOut> = Vec::new();
        let mut by_system: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
        let (seg_flow, fit_flow) = piece_flow(net);
        for (si, sg) in net.segments.iter().enumerate() {
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
            o.member_entities.push((sg.id, e));
            o.report.segments += 1;
            let mut props = size_props(r.size);
            props.push(("System", Val::Label(r.system.ifc_system().into())));
            o.pset(e, "OpenBIM_Mep", props);
            let p0 = write_port(o, (sg.id, 0x1000), "Start", pl, [0.0; 3], [0.0, 0.0, -1.0], (r.domain, r.system, seg_flow[si][0]));
            let p1 = write_port(o, (sg.id, 0x1001), "End", pl, [0.0, 0.0, len], [0.0, 0.0, 1.0], (r.domain, r.system, seg_flow[si][1]));
            nest(o, e, &[p0, p1]);
            by_system.entry(r.system.code()).or_default().push(e);
            segs.push(PieceOut { ports: vec![p0, p1] });
        }
        for (fi, nf) in net.fittings.iter().enumerate() {
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
                props.push(("Angle", Val::Angle(angle)));
            }
            props.push(("Legs", Val::Int(f.ports.len() as i64)));
            o.pset(e, "OpenBIM_Mep", props);
            let mut ports = Vec::new();
            for (k, &leg_flow) in fit_flow[fi].iter().enumerate() {
                let loc = [f.ports[k][0] * f.trim[k], f.ports[k][1] * f.trim[k], f.ports[k][2] * f.trim[k]];
                ports.push(write_port(o, (nf.id, 0x1000 + k as u16), &format!("Leg {}", k + 1), pl, loc, unit(f.ports[k]), (nf.domain, nf.system, leg_flow)));
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
            let p = write_port(o, (eq.element, part), &eq.connector, pl, loc, unit(eq.dir), (eq.domain, eq.system, eq.flow_dir));
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
            let grp = o.s.add(format!("IFCDISTRIBUTIONSYSTEM({g},$,{},$,$,$,.{}.)", string(&format!("{} {}", sys.name(), ni + 1)), sys.ifc_system()));
            let g = o.gen_gid();
            o.s.add(format!("IFCRELASSIGNSTOGROUP({g},$,$,$,({}),$,#{grp})", elems.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(flow: FlowDir, x: f64, dir: f64) -> openbim_route::graph::NetEquipment {
        use openbim_family::mep::{Domain, System};
        openbim_route::graph::NetEquipment { element: 1, connector: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: flow, pos: [x, 0.0, 0.0], dir: [dir, 0.0, 0.0], size: openbim_place::penetrations::RunSize::Round { diameter: 0.1 } }
    }
    fn pipe(net: &mut Network, id: u64) {
        use openbim_family::mep::{Domain, System};
        net.segments.push(openbim_route::graph::NetSegment { id, run: openbim_place::penetrations::Run::round("p", Domain::Pipe, System::ChilledWaterSupply, [id as f64, 0.0, 0.0], [id as f64 + 1.0, 0.0, 0.0], 0.1) });
    }
    fn port(node: NodeRef, port: usize) -> PortRef { PortRef { node, port } }

    #[test]
    fn flow_goes_from_the_equipment_through_the_pieces_and_meets_where_two_flows_meet() {
        use FlowDir::*;
        use NodeRef::*;
        // equipment that gives, two runs, equipment that takes: the flow runs along
        let mut n = Network::new(100);
        n.equipment.push(eq(Out, 1.0, 1.0));
        n.equipment.push(eq(In, 3.0, -1.0));
        pipe(&mut n, 1);
        pipe(&mut n, 2);
        n.link(port(Equipment(0), 0), port(Segment(0), 0)).unwrap();
        n.link(port(Segment(0), 1), port(Segment(1), 0)).unwrap();
        n.link(port(Segment(1), 1), port(Equipment(1), 0)).unwrap();
        let (s, f) = piece_flow(&n);
        assert_eq!(s, vec![vec![In, Out], vec![In, Out]]);
        assert!(f.is_empty());
        // the same chain seen only from the equipment that takes: the flow is known backwards
        let mut m = Network::new(100);
        m.equipment.push(eq(In, 3.0, -1.0));
        pipe(&mut m, 1);
        pipe(&mut m, 2);
        m.link(port(Segment(0), 1), port(Segment(1), 0)).unwrap();
        m.link(port(Segment(1), 1), port(Equipment(0), 0)).unwrap();
        assert_eq!(piece_flow(&m).0, vec![vec![In, Out], vec![In, Out]]);
        // equipment that works both ways says nothing; a run no equipment is linked to stays both ways
        let mut b = Network::new(100);
        b.equipment.push(eq(Both, 1.0, 1.0));
        pipe(&mut b, 1);
        pipe(&mut b, 2);
        b.link(port(Equipment(0), 0), port(Segment(0), 0)).unwrap();
        assert_eq!(piece_flow(&b).0, vec![vec![Both, Both], vec![Both, Both]]);
        // two sources feeding one run from both ends: the flows meet, whichever is followed first
        for flip in [false, true] {
            let mut c = Network::new(100);
            let (first, second) = (eq(Out, 1.0, 1.0), eq(Out, 2.0, -1.0));
            let (a, z) = if flip { (1, 0) } else { (0, 1) };
            if flip { c.equipment.extend([second, first]) } else { c.equipment.extend([first, second]) }
            pipe(&mut c, 1);
            c.link(port(Equipment(a), 0), port(Segment(0), 0)).unwrap();
            c.link(port(Equipment(z), 0), port(Segment(0), 1)).unwrap();
            assert_eq!(piece_flow(&c).0, vec![vec![Both, Both]], "flip {flip}");
        }
    }

    #[test]
    fn a_value_is_written_as_the_kind_it_measures() {
        use PropKind::*;
        assert_eq!(convert(&Val::LenAny(2.0), Length), Ok(Val::LenAny(2.0)));
        assert_eq!(convert(&Val::LenAny(2.0), PositiveLength), Ok(Val::Len(2.0)));
        assert!(convert(&Val::LenAny(0.0), PositiveLength).unwrap_err().contains("above 0"));
        assert!(convert(&Val::LenAny(-1.0), PositiveLength).is_err());
        assert_eq!(convert(&Val::Area(2.0), Area), Ok(Val::Area(2.0)));
        assert_eq!(convert(&Val::Vol(2.0), Volume), Ok(Val::Vol(2.0)));
        assert_eq!(convert(&Val::Angle(90.0), Angle), Ok(Val::Angle(90.0)));
        // a quantity only goes where its own kind goes
        for (v, k) in [(Val::LenAny(1.0), Area), (Val::LenAny(1.0), Angle), (Val::LenAny(1.0), Real), (Val::Area(1.0), Length), (Val::Vol(1.0), Area), (Val::Angle(1.0), Length), (Val::Angle(1.0), Real), (Val::LenAny(1.0), Integer), (Val::Angle(1.0), Boolean)] {
            assert!(convert(&v, k).is_err(), "{v:?} as {k:?}");
        }
        // a plain number is a real, a whole number or a yes/no, and nothing with a unit
        assert_eq!(convert(&Val::Real(2.5), Real), Ok(Val::Real(2.5)));
        assert_eq!(convert(&Val::Real(2.0), Integer), Ok(Val::Int(2)));
        assert!(convert(&Val::Real(2.5), Integer).unwrap_err().contains("whole"));
        assert_eq!(convert(&Val::Real(1.0), Boolean), Ok(Val::Bool(true)));
        assert_eq!(convert(&Val::Real(0.0), Boolean), Ok(Val::Bool(false)));
        assert!(convert(&Val::Real(2.0), Boolean).unwrap_err().contains("0 or 1"));
        for k in [Length, PositiveLength, Area, Volume, Angle] {
            assert!(convert(&Val::Real(1.0), k).is_err(), "{k:?}");
        }
        assert_eq!(convert(&Val::Int(3), Integer), Ok(Val::Int(3)));
        assert_eq!(convert(&Val::Bool(true), Boolean), Ok(Val::Bool(true)));
        // text goes to the three text kinds and nowhere else
        assert_eq!(convert(&Val::Label("a".into()), Identifier), Ok(Val::Ident("a".into())));
        assert_eq!(convert(&Val::Label("a".into()), Text), Ok(Val::Text("a".into())));
        assert_eq!(convert(&Val::Text("a".into()), Label), Ok(Val::Label("a".into())));
        assert!(convert(&Val::Label("a".into()), Real).is_err());
        assert!(convert(&Val::Real(1.0), Label).is_err());
    }
}
