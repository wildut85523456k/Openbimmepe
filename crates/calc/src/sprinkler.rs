//! Sprinkler (and hydrant, nozzle) hydraulics for fire water, in the manner of the design-area calculations of
//! EN 12845 and BS 9251: the sprinklers the caller says operate (the design area) each flow `k * sqrt(pressure)`, no
//! less than the flow the density asks for. The pressure at the source is the least that gives the most demanding
//! sprinkler its minimum; every other sprinkler then sits at a higher pressure and flows more, which loads the pipes
//! more, so the answer is found by iterating until the flows stop changing.
//!
//! The densities, areas, K-factors, minimum pressures and pipe tables are the caller's data (the standards' own).
//! Choosing which sprinklers form the most remote design area is the caller's step.

use crate::tree::{compute, sprinkler_min_flow, Demand, Results, Settings, Tree, TreeError};
use rayon::prelude::*;

/// K-factor as printed on a sprinkler (litres per minute per root bar) to SI (m3/s per root pascal).
pub fn k_factor(l_min_per_root_bar: f64) -> f64 {
    l_min_per_root_bar / 60_000.0 / 1.0e5f64.sqrt()
}

/// Flow of a density over the area one sprinkler covers, m3/s: `density_mm_per_min * area_m2` is litres per minute.
pub fn flow_from_density(density_mm_per_min: f64, area_per_sprinkler_m2: f64) -> f64 {
    density_mm_per_min * area_per_sprinkler_m2 / 60_000.0
}

#[derive(Clone, Debug, PartialEq)]
pub struct SprinklerResult {
    pub node: usize,
    /// m3/s
    pub flow: f64,
    /// Pa, at the sprinkler
    pub pressure: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SprinklerResults {
    pub results: Results,
    pub sprinklers: Vec<SprinklerResult>,
    pub iterations: usize,
}

const TOLERANCE: f64 = 1e-9;
const MAX_ITERATIONS: usize = 200;

pub fn solve_sprinklers(tree: &Tree, s: &Settings) -> Result<SprinklerResults, TreeError> {
    // every input is checked once, by the plain computation, before any division by a K-factor
    compute(tree, s)?;
    let sprinklers: Vec<(usize, f64, f64, f64)> = tree.nodes.iter().enumerate()
        .filter_map(|(i, nd)| if let Demand::Sprinkler { k, min_flow } = nd.demand { Some((i, k, min_flow, needed_pressure(k, min_flow, nd.required_pressure))) } else { None })
        .collect();
    // flows start at each sprinkler's minimum
    let mut flows: Vec<f64> = sprinklers.iter().map(|&(i, k, m, _)| sprinkler_min_flow(k, m, tree.nodes[i].required_pressure)).collect();
    let mut work = tree.clone();
    for iteration in 1..=MAX_ITERATIONS {
        for (&(i, ..), &q) in sprinklers.iter().zip(&flows) { work.nodes[i].demand = Demand::Flow(q); work.nodes[i].required_pressure = sprinklers.iter().find(|x| x.0 == i).map_or(0.0, |x| x.3); }
        let r = compute(&work, s)?;
        let top = r.source_pressure;
        // the pressure at each sprinkler: what the source gives less the losses on its way
        let at: Vec<f64> = sprinklers.iter().map(|&(i, ..)| {
            let mut loss = 0.0;
            let mut n = i;
            while let Some(p) = work.nodes[n].parent { loss += r.branches[n].dp_total; n = p; }
            top - loss
        }).collect();
        let next: Vec<f64> = sprinklers.iter().zip(&at).map(|(&(_, k, ..), &p)| k * p.max(0.0).sqrt()).collect();
        let change = next.iter().zip(&flows).map(|(a, b)| (a - b).abs() / b.max(f64::MIN_POSITIVE)).fold(0.0, f64::max);
        if change < TOLERANCE {
            let out = sprinklers.iter().zip(&next).zip(&at).map(|((&(node, ..), &flow), &pressure)| SprinklerResult { node, flow, pressure }).collect();
            return Ok(SprinklerResults { results: r, sprinklers: out, iterations: iteration });
        }
        flows = next;
    }
    Err(TreeError::NotConverged)
}

fn needed_pressure(k: f64, min_flow: f64, min_pressure: f64) -> f64 {
    min_pressure.max((min_flow / k).powi(2))
}

/// Several fire-water systems at once, each on its own core.
pub fn solve_all_sprinklers(systems: &[(Tree, Settings)]) -> Vec<Result<SprinklerResults, TreeError>> {
    systems.par_iter().map(|(t, s)| solve_sprinklers(t, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Branch, Criteria, Node};
    use crate::{Fluid, Size};

    // test data only: not any standard's schedule
    fn catalogue() -> Vec<Size> {
        [("25", 0.0272), ("32", 0.0357), ("40", 0.0419), ("50", 0.0527), ("65", 0.0685), ("80", 0.0809), ("100", 0.1045)]
            .iter().map(|(l, d)| Size::round(l, *d)).collect()
    }

    fn settings() -> Settings {
        Settings { fluid: Fluid::WATER, roughness: 0.0, criteria: Criteria { max_velocity: Some(6.0), max_friction: None }, catalogue: catalogue(), count_static: true, loading: None, hazen_williams: Some(120.0) }
    }

    fn node(parent: Option<usize>, demand: Demand, min_p: f64, z: f64, len: f64, k: f64) -> Node {
        Node { parent, demand, required_pressure: min_p, elevation: z, branch: Branch { length: len, fitting_k: k, locked: None } }
    }

    /// riser 1 to a range pipe 2, with sprinklers 3 (near), 4 (far), at 5 m height; the far one is the last on the range
    fn range() -> Tree {
        let sp = Demand::Sprinkler { k: k_factor(80.0), min_flow: flow_from_density(4.0, 12.0) };
        Tree { nodes: vec![
            node(None, Demand::None, 0.0, 0.0, 0.0, 0.0),
            node(Some(0), Demand::None, 0.0, 5.0, 5.0, 1.0),
            node(Some(1), Demand::None, 0.0, 5.0, 10.0, 0.5),
            node(Some(2), sp, 0.5e5, 5.0, 3.0, 0.0),
            node(Some(2), sp, 0.5e5, 5.0, 6.0, 0.0),
        ] }
    }

    #[test]
    fn k_factor_and_density_units() {
        assert!((k_factor(80.0) * 1.0e5f64.sqrt() * 1000.0 - 80.0 / 60.0).abs() < 1e-12, "80 at 1 bar is 80 l/min");
        assert!((flow_from_density(5.0, 12.0) * 60_000.0 - 60.0).abs() < 1e-9, "5 mm/min over 12 m2 is 60 l/min");
    }

    #[test]
    fn the_sprinklers_flow_by_the_pressure_they_get_and_the_source_gives_the_worst_one_its_minimum() {
        let r = solve_sprinklers(&range(), &settings()).unwrap();
        let near = r.sprinklers.iter().find(|x| x.node == 3).unwrap();
        let far = r.sprinklers.iter().find(|x| x.node == 4).unwrap();
        let k = k_factor(80.0);
        // the far sprinkler is the most remote: it gets its minimum, the near one more
        assert!(far.pressure >= 0.5e5 - 1.0 && near.pressure > far.pressure);
        assert!((far.pressure - 0.5e5).abs() < 1.0, "the worst one sits at its minimum: {}", far.pressure);
        for sp in &r.sprinklers { assert!((sp.flow - k * sp.pressure.sqrt()).abs() < 1e-12, "q = K sqrt(p)"); }
        assert!(near.flow > far.flow, "the nearer, higher-pressure sprinkler flows more");
        // the range carries both, the source carries the same; mass is kept
        let total = near.flow + far.flow;
        assert!((r.results.branches[0].flow - total).abs() < 1e-9 && (r.results.branches[2].flow - total).abs() < 1e-9);
        assert_eq!(r.results.critical_path, vec![0, 1, 2, 4]);
        // pressure at the source = pressure at the far sprinkler + the losses along its path (friction, fittings, 5 m of height)
        let loss: f64 = [1usize, 2, 4].iter().map(|&i| r.results.branches[i].dp_total).sum();
        assert!((r.results.source_pressure - (0.5e5 + loss)).abs() < 1.0);
        assert!((r.results.branches[1].dp_static - 998.0 * 9.80665 * 5.0).abs() < 1e-6, "5 m of water is 0.49 bar");
        assert!(r.iterations > 1 && r.iterations < 100);
    }

    #[test]
    fn a_minimum_flow_above_what_k_gives_at_the_minimum_pressure_raises_the_pressure_needed() {
        // density asks 100 l/min but K 80 at 0.5 bar gives only 56.6: the sprinkler needs (100/80)^2 = 1.5625 bar
        let mut t = range();
        t.nodes[4].demand = Demand::Sprinkler { k: k_factor(80.0), min_flow: flow_from_density(8.0, 12.5) };
        let r = solve_sprinklers(&t, &settings()).unwrap();
        let far = r.sprinklers.iter().find(|x| x.node == 4).unwrap();
        assert!((far.pressure - 1.5625e5).abs() < 1.0, "{}", far.pressure);
        assert!((far.flow * 60_000.0 - 100.0).abs() < 1e-3);
    }

    #[test]
    fn the_extra_flow_at_the_near_sprinklers_loads_the_pipes_so_the_sizes_come_from_the_converged_flows() {
        let r = solve_sprinklers(&range(), &settings()).unwrap();
        let b = &r.results.branches[2];
        assert!(b.velocity <= 6.0 && !b.exceeds_limits);
        // the same tree with every sprinkler held at its minimum flow would need less than the converged answer
        let min_only = compute(&range(), &settings()).unwrap();
        assert!(r.results.branches[0].flow > min_only.branches[0].flow);
        assert!(r.results.source_pressure >= min_only.source_pressure - 1e-6);
    }

    #[test]
    fn bad_input_is_named_and_systems_run_together() {
        let mut t = range();
        t.nodes[3].demand = Demand::Sprinkler { k: 0.0, min_flow: 0.0 };
        assert_eq!(solve_sprinklers(&t, &settings()), Err(TreeError::BadValue(3, "sprinkler")));
        let s = Settings { hazen_williams: Some(-1.0), ..settings() };
        assert_eq!(solve_sprinklers(&range(), &s), Err(TreeError::BadSettings("the Hazen-Williams C")));
        let systems = vec![(range(), settings()); 12];
        let all = solve_all_sprinklers(&systems);
        assert!(all.iter().all(|r| r == &solve_sprinklers(&range(), &settings())));
    }
}
