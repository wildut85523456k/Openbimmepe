//! Sizing and losses of a branched network of pipes or ducts.
//!
//! The method, in the order it is carried out (nothing in it depends on a standard's table):
//! 1. the flow at every end point (given, or worked out from a power and a temperature difference, or, for plumbing,
//!    the loading units of the fixtures);
//! 2. the flow in every branch, by summing the end points downstream of it (loading units are summed first and then
//!    turned into a design flow by the curve the caller gives, so simultaneity is respected);
//! 3. the size of every branch: the smallest size in the catalogue the caller gives that meets the velocity and
//!    friction limits the caller gives;
//! 4. the pressure drop of every branch: friction (Darcy-Weisbach, Colebrook-White), fittings (loss coefficients), and,
//!    when asked, the static head;
//! 5. the most resistant path: the end point that needs the most pressure at the source; its total is the pump or fan
//!    duty, and every other end point has pressure to spare that a balancing device must take up.
//!
//! The standards' tables (loading units, pipe schedules, fitting coefficients) are the caller's data, not this crate's.
//! Lengths are in metres, flows in m3/s, pressures in pascals, powers in watts, temperatures in kelvin.
//! Branches are independent once their flow is known, so sizing and losses run on all cores.

mod electrical;
mod fluid;
mod friction;
mod plumbing;
mod sprinkler;
mod tree;

pub use fluid::{flow_from_power, Fluid};
pub use friction::{colebrook, equivalent_diameter, swamee_jain, Shape, Size};
pub use electrical::{compute_all_electrical, compute_electrical, correction_vars, parse_devices, parse_pe_rule, transformer_fault, CableRow, CableTable, Device, EBranch, EBranchResult, EEnd, EError, ENode, EResults, ESettings, ETree, Earthing, FactorTable, Fault, Load, Lookup, Motor, Phases, TableError};
pub use sprinkler::{flow_from_density, k_factor, solve_all_sprinklers, solve_sprinklers, SprinklerResult, SprinklerResults};
pub use plumbing::{CurveError, LoadingCurve};
pub use tree::{compute, compute_all, Branch, Criteria, Demand, EndResult, Node, BranchResult, Settings, Results, Tree, TreeError};
