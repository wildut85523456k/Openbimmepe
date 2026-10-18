//! Sprint 1 BIM document foundation.
//!
//! This is a minimal in-memory, typed, transaction-backed editing core.
//! It intentionally does NOT claim persistence, IFC import, a full dependency
//! engine, or central-model concurrency. Element IDs survive edits/undo.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
mod spatial;
mod edit;
mod rooms;
mod roof;
mod stair;
pub use stair::{StairSpec, StairFigures, risers_for, MAX_RISER_M as MAX_STAIR_RISER_M, TREAD_M as STAIR_TREAD_M, WIDTH_M as STAIR_WIDTH_M, mesh as stair_mesh, profile as stair_profile};
pub use roof::{is_convex as roof_is_convex, RoofFigures, mesh as roof_mesh, RoofSpec, DEFAULT_SLOPE as DEFAULT_ROOF_SLOPE};
pub use rooms::RoomShape;
pub mod native;
mod rebar;
mod types;
pub mod compute;
pub mod compute_elec;
pub mod mep;
pub mod elec;
mod warnings;
pub use rebar::BarPath;
pub use types::TypeValues;
pub use elec::{ElecCable, ElecKind, ElecNode};
pub use mep::{EquipLink, EndDemand, EndRole, MepData, MepEndpoint, MepFitting, MepNode, MepPort, MepTap};
pub use warnings::{Warning, WarningKind};
pub use edit::Xf;
pub use spatial::{line_meet, system_lines, BeamExtent, Geometry, Mesh};

/// Document-local, stable element identity. Future versions require
/// a project-scoped ID allocation strategy for collaborative editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Wall,
    /// An annotation: the label of the element it is on (`Geometry::Tag`). It is drawn in the plan of its element's level
    /// and is not part of the building, so it is not exported as a product.
    Tag,
    /// An annotation: an aligned dimension between two points of the plan of a level (`Geometry::Dimension`). Not part
    /// of the building, so it is not exported as a product.
    Dimension,
    /// A space: a point in a closed region of a level (`Geometry::Room`). Its floor is bounded by the faces of the walls
    /// and columns around it, worked out when asked for.
    Room,
    /// A straight stair (`Geometry::Linear` along its run from the foot of the first riser): `width_m` wide, climbing
    /// `height_m` from `elevation_m` above its level in `count` risers over `count - 1` treads.
    Stair,
    /// A straight structural framing member, a rectangular section along a line on a level: `width_m` across,
    /// `height_m` deep, `elevation_m` the height of its centre line above the level.
    Beam,
    /// A vertical structural member at a point, from its base level (plus `elevation_m`) up to its top level:
    /// `width_m` across and `height_m` deep (its section), with a `section` and a `material`.
    Column,
    /// A horizontal slab on a level: a polygon `outline` in plan, as thick as `width_m` and `height_m` (equal),
    /// its top `elevation_m` above the level.
    Slab,
    /// An isolated (pad) footing: `width_m` by `height_m` in plan, as thick as its pad geometry says, its top
    /// `elevation_m` above the level.
    Footing,
    /// A strip footing under a wall: `width_m` wide, `height_m` deep, along the wall it is hosted by; its top is
    /// `elevation_m` above the base of the wall.
    WallFooting,
    /// A set of parallel beams generated from an outline, a direction and a spacing: `width_m` by `height_m` is
    /// the section of its beams, `elevation_m` the height of their centre lines above the level. The beams are
    /// elements of their own, with this system as their `parent`.
    BeamSystem,
    /// A flat Warren truss along an axis on a level: `width_m` is the size of its members, `height_m` its depth,
    /// `elevation_m` the height of its bottom chord above the level and `spacing_m` the panel length it aims for.
    /// Its chords and diagonals are beams of their own, with the truss as their `parent`.
    Truss,
    /// Reinforcement of a rectangular beam (`Geometry::OnMember`): `count` bars along the bottom and two along the
    /// top, `width_m` thick, with stirrups `height_m` thick every `spacing_m`, all `elevation_m` of cover inside.
    Rebar,
    /// An instance of a family from the library (door, window, equipment...): `family` names it, `type_name` its
    /// type, `overrides` its instance values; `angle_deg` turns it, `elevation_m` raises it above its level.
    Family,
    /// A named type of `type_for`: the size, section, material, usage or bars that every instance pointing at it
    /// (through its `type_id`) shares. It has no place in the model and nothing of it is drawn.
    Type,
    Duct,
    Pipe,
    Equipment,
    Level,
    Grid,
}

impl Category {
    pub fn supports_slope(self) -> bool { matches!(self, Self::Beam | Self::Duct | Self::Pipe) }
}

/// Everything that places a beam: where, how big, what for and of what.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamSpec {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub width_m: f64,
    pub depth_m: f64,
    /// height of the centre line at the start above the level
    pub elevation_m: f64,
    pub section: Section,
    pub usage: Usage,
    pub material: Material,
    /// rise over run from start to end; a brace has one
    pub slope_ratio: f64,
}

impl BeamSpec {
    /// A level girder of rectangular section, material unspecified.
    pub fn level(start: [f64; 2], end: [f64; 2], width_m: f64, depth_m: f64, elevation_m: f64) -> BeamSpec {
        BeamSpec { start, end, width_m, depth_m, elevation_m, section: Section::Rectangle, usage: Usage::Girder, material: Material::Unspecified, slope_ratio: 0.0 }
    }
}

/// The size, section and material of a column to place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnSpec {
    pub width_m: f64,
    pub depth_m: f64,
    /// how far above its base level the foot of the column is
    pub base_offset_m: f64,
    pub section: Section,
    pub material: Material,
}

/// What a slab is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlabKind {
    #[default]
    Floor,
    /// a structural slab on the ground
    Foundation,
    /// a roof by footprint (Revit's Roof): the outline at the eaves, a slope, and which edges slope
    Roof,
    /// a ceiling (Revit's Ceiling): its `elevation_m` is the height of its underside above the level, and it is
    /// `height_m` thick above that
    Ceiling,
}

impl SlabKind {
    pub const ALL: [SlabKind; 4] = [SlabKind::Floor, SlabKind::Foundation, SlabKind::Roof, SlabKind::Ceiling];
    pub fn name(self) -> &'static str {
        match self { SlabKind::Floor => "Floor", SlabKind::Foundation => "Foundation", SlabKind::Roof => "Roof", SlabKind::Ceiling => "Ceiling" }
    }
}

/// A strip footing to lay under a wall.
#[derive(Clone, Debug, PartialEq)]
pub struct WallFootingSpec {
    pub width_m: f64,
    pub depth_m: f64,
    /// how far above the base of the wall the top of the footing is (negative: below)
    pub top_offset_m: f64,
    pub material: Material,
}

/// An isolated footing to lay.
#[derive(Clone, Debug, PartialEq)]
pub struct FootingSpec {
    pub width_m: f64,
    pub length_m: f64,
    pub thickness_m: f64,
    /// how far above its level the top of the pad is (negative: below)
    pub top_offset_m: f64,
    pub material: Material,
}

/// A beam system to lay: the beams run at `angle_deg` from the x axis, `spacing_m` apart, inside `outline`.
#[derive(Clone, Debug, PartialEq)]
pub struct BeamSystemSpec {
    pub outline: Vec<[f64; 2]>,
    pub spacing_m: f64,
    pub angle_deg: f64,
    pub width_m: f64,
    pub depth_m: f64,
    /// height of the centre lines above the level
    pub elevation_m: f64,
    pub section: Section,
    pub usage: Usage,
    pub material: Material,
}

/// One member of a truss: start, end, height of its centre line at the start, rise, and whether it is a diagonal.
pub type TrussMember = ([f64; 2], [f64; 2], f64, f64, bool);

/// A family instance to place.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilySpec {
    pub family: String,
    pub type_name: String,
    pub overrides: Vec<(String, f64)>,
    pub angle_deg: f64,
    /// height above the level (a free instance) or above the base of the wall (an instance in a wall)
    pub elevation_m: f64,
}

/// Reinforcement to put in a beam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RebarSpec {
    /// bars along the bottom; two more run along the top
    pub count: u32,
    pub bar_m: f64,
    pub stirrup_m: f64,
    pub cover_m: f64,
    pub stirrup_spacing_m: f64,
}

/// A flat Warren truss to build along `start`..`end` on a level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrussSpec {
    pub start: [f64; 2],
    pub end: [f64; 2],
    /// the panel length aimed for; the truss has the whole number of equal panels nearest to it
    pub spacing_m: f64,
    pub depth_m: f64,
    /// width and depth of every member
    pub member_m: f64,
    /// height of the bottom chord's centre line above the level
    pub elevation_m: f64,
    pub material: Material,
}

/// A slab to lay.
#[derive(Clone, Debug, PartialEq)]
pub struct SlabSpec {
    pub outline: Vec<[f64; 2]>,
    pub thickness_m: f64,
    /// how far above its level the top of the slab is
    pub top_offset_m: f64,
    pub kind: SlabKind,
    pub material: Material,
}

/// What a framing member is for (Revit's Structural Usage). A brace is a sloped member.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Usage {
    #[default]
    Girder,
    Joist,
    Purlin,
    Brace,
    Other,
}

impl Usage {
    pub const ALL: [Usage; 5] = [Usage::Girder, Usage::Joist, Usage::Purlin, Usage::Brace, Usage::Other];
    pub fn name(self) -> &'static str {
        match self { Usage::Girder => "Girder", Usage::Joist => "Joist", Usage::Purlin => "Purlin", Usage::Brace => "Brace", Usage::Other => "Other" }
    }
}

/// The structural material class of a member. Which material, with what strengths, is for the project to say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Material {
    #[default]
    Unspecified,
    Concrete,
    Steel,
    Timber,
}

impl Material {
    pub const ALL: [Material; 4] = [Material::Unspecified, Material::Concrete, Material::Steel, Material::Timber];
    pub fn name(self) -> &'static str {
        match self { Material::Unspecified => "Unspecified", Material::Concrete => "Concrete", Material::Steel => "Steel", Material::Timber => "Timber" }
    }
}

/// The section of a beam, with the sizes the element itself does not carry. The element's `width_m` and `height_m`
/// are the section's width across and depth; a circle is as wide as it is deep (its diameter).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Section {
    #[default]
    Rectangle,
    Circle,
    /// a symmetric I or H: the web and the flanges are this thick
    IShape { web_m: f64, flange_m: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: ElementId,
    pub category: Category,
    pub name: String,
    /// Length-like dimensions in canonical metres, never display millimetres.
    pub width_m: f64,
    pub height_m: f64,
    pub elevation_m: f64,
    /// Signed rise / horizontal run, along the element's future start-to-end
    /// direction. UI percent = ratio * 100. No geometry is generated here.
    /// Only ducts and pipes support this parameter.
    pub slope_ratio: Option<f64>,
    /// Optional spatial definition; parameter-only elements remain supported.
    pub geometry: Option<Geometry>,
    /// The section of a beam; every other element is a rectangle.
    pub section: Section,
    /// What a beam is for; every other element is a Girder.
    pub usage: Usage,
    /// The structural material of a beam.
    pub material: Material,
    /// The plan outline of a slab (counter-clockwise or clockwise); empty for everything else.
    pub outline: Vec<[f64; 2]>,
    pub slab_kind: SlabKind,
    /// A roof: whether each edge of the outline defines a slope (rises from the eaves); empty for everything else.
    pub slope_edges: Vec<bool>,
    /// Whether each end of a beam (start, end) is joined to the column it ends in: a joined end is cut back to the
    /// face of the column. All true for everything but a beam that has had a join disallowed.
    pub join_ends: [bool; 2],
    /// Beam system: the distance between its beams, and the direction of the beams (degrees from the x axis).
    pub spacing_m: f64,
    pub angle_deg: f64,
    /// The beam system a beam was generated by.
    pub parent: Option<ElementId>,
    /// Rebar: the number of bars along the bottom of the beam.
    pub count: u32,
    /// Family instance: which family and type, and the values set on this instance.
    pub family: String,
    pub type_name: String,
    pub overrides: Vec<(String, f64)>,
    /// Family instance: choices, texts and materials set on this instance, by input name.
    pub texts: Vec<(String, String)>,
    /// The type this element takes its size, section, material, usage or bars from.
    pub type_id: Option<ElementId>,
    /// A type element: the category of the elements it serves.
    pub type_for: Option<Category>,
    /// Document-wide revision at the last successful change.
    pub revision: u64,
}

impl Element {
    /// The section as a shape, for a beam.
    pub fn profile(&self) -> openbim_geom::profile::Profile {
        use openbim_geom::profile::Profile;
        match self.section {
            Section::Rectangle => Profile::Rect { width: self.width_m, depth: self.height_m },
            Section::Circle => Profile::Circle { diameter: self.width_m },
            Section::IShape { web_m, flange_m } => Profile::I { width: self.width_m, depth: self.height_m, web: web_m, flange: flange_m },
        }
    }
    /// A roof by footprint: a slab of kind Roof.
    pub fn is_roof(&self) -> bool { self.kind() == Category::Slab && self.slab_kind == SlabKind::Roof }
    /// The category whose rules apply to the values of this element: a type is held to those of what it is a type of.
    pub fn kind(&self) -> Category { if self.category == Category::Type { self.type_for.unwrap_or(Category::Type) } else { self.category } }
    /// Whether elements of `category` can have a type.
    pub fn can_have_type(category: Category) -> bool { matches!(category, Category::Beam | Category::Column | Category::WallFooting | Category::Slab | Category::Rebar | Category::Wall | Category::Footing) }
    /// A section that is as wide as it is deep: a pipe, or a round beam.
    pub fn is_round(&self) -> bool { self.category == Category::Pipe || (matches!(self.category, Category::Beam | Category::Column) && self.section == Section::Circle) }

    fn valid(&self) -> bool {
        let k = self.kind();
        (matches!(k, Category::Beam | Category::Column | Category::BeamSystem | Category::Truss) || self.section == Section::Rectangle)
            && (matches!(k, Category::Beam | Category::BeamSystem | Category::Truss) || self.usage == Usage::Girder)
            && (matches!(k, Category::Beam | Category::Column | Category::Slab | Category::Footing | Category::WallFooting | Category::BeamSystem | Category::Truss) || self.material == Material::Unspecified)
            && (matches!(k, Category::Slab | Category::BeamSystem | Category::Truss) || (self.outline.is_empty() && self.slab_kind == SlabKind::Floor))
            && (matches!(k, Category::Beam | Category::Wall) || self.join_ends == [true; 2])
            && (matches!(k, Category::BeamSystem | Category::Truss | Category::Rebar) || (self.spacing_m == 0.0 && (self.angle_deg == 0.0 || k == Category::Family)))
            && (k != Category::Truss || self.angle_deg == 0.0)
            && (k != Category::BeamSystem || (self.spacing_m.is_finite() && (0.0..=1000.0).contains(&self.spacing_m) && self.angle_deg.is_finite() && self.angle_deg.abs() <= 360.0))
            && (self.category == Category::Beam || self.parent.is_none())
            && (matches!(k, Category::Rebar | Category::Stair) || self.count == 0)
            && (self.category == Category::Family || (self.family.is_empty() && self.type_name.is_empty() && self.overrides.is_empty() && self.texts.is_empty()))
            && self.texts.len() <= 64 && self.texts.iter().all(|(n, v)| !n.is_empty() && n.len() <= 64 && v.len() <= 256)
            && self.family.len() <= 256 && self.type_name.len() <= 256 && self.overrides.len() <= 64
            && self.overrides.iter().all(|(n, v)| !n.is_empty() && n.len() <= 64 && v.is_finite())
            && (self.category != Category::Family || (self.angle_deg.is_finite() && self.angle_deg.abs() <= 360.0))
            && (k != Category::Slab || ((self.width_m - self.height_m).abs() <= 1e-9 && self.width_m <= 5.0))
            && (!matches!(k, Category::Beam | Category::Column | Category::BeamSystem | Category::Truss) || (self.section != Section::Circle || (self.width_m - self.height_m).abs() <= 1e-9) && self.profile().check().is_ok())
            && (self.category != Category::Type || self.type_for.is_some_and(Self::can_have_type))
            && (self.category == Category::Type || self.type_for.is_none())
            && (self.type_id.is_none() || (self.category != Category::Type && Self::can_have_type(self.category)))
            && !self.name.trim().is_empty()
            && self.name.len() <= 512
            && self.width_m.is_finite()
            && self.height_m.is_finite()
            && self.elevation_m.is_finite()
            && self.width_m > 0.0
            && self.height_m > 0.0
            && match self.slope_ratio {
                Some(slope) => (self.category.supports_slope() || self.is_roof()) && slope.is_finite() && (!self.is_roof() || (0.0..=5.0).contains(&slope)),
                None => !self.category.supports_slope() && !self.is_roof(),
            }
            && if self.is_roof() {
                self.slope_edges.len() == self.outline.len()
                    && (!self.slope_edges.contains(&true) || self.slope_ratio.unwrap_or(0.0) == 0.0 || roof::is_convex(&self.outline))
            } else { self.slope_edges.is_empty() }
    }
}

/// Where the project is on the earth, and the clock its times are read on. It is a property of the project, not of
/// a view. The angle has the sign of `openbim_ifc::georef::Georef`: positive when project north (the Y axis) is turned
/// counter-clockwise from true north. Files carry the direction of true north in the project's plan, which has no sign
/// to get wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// degrees, north positive
    pub latitude: f64,
    /// degrees, east positive
    pub longitude: f64,
    /// degrees, project north turned counter-clockwise from true north
    pub angle_to_true_north_deg: f64,
    /// hours ahead of UTC of the project's clock
    pub utc_offset_hours: f64,
}

impl Site {
    fn valid(&self) -> bool {
        self.latitude.is_finite() && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite() && (-180.0..=180.0).contains(&self.longitude)
            && self.angle_to_true_north_deg.is_finite() && (-360.0..=360.0).contains(&self.angle_to_true_north_deg)
            && self.utc_offset_hours.is_finite() && (-12.0..=14.0).contains(&self.utc_offset_hours)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Set where the project is (latitude, longitude, angle to true north, clock offset).
    SetSite(Site),
    /// An element as it was saved, with every value; for restoring a document from its records.
    Restore(Box<Element>),
    Create {
        id: ElementId,
        category: Category,
        name: String,
        width_m: f64,
        height_m: f64,
        elevation_m: f64,
    },
    Resize {
        id: ElementId,
        width_m: f64,
        height_m: f64,
    },
    MoveElevation {
        id: ElementId,
        elevation_m: f64,
    },
    SetSlope { id: ElementId, slope_ratio: f64 },
    /// Change the section of a beam or a column. A round section takes the width as its diameter and the depth with it.
    SetSection { id: ElementId, section: Section },
    /// What a beam is for.
    SetUsage { id: ElementId, usage: Usage },
    /// The plan outline of a slab.
    SetOutline { id: ElementId, outline: Vec<[f64; 2]> },
    /// Floor, foundation or roof slab. A roof starts with a 30 degree slope on every edge.
    SetSlabKind { id: ElementId, kind: SlabKind },
    /// The slope (rise over run) of a roof and which of the edges of its outline define it.
    SetRoof { id: ElementId, slope_ratio: f64, edges: Vec<bool> },
    /// Allow or disallow the join of a beam's start and end with the columns they end in.
    SetJoins { id: ElementId, start: bool, end: bool },
    /// The spacing and direction of a beam system.
    SetBeamSystem { id: ElementId, spacing_m: f64, angle_deg: f64 },
    /// The beam system a beam belongs to.
    SetParent { id: ElementId, parent: Option<ElementId> },
    /// The number of bars along the bottom of a rebar.
    SetCount { id: ElementId, count: u32 },
    /// The family, type and instance values of a family instance, and its turn in degrees.
    SetFamilyTexts { id: ElementId, texts: Vec<(String, String)> },
    SetFamily { id: ElementId, family: String, type_name: String, overrides: Vec<(String, f64)>, angle_deg: f64 },
    /// The structural material of a beam.
    SetMaterial { id: ElementId, material: Material },
    SetGeometry { id: ElementId, geometry: Geometry },
    /// The type an element takes its values from; None leaves it with the values it has.
    SetType { id: ElementId, type_id: Option<ElementId> },
    /// What a type element is a type of.
    SetTypeFor { id: ElementId, category: Category },
    /// The name of a type.
    Rename { id: ElementId, name: String },
    /// The fittings of the ducts and pipes and which ports are joined, as a whole.
    SetMep(Box<MepData>),
    Delete { id: ElementId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    InvalidElement,
    AlreadyExists(ElementId),
    NotFound(ElementId),
    RevisionConflict { expected: u64, actual: u64 },
    EmptyTransaction,
    RevisionOverflow,
    UnsupportedProperty(ElementId),
    InvalidGeometry(ElementId),
    InvalidSite,
    /// A type that elements still use cannot be deleted.
    TypeInUse(ElementId),
    /// fittings or links that do not make sense: a link to a port that is not there or is taken twice
    InvalidMep,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    elements: BTreeMap<ElementId, Element>,
    site: Option<Site>,
    mep: MepData,
    revision: u64,
    /// Undo checkpoints are local, bounded by the hosting application's policy.
    history: Vec<(BTreeMap<ElementId, Element>, Option<Site>, MepData)>,
}

impl Document {
    pub fn new() -> Self { Self::default() }
    pub fn revision(&self) -> u64 { self.revision }
    /// Where the project is, if it has been set.
    pub fn site(&self) -> Option<Site> { self.site }
    /// The fittings and the links between the ducts and pipes.
    pub fn mep(&self) -> &MepData { &self.mep }
    /// The next element number that no element has.
    pub fn next_id(&self) -> ElementId { ElementId(self.elements.keys().next_back().map_or(1, |k| k.0 + 1)) }
    /// Places a beam on `level` in one undoable transaction.
    pub fn place_beam(&mut self, level: ElementId, beam: &BeamSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Beam).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Beam, name: format!("{} {n}", if beam.usage == Usage::Brace { "Brace" } else { "Beam" }), width_m: beam.width_m, height_m: beam.depth_m, elevation_m: beam.elevation_m },
            Command::SetSection { id, section: beam.section },
            Command::SetUsage { id, usage: beam.usage },
            Command::SetMaterial { id, material: beam.material },
            Command::SetSlope { id, slope_ratio: beam.slope_ratio },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: beam.start, end: beam.end, level } },
        ])?;
        Ok(id)
    }
    /// Draws a straight wall on `level` from `start` to `end`: `thickness_m` thick, `height_m` high, its base
    /// `base_offset_m` above the level. A wall with no length is refused.
    pub fn place_wall(&mut self, level: ElementId, start: [f64; 2], end: [f64; 2], thickness_m: f64, height_m: f64, base_offset_m: f64) -> Result<ElementId, EditError> {
        self.place_wall_moving(level, start, end, thickness_m, height_m, base_offset_m, &[])
    }
    /// Like `place_wall`, and in the same undoable step moves the axes of other walls (`(wall, start, end)`): the
    /// walls of a chain meet at the corners of their offset lines, so drawing the next wall moves the end of the last.
    #[allow(clippy::too_many_arguments)]
    pub fn place_wall_moving(&mut self, level: ElementId, start: [f64; 2], end: [f64; 2], thickness_m: f64, height_m: f64, base_offset_m: f64, moves: &[(ElementId, [f64; 2], [f64; 2])]) -> Result<ElementId, EditError> {
        self.place_wall_typed(level, start, end, thickness_m, height_m, base_offset_m, moves, None)
    }
    /// Like `place_wall_moving`, with the wall type `type_id` when there is one: the wall then has its thickness and height.
    #[allow(clippy::too_many_arguments)]
    pub fn place_wall_typed(&mut self, level: ElementId, start: [f64; 2], end: [f64; 2], thickness_m: f64, height_m: f64, base_offset_m: f64, moves: &[(ElementId, [f64; 2], [f64; 2])], type_id: Option<ElementId>) -> Result<ElementId, EditError> {
        let (thickness_m, height_m) = match type_id {
            Some(t) => { let t = self.get(t).filter(|t| t.category == Category::Type && t.type_for == Some(Category::Wall)).ok_or(EditError::NotFound(t))?; (t.width_m, t.height_m) }
            None => (thickness_m, height_m),
        };
        let id = self.next_id();
        if (end[0] - start[0]).hypot(end[1] - start[1]) < 0.01 { return Err(EditError::InvalidGeometry(id)); }
        let n = self.elements.values().filter(|e| e.category == Category::Wall).count() + 1;
        let mut ops = vec![
            Command::Create { id, category: Category::Wall, name: format!("Wall {n}"), width_m: thickness_m, height_m, elevation_m: base_offset_m },
            Command::SetGeometry { id, geometry: Geometry::Linear { start, end, level } },
        ];
        if let Some(t) = type_id { ops.push(Command::SetType { id, type_id: Some(t) }); }
        for &(w, a, b) in moves {
            if (b[0] - a[0]).hypot(b[1] - a[1]) < 0.01 { return Err(EditError::InvalidGeometry(w)); }
            ops.push(Command::SetGeometry { id: w, geometry: Geometry::Linear { start: a, end: b, level } });
        }
        self.transact(self.revision, &ops)?;
        Ok(id)
    }
    /// Draws several walls on `level` in one undoable step (`(start, end)` each): the same thickness, height and
    /// base offset, and the wall type `type_id` when there is one (its thickness and height then rule).
    pub fn place_walls(&mut self, level: ElementId, lines: &[([f64; 2], [f64; 2])], thickness_m: f64, height_m: f64, base_offset_m: f64, type_id: Option<ElementId>) -> Result<Vec<ElementId>, EditError> {
        let (thickness_m, height_m) = match type_id {
            Some(t) => { let t = self.get(t).filter(|t| t.category == Category::Type && t.type_for == Some(Category::Wall)).ok_or(EditError::NotFound(t))?; (t.width_m, t.height_m) }
            None => (thickness_m, height_m),
        };
        let first = self.next_id().0;
        let mut n = self.elements.values().filter(|e| e.category == Category::Wall).count();
        let (mut ops, mut ids) = (Vec::new(), Vec::new());
        for (i, &(start, end)) in lines.iter().enumerate() {
            let id = ElementId(first + i as u64);
            if (end[0] - start[0]).hypot(end[1] - start[1]) < 0.01 { return Err(EditError::InvalidGeometry(id)); }
            n += 1;
            ops.push(Command::Create { id, category: Category::Wall, name: format!("Wall {n}"), width_m: thickness_m, height_m, elevation_m: base_offset_m });
            ops.push(Command::SetGeometry { id, geometry: Geometry::Linear { start, end, level } });
            if let Some(t) = type_id { ops.push(Command::SetType { id, type_id: Some(t) }); }
            ids.push(id);
        }
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        self.transact(self.revision, &ops)?;
        Ok(ids)
    }
    /// Turns a wall over: its start and end swap, so that its exterior (on the left of the direction) faces the other way.
    pub fn flip_wall_commands(&self, id: ElementId) -> Result<Vec<Command>, String> {
        let e = self.get(id).filter(|e| e.category == Category::Wall).ok_or("flip: select a wall")?;
        let Some(Geometry::Linear { start, end, level }) = e.geometry else { return Err("flip: select a wall".into()) };
        let length = (end[0] - start[0]).hypot(end[1] - start[1]);
        let mut ops = vec![Command::SetGeometry { id, geometry: Geometry::Linear { start: end, end: start, level } }];
        // what stands in the wall is measured from its start, so it keeps its place along the wall
        for d in self.elements.values() {
            if let Some(Geometry::InWall { host, along_m }) = d.geometry { if host == id { ops.push(Command::SetGeometry { id: d.id, geometry: Geometry::InWall { host, along_m: length - along_m } }); } }
        }
        Ok(ops)
    }
    /// The name the next grid gets, counting on from the last one drawn as Revit does: 3 gives 4, C gives D, Z gives
    /// AA; a name already in use is passed over.
    pub fn next_grid_name(&self) -> String {
        let grids: Vec<&Element> = self.elements.values().filter(|e| e.category == Category::Grid).collect();
        let used: std::collections::BTreeSet<&str> = grids.iter().map(|g| g.name.as_str()).collect();
        let mut name = grids.last().map_or("1".to_string(), |g| Self::count_on(&g.name));
        while used.contains(name.as_str()) { name = Self::count_on(&name); }
        name
    }
    fn count_on(name: &str) -> String {
        if let Ok(n) = name.parse::<u64>() { return (n + 1).to_string(); }
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase()) {
            let mut chars: Vec<u8> = name.bytes().collect();
            let mut i = chars.len();
            loop {
                if i == 0 { chars.insert(0, b'A'); break; }
                i -= 1;
                if chars[i] == b'Z' { chars[i] = b'A'; } else { chars[i] += 1; break; }
            }
            return String::from_utf8(chars).unwrap_or_default();
        }
        format!("{name}1")
    }
    /// Draws a grid line from `start` to `end`, named after the last one when `name` is empty.
    pub fn place_grid(&mut self, start: [f64; 2], end: [f64; 2], name: &str) -> Result<ElementId, EditError> {
        let id = self.next_id();
        if (end[0] - start[0]).hypot(end[1] - start[1]) < 0.01 { return Err(EditError::InvalidGeometry(id)); }
        let name = if name.trim().is_empty() { self.next_grid_name() } else { name.trim().to_string() };
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Grid, name, width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Grid { start, end } },
        ])?;
        Ok(id)
    }
    /// Adds a level at `elevation_m`; two levels cannot share an elevation.
    pub fn place_level(&mut self, name: &str, elevation_m: f64) -> Result<ElementId, EditError> {
        let id = self.next_id();
        if !elevation_m.is_finite() || self.elements.values().any(|e| e.geometry == Some(Geometry::Level) && (e.elevation_m - elevation_m).abs() < 1e-3) { return Err(EditError::InvalidGeometry(id)); }
        let name = if name.trim().is_empty() {
            let mut n = self.elements.values().filter(|e| e.geometry == Some(Geometry::Level)).count() + 1;
            while self.elements.values().any(|e| e.geometry == Some(Geometry::Level) && e.name == format!("Level {n}")) { n += 1; }
            format!("Level {n}")
        } else { name.trim().to_string() };
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Level, name, width_m: 1.0, height_m: 1.0, elevation_m },
            Command::SetGeometry { id, geometry: Geometry::Level },
        ])?;
        Ok(id)
    }
    /// The level an element stands on: its own, or its host's.
    pub fn level_of(&self, id: ElementId) -> Option<ElementId> {
        match self.get(id)?.geometry? {
            Geometry::Linear { level, .. } | Geometry::Point { level, .. } | Geometry::Placed { level, .. } | Geometry::Area { level } | Geometry::Pad { level, .. } | Geometry::Dimension { level, .. } | Geometry::Room { level, .. } => Some(level),
            Geometry::InWall { host, .. } | Geometry::OnMember { host } | Geometry::OnWall { host } | Geometry::Tag { host, .. } => self.level_of(host),
            Geometry::Level | Geometry::Grid { .. } => None,
        }
    }
    /// Draws an aligned dimension from `a` to `b` on `level`, its line through `at`.
    pub fn place_dimension(&mut self, level: ElementId, a: [f64; 2], b: [f64; 2], at: [f64; 2]) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Dimension).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Dimension, name: format!("Dimension {n}"), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Dimension { a, b, at, level } },
        ])?;
        Ok(id)
    }
    /// What a tag says: the type of its element when it has one (a door's type, a wall type), else its name.
    pub fn tag_text(&self, tag: ElementId) -> String {
        let Some(Geometry::Tag { host, .. }) = self.get(tag).and_then(|t| t.geometry) else { return String::new() };
        let Some(h) = self.get(host) else { return "?".into() };
        match h.type_id.and_then(|t| self.get(t)) {
            Some(t) => t.name.clone(),
            None if h.category == Category::Family && !h.type_name.is_empty() => h.type_name.clone(),
            None if h.category == Category::Family => h.family.clone(),
            None => h.name.clone(),
        }
    }
    /// Tags `host`: the label at `at`, with a leader to `anchor` (the point of the element that was clicked).
    pub fn place_tag(&mut self, host: ElementId, at: [f64; 2], anchor: [f64; 2]) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let ok = self.get(host).is_some_and(|h| !matches!(h.category, Category::Tag | Category::Level | Category::Grid | Category::Type));
        if !ok { return Err(EditError::NotFound(host)); }
        let n = self.elements.values().filter(|e| e.category == Category::Tag).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Tag, name: format!("Tag {n}"), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Tag { host, at, anchor } },
        ])?;
        Ok(id)
    }
    /// Stands a column at `at` from `level` (raised by `base_offset_m`) up to the level `top`.
    pub fn place_column(&mut self, level: ElementId, top: ElementId, at: [f64; 2], col: &ColumnSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Column).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Column, name: format!("Column {n}"), width_m: col.width_m, height_m: col.depth_m, elevation_m: col.base_offset_m },
            Command::SetSection { id, section: col.section },
            Command::SetMaterial { id, material: col.material },
            Command::SetGeometry { id, geometry: Geometry::Point { at, level, top } },
        ])?;
        Ok(id)
    }
    /// Lays a slab on `level`: `outline` in plan, `thickness_m` thick with its top `top_offset_m` above the level.
    pub fn place_slab(&mut self, level: ElementId, slab: &SlabSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Slab).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Slab, name: format!("{} {n}", slab.kind.name()), width_m: slab.thickness_m, height_m: slab.thickness_m, elevation_m: slab.top_offset_m },
            Command::SetSlabKind { id, kind: slab.kind },
            Command::SetMaterial { id, material: slab.material },
            Command::SetOutline { id, outline: slab.outline.clone() },
            Command::SetGeometry { id, geometry: Geometry::Area { level } },
        ])?;
        Ok(id)
    }
    /// Hangs a ceiling on `level`: `outline` in plan, `height_m` above the level to its underside, `thickness_m` thick.
    pub fn place_ceiling(&mut self, level: ElementId, outline: Vec<[f64; 2]>, height_m: f64, thickness_m: f64, material: Material) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Slab && e.slab_kind == SlabKind::Ceiling).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Slab, name: format!("Ceiling {n}"), width_m: thickness_m, height_m: thickness_m, elevation_m: height_m },
            Command::SetSlabKind { id, kind: SlabKind::Ceiling },
            Command::SetMaterial { id, material },
            Command::SetOutline { id, outline },
            Command::SetGeometry { id, geometry: Geometry::Area { level } },
        ])?;
        Ok(id)
    }
    /// Lays a roof by footprint on `level`: see `RoofSpec`.
    pub fn place_roof(&mut self, level: ElementId, roof: &RoofSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Slab && e.slab_kind == SlabKind::Roof).count() + 1;
        let edges = if roof.edges.is_empty() { vec![true; roof.outline.len()] } else { roof.edges.clone() };
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Slab, name: format!("Roof {n}"), width_m: roof.thickness_m, height_m: roof.thickness_m, elevation_m: roof.base_offset_m },
            Command::SetSlabKind { id, kind: SlabKind::Roof },
            Command::SetMaterial { id, material: roof.material },
            Command::SetOutline { id, outline: roof.outline.clone() },
            Command::SetRoof { id, slope_ratio: roof.slope_ratio, edges },
            Command::SetGeometry { id, geometry: Geometry::Area { level } },
        ])?;
        Ok(id)
    }
    /// Lays an isolated footing on `level` under the point `at`.
    pub fn place_footing(&mut self, level: ElementId, at: [f64; 2], f: &FootingSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Footing).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Footing, name: format!("Footing {n}"), width_m: f.width_m, height_m: f.length_m, elevation_m: f.top_offset_m },
            Command::SetMaterial { id, material: f.material },
            Command::SetGeometry { id, geometry: Geometry::Pad { at, level, thickness_m: f.thickness_m } },
        ])?;
        Ok(id)
    }
    /// Lays a strip footing under the wall `host`: it follows the wall wherever it goes.
    pub fn place_wall_footing(&mut self, host: ElementId, f: &WallFootingSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::WallFooting).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::WallFooting, name: format!("Wall Footing {n}"), width_m: f.width_m, height_m: f.depth_m, elevation_m: f.top_offset_m },
            Command::SetMaterial { id, material: f.material },
            Command::SetGeometry { id, geometry: Geometry::OnWall { host } },
        ])?;
        Ok(id)
    }
    /// The commands that create the beams of the system `system` on `level` (the numbers from `first`).
    fn system_beam_commands(&self, system: &str, id: ElementId, level: ElementId, spec: &BeamSystemSpec, first: u64) -> Result<Vec<Command>, EditError> {
        let lines = spatial::system_lines(&spec.outline, spec.angle_deg, spec.spacing_m);
        if lines.len() > 2000 { return Err(EditError::InvalidElement); }
        let mut ops = Vec::new();
        for (i, (a, b)) in lines.into_iter().enumerate() {
            let bid = ElementId(first + i as u64);
            ops.extend([
                Command::Create { id: bid, category: Category::Beam, name: format!("{system} Beam {}", i + 1), width_m: spec.width_m, height_m: spec.depth_m, elevation_m: spec.elevation_m },
                Command::SetSection { id: bid, section: spec.section },
                Command::SetUsage { id: bid, usage: spec.usage },
                Command::SetMaterial { id: bid, material: spec.material },
                Command::SetSlope { id: bid, slope_ratio: 0.0 },
                Command::SetGeometry { id: bid, geometry: Geometry::Linear { start: a, end: b, level } },
                Command::SetParent { id: bid, parent: Some(id) },
            ]);
        }
        let _ = level;
        Ok(ops)
    }

    /// The members of a Warren truss as (start, end, centre-line height at the start, rise, brace?) along its axis.
    pub fn truss_members(spec: &TrussSpec) -> Vec<TrussMember> {
        let (dx, dy) = (spec.end[0] - spec.start[0], spec.end[1] - spec.start[1]);
        let len = dx.hypot(dy);
        if len < 1e-6 || spec.spacing_m.is_nan() || spec.spacing_m <= 0.0 { return Vec::new(); }
        let n = ((len / spec.spacing_m).round() as usize).clamp(1, 200);
        let p = len / n as f64;
        let at = |s: f64| [spec.start[0] + dx * s / len, spec.start[1] + dy * s / len];
        let d = spec.depth_m;
        let mut out = vec![(at(0.0), at(len), 0.0, 0.0, false)];
        if n > 1 { out.push((at(p / 2.0), at(len - p / 2.0), d, 0.0, false)); }
        for i in 0..n {
            let x = i as f64 * p;
            out.push((at(x), at(x + p / 2.0), 0.0, d, true));
            out.push((at(x + p / 2.0), at(x + p), d, -d, true));
        }
        out
    }

    fn truss_commands(&self, name: &str, id: ElementId, level: ElementId, spec: &TrussSpec, first: u64) -> Vec<Command> {
        let mut ops = Vec::new();
        for (i, (a, b, z, rise, brace)) in Self::truss_members(spec).into_iter().enumerate() {
            let bid = ElementId(first + i as u64);
            let run = (b[0] - a[0]).hypot(b[1] - a[1]);
            let part = if brace { "Diagonal" } else if z == 0.0 { "Bottom Chord" } else { "Top Chord" };
            ops.extend([
                Command::Create { id: bid, category: Category::Beam, name: format!("{name} {part} {}", i + 1), width_m: spec.member_m, height_m: spec.member_m, elevation_m: spec.elevation_m + z },
                Command::SetUsage { id: bid, usage: if brace { Usage::Brace } else { Usage::Girder } },
                Command::SetMaterial { id: bid, material: spec.material },
                Command::SetSlope { id: bid, slope_ratio: rise / run },
                Command::SetGeometry { id: bid, geometry: Geometry::Linear { start: a, end: b, level } },
                Command::SetParent { id: bid, parent: Some(id) },
            ]);
        }
        let _ = id;
        ops
    }

    /// Builds a truss on `level`: the truss and its members, in one undoable transaction.
    pub fn place_truss(&mut self, level: ElementId, spec: &TrussSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Truss).count() + 1;
        let name = format!("Truss {n}");
        let mut ops = vec![
            Command::Create { id, category: Category::Truss, name: name.clone(), width_m: spec.member_m, height_m: spec.depth_m, elevation_m: spec.elevation_m },
            Command::SetMaterial { id, material: spec.material },
            Command::SetBeamSystem { id, spacing_m: spec.spacing_m, angle_deg: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: spec.start, end: spec.end, level } },
        ];
        ops.extend(self.truss_commands(&name, id, level, spec, id.0 + 1));
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// Changes a truss and makes its members again, in one transaction.
    pub fn edit_truss(&mut self, id: ElementId, spec: &TrussSpec) -> Result<(), EditError> {
        let t = self.elements.get(&id).filter(|e| e.category == Category::Truss).ok_or(EditError::NotFound(id))?;
        let Some(Geometry::Linear { level, .. }) = t.geometry else { return Err(EditError::InvalidElement) };
        let name = t.name.clone();
        let first = self.next_id().0;
        let mut ops = vec![
            Command::Resize { id, width_m: spec.member_m, height_m: spec.depth_m },
            Command::MoveElevation { id, elevation_m: spec.elevation_m },
            Command::SetMaterial { id, material: spec.material },
            Command::SetBeamSystem { id, spacing_m: spec.spacing_m, angle_deg: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: spec.start, end: spec.end, level } },
        ];
        ops.extend(self.system_beams(id).into_iter().map(|b| Command::Delete { id: b }));
        ops.extend(self.truss_commands(&name, id, level, spec, first));
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    fn family_commands(&self, id: ElementId, spec: &FamilySpec, geometry: Geometry) -> Vec<Command> {
        let n = self.elements.values().filter(|e| e.category == Category::Family).count() + 1;
        vec![
            Command::Create { id, category: Category::Family, name: format!("{} {n}", spec.family), width_m: 1.0, height_m: 1.0, elevation_m: spec.elevation_m },
            Command::SetFamily { id, family: spec.family.clone(), type_name: spec.type_name.clone(), overrides: spec.overrides.clone(), angle_deg: spec.angle_deg },
            Command::SetGeometry { id, geometry },
        ]
    }

    /// Places a family instance at `at` on `level`.
    pub fn place_family(&mut self, level: ElementId, at: [f64; 2], spec: &FamilySpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let ops = self.family_commands(id, spec, Geometry::Placed { at, level });
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// Places a family instance in the wall `host`, `along_m` from its start. It follows the wall.
    pub fn place_family_in_wall(&mut self, host: ElementId, along_m: f64, spec: &FamilySpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let ops = self.family_commands(id, spec, Geometry::InWall { host, along_m });
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// Reinforces `beam` (a rectangular one), in one undoable transaction.
    pub fn place_rebar(&mut self, beam: ElementId, spec: &RebarSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::Rebar).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Rebar, name: format!("Rebar {n}"), width_m: spec.bar_m, height_m: spec.stirrup_m, elevation_m: spec.cover_m },
            Command::SetBeamSystem { id, spacing_m: spec.stirrup_spacing_m, angle_deg: 0.0 },
            Command::SetCount { id, count: spec.count },
            Command::SetGeometry { id, geometry: Geometry::OnMember { host: beam } },
        ])?;
        if !self.elements.get(&id).is_some_and(|e| self.rebar_fits(e)) {
            let _ = self.undo();
            return Err(EditError::InvalidGeometry(id));
        }
        Ok(id)
    }

    /// Lays a beam system on `level`: the system and its beams, in one undoable transaction.
    pub fn place_beam_system(&mut self, level: ElementId, spec: &BeamSystemSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| e.category == Category::BeamSystem).count() + 1;
        let name = format!("Beam System {n}");
        let mut ops = vec![
            Command::Create { id, category: Category::BeamSystem, name: name.clone(), width_m: spec.width_m, height_m: spec.depth_m, elevation_m: spec.elevation_m },
            Command::SetSection { id, section: spec.section },
            Command::SetUsage { id, usage: spec.usage },
            Command::SetMaterial { id, material: spec.material },
            Command::SetBeamSystem { id, spacing_m: spec.spacing_m, angle_deg: spec.angle_deg },
            Command::SetOutline { id, outline: spec.outline.clone() },
            Command::SetGeometry { id, geometry: Geometry::Area { level } },
        ];
        ops.extend(self.system_beam_commands(&name, id, level, spec, id.0 + 1)?);
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// The beams of a beam system, in the order they were generated.
    pub fn system_beams(&self, system: ElementId) -> Vec<ElementId> {
        self.elements.values().filter(|e| e.parent == Some(system)).map(|e| e.id).collect()
    }

    /// Changes a beam system and makes its beams again, in one transaction. The beams are replaced: edits made to
    /// them one by one are lost.
    pub fn edit_beam_system(&mut self, id: ElementId, spec: &BeamSystemSpec) -> Result<(), EditError> {
        let sys = self.elements.get(&id).filter(|e| e.category == Category::BeamSystem).ok_or(EditError::NotFound(id))?;
        let Some(Geometry::Area { level }) = sys.geometry else { return Err(EditError::InvalidElement) };
        let name = sys.name.clone();
        let first = self.next_id().0;
        let mut ops = vec![
            Command::Resize { id, width_m: spec.width_m, height_m: spec.depth_m },
            Command::MoveElevation { id, elevation_m: spec.elevation_m },
            Command::SetSection { id, section: spec.section },
            Command::SetUsage { id, usage: spec.usage },
            Command::SetMaterial { id, material: spec.material },
            Command::SetBeamSystem { id, spacing_m: spec.spacing_m, angle_deg: spec.angle_deg },
            Command::SetOutline { id, outline: spec.outline.clone() },
        ];
        ops.extend(self.system_beams(id).into_iter().map(|b| Command::Delete { id: b }));
        ops.extend(self.system_beam_commands(&name, id, level, spec, first)?);
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Deletes a beam system with its beams, in one transaction.
    pub fn delete_beam_system(&mut self, id: ElementId) -> Result<(), EditError> {
        if self.elements.get(&id).is_none_or(|e| !matches!(e.category, Category::BeamSystem | Category::Truss)) { return Err(EditError::NotFound(id)); }
        let mut ops: Vec<Command> = self.system_beams(id).into_iter().map(|b| Command::Delete { id: b }).collect();
        ops.push(Command::Delete { id });
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> { self.elements.get(&id) }
    pub fn elements(&self) -> impl Iterator<Item = &Element> { self.elements.values() }
    pub fn can_undo(&self) -> bool { !self.history.is_empty() }
    /// Establish a loaded/new sample as the undo baseline, without changing its revision.
    pub fn clear_undo_history(&mut self) { self.history.clear(); }

    /// Apply a batch atomically. A failed batch never updates state or history.
    /// The expected document revision protects against stale UI commands;
    /// centralized edits will need additional per-element semantic checks.
    pub fn transact(&mut self, expected: u64, ops: &[Command]) -> Result<u64, EditError> {
        if ops.is_empty() { return Err(EditError::EmptyTransaction); }
        if self.revision != expected {
            return Err(EditError::RevisionConflict { expected, actual: self.revision });
        }
        let revision = self.revision.checked_add(1).ok_or(EditError::RevisionOverflow)?;
        let mut trial = self.elements.clone();
        let mut trial_site = self.site;
        let mut trial_mep = self.mep.clone();
        for op in ops {
            match op {
                Command::Create { id, category, name, width_m, height_m, elevation_m } => {
                    if trial.contains_key(id) { return Err(EditError::AlreadyExists(*id)); }
                    let element = Element {
                        id: *id, category: *category, name: name.clone(),
                        width_m: *width_m, height_m: *height_m,
                        elevation_m: *elevation_m, revision,
                        slope_ratio: category.supports_slope().then_some(0.0),
                        geometry: None, section: Section::Rectangle, usage: Usage::Girder, material: Material::Unspecified,
                        outline: Vec::new(), slab_kind: SlabKind::Floor, slope_edges: Vec::new(), join_ends: [true; 2], spacing_m: 0.0, angle_deg: 0.0, parent: None, count: 0, family: String::new(), type_name: String::new(), overrides: Vec::new(), texts: Vec::new(), type_id: None, type_for: None,
                    };
                    // a type is made in steps: what it is a type of is set by the next command
                    if *category != Category::Type && !element.valid() { return Err(EditError::InvalidElement); }
                    trial.insert(*id, element);
                }
                Command::Resize { id, width_m, height_m } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if matches!(el.category, Category::Level | Category::Grid) {
                        return Err(EditError::UnsupportedProperty(*id));
                    }
                    el.width_m = *width_m;
                    el.height_m = *height_m;
                    // a beam is checked whole at the end, so that a size and a section can change together
                    if !matches!(el.kind(), Category::Beam | Category::Column | Category::BeamSystem | Category::Truss | Category::Type) && !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::MoveElevation { id, elevation_m } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category == Category::Grid { return Err(EditError::UnsupportedProperty(*id)); }
                    el.elevation_m = *elevation_m;
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::SetSlope { id, slope_ratio } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !el.category.supports_slope() { return Err(EditError::UnsupportedProperty(*id)); }
                    el.slope_ratio = Some(*slope_ratio);
                    if !el.valid() { return Err(EditError::InvalidElement); }
                    el.revision = revision;
                }
                Command::SetUsage { id, usage } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.kind(), Category::Beam | Category::BeamSystem | Category::Truss) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.usage = *usage;
                    el.revision = revision;
                }
                Command::SetOutline { id, outline } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.category, Category::Slab | Category::BeamSystem | Category::Truss) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.outline = outline.clone();
                    if el.slab_kind == SlabKind::Roof && el.slope_edges.len() != el.outline.len() { el.slope_edges = vec![true; el.outline.len()]; }
                    el.revision = revision;
                }
                Command::SetBeamSystem { id, spacing_m, angle_deg } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.kind(), Category::BeamSystem | Category::Truss | Category::Rebar) { return Err(EditError::UnsupportedProperty(*id)); }
                    (el.spacing_m, el.angle_deg) = (*spacing_m, *angle_deg);
                    el.revision = revision;
                }
                Command::SetFamilyTexts { id, texts } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Family { return Err(EditError::UnsupportedProperty(*id)); }
                    el.texts = texts.clone();
                    el.revision = revision;
                }
                Command::SetFamily { id, family, type_name, overrides, angle_deg } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Family { return Err(EditError::UnsupportedProperty(*id)); }
                    (el.family, el.type_name, el.overrides, el.angle_deg) = (family.clone(), type_name.clone(), overrides.clone(), *angle_deg);
                    el.revision = revision;
                }
                Command::SetCount { id, count } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.kind(), Category::Rebar | Category::Stair) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.count = *count;
                    el.revision = revision;
                }
                Command::SetParent { id, parent } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Beam { return Err(EditError::UnsupportedProperty(*id)); }
                    el.parent = *parent;
                    el.revision = revision;
                }
                Command::SetJoins { id, start, end } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.category, Category::Beam | Category::Wall) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.join_ends = [*start, *end];
                    el.revision = revision;
                }
                Command::SetSlabKind { id, kind } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Slab { return Err(EditError::UnsupportedProperty(*id)); }
                    el.slab_kind = *kind;
                    if *kind == SlabKind::Roof {
                        el.slope_ratio.get_or_insert(roof::DEFAULT_SLOPE);
                        if el.slope_edges.len() != el.outline.len() { el.slope_edges = vec![true; el.outline.len()]; }
                    } else { el.slope_ratio = None; el.slope_edges.clear(); }
                    el.revision = revision;
                }
                Command::SetRoof { id, slope_ratio, edges } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Slab || el.slab_kind != SlabKind::Roof { return Err(EditError::UnsupportedProperty(*id)); }
                    el.slope_ratio = Some(*slope_ratio);
                    el.slope_edges = edges.clone();
                    el.revision = revision;
                }
                Command::SetMaterial { id, material } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.kind(), Category::Beam | Category::Column | Category::Slab | Category::Footing | Category::WallFooting | Category::BeamSystem | Category::Truss) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.material = *material;
                    el.revision = revision;
                }
                Command::SetSection { id, section } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !matches!(el.kind(), Category::Beam | Category::Column | Category::BeamSystem | Category::Truss) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.section = *section;
                    if *section == Section::Circle { el.height_m = el.width_m; }
                    el.revision = revision;
                }
                Command::SetGeometry { id, geometry } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    el.geometry = Some(*geometry);
                    el.revision = revision;
                }
                Command::SetType { id, type_id } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if !Element::can_have_type(el.category) { return Err(EditError::UnsupportedProperty(*id)); }
                    el.type_id = *type_id;
                    el.revision = revision;
                }
                Command::SetTypeFor { id, category } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Type { return Err(EditError::UnsupportedProperty(*id)); }
                    el.type_for = Some(*category);
                    el.revision = revision;
                }
                Command::SetMep(mep) => { trial_mep = (**mep).clone(); }
                Command::Rename { id, name } => {
                    let el = trial.get_mut(id).ok_or(EditError::NotFound(*id))?;
                    if el.category != Category::Type { return Err(EditError::UnsupportedProperty(*id)); }
                    el.name = name.trim().into();
                    el.revision = revision;
                }
                Command::Restore(element) => {
                    if trial.contains_key(&element.id) { return Err(EditError::AlreadyExists(element.id)); }
                    trial.insert(element.id, Element { revision, ..(**element).clone() });
                }
                Command::SetSite(site) => {
                    if !site.valid() { return Err(EditError::InvalidSite); }
                    trial_site = Some(*site);
                }
                Command::Delete { id } => {
                    if trial.remove(id).is_none() { return Err(EditError::NotFound(*id)); }
                    // the tags of an element go with it
                    trial.retain(|_, e| !matches!(e.geometry, Some(Geometry::Tag { host, .. }) if host == *id));
                }
            }
        }
        if trial.values().any(|e| !e.valid()) { return Err(EditError::InvalidElement); }
        spatial::validate(&trial)?;
        // what was joined to a run that is gone is no longer joined
        let trial_mep = trial_mep.pruned(&trial);
        trial_mep.check(&trial)?;
        self.history.push((std::mem::replace(&mut self.elements, trial), std::mem::replace(&mut self.site, trial_site), std::mem::replace(&mut self.mep, trial_mep)));
        self.revision = revision;
        Ok(revision)
    }

    /// Undo the most recent complete transaction. Does not reuse a revision
    /// number, because undo is itself a document change.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        if self.history.is_empty() { return Ok(false); }
        let revision = self.revision.checked_add(1).ok_or(EditError::RevisionOverflow)?;
        (self.elements, self.site, self.mep) = self.history.pop().expect("checked history");
        self.revision = revision;
        // Elements restored by undo are changed at the new document revision.
        for el in self.elements.values_mut() { el.revision = revision; }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(id: u64) -> Command {
        Command::Create {
            id: ElementId(id), category: Category::Duct, name: "Supply Air".into(),
            width_m: 0.2, height_m: 0.3, elevation_m: 2.7,
        }
    }

    #[test]
    fn the_site_is_validated_set_and_undone_with_the_elements() {
        let site = Site { latitude: 51.5, longitude: -0.12, angle_to_true_north_deg: 30.0, utc_offset_hours: 0.0 };
        let mut d = Document::new();
        assert_eq!(d.site(), None);
        assert_eq!(d.transact(0, &[Command::SetSite(site)]), Ok(1));
        assert_eq!(d.site(), Some(site));
        for bad in [Site { latitude: 91.0, ..site }, Site { longitude: -181.0, ..site }, Site { angle_to_true_north_deg: f64::NAN, ..site }, Site { utc_offset_hours: 20.0, ..site }] {
            assert_eq!(d.transact(1, &[Command::SetSite(bad)]), Err(EditError::InvalidSite));
        }
        assert_eq!((d.revision(), d.site()), (1, Some(site)), "a refused edit changes nothing");
        // one transaction: the site and an element together, and one undo takes both back
        let moved = Site { angle_to_true_north_deg: -45.0, ..site };
        assert_eq!(d.transact(1, &[create(7), Command::SetSite(moved)]), Ok(2));
        assert_eq!(d.undo(), Ok(true));
        assert_eq!((d.site(), d.get(ElementId(7)).is_none()), (Some(site), true));
        assert_eq!(d.undo(), Ok(true));
        assert_eq!(d.site(), None);
    }

    #[test]
    fn create_resize_and_undo_are_atomic() {
        let mut d = Document::new();
        assert_eq!(d.transact(0, &[create(1)]), Ok(1));
        assert_eq!(d.transact(1, &[Command::Resize {
            id: ElementId(1), width_m: 0.5, height_m: 0.4,
        }]), Ok(2));
        assert_eq!(d.get(ElementId(1)).unwrap().width_m, 0.5);
        assert_eq!(d.undo(), Ok(true));
        assert_eq!(d.revision(), 3);
        assert_eq!(d.get(ElementId(1)).unwrap().width_m, 0.2);
    }

    #[test]
    fn invalid_batch_rolls_back_completely() {
        let mut d = Document::new();
        let result = d.transact(0, &[create(1), Command::Resize {
            id: ElementId(1), width_m: f64::NAN, height_m: 0.4,
        }]);
        assert_eq!(result, Err(EditError::InvalidElement));
        assert_eq!(d.revision(), 0);
        assert_eq!(d.elements().count(), 0);
        assert!(!d.can_undo());
    }

    #[test]
    fn stale_revision_rejected() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        assert_eq!(d.transact(0, &[create(2)]),
            Err(EditError::RevisionConflict { expected: 0, actual: 1 }));
        assert!(d.get(ElementId(2)).is_none());
    }

    #[test]
    fn duplicate_and_missing_ids_rejected() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        assert_eq!(d.transact(1, &[create(1)]), Err(EditError::AlreadyExists(ElementId(1))));
        assert_eq!(d.transact(1, &[Command::Delete { id: ElementId(2) }]),
            Err(EditError::NotFound(ElementId(2))));
        assert_eq!(d.revision(), 1);
    }

    #[test]
    fn document_ids_remain_stable_after_undo() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        d.transact(1, &[Command::Delete { id: ElementId(1) }]).unwrap();
        d.undo().unwrap();
        assert_eq!(d.get(ElementId(1)).unwrap().id, ElementId(1));
    }

    #[test]
    fn duct_and_pipe_slope_edits_are_signed_and_undoable() {
        for category in [Category::Duct, Category::Pipe] {
            let mut d = Document::new();
            let mut command = create(1);
            if let Command::Create { category: kind, .. } = &mut command { *kind = category; }
            d.transact(0, &[command]).unwrap();
            d.transact(1, &[Command::SetSlope { id: ElementId(1), slope_ratio: -0.02 }]).unwrap();
            assert_eq!(d.get(ElementId(1)).unwrap().slope_ratio, Some(-0.02));
            d.undo().unwrap();
            assert_eq!(d.get(ElementId(1)).unwrap().slope_ratio, Some(0.0));
        }
    }

    #[test]
    fn invalid_slope_rolls_back_batch_and_other_categories_reject_slope() {
        let mut d = Document::new();
        d.transact(0, &[create(1)]).unwrap();
        assert_eq!(d.transact(1, &[
            Command::Resize { id: ElementId(1), width_m: 0.8, height_m: 0.6 },
            Command::SetSlope { id: ElementId(1), slope_ratio: f64::INFINITY },
        ]), Err(EditError::InvalidElement));
        assert_eq!(d.get(ElementId(1)).unwrap().width_m, 0.2);
        assert_eq!(d.revision(), 1);
        for category in [Category::Wall, Category::Equipment] {
            let mut d = Document::new();
            let mut command = create(1);
            if let Command::Create { category: kind, .. } = &mut command { *kind = category; }
            d.transact(0, &[command]).unwrap();
            assert_eq!(d.get(ElementId(1)).unwrap().slope_ratio, None);
            assert_eq!(d.transact(1, &[Command::SetSlope { id: ElementId(1), slope_ratio: 0.02 }]),
                Err(EditError::UnsupportedProperty(ElementId(1))));
        }
    }
}
