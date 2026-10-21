//! Live electrical values on the single-line schematic (doc 06, SCH-006).
//!
//! Imports calculated electrical results from `edesign-alike`'s `openbim-electrical-model` graph
//! and `openbim-electrical-solver` calculation (pinned at 57ef01848860771d29960dcf2e8af2f831fe1e6c).
//! There is no parallel calculation engine here: revit-alike's own `compute_elec` (using `openbim-calc`)
//! is a separate, pre-existing sizing tool, and the clean-room rule forbids duplicating a calculation
//! engine. SCH-006 reads the edesign-alike solver results only.
//!
//! Narrow slice (SCH-006): feeder design current (A) only. Voltage drop (V, %) is deferred to
//! MEP-002 as an explicit release gate. The import stamps each ElecCable's calculated current onto
//! the downstream board symbol's "A" annotation (the node the cable feeds). Voltage-drop annotations
//! ("V —", "% —") are left as placeholders until MEP-002.
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
/// `Ib = P / (sqrt(3) * V * pf)` (voltage drops are deferred to MEP-002). Real R/X come from the
/// breaker-catalog import (MEP-010).
const PROTOTYPE_RESISTANCE_OHM_PER_KM: f64 = 1e-9;
const PROTOTYPE_REACTANCE_OHM_PER_KM: f64 = 1e-9;

/// One MVA source: enough for the load flow to run and produce currents; the exact value only
/// shapes fault results (deferred), so it is not a design decision here.
const PROTOTYPE_SOURCE_SC_VA: f64 = 1_000_000.0;

const WARNING_IMPEDANCE: &str = "electrical currents use a prototype cable impedance (stiff-source, 1e-9 ohm/km); replace with real values from the breaker-catalog import of MEP-010 before use — not_approved";
const WARNING_VOLTAGE: &str = "system voltage assumed 400 V line-to-line (prototype; derive from the source connector per MEP-010); single-phase loads use 400/√3 V line-to-neutral (~231 V) — not_approved";
const WARNING_DROP: &str = "voltage drop (V, %) is deferred to MEP-002: the 'V —' and '% —' annotations are not calculated — not_approved";

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
/// runs over a `Network` built from `doc`'s electrical nodes and cables; each cable's
/// `design_current_a` is written to the "A" annotation of the downstream board symbol (the node the
/// cable feeds). On any solver failure the view is left un-stamped (placeholders remain) and a
/// warning carries the reason. Graph-construction failures (rejected nodes/branches) are surfaced
/// as warnings and leave the placeholders in place: partial networks are never stamped as valid currents.
pub(crate) fn stamp(doc: &Document, view: &mut SchematicView) {
    let mep = doc.mep();
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
    match solve(&network, &scenario, &SolverSettings::default()) {
        Ok(report) => {
            for br in &report.branches {
                if let Some(c) = branch_to_cable.get(&br.branch) {
                    stamped.insert(c.to, br.design_current_a);
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
    for w in [WARNING_IMPEDANCE, WARNING_VOLTAGE, WARNING_DROP] {
        if !view.warnings.iter().any(|x| x.contains(w)) {
            view.warnings.push(w.to_string());
        }
    }
    view.not_approved = true;
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
