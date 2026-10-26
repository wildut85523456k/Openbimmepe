//! The interface between the model and an electrical calculation engine (SCH-006, MEP-002).
//!
//! The document never calculates feeder currents or voltage drops itself (clean-room rule: no
//! duplicate calculation engine). It describes the single-line network as plain data
//! ([`ElectricalNetwork`]) and asks whichever [`ElectricalCalc`] the application installed for the
//! results. The open build installs none, so the schematic is still generated but every board keeps
//! its "A —" / "V —" / "% —" placeholders and the view says the currents were not calculated: no
//! value is ever invented to fill the gap.
//!
//! The engine that performs the calculation lives outside this workspace (the `full/` build plugs in
//! the `edesign-alike` solver through `openbim-electrical-adapter`), so this crate, and everything
//! above it, builds without access to that repository.

use crate::ElecKind;
use std::sync::OnceLock;

/// One node of the single-line network, as the model describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkNode {
    /// The model's node id (`ElecNode::id`); results are keyed by it.
    pub id: u64,
    pub name: String,
    pub kind: ElecKind,
    /// Real power taken by the node, W (a source and a load-free board take none).
    pub watts: f64,
    pub power_factor: f64,
    /// Three-phase when any cable at the node is three-phase.
    pub three_phase: bool,
}

/// One cable of the single-line network.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkCable {
    /// Position of the cable in the model's cable list; used in messages.
    pub index: usize,
    pub from: u64,
    pub to: u64,
    pub three_phase: bool,
    /// Routed length in metres (plan route, level-to-level rise and extra allowance); `0.0` when
    /// an end node is missing, which the engine reports as a rejection.
    pub length_m: f64,
    /// The cable's own impedance per kilometre, when the model carries one.
    pub r_ohm_per_km: Option<f64>,
    pub x_ohm_per_km: Option<f64>,
}

/// The single-line electrical network of the model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElectricalNetwork {
    pub nodes: Vec<NetworkNode>,
    pub cables: Vec<NetworkCable>,
}

/// The calculated values for the board a cable feeds.
#[derive(Clone, Debug, PartialEq)]
pub struct FeederResult {
    /// The node the cable feeds (the downstream board whose symbol carries the annotation).
    pub to: u64,
    /// Feeder design current, A.
    pub design_current_a: f64,
    /// Cumulative source-to-node voltage drop, V.
    pub drop_v: f64,
    /// Cumulative source-to-node voltage drop, %.
    pub drop_pct: f64,
}

/// What an engine made of a network.
#[derive(Clone, Debug, PartialEq)]
pub enum CalcOutcome {
    /// No engine is installed: nothing is calculated and nothing is stamped.
    Unavailable { reason: String },
    /// The engine refused the network (an invalid node, an unknown end, a mixed-phase cable): no
    /// partial result is stamped.
    Rejected { reasons: Vec<String> },
    /// Calculated values, preliminary until approved, with the engine's warnings. An engine whose
    /// run failed returns no feeders and says why in `warnings`.
    Solved { feeders: Vec<FeederResult>, warnings: Vec<String> },
}

/// An electrical calculation engine.
pub trait ElectricalCalc: Send + Sync {
    /// Short name for messages and the About box.
    fn name(&self) -> &str;
    /// Calculate the network. Must not panic on an invalid network: return `Rejected`.
    fn calculate(&self, network: &ElectricalNetwork) -> CalcOutcome;
}

/// The engine of a build with no electrical solver.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSolver;

/// The warning a schematic carries when no engine is installed.
pub const NOT_CALCULATED: &str =
    "electrical currents and voltage drops not calculated: this build has no electrical solver installed";

impl ElectricalCalc for NoSolver {
    fn name(&self) -> &str { "none" }
    fn calculate(&self, _network: &ElectricalNetwork) -> CalcOutcome {
        CalcOutcome::Unavailable { reason: NOT_CALCULATED.into() }
    }
}

static INSTALLED: OnceLock<Box<dyn ElectricalCalc>> = OnceLock::new();
static NONE: NoSolver = NoSolver;

/// Install the process-wide engine, once, before the first schematic is generated. A second call
/// is refused and hands the engine back.
pub fn install(calc: Box<dyn ElectricalCalc>) -> Result<(), Box<dyn ElectricalCalc>> {
    INSTALLED.set(calc)
}

/// The installed engine, or [`NoSolver`] when none was installed.
pub fn installed() -> &'static dyn ElectricalCalc {
    match INSTALLED.get() {
        Some(calc) => calc.as_ref(),
        None => &NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_open_build_calculates_nothing() {
        let outcome = NoSolver.calculate(&ElectricalNetwork::default());
        assert_eq!(outcome, CalcOutcome::Unavailable { reason: NOT_CALCULATED.into() });
        assert_eq!(NoSolver.name(), "none");
    }
}
