//! Electrical power distribution after the IEC 60364 series: the design current of every branch from the loads
//! downstream, a protective device and a cable (or several in parallel) for it, voltage drop along the path and at motor
//! starting, short-circuit withstand, protection at the far end against the smallest fault, automatic disconnection
//! for TN, TT and IT systems, the protective conductor, and the most loaded path.
//!
//! References, by clause: 60364-5-52 (cable selection: capacity, correction factors, voltage drop), 60364-4-43
//! (overcurrent: Ib <= In <= Iz, adiabatic S >= I sqrt(t) / k), 60364-4-41 and 60364-5-54 (shock protection, protective
//! conductors). Every table, rating, factor, limit and constant is the caller's data: this crate ships none.
//!
//! Assumptions, so nobody has to guess: balanced three-phase loading; loads add as complex power; percentages of voltage
//! drop add along a path; the source phase-to-earth loop impedance equals its three-phase impedance (usual for a
//! delta-star transformer); a protective conductor is one conductor per branch; the reactance of a return conductor
//! equals the phase conductor's; a fault device opens instantaneously above `trip_multiple x In`.

use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phases { Single, Three }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motor {
    /// starting current over running current
    pub start_multiple: f64,
    /// power factor while starting (lagging)
    pub start_power_factor: f64,
}

/// An electrical load at an end point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    /// rated output, W
    pub watts: f64,
    /// lagging, in (0, 1]
    pub power_factor: f64,
    /// output over input, in (0, 1] (1 for a heater or a light)
    pub efficiency: f64,
    pub motor: Option<Motor>,
}

impl Load {
    pub fn plain(watts: f64, power_factor: f64) -> Self { Self { watts, power_factor, efficiency: 1.0, motor: None } }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lookup { Step, Interpolate }

/// A table of (key, factor): the correction factors of 60364-5-52 as the owner has them.
#[derive(Clone, Debug, PartialEq)]
pub struct FactorTable { lookup: Lookup, points: Vec<(f64, f64)> }

impl FactorTable {
    /// CSV `key,factor` (header, blank lines and `#` comments allowed), keys ascending, factors in (0, 1.5].
    pub fn from_csv(lookup: Lookup, text: &str) -> Result<Self, TableError> {
        let mut points: Vec<(f64, f64)> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let c: Vec<&str> = line.split([',', ';']).map(str::trim).collect();
            if c.len() != 2 { return Err(TableError::Line(n + 1, "two columns are expected")); }
            match (c[0].parse::<f64>(), c[1].parse::<f64>()) {
                (Ok(k), Ok(f)) => {
                    if !(k.is_finite() && f.is_finite() && f > 0.0 && f <= 1.5) { return Err(TableError::Line(n + 1, "a value is not usable")); }
                    if points.last().is_some_and(|p| k <= p.0) { return Err(TableError::Line(n + 1, "the keys must rise")); }
                    points.push((k, f));
                }
                _ if points.is_empty() => continue,
                _ => return Err(TableError::Line(n + 1, "not a number")),
            }
        }
        if points.is_empty() { Err(TableError::TooShort) } else { Ok(Self { lookup, points }) }
    }

    /// The factor at `key`. A step table takes the last row at or below the key (1 below the first row); an
    /// interpolating table draws straight lines between rows and holds the end values.
    pub fn at(&self, key: f64) -> f64 {
        match self.lookup {
            Lookup::Step => self.points.iter().rev().find(|p| p.0 <= key).map_or(1.0, |p| p.1),
            Lookup::Interpolate => {
                let p = &self.points;
                if key <= p[0].0 { return p[0].1; }
                if key >= p[p.len() - 1].0 { return p[p.len() - 1].1; }
                let k = p.partition_point(|x| x.0 < key);
                let ((x0, y0), (x1, y1)) = (p[k - 1], p[k]);
                y0 + (y1 - y0) * (key - x0) / (x1 - x0)
            }
        }
    }
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
    /// factor by number of circuits grouped together (a step table)
    pub grouping: Option<FactorTable>,
    /// factor by ambient temperature, degrees C (an interpolating table)
    pub temperature: Option<FactorTable>,
    /// factor by third-harmonic content of the phase current, percent (a step table), for four-core three-phase cables
    pub harmonics: Option<FactorTable>,
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
        if rows.is_empty() || !(k_adiabatic.is_finite() && k_adiabatic > 0.0) { return Err(TableError::TooShort); }
        Ok(Self { name: name.to_string(), k_adiabatic, rows, grouping: None, temperature: None, harmonics: None })
    }
    pub fn with_grouping(mut self, t: FactorTable) -> Self { self.grouping = Some(t); self }
    pub fn with_temperature(mut self, t: FactorTable) -> Self { self.temperature = Some(t); self }
    pub fn with_harmonics(mut self, t: FactorTable) -> Self { self.harmonics = Some(t); self }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    pub rating_a: f64,
    /// rated short-circuit breaking capacity, A, when the owner gives it
    pub breaking_a: Option<f64>,
}

/// Standard protective devices: CSV `rating_a[,breaking_ka]`, ascending; `#` comments allowed.
pub fn parse_devices(text: &str) -> Result<Vec<Device>, TableError> {
    let mut out: Vec<Device> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let c: Vec<&str> = line.split([',', ';']).map(str::trim).collect();
        let rating = match c[0].parse::<f64>() { Ok(v) => v, Err(_) if out.is_empty() => continue, Err(_) => return Err(TableError::Line(n + 1, "not a number")) };
        let breaking = match c.get(1) {
            None => None,
            Some(b) => Some(b.parse::<f64>().map_err(|_| TableError::Line(n + 1, "not a number"))? * 1000.0),
        };
        if c.len() > 2 { return Err(TableError::Line(n + 1, "one or two columns are expected")); }
        if !(rating.is_finite() && rating > 0.0) || breaking.is_some_and(|b| !(b.is_finite() && b > 0.0)) || out.last().is_some_and(|p| rating <= p.rating_a) {
            return Err(TableError::Line(n + 1, "ratings must be positive and rise"));
        }
        out.push(Device { rating_a: rating, breaking_a: breaking });
    }
    if out.is_empty() { Err(TableError::TooShort) } else { Ok(out) }
}

/// Protective conductor size by phase conductor size: CSV `phase_area_mm2,pe_area_mm2` (the PE for phases from that
/// size up), as the owner's copy of 60364-5-54 gives it.
pub fn parse_pe_rule(text: &str) -> Result<Vec<(f64, f64)>, TableError> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let c: Vec<&str> = line.split([',', ';']).map(str::trim).collect();
        if c.len() != 2 { return Err(TableError::Line(n + 1, "two columns are expected")); }
        match (c[0].parse::<f64>(), c[1].parse::<f64>()) {
            (Ok(a), Ok(b)) => {
                if !(a > 0.0 && b > 0.0) || out.last().is_some_and(|p| a <= p.0) { return Err(TableError::Line(n + 1, "areas must be positive and rise")); }
                out.push((a, b));
            }
            _ if out.is_empty() => continue,
            _ => return Err(TableError::Line(n + 1, "not a number")),
        }
    }
    if out.is_empty() { Err(TableError::TooShort) } else { Ok(out) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EBranch {
    pub length_m: f64,
    pub phases: Phases,
    /// which of the settings' tables the cable comes from
    pub table: usize,
    /// other correction factors for this branch (soil, sun, anything the tables above do not cover), 1 if none
    pub derating: f64,
    /// circuits grouped with this one (itself included): looked up in the table's grouping factors
    pub grouped_circuits: u32,
    /// ambient temperature along the route, degrees C: looked up in the table's temperature factors
    pub ambient_c: Option<f64>,
    /// third-harmonic content of the phase current, percent, for a four-core three-phase branch
    pub harmonic3_pct: f64,
    /// diversity: the share of the downstream load that is on at once, in (0, 1]
    pub demand_factor: f64,
    /// residual-current device sensitivity, mA, if the branch has one
    pub rcd_ma: Option<f64>,
    /// instantaneous trip current over In for this branch's device; the settings' default when None
    pub trip_multiple: Option<f64>,
    /// a row the designer fixed: kept, and reported when it fails a check
    pub locked: Option<usize>,
}

impl EBranch {
    pub fn new(length_m: f64, phases: Phases) -> Self {
        Self { length_m, phases, table: 0, derating: 1.0, grouped_circuits: 1, ambient_c: None, harmonic3_pct: 0.0, demand_factor: 1.0, rcd_ma: None, trip_multiple: None, locked: None }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ENode {
    pub parent: Option<usize>,
    pub load: Option<Load>,
    pub branch: EBranch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ETree { pub nodes: Vec<ENode> }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Earthing {
    Tn,
    /// the earth electrode resistance of the installation, ohm: protection is by residual-current devices
    Tt { electrode_ohm: f64 },
    /// second fault between exposed conductive parts; the neutral may be distributed or not
    It { neutral_distributed: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fault {
    /// prospective three-phase short-circuit current at the source terminals, rms, A
    pub isc_a: f64,
    /// reactance over resistance of the source impedance
    pub x_over_r: f64,
    /// the time the protective device takes to clear a short circuit, s (for the adiabatic check)
    pub clearing_time_s: f64,
    /// voltage factor for the smallest fault current (IEC 60909 c_min), as the owner uses it
    pub c_min: f64,
    /// factor on conductor resistance at fault temperature against the table, for the smallest fault current
    pub hot_resistance_factor: f64,
    /// instantaneous trip current over In, unless a branch says otherwise (the magnetic trip of the curve chosen)
    pub trip_multiple: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ESettings {
    /// between phases, V (400 for a 230 V phase voltage)
    pub line_voltage: f64,
    pub tables: Vec<CableTable>,
    /// standard devices; empty means the design current is the device rating
    pub devices: Vec<Device>,
    /// the whole path from the source to an end point, percent
    pub max_total_drop_pct: f64,
    /// one branch, percent
    pub max_branch_drop_pct: Option<f64>,
    /// motor starting, percent at the motor
    pub max_start_drop_pct: Option<f64>,
    /// the most cables run in parallel for one branch (1 = never)
    pub max_parallel: u32,
    pub earthing: Earthing,
    pub fault: Option<Fault>,
    /// highest touch voltage tolerated, V (50 in normal conditions), for TT and for residual-current devices
    pub touch_voltage_v: f64,
    /// protective conductor by phase size; without it the protective conductor is the phase size
    pub pe_rule: Option<Vec<(f64, f64)>>,
    /// transformer or generator rating, VA, to report its loading
    pub source_va: Option<f64>,
}

impl ESettings {
    pub fn new(line_voltage: f64, tables: Vec<CableTable>, max_total_drop_pct: f64) -> Self {
        Self { line_voltage, tables, devices: Vec::new(), max_total_drop_pct, max_branch_drop_pct: None, max_start_drop_pct: None, max_parallel: 1, earthing: Earthing::Tn, fault: None, touch_voltage_v: 50.0, pe_rule: None, source_va: None }
    }
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
    /// no cable (nor parallel set) in the table is enough
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
    /// row of the table; None for the source
    pub cable: Option<usize>,
    /// cables in parallel
    pub parallel: u32,
    /// the factor applied to the table's capacity: grouping x temperature x harmonics x other
    pub derating: f64,
    /// Iz = cables x capacity x derating, A
    pub capacity: f64,
    pub drop_v: f64,
    pub drop_pct: f64,
    pub path_drop_pct: f64,
    /// prospective fault current at the start of the cable, A
    pub fault_start: f64,
    /// smallest total conductor area that withstands it, mm2
    pub min_area_fault: f64,
    /// the device's breaking capacity covers the fault at its position (true when unknown)
    pub breaking_ok: bool,
    /// the smallest fault current at the far end of the branch (earth fault or line to neutral), A; 0 when not computed
    pub fault_end_min: f64,
    /// the loop impedance to the far end, ohm
    pub loop_impedance: f64,
    /// the device trips on the smallest fault (true when not computed)
    pub trip_ok: bool,
    /// residual-current device check, when the branch has one (or needs one, in TT)
    pub rcd_ok: Option<bool>,
    /// the protective conductor, mm2
    pub pe_area: f64,
    /// a check failed on a cable the designer locked, or one that cannot be improved
    pub fails: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EEnd {
    pub node: usize,
    pub path_drop_pct: f64,
    pub margin_pct: f64,
    /// the drop at the motor while it starts, percent
    pub start_drop_pct: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EResults {
    pub branches: Vec<EBranchResult>,
    pub ends: Vec<EEnd>,
    /// the path with the greatest voltage drop
    pub critical_path: Vec<usize>,
    /// power factor at the source
    pub source_power_factor: f64,
    /// source load over its rating, percent
    pub source_loading_pct: Option<f64>,
}

/// Reactive power to install (var) to bring a load of `watts` and `vars` to a target power factor, never negative.
pub fn correction_vars(watts: f64, vars: f64, target_power_factor: f64) -> f64 {
    if !(target_power_factor > 0.0 && target_power_factor <= 1.0) || watts <= 0.0 { return 0.0; }
    (vars - watts * (1.0 / (target_power_factor * target_power_factor) - 1.0).sqrt()).max(0.0)
}

/// Three-phase short-circuit current at transformer terminals, A, from its rating (VA), secondary line voltage and
/// impedance voltage (percent).
pub fn transformer_fault(va: f64, line_voltage: f64, impedance_pct: f64) -> f64 {
    va / (3f64.sqrt() * line_voltage * impedance_pct / 100.0)
}

fn sqrt3() -> f64 { 3f64.sqrt() }

fn order_of(tree: &ETree, s: &ESettings) -> Result<Vec<usize>, EError> {
    let pos = |v: f64| v.is_finite() && v > 0.0;
    if !pos(s.line_voltage) { return Err(EError::BadSettings("the voltage")); }
    if !pos(s.max_total_drop_pct) { return Err(EError::BadSettings("the total drop limit")); }
    if s.max_branch_drop_pct.is_some_and(|x| !pos(x)) { return Err(EError::BadSettings("the branch drop limit")); }
    if s.max_start_drop_pct.is_some_and(|x| !pos(x)) { return Err(EError::BadSettings("the starting drop limit")); }
    if s.max_parallel == 0 || s.max_parallel > 12 { return Err(EError::BadSettings("parallel cables must be 1 to 12")); }
    if !pos(s.touch_voltage_v) { return Err(EError::BadSettings("the touch voltage")); }
    if s.source_va.is_some_and(|x| !pos(x)) { return Err(EError::BadSettings("the source rating")); }
    if let Some(f) = s.fault {
        if !(pos(f.isc_a) && pos(f.clearing_time_s) && f.x_over_r.is_finite() && f.x_over_r >= 0.0 && pos(f.c_min) && f.c_min <= 1.2 && pos(f.hot_resistance_factor) && pos(f.trip_multiple)) {
            return Err(EError::BadSettings("the fault data"));
        }
    }
    if let Earthing::Tt { electrode_ohm } = s.earthing { if !pos(electrode_ohm) { return Err(EError::BadSettings("the electrode resistance")); } }
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
            if !(pos(b.derating) && b.derating <= 1.0) { return Err(EError::BadValue(i, "derating")); }
            if !(pos(b.demand_factor) && b.demand_factor <= 1.0) { return Err(EError::BadValue(i, "demand factor")); }
            if b.grouped_circuits == 0 { return Err(EError::BadValue(i, "grouped circuits")); }
            if !(b.harmonic3_pct.is_finite() && (0.0..=100.0).contains(&b.harmonic3_pct)) { return Err(EError::BadValue(i, "harmonic content")); }
            if b.ambient_c.is_some_and(|t| !t.is_finite()) { return Err(EError::BadValue(i, "ambient temperature")); }
            if b.rcd_ma.is_some_and(|m| !pos(m)) { return Err(EError::BadValue(i, "residual current")); }
            if b.trip_multiple.is_some_and(|m| !pos(m)) { return Err(EError::BadValue(i, "trip multiple")); }
            let Some(t) = s.tables.get(b.table) else { return Err(EError::BadValue(i, "table")) };
            if b.locked.is_some_and(|r| r >= t.rows.len()) { return Err(EError::BadValue(i, "locked cable")); }
        }
        if let Some(l) = nd.load {
            if !(l.watts.is_finite() && l.watts >= 0.0) { return Err(EError::BadValue(i, "watts")); }
            if !(pos(l.power_factor) && l.power_factor <= 1.0) { return Err(EError::BadValue(i, "power factor")); }
            if !(pos(l.efficiency) && l.efficiency <= 1.0) { return Err(EError::BadValue(i, "efficiency")); }
            if let Some(m) = l.motor {
                if !(pos(m.start_multiple) && m.start_multiple >= 1.0) { return Err(EError::BadValue(i, "start multiple")); }
                if !(pos(m.start_power_factor) && m.start_power_factor <= 1.0) { return Err(EError::BadValue(i, "start power factor")); }
            }
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

/// A cable choice: a table row and the number of cables in parallel.
#[derive(Clone, Copy)]
struct Cand { row: usize, n: u32 }

/// Sizes the protective devices and cables and checks every branch: see the module description.
pub fn compute_electrical(tree: &ETree, s: &ESettings) -> Result<EResults, EError> {
    let order = order_of(tree, s)?;
    let n = tree.nodes.len();
    let u_line = s.line_voltage;
    let u0 = u_line / sqrt3();
    // complex power: the load's own, then children, times the demand factor of the branch
    let own: Vec<(f64, f64)> = tree.nodes.iter().map(|nd| nd.load.map_or((0.0, 0.0), |l| {
        let p = l.watts / l.efficiency;
        (p, p * (1.0 / (l.power_factor * l.power_factor) - 1.0).max(0.0).sqrt())
    })).collect();
    let mut power = own.clone();
    for &i in order.iter().rev() {
        let df = if tree.nodes[i].parent.is_some() { tree.nodes[i].branch.demand_factor } else { 1.0 };
        power[i] = (power[i].0 * df, power[i].1 * df);
        if let Some(p) = tree.nodes[i].parent { let (a, b) = power[i]; power[p].0 += a; power[p].1 += b; }
    }
    // candidate cables for each table: single cables and parallel sets, by total conductor area
    let cands: Vec<Vec<Cand>> = s.tables.iter().map(|t| {
        let mut v: Vec<Cand> = (0..t.rows.len()).flat_map(|row| (1..=s.max_parallel).map(move |n| Cand { row, n })).collect();
        v.sort_by(|a, b| (a.n as f64 * t.rows[a.row].area_mm2).total_cmp(&(b.n as f64 * t.rows[b.row].area_mm2)).then(a.n.cmp(&b.n)));
        v
    }).collect();
    // the correction factor of each branch
    let derating: Vec<f64> = tree.nodes.iter().map(|nd| {
        let b = &nd.branch;
        let t = &s.tables[b.table.min(s.tables.len() - 1)];
        let mut f = b.derating;
        if let Some(g) = &t.grouping { f *= g.at(b.grouped_circuits as f64); }
        if let (Some(tt), Some(a)) = (&t.temperature, b.ambient_c) { f *= tt.at(a); }
        if let (Some(h), Phases::Three) = (&t.harmonics, b.phases) { f *= h.at(b.harmonic3_pct); }
        f
    }).collect();
    let capacity_of = |i: usize, c: Cand| s.tables[tree.nodes[i].branch.table].rows[c.row].ampacity_a * c.n as f64 * derating[i];
    // current, device and the first cable that carries it, all branches at once
    struct Start { ib: f64, device: Device }
    let starts: Vec<Result<Option<(Start, usize)>, EError>> = (0..n).into_par_iter().map(|i| {
        let nd = &tree.nodes[i];
        if nd.parent.is_none() { return Ok(None); }
        let (p, q) = power[i];
        let sva = p.hypot(q);
        let ib = match nd.branch.phases { Phases::Three => sva / (sqrt3() * u_line), Phases::Single => sva / u0 };
        let device = if s.devices.is_empty() { Device { rating_a: ib, breaking_a: None } }
            else { *s.devices.iter().find(|d| d.rating_a >= ib * (1.0 - 1e-12)).ok_or(EError::NoRating(i))? };
        let list = &cands[nd.branch.table];
        let at = match nd.branch.locked {
            Some(r) => list.iter().position(|c| c.row == r && c.n == 1).ok_or(EError::BadValue(i, "locked cable"))?,
            None => list.iter().position(|c| capacity_of(i, *c) >= device.rating_a * (1.0 - 1e-12)).ok_or(EError::NoCable(i))?,
        };
        Ok(Some((Start { ib, device }, at)))
    }).collect();
    let starts: Vec<Option<(Start, usize)>> = starts.into_iter().collect::<Result<_, _>>()?;
    let mut at: Vec<usize> = starts.iter().map(|x| x.as_ref().map_or(0, |s| s.1)).collect();
    let st = |i: usize| &starts[i].as_ref().expect("a start for every branch").0;
    let last = |i: usize| cands[tree.nodes[i].branch.table].len() - 1;
    let cand = |i: usize, at: &Vec<usize>| cands[tree.nodes[i].branch.table][at[i]];
    let row_of = |i: usize, c: Cand| &s.tables[tree.nodes[i].branch.table].rows[c.row];

    // source impedance
    let (z_src_r, z_src_x) = s.fault.map_or((0.0, 0.0), |f| {
        let z = u0 / f.isc_a;
        let r = z / (1.0 + f.x_over_r * f.x_over_r).sqrt();
        (r, r * f.x_over_r)
    });
    // the drop, in percent, of branch i carrying complex power sp at the chosen cable
    let drop_of = |i: usize, c: Cand, sp: (f64, f64)| -> (f64, f64) {
        let nd = &tree.nodes[i];
        let r = row_of(i, c);
        let sva = sp.0.hypot(sp.1);
        let (cos, sin) = if sva > 0.0 { (sp.0 / sva, sp.1 / sva) } else { (1.0, 0.0) };
        let (k, base, ib) = match nd.branch.phases { Phases::Three => (sqrt3(), u_line, sva / (sqrt3() * u_line)), Phases::Single => (2.0, u0, sva / u0) };
        let l = nd.branch.length_m / 1000.0 / c.n as f64;
        let v = k * ib * l * (r.r_ohm_per_km * cos + r.x_ohm_per_km * sin);
        (v, v / base * 100.0)
    };
    let path_of = |end: usize| -> Vec<usize> {
        let mut v = vec![end];
        while let Some(p) = tree.nodes[*v.last().unwrap_or(&end)].parent { v.push(p); }
        v.pop(); // the source has no branch
        v
    };
    // the motor's starting increment of complex power
    let start_increment = |m: usize| -> Option<(f64, f64)> {
        let l = tree.nodes[m].load?;
        let mo = l.motor?;
        let run = own[m];
        let sva = run.0.hypot(run.1) * mo.start_multiple;
        let (cos, sin) = (mo.start_power_factor, (1.0 - mo.start_power_factor * mo.start_power_factor).max(0.0).sqrt());
        Some((sva * cos - run.0, sva * sin - run.1))
    };

    let mut held_drop = vec![false; n];
    let mut held_start = vec![false; n];
    let mut held_trip = vec![false; n];
    let mut st_drop_v = vec![0.0f64; n];
    let mut drop_pct = vec![0.0f64; n];
    let mut path_pct = vec![0.0f64; n];
    let mut fault_start = vec![0.0f64; n];
    let mut min_total = vec![0.0f64; n];
    let mut ia_end = vec![0.0f64; n];
    let mut loop_z = vec![0.0f64; n];
    let mut pe_area = vec![0.0f64; n];
    let mut seg_loop = vec![0.0f64; n];
    let mut start_pct: Vec<Option<f64>> = vec![None; n];
    let trip_of = |i: usize| tree.nodes[i].branch.trip_multiple.or(s.fault.map(|f| f.trip_multiple)).unwrap_or(0.0);
    let pe_for = |area: f64, min: f64, i: usize| -> f64 {
        let ruled = s.pe_rule.as_ref().and_then(|r| r.iter().rev().find(|p| p.0 <= area).map(|p| p.1)).unwrap_or(area);
        let want = ruled.max(min);
        let rows = &s.tables[tree.nodes[i].branch.table].rows;
        rows.iter().map(|r| r.area_mm2).find(|a| *a >= want * (1.0 - 1e-12)).unwrap_or(want.max(area))
    };

    for _ in 0..(n * 64 + 64) {
        // impedance from the source to the start of every branch, then the checks of each
        let mut zr = vec![0.0f64; n];
        let mut zx = vec![0.0f64; n];
        let mut lr = vec![z_src_r; n];
        let mut lx = vec![z_src_x; n];
        for &i in &order {
            let nd = &tree.nodes[i];
            let Some(p) = nd.parent else { continue };
            let c = cand(i, &at);
            let row = row_of(i, c);
            let (v, pct) = drop_of(i, c, power[i]);
            st_drop_v[i] = v;
            drop_pct[i] = pct;
            path_pct[i] = path_pct[p] + pct;
            let len = nd.branch.length_m / 1000.0 / c.n as f64;
            let (r, x) = (row.r_ohm_per_km * len, row.x_ohm_per_km * len);
            if let Some(f) = s.fault {
                let k = s.tables[nd.branch.table].k_adiabatic;
                let z_start = ((z_src_r + zr[p]).powi(2) + (z_src_x + zx[p]).powi(2)).sqrt();
                fault_start[i] = u0 / z_start;
                min_total[i] = fault_start[i] * f.clearing_time_s.sqrt() / k;
                pe_area[i] = pe_for(row.area_mm2 * c.n as f64, fault_start[i] * f.clearing_time_s.sqrt() / k, i);
                // the return conductor: a protective conductor, or the neutral of the same size for a single-phase branch
                let (rr, hot) = (match nd.branch.phases {
                    Phases::Single => r,
                    Phases::Three => row.r_ohm_per_km * nd.branch.length_m / 1000.0 * row.area_mm2 / pe_area[i],
                }, f.hot_resistance_factor);
                lr[i] = lr[p] + hot * (r + rr);
                lx[i] = lx[p] + 2.0 * x;
                seg_loop[i] = ((hot * (r + rr)).powi(2) + (2.0 * x).powi(2)).sqrt();
                loop_z[i] = lr[i].hypot(lx[i]);
                ia_end[i] = match s.earthing {
                    Earthing::Tn => f.c_min * u0 / loop_z[i],
                    Earthing::Tt { .. } => 0.0,
                    Earthing::It { neutral_distributed: false } => 0.8 * u_line / (2.0 * loop_z[i]),
                    Earthing::It { neutral_distributed: true } => 0.8 * u0 / (2.0 * loop_z[i]),
                };
            }
            zr[i] = zr[p] + r;
            zx[i] = zx[p] + x;
        }
        // motor starting drops
        for (m, slot) in start_pct.iter_mut().enumerate() {
            *slot = start_increment(m).map(|inc| {
                path_of(m).iter().map(|&j| drop_of(j, cand(j, &at), (power[j].0 + inc.0, power[j].1 + inc.1)).1).sum()
            });
        }

        let mut changed = false;
        // capacity, then the fault withstand
        for i in 0..n {
            let nd = &tree.nodes[i];
            if nd.parent.is_none() || nd.branch.locked.is_some() { continue; }
            let list = &cands[nd.branch.table];
            let k = s.tables[nd.branch.table].rows.as_slice();
            let ok = |c: Cand| capacity_of(i, c) >= st(i).device.rating_a * (1.0 - 1e-12) && k[c.row].area_mm2 * c.n as f64 >= min_total[i] * (1.0 - 1e-12);
            if !ok(list[at[i]]) {
                let next = (at[i]..list.len()).find(|j| ok(list[*j])).ok_or(EError::NoCable(i))?;
                at[i] = next;
                changed = true;
            }
        }
        if changed { continue; }
        // a branch over its own drop limit grows itself
        if let Some(lim) = s.max_branch_drop_pct {
            for i in 0..n {
                if tree.nodes[i].parent.is_some() && tree.nodes[i].branch.locked.is_none() && drop_pct[i] > lim * (1.0 + 1e-12) {
                    if at[i] >= last(i) { return Err(EError::DropNotMet(i)); }
                    at[i] += 1;
                    changed = true;
                }
            }
            if changed { continue; }
        }
        // a path over a limit: the branch on it that contributes most grows one size
        let bump = |end: usize, held: &mut Vec<bool>, contrib: &dyn Fn(usize) -> f64, at: &mut Vec<usize>| -> Result<bool, EError> {
            let mut best: Option<usize> = None;
            let mut locked_on_path = false;
            for j in path_of(end) {
                locked_on_path |= tree.nodes[j].branch.locked.is_some();
                if tree.nodes[j].branch.locked.is_none() && at[j] < last(j) && best.is_none_or(|b| contrib(j) > contrib(b)) { best = Some(j); }
            }
            match best {
                Some(b) => { at[b] += 1; Ok(true) }
                None if locked_on_path => { held[end] = true; Ok(true) }
                None => Err(EError::DropNotMet(end)),
            }
        };
        let worst = (0..n).filter(|i| !held_drop[*i] && tree.nodes[*i].parent.is_some() && path_pct[*i] > s.max_total_drop_pct * (1.0 + 1e-12))
            .max_by(|a, b| path_pct[*a].total_cmp(&path_pct[*b]));
        if let Some(end) = worst {
            changed = bump(end, &mut held_drop, &|j| drop_pct[j], &mut at)?;
        }
        if changed { continue; }
        if let Some(lim) = s.max_start_drop_pct {
            let worst = (0..n).filter(|m| !held_start[*m]).filter_map(|m| start_pct[m].filter(|v| *v > lim * (1.0 + 1e-12)).map(|v| (m, v)))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((m, _)) = worst {
                let inc = start_increment(m).unwrap_or((0.0, 0.0));
                let contrib = |j: usize| drop_of(j, cand(j, &at), (power[j].0 + inc.0, power[j].1 + inc.1)).1;
                let contrib_vals: Vec<f64> = (0..n).map(|j| if tree.nodes[j].parent.is_some() { contrib(j) } else { 0.0 }).collect();
                changed = bump(m, &mut held_start, &|j| contrib_vals[j], &mut at)?;
            }
        }
        if changed { continue; }
        // the smallest fault must trip the device
        if s.fault.is_some() && matches!(s.earthing, Earthing::Tn | Earthing::It { .. }) {
            let worst = (0..n).filter(|i| !held_trip[*i] && tree.nodes[*i].parent.is_some() && ia_end[*i] < trip_of(*i) * st(*i).device.rating_a * (1.0 - 1e-12))
                .min_by(|a, b| (ia_end[*a] / (trip_of(*a) * st(*a).device.rating_a)).total_cmp(&(ia_end[*b] / (trip_of(*b) * st(*b).device.rating_a))));
            if let Some(end) = worst {
                // a bigger cable lowers the loop impedance; not an error if it cannot be improved: the branch fails
                match bump(end, &mut held_trip, &|j| seg_loop[j], &mut at) { Ok(c) => changed = c, Err(_) => { held_trip[end] = true; changed = true; } }
            }
        }
        if !changed { break; }
    }

    let branches: Vec<EBranchResult> = (0..n).map(|i| {
        let nd = &tree.nodes[i];
        if starts[i].is_none() {
            return EBranchResult { power: power[i], design_current: 0.0, device: 0.0, table: 0, cable: None, parallel: 0, derating: 1.0, capacity: 0.0, drop_v: 0.0, drop_pct: 0.0, path_drop_pct: 0.0, fault_start: 0.0, min_area_fault: 0.0, breaking_ok: true, fault_end_min: 0.0, loop_impedance: 0.0, trip_ok: true, rcd_ok: None, pe_area: 0.0, fails: false };
        }
        let c = cand(i, &at);
        let row = row_of(i, c);
        let capacity = capacity_of(i, c);
        let device = st(i).device;
        let breaking_ok = s.fault.is_none() || device.breaking_a.is_none_or(|b| b >= fault_start[i] * (1.0 - 1e-12));
        let (trip_ok, rcd_ok) = match (s.fault, s.earthing) {
            (None, _) => (true, nd.branch.rcd_ma.map(|_| true)),
            (Some(_), Earthing::Tt { electrode_ohm }) => {
                let rcd = nd.branch.rcd_ma.map(|ma| electrode_ohm * ma / 1000.0 <= s.touch_voltage_v * (1.0 + 1e-12));
                (true, Some(rcd.unwrap_or(false)))
            }
            (Some(_), _) => {
                let trip = ia_end[i] >= trip_of(i) * device.rating_a * (1.0 - 1e-12);
                let rcd = nd.branch.rcd_ma.map(|ma| loop_z[i] * ma / 1000.0 <= s.touch_voltage_v * (1.0 + 1e-12));
                (trip, rcd)
            }
        };
        let fails = capacity < device.rating_a * (1.0 - 1e-12)
            || row.area_mm2 * (c.n as f64) < min_total[i] * (1.0 - 1e-12)
            || s.max_branch_drop_pct.is_some_and(|l| drop_pct[i] > l * (1.0 + 1e-12))
            || path_pct[i] > s.max_total_drop_pct * (1.0 + 1e-12)
            || !breaking_ok || !trip_ok || rcd_ok == Some(false);
        EBranchResult { power: power[i], design_current: st(i).ib, device: device.rating_a, table: nd.branch.table, cable: Some(c.row), parallel: c.n, derating: derating[i], capacity, drop_v: st_drop_v[i], drop_pct: drop_pct[i], path_drop_pct: path_pct[i], fault_start: fault_start[i], min_area_fault: min_total[i], breaking_ok, fault_end_min: ia_end[i], loop_impedance: loop_z[i], trip_ok, rcd_ok, pe_area: pe_area[i], fails }
    }).collect();
    let mut has_child = vec![false; n];
    for nd in &tree.nodes { if let Some(p) = nd.parent { has_child[p] = true; } }
    let ends: Vec<EEnd> = (0..n).filter(|i| tree.nodes[*i].parent.is_some() && (tree.nodes[*i].load.is_some() || !has_child[*i]))
        .map(|i| EEnd { node: i, path_drop_pct: path_pct[i], margin_pct: s.max_total_drop_pct - path_pct[i], start_drop_pct: start_pct[i] }).collect();
    let critical_path = ends.iter().max_by(|a, b| a.path_drop_pct.total_cmp(&b.path_drop_pct)).map_or_else(Vec::new, |e| {
        let mut p = path_of(e.node);
        p.push(order[0]);
        p.reverse();
        p
    });
    let root = power[order[0]];
    let sva = root.0.hypot(root.1);
    Ok(EResults {
        branches, ends, critical_path,
        source_power_factor: if sva > 0.0 { root.0 / sva } else { 1.0 },
        source_loading_pct: s.source_va.map(|v| sva / v * 100.0),
    })
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
    fn devices() -> Vec<Device> { parse_devices("# test\n10\n16\n20\n25\n32\n40\n50\n63\n80\n100\n125\n160\n200\n250\n").unwrap() }
    fn settings() -> ESettings { ESettings { devices: devices(), ..ESettings::new(400.0, vec![table()], 5.0) } }
    fn fault(isc: f64, t: f64) -> Fault { Fault { isc_a: isc, x_over_r: 0.0, clearing_time_s: t, c_min: 0.95, hot_resistance_factor: 1.5, trip_multiple: 10.0 } }
    fn node(parent: Option<usize>, load: Option<Load>, len: f64) -> ENode { ENode { parent, load, branch: EBranch::new(len, Phases::Three) } }
    fn load(kw: f64, pf: f64) -> Option<Load> { Some(Load::plain(kw * 1000.0, pf)) }
    fn one_load(kw: f64, pf: f64, len: f64) -> ETree { ETree { nodes: vec![node(None, None, 0.0), node(Some(0), load(kw, pf), len)] } }
    fn near(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() <= tol }

    #[test]
    fn a_three_phase_load_gets_its_current_device_cable_and_drop_by_hand_calculation() {
        let r = compute_electrical(&one_load(30.0, 0.85, 40.0), &settings()).unwrap();
        let b = &r.branches[1];
        // I = 30000 / (sqrt3 x 400 x 0.85) = 50.94 A; device 63 A; 16 mm2 carries 77 A (10 mm2 only 58)
        assert!(near(b.design_current, 50.94, 0.01), "{}", b.design_current);
        assert_eq!(b.device, 63.0);
        assert_eq!((b.cable, b.parallel), (Some(5), 1));
        // drop = sqrt3 x 50.94 x 40 m x (1.38e-3 x 0.85 + 0.08e-3 x 0.5268) = 4.29 V = 1.07 %
        assert!(near(b.drop_v, 4.288, 0.01) && near(b.drop_pct, 1.072, 0.005), "{} {}", b.drop_v, b.drop_pct);
        assert!(!b.fails && b.fault_start == 0.0 && b.trip_ok && b.breaking_ok);
    }

    #[test]
    fn correction_factors_are_looked_up_in_the_tables_and_multiply() {
        let g = FactorTable::from_csv(Lookup::Step, "circuits,f\n1,1.0\n2,0.8\n3,0.7\n4,0.65").unwrap();
        let t = FactorTable::from_csv(Lookup::Interpolate, "30,1.0\n40,0.87\n50,0.71").unwrap();
        let h = FactorTable::from_csv(Lookup::Step, "0,1.0\n15,0.86\n33,0.7").unwrap();
        assert_eq!((g.at(0.0), g.at(3.0), g.at(9.0)), (1.0, 0.7, 0.65));
        assert!(near(t.at(35.0), 0.935, 1e-12) && t.at(10.0) == 1.0 && t.at(80.0) == 0.71);
        assert_eq!((h.at(10.0), h.at(20.0), h.at(40.0)), (1.0, 0.86, 0.7));
        let tab = table().with_grouping(g).with_temperature(t).with_harmonics(h);
        let mut tr = one_load(30.0, 0.85, 40.0);
        tr.nodes[1].branch.grouped_circuits = 3;
        tr.nodes[1].branch.ambient_c = Some(35.0);
        tr.nodes[1].branch.harmonic3_pct = 20.0;
        tr.nodes[1].branch.derating = 0.9;
        let s = ESettings { devices: devices(), ..ESettings::new(400.0, vec![tab], 5.0) };
        let r = compute_electrical(&tr, &s).unwrap();
        let f = 0.9 * 0.7 * 0.935 * 0.86;
        let b = &r.branches[1];
        assert!(near(b.derating, f, 1e-12), "{}", b.derating);
        // 63 A device / 0.506 = 124.5 A: 35 mm2 (126)
        assert_eq!(b.cable, Some(7));
        assert!(near(b.capacity, 126.0 * f, 1e-9));
        // a single-phase branch ignores the harmonic factor
        tr.nodes[1].branch.phases = Phases::Single;
        tr.nodes[1].load = load(5.0, 0.85);
        let r = compute_electrical(&tr, &s).unwrap();
        assert!(near(r.branches[1].derating, 0.9 * 0.7 * 0.935, 1e-12));
        assert_eq!(FactorTable::from_csv(Lookup::Step, "2,1.0\n1,0.9"), Err(TableError::Line(2, "the keys must rise")));
    }

    #[test]
    fn loads_with_different_power_factors_add_as_complex_power() {
        let t = ETree { nodes: vec![node(None, None, 0.0), node(Some(0), None, 10.0), node(Some(1), load(30.0, 0.6), 10.0), node(Some(1), load(30.0, 1.0), 10.0)] };
        let r = compute_electrical(&t, &settings()).unwrap();
        // 30 kW at 0.6 is 50 kVA, 40 kvar; plus 30 kW: P 60 kW, Q 40 kvar, S 72.11 kVA, not 80
        let (p, q) = r.branches[1].power;
        assert!(near(p, 60_000.0, 1e-6) && near(q, 40_000.0, 1e-6));
        assert!(near(r.branches[1].design_current, 72_111.0 / (sqrt3() * 400.0), 0.01));
        assert!(near(r.source_power_factor, 60.0 / 72.111, 1e-4));
        // 60 kW, 40 kvar to 0.95: 40 - 60 x 0.32868 = 20.28 kvar
        assert!(near(correction_vars(60_000.0, 40_000.0, 0.95), 20_279.0, 5.0));
        assert_eq!(correction_vars(60_000.0, 10_000.0, 0.95), 0.0);
    }

    #[test]
    fn a_long_run_grows_until_the_total_drop_is_met_and_no_smaller_cable_would_do() {
        let t = one_load(30.0, 0.85, 250.0);
        let r = compute_electrical(&t, &settings()).unwrap();
        let b = &r.branches[1];
        // 16 mm2 would drop 6.7 % over 250 m; 25 mm2 drops 4.3 %
        assert!(b.path_drop_pct <= 5.0 && b.cable == Some(6), "{:?} {}", b.cable, b.path_drop_pct);
        let mut locked = t.clone();
        locked.nodes[1].branch.locked = Some(5);
        let l = compute_electrical(&locked, &settings()).unwrap();
        assert!(l.branches[1].path_drop_pct > 5.0 && l.branches[1].fails);
    }

    #[test]
    fn parallel_cables_carry_what_no_single_cable_does_and_halve_the_impedance() {
        let t = one_load(150.0, 0.9, 30.0);
        assert_eq!(compute_electrical(&t, &settings()), Err(EError::NoCable(1)));
        let s = ESettings { max_parallel: 2, ..settings() };
        let r = compute_electrical(&t, &s).unwrap();
        let b = &r.branches[1];
        // I = 150000 / (sqrt3 x 400 x 0.9) = 240.6 A; device 250 A; 70 mm2 carries 196; two 35 mm2 carry 252
        assert_eq!((b.device, b.cable, b.parallel), (250.0, Some(7), 2));
        assert!(near(b.capacity, 252.0, 1e-9));
        let sin = (1.0f64 - 0.81).sqrt();
        let v = sqrt3() * b.design_current * 0.030 / 2.0 * (0.629 * 0.9 + 0.08 * sin);
        assert!(near(b.drop_v, v, 1e-9), "{} {}", b.drop_v, v);
        assert_eq!(compute_electrical(&t, &ESettings { max_parallel: 0, ..settings() }), Err(EError::BadSettings("parallel cables must be 1 to 12")));
    }

    #[test]
    fn short_circuit_withstand_the_smallest_fault_and_the_protective_conductor_by_hand_calculation() {
        let s = ESettings { fault: Some(fault(10_000.0, 0.05)), ..settings() };
        let r = compute_electrical(&one_load(30.0, 0.85, 40.0), &s).unwrap();
        let b = &r.branches[1];
        // 10 kA at the start; S >= 10000 x sqrt(0.05) / 143 = 15.64 mm2: the 16 mm2 passes
        assert!(near(b.fault_start, 10_000.0, 1e-6) && near(b.min_area_fault, 15.64, 0.01) && b.cable == Some(5));
        // loop to the far end: source 230.94/10000 = 0.023094; cable 1.38 x 0.04 = 0.0552 ohm each way, hot x1.5:
        // R = 0.023094 + 1.5 x 0.1104 = 0.188694, X = 2 x 0.0032; Ia = 0.95 x 230.94 / |Z| = 1162 A >= 10 x 63
        assert!(near(b.loop_impedance, 0.188805, 1e-5) && near(b.fault_end_min, 1162.0, 1.0), "{} {}", b.loop_impedance, b.fault_end_min);
        assert!(b.trip_ok && b.breaking_ok && b.rcd_ok.is_none() && b.pe_area == 16.0);
    }

    #[test]
    fn a_long_branch_whose_smallest_fault_would_not_trip_the_device_gets_a_bigger_cable() {
        let s = ESettings { fault: Some(fault(10_000.0, 0.05)), ..settings() };
        let t = one_load(30.0, 0.85, 150.0);
        let r = compute_electrical(&t, &s).unwrap();
        let b = &r.branches[1];
        assert!(b.trip_ok && b.fault_end_min >= 630.0 * (1.0 - 1e-9) && b.cable.unwrap() > 5, "{:?} {}", b.cable, b.fault_end_min);
        // one size down does not trip
        let mut locked = t.clone();
        locked.nodes[1].branch.locked = Some(b.cable.unwrap() - 1);
        let l = compute_electrical(&locked, &s).unwrap();
        assert!(!l.branches[1].trip_ok && l.branches[1].fails);
        // a magnetic trip of 5 x In needs less
        let mut soft = t.clone();
        soft.nodes[1].branch.trip_multiple = Some(5.0);
        let r5 = compute_electrical(&soft, &s).unwrap();
        assert!(r5.branches[1].cable.unwrap() <= b.cable.unwrap());
    }

    #[test]
    fn a_device_that_cannot_break_the_fault_is_reported() {
        let s = ESettings { devices: parse_devices("rating,kA\n63,6\n125,10\n").unwrap(), fault: Some(fault(10_000.0, 0.05)), ..settings() };
        let r = compute_electrical(&one_load(30.0, 0.85, 40.0), &s).unwrap();
        assert!(!r.branches[1].breaking_ok && r.branches[1].fails);
        let ok = ESettings { devices: parse_devices("63,10\n").unwrap(), ..s };
        assert!(compute_electrical(&one_load(30.0, 0.85, 40.0), &ok).unwrap().branches[1].breaking_ok);
    }

    #[test]
    fn tt_needs_a_residual_current_device_and_it_uses_the_second_fault_loop() {
        let mut t = one_load(30.0, 0.85, 40.0);
        let tt = |ohm: f64| ESettings { earthing: Earthing::Tt { electrode_ohm: ohm }, fault: Some(fault(10_000.0, 0.05)), ..settings() };
        // no RCD: not protected
        assert_eq!(compute_electrical(&t, &tt(20.0)).unwrap().branches[1].rcd_ok, Some(false));
        t.nodes[1].branch.rcd_ma = Some(300.0);
        assert_eq!(compute_electrical(&t, &tt(20.0)).unwrap().branches[1].rcd_ok, Some(true), "20 ohm x 0.3 A = 6 V");
        assert_eq!(compute_electrical(&t, &tt(200.0)).unwrap().branches[1].rcd_ok, Some(false), "200 ohm x 0.3 A = 60 V");
        assert!(compute_electrical(&t, &tt(200.0)).unwrap().branches[1].fails);
        // IT: Ia = 0.8 U / (2 Zs) without a distributed neutral
        let it = ESettings { earthing: Earthing::It { neutral_distributed: false }, fault: Some(fault(10_000.0, 0.05)), ..settings() };
        let tn = ESettings { fault: Some(fault(10_000.0, 0.05)), ..settings() };
        let (bi, bt) = (&compute_electrical(&t, &it).unwrap().branches[1].clone(), compute_electrical(&t, &tn).unwrap().branches[1].clone());
        assert!(near(bi.fault_end_min, 0.8 * 400.0 / (2.0 * bi.loop_impedance), 1e-9));
        assert!(bi.fault_end_min < bt.fault_end_min);
        // an RCD on a TN branch: Zs x IDn <= 50 V (0.1888 ohm x 0.3 A)
        assert_eq!(compute_electrical(&t, &tn).unwrap().branches[1].rcd_ok, Some(true));
    }

    #[test]
    fn the_protective_conductor_follows_the_rule_and_never_less_than_the_adiabatic_minimum() {
        let rule = parse_pe_rule("phase,pe\n16,16\n35,16\n50,25").unwrap();
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].branch.locked = Some(9); // 70 mm2
        let s = ESettings { fault: Some(fault(10_000.0, 0.05)), pe_rule: Some(rule.clone()), ..settings() };
        assert_eq!(compute_electrical(&t, &s).unwrap().branches[1].pe_area, 25.0);
        // a hard fault asks for more than the rule: 30 kA for 0.1 s needs 66 mm2 -> the 70 mm2 row
        let hard = ESettings { fault: Some(fault(30_000.0, 0.1)), pe_rule: Some(rule), ..settings() };
        assert_eq!(compute_electrical(&t, &hard).unwrap().branches[1].pe_area, 70.0);
        assert_eq!(parse_pe_rule("16,16\n10,10"), Err(TableError::Line(2, "areas must be positive and rise")));
    }

    #[test]
    fn motor_starting_drops_are_computed_and_limit_the_cable() {
        let m = Load { watts: 30_000.0, power_factor: 0.85, efficiency: 0.9, motor: Some(Motor { start_multiple: 6.0, start_power_factor: 0.4 }) };
        let mut t = one_load(0.0, 1.0, 100.0);
        t.nodes[1].load = Some(m);
        let free = compute_electrical(&t, &settings()).unwrap();
        let b = &free.branches[1];
        // running: 33.33 kW / 0.85 = 39.2 kVA, 56.6 A; starting: 6 x 56.6 = 339.6 A at pf 0.4
        let i_run = 30_000.0 / 0.9 / 0.85 / (sqrt3() * 400.0);
        let i_start = 6.0 * i_run;
        let row = &table().rows[b.cable.unwrap()];
        let expect = sqrt3() * i_start * 0.1 * (row.r_ohm_per_km * 0.4 + row.x_ohm_per_km * (1.0f64 - 0.16).sqrt()) / 400.0 * 100.0;
        assert!(near(free.ends[0].start_drop_pct.unwrap(), expect, 1e-9), "{:?} {}", free.ends[0].start_drop_pct, expect);
        // a limit of 5 % at start makes the cable bigger
        let s = ESettings { max_start_drop_pct: Some(5.0), ..settings() };
        let lim = compute_electrical(&t, &s).unwrap();
        assert!(lim.ends[0].start_drop_pct.unwrap() <= 5.0 && lim.branches[1].cable.unwrap() > b.cable.unwrap(), "{:?}", lim.ends[0].start_drop_pct);
        assert!(free.ends[0].start_drop_pct.unwrap() > 5.0);
    }

    #[test]
    fn the_far_end_is_the_critical_path_and_every_end_reports_its_margin() {
        let t = ETree { nodes: vec![node(None, None, 0.0), node(Some(0), None, 20.0), node(Some(1), load(20.0, 0.9), 10.0), node(Some(1), load(20.0, 0.9), 80.0)] };
        let r = compute_electrical(&t, &settings()).unwrap();
        assert_eq!(r.critical_path, vec![0, 1, 3]);
        let e2 = r.ends.iter().find(|e| e.node == 2).unwrap();
        let e3 = r.ends.iter().find(|e| e.node == 3).unwrap();
        assert!(e3.path_drop_pct > e2.path_drop_pct && e3.start_drop_pct.is_none());
        assert!(near(e3.margin_pct, 5.0 - e3.path_drop_pct, 1e-12) && e3.margin_pct >= 0.0);
        assert!(near(e3.path_drop_pct, r.branches[1].drop_pct + r.branches[3].drop_pct, 1e-12));
    }

    #[test]
    fn a_single_phase_branch_uses_twice_the_length_and_the_phase_voltage() {
        let mut t = one_load(2.0, 1.0, 30.0);
        t.nodes[1].branch.phases = Phases::Single;
        let r = compute_electrical(&t, &settings()).unwrap();
        let b = &r.branches[1];
        let u0 = 400.0 / sqrt3();
        assert!(near(b.design_current, 2000.0 / u0, 1e-9));
        assert!(near(b.drop_v, 2.0 * (2000.0 / u0) * 30.0 * 14.8e-3, 1e-9));
        assert!(near(b.drop_pct, b.drop_v / u0 * 100.0, 1e-9));
    }

    #[test]
    fn the_source_reports_its_loading_and_a_transformer_gives_its_fault_level() {
        let s = ESettings { source_va: Some(100_000.0), ..settings() };
        let r = compute_electrical(&one_load(30.0, 0.85, 40.0), &s).unwrap();
        assert!(near(r.source_loading_pct.unwrap(), 30_000.0 / 0.85 / 100_000.0 * 100.0, 1e-9));
        assert!(near(transformer_fault(1_000_000.0, 400.0, 5.0), 28_867.5, 0.5));
    }

    #[test]
    fn tables_ratings_and_trees_that_cannot_be_used_are_refused_and_named() {
        assert_eq!(CableTable::from_csv("x", 143.0, "a,1,2,3"), Err(TableError::Line(1, "five columns are expected")));
        assert_eq!(CableTable::from_csv("x", 143.0, "l,a,r,x,i\n1,2,1,0.1,10\n2,1,1,0.1,10"), Err(TableError::Line(3, "the table must rise")));
        assert_eq!(CableTable::from_csv("x", 143.0, "l,a,r,x,i"), Err(TableError::TooShort));
        assert_eq!(parse_devices("16\n10"), Err(TableError::Line(2, "ratings must be positive and rise")));
        assert_eq!(parse_devices("16,x"), Err(TableError::Line(1, "not a number")));
        assert_eq!(compute_electrical(&one_load(400.0, 0.9, 10.0), &settings()), Err(EError::NoRating(1)));
        let s = ESettings { devices: vec![Device { rating_a: 400.0, breaking_a: None }], ..settings() };
        assert_eq!(compute_electrical(&one_load(100.0, 0.9, 10.0), &s), Err(EError::NoCable(1)));
        let s = ESettings { max_total_drop_pct: 0.5, ..settings() };
        assert_eq!(compute_electrical(&one_load(30.0, 0.85, 400.0), &s), Err(EError::DropNotMet(1)));
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].load = Some(Load::plain(1.0, 0.0));
        assert_eq!(compute_electrical(&t, &settings()), Err(EError::BadValue(1, "power factor")));
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].parent = Some(1);
        assert_eq!(compute_electrical(&t, &settings()), Err(EError::BadParent(1)));
        let mut t = one_load(30.0, 0.85, 40.0);
        t.nodes[1].branch.grouped_circuits = 0;
        assert_eq!(compute_electrical(&t, &settings()), Err(EError::BadValue(1, "grouped circuits")));
        let bad = ESettings { fault: Some(Fault { isc_a: 0.0, ..fault(1.0, 1.0) }), ..settings() };
        assert_eq!(compute_electrical(&one_load(30.0, 0.85, 40.0), &bad), Err(EError::BadSettings("the fault data")));
    }

    #[test]
    fn many_systems_at_once_give_what_one_at_a_time_gives() {
        let systems: Vec<(ETree, ESettings)> = (1..30).map(|k| (one_load(5.0 * k as f64, 0.9, 20.0 + k as f64), ESettings { fault: Some(fault(15_000.0, 0.1)), ..settings() })).collect();
        let all = compute_all_electrical(&systems);
        for ((t, s), r) in systems.iter().zip(&all) { assert_eq!(&compute_electrical(t, s), r); }
    }
}
