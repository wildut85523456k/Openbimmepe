//! Versioned file/stdio interchange. This transports immutable snapshots and
//! preview reports, never executable host commands or a transferable apply token.
use crate::{Change, Graph, Value};
use openbim_document::{Category, Command, Document, ElementId, Geometry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SnapshotGeometry {
    Level,
    Grid { start:[f64;2],end:[f64;2] },
    Linear { start:[f64;2],end:[f64;2],level:u64 },
    Point { at:[f64;2],level:u64,top:u64 },
    Area { level:u64 },
    Pad { at:[f64;2],level:u64,thickness_m:f64 },
    OnWall { host:u64 },
    OnMember { host:u64 },
    Placed { at:[f64;2],level:u64 },
    InWall { host:u64,along_m:f64 },
    Tag { host:u64,at:[f64;2],anchor:[f64;2] },
    Dimension { a:[f64;2],b:[f64;2],at:[f64;2],level:u64 },
    Room { at:[f64;2],level:u64 },
    Shaft { level:u64,top:u64,base_offset_m:f64,top_offset_m:f64 },
}
impl From<Geometry> for SnapshotGeometry {
    fn from(value:Geometry)->Self {
        match value {
            Geometry::Level=>Self::Level,
            Geometry::Grid { start,end }=>Self::Grid { start,end },
            Geometry::Linear { start,end,level }=>Self::Linear { start,end,level:level.0 },
            Geometry::Point { at,level,top }=>Self::Point { at,level:level.0,top:top.0 },
            Geometry::Area { level }=>Self::Area { level:level.0 },
            Geometry::Pad { at,level,thickness_m }=>Self::Pad { at,level:level.0,thickness_m },
            Geometry::OnWall { host }=>Self::OnWall { host:host.0 },
            Geometry::OnMember { host }=>Self::OnMember { host:host.0 },
            Geometry::Placed { at,level }=>Self::Placed { at,level:level.0 },
            Geometry::InWall { host,along_m }=>Self::InWall { host:host.0,along_m },
            Geometry::Tag { host,at,anchor }=>Self::Tag { host:host.0,at,anchor },
            Geometry::Dimension { a,b,at,level }=>Self::Dimension { a,b,at,level:level.0 },
            Geometry::Room { at,level }=>Self::Room { at,level:level.0 },
            Geometry::Shaft { level,top,base_offset_m,top_offset_m }=>Self::Shaft { level:level.0,top:top.0,base_offset_m,top_offset_m },
        }
    }
}
impl From<SnapshotGeometry> for Geometry {
    fn from(value:SnapshotGeometry)->Self {
        match value {
            SnapshotGeometry::Level=>Self::Level,
            SnapshotGeometry::Grid { start,end }=>Self::Grid { start,end },
            SnapshotGeometry::Linear { start,end,level }=>Self::Linear { start,end,level:ElementId(level) },
            SnapshotGeometry::Point { at,level,top }=>Self::Point { at,level:ElementId(level),top:ElementId(top) },
            SnapshotGeometry::Area { level }=>Self::Area { level:ElementId(level) },
            SnapshotGeometry::Pad { at,level,thickness_m }=>Self::Pad { at,level:ElementId(level),thickness_m },
            SnapshotGeometry::OnWall { host }=>Self::OnWall { host:ElementId(host) },
            SnapshotGeometry::OnMember { host }=>Self::OnMember { host:ElementId(host) },
            SnapshotGeometry::Placed { at,level }=>Self::Placed { at,level:ElementId(level) },
            SnapshotGeometry::InWall { host,along_m }=>Self::InWall { host:ElementId(host),along_m },
            SnapshotGeometry::Tag { host,at,anchor }=>Self::Tag { host:ElementId(host),at,anchor },
            SnapshotGeometry::Dimension { a,b,at,level }=>Self::Dimension { a,b,at,level:ElementId(level) },
            SnapshotGeometry::Room { at,level }=>Self::Room { at,level:ElementId(level) },
            SnapshotGeometry::Shaft { level,top,base_offset_m,top_offset_m }=>Self::Shaft { level:ElementId(level),top:ElementId(top),base_offset_m,top_offset_m },
        }
    }
}

/// The categories a type can serve.
const CATEGORIES: [Category; 5] = [Category::Beam, Category::Column, Category::WallFooting, Category::Slab, Category::Rebar];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotElement {
    pub id: u64,
    pub category: String,
    pub name: String,
    pub width_m: f64,
    pub height_m: f64,
    pub elevation_m: f64,
    pub slope_ratio: Option<f64>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub geometry:Option<SnapshotGeometry>,
    /// the section of a beam when it is not a rectangle
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub section:Option<SnapshotSection>,
    /// the usage of a beam when it is not a Girder: Joist, Purlin, Brace or Other
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub usage:Option<String>,
    /// the structural material of a beam: Concrete, Steel or Timber
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub material:Option<String>,
    /// the plan outline of a slab
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub outline:Vec<[f64;2]>,
    /// the kind of a slab when it is not a Floor: Foundation
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub slab_kind:Option<String>,
    /// a roof: whether each edge of its outline defines a slope
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub slope_edges:Vec<bool>,
    /// a beam system: the distance between its beams and their direction (degrees)
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub spacing_m:Option<f64>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub angle_deg:Option<f64>,
    /// the beam system a beam belongs to
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub parent:Option<u64>,
    /// a beam: whether its start and end are joined to the columns they end in, when not both
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub join_ends:Option<[bool;2]>,
    /// a rebar: the number of bars along the bottom of its beam
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub count:Option<u32>,
    /// the type an element takes its values from
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub type_id:Option<u64>,
    /// a type element: the category it is a type of
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub type_for:Option<String>,
    /// a family instance: its family, type, instance values and turn
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub family:Option<String>,
    #[serde(default,skip_serializing_if="String::is_empty")]
    pub type_name:String,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub overrides:Vec<(String,f64)>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub texts:Vec<(String,String)>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotSection {
    /// "Circle" or "IShape"
    pub kind: String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub web_m: Option<f64>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub flange_m: Option<f64>,
}
impl SnapshotSection {
    fn from_document(s: openbim_document::Section) -> Option<Self> {
        match s {
            openbim_document::Section::Rectangle => None,
            openbim_document::Section::Circle => Some(Self { kind: "Circle".into(), web_m: None, flange_m: None }),
            openbim_document::Section::IShape { web_m, flange_m } => Some(Self { kind: "IShape".into(), web_m: Some(web_m), flange_m: Some(flange_m) }),
        }
    }
    fn to_document(&self) -> Result<openbim_document::Section, String> {
        match (self.kind.as_str(), self.web_m, self.flange_m) {
            ("Circle", None, None) => Ok(openbim_document::Section::Circle),
            ("IShape", Some(web_m), Some(flange_m)) => Ok(openbim_document::Section::IShape { web_m, flange_m }),
            _ => Err(format!("Unsupported section {}", self.kind)),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub document_id: String,
    pub revision: u64,
    pub elements: Vec<SnapshotElement>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    pub version: u32,
    pub request_id: String,
    pub snapshot: Snapshot,
    pub graph: Graph,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewResponse {
    pub version: u32,
    pub request_id: String,
    pub document_id: String,
    pub source_revision: u64,
    pub changes: Vec<Change>,
    pub values: BTreeMap<u64, Value>,
    #[serde(default)]
    pub outputs: Vec<OutputValue>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OutputValue { pub node:u64, pub port:u16, pub value:Value }
impl Snapshot {
    pub fn to_json(&self)->Result<String,String> {
        serde_json::to_string_pretty(self).map_err(|e|e.to_string())
    }
    pub fn capture(document_id: &str, document: &Document) -> Self {
        Self { document_id: document_id.into(), revision: document.revision(), elements: document.elements().map(|e| SnapshotElement {
            id: e.id.0, category: format!("{:?}",e.category), name: e.name.clone(),
            width_m: e.width_m, height_m: e.height_m, elevation_m: e.elevation_m, slope_ratio: e.slope_ratio,
            geometry:e.geometry.map(Into::into), section: SnapshotSection::from_document(e.section),
            usage: (e.usage != openbim_document::Usage::Girder).then(|| e.usage.name().to_string()),
            material: (e.material != openbim_document::Material::Unspecified).then(|| e.material.name().to_string()),
            outline: e.outline.clone(),
            slab_kind: (e.slab_kind != openbim_document::SlabKind::Floor).then(|| e.slab_kind.name().to_string()),
            slope_edges: e.slope_edges.clone(),
            spacing_m: matches!(e.kind(), Category::BeamSystem | Category::Truss | Category::Rebar).then_some(e.spacing_m),
            angle_deg: matches!(e.kind(), Category::BeamSystem | Category::Truss | Category::Rebar | Category::Family).then_some(e.angle_deg),
            parent: e.parent.map(|p| p.0),
            join_ends: (e.join_ends != [true; 2]).then_some(e.join_ends),
            count: (e.kind() == Category::Rebar).then_some(e.count),
            type_id: e.type_id.map(|t| t.0),
            type_for: e.type_for.map(|c| format!("{c:?}")),
            family: (e.category == Category::Family).then(|| e.family.clone()),
            type_name: e.type_name.clone(),
            overrides: e.overrides.clone(),
            texts: e.texts.clone(),
        }).collect() }
    }
    /// Rebuild a temporary document through host validation. Its local revision
    /// is intentionally independent of the source revision carried in reports.
    pub fn materialize(&self) -> Result<Document, String> {
        if self.document_id.trim().is_empty() { return Err("Missing document identity".into()); }
        if self.elements.len() > 100_000 { return Err("Snapshot element limit exceeded".into()); }
        let mut seen = BTreeSet::new();
        let mut ops = Vec::new();
        for el in &self.elements {
            if !seen.insert(el.id) { return Err(format!("Duplicate element {}",el.id)); }
            let category = match el.category.as_str() {
                "Wall" => Category::Wall, "Beam" => Category::Beam, "Column" => Category::Column, "Slab" => Category::Slab, "Footing" => Category::Footing, "WallFooting" => Category::WallFooting, "BeamSystem" => Category::BeamSystem, "Truss" => Category::Truss, "Rebar" => Category::Rebar, "Family" => Category::Family, "Type" => Category::Type, "Pipe" => Category::Pipe,
                "Duct" => Category::Duct, "Equipment" => Category::Equipment,
                "Level" => Category::Level, "Grid" => Category::Grid, "Tag" => Category::Tag, "Dimension" => Category::Dimension, "Room" => Category::Room, "Stair" => Category::Stair, "Opening" => Category::Opening, "Section" => Category::Section,
                _ => return Err(format!("Unsupported category {}",el.category)),
            };
            let type_for = match &el.type_for {
                Some(c) => Some(CATEGORIES.iter().copied().find(|k| format!("{k:?}") == *c).ok_or(format!("Unsupported type category {c}"))?),
                None => None,
            };
            let roof = el.slab_kind.as_deref() == Some("Roof");
            if (category.supports_slope() || (roof && category == Category::Slab)) != el.slope_ratio.is_some() {
                return Err(format!("Invalid slope property for element {}",el.id));
            }
            ops.push(Command::Create { id: ElementId(el.id), category, name: el.name.clone(),
                width_m: el.width_m, height_m: el.height_m, elevation_m: el.elevation_m });
            if let Some(category) = type_for {
                ops.push(Command::SetTypeFor { id: ElementId(el.id), category });
            }
            if let Some(t) = el.type_id {
                ops.push(Command::SetType { id: ElementId(el.id), type_id: Some(ElementId(t)) });
            }
            if let (Some(slope_ratio), false) = (el.slope_ratio, roof) {
                ops.push(Command::SetSlope { id: ElementId(el.id), slope_ratio });
            }
            if let Some(u) = &el.usage {
                let usage = openbim_document::Usage::ALL.into_iter().find(|x| x.name() == u).ok_or(format!("Unsupported usage {u}"))?;
                ops.push(Command::SetUsage { id: ElementId(el.id), usage });
            }
            if let Some(m) = &el.material {
                let material = openbim_document::Material::ALL.into_iter().find(|x| x.name() == m).ok_or(format!("Unsupported material {m}"))?;
                ops.push(Command::SetMaterial { id: ElementId(el.id), material });
            }
            if let Some(section) = &el.section {
                ops.push(Command::SetSection { id: ElementId(el.id), section: section.to_document()? });
            }
            if let Some(k) = &el.slab_kind {
                let kind = openbim_document::SlabKind::ALL.into_iter().find(|x| x.name() == k).ok_or(format!("Unsupported slab kind {k}"))?;
                ops.push(Command::SetSlabKind { id: ElementId(el.id), kind });
            }
            if let (true, Some(slope_ratio)) = (roof, el.slope_ratio) {
                ops.push(Command::SetRoof { id: ElementId(el.id), slope_ratio, edges: el.slope_edges.clone() });
            }
            if let (Some(family), Some(angle_deg)) = (&el.family, el.angle_deg) {
                ops.push(Command::SetFamily { id: ElementId(el.id), family: family.clone(), type_name: el.type_name.clone(), overrides: el.overrides.clone(), angle_deg });
            } else if let (Some(spacing_m), Some(angle_deg)) = (el.spacing_m, el.angle_deg) {
                ops.push(Command::SetBeamSystem { id: ElementId(el.id), spacing_m, angle_deg });
            }
            if let Some(count) = el.count {
                ops.push(Command::SetCount { id: ElementId(el.id), count });
            }
            if !el.texts.is_empty() {
                ops.push(Command::SetFamilyTexts { id: ElementId(el.id), texts: el.texts.clone() });
            }
            if let Some(p) = el.parent {
                ops.push(Command::SetParent { id: ElementId(el.id), parent: Some(ElementId(p)) });
            }
            if let Some([start, end]) = el.join_ends {
                ops.push(Command::SetJoins { id: ElementId(el.id), start, end });
            }
            if !el.outline.is_empty() {
                ops.push(Command::SetOutline { id: ElementId(el.id), outline: el.outline.clone() });
            }
            if let Some(geometry)=el.geometry {
                ops.push(Command::SetGeometry { id:ElementId(el.id),geometry:geometry.into() });
            }
        }
        let mut document = Document::new();
        if !ops.is_empty() { document.transact(0, &ops).map_err(|e|format!("Invalid snapshot: {e:?}"))?; }
        Ok(document)
    }
}
impl PreviewRequest {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
    pub fn evaluate(&self, python: Option<&crate::worker::PythonWorker>, cancel: &std::sync::atomic::AtomicBool) -> Result<PreviewResponse,String> {
        if self.version != 1 { return Err("Unsupported protocol version".into()); }
        if self.request_id.is_empty() || self.request_id.len() > 256 { return Err("Invalid request identity".into()); }
        let document = self.snapshot.materialize()?;
        let plan = self.graph.preview_with(&document,python,cancel).map_err(|e|format!("{e:?}"))?;
        Ok(PreviewResponse { version:1, request_id:self.request_id.clone(), document_id:self.snapshot.document_id.clone(),
            source_revision:self.snapshot.revision, changes:plan.changes().to_vec(), values:plan.values().clone(),
            outputs:plan.port_values().iter().map(|(&(node,port),value)|OutputValue {node,port,value:value.clone()}).collect() })
    }
}
