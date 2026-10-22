//! Live electrical values on the single-line schematic (doc 06, SCH-006).
//!
//! Imports calculated electrical results from `edesign-alike`'s `openbim-electrical-model` graph
//! and `openbim-electrical-solver` calculation (pinned at 57ef01848860771d29960dcf2e8af2f831fe1e6c).
//! There is no parallel calculation engine here: revit-alike's own `compute_elec` (using `openbim-calc`)
//! is a separate, pre-existing sizing tool, and the clean-room rule forbids duplicating a calculation
//! engine. SCH-006 reads the edesign-alike solver results only.
//!
//! SCH-006 stamps feeder design current (A); MEP-002 stamps voltage drop (V, %). Both read the
//! edesign-alike solver results only — no calculation engine is duplicated here. The import stamps
//! each ElecCable's results onto the downstream board symbol's annotations: "A" = design current,
//! "V" = volts dropped at the board (`nominal * path_percent/100`, the cumulative source-to-board
//! drop), "%" = the cumulative drop in percent. Voltage-drop values come straight from the solver's
//! `path_voltage_drop_pct` (cumulative source-to-node, per-kilometre R/X summed along the path),
//! read-only — `branch_voltage_drop_pct` (the feeder's own drop) is not used, because a board's
//! annotation must report the drop seen at that board.
//!
//! Every imported value is `not_approved` — the solver's `approval` is always `"not_approved"` (it
//! computes values and findings only; engineering sign-off is a separate step) — and the cable
//! impedance is a prototype stub (the stiff-source approximation) pending the breaker-catalog import
//! of MEP-010, so each view carries a warning that the values are preliminary. The model is read-only
//! here; the view is regenerated from the model on every generation, so undo restores the prior single-line.

use crate::{Document, ElecCable, ElecKind, ElecNode, MepData, SchematicView, SymbolKind};
use openbim_electrical_model::{
    Branch, BranchId, BranchKind, ConductorImpedance, EarthingSystem, Load, Network, Node, NodeId,
    NodeKind, PhaseConfiguration, Scenario, ScenarioId, Source,
};
use openbim_electrical_solver::{solve, Severity, SolverSettings};
use std::collections::HashMap;

/// Prototype nominal system voltage, line-to-line (V) for a three-phase LV system. `MepData` does
/// not yet persist the LV system voltage (revit-alike sources it externally in `ElecComputeSettings`);
/// the real value is read from the source conductor's family connector (`Size::Electrical { voltage }`)
/// once MEP-010 wires up the catalog. Until then this stiff-source nominal is used and flagged
/// `not_approved`. The solver treats a single-phase node's `nominal_voltage_v` as line-to-neutral,
/// so single-phase nodes are fed `NOMINAL_VOLTAGE_V / sqrt(3)` to preserve the 400 V line-to-line
/// system invariant (400 V LL -> ~231 V LN).
const NOMINAL_VOLTAGE_V: f64 = 400.0;
const SQRT_3: f64 = 1.7320508075688772;

/// Prototype per-kilometre cable impedance (ohm/km) fed to the solver. The solver multiplies these by
/// length_km; this value is negligibly small so design currents reduce to the load formula
/// `Ib = P / (sqrt(3) * V * pf)` and voltage drops collapse to ~0 (both stamped by SCH-006/MEP-002
/// from the solver). Real R/X come from the breaker-catalog import (MEP-010).
const PROTOTYPE_RESISTANCE_OHM_PER_KM: f64 = 1e-9;
const PROTOTYPE_REACTANCE_OHM_PER_KM: f64 = 1e-9;

/// One MVA source: enough for the load flow to run and produce currents; the exact value only
/// shapes fault results (deferred), so it is not a design decision here.
const PROTOTYPE_SOURCE_SC_VA: f64 = 1_000_000.0;

const WARNING_IMPEDANCE: &str = "electrical currents use a prototype cable impedance (stiff-source, 1e-9 ohm/km); replace with real values from the breaker-catalog import of MEP-010 before use — not_approved";
const WARNING_VOLTAGE: &str = "system voltage assumed 400 V line-to-line (prototype; derive from the source connector per MEP-010); single-phase loads use 400/√3 V line-to-neutral (~231 V) — not_approved";
const WARNING_DROP: &str = "voltage drop (V, %) is imported from the edesign-alike solver (MEP-002), read-only: the % is the cumulative source-to-board drop (solver `path_voltage_drop_pct`), not the feeder's own drop; with the prototype cable impedance (1e-9 ohm/km, stiff source) the drop is ~0 V — replace R/X from the breaker-catalog import of MEP-010 before use — not_approved";

/// Nominal voltage for a node of the given phase configuration: line-to-line for three-phase,
/// line-to-neutral for single-phase (the solver's single-phase convention).
fn voltage_for(phases: PhaseConfiguration) -> f64 {
    match phases {
        PhaseConfiguration::ThreePhase => NOMINAL_VOLTAGE_V,
        PhaseConfiguration::SinglePhase => NOMINAL_VOLTAGE_V / SQRT_3,
    }
}

/// Plan length of a cable through its via points (`from.at → via… → to.at`) plus the level-to-level
/// height and any extra allowance, in metres — the same route the drawn cable reports measure.
fn cable_length_m(doc: &Document, from: &ElecNode, c: &ElecCable, to: &ElecNode) -> f64 {
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

/// Reactive power (var) for a load from its real power and power factor.
fn reactive_var(watts: f64, power_factor: f64) -> f64 {
    let pf = power_factor.clamp(0.0, 1.0);
    if pf <= 0.0 || !pf.is_finite() || !watts.is_finite() {
        0.0
    } else {
        watts * (1.0 - pf * pf).sqrt() / pf
    }
}

/// Phase configuration of a node: three-phase when any cable at it is three-phase.
fn node_phases(n: &ElecNode, mep: &MepData) -> PhaseConfiguration {
    let tp = mep
        .elec_cables
        .iter()
        .any(|c| (c.from == n.id || c.to == n.id) && c.three_phase);
    if tp {
        PhaseConfiguration::ThreePhase
    } else {
        PhaseConfiguration::SinglePhase
    }
}

/// Map an `ElecKind` onto the solver's `NodeKind`. A board that carries its own local load
/// (`watts > 0`) becomes a `Load` so that load is represented in the graph; a load-free board
/// stays a `Busbar` distribution point.
fn elec_node_kind(n: &ElecNode) -> NodeKind {
    match n.kind {
        ElecKind::Source => NodeKind::Source(Source {
            short_circuit_power_va: Some(PROTOTYPE_SOURCE_SC_VA),
            r_over_x: None,
        }),
        ElecKind::Board => {
            if n.watts > 0.0 && n.watts.is_finite() {
                NodeKind::Load(Load {
                    active_power_w: n.watts,
                    reactive_power_var: reactive_var(n.watts, n.power_factor),
                })
            } else {
                NodeKind::Busbar
            }
        }
        ElecKind::Load => NodeKind::Load(Load {
            active_power_w: n.watts,
            reactive_power_var: reactive_var(n.watts, n.power_factor),
        }),
    }
}

/// Stamp calculated feeder currents onto the electrical symbols of `view`. Read-only: the solver
/// runs over a `Network` built from `mep`'s electrical nodes and cables; each cable's
/// `design_current_a` is written to the "A" annotation of the downstream board symbol (the node the
/// cable feeds). On any solver failure the view is left un-stamped (placeholders remain) and a
/// warning carries the reason. Graph-construction failures (rejected nodes/branches) are surfaced
/// as warnings and leave the placeholders in place: partial networks are never stamped as valid currents.
///
/// `mep` is passed in (rather than read from `doc`) so the `GenerateSchematic` apply arm and the
/// redo path can stamp the staged/live model instead of the pre-transaction document.
pub(crate) fn stamp(mep: &MepData, doc: &Document, view: &mut SchematicView) {
    // Reset every Electrical board's A/V/% slots to placeholders before stamping. A re-stamp after
    // a rejected graph (or on redo) must never leave a stale current behind or index an empty
    // annotations vector: placeholders carry the "not calculated" state until a current is written.
    reset_board_annotations(view);
    if mep.elec_nodes.is_empty() || view.symbols.is_empty() {
        return;
    }
    let mut network = Network::new();
    let mut node_id: HashMap<u64, NodeId> = HashMap::new();
    for (i, n) in mep.elec_nodes.iter().enumerate() {
        node_id.insert(n.id, NodeId(i as u64 + 1));
    }

    // A partial or invalid network never produces valid currents: collect every construction
    // failure, surface it as a warning, and leave the placeholders in place rather than stamping
    // partial results.
    let mut errors: Vec<String> = Vec::new();
    for n in &mep.elec_nodes {
        let phases = node_phases(n, mep);
        let node = Node {
            id: node_id[&n.id],
            label: n.name.clone(),
            kind: elec_node_kind(n),
            nominal_voltage_v: voltage_for(phases),
            phases,
            earthing: EarthingSystem::TnS,
        };
        if let Err(e) = network.add_node(node) {
            errors.push(format!(
                "electrical node \"{}\" id {}: rejected by the solver graph — current not stamped (not_approved): {e}",
                n.name, n.id
            ));
        }
    }

    let n_nodes = mep.elec_nodes.len();
    let mut branch_to_cable: HashMap<BranchId, &ElecCable> = HashMap::new();
    for (i, c) in mep.elec_cables.iter().enumerate() {
        let Some(&na) = node_id.get(&c.from) else {
            errors.push(format!(
                "electrical cable {i}: references an unknown from-node — rejected by the solver graph (not_approved)"
            ));
            continue;
        };
        let Some(&nb) = node_id.get(&c.to) else {
            errors.push(format!(
                "electrical cable {i}: references an unknown to-node — rejected by the solver graph (not_approved)"
            ));
            continue;
        };
        let Some(from_node) = mep.elec_nodes.iter().find(|n| n.id == c.from) else {
            continue;
        };
        let Some(to_node) = mep.elec_nodes.iter().find(|n| n.id == c.to) else {
            continue;
        };
        let from_phases = node_phases(from_node, mep);
        let to_phases = node_phases(to_node, mep);
        if from_phases != to_phases {
            errors.push(format!(
                "electrical cable {i}: mixed-phase {from_phases:?}→{to_phases:?} connection rejected by the solver — current not stamped (not_approved)"
            ));
            continue;
        }
        let bid = BranchId(n_nodes as u64 + i as u64 + 1);
        let branch = Branch {
            id: bid,
            label: format!("cable {i}"),
            from: na,
            to: nb,
            kind: BranchKind::Cable {
                phase: ConductorImpedance {
                    resistance_ohm: PROTOTYPE_RESISTANCE_OHM_PER_KM,
                    reactance_ohm: PROTOTYPE_REACTANCE_OHM_PER_KM,
                },
                neutral: None,
                protective: None,
                ampacity_a: 1e6,
                length_m: cable_length_m(doc, from_node, c, to_node),
            },
            normally_closed: true,
        };
        match network.add_branch(branch) {
            Ok(()) => {
                branch_to_cable.insert(bid, c);
            }
            Err(e) => errors.push(format!(
                "electrical cable {i}: rejected by the solver graph — current not stamped (not_approved): {e}"
            )),
        }
    }

    if !errors.is_empty() {
        view.warnings.extend(errors);
        view.not_approved = true;
        return;
    }

    let scenario = Scenario::new(ScenarioId(1), "schematic");
    let mut stamped: HashMap<u64, f64> = HashMap::new();
    // MEP-002: voltage drop (volts, percent) read-only from the solver, keyed by the downstream
    // node the cable feeds (the board symbol that carries the annotation). Both slots report the
    // cumulative source-to-board drop: the % is the solver's `path_voltage_drop_pct`, and volts is
    // that percent of the downstream node's nominal voltage.
    let mut vdrop: HashMap<u64, (f64, f64)> = HashMap::new();
    match solve(&network, &scenario, &SolverSettings::default()) {
        Ok(report) => {
            for br in &report.branches {
                if let Some(c) = branch_to_cable.get(&br.branch) {
                    stamped.insert(c.to, br.design_current_a);
                    if let Some(to_node) = mep.elec_nodes.iter().find(|n| n.id == c.to) {
                        let v_nom = voltage_for(node_phases(to_node, mep));
                        let path_pct = br.path_voltage_drop_pct;
                        debug_assert!(
                            path_pct.is_finite(),
                            "solver returned a non-finite cumulative voltage drop for a cable"
                        );
                        vdrop.insert(c.to, (v_nom * path_pct / 100.0, path_pct));
                    }
                }
            }
            for f in &report.findings {
                if f.severity != Severity::Info {
                    view.warnings.push(format!(
                        "electrical solver finding ({}) : {} — not_approved",
                        f.rule_id, f.expression
                    ));
                }
            }
        }
        Err(e) => view.warnings.push(format!(
            "electrical current import failed and were left as placeholders (not_approved): {e}"
        )),
    }
    stamp_symbols(view, &stamped);
    stamp_vdrop_symbols(view, &vdrop);
    for w in [WARNING_IMPEDANCE, WARNING_VOLTAGE, WARNING_DROP] {
        if !view.warnings.iter().any(|x| x.contains(w)) {
            view.warnings.push(w.to_string());
        }
    }
    view.not_approved = true;
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

/// Copy a solver current (A) into the "A" slot of each board symbol fed by a cable.
fn stamp_symbols(view: &mut SchematicView, stamped: &HashMap<u64, f64>) {
    for s in &mut view.symbols {
        if s.kind == SymbolKind::Board {
            if let Some(&current) = stamped.get(&s.element) {
                s.annotations[0] = format!("A {:.3}", current);
            }
        }
    }
}

/// Copy a solver voltage drop (V, %, MEP-002) into the "V"/"%" slots of each board symbol fed by a
/// cable. Read-only reuse: the percent is the solver's `path_voltage_drop_pct`, the cumulative
/// source-to-board drop; volts is that percent of the downstream node's nominal voltage
/// (`voltage_for(node_phases(to_node, mep))`). Boards with no downstream cable keep their
/// "V —"/"% —" placeholders (e.g. a pure source or an un-fed board). A non-finite pair is never
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

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_electrical_model::BranchId;

    /// Parse a stamped "V <n>"/"% <n>" annotation and assert it is a finite number. This is the
    /// assertion the V/% re-stamp and redo regressions use: it fails on the placeholder and on any
    /// non-numeric or non-finite text, which `assert_ne` against the placeholder alone would accept.
    pub(crate) fn finite_number(annotation: &str, prefix: &str) -> f64 {
        let text = annotation
            .strip_prefix(prefix)
            .unwrap_or_else(|| panic!("{annotation:?} is not stamped with {prefix:?}"));
        let value: f64 = text
            .parse()
            .unwrap_or_else(|_| panic!("{annotation:?} does not parse as a number"));
        assert!(value.is_finite(), "{annotation:?} is not finite");
        value
    }

    /// A board symbol carrying the mandatory A/V/% annotation triple (the Electrical label set).
    fn board_view(element: u64) -> crate::SchematicView {
        crate::SchematicView {
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

    #[test]
    fn a_non_finite_solver_drop_never_reaches_the_annotation() {
        // The stamp must not serialise a NaN/inf solver result into the view. A branch whose
        // cumulative drop is not finite keeps the placeholder rather than writing "V NaN"/"% inf".
        let mut view = board_view(1 << 50);
        let mut vdrop = HashMap::new();
        vdrop.insert(1u64 << 50, (f64::NAN, f64::NAN));
        stamp_vdrop_symbols(&mut view, &vdrop);
        assert_eq!(view.symbols[0].annotations[1], "V —");
        assert_eq!(view.symbols[0].annotations[2], "% —");
        // A finite pair is written, and parses back as a finite number.
        vdrop.insert(1u64 << 50, (4.0, 1.0));
        stamp_vdrop_symbols(&mut view, &vdrop);
        assert_eq!(finite_number(&view.symbols[0].annotations[1], "V "), 4.0);
        assert_eq!(finite_number(&view.symbols[0].annotations[2], "% "), 1.0);
    }

    #[test]
    fn cumulative_path_drop_differs_from_the_feeder_drop() {
        // The cumulative source-to-board drop is not the feeder's own drop: for a source -> MSB ->
        // DB2 chain the DB2 branch carries both feeders' drops. Constructing the pinned solver's
        // BranchResult with distinct values proves the stamp consumes `path_voltage_drop_pct`, not
        // `branch_voltage_drop_pct`: if the stamp read the branch field, this test's construction
        // would have to change to keep the annotation equal.
        let result = openbim_electrical_solver::BranchResult {
            branch: BranchId(2),
            design_current_a: 100.0,
            carried_power: (80_000.0, 30_000.0),
            branch_voltage_drop_pct: 1.0,
            path_voltage_drop_pct: 2.5,
            fault_max_a: 10_000.0,
            fault_min_a: 6_000.0,
            fault_peak_a: 21_200.0,
        };
        assert_ne!(
            result.branch_voltage_drop_pct, result.path_voltage_drop_pct,
            "the fixture must carry distinct branch and path drops"
        );
        // The stamp is keyed on the downstream node, so the pair is (volts, path percent).
        let mut view = board_view(7);
        // the cumulative drop is percent of the downstream node's nominal voltage
        let v_nom = voltage_for(openbim_electrical_model::PhaseConfiguration::ThreePhase);
        let mut vdrop = HashMap::new();
        vdrop.insert(
            7u64,
            (
                v_nom * result.path_voltage_drop_pct / 100.0,
                result.path_voltage_drop_pct,
            ),
        );
        stamp_vdrop_symbols(&mut view, &vdrop);
        assert_eq!(
            finite_number(&view.symbols[0].annotations[1], "V "),
            v_nom * result.path_voltage_drop_pct / 100.0
        );
        assert_eq!(
            finite_number(&view.symbols[0].annotations[2], "% "),
            result.path_voltage_drop_pct
        );
    }
}
