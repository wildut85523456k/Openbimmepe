//! Schematic zones and the extraction query behind `MEP: Generate Schematic` (doc 06, backlog SCH-001).
//!
//! A zone is a named rectangle drawn in the plan of one level. End points inside a zone are summarized into one
//! group of the schematic; electrical-family disciplines pull their boards as the nodes instead (06, decisions 1
//! and 4). Zones live in `MepData` with the schedules and the sheets, so they are saved, undone and exported with
//! the model. The plan rectangle-drag tool itself is GUI work and lands with GUI-004; this module is the data
//! model, the assignment rules and the extraction query, with a JSON dump of the extracted draft.
//!
//! Lengths are in metres, flows in m3/s, powers in watts, as in the rest of the engine.
use crate::mep::{EndDemand, EndRole, MepNode};
use crate::{Category, Command, Document, EditError, ElementId, Geometry};
use openbim_family::mep::System;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// First number of a schematic zone (kept apart from elements, fittings and electrical nodes).
pub const ZONE_BASE: u64 = 1 << 45;
/// Most zones in one model.
pub const MAX_ZONES: usize = 256;
/// Longest zone label.
pub const MAX_ZONE_LABEL: usize = 64;

/// A rectangle in the plan of a level, in metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl Rect {
    /// The rectangle between two corners, whichever way round they were drawn. A rectangle with no area is refused.
    pub fn new(a: [f64; 2], b: [f64; 2]) -> Result<Self, String> {
        if [a, b].iter().flatten().any(|v| !v.is_finite()) {
            return Err("a zone rectangle with a point that is not a number".into());
        }
        let (min, max) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
        if (max[0] - min[0]) * (max[1] - min[1]) < 1e-6 {
            return Err("a zone rectangle is too small to draw".into());
        }
        Ok(Self { min, max })
    }
    /// Whether a plan point is inside, edges included.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.min[0] && p[0] <= self.max[0] && p[1] >= self.min[1] && p[1] <= self.max[1]
    }
    /// Whether two rectangles share an area; touching edges do not count.
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.min[0] < o.max[0] && o.min[0] < self.max[0] && self.min[1] < o.max[1] && o.min[1] < self.max[1]
    }
}

/// A schematic zone: a named rectangle drawn in the plan of one level (doc 06, decision 1). An end point whose
/// plan position is inside the rectangle belongs to the zone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchematicZone {
    /// stable for the life of the zone; at least `ZONE_BASE`
    pub id: ElementId,
    pub level: ElementId,
    pub rect: Rect,
    /// the name the designer gave it; empty until labelled (the room name is the fallback)
    pub label: String,
}

/// The pairs of zones on one level whose rectangles share an area. The designer resolves overlaps; the extraction
/// assigns an end point inside several zones to the first of them by id (creation order).
pub fn overlapping_zones(zones: &[SchematicZone]) -> Vec<(ElementId, ElementId)> {
    let mut out = Vec::new();
    for (i, a) in zones.iter().enumerate() {
        for b in &zones[i + 1..] {
            if a.level == b.level && a.rect.overlaps(&b.rect) {
                out.push((a.id, b.id));
            }
        }
    }
    out
}

/// The name of the room whose point is inside the zone, for a zone the designer has not labelled (doc 06: room
/// containment is a fallback labeler, never the definition of a group).
pub fn room_fallback_label(doc: &Document, zone: &SchematicZone) -> Option<String> {
    doc.elements()
        .filter(|e| e.category == Category::Room)
        .filter_map(|e| match e.geometry {
            Some(Geometry::Room { at, level }) if level == zone.level && zone.rect.contains(at) => {
                Some(e.name.clone())
            }
            _ => None,
        })
        .next()
}

/// The label a zone shows on the schematic: its own, else the room's, else its number.
pub fn zone_display_label(doc: &Document, zone: &SchematicZone) -> String {
    if !zone.label.is_empty() {
        return zone.label.clone();
    }
    if let Some(name) = room_fallback_label(doc, zone) {
        return name;
    }
    format!("Zone {}", zone.id.0.saturating_sub(ZONE_BASE) + 1)
}

/// The eight disciplines of doc 06, decision 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Discipline {
    Electrical,
    /// extra-low voltage
    Elv,
    AccessControl,
    Bms,
    Plumbing,
    /// hydronic HVAC: chilled, heating and condenser water
    ChilledWater,
    AirDistribution,
    Sprinkler,
}

/// The two annotation families of doc 06: electrical-family disciplines annotate current and voltage drop,
/// hydraulic/air disciplines annotate flow and pressure drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Electrical,
    HydraulicAir,
}

impl Discipline {
    pub const ALL: [Discipline; 8] = [
        Discipline::Electrical,
        Discipline::Elv,
        Discipline::AccessControl,
        Discipline::Bms,
        Discipline::Plumbing,
        Discipline::ChilledWater,
        Discipline::AirDistribution,
        Discipline::Sprinkler,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Discipline::Electrical => "Electrical",
            Discipline::Elv => "ELV",
            Discipline::AccessControl => "Access control",
            Discipline::Bms => "BMS",
            Discipline::Plumbing => "Plumbing",
            Discipline::ChilledWater => "HVAC chilled water",
            Discipline::AirDistribution => "HVAC air distribution",
            Discipline::Sprinkler => "Sprinkler / fire fighting",
        }
    }
    pub fn family(self) -> Family {
        match self {
            Discipline::Electrical | Discipline::Elv | Discipline::AccessControl | Discipline::Bms => {
                Family::Electrical
            }
            _ => Family::HydraulicAir,
        }
    }
    /// The discipline a system type belongs to; `Other` is none of them until the designer says.
    pub fn of_system(system: System) -> Option<Discipline> {
        use System::*;
        match system {
            Power | Data => Some(Discipline::Electrical),
            SupplyAir | ReturnAir | ExhaustAir | OutsideAir => Some(Discipline::AirDistribution),
            ChilledWaterSupply | ChilledWaterReturn | HeatingWaterSupply | HeatingWaterReturn
            | CondenserWaterSupply | CondenserWaterReturn => Some(Discipline::ChilledWater),
            FireProtection => Some(Discipline::Sprinkler),
            Sanitary | DomesticCold | DomesticHot | Vent | StormWater | Condensate | Gas => {
                Some(Discipline::Plumbing)
            }
            Other => None,
        }
    }
}

/// One level of the schematic: a row of the grid (doc 06).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftLevel {
    pub id: ElementId,
    pub name: String,
    pub elevation_m: f64,
}

/// A panel, board or controller pulled as a node of the schematic (electrical-family disciplines, doc 06).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftNode {
    /// the electrical node's number
    pub id: u64,
    pub name: String,
    pub level: ElementId,
    pub at: [f64; 2],
    /// the real power it carries, W
    pub watts: f64,
}

/// A group of end points summarized by zone (hydraulic/air disciplines, doc 06). `zone` is `None` for end points
/// inside no zone: they are not dropped, they are grouped apart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftGroup {
    pub zone: Option<ElementId>,
    pub zone_label: String,
    pub level: ElementId,
    #[serde(serialize_with = "system_as_code", deserialize_with = "system_from_code")]
    pub system: System,
    pub endpoints: usize,
    /// The summed flow of the group's end points, m3/s. Demands that are not flows yet (power, loading units) are
    /// counted but not summed: the calculation modules turn them into flows in SCH-006.
    pub flow_m3s: f64,
}

/// An end point inside more than one zone: assigned to `assigned`, the others reported (doc 06).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoneAmbiguity {
    pub level: ElementId,
    pub at: [f64; 2],
    /// every zone that contains the end point, by id
    pub zones: Vec<ElementId>,
    /// the zone the extraction assigned it to: the first by id, creation order
    pub assigned: ElementId,
}

/// The extracted data a schematic is laid out from: the levels as rows, the nodes, the groups, and the warnings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchematicDraft {
    pub discipline: Discipline,
    /// sorted by elevation, then name
    pub levels: Vec<DraftLevel>,
    pub nodes: Vec<DraftNode>,
    pub groups: Vec<DraftGroup>,
    pub ambiguities: Vec<ZoneAmbiguity>,
    /// terminal end points on a fitting port, with no plan position of their own yet
    pub unplaced_endpoints: usize,
}

impl SchematicDraft {
    /// The JSON dump of the extraction (doc 06, SCH-001): the interface the grid layout of SCH-002 reads.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

fn system_as_code<S: serde::Serializer>(system: &System, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_u8(system.code())
}

fn system_from_code<'de, D: serde::Deserializer<'de>>(de: D) -> Result<System, D::Error> {
    System::from_code(u8::deserialize(de)?)
        .ok_or_else(|| serde::de::Error::custom("an unknown system code"))
}

/// The extraction query of doc 06: reads the model and the zones, and gives the draft a schematic is laid out
/// from. Electrical pulls its boards as nodes; the hydraulic/air disciplines summarize terminal end points per
/// zone and system. ELV, access control and BMS have no model of their own yet, so they extract empty.
pub fn extract_schematic(doc: &Document, discipline: Discipline) -> SchematicDraft {
    let mut levels: Vec<DraftLevel> = doc
        .elements()
        .filter(|e| e.category == Category::Level)
        .map(|e| DraftLevel { id: e.id, name: e.name.clone(), elevation_m: e.elevation_m })
        .collect();
    levels.sort_by(|a, b| {
        a.elevation_m
            .total_cmp(&b.elevation_m)
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });
    let mut draft = SchematicDraft {
        discipline,
        levels,
        nodes: Vec::new(),
        groups: Vec::new(),
        ambiguities: Vec::new(),
        unplaced_endpoints: 0,
    };
    let mep = doc.mep();
    if discipline.family() == Family::Electrical {
        if discipline == Discipline::Electrical {
            for n in &mep.elec_nodes {
                if n.kind == crate::ElecKind::Board {
                    draft.nodes.push(DraftNode {
                        id: n.id,
                        name: n.name.clone(),
                        level: n.level,
                        at: n.at,
                        watts: n.watts,
                    });
                }
            }
        }
        return draft;
    }
    let mut acc: BTreeMap<(Option<ElementId>, u8), (ElementId, usize, f64)> = BTreeMap::new();
    for ep in &mep.endpoints {
        if ep.role != EndRole::Terminal {
            continue;
        }
        let MepNode::Run(run) = ep.port.node else {
            draft.unplaced_endpoints += 1;
            continue;
        };
        let Some(el) = doc.get(run) else {
            draft.unplaced_endpoints += 1;
            continue;
        };
        let Some(Geometry::Linear { start, end, level }) = el.geometry else {
            draft.unplaced_endpoints += 1;
            continue;
        };
        let at = if ep.port.port == 0 { start } else { end };
        let system = crate::compute::system_of_run(doc, run);
        if Discipline::of_system(system) != Some(discipline) {
            continue;
        }
        let flow = match ep.demand {
            EndDemand::Flow(q) => q,
            EndDemand::Sprinkler { min_flow_l_min, .. } => min_flow_l_min / 60_000.0,
            _ => 0.0,
        };
        let mut containing: Vec<&SchematicZone> = mep
            .zones
            .iter()
            .filter(|z| z.level == level && z.rect.contains(at))
            .collect();
        containing.sort_by_key(|z| z.id);
        let zone = if containing.is_empty() {
            None
        } else {
            let assigned = containing[0].id;
            if containing.len() > 1 {
                draft.ambiguities.push(ZoneAmbiguity {
                    level,
                    at,
                    assigned,
                    zones: containing.iter().map(|z| z.id).collect(),
                });
            }
            Some(assigned)
        };
        let entry = acc.entry((zone, system.code())).or_insert((level, 0, 0.0));
        entry.1 += 1;
        entry.2 += flow;
    }
    for ((zone, sys_code), (level, endpoints, flow_m3s)) in acc {
        let system = System::from_code(sys_code).unwrap_or(System::Other);
        let zone_label = zone.map_or(String::new(), |id| {
            mep.zones
                .iter()
                .find(|z| z.id == id)
                .map_or(String::new(), |z| zone_display_label(doc, z))
        });
        draft.groups.push(DraftGroup {
            zone,
            zone_label,
            level,
            system,
            endpoints,
            flow_m3s,
        });
    }
    draft
}

impl Document {
    /// The schematic zones of the model.
    pub fn zones(&self) -> &[SchematicZone] {
        &self.mep().zones
    }

    /// Draws a zone on `level` between two plan corners. The label is trimmed; empty is allowed (the room name is
    /// the fallback when the schematic is built). Gives the new zone's id and the commands of one undoable step;
    /// the model is not changed until they are applied.
    pub fn place_zone_commands(
        &self,
        level: ElementId,
        a: [f64; 2],
        b: [f64; 2],
        label: &str,
    ) -> Result<(ElementId, Vec<Command>), String> {
        if !self.get(level).is_some_and(|l| l.category == Category::Level) {
            return Err("a zone is drawn on a level".into());
        }
        let rect = Rect::new(a, b)?;
        let label = label.trim();
        if label.chars().count() > MAX_ZONE_LABEL {
            return Err(format!("a zone label is at most {MAX_ZONE_LABEL} characters"));
        }
        let zones = &self.mep().zones;
        if zones.len() >= MAX_ZONES {
            return Err(format!("a model has at most {MAX_ZONES} zones"));
        }
        let id = ElementId(zones.iter().map(|z| z.id.0).max().unwrap_or(ZONE_BASE - 1) + 1);
        let mut data = self.mep().clone();
        data.zones.push(SchematicZone {
            id,
            level,
            rect,
            label: label.to_string(),
        });
        Ok((id, vec![Command::SetMep(Box::new(data))]))
    }

    /// Like `place_zone_commands`, applied in one transaction.
    pub fn place_zone(
        &mut self,
        level: ElementId,
        a: [f64; 2],
        b: [f64; 2],
        label: &str,
    ) -> Result<ElementId, EditError> {
        let (id, ops) = self
            .place_zone_commands(level, a, b, label)
            .map_err(|_| EditError::InvalidMep)?;
        self.transact(self.revision(), &ops)?;
        Ok(id)
    }

    /// Takes a zone out of the model. Gives the commands of one undoable step; the model is not changed until
    /// they are applied.
    pub fn delete_zone_commands(&self, id: ElementId) -> Result<Vec<Command>, String> {
        let mut data = self.mep().clone();
        let n = data.zones.len();
        data.zones.retain(|z| z.id != id);
        if data.zones.len() == n {
            return Err("there is no zone with that number".into());
        }
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// Like `delete_zone_commands`, applied in one transaction.
    pub fn delete_zone(&mut self, id: ElementId) -> Result<(), EditError> {
        let ops = self
            .delete_zone_commands(id)
            .map_err(|_| EditError::NotFound(id))?;
        self.transact(self.revision(), &ops)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mep::{MepData, MepEndpoint, MepFitting, MepPort, FITTING_BASE};
    use crate::{ElecKind, ElecNode};
    use openbim_family::mep::Domain;
    use openbim_place::penetrations::RunSize;
    use openbim_route::{Fitting, FittingKind};

    fn doc_with_levels() -> (Document, ElementId, ElementId) {
        let mut d = Document::new();
        let l1 = d.place_level("Level 1", 0.0).unwrap();
        let l2 = d.place_level("Level 2", 3.0).unwrap();
        (d, l1, l2)
    }

    /// A drawn pipe as the plumbing systems need it: a pipe domain, so a Sanitary or water system fits it.
    fn place_pipe(d: &mut Document, level: ElementId, start: [f64; 2], end: [f64; 2]) -> ElementId {
        let id = d.next_id();
        d.transact(d.revision(), &[
            Command::Create {
                id,
                category: Category::Pipe,
                name: format!("Pipe {}", id.0),
                width_m: 0.05,
                height_m: 0.05,
                elevation_m: 2.7,
            },
            Command::SetGeometry { id, geometry: Geometry::Linear { start, end, level } },
        ])
        .unwrap();
        id
    }

    /// A fitting as the connection code keeps it, for the end point on a fitting leg.
    fn fitting(id: u64, legs: usize) -> MepFitting {
        MepFitting {
            id,
            domain: Domain::Pipe,
            system: System::ChilledWaterSupply,
            fitting: Fitting {
                type_name: "Test".into(),
                part_code: String::new(),
                kind: FittingKind::Elbow { angle: 90.0 },
                at: [0.0, 0.0, 2.7],
                ports: vec![[1.0, 0.0, 0.0]; legs],
                port_sizes: vec![RunSize::Round { diameter: 0.05 }; legs],
                radius: 0.0,
                trim: vec![0.0; legs],
            },
        }
    }

    /// A room element as the fallback labeler reads it. `place_room` also requires the point to be enclosed by
    /// walls; the labeler does not, so the test builds the element directly.
    fn place_room_element(d: &mut Document, level: ElementId, at: [f64; 2], name: &str) -> ElementId {
        let id = d.next_id();
        d.transact(d.revision(), &[
            Command::Create {
                id,
                category: Category::Room,
                name: name.into(),
                width_m: 1.0,
                height_m: 1.0,
                elevation_m: 0.0,
            },
            Command::SetGeometry { id, geometry: Geometry::Room { at, level } },
        ])
        .unwrap();
        id
    }

    fn set_mep(d: &mut Document, f: impl FnOnce(&mut MepData)) {
        let mut m = d.mep().clone();
        f(&mut m);
        d.transact(d.revision(), &[Command::SetMep(Box::new(m))]).unwrap();
    }

    fn run_port(run: ElementId, port: usize) -> MepPort {
        MepPort { node: MepNode::Run(run), port }
    }

    fn end(port: MepPort, demand: EndDemand) -> MepEndpoint {
        MepEndpoint { port, role: EndRole::Terminal, demand, required_pressure_pa: 0.0 }
    }

    #[test]
    fn rect_normalizes_contains_overlaps() {
        let r = Rect::new([4.0, 5.0], [1.0, 2.0]).unwrap();
        assert_eq!(r, Rect { min: [1.0, 2.0], max: [4.0, 5.0] });
        assert!(r.contains([1.0, 2.0])); // edges included
        assert!(r.contains([4.0, 5.0]));
        assert!(!r.contains([4.1, 3.0]));
        assert!(r.overlaps(&Rect::new([3.0, 4.0], [6.0, 6.0]).unwrap()));
        assert!(!r.overlaps(&Rect::new([4.0, 2.0], [6.0, 4.0]).unwrap())); // touching edge: no area
        assert!(Rect::new([1.0, 1.0], [1.0, 1.0]).is_err());
        assert!(Rect::new([0.0, 0.0], [f64::NAN, 1.0]).is_err());
    }

    #[test]
    fn place_and_delete_zone() {
        let (mut d, l1, _) = doc_with_levels();
        let z = d.place_zone(l1, [0.0, 0.0], [10.0, 8.0], "  Toilets  ").unwrap();
        assert!(z.0 >= ZONE_BASE);
        assert_eq!(d.zones().len(), 1);
        assert_eq!(d.zones()[0].label, "Toilets");
        assert_eq!(d.zones()[0].rect, Rect { min: [0.0, 0.0], max: [10.0, 8.0] });
        assert!(d.place_zone(ElementId(999), [0.0, 0.0], [1.0, 1.0], "x").is_err()); // no such level
        assert!(d.place_zone(l1, [0.0, 0.0], [0.0, 0.0], "x").is_err()); // no area
        assert!(d.place_zone(l1, [0.0, 0.0], [1.0, 1.0], &"x".repeat(MAX_ZONE_LABEL + 1)).is_err());
        d.delete_zone(z).unwrap();
        assert!(d.zones().is_empty());
        assert!(d.delete_zone(z).is_err());
    }

    #[test]
    fn zone_undo_redo() {
        let (mut d, l1, _) = doc_with_levels();
        let z = d.place_zone(l1, [0.0, 0.0], [5.0, 5.0], "A").unwrap();
        assert!(d.undo().unwrap());
        assert!(d.zones().is_empty());
        assert!(d.redo().unwrap());
        assert_eq!(d.zones().len(), 1);
        assert_eq!(d.zones()[0].id, z);
    }

    #[test]
    fn overlaps_same_level_only() {
        let (mut d, l1, l2) = doc_with_levels();
        let a = d.place_zone(l1, [0.0, 0.0], [4.0, 4.0], "A").unwrap();
        let b = d.place_zone(l1, [2.0, 2.0], [6.0, 6.0], "B").unwrap();
        d.place_zone(l2, [0.0, 0.0], [4.0, 4.0], "C").unwrap(); // same rectangle, other level
        let zones = d.zones();
        assert_eq!(overlapping_zones(&[zones[0].clone(), zones[1].clone()]), vec![(a, b)]);
        assert!(overlapping_zones(&[zones[0].clone(), zones[2].clone()]).is_empty());
    }

    #[test]
    fn room_fallback_labels() {
        let (mut d, l1, _) = doc_with_levels();
        let z = d.place_zone(l1, [0.0, 0.0], [10.0, 10.0], "").unwrap();
        let zc = d.zones()[0].clone();
        assert_eq!(room_fallback_label(&d, &zc), None);
        place_room_element(&mut d, l1, [5.0, 5.0], "Room 101");
        assert_eq!(room_fallback_label(&d, &zc), Some("Room 101".to_string()));
        assert_eq!(zone_display_label(&d, &zc), "Room 101");
        // a labelled zone keeps its own label
        let z2 = d.place_zone(l1, [20.0, 20.0], [30.0, 30.0], "Risers").unwrap();
        let z2c = d.zones().iter().find(|z| z.id == z2).unwrap().clone();
        assert_eq!(zone_display_label(&d, &z2c), "Risers");
        // a zone with no room inside falls back to its number
        let z3 = d.place_zone(l1, [40.0, 40.0], [50.0, 50.0], "").unwrap();
        let z3c = d.zones().iter().find(|z| z.id == z3).unwrap().clone();
        assert_eq!(room_fallback_label(&d, &z3c), None);
        assert_eq!(zone_display_label(&d, &z3c), format!("Zone {}", z3.0 - ZONE_BASE + 1));
        let _ = z;
    }

    #[test]
    fn extract_electrical_pulls_boards_only() {
        let (mut d, l1, l2) = doc_with_levels();
        let base = crate::elec::NODE_BASE;
        set_mep(&mut d, |m| {
            m.elec_nodes = vec![
                ElecNode::new(base, "TX1", ElecKind::Source, [0.0, 0.0], l1),
                ElecNode::new(base + 1, "MSB", ElecKind::Board, [10.0, 0.0], l1),
                ElecNode::new(base + 2, "DB-2", ElecKind::Board, [0.0, 5.0], l2),
                ElecNode::new(base + 3, "CH-1", ElecKind::Load, [1.0, 1.0], l1),
            ];
        });
        let draft = extract_schematic(&d, Discipline::Electrical);
        assert_eq!(draft.nodes.len(), 2);
        assert!(draft.nodes.iter().all(|n| n.name == "MSB" || n.name == "DB-2"));
        assert_eq!(draft.levels.len(), 2);
        assert_eq!(draft.levels[0].name, "Level 1"); // sorted by elevation
        assert_eq!(draft.levels[1].elevation_m, 3.0);
        // ELV has no model yet: empty, not an error
        assert!(extract_schematic(&d, Discipline::Elv).nodes.is_empty());
    }

    #[test]
    fn extract_plumbing_groups_endpoints_by_zone() {
        let (mut d, l1, _) = doc_with_levels();
        let r1 = place_pipe(&mut d, l1, [1.0, 1.0], [2.0, 1.0]);
        let r2 = place_pipe(&mut d, l1, [3.0, 3.0], [4.0, 3.0]);
        let r3 = place_pipe(&mut d, l1, [8.0, 8.0], [9.0, 8.0]);
        let r4 = place_pipe(&mut d, l1, [20.0, 20.0], [21.0, 20.0]);
        let za = d.place_zone(l1, [0.0, 0.0], [5.0, 5.0], "Wet").unwrap();
        let zb = d.place_zone(l1, [6.0, 6.0], [10.0, 10.0], "Other").unwrap();
        set_mep(&mut d, |m| {
            m.run_systems = vec![
                (r1, System::Sanitary),
                (r2, System::Sanitary),
                (r3, System::Sanitary),
                (r4, System::ChilledWaterSupply),
            ];
            m.fittings = vec![fitting(FITTING_BASE, 2)];
            m.endpoints = vec![
                end(run_port(r1, 1), EndDemand::Flow(0.001)),
                end(run_port(r2, 0), EndDemand::Flow(0.002)),
                end(run_port(r3, 1), EndDemand::Sprinkler { k_l_min_bar: 80.0, min_flow_l_min: 60.0 }),
                end(run_port(r4, 1), EndDemand::Flow(0.5)),
                MepEndpoint {
                    port: MepPort { node: MepNode::Fitting(FITTING_BASE), port: 0 },
                    role: EndRole::Terminal,
                    demand: EndDemand::Flow(0.001),
                    required_pressure_pa: 0.0,
                },
            ];
        });
        let draft = extract_schematic(&d, Discipline::Plumbing);
        assert_eq!(draft.unplaced_endpoints, 1); // the fitting port has no plan position
        assert_eq!(draft.groups.len(), 2);
        let ga = draft.groups.iter().find(|g| g.zone == Some(za)).unwrap();
        assert_eq!(ga.endpoints, 2);
        assert!((ga.flow_m3s - 0.003).abs() < 1e-12);
        assert_eq!(ga.zone_label, "Wet");
        assert_eq!(ga.system, System::Sanitary);
        let gb = draft.groups.iter().find(|g| g.zone == Some(zb)).unwrap();
        assert_eq!(gb.endpoints, 1);
        assert!((gb.flow_m3s - 0.001).abs() < 1e-12); // 60 l/min = 0.001 m3/s
        // the chilled-water end point is not in the plumbing draft; it is grouped apart in its own
        let cw = extract_schematic(&d, Discipline::ChilledWater);
        assert_eq!(cw.groups.len(), 1);
        assert_eq!(cw.groups[0].zone, None);
        assert!((cw.groups[0].flow_m3s - 0.5).abs() < 1e-12);
    }

    #[test]
    fn ambiguous_endpoint_assigned_first_reported() {
        let (mut d, l1, _) = doc_with_levels();
        let r = place_pipe(&mut d, l1, [1.0, 1.0], [2.0, 1.0]);
        let za = d.place_zone(l1, [0.0, 0.0], [5.0, 5.0], "A").unwrap();
        let zb = d.place_zone(l1, [1.0, 1.0], [6.0, 6.0], "B").unwrap();
        set_mep(&mut d, |m| {
            m.run_systems = vec![(r, System::DomesticCold)];
            m.endpoints = vec![end(run_port(r, 1), EndDemand::Flow(0.001))]; // end (2,1): inside both
        });
        let draft = extract_schematic(&d, Discipline::Plumbing);
        assert_eq!(draft.ambiguities.len(), 1);
        assert_eq!(draft.ambiguities[0].assigned, za); // first by id = creation order
        assert_eq!(draft.ambiguities[0].zones, vec![za, zb]);
        assert_eq!(draft.groups.len(), 1);
        assert_eq!(draft.groups[0].zone, Some(za));
    }

    #[test]
    fn discipline_of_system_mapping() {
        use System::*;
        assert_eq!(Discipline::of_system(Power), Some(Discipline::Electrical));
        assert_eq!(Discipline::of_system(SupplyAir), Some(Discipline::AirDistribution));
        assert_eq!(Discipline::of_system(ChilledWaterSupply), Some(Discipline::ChilledWater));
        assert_eq!(Discipline::of_system(FireProtection), Some(Discipline::Sprinkler));
        assert_eq!(Discipline::of_system(Sanitary), Some(Discipline::Plumbing));
        assert_eq!(Discipline::of_system(Gas), Some(Discipline::Plumbing));
        assert_eq!(Discipline::of_system(Other), None);
        assert_eq!(Discipline::Electrical.family(), Family::Electrical);
        assert_eq!(Discipline::Bms.family(), Family::Electrical);
        assert_eq!(Discipline::Sprinkler.family(), Family::HydraulicAir);
    }

    #[test]
    fn draft_json_round_trip() {
        let (mut d, l1, _) = doc_with_levels();
        let r = place_pipe(&mut d, l1, [1.0, 1.0], [2.0, 1.0]);
        d.place_zone(l1, [0.0, 0.0], [5.0, 5.0], "Wet").unwrap();
        set_mep(&mut d, |m| {
            m.run_systems = vec![(r, System::Sanitary)];
            m.endpoints = vec![end(run_port(r, 1), EndDemand::Flow(0.001))];
        });
        let draft = extract_schematic(&d, Discipline::Plumbing);
        let json = draft.to_json().unwrap();
        let back = SchematicDraft::from_json(&json).unwrap();
        assert_eq!(draft, back);
        assert!(json.contains("\"flow_m3s\": 0.001"));
    }

    #[test]
    fn zone_records_round_trip() {
        let (mut d, l1, l2) = doc_with_levels();
        d.place_zone(l1, [0.0, 0.0], [5.0, 5.0], "Wet").unwrap();
        d.place_zone(l2, [10.0, 10.0], [20.0, 20.0], "").unwrap();
        let lines = crate::native::mep_records(d.mep());
        let parsed = crate::native::parse_mep(&lines).unwrap();
        assert_eq!(parsed.zones, d.mep().zones);
        // and through the whole document record set
        let all = crate::native::records(&d);
        assert!(all.iter().any(|l| l.starts_with("X|")));
        let parsed2 = crate::native::parse_mep(&all).unwrap();
        assert_eq!(parsed2.zones, d.mep().zones);
    }
}
