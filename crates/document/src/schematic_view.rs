//! The single-line schematic view over the model (doc 06, SCH-003).
//!
//! The generator reads a `SchematicDraft` (the SCH-001 extraction, which already carries its
//! connectivity) and the SCH-002 grid layout, and produces a `SchematicView`: one symbol per laid-out
//! slot, wires taken from the layout's connectivity, and the per-discipline annotation matrix of 06
//! as fixture/reference labels.
//!
//! The draft is read-only here; this module never mutates the design model. The view is undoable
//! via [`Command::GenerateSchematic`]: it is regenerated from the model on every call, so an edit
//! then a regeneration reflects the edit, and undo restores the previous single-line.
//!
//! Values for the mandatory annotations come from the draft only and are placeholder labels in this
//! phase — the calculated electrical results live in edesign-alike and are imported by SCH-006; the
//! hydraulic/airflow pressure drops by MEP-002. Sheet rendering (PDF/DXF) and ribbon binding land in
//! DOC-001; the OpenBIM Graph command wiring and the rectangle-drag zone tool are GUI-004.
use crate::schematic::{group_element_id, Discipline, DraftGroup, DraftNode, SchematicDraft};
use crate::{Command, Document, EditError};
use openbim_schematic::layout::{
    Layout, LayoutElement, LayoutInput, LayoutLevel, SpacingMode, DEFAULT_COLUMN_PITCH,
    DEFAULT_MIN_SLOT, DEFAULT_ROW_PITCH,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One row of the single-line grid: a level of the model, ordered the way the grid orders levels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagramRow {
    pub level: crate::ElementId,
    pub label: String,
    pub elevation_m: f64,
    pub row: usize,
}

/// The shape drawn for a symbol, IEC 60617-style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// an electrical board / switchboard
    Board,
    /// a hydraulic group box (one per zone/system the draft summarized)
    Group,
}

/// One symbol on the single-line: what it is, its label, and the mandatory annotations under it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub element: u64,
    pub kind: SymbolKind,
    pub label: String,
    /// the mandatory annotations for the discipline, in the 06 matrix order. The strings are fixture
    /// labels carrying the unit — values are not calculated in this phase.
    pub annotations: Vec<String>,
}

/// A wire on the drawn single-line: the two symbols it joins and the bends between them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderedWire {
    pub a: u64,
    pub b: u64,
    pub bends: Vec<[f64; 2]>,
}

/// The finished single-line view over one draft: the rows, the symbols, the wires, and the warnings
/// the extraction and the zone check produced, carried through so the sheet can surface them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchematicView {
    pub discipline: Discipline,
    pub rows: Vec<DiagramRow>,
    pub symbols: Vec<Symbol>,
    pub wires: Vec<RenderedWire>,
    pub warnings: Vec<String>,
    /// whether the view carries results imported from a calculation (SCH-006 electrical currents,
    /// MEP-002 pressure drops) that the solver marks `not_approved` — so the sheet can warn the
    /// designer that the displayed values are preliminary. Defaults to false for views that carry no
    /// imported values (the SCH-003 fixture labels), and is kept false for the empty model.
    #[serde(default)]
    pub not_approved: bool,
}

/// Width in metres of sheet space given to every symbol on the grid (10 mm); uniform is the default
/// per doc 06, and the symbol footprint is the same for every discipline.
const SYMBOL_FOOTPRINT: f64 = 0.01;

/// The mandatory annotation labels per discipline, including units (06, annotation matrix). Electrical
/// family = current (A), voltage drop (V), voltage drop (%); hydraulic/air = flow, pressure drop.
fn annotation_labels(d: Discipline) -> Vec<String> {
    let labels: &[&str] = match d {
        Discipline::Electrical | Discipline::Elv | Discipline::AccessControl | Discipline::Bms => {
            &["A —", "V —", "% —"]
        }
        Discipline::Plumbing | Discipline::ChilledWater => &["L/s —", "kPa —"],
        Discipline::AirDistribution => &["m³/h —", "Pa —"],
        Discipline::Sprinkler => &["L/s —", "kPa —"],
    };
    labels.iter().map(|s| s.to_string()).collect()
}

/// The label a group symbol shows: its own, else a placeholder.
fn group_label(g: &DraftGroup) -> String {
    if !g.zone_label.is_empty() {
        g.zone_label.clone()
    } else {
        "Group".to_string()
    }
}

impl SchematicDraft {
    /// Translate this extraction draft into the input the grid layout engine expects: levels as rows,
    /// nodes (keyed by watts) and groups (keyed by flow) as columns, the draft's connections as the
    /// connectivity to wire. The element id of the n-th group is stable within a draft and cannot
    /// collide with an electrical node id (those start at `NODE_BASE`), so render can match slots back.
    pub fn layout_input(&self, mode: SpacingMode) -> LayoutInput {
        let levels: Vec<LayoutLevel> = self
            .levels
            .iter()
            .map(|l| LayoutLevel {
                id: l.id.0,
                name: l.name.clone(),
                elevation_m: l.elevation_m,
            })
            .collect();
        let mut elements: Vec<LayoutElement> = Vec::new();
        for n in &self.nodes {
            elements.push(LayoutElement {
                id: n.id,
                level: n.level.0,
                key: n.watts,
                footprint: SYMBOL_FOOTPRINT,
            });
        }
        for (i, g) in self.groups.iter().enumerate() {
            elements.push(LayoutElement {
                id: group_element_id(i),
                level: g.level.0,
                key: g.flow_m3s,
                footprint: SYMBOL_FOOTPRINT,
            });
        }
        LayoutInput {
            levels,
            elements,
            connections: self.connections.clone(),
            mode,
            row_pitch: DEFAULT_ROW_PITCH,
            column_pitch: DEFAULT_COLUMN_PITCH,
            min_slot: DEFAULT_MIN_SLOT,
        }
    }

    /// Render this draft (already laid out by SCH-002) into the single-line view: a symbol per slot,
    /// a wire per route, and the discipline annotations. Symbols are ordered row-then-column so the
    /// view matches the grid the engine placed.
    pub fn render(&self, layout: &Layout) -> SchematicView {
        let mut view = SchematicView {
            discipline: self.discipline,
            rows: Vec::new(),
            symbols: Vec::new(),
            wires: Vec::new(),
            warnings: Vec::new(),
            not_approved: false,
        };
        // rows = levels in the order the draft sorted them (elevation, name, id) == the grid's rows
        for (row, l) in self.levels.iter().enumerate() {
            view.rows.push(DiagramRow {
                level: l.id,
                label: l.name.clone(),
                elevation_m: l.elevation_m,
                row,
            });
        }
        let nodes: HashMap<u64, &DraftNode> = self.nodes.iter().map(|n| (n.id, n)).collect();
        let groups: Vec<(u64, &DraftGroup)> = self
            .groups
            .iter()
            .enumerate()
            .map(|(i, g)| (group_element_id(i), g))
            .collect();
        for slot in &layout.slots {
            if let Some(n) = nodes.get(&slot.element) {
                let label = if n.name.is_empty() {
                    format!("Node {}", n.id)
                } else {
                    n.name.clone()
                };
                view.symbols.push(Symbol {
                    element: n.id,
                    kind: SymbolKind::Board,
                    label,
                    annotations: annotation_labels(self.discipline),
                });
            } else if let Some((_, g)) = groups.iter().find(|(id, _)| *id == slot.element) {
                view.symbols.push(Symbol {
                    element: slot.element,
                    kind: SymbolKind::Group,
                    label: group_label(g),
                    annotations: annotation_labels(self.discipline),
                });
            }
        }
        // order symbols the way the grid placed them (row then column), so view and layout agree
        let order: HashMap<u64, usize> = layout
            .slots
            .iter()
            .enumerate()
            .map(|(i, s)| (s.element, i))
            .collect();
        view.symbols
            .sort_by_key(|s| order.get(&s.element).copied().unwrap_or(usize::MAX));
        // the bends the grid computed are the drawn path: straight within a row, an elbow across rows
        for w in &layout.wires {
            view.wires.push(RenderedWire {
                a: w.a,
                b: w.b,
                bends: w.via.clone(),
            });
        }
        for a in &self.ambiguities {
            view.warnings.push(format!(
                "endpoint at ({}, {}) assigned to one of {} zones",
                a.at[0],
                a.at[1],
                a.zones.len()
            ));
        }
        if self.unplaced_endpoints > 0 {
            view.warnings.push(format!(
                "{} end point(s) on a fitting with no plan position",
                self.unplaced_endpoints
            ));
        }
        view
    }
}

impl Document {
    /// The single-line schematic views generated from this model (SCH-003).
    pub fn schematic_views(&self) -> &[crate::SchematicView] {
        &self.mep().schematics
    }

    /// Generate the single-line schematic for `discipline` in `mode` as one undoable command. The view
    /// is carried by the command: applying it stores the view for the sheet to read, undoing it takes
    /// the view back and regenerating after an edit reflects the latest model.
    pub fn generate_schematic_commands(
        &self,
        discipline: Discipline,
        mode: SpacingMode,
    ) -> Result<Vec<Command>, EditError> {
        let draft = crate::schematic::extract_schematic(self, discipline);
        let input = draft.layout_input(mode);
        let mut view = draft.render(&openbim_schematic::layout::layout(&input));
        if discipline.family() == crate::schematic::Family::Electrical {
            crate::schematic_electrical::stamp(self.mep(), self, &mut view);
        }
        for (a, b) in crate::schematic::overlapping_zones(self.zones()) {
            view.warnings
                .push(format!("overlapping zones on a level: {a:?} and {b:?}"));
        }
        Ok(vec![Command::GenerateSchematic {
            diagram: Box::new(view),
        }])
    }

    /// Like `generate_schematic_commands` but yields the view directly, in the uniform spacing mode.
    pub fn generate_schematic(
        &self,
        discipline: Discipline,
    ) -> Result<crate::SchematicView, EditError> {
        let cmds = self.generate_schematic_commands(discipline, SpacingMode::Uniform)?;
        match &cmds[0] {
            Command::GenerateSchematic { diagram } => Ok((**diagram).clone()),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{Discipline, DraftGroup, DraftLevel, DraftNode, SchematicDraft};
    use crate::{Document, ElementId};
    use openbim_schematic::layout::{layout, Connection, SpacingMode};

    fn draft(discipline: Discipline) -> SchematicDraft {
        SchematicDraft {
            discipline,
            levels: vec![
                DraftLevel {
                    id: ElementId(1),
                    name: "Level 1".into(),
                    elevation_m: 0.0,
                },
                DraftLevel {
                    id: ElementId(2),
                    name: "Level 2".into(),
                    elevation_m: 3.0,
                },
            ],
            nodes: match discipline {
                Discipline::Electrical => vec![
                    DraftNode {
                        id: 1,
                        name: "MSB".into(),
                        level: ElementId(1),
                        at: [0.0, 0.0],
                        watts: 100.0,
                    },
                    DraftNode {
                        id: 2,
                        name: "DB-2".into(),
                        level: ElementId(2),
                        at: [1.0, 0.0],
                        watts: 50.0,
                    },
                ],
                _ => Vec::new(),
            },
            groups: match discipline {
                Discipline::Plumbing => vec![DraftGroup {
                    zone: Some(ElementId(crate::schematic::ZONE_BASE)),
                    zone_label: "Wet".into(),
                    level: ElementId(1),
                    system: openbim_family::mep::System::Sanitary,
                    endpoints: 3,
                    flow_m3s: 0.03,
                }],
                _ => Vec::new(),
            },
            ambiguities: Vec::new(),
            unplaced_endpoints: 0,
            connections: match discipline {
                Discipline::Electrical => vec![Connection { a: 1, b: 2 }],
                _ => Vec::new(),
            },
        }
    }

    #[test]
    fn render_orders_symbols_row_then_column_annotates_and_routes_wires() {
        let d = draft(Discipline::Electrical);
        let view = d.render(&layout(&d.layout_input(SpacingMode::Uniform)));
        // the two boards, ordered Level 1 then Level 2 (row order)
        assert_eq!(
            view.symbols.iter().map(|s| s.element).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            view.symbols
                .iter()
                .map(|s| s.label.as_str())
                .collect::<Vec<_>>(),
            vec!["MSB", "DB-2"]
        );
        assert!(view.symbols.iter().all(|s| s.kind == SymbolKind::Board));
        // electrical annotations are current, voltage drop V, voltage drop % (three)
        assert!(view.symbols.iter().all(|s| s.annotations.len() == 3));
        // the cross-row connection is a 2-bend elbow
        assert_eq!(view.wires.len(), 1);
        assert_eq!(view.wires[0].bends.len(), 2);
        assert_eq!(view.rows.len(), 2);
    }

    #[test]
    fn plumbing_symbol_is_a_group_annotated_flow_and_pressure() {
        let d = draft(Discipline::Plumbing);
        let view = d.render(&layout(&d.layout_input(SpacingMode::Uniform)));
        assert_eq!(view.symbols.len(), 1);
        assert_eq!(view.symbols[0].kind, SymbolKind::Group);
        assert_eq!(view.symbols[0].label, "Wet");
        // hydraulic/air annotations are flow and pressure drop (two)
        assert_eq!(view.symbols[0].annotations.len(), 2);
        assert!(view.wires.is_empty());
    }

    #[test]
    fn both_spacing_modes_render_without_changing_the_order() {
        let d = draft(Discipline::Electrical);
        let u = d.render(&layout(&d.layout_input(SpacingMode::Uniform)));
        let f = d.render(&layout(&d.layout_input(SpacingMode::FootprintProportional)));
        assert_eq!(
            u.symbols.iter().map(|s| s.element).collect::<Vec<_>>(),
            f.symbols.iter().map(|s| s.element).collect::<Vec<_>>()
        );
    }

    #[test]
    fn generate_schematic_is_one_undoable_command() {
        let mut d = Document::new();
        // an empty model still builds a (trivial) schematic; the command is applied, undone, redone
        let cmds = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        let rev = d.revision();
        d.transact(rev, &cmds).unwrap();
        assert_eq!(d.schematic_views().len(), 1);
        assert_eq!(d.schematic_views()[0].discipline, Discipline::Electrical);
        assert!(d.schematic_views()[0].symbols.is_empty());
        // no electrical model on an empty document -> no imported values -> not a not_approved view
        assert!(!d.schematic_views()[0].not_approved);
        assert!(d.undo().unwrap());
        assert!(d.schematic_views().is_empty());
        assert!(d.redo().unwrap());
        assert_eq!(d.schematic_views().len(), 1);
    }

    #[test]
    fn generate_schematic_stamps_electrical_current_and_marks_not_approved() {
        use crate::elec::NODE_BASE;
        use crate::{Command, ElecCable, ElecKind, ElecNode, ElementId, MepData};
        let mut d = Document::spatial_sample();
        let (a, b, c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
        let mut chiller = ElecNode::new(c, "Chiller", ElecKind::Load, [20.0, 10.0], ElementId(10));
        chiller.watts = 45000.0;
        chiller.power_factor = 0.85;
        let mut feeder = ElecCable::new(b, c, true);
        feeder.via = vec![[20.0, 0.0]];
        let data = MepData {
            elec_nodes: vec![
                ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)),
                ElecNode::new(b, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10)),
                chiller,
            ],
            elec_cables: vec![ElecCable::new(a, b, true), feeder],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))])
            .unwrap();
        let cmds = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        d.transact(d.revision(), &cmds).unwrap();
        assert_eq!(d.schematic_views().len(), 1);
        let view = &d.schematic_views()[0];
        // only boards are single-line symbols (TX1 is a source, the chiller is a load)
        assert_eq!(view.symbols.len(), 1, "only the MSB board is a symbol");
        let msb = view.symbols.iter().find(|s| s.element == b).unwrap();
        assert_eq!(msb.label, "MSB");
        // the feeder current is the chiller load current: Ib = P / (sqrt(3) * V * pf)
        let a_slot = &msb.annotations[0];
        assert!(a_slot.starts_with("A "), "{a_slot}");
        let value: f64 = a_slot.strip_prefix("A ").unwrap().parse().unwrap();
        let expected = 45000.0 / (3f64.sqrt() * 400.0 * 0.85);
        assert!(
            (value - expected).abs() < 1e-2,
            "stamped {value} vs expected {expected}"
        );
        // MEP-002 stamps voltage drop (V, %); with the prototype impedance the drop is ~0, but the
        // slots are computed, not placeholders.
        assert_ne!(msb.annotations[1], "V —", "V slot should be stamped");
        let v_val: f64 = msb.annotations[1]
            .strip_prefix("V ")
            .unwrap()
            .parse()
            .unwrap();
        assert!(v_val.abs() < 1e-3, "stiff-source drop ~0 V, got {v_val}");
        assert_ne!(msb.annotations[2], "% —", "% slot should be stamped");
        let pct_val: f64 = msb.annotations[2]
            .strip_prefix("% ")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            pct_val.abs() < 1e-3,
            "stiff-source drop ~0 %, got {pct_val}"
        );
        // every imported value is preliminary
        assert!(view.not_approved);
        assert!(view.warnings.iter().any(|w| w.contains("MEP-010")));
        assert!(view.warnings.iter().any(|w| w.contains("MEP-002")));
        assert!(view.warnings.iter().any(|w| w.contains("not_approved")));
        // undo drops the view; redo regenerates it from the model with the stamped values
        assert!(d.undo().unwrap());
        assert!(d.schematic_views().is_empty());
        assert!(d.redo().unwrap());
        let restored = &d.schematic_views()[0];
        assert!(restored.not_approved);
        let r = restored.symbols.iter().find(|s| s.element == b).unwrap();
        assert!(r.annotations[0].starts_with("A "), "{}", r.annotations[0]);
        // redo regenerates the stamp from the live model rather than replaying a snapshot
        let value: f64 = r.annotations[0]
            .strip_prefix("A ")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (value - expected).abs() < 1e-2,
            "redo regenerated {value} vs expected {expected}"
        );
    }

    #[test]
    fn redo_regenerates_the_electrical_stamp_from_the_model() {
        use crate::elec::NODE_BASE;
        use crate::{Command, ElecCable, ElecKind, ElecNode, ElementId, MepData};
        let mut d = Document::spatial_sample();
        let (a, b, _c) = (NODE_BASE, NODE_BASE + 1, NODE_BASE + 2);
        let mut chiller = ElecNode::new(
            NODE_BASE + 2,
            "Chiller",
            ElecKind::Load,
            [20.0, 10.0],
            ElementId(10),
        );
        chiller.watts = 45000.0;
        chiller.power_factor = 0.85;
        let data = MepData {
            elec_nodes: vec![
                ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)),
                ElecNode::new(b, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10)),
                chiller,
            ],
            elec_cables: vec![
                ElecCable::new(a, b, true),
                ElecCable::new(b, NODE_BASE + 2, true),
            ],
            ..Default::default()
        };
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))])
            .unwrap();
        let mut cmds = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        // Corrupt the annotation carried on the command; apply must regenerate it from the model.
        if let Command::GenerateSchematic { diagram } = &mut cmds[0] {
            diagram.symbols[0].annotations[0] = "A snapshot-marker".into();
        }
        d.transact(d.revision(), &cmds).unwrap();
        let expected = 45000.0 / (3f64.sqrt() * 400.0 * 0.85);
        // Apply regenerates the stamp, ignoring the corrupted annotation on the command.
        assert_ne!(
            d.schematic_views()[0].symbols[0].annotations[0],
            "A snapshot-marker"
        );
        let v: f64 = d.schematic_views()[0].symbols[0].annotations[0]
            .strip_prefix("A ")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (v - expected).abs() < 1e-2,
            "apply {v} vs expected {expected}"
        );
        // Undo then redo must also regenerate from the model, not replay the corrupted snapshot.
        assert!(d.undo().unwrap());
        assert!(d.schematic_views().is_empty());
        assert!(d.redo().unwrap());
        assert_ne!(
            d.schematic_views()[0].symbols[0].annotations[0],
            "A snapshot-marker"
        );
        let v: f64 = d.schematic_views()[0].symbols[0].annotations[0]
            .strip_prefix("A ")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (v - expected).abs() < 1e-2,
            "redo {v} vs expected {expected}"
        );
    }

    #[test]
    fn electrical_stamp_handles_board_local_load_phase_and_rejections() {
        let three = 45000.0 / (3f64.sqrt() * 400.0 * 0.85);
        let one = 45000.0 / ((400.0 / 3f64.sqrt()) * 0.85);
        // downstream 45 kW load on a 3-phase feeder (golden)
        let v = elec_doc(0.0, 45000.0, true, true, "Chiller")
            .generate_schematic(Discipline::Electrical)
            .unwrap();
        assert_current(&v.symbols[0].annotations[0], three);
        assert!(v.not_approved);
        // board's own local 45 kW load (downstream load zero) is still stamped from the board's load
        let v = elec_doc(45000.0, 0.0, true, true, "Chiller")
            .generate_schematic(Discipline::Electrical)
            .unwrap();
        assert_current(&v.symbols[0].annotations[0], three);
        // all single-phase: phase voltage is 400/sqrt(3) line-to-neutral, not 400 line-to-line
        let v = elec_doc(0.0, 45000.0, false, false, "Chiller")
            .generate_schematic(Discipline::Electrical)
            .unwrap();
        assert_current(&v.symbols[0].annotations[0], one);
        // mixed-phase (3-phase feeder into 1-phase load) is rejected, not silently stamped 0 A
        let v = elec_doc(0.0, 45000.0, true, false, "Chiller")
            .generate_schematic(Discipline::Electrical)
            .unwrap();
        assert_eq!(v.symbols[0].annotations[0], "A —");
        assert!(v.warnings.iter().any(|w| w.contains("mixed-phase")));
        // an empty load name is rejected by the solver graph, not silently stamped 0 A
        let v = elec_doc(0.0, 45000.0, true, true, "")
            .generate_schematic(Discipline::Electrical)
            .unwrap();
        assert_eq!(v.symbols[0].annotations[0], "A —");
        assert!(v.warnings.iter().any(|w| w.contains("rejected")));
    }

    fn elec_doc(
        board_w: f64,
        load_w: f64,
        feeder_three: bool,
        branch_three: bool,
        name: &str,
    ) -> Document {
        use crate::{Command, ElecCable, ElecKind, ElecNode, MepData};
        let a = crate::elec::NODE_BASE;
        let b = a + 1;
        let c = a + 2;
        let mut board = ElecNode::new(b, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10));
        board.watts = board_w;
        board.power_factor = 0.85;
        let mut load = ElecNode::new(c, name, ElecKind::Load, [20.0, 10.0], ElementId(10));
        load.watts = load_w;
        load.power_factor = 0.85;
        let data = MepData {
            elec_nodes: vec![
                ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)),
                board,
                load,
            ],
            elec_cables: vec![
                ElecCable::new(a, b, feeder_three),
                ElecCable::new(b, c, branch_three),
            ],
            ..Default::default()
        };
        let mut d = Document::spatial_sample();
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))])
            .unwrap();
        d
    }

    fn assert_current(a: &str, expected: f64) {
        assert!(a.starts_with("A "), "got {a}");
        let value: f64 = a.strip_prefix("A ").unwrap().parse().unwrap();
        assert!(
            (value - expected).abs() < 1e-2,
            "stamped {value} vs expected {expected}"
        );
    }

    #[test]
    fn batch_setmep_then_generate_stamps_the_staged_model() {
        // A batched SetMep + GenerateSchematic must stamp the staged model, not the
        // pre-transaction document (lib.rs apply arm).
        let mut d = elec_doc(0.0, 45000.0, true, true, "Chiller");
        let cmds = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        let staged = elec_doc(0.0, 90000.0, true, true, "Chiller").mep().clone();
        d.transact(
            d.revision(),
            &[crate::Command::SetMep(Box::new(staged)), cmds[0].clone()],
        )
        .unwrap();
        assert_current(
            &d.schematic_views()[0].symbols[0].annotations[0],
            90000.0 / (3f64.sqrt() * 400.0 * 0.85),
        );
    }

    #[test]
    fn restamp_after_graph_rejection_clears_stale_current() {
        // A retained GenerateSchematic applied to a rejected graph must reset stale currents to
        // placeholders rather than leaving the prior numeric value.
        let mut d = elec_doc(0.0, 45000.0, true, true, "Chiller");
        let original = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        let rejected = elec_doc(0.0, 45000.0, true, true, "").mep().clone();
        d.transact(d.revision(), &[crate::Command::SetMep(Box::new(rejected))])
            .unwrap();
        d.transact(d.revision(), &original).unwrap();
        assert_eq!(d.schematic_views()[0].symbols[0].annotations[0], "A —");
        assert!(d.schematic_views()[0]
            .warnings
            .iter()
            .any(|w| w.contains("rejected")));
    }

    #[test]
    fn redo_of_stored_view_regenerates_the_stamp() {
        // A view stored (via SetMep) with a stale "A" annotation must be regenerated, not replayed,
        // on redo.
        let mut d = elec_doc(0.0, 45000.0, true, true, "Chiller");
        let mut m = d.mep().clone();
        let mut v = d.generate_schematic(Discipline::Electrical).unwrap();
        v.symbols[0].annotations[0] = "A stale-marker".into();
        m.schematics.push(v);
        d.transact(d.revision(), &[crate::Command::SetMep(Box::new(m))])
            .unwrap();
        d.undo().unwrap();
        d.redo().unwrap();
        let annotations = &d.schematic_views()[0].symbols[0].annotations[0];
        assert_ne!(annotations, "A stale-marker");
        assert_current(annotations, 45000.0 / (3f64.sqrt() * 400.0 * 0.85));
    }

    #[test]
    fn generate_schematic_with_empty_annotations_does_not_panic() {
        let mut d = elec_doc(0.0, 45000.0, true, true, "Chiller");
        let mut ops = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        if let crate::Command::GenerateSchematic { diagram } = &mut ops[0] {
            diagram.symbols[0].annotations.clear();
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            d.transact(d.revision(), &ops)
        }));
        assert!(
            result.is_ok() && result.as_ref().unwrap().is_ok(),
            "re-stamping a view with empty annotations must not panic nor fail"
        );
        let msb = &d.schematic_views()[0].symbols[0];
        assert_eq!(msb.annotations.len(), 3);
        // MEP-002 stamps V/% (not placeholders) even when re-stamping a view whose annotations
        // were deliberately cleared before apply.
        assert_ne!(msb.annotations[1], "V —", "V slot stamped on re-stamp");
        assert_ne!(msb.annotations[2], "% —", "% slot stamped on re-stamp");
        assert_current(&msb.annotations[0], 45000.0 / (3f64.sqrt() * 400.0 * 0.85));
    }

    #[test]
    fn restamp_after_graph_rejection_clears_stale_vdrop() {
        // A retained GenerateSchematic applied to a rejected graph must reset stale voltage-drop
        // annotations to placeholders rather than leaving the prior numeric value.
        let mut d = elec_doc(0.0, 45000.0, true, true, "Chiller");
        let original = d
            .generate_schematic_commands(Discipline::Electrical, SpacingMode::Uniform)
            .unwrap();
        d.transact(d.revision(), &original).unwrap();
        // good graph -> V/% are stamped (not placeholders)
        assert_ne!(d.schematic_views()[0].symbols[0].annotations[1], "V —");
        assert_ne!(d.schematic_views()[0].symbols[0].annotations[2], "% —");
        let rejected = elec_doc(0.0, 45000.0, true, true, "").mep().clone();
        d.transact(d.revision(), &[crate::Command::SetMep(Box::new(rejected))])
            .unwrap();
        // re-applying the original commands over the rejected model stamps the rejected model,
        // so V/% fall back to placeholders
        d.transact(d.revision(), &original).unwrap();
        assert_eq!(d.schematic_views()[0].symbols[0].annotations[1], "V —");
        assert_eq!(d.schematic_views()[0].symbols[0].annotations[2], "% —");
        assert!(d.schematic_views()[0]
            .warnings
            .iter()
            .any(|w| w.contains("rejected")));
    }

    #[test]
    fn redo_of_stored_view_regenerates_the_vdrop() {
        // A view stored (via SetMep) with a stale V/% marker must be regenerated, not replayed, on redo.
        let mut d = elec_doc(0.0, 45000.0, true, true, "Chiller");
        let mut m = d.mep().clone();
        let mut v = d.generate_schematic(Discipline::Electrical).unwrap();
        v.symbols[0].annotations[1] = "V stale-marker".into();
        v.symbols[0].annotations[2] = "% stale-marker".into();
        m.schematics.push(v);
        d.transact(d.revision(), &[crate::Command::SetMep(Box::new(m))])
            .unwrap();
        d.undo().unwrap();
        d.redo().unwrap();
        assert_ne!(
            d.schematic_views()[0].symbols[0].annotations[1],
            "V stale-marker"
        );
        assert_ne!(
            d.schematic_views()[0].symbols[0].annotations[2],
            "% stale-marker"
        );
        // regenerated from the model, so V is stamped (not a placeholder) again
        assert_ne!(d.schematic_views()[0].symbols[0].annotations[1], "V —");
        assert_ne!(d.schematic_views()[0].symbols[0].annotations[2], "% —");
    }
}
