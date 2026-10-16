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
}
impl From<Geometry> for SnapshotGeometry {
    fn from(value:Geometry)->Self {
        match value {
            Geometry::Level=>Self::Level,
            Geometry::Grid { start,end }=>Self::Grid { start,end },
            Geometry::Linear { start,end,level }=>Self::Linear { start,end,level:level.0 },
        }
    }
}
impl From<SnapshotGeometry> for Geometry {
    fn from(value:SnapshotGeometry)->Self {
        match value {
            SnapshotGeometry::Level=>Self::Level,
            SnapshotGeometry::Grid { start,end }=>Self::Grid { start,end },
            SnapshotGeometry::Linear { start,end,level }=>Self::Linear { start,end,level:ElementId(level) },
        }
    }
}

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
            geometry:e.geometry.map(Into::into),
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
                "Wall" => Category::Wall, "Pipe" => Category::Pipe,
                "Duct" => Category::Duct, "Equipment" => Category::Equipment,
                "Level" => Category::Level, "Grid" => Category::Grid,
                _ => return Err(format!("Unsupported category {}",el.category)),
            };
            if category.supports_slope() != el.slope_ratio.is_some() {
                return Err(format!("Invalid slope property for element {}",el.id));
            }
            ops.push(Command::Create { id: ElementId(el.id), category, name: el.name.clone(),
                width_m: el.width_m, height_m: el.height_m, elevation_m: el.elevation_m });
            if let Some(slope_ratio) = el.slope_ratio {
                ops.push(Command::SetSlope { id: ElementId(el.id), slope_ratio });
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
