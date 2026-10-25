//! The `edesign-alike` electrical solver as revit-alike's [`ElectricalCalc`] engine (SCH-006, MEP-002).
//!
//! This crate is the only place that depends on `edesign-alike`'s `openbim-electrical-model` and
//! `openbim-electrical-solver` (pinned at 57ef01848860771d29960dcf2e8af2f831fe1e6c). It lives in the
//! separate `full/` workspace so the main workspace builds without access to that repository.
//!
//! The adapter maps the model's [`ElectricalNetwork`] onto the solver graph and returns the solver's
//! results unchanged. It calculates nothing itself (clean-room rule). Feeder design current comes from
//! `design_current_a`; the voltage drop is the cumulative source-to-node `path_voltage_drop_pct`
//! (not the feeder's own `branch_voltage_drop_pct`, because a board's annotation must report the
//! drop seen at that board), in volts as that percent of the node's nominal voltage.
//!
//! Every value is `not_approved`: the solver computes values and findings only (engineering sign-off
//! is a separate step), and the cable impedance and system voltage are prototype stubs pending the
//! breaker-catalog import of MEP-010.

use openbim_document::electrical_calc::{CalcOutcome, ElectricalCalc, ElectricalNetwork, FeederResult, NetworkNode};
use openbim_document::ElecKind;
use openbim_electrical_model::{
    Branch, BranchId, BranchKind, ConductorImpedance, EarthingSystem, Load, Network, Node, NodeId,
    NodeKind, PhaseConfiguration, Scenario, ScenarioId, Source,
};
use openbim_electrical_solver::{solve, Severity, SolverSettings};
use std::collections::HashMap;

/// Prototype nominal system voltage, line-to-line (V) for a three-phase LV system. The model does
/// not yet persist the LV system voltage; the real value is read from the source conductor's family
/// connector once MEP-010 wires up the catalog. The solver treats a single-phase node's
/// `nominal_voltage_v` as line-to-neutral, so single-phase nodes are fed `NOMINAL_VOLTAGE_V / sqrt(3)`
/// to preserve the 400 V line-to-line system invariant (400 V LL -> ~231 V LN).
pub const NOMINAL_VOLTAGE_V: f64 = 400.0;
const SQRT_3: f64 = 1.7320508075688772;

/// Prototype per-kilometre cable impedance (ohm/km) fed to the solver when a cable carries none.
/// Negligibly small, so design currents reduce to the load formula `Ib = P / (sqrt(3) * V * pf)` and
/// voltage drops collapse to ~0. Real R/X come from the breaker-catalog import (MEP-010).
const PROTOTYPE_RESISTANCE_OHM_PER_KM: f64 = 1e-9;
const PROTOTYPE_REACTANCE_OHM_PER_KM: f64 = 1e-9;

/// One MVA source: enough for the load flow to run and produce currents; the exact value only
/// shapes fault results (deferred), so it is not a design decision here.
const PROTOTYPE_SOURCE_SC_VA: f64 = 1_000_000.0;

pub const WARNING_IMPEDANCE: &str = "electrical currents use a prototype cable impedance (stiff-source, 1e-9 ohm/km); replace with real values from the breaker-catalog import of MEP-010 before use — not_approved";
pub const WARNING_VOLTAGE: &str = "system voltage assumed 400 V line-to-line (prototype; derive from the source connector per MEP-010); single-phase loads use 400/√3 V line-to-neutral (~231 V) — not_approved";
pub const WARNING_DROP: &str = "voltage drop (V, %) is imported from the edesign-alike solver (MEP-002), read-only: the % is the cumulative source-to-board drop (solver `path_voltage_drop_pct`), not the feeder's own drop; with the prototype cable impedance (1e-9 ohm/km, stiff source) the drop is ~0 V — replace R/X from the breaker-catalog import of MEP-010 before use — not_approved";

/// The `edesign-alike` solver as an [`ElectricalCalc`]. Install it once at start-up with
/// `openbim_document::electrical_calc::install(Box::new(EdesignSolver))`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdesignSolver;

fn phases(three_phase: bool) -> PhaseConfiguration {
    if three_phase { PhaseConfiguration::ThreePhase } else { PhaseConfiguration::SinglePhase }
}

/// Nominal voltage for a node of the given phase configuration: line-to-line for three-phase,
/// line-to-neutral for single-phase (the solver's single-phase convention).
pub fn voltage_for(three_phase: bool) -> f64 {
    if three_phase { NOMINAL_VOLTAGE_V } else { NOMINAL_VOLTAGE_V / SQRT_3 }
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

/// Map a node onto the solver's `NodeKind`. A board that carries its own local load (`watts > 0`)
/// becomes a `Load` so that load is represented in the graph; a load-free board stays a `Busbar`.
fn node_kind(n: &NetworkNode) -> NodeKind {
    let load = || NodeKind::Load(Load { active_power_w: n.watts, reactive_power_var: reactive_var(n.watts, n.power_factor) });
    match n.kind {
        ElecKind::Source => NodeKind::Source(Source { short_circuit_power_va: Some(PROTOTYPE_SOURCE_SC_VA), r_over_x: None }),
        ElecKind::Board if n.watts > 0.0 && n.watts.is_finite() => load(),
        ElecKind::Board => NodeKind::Busbar,
        ElecKind::Load => load(),
    }
}

/// The solver graph for `net`, with the cable each branch stands for, or every construction
/// failure. A partial or invalid network never produces valid currents.
pub fn solver_network(net: &ElectricalNetwork) -> Result<(Network, HashMap<BranchId, usize>), Vec<String>> {
    let mut network = Network::new();
    let node_id: HashMap<u64, NodeId> = net.nodes.iter().enumerate().map(|(i, n)| (n.id, NodeId(i as u64 + 1))).collect();
    let mut errors = Vec::new();
    for n in &net.nodes {
        let node = Node {
            id: node_id[&n.id],
            label: n.name.clone(),
            kind: node_kind(n),
            nominal_voltage_v: voltage_for(n.three_phase),
            phases: phases(n.three_phase),
            earthing: EarthingSystem::TnS,
        };
        if let Err(e) = network.add_node(node) {
            errors.push(format!("electrical node \"{}\" id {}: rejected by the solver graph — current not stamped (not_approved): {e}", n.name, n.id));
        }
    }
    let mut branch_to_cable = HashMap::new();
    for (pos, c) in net.cables.iter().enumerate() {
        let i = c.index;
        let Some(&na) = node_id.get(&c.from) else {
            errors.push(format!("electrical cable {i}: references an unknown from-node — rejected by the solver graph (not_approved)"));
            continue;
        };
        let Some(&nb) = node_id.get(&c.to) else {
            errors.push(format!("electrical cable {i}: references an unknown to-node — rejected by the solver graph (not_approved)"));
            continue;
        };
        let from_phases = net.nodes.iter().find(|n| n.id == c.from).map(|n| phases(n.three_phase));
        let to_phases = net.nodes.iter().find(|n| n.id == c.to).map(|n| phases(n.three_phase));
        if from_phases != to_phases {
            errors.push(format!("electrical cable {i}: mixed-phase {from_phases:?}→{to_phases:?} connection rejected by the solver — current not stamped (not_approved)"));
            continue;
        }
        let bid = BranchId(net.nodes.len() as u64 + pos as u64 + 1);
        let branch = Branch {
            id: bid,
            label: format!("cable {i}"),
            from: na,
            to: nb,
            kind: BranchKind::Cable {
                // the cable's own R/X when it carries them, otherwise the stiff-source prototype
                phase: ConductorImpedance {
                    resistance_ohm: c.r_ohm_per_km.unwrap_or(PROTOTYPE_RESISTANCE_OHM_PER_KM),
                    reactance_ohm: c.x_ohm_per_km.unwrap_or(PROTOTYPE_REACTANCE_OHM_PER_KM),
                },
                neutral: None,
                protective: None,
                ampacity_a: 1e6,
                length_m: c.length_m,
            },
            normally_closed: true,
        };
        match network.add_branch(branch) {
            Ok(()) => { branch_to_cable.insert(bid, pos); }
            Err(e) => errors.push(format!("electrical cable {i}: rejected by the solver graph — current not stamped (not_approved): {e}")),
        }
    }
    if errors.is_empty() { Ok((network, branch_to_cable)) } else { Err(errors) }
}

/// The scenario every schematic is solved in.
pub fn scenario() -> Scenario { Scenario::new(ScenarioId(1), "schematic") }

impl ElectricalCalc for EdesignSolver {
    fn name(&self) -> &str { "edesign-alike solver" }

    fn calculate(&self, net: &ElectricalNetwork) -> CalcOutcome {
        let (network, branch_to_cable) = match solver_network(net) {
            Ok(built) => built,
            Err(reasons) => return CalcOutcome::Rejected { reasons },
        };
        let mut feeders = Vec::new();
        let mut warnings = Vec::new();
        match solve(&network, &scenario(), &SolverSettings::default()) {
            Ok(report) => {
                for br in &report.branches {
                    let Some(&pos) = branch_to_cable.get(&br.branch) else { continue };
                    let c = &net.cables[pos];
                    let three_phase = net.nodes.iter().find(|n| n.id == c.to).is_some_and(|n| n.three_phase);
                    let pct = br.path_voltage_drop_pct;
                    feeders.push(FeederResult { to: c.to, design_current_a: br.design_current_a, drop_v: voltage_for(three_phase) * pct / 100.0, drop_pct: pct });
                }
                for f in &report.findings {
                    if f.severity != Severity::Info {
                        warnings.push(format!("electrical solver finding ({}) : {} — not_approved", f.rule_id, f.expression));
                    }
                }
            }
            Err(e) => warnings.push(format!("electrical current import failed and were left as placeholders (not_approved): {e}")),
        }
        warnings.extend([WARNING_IMPEDANCE, WARNING_VOLTAGE, WARNING_DROP].map(String::from));
        CalcOutcome::Solved { feeders, warnings }
    }
}
