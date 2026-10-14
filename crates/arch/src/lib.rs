//! Levels, grids and rooms. A room is the clear floor area inside a closed chain of walls (faces, not
//! centrelines), less the footprints of closed chains standing inside it (cores, shafts, columns).
//! Everything is a pure function of its inputs; there is no hidden state.

pub mod envelope;
pub mod glazing;
pub mod layers;
pub mod spaces;

use openbim_geom::{GeomError, WallChain};

type P = [f64; 2];

pub const MAX_LEVELS: usize = 1_000;
pub const MAX_GRIDS: usize = 10_000;
pub const MAX_NAME: usize = 128;
/// Levels closer than this (metres) are refused.
pub const MIN_LEVEL_GAP: f64 = 0.1;
/// Largest occupancy accepted for one room.
pub const MAX_OCCUPANCY: f64 = 100_000.0;

#[derive(Debug, Clone, PartialEq)]
pub enum ArchError {
    TooMany(&'static str),
    BadName(String),
    DuplicateName(String),
    NotFinite(&'static str),
    LevelsTooClose { a: String, b: String },
    UnknownLevel(usize),
    ZeroLengthGrid(String),
    NeedClosedChain,
    Chain(GeomError),
    BadHeight,
    BadOccupancy,
    /// the hole (by index) is not entirely inside the room, or touches its boundary or another hole
    BadHole(usize),
    NoArea,
    BadValue { what: &'static str, name: String },
    UnknownMaterial(String),
    Unsupported(&'static str),
}

impl std::fmt::Display for ArchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ArchError::*;
        match self {
            TooMany(w) => write!(f, "too many {w}"),
            BadName(n) => write!(f, "name '{n}' is empty, too long or has control characters"),
            DuplicateName(n) => write!(f, "name '{n}' is used twice"),
            NotFinite(w) => write!(f, "{w} is not a finite number"),
            LevelsTooClose { a, b } => write!(f, "levels '{a}' and '{b}' are less than {MIN_LEVEL_GAP} m apart"),
            UnknownLevel(i) => write!(f, "no level number {i}"),
            ZeroLengthGrid(n) => write!(f, "grid '{n}' has no length"),
            NeedClosedChain => write!(f, "a room needs a closed chain of walls"),
            Chain(e) => write!(f, "walls: {e:?}"),
            BadHeight => write!(f, "room height must be above zero and not more than the wall height"),
            BadOccupancy => write!(f, "occupancy must be from 0 to {MAX_OCCUPANCY}"),
            BadHole(i) => write!(f, "inner chain {i} is not completely inside the room"),
            NoArea => write!(f, "the walls enclose no floor area"),
            BadValue { what, name } => write!(f, "{what} of '{name}' is not acceptable"),
            Unsupported(w) => write!(f, "not supported yet: {w}"),
            UnknownMaterial(n) => write!(f, "material '{n}' is not in the library"),
        }
    }
}
impl std::error::Error for ArchError {}

pub(crate) fn check_name(n: &str) -> Result<(), ArchError> {
    if n.is_empty() || n.chars().count() > MAX_NAME || n.chars().any(|c| c.is_control()) { Err(ArchError::BadName(n.to_string())) } else { Ok(()) }
}
pub(crate) fn fin(v: f64, what: &'static str) -> Result<f64, ArchError> { if v.is_finite() { Ok(v) } else { Err(ArchError::NotFinite(what)) } }

// ------------------------------------------------------------------ levels
#[derive(Debug, Clone, PartialEq)]
pub struct Level { pub name: String, pub elevation: f64 }

/// Levels sorted by elevation, with unique names.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Levels { list: Vec<Level> }

impl Levels {
    pub fn new(mut list: Vec<Level>) -> Result<Levels, ArchError> {
        if list.len() > MAX_LEVELS { return Err(ArchError::TooMany("levels")); }
        for l in &list { check_name(&l.name)?; fin(l.elevation, "level elevation")?; }
        list.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        for w in list.windows(2) {
            if w[1].elevation - w[0].elevation < MIN_LEVEL_GAP { return Err(ArchError::LevelsTooClose { a: w[0].name.clone(), b: w[1].name.clone() }); }
        }
        let mut names: Vec<&str> = list.iter().map(|l| l.name.as_str()).collect();
        names.sort_unstable();
        if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) { return Err(ArchError::DuplicateName(w[0].to_string())); }
        Ok(Levels { list })
    }
    pub fn all(&self) -> &[Level] { &self.list }
    pub fn index_of(&self, name: &str) -> Option<usize> { self.list.iter().position(|l| l.name == name) }
    /// Distance from level `i` up to the next level; None for the top level.
    pub fn floor_to_floor(&self, i: usize) -> Result<Option<f64>, ArchError> {
        let l = self.list.get(i).ok_or(ArchError::UnknownLevel(i))?;
        Ok(self.list.get(i + 1).map(|n| n.elevation - l.elevation))
    }
}

// ------------------------------------------------------------------ grids
#[derive(Debug, Clone, PartialEq)]
pub struct Grid { pub name: String, pub a: P, pub b: P }

pub fn check_grids(grids: &[Grid]) -> Result<(), ArchError> {
    if grids.len() > MAX_GRIDS { return Err(ArchError::TooMany("grids")); }
    for g in grids {
        check_name(&g.name)?;
        for v in [g.a[0], g.a[1], g.b[0], g.b[1]] { fin(v, "grid point")?; }
        if (g.a[0] - g.b[0]).hypot(g.a[1] - g.b[1]) < 1e-6 { return Err(ArchError::ZeroLengthGrid(g.name.clone())); }
    }
    let mut names: Vec<&str> = grids.iter().map(|g| g.name.as_str()).collect();
    names.sort_unstable();
    if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) { return Err(ArchError::DuplicateName(w[0].to_string())); }
    Ok(())
}

/// Where two grid lines cross (as infinite lines); None if parallel.
pub fn grid_crossing(g: &Grid, h: &Grid) -> Option<P> {
    let d1 = [g.b[0] - g.a[0], g.b[1] - g.a[1]];
    let d2 = [h.b[0] - h.a[0], h.b[1] - h.a[1]];
    let den = d1[0] * d2[1] - d1[1] * d2[0];
    if den.abs() < 1e-12 * (d1[0].hypot(d1[1]) * d2[0].hypot(d2[1])) { return None; }
    let t = ((h.a[0] - g.a[0]) * d2[1] - (h.a[1] - g.a[1]) * d2[0]) / den;
    Some([g.a[0] + t * d1[0], g.a[1] + t * d1[1]])
}

// ------------------------------------------------------------------ rooms
#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    pub name: String,
    /// room use as the project names it (a type list comes from the library later)
    pub usage: String,
    /// number of people
    pub occupancy: f64,
    /// level index in the `Levels` it was made for
    pub level: usize,
    /// clear floor outline, counter-clockwise
    pub outline: Vec<P>,
    /// inner outlines (clockwise): cores and shafts standing in the room
    pub holes: Vec<Vec<P>>,
    pub height: f64,
}

pub fn ring_area(p: &[P]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() / 2.0
}
fn ring_len(p: &[P]) -> f64 { let n = p.len(); (0..n).map(|i| (p[(i + 1) % n][0] - p[i][0]).hypot(p[(i + 1) % n][1] - p[i][1])).sum() }

fn point_in(p: &[P], q: P) -> bool {
    let n = p.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        if (p[i][1] > q[1]) != (p[j][1] > q[1]) && q[0] < (p[j][0] - p[i][0]) * (q[1] - p[i][1]) / (p[j][1] - p[i][1]) + p[i][0] { inside = !inside; }
        j = i;
    }
    inside
}
fn orient(a: P, b: P, c: P) -> f64 { (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) }
/// Do two segments share a point (crossing, touching, or overlapping along a line)?
fn segs_meet(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1 == 0.0 && o2 == 0.0 && o3 == 0.0 && o4 == 0.0 {
        // all on one line: they meet only if their ranges along the line overlap
        let k = if (a[0] - b[0]).abs() >= (a[1] - b[1]).abs() { 0 } else { 1 };
        let (lo1, hi1) = (a[k].min(b[k]), a[k].max(b[k]));
        let (lo2, hi2) = (c[k].min(d[k]), c[k].max(d[k]));
        return lo1 <= hi2 && lo2 <= hi1;
    }
    ((o1 > 0.0) != (o2 > 0.0) || o1 == 0.0 || o2 == 0.0) && ((o3 > 0.0) != (o4 > 0.0) || o3 == 0.0 || o4 == 0.0)
}
fn rings_meet(p: &[P], q: &[P]) -> bool {
    let (n, m) = (p.len(), q.len());
    (0..n).any(|i| (0..m).any(|j| segs_meet(p[i], p[(i + 1) % n], q[j], q[(j + 1) % m])))
}

/// The face polygon of a closed chain on the inside (`inner = true`) or the outside.
/// Inside is the side the chain's own area lies on, whatever direction it was drawn in.
pub fn chain_face(c: &WallChain, inner: bool) -> Result<Vec<P>, ArchError> {
    if !c.closed { return Err(ArchError::NeedClosedChain); }
    let plans = c.plans().map_err(ArchError::Chain)?;
    let centre_area = ring_area(&c.points);
    // counter-clockwise: the left faces look inward
    let left = (centre_area > 0.0) == inner;
    let mut v: Vec<P> = plans.iter().map(|p| if left { p.l0 } else { p.r0 }).collect();
    if ring_area(&v) < 0.0 { v.reverse(); }
    Ok(v)
}

impl Room {
    /// Room inside the closed chain `walls`; `inner` chains (closed) stand inside it and are cut out by their outer faces.
    pub fn from_chain(name: &str, usage: &str, occupancy: f64, level: usize, walls: &WallChain, inner: &[&WallChain], height: Option<f64>) -> Result<Room, ArchError> {
        check_name(name)?;
        if usage.chars().count() > MAX_NAME || usage.chars().any(|c| c.is_control()) { return Err(ArchError::BadName(usage.to_string())); }
        fin(occupancy, "occupancy")?;
        if !(0.0..=MAX_OCCUPANCY).contains(&occupancy) { return Err(ArchError::BadOccupancy); }
        let height = match height { Some(h) => fin(h, "room height")?, None => walls.height };
        if height <= 0.0 || height > walls.height { return Err(ArchError::BadHeight); }
        let outline = chain_face(walls, true)?;
        if ring_area(&outline) <= 1e-9 { return Err(ArchError::NoArea); }
        let mut holes: Vec<Vec<P>> = Vec::new();
        for (i, h) in inner.iter().enumerate() {
            let mut ring = chain_face(h, false)?;
            ring.reverse(); // holes run clockwise
            if !ring.iter().all(|q| point_in(&outline, *q)) || rings_meet(&outline, &ring) { return Err(ArchError::BadHole(i)); }
            for prev in &holes {
                if rings_meet(prev, &ring) || point_in(prev, ring[0]) || point_in(&ring, prev[0]) { return Err(ArchError::BadHole(i)); }
            }
            holes.push(ring);
        }
        Ok(Room { name: name.to_string(), usage: usage.to_string(), occupancy, level, outline, holes, height })
    }

    /// Clear floor area.
    pub fn area(&self) -> f64 { ring_area(&self.outline) + self.holes.iter().map(|h| ring_area(h)).sum::<f64>() }
    /// Length of the room's walls at floor level, including inner chains.
    pub fn perimeter(&self) -> f64 { ring_len(&self.outline) + self.holes.iter().map(|h| ring_len(h)).sum::<f64>() }
    pub fn volume(&self) -> f64 { self.area() * self.height }
    /// Floor area per person; None for an empty room.
    pub fn area_per_person(&self) -> Option<f64> { if self.occupancy > 0.0 { Some(self.area() / self.occupancy) } else { None } }
    pub fn contains(&self, q: P) -> bool { point_in(&self.outline, q) && !self.holes.iter().any(|h| point_in(h, q)) }
}
