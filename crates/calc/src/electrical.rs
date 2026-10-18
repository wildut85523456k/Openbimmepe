//! Electrical power distribution: the design current of every branch from the loads downstream, a protective device
//! and a cable for it, voltage drop along the path, and the most loaded path, after the IEC 60364 series
//! (60364-5-52 cable selection, 60364-4-43 overcurrent protection: Ib <= In <= Iz, and the adiabatic short-circuit
//! check S >= I sqrt(t) / k). The cable tables (resistance, reactance, current-carrying capacity per installation
//! method), the device ratings and the k constants are the caller's data; this crate ships none.
//!
//! Loads are added as complex power, so loads with different power factors combine correctly. Three-phase
//! branches carry I = |S| / (sqrt(3) U), single-phase ones I = |S| / U0. Percentages of voltage drop add along a path.

use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phases { Single, Three }

/// An electrical load at an end point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    /// mechanical or rated output, W
    pub watts: f64,
    /// lagging, in (0, 1]
    pub power_factor: f64,
    /// output over input, in (0, 1] (1 for a heater or a light)
    pub efficiency: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CableRow {
    pub label: String,
    pub area_mm2: f64,
    pub r_ohm_per_km: f64,
    pub x_ohm_per_km: f64,
    /// current-carrying capacity for the installation method of the table, A
    pub ampacity_a: f64,
}

/// The cables of one conductor material, insulation and installation method.
#[derive(Clone, Debug, PartialEq)]
pub struct CableTable {
    pub name: String,
    /// the constant of the adiabatic equation for this conductor and insulation
    pub k_adiabatic: f64,
    /// ascending in area and in capacity
    pub rows: Vec<CableRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TableError {
    TooShort,
    Line(usize, &'static str),
}

impl CableTable {
    /// CSV: `label,area_mm2,r_ohm_per_km,x_ohm_per_km,ampacity_a`; a header row, blank lines and `#` comments allowed.
    pub fn from_csv(name: &str, k_adiabatic: f64, text: &str) -> Result<Self, TableError> {
        let mut rows: Vec<CableRow> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let c: Vec<&str> = line.split([',', ';']).map(str::trim).collect();
            if c.len() != 5 { return Err(TableError::Line(n + 1, "five columns are expected")); }
            let nums: Result<Vec<f64>, _> = c[1..].iter().map(|x| x.parse::<f64>()).collect();
            match nums {
                Ok(v) => {
                    if v.iter().any(|x| !x.is_finite() || *x < 0.0) || v[0] <= 0.0 || v[3] <= 0.0 { return Err(TableError::Line(n + 1, "a value is not usable")); }
                    if let Some(prev) = rows.last() { if v[0] <= prev.area_mm2 || v[3] < prev.ampacity_a { return Err(TableError::Line(n + 1, "the table must rise")); } }
                    rows.push(CableRow { label: c[0].to_string(), area_mm2: v[0], r_ohm_per_km: v[1], x_ohm_per_km: v[2], ampacity_a: v[3] });
                }
                Err(_) if rows.is_empty() => continue, // the header
                Err(_) => return Err(TableError::Line(n + 1, "not a number")),
            }
        }
        if rows.is_empty() { return Err(TableError::TooShort); }
        Ok(Self { name: name.to_string(), k_adiabatic, rows })
    }
}

/// Standard ratings of protective devices, one number per line (A), ascending; `#` comments allowed.
pub fn parse_ratings(text: &str) -> Result<Vec<f64>, TableError> {
    let mut out: Vec<f64> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let v: f64 = line.parse().map_err(|_| TableError::Line(n + 1, "not a number"))?;
        if !(v.is_finite() && v > 0.0) || out.last().is_some_and(|p| v <= *p) { return Err(TableError::Line(n + 1, "ratings must be positive and rise")); }
        out.push(v);
    }
    if out.is_empty() { Err(TableError::TooShort) } else { Ok(out) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EBranch {
    pub length_m: f64,
    pub phases: Phases,
    /// which of the settings' tables the cable comes from
    pub table: usize,
    /// product of the correction factors for temperature, grouping and the like (IEC 60364-5-52), 1 if none
    pub derating: f64,
    /// diversity: the share of the downstream load that is on at once, in (0, 1]
    pub demand_factor: f64,
    /// a row the designer fixed: kept, and reported when it fails a check
    pub locked: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ENode {
    pub parent: Option<usize>,
    pub load: Option<Load>,
    pub branch: EBranch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ETree { pub nodes: Vec<ENode> }

#[derive(Clone, Debug, PartialEq)]
pub struct ESettings {
    /// between phases, V (400 for a 230 V phase voltage)
    pub line_voltage: f64,
    pub tables: Vec<CableTable>,
    /// standard device ratings, A, ascending; empty means the design current is the device rating
    pub device_ratings: Vec<f64>,
    /// the whole path from the source to an end point, percent
    pub max_total_drop_pct: f64,
    /// one branch, percent
    pub max_branch_drop_pct: Option<f64>,
    /// prospective short-circuit current of the source (three-phase, rms), A, with the time the device takes to clear it
    pub source_fault: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EError {
    NoSource,
    MoreThanOneSource,
    BadParent(usize),
    Disconnected(usize),
    BadValue(usize, &'static str),
    BadSettings(&'static str),
    /// the design current is above the largest device rating
    NoRating(usize),
    /// no cable in the table is enough
    NoCable(usize),
    /// the path to this end point cannot meet the voltage drop limit even with the largest cables
    DropNotMet(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EBranchResult {
    /// complex power carried, VA (P, Q)
    pub power: (f64, f64),
    /// design current Ib, A
    pub design_current: f64,
    /// protective device In, A
    pub device: f64,
    pub table: usize,
    pub cable: Option<usize>,
    /// Iz = capacity x derating, A
    pub capacity: f64,
    pub drop_v: f64,
    pub drop_pct: f64,
    pub path_drop_pct: f64,
    /// prospective fault current at the start of the cable, A
    pub fault_start: f64,
    /// smallest area that withstands it, mm2
    pub min_area_fault: f64,
    /// a locked cable that fails the capacity, fault withstand or a drop limit
    pub fails: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EEnd { pub node: usize, pub path_drop_pct: f64, pub margin_pct: f64 }

#[derive(Clone, Debug, PartialEq)]
pub struct EResults {
    pub branches: Vec<EBranchResult>,
    pub ends: Vec<EEnd>,
    /// the path with the greatest voltage drop
    pub critical_path: Vec<usize>,
}

fn sqrt3() -> f64 { 3f64.sqrt() }

fn order_of(tree: &ETree, s: &ESettings) -> Result<Vec<usize>, EError> {
    if !(s.line_voltage.is_finite() && s.line_voltage > 0.0) { return Err(EError::BadSettings("the voltage")); }
    if !(s.max_total_drop_pct.is_finite() && s.max_total_drop_pct > 0.0) { return Err(EError::BadSettings("the total drop limit")); }
    if s.max_branch_drop_pct.is_some_and(|x| !(x.is_finite() && x > 0.0)) { return Err(EError::BadSettings("the branch drop limit")); }
    if s.source_fault.is_some_and(|(i, t)| !(i.is_finite() && i > 0.0 && t.is_finite() && t > 0.0)) { return Err(EError::BadSettings("the source fault")); }
    if s.tables.is_empty() { return Err(EError::BadSettings("no cable table")); }
    let n = tree.nodes.len();
    let sources: Vec<usize> = (0..n).filter(|i| tree.nodes[*i].parent.is_none()).collect();
    match sources.len() { 0 => return Err(EError::NoSource), 1 => {}, _ => return Err(EError::MoreThanOneSource) }
    let mut children = vec![Vec::new(); n];
    for (i, nd) in tree.nodes.iter().enumerate() {
        if let Some(p) = nd.parent {
            if p >= n || p == i { return Err(EError::BadParent(i)); }
            children[p].push(i);
            let b = &nd.branch;
            if !(b.length_m.is_finite() && b.length_m >= 0.0) { return Err(EError::BadValue(i, "length")); }
            if !(b.derating.is_finite() && b.derating > 0.0 && b.derating <= 1.0) { return Err(EError::BadValue(i, "derating")); }
            if !(b.demand_factor.is_finite() && b.demand_factor > 0.0 && b.demand_factor <= 1.0) { return Err(EError::BadValue(i, "demand factor")); }
            let Some(t) = s.tables.get(b.table) else { return Err(EError::BadValue(i, "table")) };
            if b.locked.is_some_and(|r| r >= t.rows.len()) { return Err(EError::BadValue(i, "locked cable")); }
        }
        if let Some(l) = nd.load {
            if !(l.watts.is_finite() && l.watts >= 0.0) { return Err(EError::BadValue(i, "watts")); }
            if !(l.power_factor.is_finite() && l.power_factor > 0.0 && l.power_factor <= 1.0) { return Err(EError::BadValue(i, "power factor")); }
            if !(l.efficiency.is_finite() && l.efficiency > 0.0 && l.efficiency <= 1.0) { return Err(EError::BadValue(i, "efficiency")); }
        }
    }
    let mut order = vec![sources[0]];
    let mut k = 0;
    while k < order.len() { let c = &children[order[k]]; order.extend(c.iter().copied()); k += 1; }
    if order.len() != n {
        let seen: std::collections::HashSet<usize> = order.iter().copied().collect();
        return Err(EError::Disconnected((0..n).find(|i| !seen.contains(i)).unwrap_or(0)));
    }
    Ok(order)
}

/// Sizes the protective devices and cables and finds the voltage drop of every branch.
pub fn compute_electrical(tree: &ETree, s: &ESettings) -> Result<EResults, EError> {
    let order = order_of(tree, s)?;
    let n = tree.nodes.len();
    let u_line = s.line_voltage;
    let u0 = u_line / sqrt3();
    // complex power of every branch: own load plus children, times the demand factor of the branch
    let mut power: Vec<(f64, f64)> = tree.nodes.iter().map(|nd| nd.load.map_or((0.0, 0.0), |l| {
        let p = l.watts / l.efficiency;
        (p, p * (1.0 / (l.power_factor * l.power_factor) - 1.0).max(0.0).sqrt())
    })).collect();
    for &i in order.iter().rev() {
        let df = if tree.nodes[i].parent.is_some() { tree.nodes[i].branch.demand_factor } else { 1.0 };
        power[i] = (power[i].0 * df, power[i].1 * df);
        if let Some(p) = tree.nodes[i].parent { let (a, b) = power[i]; power[p].0 += a; power[p].1 += b; }
    }
    // current, device and the first (smallest) cable for the capacity, all branches at once
    struct Start { ib: f64, cosphi: f64, sinphi: f64, device: f64, row: usize }
    let starts: Vec<Result<Option<Start>, EError>> = (0..n).into_par_iter().map(|i| {
        let nd = &tree.nodes[i];
        if nd.parent.is_none() { return Ok(None); }
        let (p, q) = power[i];
        let sva = p.hypot(q);
        let ib = match nd.branch.phases { Phases::Three => sva / (sqrt3() * u_line), Phases::Single => sva / u0 };
        let (cosphi, sinphi) = if sva > 0.0 { (p / sva, q / sva) } else { (1.0, 0.0) };
        let device = if s.device_ratings.is_empty() { ib } else { *s.device_ratings.iter().find(|r| **r >= ib * (1.0 - 1e-12)).ok_or(EError::NoRating(i))? };
        let table = &s.tables[nd.branch.table];
        let row = match nd.branch.locked {
            Some(r) => r,
            None => table.rows.iter().position(|r| r.ampacity_a * nd.branch.derating >= device * (1.0 - 1e-12)).ok_or(EError::NoCable(i))?,
        };
        Ok(Some(Start { ib, cosphi, sinphi, device, row }))
    }).collect();
    let starts: Vec<Option<Start>> = starts.into_iter().collect::<Result<_, _>>()?;
    let mut row: Vec<usize> = starts.iter().map(|x| x.as_ref().map_or(0, |s| s.row)).collect();

    let z_source = s.source_fault.map(|(isc, _)| u0 / isc);
    let limit_row = |i: usize| s.tables[tree.nodes[i].branch.table].rows.len() - 1;
    // the loop: sizes only ever grow, so it ends
    let mut fault_start = vec![0.0f64; n];
    let mut min_area = vec![0.0f64; n];
    let mut drop_v = vec![0.0f64; n];
    let mut drop_pct = vec![0.0f64; n];
    let mut path_pct = vec![0.0f64; n];
    // ends held back only by a cable the designer locked: reported as failing, not an error
    let mut held = vec![false; n];
    for _ in 0..(n * 64 + 64) {
        // impedance from the source to the start of every branch (resistance, reactance), and the drop of each
        let mut zr = vec![0.0f64; n];
        let mut zx = vec![0.0f64; n];
        for &i in &order {
            let nd = &tree.nodes[i];
            let Some(p) = nd.parent else { continue };
            let st = starts[i].as_ref().expect("a start for every branch");
            let cr = &s.tables[nd.branch.table].rows[row[i]];
            let (r, x) = (cr.r_ohm_per_km / 1000.0 * nd.branch.length_m, cr.x_ohm_per_km / 1000.0 * nd.branch.length_m);
            let (k, base) = match nd.branch.phases { Phases::Three => (sqrt3(), u_line), Phases::Single => (2.0, u0) };
            drop_v[i] = k * st.ib * (r * st.cosphi + x * st.sinphi);
            drop_pct[i] = drop_v[i] / base * 100.0;
            path_pct[i] = path_pct[p] + drop_pct[i];
            // the fault current the cable sees at its start: the source and the cables before it
            let (zs_r, zs_x) = (zr[p], zx[p] + z_source.unwrap_or(0.0));
            fault_start[i] = if s.source_fault.is_some() { u0 / zs_r.hypot(zs_x) } else { 0.0 };
            min_area[i] = s.source_fault.map_or(0.0, |(_, t)| fault_start[i] * t.sqrt() / s.tables[nd.branch.table].k_adiabatic);
            let loop_factor = if nd.branch.phases == Phases::Single { 2.0 } else { 1.0 };
            zr[i] = zr[p] + r * loop_factor;
            zx[i] = zx[p] + x * loop_factor;
        }
        let mut changed = false;
        // 1: the fault withstand
        for i in 0..n {
            if tree.nodes[i].parent.is_none() || tree.nodes[i].branch.locked.is_some() { continue; }
            let rows = &s.tables[tree.nodes[i].branch.table].rows;
            if rows[row[i]].area_mm2 < min_area[i] * (1.0 - 1e-12) {
                let next = rows.iter().position(|r| r.area_mm2 >= min_area[i] * (1.0 - 1e-12)).ok_or(EError::NoCable(i))?;
                row[i] = next.max(row[i]);
                changed = true;
            }
        }
        if changed { continue; }
        // 2: a branch over its own limit grows itself
        if let Some(lim) = s.max_branch_drop_pct {
            for i in 0..n {
                if tree.nodes[i].parent.is_some() && tree.nodes[i].branch.locked.is_none() && drop_pct[i] > lim * (1.0 + 1e-12) {
                    if row[i] >= limit_row(i) { return Err(EError::DropNotMet(i)); }
                    row[i] += 1;
                    changed = true;
                }
            }
            if changed { continue; }
        }
        // 3: a path over the total limit: the branch on it that drops most grows one size
        let worst = (0..n).filter(|i| !held[*i] && tree.nodes[*i].parent.is_some() && path_pct[*i] > s.max_total_drop_pct * (1.0 + 1e-12))
            .max_by(|a, b| path_pct[*a].total_cmp(&path_pct[*b]));
        if let Some(end) = worst {
            let mut best: Option<usize> = None;
            let mut m = end;
            let mut locked_on_path = false;
            while let Some(p) = tree.nodes[m].parent {
                locked_on_path |= tree.nodes[m].branch.locked.is_some();
                if tree.nodes[m].branch.locked.is_none() && row[m] < limit_row(m) && best.is_none_or(|b| drop_pct[m] > drop_pct[b]) { best = Some(m); }
                m = p;
            }
            match best { Some(b) => { row[b] += 1; changed = true; } None if locked_on_path => { held[end] = true; changed = true; }
                None => return Err(EError::DropNotMet(end)) }
        }
        if !changed { break; }
    }
    let branches: Vec<EBranchResult> = (0..n).map(|i| {
        let nd = &tree.nodes[i];
        let Some(st) = starts[i].as_ref() else {
            return EBranchResult { power: power[i], design_current: 0.0, device: 0.0, table: 0, cable: None, capacity: 0.0, drop_v: 0.0, drop_pct: 0.0, path_drop_pct: 0.0, fault_start: 0.0, min_area_fault: 0.0, fails: false };
        };
        let cr = &s.tables[nd.branch.table].rows[row[i]];
        let capacity = cr.ampacity_a * nd.branch.derating;
        let fails = capacity < st.device * (1.0 - 1e-12)
            || cr.area_mm2 < min_area[i] * (1.0 - 1e-12)
            || s.max_branch_drop_pct.is_some_and(|l| drop_pct[i] > l * (1.0 + 1e-12))
            || path_pct[i] > s.max_total_drop_pct * (1.0 + 1e-12);
        EBranchResult { power: power[i], design_current: st.ib, device: st.device, table: nd.branch.table, cable: Some(row[i]), capacity, drop_v: drop_v[i], drop_pct: drop_pct[i], path_drop_pct: path_pct[i], fault_start: fault_start[i], min_area_fault: min_area[i], fails }
    }).collect();
    let mut has_child = vec![false; n];
    for nd in &tree.nodes { if let Some(p) = nd.parent { has_child[p] = true; } }
    let ends: Vec<EEnd> = (0..n).filter(|i| tree.nodes[*i].parent.is_some() && (tree.nodes[*i].load.is_some() || !has_child[*i]))
        .map(|i| EEnd { node: i, path_drop_pct: path_pct[i], margin_pct: s.max_total_drop_pct - path_pct[i] }).collect();
    let critical_path = ends.iter().max_by(|a, b| a.path_drop_pct.total_cmp(&b.path_drop_pct)).map_or_else(Vec::new, |e| {
        let mut path = vec![e.node];
        while let Some(p) = tree.nodes[*path.last().unwrap_or(&e.node)].parent { path.push(p); }
        path.reverse();
        path
    });
    Ok(EResults { branches, ends, critical_path })
}

/// Several distribution systems at once, each on its own core.
pub fn compute_all_electrical(systems: &[(ETree, ESettings)]) -> Vec<Result<EResults, EError>> {
    systems.par_iter().map(|(t, s)| compute_electrical(t, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // test data only: plausible but not any standard's table
    fn table() -> CableTable {
        CableTable::from_csv("test copper XLPE", 143.0, "label,area,r,x,amps\n1.5,1.5,14.8,0.10,17\n2.5,2.5,8.91,0.10,24\n4,4,5.57,0.09,32\n6,6,3.71,0.09,42\n10,10,2.24,0.08,58\n16,16,1.38,0.08,77\n25,25,0.873,0.08,102\n35,35,0.629,0.08,126\n50,50,0.464,0.08,153\n70,70,0.321,0.08,196\n").unwrap()
    }
    fn ratings() -> Vec<f64> { parse_ratings("# test\n10\n16\n20\n25\n32\n40\n50\n63\n80\n100\n125\n160\n200\n250\n").unwrap() }
    fn settings() -> ESettings {
        ESettings { line_voltage: 400.0, tables: vec![table()], device_ratings: ratings(), max_total_drop_pct: 5.0, max_branch_drop_pct: None, source_fault: None }
    }
    fn branch(len: f64, phases: Phases) -> EBranch { EBranch { length_m: len, phases, table: 0, derating: 1.0, demand_factor: 1.0, locked: None } }
    fn node(parent: Option<usize>, load: Option<Load>, len: f64) -> ENode { ENode { parent, load, branch: branch(len, Phases::Three) } }
    fn load(kw: f64, pf: f64) -> Option<Load> { Some(Load { watts: kw * 1000.0, power_factor: pf, efficiency: 1.0 }) }

    fn one_load(kw: f64, pf: f64, len: f64) -> ETree {
        ETree { nodes: vec![node(None, None, 0.0), node(Some(0), load(kw, pf), len)] }
    }

    #[test]
    fn a_three_phase_load_gets_its_current_device_cable_and_drop_by_hand_calculation() {
        let r = compute_electrical(&one_load(30.0, 0.85, 40.0), &settings()).unwrap();
        let b = &r.branches[1];
        // I = 30000 / (sqrt3 x 400 x 0.85) = 50.94 A; device 63 A; 16 mm2 carries 77 A (10 mm2 only 58)
        assert!((b.design_current - 50.94).abs() < 0.01, "{}", b.design_current);
        assert_eq!(b.device, 63.0);
        assert_eq!(b.cable, Some(5));
        // drop = sqrt3 x 50.94 x 40 m x (1.38e-3 x 0.85 + 0.08e-3 x 0.5268) = 4.29 V = 1.07 %
        assert!((b.drop_v - 4.288).abs() < 0.01 && (b.drop_pct - 1.072).abs() < 0.005, "{} {}", b.drop_v, b.drop_pct);
        assert!(!b.fails && b.fault_start == 0.0);
    }

    #[test]
    fn derating_makes_the_cable_bigger() {
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].branch.derating = 0.8;
        // 16 mm2 x 0.8 = 61.6 A is under the 63 A device: 25 mm2
        let r = compute_electrical(&t, &settings()).unwrap();
        assert_eq!(r.branches[1].cable, Some(6));
        assert!((r.branches[1].capacity - 81.6).abs() < 1e-9);
    }

    #[test]
    fn loads_with_different_power_factors_add_as_complex_power() {
        let t = ETree { nodes: vec![
            node(None, None, 0.0),
            node(Some(0), None, 10.0),
            node(Some(1), load(30.0, 0.6), 10.0),
            node(Some(1), load(30.0, 1.0), 10.0),
        ] };
        let r = compute_electrical(&t, &settings()).unwrap();
        // 30 kW at 0.6 is 50 kVA, 40 kvar; plus 30 kW: P 60 kW, Q 40 kvar, S 72.11 kVA, not 80
        let (p, q) = r.branches[1].power;
        assert!((p - 60_000.0).abs() < 1e-6 && (q - 40_000.0).abs() < 1e-6);
        assert!((r.branches[1].design_current - 72_111.0 / (sqrt3() * 400.0)).abs() < 0.01);
    }

    #[test]
    fn a_long_run_grows_until_the_total_drop_is_met_and_no_smaller_cable_would_do() {
        let t = one_load(30.0, 0.85, 250.0);
        let r = compute_electrical(&t, &settings()).unwrap();
        let b = &r.branches[1];
        // 16 mm2 would drop 6.7 % over 250 m; 25 mm2 drops 4.3 %
        assert!(b.path_drop_pct <= 5.0 && b.cable == Some(6), "{:?} {}", b.cable, b.path_drop_pct);
        // the next size down breaks the limit (checked by running it locked)
        let mut locked = t.clone();
        locked.nodes[1].branch.locked = Some(b.cable.unwrap() - 1);
        let l = compute_electrical(&locked, &settings()).unwrap();
        assert!(l.branches[1].path_drop_pct > 5.0 && l.branches[1].fails, "{}", l.branches[1].path_drop_pct);
    }

    #[test]
    fn the_adiabatic_check_with_the_source_fault_level_can_ask_for_more_than_the_load_does() {
        let s = ESettings { source_fault: Some((10_000.0, 0.2)), ..settings() };
        let r = compute_electrical(&one_load(30.0, 0.85, 5.0), &s).unwrap();
        let b = &r.branches[1];
        // at the start of the first cable there is only the source impedance: 10 kA; S >= 10000 x sqrt(0.2) / 143 = 31.3 mm2
        assert!((b.fault_start - 10_000.0).abs() < 1e-6 && (b.min_area_fault - 31.27).abs() < 0.05, "{} {}", b.fault_start, b.min_area_fault);
        assert_eq!(b.cable, Some(7), "35 mm2, though 16 carries the current");
    }

    #[test]
    fn the_far_end_is_the_critical_path_and_every_end_reports_its_margin() {
        let t = ETree { nodes: vec![
            node(None, None, 0.0),
            node(Some(0), None, 20.0),
            node(Some(1), load(20.0, 0.9), 10.0),
            node(Some(1), load(20.0, 0.9), 80.0),
        ] };
        let r = compute_electrical(&t, &settings()).unwrap();
        assert_eq!(r.critical_path, vec![0, 1, 3]);
        let e2 = r.ends.iter().find(|e| e.node == 2).unwrap();
        let e3 = r.ends.iter().find(|e| e.node == 3).unwrap();
        assert!(e3.path_drop_pct > e2.path_drop_pct);
        assert!((e3.margin_pct - (5.0 - e3.path_drop_pct)).abs() < 1e-12 && e3.margin_pct >= 0.0);
        assert!((e3.path_drop_pct - r.branches[1].drop_pct - r.branches[3].drop_pct).abs() < 1e-12);
    }

    #[test]
    fn a_single_phase_branch_uses_twice_the_length_and_the_phase_voltage() {
        let mut t = one_load(2.0, 1.0, 30.0);
        t.nodes[1].branch.phases = Phases::Single;
        let r = compute_electrical(&t, &settings()).unwrap();
        let b = &r.branches[1];
        let u0 = 400.0 / sqrt3();
        assert!((b.design_current - 2000.0 / u0).abs() < 1e-9);
        // 8.66 A, device 10 A, 1.5 mm2 (17 A): drop = 2 x 8.66 x 30 x 14.8e-3
        assert!((b.drop_v - 2.0 * (2000.0 / u0) * 30.0 * 14.8e-3).abs() < 1e-9);
        assert!((b.drop_pct - b.drop_v / u0 * 100.0).abs() < 1e-9);
    }

    #[test]
    fn tables_ratings_and_trees_that_cannot_be_used_are_refused_and_named() {
        assert_eq!(CableTable::from_csv("x", 143.0, "a,1,2,3"), Err(TableError::Line(1, "five columns are expected")));
        assert_eq!(CableTable::from_csv("x", 143.0, "l,a,r,x,i\n1,2,1,0.1,10\n2,1,1,0.1,10"), Err(TableError::Line(3, "the table must rise")));
        assert_eq!(CableTable::from_csv("x", 143.0, "l,a,r,x,i"), Err(TableError::TooShort));
        assert_eq!(parse_ratings("16\n10"), Err(TableError::Line(2, "ratings must be positive and rise")));
        // 400 kW is far more than the 250 A device
        assert_eq!(compute_electrical(&one_load(400.0, 0.9, 10.0), &settings()), Err(EError::NoRating(1)));
        // a rating exists but no cable carries it
        let s = ESettings { device_ratings: vec![400.0], ..settings() };
        assert_eq!(compute_electrical(&one_load(100.0, 0.9, 10.0), &s), Err(EError::NoCable(1)));
        // 200 m of the largest cable cannot meet 0.5 %
        let s = ESettings { max_total_drop_pct: 0.5, ..settings() };
        assert_eq!(compute_electrical(&one_load(30.0, 0.85, 400.0), &s), Err(EError::DropNotMet(1)));
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].load = Some(Load { watts: 1.0, power_factor: 0.0, efficiency: 1.0 });
        assert_eq!(compute_electrical(&t, &settings()), Err(EError::BadValue(1, "power factor")));
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].parent = Some(1);
        assert_eq!(compute_electrical(&t, &settings()), Err(EError::BadParent(1)));
    }

    #[test]
    fn many_systems_at_once_give_what_one_at_a_time_gives() {
        let systems: Vec<(ETree, ESettings)> = (1..30).map(|k| (one_load(5.0 * k as f64, 0.9, 20.0 + k as f64), settings())).collect();
        let all = compute_all_electrical(&systems);
        for ((t, s), r) in systems.iter().zip(&all) { assert_eq!(&compute_electrical(t, s), r); }
    }
}
