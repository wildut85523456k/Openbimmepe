//! Live electrical values on the single-line schematic (doc 06, SCH-006, MEP-002).
//!
//! The model is read-only here. It is described to the installed [`ElectricalCalc`] engine as an
//! [`ElectricalNetwork`]; the engine's results are stamped onto the board symbols. There is no
//! calculation engine in this crate (clean-room rule): revit-alike's own `compute_elec` (using
//! `openbim-calc`) is a separate, pre-existing sizing tool. The engine of the full build is the
//! `edesign-alike` solver, plugged in by `openbim-electrical-adapter` outside this workspace; the
//! open build has none, so boards keep their placeholders and the view says nothing was calculated.
//!
//! Each feeder result is stamped onto the downstream board symbol's annotations: "A" = design
//! current, "V" = cumulative source-to-board drop in volts, "%" = the same drop in percent. The view
//! is regenerated from the model on every generation, so undo restores the prior single-line.

use crate::electrical_calc::{self, CalcOutcome, ElectricalCalc, ElectricalNetwork, NetworkCable, NetworkNode};
use crate::{Document, ElecCable, ElecNode, MepData, SchematicView, SymbolKind};
use std::collections::HashMap;

/// Plan length of a cable through its via points (`from.at → via… → to.at`) plus the level-to-level
/// height and any extra allowance, in metres — the same route the drawn cable reports measure.
pub(crate) fn cable_length_m(doc: &Document, from: &ElecNode, c: &ElecCable, to: &ElecNode) -> f64 {
    let mut pts = vec![from.at];
    pts.extend_from_slice(&c.via);
    pts.push(to.at);
    let plan: f64 = pts
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum();
    let z = |n: &ElecNode| doc.get(n.level).map_or(0.0, |l| l.elevation_m);
    plan + (z(from) - z(to)).abs() + c.extra_m
}

/// A node is three-phase when any cable at it is three-phase.
fn node_three_phase(n: &ElecNode, mep: &MepData) -> bool {
    mep.elec_cables
        .iter()
        .any(|c| (c.from == n.id || c.to == n.id) && c.three_phase)
}

/// The single-line network of `mep`, as plain data for an engine.
pub(crate) fn network(mep: &MepData, doc: &Document) -> ElectricalNetwork {
    let nodes = mep
        .elec_nodes
        .iter()
        .map(|n| NetworkNode {
            id: n.id,
            name: n.name.clone(),
            kind: n.kind,
            watts: n.watts,
            power_factor: n.power_factor,
            three_phase: node_three_phase(n, mep),
        })
        .collect();
    let node = |id: u64| mep.elec_nodes.iter().find(|n| n.id == id);
    let cables = mep
        .elec_cables
        .iter()
        .enumerate()
        .map(|(index, c)| NetworkCable {
            index,
            from: c.from,
            to: c.to,
            three_phase: c.three_phase,
            length_m: match (node(c.from), node(c.to)) {
                (Some(a), Some(b)) => cable_length_m(doc, a, c, b),
                _ => 0.0,
            },
            r_ohm_per_km: c.r_ohm_per_km,
            x_ohm_per_km: c.x_ohm_per_km,
        })
        .collect();
    ElectricalNetwork { nodes, cables }
}

impl Document {
    /// The model's single-line electrical network, as an engine receives it.
    pub fn electrical_network(&self) -> ElectricalNetwork {
        network(self.mep(), self)
    }
}

/// Stamp the installed engine's results onto the electrical symbols of `view`.
///
/// `mep` is passed in (rather than read from `doc`) so the `GenerateSchematic` apply arm and the
/// redo path can stamp the staged/live model instead of the pre-transaction document.
pub(crate) fn stamp(mep: &MepData, doc: &Document, view: &mut SchematicView) {
    stamp_with(electrical_calc::installed(), mep, doc, view);
}

/// Stamp `calc`'s results onto `view`. Every board's A/V/% slots are reset to placeholders first, so
/// a re-stamp never leaves a stale value behind; a rejected or unavailable calculation leaves them.
pub(crate) fn stamp_with(calc: &dyn ElectricalCalc, mep: &MepData, doc: &Document, view: &mut SchematicView) {
    reset_board_annotations(view);
    if mep.elec_nodes.is_empty() || view.symbols.is_empty() {
        return;
    }
    match calc.calculate(&network(mep, doc)) {
        CalcOutcome::Unavailable { reason } => {
            // nothing was imported, so nothing is preliminary: the view only says why it is blank
            if !view.warnings.contains(&reason) {
                view.warnings.push(reason);
            }
        }
        CalcOutcome::Rejected { reasons } => {
            view.warnings.extend(reasons);
            view.not_approved = true;
        }
        CalcOutcome::Solved { feeders, warnings } => {
            let current: HashMap<u64, f64> = feeders.iter().map(|f| (f.to, f.design_current_a)).collect();
            let drop: HashMap<u64, (f64, f64)> = feeders.iter().map(|f| (f.to, (f.drop_v, f.drop_pct))).collect();
            stamp_symbols(view, &current);
            stamp_vdrop_symbols(view, &drop);
            for w in warnings {
                if !view.warnings.contains(&w) {
                    view.warnings.push(w);
                }
            }
            // imported values are preliminary until engineering sign-off
            view.not_approved = true;
        }
    }
}

/// Restore the mandatory A/V/% placeholder triple on every board symbol. Strips stale imported
/// currents and drops before a re-stamp (e.g. after a rejected graph or on redo) and guarantees the
/// three annotation slots exist so stamping never indexes an empty annotations vector.
fn reset_board_annotations(view: &mut SchematicView) {
    for s in &mut view.symbols {
        if s.kind == SymbolKind::Board {
            if s.annotations.len() < 3 {
                s.annotations.resize(3, String::from("A —"));
            }
            s.annotations[0] = String::from("A —");
            s.annotations[1] = String::from("V —");
            s.annotations[2] = String::from("% —");
        }
    }
}

/// Copy a calculated current (A) into the "A" slot of each board symbol fed by a cable. A non-finite
/// value is never written: it leaves the placeholder.
fn stamp_symbols(view: &mut SchematicView, stamped: &HashMap<u64, f64>) {
    for s in &mut view.symbols {
        if s.kind == SymbolKind::Board {
            if let Some(&current) = stamped.get(&s.element) {
                if current.is_finite() {
                    s.annotations[0] = format!("A {:.3}", current);
                }
            }
        }
    }
}

/// Copy a calculated voltage drop (V, %) into the "V"/"%" slots of each board symbol fed by a
/// cable. Boards with no downstream cable keep their placeholders. A non-finite pair is never
/// written: it leaves the placeholder, so no NaN/inf string can reach the sheet.
fn stamp_vdrop_symbols(view: &mut SchematicView, vdrop: &HashMap<u64, (f64, f64)>) {
    for s in &mut view.symbols {
        if s.kind == SymbolKind::Board {
            if let Some(&(volts, pct)) = vdrop.get(&s.element) {
                if volts.is_finite() && pct.is_finite() {
                    s.annotations[1] = format!("V {:.1}", volts);
                    s.annotations[2] = format!("% {:.1}", pct);
                }
            }
        }
    }
}

/// A stand-in engine for this crate's tests, which exercise the plumbing (stamping, regeneration on
/// redo, staged models, rejection clearing stale values), not electrical arithmetic. Its numbers are
/// a marker, not a calculation: the current a cable carries is the watts it feeds (its node and
/// everything downstream) / 1000,
/// the drop is `cable length / 100` V and % — so a test can tell which value reached which slot.
/// It rejects a network with an unnamed node, as a real engine does. The real engine's arithmetic is
/// tested in `openbim-electrical-adapter`.
#[cfg(test)]
pub(crate) mod test_engine {
    use super::*;
    use crate::electrical_calc::FeederResult;

    pub(crate) struct Marker;

    impl ElectricalCalc for Marker {
        fn name(&self) -> &str { "test marker" }
        fn calculate(&self, net: &ElectricalNetwork) -> CalcOutcome {
            let unnamed: Vec<String> = net
                .nodes
                .iter()
                .filter(|n| n.name.trim().is_empty())
                .map(|n| format!("electrical node id {}: rejected (no name)", n.id))
                .collect();
            if !unnamed.is_empty() {
                return CalcOutcome::Rejected { reasons: unnamed };
            }
            let feeders = net
                .cables
                .iter()
                .map(|c| {
                    // the watts of the fed node and of everything downstream of it
                    let mut seen = vec![c.to];
                    let mut i = 0;
                    while i < seen.len() {
                        let at = seen[i];
                        for d in net.cables.iter().filter(|d| d.from == at) {
                            if !seen.contains(&d.to) { seen.push(d.to); }
                        }
                        i += 1;
                    }
                    let watts: f64 = net.nodes.iter().filter(|n| seen.contains(&n.id)).map(|n| n.watts).sum();
                    FeederResult { to: c.to, design_current_a: watts / 1000.0, drop_v: c.length_m / 100.0, drop_pct: c.length_m / 100.0 }
                })
                .collect();
            CalcOutcome::Solved { feeders, warnings: vec!["test marker values — not_approved".into()] }
        }
    }

    /// Install the marker engine for this test binary; every test that generates an electrical
    /// schematic through the document calls this first.
    pub(crate) fn install() {
        let _ = crate::electrical_calc::install(Box::new(Marker));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::electrical_calc::{FeederResult, NoSolver, NOT_CALCULATED};

    /// A board symbol carrying the mandatory A/V/% annotation triple (the Electrical label set).
    fn board_view(element: u64) -> SchematicView {
        SchematicView {
            discipline: crate::schematic::Discipline::Electrical,
            rows: Vec::new(),
            symbols: vec![crate::Symbol {
                element,
                kind: SymbolKind::Board,
                label: "MSB".into(),
                annotations: vec!["A —".into(), "V —".into(), "% —".into()],
            }],
            wires: Vec::new(),
            warnings: Vec::new(),
            not_approved: false,
        }
    }

    /// TX1 -> MSB -> DB2, both cables three-phase, 45 kW at DB2.
    fn chain() -> (Document, MepData) {
        use crate::{Command, ElecKind, ElementId};
        let a = crate::elec::NODE_BASE;
        let mut db2 = ElecNode::new(a + 2, "DB2", ElecKind::Board, [20.0, 0.0], ElementId(10));
        db2.watts = 45_000.0;
        db2.power_factor = 0.85;
        let mut feeder = ElecCable::new(a + 1, a + 2, true);
        feeder.extra_m = 10.0;
        feeder.r_ohm_per_km = Some(0.5);
        let data = MepData {
            elec_nodes: vec![
                ElecNode::new(a, "TX1", ElecKind::Source, [0.0, 0.0], ElementId(10)),
                ElecNode::new(a + 1, "MSB", ElecKind::Board, [10.0, 0.0], ElementId(10)),
                db2,
            ],
            elec_cables: vec![ElecCable::new(a, a + 1, true), feeder],
            ..Default::default()
        };
        let mut d = Document::spatial_sample();
        d.transact(d.revision(), &[Command::SetMep(Box::new(data))]).unwrap();
        let mep = d.mep().clone();
        (d, mep)
    }

    #[test]
    fn the_network_describes_the_model_without_calculating() {
        let (doc, mep) = chain();
        let net = network(&mep, &doc);
        assert_eq!(net.nodes.len(), 3);
        assert!(net.nodes.iter().all(|n| n.three_phase), "every node has a three-phase cable");
        let db2 = &net.nodes[2];
        assert_eq!((db2.name.as_str(), db2.watts, db2.power_factor), ("DB2", 45_000.0, 0.85));
        let feeder = &net.cables[1];
        assert_eq!((feeder.index, feeder.from, feeder.to), (1, mep.elec_nodes[1].id, mep.elec_nodes[2].id));
        // 10 m in plan on one level plus the 10 m allowance
        assert!((feeder.length_m - 20.0).abs() < 1e-9, "{}", feeder.length_m);
        assert_eq!((feeder.r_ohm_per_km, feeder.x_ohm_per_km), (Some(0.5), None));
        // a cable to an unknown node is still described, for the engine to reject
        let mut broken = mep.clone();
        broken.elec_cables.push(ElecCable::new(mep.elec_nodes[0].id, 42, true));
        let net = network(&broken, &doc);
        assert_eq!((net.cables[2].to, net.cables[2].length_m), (42, 0.0));
    }

    #[test]
    fn without_a_solver_the_boards_keep_their_placeholders_and_say_why() {
        let (doc, mep) = chain();
        let mut view = board_view(mep.elec_nodes[2].id);
        view.symbols[0].annotations[0] = "A stale".into();
        stamp_with(&NoSolver, &mep, &doc, &mut view);
        assert_eq!(view.symbols[0].annotations, vec!["A —", "V —", "% —"], "nothing is invented");
        assert_eq!(view.warnings, vec![NOT_CALCULATED.to_string()]);
        assert!(!view.not_approved, "nothing was imported, so nothing is preliminary");
        // regenerating does not pile the warning up
        stamp_with(&NoSolver, &mep, &doc, &mut view);
        assert_eq!(view.warnings.len(), 1);
    }

    #[test]
    fn a_solved_network_stamps_the_board_its_feeder_reaches() {
        let (doc, mep) = chain();
        let db2 = mep.elec_nodes[2].id;
        let mut view = board_view(db2);
        stamp_with(&test_engine::Marker, &mep, &doc, &mut view);
        assert_eq!(view.symbols[0].annotations, vec!["A 45.000", "V 0.2", "% 0.2"]);
        assert!(view.not_approved);
        assert!(view.warnings.iter().any(|w| w.contains("test marker")));
    }

    #[test]
    fn a_rejected_network_leaves_placeholders() {
        let (doc, mut mep) = chain();
        mep.elec_nodes[2].name = " ".into();
        let mut view = board_view(mep.elec_nodes[2].id);
        view.symbols[0].annotations[0] = "A stale".into();
        stamp_with(&test_engine::Marker, &mep, &doc, &mut view);
        assert_eq!(view.symbols[0].annotations[0], "A —");
        assert!(view.not_approved && view.warnings.iter().any(|w| w.contains("rejected")));
    }

    #[test]
    fn a_non_finite_value_never_reaches_the_annotation() {
        struct Broken;
        impl ElectricalCalc for Broken {
            fn name(&self) -> &str { "broken" }
            fn calculate(&self, net: &ElectricalNetwork) -> CalcOutcome {
                let to = net.cables[1].to;
                CalcOutcome::Solved { feeders: vec![FeederResult { to, design_current_a: f64::NAN, drop_v: f64::INFINITY, drop_pct: f64::NAN }], warnings: vec![] }
            }
        }
        let (doc, mep) = chain();
        let mut view = board_view(mep.elec_nodes[2].id);
        stamp_with(&Broken, &mep, &doc, &mut view);
        assert_eq!(view.symbols[0].annotations, vec!["A —", "V —", "% —"]);
    }
}
