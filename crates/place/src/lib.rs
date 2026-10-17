//! Spike S2b: place family instances in walls.
//!
//! A `Model` holds wall chains, compiled families and placements. `build` solves every placement, cuts
//! the openings through the S1 wall geometry and returns the net volume. It is a pure function of the
//! model: nothing is changed unless the whole build succeeds, so a bad edit cannot leave a half-cut wall.
//!
//! Frame of a placed family: origin on the wall centreline at `centre` metres from the start of the
//! segment, at the wall base level; x along the wall, y to the left of the wall direction, z up.

use openbim_family::{Compiled, FamilyError, Instance, Request, Scope};
use openbim_geom::{GeomError, Opening, WallChain};
pub mod floors;
pub mod penetrations;
use penetrations::{Penetration, Run, RunProblem};
use rayon::prelude::*;

pub const MAX_PLACEMENTS: usize = 5_000_000;

/// Chains with more segments than this share their segments among the threads.
const LONG_CHAIN: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub family: usize,
    pub type_name: Option<String>,
    /// instance values; `wall_t` is filled in from the host wall when the family has that instance input
    pub overrides: Vec<(String, f64)>,
    /// text, choice and material values, by name
    pub texts: Vec<(String, String)>,
    pub chain: usize,
    pub segment: usize,
    /// distance of the family origin from the start of the segment, along its centreline
    pub centre: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaceError {
    TooMany,
    BadReference { placement: usize, what: &'static str },
    NotFinite { placement: usize },
    Family { placement: usize, err: FamilyError },
    Wall { placement: usize, err: GeomError },
    Chain { chain: usize, err: GeomError },
    /// a service run that cannot be turned into wall openings
    Run { run: usize, problem: RunProblem },
    /// the opening a service run needs does not fit its wall
    Penetration { id: penetrations::PenetrationId, err: GeomError },
}

impl std::fmt::Display for PlaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaceError::TooMany => write!(f, "too many placements"),
            PlaceError::BadReference { placement, what } => write!(f, "placement {placement}: {what}"),
            PlaceError::NotFinite { placement } => write!(f, "placement {placement}: position is not a finite number"),
            PlaceError::Family { placement, err } => write!(f, "placement {placement}: {err}"),
            PlaceError::Wall { placement, err } => write!(f, "placement {placement} does not fit its wall: {err:?}"),
            PlaceError::Chain { chain, err } => write!(f, "wall chain {chain}: {err:?}"),
            PlaceError::Run { run, problem } => write!(f, "run {run}: {}", problem.text()),
            PlaceError::Penetration { id, err } => write!(f, "penetration {id} does not fit its wall: {err:?}"),
        }
    }
}
impl std::error::Error for PlaceError {}

pub struct Model {
    pub chains: Vec<WallChain>,
    pub families: Vec<Compiled>,
    pub placements: Vec<Placement>,
    /// straight service runs (duct, pipe, conduit, cable tray); each wall they cross gets an opening
    pub runs: Vec<Run>,
    /// openings of different runs closer than this (metres) are merged into one rectangular penetration (default 0.40, a project parameter)
    pub merge_gap: f64,
}

impl Default for Model {
    fn default() -> Model {
        Model { chains: vec![], families: vec![], placements: vec![], runs: vec![], merge_gap: penetrations::DEFAULT_GROUP_RADIUS }
    }
}

pub struct Built {
    /// the chains with the family openings added
    pub chains: Vec<WallChain>,
    /// one solved instance per placement, in placement order
    pub instances: Vec<Instance>,
    /// net wall volume of all chains, openings cut
    pub volume: f64,
    /// the register of wall penetrations by service runs, ordered by wall, segment and position
    pub penetrations: Vec<Penetration>,
    /// what `Model::update_built` reuses: the plan of each segment and the volume of each segment and chain
    pub(crate) cache: BuiltCache,
}

/// Per chain: the plan of every segment (they depend on the centreline only), the exact volume of every segment with
/// its openings cut, and the volume of the chain added in segment order.
#[derive(Default)]
pub(crate) struct BuiltCache {
    plans: Vec<Vec<openbim_geom::SegmentPlan>>,
    seg_volumes: Vec<Vec<f64>>,
    chain_volumes: Vec<f64>,
}

/// What a lenient build skipped, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    /// the run that was left out, when the warning is about a run
    pub run: Option<usize>,
    /// the placement that was left out, or None when a whole wall chain was left out
    pub placement: Option<usize>,
    pub error: PlaceError,
}

/// Result of `build_lenient`: everything that is fine is built, the rest is listed in `warnings`.
pub struct LenientBuilt {
    /// all chains, in order; placements that were skipped have no opening in them
    pub chains: Vec<WallChain>,
    /// one entry per placement; None for a placement that was skipped
    pub instances: Vec<Option<Instance>>,
    /// net volume of the valid chains, openings cut; an invalid chain counts for 0
    pub volume: f64,
    /// penetrations of the runs that could be used; `cut` is false for one that did not fit its wall
    pub penetrations: Vec<Penetration>,
    pub warnings: Vec<Warning>,
}

/// A solid of a placed family in world coordinates: an oriented box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBox {
    pub solid: usize,
    /// world position of the box corner with the smallest local x, y, z
    pub origin: [f64; 3],
    /// unit vectors of the local x and y axes in plan (y is the left normal of the wall)
    pub ax: [f64; 2],
    pub ay: [f64; 2],
    /// size along local x, y, z
    pub size: [f64; 3],
}

impl Model {
    /// Checks every reference; returns which chains are valid (each is validated once).
    fn check_refs(&self) -> Result<Vec<bool>, PlaceError> {
        if self.placements.len() > MAX_PLACEMENTS { return Err(PlaceError::TooMany); }
        // each chain is validated once, not once per placement on it (that made a long chain cost n squared)
        let valid: Vec<bool> = self.chains.par_iter().map(|c| c.validate().is_ok()).collect();
        // the placements are checked on all threads; the one with the lowest number that is wrong is reported
        let bad = self.placements.par_iter().enumerate().find_map_first(|(k, p)| {
            if p.family >= self.families.len() { return Some(PlaceError::BadReference { placement: k, what: "no such family" }); }
            let Some(c) = self.chains.get(p.chain) else { return Some(PlaceError::BadReference { placement: k, what: "no such wall chain" }) };
            if !valid[p.chain] { return None; } // reported with the chain below
            if p.segment >= c.segment_count() { return Some(PlaceError::BadReference { placement: k, what: "no such wall segment" }); }
            if !p.centre.is_finite() { return Some(PlaceError::NotFinite { placement: k }); }
            None
        });
        match bad { Some(e) => Err(e), None => Ok(valid) }
    }

    /// Solve placement `k` (its references must be good). `inject`: the family takes `wall_t` per instance.
    fn solve_placement(&self, k: usize, inject: bool) -> Result<Instance, openbim_family::FamilyError> {
        let p = &self.placements[k];
        let mut ov: Vec<(&str, f64)> = p.overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        if inject && !ov.iter().any(|(n, _)| *n == "wall_t") { ov.push(("wall_t", self.chains[p.chain].thickness)); }
        self.families[p.family].solve(&Request { type_name: p.type_name.as_deref(), overrides: ov, texts: p.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() })
    }

    /// Solve every placement and cut the wall openings. Does not change the model.
    pub fn build(&self) -> Result<Built, PlaceError> {
        let valid = self.check_refs()?;
        if let Some(i) = valid.iter().position(|v| !v) { self.chains[i].validate().map_err(|err| PlaceError::Chain { chain: i, err })?; }
        if self.runs.len() > penetrations::MAX_RUNS { return Err(PlaceError::TooMany); }
        let (pens, issues) = penetrations::find(&self.chains, &self.runs, self.merge_gap);
        if let Some(i) = issues.first() { return Err(PlaceError::Run { run: i.run, problem: i.problem }); }
        let mut pens_by_chain: Vec<Vec<&Penetration>> = vec![Vec::new(); self.chains.len()];
        for p in &pens { pens_by_chain[p.id.chain].push(p); }
        // 1. solve every placement on all threads; the request is made on the thread that solves it, so the work of
        // building it is shared too
        let inject: Vec<bool> = self.families.iter().map(|f| f.input_scope("wall_t") == Some(Scope::Instance)).collect();
        let solved: Result<Vec<Instance>, openbim_family::FamilyError> = (0..self.placements.len()).into_par_iter().map(|k| self.solve_placement(k, inject[self.placements[k].family])).collect();
        let instances = match solved {
            Ok(v) => v,
            Err(_) => {
                // the error to report is the first in family order, then placement order; replay to find it
                for (fi, &inj) in inject.iter().enumerate() {
                    for k in (0..self.placements.len()).filter(|&k| self.placements[k].family == fi) {
                        if let Err(err) = self.solve_placement(k, inj) { return Err(PlaceError::Family { placement: k, err }); }
                    }
                }
                unreachable!("a placement failed in parallel but not in the replay")
            }
        };
        // 2. cut the openings, one chain per task; placements keep their order inside a chain
        let mut by_chain: Vec<Vec<usize>> = vec![Vec::new(); self.chains.len()];
        for (k, p) in self.placements.iter().enumerate() { by_chain[p.chain].push(k); }
        let results: Vec<Result<(WallChain, Vec<openbim_geom::SegmentPlan>), PlaceError>> = self.chains.par_iter().zip(by_chain.par_iter()).enumerate().map(|(ci, (c, ks))| {
            let mut c = c.clone();
            let n = c.segment_count();
            if c.openings.len() < n { c.openings.resize(n, Vec::new()); }
            let plans = c.plans().map_err(|err| PlaceError::Chain { chain: ci, err })?;
            let mut cut = c.clone();
            let mut touched: Vec<usize> = Vec::new();
            for &k in ks {
                let p = &self.placements[k];
                if let Some([w, h, sill]) = instances[k].opening {
                    if instances[k].opening_rise == 0.0 {
                        cut.openings[p.segment].push(Opening { offset: p.centre - w / 2.0, width: w, sill, height: h });
                    } else {
                        if cut.poly_openings.len() < n { cut.poly_openings.resize(n, Vec::new()); }
                        let outline = instances[k].opening_outline(p.centre - w / 2.0).expect("opening present");
                        cut.poly_openings[p.segment].push(outline);
                    }
                    touched.push(p.segment);
                }
            }
            touched.sort_unstable();
            touched.dedup();
            // all openings are in: a long chain checks its touched segments on all threads. When one fails, the
            // placements are replayed one by one, so the error names the placement that caused it
            let all_good = if touched.len() > 64 {
                touched.par_iter().all(|&sg| cut.segment_volume_exact(sg, &plans[sg]).is_ok())
            } else {
                touched.iter().all(|&sg| cut.segment_volume_exact(sg, &plans[sg]).is_ok())
            };
            if all_good {
                c = cut;
            } else {
                for &k in ks {
                    let p = &self.placements[k];
                    if let Some([w, h, sill]) = instances[k].opening {
                        if instances[k].opening_rise == 0.0 {
                            c.openings[p.segment].push(Opening { offset: p.centre - w / 2.0, width: w, sill, height: h });
                        } else {
                            if c.poly_openings.len() < n { c.poly_openings.resize(n, Vec::new()); }
                            let outline = instances[k].opening_outline(p.centre - w / 2.0).expect("opening present");
                            c.poly_openings[p.segment].push(outline);
                        }
                        c.segment_volume_exact(p.segment, &plans[p.segment]).map_err(|err| PlaceError::Wall { placement: k, err })?;
                    }
                }
            }
            for pen in &pens_by_chain[ci] {
                c.openings[pen.id.segment].push(Opening { offset: pen.offset, width: pen.width, sill: pen.sill, height: pen.height });
                c.segment_volume_exact(pen.id.segment, &plans[pen.id.segment]).map_err(|err| PlaceError::Penetration { id: pen.id, err })?;
            }
            Ok((c, plans))
        }).collect();
        let (chains, plans): (Vec<WallChain>, Vec<Vec<openbim_geom::SegmentPlan>>) = results.into_iter().collect::<Result<Vec<_>, _>>()?.into_iter().unzip();
        // the volume of every segment, kept so that an edit can redo only the segments it touches; a long chain shares its
        // segments among the threads, and the sums are made in segment, chain and chunk order whatever the thread count
        let to_err = |err| PlaceError::Chain { chain: usize::MAX, err };
        let seg_volumes: Vec<Vec<f64>> = chains.par_iter().zip(plans.par_iter()).map(|(c, pl)| -> Result<Vec<f64>, GeomError> {
            if pl.len() > LONG_CHAIN { pl.par_iter().enumerate().map(|(i, p)| c.segment_volume_exact(i, p)).collect() }
            else { pl.iter().enumerate().map(|(i, p)| c.segment_volume_exact(i, p)).collect() }
        }).collect::<Result<_, _>>().map_err(to_err)?;
        let chain_volumes: Vec<f64> = seg_volumes.iter().map(|v| openbim_geom::chain_volume_from_segments(v)).collect();
        let volume = openbim_geom::combine_chain_volumes(&chain_volumes);
        Ok(Built { chains, instances, volume, penetrations: pens, cache: BuiltCache { plans, seg_volumes, chain_volumes } })
    }

    /// Like `build`, but a bad element does not stop the model: a placement that cannot be solved, has a bad
    /// reference, or does not fit its wall is skipped, and a wall chain that is not valid is left out of the
    /// volume. Each skipped element gets a warning. Only a model with too many placements is refused.
    /// With no warnings the result equals `build`.
    pub fn build_lenient(&self) -> Result<LenientBuilt, PlaceError> {
        if self.placements.len() > MAX_PLACEMENTS { return Err(PlaceError::TooMany); }
        let mut warnings: Vec<Warning> = Vec::new();
        let chain_ok: Vec<bool> = self.chains.iter().enumerate().map(|(i, c)| match c.validate() {
            Ok(()) => true,
            Err(err) => { warnings.push(Warning { run: None, placement: None, error: PlaceError::Chain { chain: i, err } }); false }
        }).collect();
        // a placement is usable when its references are good and its wall chain is valid
        let mut usable = vec![true; self.placements.len()];
        for (k, p) in self.placements.iter().enumerate() {
            let why = if p.family >= self.families.len() { Some("no such family") }
                else if p.chain >= self.chains.len() { Some("no such wall chain") }
                else if !chain_ok[p.chain] { Some("its wall chain is not valid") }
                else if p.segment >= self.chains[p.chain].segment_count() { Some("no such wall segment") }
                else { None };
            if let Some(what) = why { usable[k] = false; warnings.push(Warning { run: None, placement: Some(k), error: PlaceError::BadReference { placement: k, what } }); }
            else if !p.centre.is_finite() { usable[k] = false; warnings.push(Warning { run: None, placement: Some(k), error: PlaceError::NotFinite { placement: k } }); }
        }
        // service runs: a run with a problem is left out; its penetrations are not recorded
        if self.runs.len() > penetrations::MAX_RUNS { return Err(PlaceError::TooMany); }
        let (mut pens, issues) = penetrations::find(&self.chains, &self.runs, self.merge_gap);
        for i in &issues { warnings.push(Warning { run: Some(i.run), placement: None, error: PlaceError::Run { run: i.run, problem: i.problem } }); }
        let mut pens_by_chain: Vec<Vec<usize>> = vec![Vec::new(); self.chains.len()];
        for (n, p) in pens.iter().enumerate() { pens_by_chain[p.id.chain].push(n); }
        // solve the families
        let mut instances: Vec<Option<Instance>> = vec![None; self.placements.len()];
        for (fi, fam) in self.families.iter().enumerate() {
            let idx: Vec<usize> = (0..self.placements.len()).filter(|&k| usable[k] && self.placements[k].family == fi).collect();
            if idx.is_empty() { continue; }
            let inject = fam.input_scope("wall_t") == Some(Scope::Instance);
            let reqs: Vec<Request> = idx.iter().map(|&k| {
                let p = &self.placements[k];
                let mut ov: Vec<(&str, f64)> = p.overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect();
                if inject && !ov.iter().any(|(n, _)| *n == "wall_t") { ov.push(("wall_t", self.chains[p.chain].thickness)); }
                Request { type_name: p.type_name.as_deref(), overrides: ov, texts: p.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() }
            }).collect();
            for (&k, r) in idx.iter().zip(fam.solve_many(&reqs)) {
                match r {
                    Ok(i) => instances[k] = Some(i),
                    Err(err) => warnings.push(Warning { run: None, placement: Some(k), error: PlaceError::Family { placement: k, err } }),
                }
            }
        }
        // cut the openings, one chain per task; a placement that breaks its wall is taken out again
        let mut by_chain: Vec<Vec<usize>> = vec![Vec::new(); self.chains.len()];
        for (k, p) in self.placements.iter().enumerate() { if instances[k].is_some() { by_chain[p.chain].push(k); } }
        type Cut = (WallChain, Vec<usize>, Vec<Warning>, Vec<(usize, GeomError)>);
        let cuts: Vec<Cut> = self.chains.par_iter().zip(by_chain.par_iter()).enumerate().map(|(ci, (c, ks))| {
            let mut c = c.clone();
            let mut dropped = Vec::new();
            let mut warns = Vec::new();
            let mut pen_fail: Vec<(usize, GeomError)> = Vec::new();
            if !chain_ok[ci] { return (c, dropped, warns, pen_fail); }
            let n = c.segment_count();
            if c.openings.len() < n { c.openings.resize(n, Vec::new()); }
            let plans = match c.plans() {
                Ok(p) => p,
                Err(err) => { warns.push(Warning { run: None, placement: None, error: PlaceError::Chain { chain: ci, err } }); return (c, ks.clone(), warns, pen_fail); }
            };
            for &k in ks {
                let p = &self.placements[k];
                let inst = instances[k].as_ref().expect("solved");
                let Some([w, h, sill]) = inst.opening else { continue };
                let poly = inst.opening_rise != 0.0;
                if poly {
                    if c.poly_openings.len() < n { c.poly_openings.resize(n, Vec::new()); }
                    c.poly_openings[p.segment].push(inst.opening_outline(p.centre - w / 2.0).expect("opening present"));
                } else {
                    c.openings[p.segment].push(Opening { offset: p.centre - w / 2.0, width: w, sill, height: h });
                }
                if let Err(err) = c.segment_volume_exact(p.segment, &plans[p.segment]) {
                    if poly { c.poly_openings[p.segment].pop(); } else { c.openings[p.segment].pop(); }
                    dropped.push(k);
                    warns.push(Warning { run: None, placement: Some(k), error: PlaceError::Wall { placement: k, err } });
                }
            }
            for &n in &pens_by_chain[ci] {
                let pen = &pens[n];
                c.openings[pen.id.segment].push(Opening { offset: pen.offset, width: pen.width, sill: pen.sill, height: pen.height });
                if let Err(err) = c.segment_volume_exact(pen.id.segment, &plans[pen.id.segment]) {
                    c.openings[pen.id.segment].pop();
                    pen_fail.push((n, err));
                }
            }
            (c, dropped, warns, pen_fail)
        }).collect();
        let mut chains = Vec::with_capacity(cuts.len());
        for (c, dropped, warns, pen_fail) in cuts {
            for k in dropped { instances[k] = None; }
            warnings.extend(warns);
            for (n, err) in pen_fail {
                pens[n].cut = false;
                let r = pens[n].runs.first().copied();
                warnings.push(Warning { run: r, placement: None, error: PlaceError::Penetration { id: pens[n].id, err } });
            }
            chains.push(c);
        }
        let good: Vec<WallChain> = chains.iter().zip(&chain_ok).filter(|(_, ok)| **ok).map(|(c, _)| c.clone()).collect();
        let volume = openbim_geom::total_volume_parallel(&good).map_err(|err| PlaceError::Chain { chain: usize::MAX, err })?;
        warnings.sort_by_key(|w| (w.placement.is_some() || w.run.is_some(), w.placement.unwrap_or(0), w.run.unwrap_or(0))); // chain warnings first, then by placement, then run
        Ok(LenientBuilt { chains, instances, volume, penetrations: pens, warnings })
    }

    /// Replace placement `index` and bring `built` (the result of `build` on this model) up to date, redoing only what the
    /// change touches: the one placement is solved again, only the wall segments it leaves and enters get their openings
    /// cut again, and only their volumes are computed again. The result is the one `build` gives for the changed model, bit
    /// for bit. When the change cannot be made (a bad reference, a family that does not solve, an opening that does not
    /// fit) the model and `built` stay as they were and the error is the one `build` reports.
    pub fn update_built(&mut self, built: &mut Built, index: usize, new: Placement) -> Result<(), PlaceError> {
        if index >= self.placements.len() { return Err(PlaceError::BadReference { placement: index, what: "no such placement" }); }
        if self.update_quick(built, index, new.clone()) { return Ok(()); }
        let old = std::mem::replace(&mut self.placements[index], new);
        // anything out of the ordinary is left to the full build, which also gives the exact error
        match self.build() {
            Ok(b) => { *built = b; Ok(()) }
            Err(e) => { self.placements[index] = old; Err(e) }
        }
    }

    /// Only the quick path of `update_built`: false, with model and `built` unchanged, when the full build has to decide.
    #[doc(hidden)]
    pub fn update_quick(&mut self, built: &mut Built, index: usize, new: Placement) -> bool {
        if index >= self.placements.len() { return false; }
        let old = std::mem::replace(&mut self.placements[index], new);
        if self.update_in_place(built, index, &old).is_some() { return true; }
        self.placements[index] = old;
        false
    }

    /// The quick path of `update_built`; None (with `built` unchanged) when the full build has to decide.
    fn update_in_place(&self, built: &mut Built, index: usize, old: &Placement) -> Option<()> {
        let n_ch = self.chains.len();
        if built.instances.len() != self.placements.len() || built.chains.len() != n_ch || built.cache.plans.len() != n_ch || built.cache.chain_volumes.len() != n_ch { return None; }
        let p = &self.placements[index];
        if p.family >= self.families.len() || p.chain >= n_ch || p.segment >= self.chains[p.chain].segment_count() || !p.centre.is_finite() { return None; }
        let inject = self.families[p.family].input_scope("wall_t") == Some(Scope::Instance);
        let inst = self.solve_placement(index, inject).ok()?;
        let mut segs = vec![(old.chain, old.segment)];
        if (p.chain, p.segment) != (old.chain, old.segment) { segs.push((p.chain, p.segment)); }
        if segs.iter().any(|&(c, s)| c >= n_ch || s >= self.chains[c].segment_count()) { return None; }
        let instance_of = |k: usize| if k == index { &inst } else { &built.instances[k] };
        // the openings of each segment as `build` makes them: those the chain came with, then the placements in order, then the penetrations
        struct Seg { ci: usize, s: usize, ops: Vec<Opening>, pens: Vec<Opening>, polys: Vec<Vec<[f64; 2]>> }
        let mut new_segs: Vec<Seg> = Vec::new();
        for &(ci, s) in &segs {
            let orig = &self.chains[ci];
            let mut ops = orig.openings.get(s).cloned().unwrap_or_default();
            let mut polys = orig.poly_openings.get(s).cloned().unwrap_or_default();
            for (k, q) in self.placements.iter().enumerate() {
                if q.chain != ci || q.segment != s { continue; }
                let ins = instance_of(k);
                if let Some([w, h, sill]) = ins.opening {
                    if ins.opening_rise == 0.0 { ops.push(Opening { offset: q.centre - w / 2.0, width: w, sill, height: h }); }
                    else { polys.push(ins.opening_outline(q.centre - w / 2.0)?); }
                }
            }
            let pens = built.penetrations.iter().filter(|x| x.id.chain == ci && x.id.segment == s).map(|x| Opening { offset: x.offset, width: x.width, sill: x.sill, height: x.height }).collect();
            new_segs.push(Seg { ci, s, ops, pens, polys });
        }
        // does any chain now hold (or stop holding) a polygon opening: the list of them is as long as the chain only then
        let has_poly = |ci: usize| self.placements.iter().enumerate().any(|(k, q)| q.chain == ci && { let i = instance_of(k); i.opening.is_some() && i.opening_rise != 0.0 });
        // work on the chains in `built`, keeping what is replaced so that a failure puts everything back
        struct Undo { ci: usize, s: usize, ops: Vec<Opening>, polys: Option<Vec<Vec<[f64; 2]>>>, poly_len: usize }
        let mut undo: Vec<Undo> = Vec::new();
        let mut volumes: Vec<(usize, usize, f64)> = Vec::new();
        let mut ok = true;
        for sg in &new_segs {
            let n = self.chains[sg.ci].segment_count();
            let chain = &mut built.chains[sg.ci];
            let orig_poly_len = self.chains[sg.ci].poly_openings.len();
            let want = if has_poly(sg.ci) { orig_poly_len.max(n) } else { orig_poly_len };
            undo.push(Undo { ci: sg.ci, s: sg.s, ops: std::mem::take(&mut chain.openings[sg.s]), polys: chain.poly_openings.get_mut(sg.s).map(std::mem::take), poly_len: chain.poly_openings.len() });
            chain.poly_openings.resize(want, Vec::new());
            if let Some(slot) = chain.poly_openings.get_mut(sg.s) { *slot = sg.polys.clone(); }
            chain.openings[sg.s] = sg.ops.clone();
            let plan = &built.cache.plans[sg.ci][sg.s];
            let mut v = chain.segment_volume_exact(sg.s, plan);
            for pen in &sg.pens {
                if v.is_err() { break; }
                chain.openings[sg.s].push(*pen);
                v = chain.segment_volume_exact(sg.s, plan);
            }
            match v { Ok(x) => volumes.push((sg.ci, sg.s, x)), Err(_) => { ok = false; break; } }
        }
        if !ok {
            for u in undo.into_iter().rev() {
                let chain = &mut built.chains[u.ci];
                chain.poly_openings.resize(u.poly_len, Vec::new());
                if let (Some(pl), Some(slot)) = (u.polys, chain.poly_openings.get_mut(u.s)) { *slot = pl; }
                chain.openings[u.s] = u.ops;
            }
            return None;
        }
        built.instances[index] = inst;
        for (ci, s, x) in volumes { built.cache.seg_volumes[ci][s] = x; }
        for sg in &new_segs {
            built.cache.chain_volumes[sg.ci] = openbim_geom::chain_volume_from_segments(&built.cache.seg_volumes[sg.ci]);
        }
        built.volume = openbim_geom::combine_chain_volumes(&built.cache.chain_volumes);
        Some(())
    }

    /// Replace one placement; keep the change only if the whole model still builds.
    pub fn try_update(&mut self, index: usize, new: Placement) -> Result<Built, PlaceError> {
        if index >= self.placements.len() { return Err(PlaceError::BadReference { placement: index, what: "no such placement" }); }
        let old = std::mem::replace(&mut self.placements[index], new);
        match self.build() {
            Ok(b) => Ok(b),
            Err(e) => { self.placements[index] = old; Err(e) }
        }
    }

    /// World boxes of placement `k` (call with the result of `build`).
    pub fn world_boxes(&self, built: &Built, k: usize) -> Vec<WorldBox> {
        let p = &self.placements[k];
        let c = &self.chains[p.chain];
        let (a, b) = (c.points[p.segment], c.points[(p.segment + 1) % c.points.len()]);
        let d = [b[0] - a[0], b[1] - a[1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let u = [d[0] / l, d[1] / l];
        let nl = [-u[1], u[0]];
        let inst = &built.instances[k];
        let org = [a[0] + u[0] * p.centre, a[1] + u[1] * p.centre];
        inst.solids.iter().enumerate().map(|(n, (si, bx))| {
            // the box in the family frame, then its turn (if any), then the wall frame
            let corner = [bx.min[0], bx.min[1]];
            let (pt, axl, ayl) = match inst.xf[n] { Some(x) => { let (ax, ay) = x.axes(); (x.apply(corner), ax, ay) } None => (corner, [1.0, 0.0], [0.0, 1.0]) };
            let to_world = |v: [f64; 2]| [u[0] * v[0] + nl[0] * v[1], u[1] * v[0] + nl[1] * v[1]];
            let (ax, ay) = (to_world(axl), to_world(ayl));
            let w = to_world(pt);
            WorldBox {
                solid: *si,
                origin: [org[0] + w[0], org[1] + w[1], c.base_z + bx.min[2]],
                ax, ay,
                size: [bx.max[0] - bx.min[0], bx.max[1] - bx.min[1], bx.max[2] - bx.min[2]],
            }
        }).collect()
    }
}
