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
use rayon::prelude::*;

pub const MAX_PLACEMENTS: usize = 5_000_000;

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
        }
    }
}
impl std::error::Error for PlaceError {}

#[derive(Default)]
pub struct Model {
    pub chains: Vec<WallChain>,
    pub families: Vec<Compiled>,
    pub placements: Vec<Placement>,
}

pub struct Built {
    /// the chains with the family openings added
    pub chains: Vec<WallChain>,
    /// one solved instance per placement, in placement order
    pub instances: Vec<Instance>,
    /// net wall volume of all chains, openings cut
    pub volume: f64,
}

/// What a lenient build skipped, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
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
    fn check_refs(&self) -> Result<(), PlaceError> {
        if self.placements.len() > MAX_PLACEMENTS { return Err(PlaceError::TooMany); }
        for (k, p) in self.placements.iter().enumerate() {
            if p.family >= self.families.len() { return Err(PlaceError::BadReference { placement: k, what: "no such family" }); }
            let Some(c) = self.chains.get(p.chain) else { return Err(PlaceError::BadReference { placement: k, what: "no such wall chain" }) };
            if c.validate().is_err() { continue; } // reported with the chain below
            if p.segment >= c.segment_count() { return Err(PlaceError::BadReference { placement: k, what: "no such wall segment" }); }
            if !p.centre.is_finite() { return Err(PlaceError::NotFinite { placement: k }); }
        }
        Ok(())
    }

    /// Solve every placement and cut the wall openings. Does not change the model.
    pub fn build(&self) -> Result<Built, PlaceError> {
        self.check_refs()?;
        for (i, c) in self.chains.iter().enumerate() { c.validate().map_err(|err| PlaceError::Chain { chain: i, err })?; }
        // 1. solve the families, one parallel batch per family
        let mut instances: Vec<Option<Instance>> = vec![None; self.placements.len()];
        for (fi, fam) in self.families.iter().enumerate() {
            let idx: Vec<usize> = (0..self.placements.len()).filter(|&k| self.placements[k].family == fi).collect();
            if idx.is_empty() { continue; }
            let inject = fam.input_scope("wall_t") == Some(Scope::Instance);
            let reqs: Vec<Request> = idx.iter().map(|&k| {
                let p = &self.placements[k];
                let mut ov: Vec<(&str, f64)> = p.overrides.iter().map(|(n, v)| (n.as_str(), *v)).collect();
                if inject && !ov.iter().any(|(n, _)| *n == "wall_t") { ov.push(("wall_t", self.chains[p.chain].thickness)); }
                Request { type_name: p.type_name.as_deref(), overrides: ov, texts: p.texts.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect() }
            }).collect();
            for (&k, r) in idx.iter().zip(fam.solve_many(&reqs)) {
                instances[k] = Some(r.map_err(|err| PlaceError::Family { placement: k, err })?);
            }
        }
        let instances: Vec<Instance> = instances.into_iter().map(|i| i.expect("every placement solved")).collect();
        // 2. cut the openings, one chain per task; placements keep their order inside a chain
        let mut by_chain: Vec<Vec<usize>> = vec![Vec::new(); self.chains.len()];
        for (k, p) in self.placements.iter().enumerate() { by_chain[p.chain].push(k); }
        let results: Vec<Result<WallChain, PlaceError>> = self.chains.par_iter().zip(by_chain.par_iter()).enumerate().map(|(ci, (c, ks))| {
            let mut c = c.clone();
            let n = c.segment_count();
            if c.openings.len() < n { c.openings.resize(n, Vec::new()); }
            let plans = c.plans().map_err(|err| PlaceError::Chain { chain: ci, err })?;
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
                    // check this segment now so a failure names the placement that caused it
                    c.segment_volume_exact(p.segment, &plans[p.segment]).map_err(|err| PlaceError::Wall { placement: k, err })?;
                }
            }
            Ok(c)
        }).collect();
        let chains = results.into_iter().collect::<Result<Vec<_>, _>>()?;
        let volume = openbim_geom::total_volume_parallel(&chains).map_err(|err| PlaceError::Chain { chain: usize::MAX, err })?;
        Ok(Built { chains, instances, volume })
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
            Err(err) => { warnings.push(Warning { placement: None, error: PlaceError::Chain { chain: i, err } }); false }
        }).collect();
        // a placement is usable when its references are good and its wall chain is valid
        let mut usable = vec![true; self.placements.len()];
        for (k, p) in self.placements.iter().enumerate() {
            let why = if p.family >= self.families.len() { Some("no such family") }
                else if p.chain >= self.chains.len() { Some("no such wall chain") }
                else if !chain_ok[p.chain] { Some("its wall chain is not valid") }
                else if p.segment >= self.chains[p.chain].segment_count() { Some("no such wall segment") }
                else { None };
            if let Some(what) = why { usable[k] = false; warnings.push(Warning { placement: Some(k), error: PlaceError::BadReference { placement: k, what } }); }
            else if !p.centre.is_finite() { usable[k] = false; warnings.push(Warning { placement: Some(k), error: PlaceError::NotFinite { placement: k } }); }
        }
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
                    Err(err) => warnings.push(Warning { placement: Some(k), error: PlaceError::Family { placement: k, err } }),
                }
            }
        }
        // cut the openings, one chain per task; a placement that breaks its wall is taken out again
        let mut by_chain: Vec<Vec<usize>> = vec![Vec::new(); self.chains.len()];
        for (k, p) in self.placements.iter().enumerate() { if instances[k].is_some() { by_chain[p.chain].push(k); } }
        type Cut = (WallChain, Vec<usize>, Vec<Warning>);
        let cuts: Vec<Cut> = self.chains.par_iter().zip(by_chain.par_iter()).enumerate().map(|(ci, (c, ks))| {
            let mut c = c.clone();
            let mut dropped = Vec::new();
            let mut warns = Vec::new();
            if !chain_ok[ci] { return (c, dropped, warns); }
            let n = c.segment_count();
            if c.openings.len() < n { c.openings.resize(n, Vec::new()); }
            let plans = match c.plans() {
                Ok(p) => p,
                Err(err) => { warns.push(Warning { placement: None, error: PlaceError::Chain { chain: ci, err } }); return (c, ks.clone(), warns); }
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
                    warns.push(Warning { placement: Some(k), error: PlaceError::Wall { placement: k, err } });
                }
            }
            (c, dropped, warns)
        }).collect();
        let mut chains = Vec::with_capacity(cuts.len());
        for (c, dropped, warns) in cuts {
            for k in dropped { instances[k] = None; }
            warnings.extend(warns);
            chains.push(c);
        }
        let good: Vec<WallChain> = chains.iter().zip(&chain_ok).filter(|(_, ok)| **ok).map(|(c, _)| c.clone()).collect();
        let volume = openbim_geom::total_volume_parallel(&good).map_err(|err| PlaceError::Chain { chain: usize::MAX, err })?;
        warnings.sort_by_key(|w| (w.placement.is_some(), w.placement.unwrap_or(0))); // chain warnings first, then by placement
        Ok(LenientBuilt { chains, instances, volume, warnings })
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
