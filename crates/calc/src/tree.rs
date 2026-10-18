use crate::friction::{friction_per_metre, hazen_williams_per_metre};
use crate::{flow_from_power, Fluid, LoadingCurve, Size};
use rayon::prelude::*;

/// What an end point takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Demand {
    /// nothing (a junction)
    None,
    /// a flow, m3/s
    Flow(f64),
    /// a power across a temperature difference: a coil, a radiator, a terminal unit
    Power { watts: f64, delta_t: f64 },
    /// loading units of a fixture (plumbing): the flow of a branch comes from the sum downstream, by the loading curve
    LoadingUnits(f64),
    /// a sprinkler or nozzle: flow = k sqrt(pressure), never below `min_flow`; `k` in m3/s per sqrt(Pa).
    /// Here it flows at its own minimum pressure; `solve_sprinklers` finds what it flows at the pressure it gets.
    Sprinkler { k: f64, min_flow: f64 },
}

/// The branch that leads to a node from its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    /// along the centre line, metres
    pub length: f64,
    /// the sum of the loss coefficients of its fittings, for the velocity in this branch
    pub fitting_k: f64,
    /// a size the designer fixed: it is kept, and reported if it breaks a limit
    pub locked: Option<Size>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// None for the source, which every other node leads back to
    pub parent: Option<usize>,
    pub demand: Demand,
    /// the pressure this end point needs, Pa (a coil's drop, a sprinkler's, a terminal's)
    pub required_pressure: f64,
    /// height, metres
    pub elevation: f64,
    /// the branch from the parent (not used for the source)
    pub branch: Branch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tree { pub nodes: Vec<Node> }

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Criteria {
    /// m/s
    pub max_velocity: Option<f64>,
    /// Pa/m
    pub max_friction: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub fluid: Fluid,
    /// absolute roughness of the inside wall, metres
    pub roughness: f64,
    pub criteria: Criteria,
    /// the sizes to choose from, in any order
    pub catalogue: Vec<Size>,
    /// count the height difference as a pressure (an open or draining system); false for a closed circuit or a duct
    pub count_static: bool,
    /// loading units to flow; needed when any end point gives loading units
    pub loading: Option<LoadingCurve>,
    /// Hazen-Williams C, for fire water; when set it replaces Darcy-Weisbach and the roughness
    pub hazen_williams: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TreeError {
    NoSource,
    MoreThanOneSource,
    /// a node whose parent is not a node
    BadParent(usize),
    /// a node that does not lead back to the source (a cycle, or another network)
    Disconnected(usize),
    BadValue(usize, &'static str),
    BadSettings(&'static str),
    /// the loading units downstream of this node are more than the loading curve covers
    OutsideCurve(usize),
    /// the sprinkler flows did not settle
    NotConverged,
    /// no size in the catalogue meets the limits for this node's branch
    NoSize(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BranchResult {
    /// the flow in the branch, m3/s (for the source: the total)
    pub flow: f64,
    /// None for the source
    pub size: Option<Size>,
    pub velocity: f64,
    pub reynolds: f64,
    /// Pa/m
    pub friction_per_metre: f64,
    pub dp_friction: f64,
    pub dp_fittings: f64,
    pub dp_static: f64,
    pub dp_total: f64,
    /// a locked size that breaks a limit
    pub exceeds_limits: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EndResult {
    pub node: usize,
    /// the pressure the source must give for this end point: the drops along its path plus what it needs
    pub needed_at_source: f64,
    /// what is left over compared with the most resistant path: a balancing valve or damper must take it up
    pub excess: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    /// by node
    pub branches: Vec<BranchResult>,
    /// the end points: nodes with a demand, and the nodes nothing hangs from
    pub ends: Vec<EndResult>,
    /// the nodes from the source to the end point that needs the most pressure
    pub critical_path: Vec<usize>,
    /// the pump or fan duty, Pa
    pub source_pressure: f64,
}

const GRAVITY: f64 = 9.80665;

fn check(tree: &Tree, s: &Settings) -> Result<Vec<usize>, TreeError> {
    if !s.fluid.valid() { return Err(TreeError::BadSettings("the fluid")); }
    if !(s.roughness.is_finite() && s.roughness >= 0.0) { return Err(TreeError::BadSettings("the roughness")); }
    for lim in [s.criteria.max_velocity, s.criteria.max_friction].into_iter().flatten() {
        if !(lim.is_finite() && lim > 0.0) { return Err(TreeError::BadSettings("a limit")); }
    }
    if s.hazen_williams.is_some_and(|c| !(c.is_finite() && c > 0.0)) { return Err(TreeError::BadSettings("the Hazen-Williams C")); }
    if s.catalogue.iter().any(|z| !z.valid()) { return Err(TreeError::BadSettings("a size")); }
    let n = tree.nodes.len();
    let sources: Vec<usize> = (0..n).filter(|i| tree.nodes[*i].parent.is_none()).collect();
    match sources.len() { 0 => return Err(TreeError::NoSource), 1 => {}, _ => return Err(TreeError::MoreThanOneSource) }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, nd) in tree.nodes.iter().enumerate() {
        if let Some(p) = nd.parent {
            if p >= n || p == i { return Err(TreeError::BadParent(i)); }
            children[p].push(i);
        }
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        if !nd.elevation.is_finite() { return Err(TreeError::BadValue(i, "elevation")); }
        if !ok(nd.required_pressure) { return Err(TreeError::BadValue(i, "required pressure")); }
        if nd.parent.is_some() {
            if !ok(nd.branch.length) { return Err(TreeError::BadValue(i, "length")); }
            if !ok(nd.branch.fitting_k) { return Err(TreeError::BadValue(i, "fitting coefficient")); }
            if nd.branch.locked.as_ref().is_some_and(|z| !z.valid()) { return Err(TreeError::BadValue(i, "locked size")); }
        }
        match nd.demand {
            Demand::None => {}
            Demand::Flow(q) => if !ok(q) { return Err(TreeError::BadValue(i, "flow")); },
            Demand::Sprinkler { k, min_flow } => if !(k.is_finite() && k > 0.0 && ok(min_flow)) { return Err(TreeError::BadValue(i, "sprinkler")); },
            Demand::LoadingUnits(u) => {
                if !ok(u) { return Err(TreeError::BadValue(i, "loading units")); }
                if s.loading.is_none() { return Err(TreeError::BadSettings("no loading curve for loading units")); }
            }
            Demand::Power { watts, delta_t } => if flow_from_power(watts, delta_t, &s.fluid).is_none() { return Err(TreeError::BadValue(i, "power or temperature difference")); },
        }
    }
    // from the source outwards: parents come before their children
    let mut order = vec![sources[0]];
    let mut k = 0;
    while k < order.len() { let c = &children[order[k]]; order.extend(c.iter().copied()); k += 1; }
    if order.len() != n {
        let seen: std::collections::HashSet<usize> = order.iter().copied().collect();
        return Err(TreeError::Disconnected((0..n).find(|i| !seen.contains(i)).unwrap_or(0)));
    }
    Ok(order)
}

fn demand_flow(nd: &Node, fluid: &Fluid) -> f64 {
    match nd.demand { Demand::None => 0.0, Demand::Flow(q) => q, Demand::Power { watts, delta_t } => flow_from_power(watts, delta_t, fluid).unwrap_or(0.0), Demand::LoadingUnits(_) => 0.0, Demand::Sprinkler { k, min_flow } => sprinkler_min_flow(k, min_flow, nd.required_pressure) }
}

/// The flow of a sprinkler at the least pressure it can work at: that of its own rating, or the one its minimum flow needs.
pub(crate) fn sprinkler_min_flow(k: f64, min_flow: f64, min_pressure: f64) -> f64 {
    (k * min_pressure.sqrt()).max(min_flow)
}

pub(crate) fn friction_of(q: f64, z: &Size, s: &Settings) -> (f64, f64, f64) {
    match s.hazen_williams {
        Some(c) => hazen_williams_per_metre(q, z, c, &s.fluid),
        None => friction_per_metre(q, z, s.roughness, &s.fluid),
    }
}

/// Sizes and losses of the whole tree: see the crate's description for the method.
pub fn compute(tree: &Tree, s: &Settings) -> Result<Results, TreeError> {
    let order = check(tree, s)?;
    let n = tree.nodes.len();
    // 1 and 2: the flow in every branch, from the end points back to the source
    // flows that are given add up as they are; loading units add up as units and then become a flow by the curve
    let mut flow: Vec<f64> = tree.nodes.iter().map(|nd| demand_flow(nd, &s.fluid)).collect();
    let mut units: Vec<f64> = tree.nodes.iter().map(|nd| if let Demand::LoadingUnits(u) = nd.demand { u } else { 0.0 }).collect();
    for &i in order.iter().rev() {
        if let Some(p) = tree.nodes[i].parent { let (q, u) = (flow[i], units[i]); flow[p] += q; units[p] += u; }
    }
    if let Some(curve) = &s.loading {
        for i in 0..n {
            if units[i] > 0.0 { flow[i] += curve.flow(units[i]).ok_or(TreeError::OutsideCurve(i))?; }
        }
    }
    // 3 and 4: size and losses of each branch, all at once (they do not depend on each other)
    let mut catalogue = s.catalogue.clone();
    catalogue.sort_by(|a, b| a.area().total_cmp(&b.area()));
    let rho = s.fluid.density;
    let per_node: Vec<Result<BranchResult, TreeError>> = (0..n).into_par_iter().map(|i| {
        let nd = &tree.nodes[i];
        let Some(p) = nd.parent else {
            return Ok(BranchResult { flow: flow[i], size: None, velocity: 0.0, reynolds: 0.0, friction_per_metre: 0.0, dp_friction: 0.0, dp_fittings: 0.0, dp_static: 0.0, dp_total: 0.0, exceeds_limits: false });
        };
        let q = flow[i];
        let meets = |z: &Size| {
            let (per_m, v, _) = friction_of(q, z, s);
            s.criteria.max_velocity.is_none_or(|m| v <= m * (1.0 + 1e-12)) && s.criteria.max_friction.is_none_or(|m| per_m <= m * (1.0 + 1e-12))
        };
        let (size, exceeds) = match &nd.branch.locked {
            Some(z) => (z.clone(), !meets(z)),
            None => {
                if s.criteria.max_velocity.is_none() && s.criteria.max_friction.is_none() { return Err(TreeError::BadSettings("no limit to size by")); }
                (catalogue.iter().find(|z| meets(z)).ok_or(TreeError::NoSize(i))?.clone(), false)
            }
        };
        let (per_m, v, re) = friction_of(q, &size, s);
        let dp_friction = per_m * nd.branch.length;
        let dp_fittings = nd.branch.fitting_k * rho * v * v / 2.0;
        let dp_static = if s.count_static { rho * GRAVITY * (nd.elevation - tree.nodes[p].elevation) } else { 0.0 };
        Ok(BranchResult { flow: q, size: Some(size), velocity: v, reynolds: re, friction_per_metre: per_m, dp_friction, dp_fittings, dp_static, dp_total: dp_friction + dp_fittings + dp_static, exceeds_limits: exceeds })
    }).collect();
    let branches: Vec<BranchResult> = per_node.into_iter().collect::<Result<_, _>>()?;
    // 5: the pressure the source must give for each end point, and the most resistant path
    let mut at_source = vec![0.0f64; n];
    for &i in &order {
        if let Some(p) = tree.nodes[i].parent { at_source[i] = at_source[p] + branches[i].dp_total; }
    }
    let has_child: Vec<bool> = {
        let mut h = vec![false; n];
        for nd in &tree.nodes { if let Some(p) = nd.parent { h[p] = true; } }
        h
    };
    let mut ends: Vec<EndResult> = (0..n)
        .filter(|i| tree.nodes[*i].parent.is_some() && (tree.nodes[*i].demand != Demand::None || !has_child[*i]))
        .map(|i| EndResult { node: i, needed_at_source: at_source[i] + tree.nodes[i].required_pressure, excess: 0.0 })
        .collect();
    let mut critical: Option<(usize, f64)> = None;
    for e in &ends { if critical.is_none_or(|(_, best)| e.needed_at_source > best) { critical = Some((e.node, e.needed_at_source)); } }
    let (critical_path, source_pressure) = match critical {
        Some((node, total)) => {
            for e in &mut ends { e.excess = total - e.needed_at_source; }
            let mut path = vec![node];
            while let Some(p) = tree.nodes[*path.last().unwrap_or(&node)].parent { path.push(p); }
            path.reverse();
            (path, total)
        }
        None => (Vec::new(), 0.0),
    };
    Ok(Results { branches, ends, critical_path, source_pressure })
}

/// Many networks at once (the systems of a project), each on its own core.
pub fn compute_all(systems: &[(Tree, Settings)]) -> Vec<Result<Results, TreeError>> {
    systems.par_iter().map(|(t, s)| compute(t, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::friction::friction_per_metre;

    fn catalogue() -> Vec<Size> {
        // test data only: the inside diameters are not any standard's
        [("A", 0.0217), ("B", 0.0273), ("C", 0.0357), ("D", 0.0419), ("E", 0.0539), ("F", 0.0685), ("G", 0.0825), ("H", 0.1071)]
            .iter().map(|(l, d)| Size::round(l, *d)).collect()
    }

    fn settings() -> Settings {
        Settings { fluid: Fluid::WATER, roughness: 0.045e-3, criteria: Criteria { max_velocity: Some(1.5), max_friction: None }, catalogue: catalogue(), count_static: false, loading: None, hazen_williams: None }
    }

    fn node(parent: Option<usize>, demand: Demand, required: f64, length: f64, k: f64) -> Node {
        Node { parent, demand, required_pressure: required, elevation: 0.0, branch: Branch { length, fitting_k: k, locked: None } }
    }

    /// source 0 - junction 1 - end points 2 (near) and 3 (far)
    fn two_coils() -> Tree {
        Tree { nodes: vec![
            node(None, Demand::None, 0.0, 0.0, 0.0),
            node(Some(0), Demand::None, 0.0, 10.0, 2.0),
            node(Some(1), Demand::Power { watts: 50_000.0, delta_t: 5.0 }, 20_000.0, 5.0, 3.0),
            node(Some(1), Demand::Power { watts: 30_000.0, delta_t: 5.0 }, 30_000.0, 20.0, 3.0),
        ] }
    }

    #[test]
    fn flows_add_up_from_the_end_points_to_the_source() {
        let r = compute(&two_coils(), &settings()).unwrap();
        let q2 = 50_000.0 / (998.0 * 4186.0 * 5.0);
        let q3 = 30_000.0 / (998.0 * 4186.0 * 5.0);
        assert!((r.branches[2].flow - q2).abs() < 1e-12 && (r.branches[3].flow - q3).abs() < 1e-12);
        assert!((r.branches[1].flow - (q2 + q3)).abs() < 1e-12);
        assert!((r.branches[0].flow - (q2 + q3)).abs() < 1e-12, "the source carries it all");
    }

    #[test]
    fn each_branch_gets_the_smallest_size_that_keeps_the_velocity_and_the_losses_are_darcy_weisbach() {
        let r = compute(&two_coils(), &settings()).unwrap();
        for i in 1..4 {
            let b = &r.branches[i];
            let z = b.size.clone().unwrap();
            assert!(b.velocity <= 1.5, "{} {}", i, b.velocity);
            // the next size down would break the limit
            let smaller = catalogue().into_iter().filter(|c| c.area() < z.area()).max_by(|a, c| a.area().total_cmp(&c.area()));
            if let Some(s) = smaller { assert!(b.flow / s.area() > 1.5, "branch {i}: {} would do", s.label); }
            // the losses are the sum of friction, fittings and nothing else
            let (per_m, v, _) = friction_per_metre(b.flow, &z, 0.045e-3, &Fluid::WATER);
            let nd = &two_coils().nodes[i];
            assert!((b.dp_friction - per_m * nd.branch.length).abs() < 1e-9);
            assert!((b.dp_fittings - nd.branch.fitting_k * 998.0 * v * v / 2.0).abs() < 1e-9);
            assert_eq!(b.dp_static, 0.0);
            assert!((b.dp_total - b.dp_friction - b.dp_fittings).abs() < 1e-9);
        }
        // the main (3.82 l/s) needs 0.00382 / 1.5 = 0.002547 m2 at least: size F, 68.5 mm
        assert_eq!(r.branches[1].size.as_ref().unwrap().label, "F");
    }

    #[test]
    fn the_most_resistant_path_sets_the_duty_and_the_other_end_has_the_difference_to_spare() {
        let r = compute(&two_coils(), &settings()).unwrap();
        let t = two_coils();
        let need = |i: usize| r.branches[1].dp_total + r.branches[i].dp_total + t.nodes[i].required_pressure;
        let (n2, n3) = (need(2), need(3));
        assert!(n3 > n2, "the far, hungrier coil is the critical one");
        assert_eq!(r.critical_path, vec![0, 1, 3]);
        assert!((r.source_pressure - n3).abs() < 1e-6);
        let e2 = r.ends.iter().find(|e| e.node == 2).unwrap();
        let e3 = r.ends.iter().find(|e| e.node == 3).unwrap();
        assert!((e2.needed_at_source - n2).abs() < 1e-6 && e3.excess == 0.0);
        assert!((e2.excess - (n3 - n2)).abs() < 1e-6, "{} spare to be taken up by a balancing valve", e2.excess);
        assert!(r.ends.iter().all(|e| e.excess >= 0.0));
    }

    #[test]
    fn height_counts_as_pressure_only_when_asked() {
        let mut t = two_coils();
        t.nodes[2].elevation = 10.0;
        let dry = compute(&t, &settings()).unwrap();
        let wet = compute(&t, &Settings { count_static: true, ..settings() }).unwrap();
        assert_eq!(dry.branches[2].dp_static, 0.0);
        assert!((wet.branches[2].dp_static - 998.0 * 9.80665 * 10.0).abs() < 1e-6);
        assert!((wet.branches[2].dp_total - dry.branches[2].dp_total - 998.0 * 9.80665 * 10.0).abs() < 1e-6);
    }

    #[test]
    fn a_locked_size_is_kept_and_reported_when_it_breaks_a_limit() {
        let mut t = two_coils();
        t.nodes[1].branch.locked = Some(Size::round("small", 0.0273));
        let r = compute(&t, &settings()).unwrap();
        assert_eq!(r.branches[1].size.as_ref().unwrap().label, "small");
        assert!(r.branches[1].exceeds_limits && !r.branches[2].exceeds_limits);
    }

    #[test]
    fn a_friction_limit_can_size_a_rectangular_duct() {
        // 1 m3/s of air; the friction must stay under 1 Pa/m
        let t = Tree { nodes: vec![
            node(None, Demand::None, 0.0, 0.0, 0.0),
            node(Some(0), Demand::Flow(1.0), 50.0, 30.0, 1.0),
        ] };
        let cat = vec![Size::rect("400 x 300", 0.4, 0.3), Size::rect("800 x 400", 0.8, 0.4), Size::rect("1000 x 500", 1.0, 0.5), Size::rect("1200 x 600", 1.2, 0.6)];
        let s = Settings { fluid: Fluid::AIR, roughness: 0.09e-3, criteria: Criteria { max_velocity: Some(8.0), max_friction: Some(1.0) }, catalogue: cat, count_static: false, loading: None, hazen_williams: None };
        let r = compute(&t, &s).unwrap();
        let b = &r.branches[1];
        assert!(b.friction_per_metre <= 1.0 && b.velocity <= 8.0);
        let z = b.size.clone().unwrap();
        assert_eq!(z.label, "800 x 400", "the 400 x 300 is too small and the 800 x 400 is the first that fits: {z:?} {} Pa/m {} m/s", b.friction_per_metre, b.velocity);
        assert!((r.source_pressure - (b.dp_total + 50.0)).abs() < 1e-9);
    }

    #[test]
    fn things_that_cannot_be_computed_are_refused_and_named() {
        let s = settings();
        let mut t = two_coils();
        t.nodes[0].parent = Some(3);
        assert_eq!(compute(&t, &s), Err(TreeError::NoSource));
        let mut t = two_coils();
        t.nodes[1].parent = None;
        assert_eq!(compute(&t, &s), Err(TreeError::MoreThanOneSource));
        let mut t = two_coils();
        t.nodes[2].parent = Some(9);
        assert_eq!(compute(&t, &s), Err(TreeError::BadParent(2)));
        // 4 and 5 lead to each other and not to the source
        let mut t = two_coils();
        t.nodes.push(node(Some(5), Demand::Flow(0.001), 0.0, 1.0, 0.0));
        t.nodes.push(node(Some(4), Demand::None, 0.0, 1.0, 0.0));
        assert_eq!(compute(&t, &s), Err(TreeError::Disconnected(4)));
        let mut t = two_coils();
        t.nodes[2].demand = Demand::Power { watts: 1000.0, delta_t: 0.0 };
        assert_eq!(compute(&t, &s), Err(TreeError::BadValue(2, "power or temperature difference")));
        let mut t = two_coils();
        t.nodes[3].branch.length = -1.0;
        assert_eq!(compute(&t, &s), Err(TreeError::BadValue(3, "length")));
        // nothing in the catalogue carries 3.8 l/s at 1.5 m/s
        let small = Settings { catalogue: catalogue().into_iter().take(3).collect(), ..s.clone() };
        assert_eq!(compute(&two_coils(), &small), Err(TreeError::NoSize(1)));
        let no_rule = Settings { criteria: Criteria::default(), ..s };
        assert_eq!(compute(&two_coils(), &no_rule), Err(TreeError::BadSettings("no limit to size by")));
    }

    #[test]
    fn many_networks_at_once_give_what_one_at_a_time_gives() {
        let systems: Vec<(Tree, Settings)> = (1..40).map(|k| {
            let mut t = two_coils();
            t.nodes[2].demand = Demand::Power { watts: 10_000.0 * k as f64, delta_t: 5.0 };
            (t, settings())
        }).collect();
        let together = compute_all(&systems);
        for ((t, s), r) in systems.iter().zip(&together) { assert_eq!(&compute(t, s), r); }
    }

    // made-up table (q = 0.1 * LU^0.6 l/s), not any code's
    fn curve() -> LoadingCurve {
        LoadingCurve::from_csv("lu,l/s\n1,0.1\n10,0.398107\n100,1.584893\n1000,6.309573").unwrap()
    }

    /// source 0 - riser 1 - floors 2 and 3, each with two fixtures (4,5 and 6,7)
    fn building() -> Tree {
        Tree { nodes: vec![
            node(None, Demand::None, 0.0, 0.0, 0.0),
            node(Some(0), Demand::None, 0.0, 12.0, 1.0),
            node(Some(1), Demand::None, 0.0, 8.0, 1.0),
            node(Some(1), Demand::None, 0.0, 14.0, 1.0),
            node(Some(2), Demand::LoadingUnits(6.0), 100_000.0, 5.0, 1.0),
            node(Some(2), Demand::LoadingUnits(14.0), 100_000.0, 9.0, 1.0),
            node(Some(3), Demand::LoadingUnits(6.0), 100_000.0, 4.0, 1.0),
            node(Some(3), Demand::LoadingUnits(2.0), 100_000.0, 6.0, 1.0),
        ] }
    }

    #[test]
    fn loading_units_are_summed_and_then_turned_into_a_flow_so_simultaneity_is_respected() {
        let s = Settings { loading: Some(curve()), ..settings() };
        let r = compute(&building(), &s).unwrap();
        let c = curve();
        let q = |lu: f64| c.flow(lu).unwrap();
        assert!((r.branches[4].flow - q(6.0)).abs() < 1e-12);
        assert!((r.branches[2].flow - q(20.0)).abs() < 1e-12, "the floor takes the curve of 20 units, not 6 + 14 worth of flows");
        assert!((r.branches[1].flow - q(28.0)).abs() < 1e-12);
        assert!((r.branches[0].flow - q(28.0)).abs() < 1e-12);
        assert!(r.branches[1].flow < q(6.0) + q(14.0) + q(6.0) + q(2.0), "diversity: less than all fixtures open together");
        // the floor 2 path (more units, longer pipes at the far fixture) is the more resistant one, and every end has a size
        assert!(r.branches.iter().skip(1).all(|b| b.size.is_some() && b.velocity <= 1.5));
        assert_eq!(*r.critical_path.first().unwrap(), 0);
        assert!(r.ends.iter().all(|e| e.excess >= 0.0) && r.ends.iter().any(|e| e.excess == 0.0));
    }

    #[test]
    fn a_continuous_flow_is_added_after_the_curve_and_a_missing_curve_is_named() {
        let s = Settings { loading: Some(curve()), ..settings() };
        let mut t = building();
        t.nodes.push(node(Some(3), Demand::Flow(0.0005), 0.0, 3.0, 0.0));
        let r = compute(&t, &s).unwrap();
        assert!((r.branches[3].flow - (curve().flow(8.0).unwrap() + 0.0005)).abs() < 1e-12);
        assert_eq!(compute(&building(), &settings()), Err(TreeError::BadSettings("no loading curve for loading units")));
    }

    #[test]
    fn more_units_than_the_table_covers_are_refused_and_the_node_is_named() {
        let s = Settings { loading: Some(curve()), ..settings() };
        let mut t = building();
        t.nodes[7].demand = Demand::LoadingUnits(990.0);
        assert_eq!(compute(&t, &s), Err(TreeError::OutsideCurve(0)));
        t.nodes[5].demand = Demand::LoadingUnits(-1.0);
        assert_eq!(compute(&t, &s), Err(TreeError::BadValue(5, "loading units")));
    }
}
