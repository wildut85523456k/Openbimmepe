//! Ceilings with a service void above them, and vertical shafts (risers) that run through floors.

use crate::{check_name, fin, ring_area, ArchError, Levels, Room};

type P = [f64; 2];

pub const MAX_SHAFTS: usize = 10_000;
pub const MIN_VOID: f64 = 0.0;

/// A ceiling under a floor above: its underside height and thickness, measured from the floor of its level.
#[derive(Debug, Clone, PartialEq)]
pub struct Ceiling {
    pub name: String,
    /// height of the underside above the floor, metres
    pub height: f64,
    /// thickness of the ceiling build-up (tiles and grid, board), metres
    pub thickness: f64,
}

impl Ceiling {
    pub fn check(&self) -> Result<(), ArchError> {
        check_name(&self.name)?;
        fin(self.height, "ceiling height")?; fin(self.thickness, "ceiling thickness")?;
        if self.height <= 0.0 || self.height > 30.0 { return Err(ArchError::BadValue { what: "ceiling height", name: self.name.clone() }); }
        if self.thickness < 0.0 || self.thickness > 2.0 { return Err(ArchError::BadValue { what: "ceiling thickness", name: self.name.clone() }); }
        Ok(())
    }

    /// Clear depth above the ceiling up to the underside of the structure above:
    /// level height - structure depth - ceiling height - ceiling thickness. Negative means it does not fit.
    pub fn void_depth(&self, level_height: f64, structure_depth: f64) -> Result<f64, ArchError> {
        self.check()?;
        fin(level_height, "level height")?; fin(structure_depth, "structure depth")?;
        if level_height <= 0.0 || structure_depth < 0.0 { return Err(ArchError::BadValue { what: "level height or structure depth", name: self.name.clone() }); }
        let v = level_height - structure_depth - self.height - self.thickness;
        if v < MIN_VOID { return Err(ArchError::BadValue { what: "void depth (ceiling does not fit under the structure)", name: self.name.clone() }); }
        Ok(v)
    }

    /// Does a pipe or duct of this overall depth (with the clearance the designer wants around it) fit in the void?
    pub fn fits(&self, level_height: f64, structure_depth: f64, item_depth: f64, clearance: f64) -> Result<bool, ArchError> {
        fin(item_depth, "item depth")?; fin(clearance, "clearance")?;
        if item_depth < 0.0 || clearance < 0.0 { return Err(ArchError::BadValue { what: "item depth or clearance", name: self.name.clone() }); }
        Ok(self.void_depth(level_height, structure_depth)? >= item_depth + 2.0 * clearance)
    }

    /// Ceiling for a room: its area is the room's floor area (the ceiling follows the room outline).
    pub fn area_of(room: &Room) -> f64 { room.area() }
}

/// A vertical shaft: plan outline, running from the floor of level `from` up to the floor of level `to`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shaft {
    pub name: String,
    pub usage: String,
    /// plan outline, any direction, simple polygon
    pub outline: Vec<P>,
    pub from: usize,
    pub to: usize,
}

impl Shaft {
    pub fn check(&self, levels: &Levels) -> Result<(), ArchError> {
        check_name(&self.name)?;
        if self.usage.chars().count() > crate::MAX_NAME || self.usage.chars().any(|c| c.is_control()) { return Err(ArchError::BadName(self.usage.clone())); }
        let n = levels.all().len();
        if self.from >= n { return Err(ArchError::UnknownLevel(self.from)); }
        if self.to >= n { return Err(ArchError::UnknownLevel(self.to)); }
        if self.to <= self.from { return Err(ArchError::BadValue { what: "shaft levels (top must be above bottom)", name: self.name.clone() }); }
        if self.outline.len() < 3 || self.outline.len() > 1_000 { return Err(ArchError::BadValue { what: "shaft outline points", name: self.name.clone() }); }
        for q in &self.outline { fin(q[0], "shaft point")?; fin(q[1], "shaft point")?; }
        let a = ring_area(&self.outline);
        if a.abs() < 1e-6 { return Err(ArchError::BadValue { what: "shaft area", name: self.name.clone() }); }
        let m = self.outline.len();
        for i in 0..m { for j in (i + 2)..m {
            if i == 0 && j == m - 1 { continue; }
            if segs_cross(self.outline[i], self.outline[(i + 1) % m], self.outline[j], self.outline[(j + 1) % m]) {
                return Err(ArchError::BadValue { what: "shaft outline (crosses itself)", name: self.name.clone() });
            }
        } }
        Ok(())
    }

    pub fn area(&self) -> f64 { ring_area(&self.outline).abs() }

    /// Levels whose floor slab the shaft passes through. The shaft runs from the floor of level `from` up to
    /// the floor of level `to`, so it cuts the floors of the levels strictly between them.
    pub fn floors_cut(&self) -> Vec<usize> { ((self.from + 1)..self.to).collect() }

    /// Height of the shaft from the floor of its bottom level to the floor of its top level.
    pub fn height(&self, levels: &Levels) -> Result<f64, ArchError> {
        self.check(levels)?;
        Ok(levels.all()[self.to].elevation - levels.all()[self.from].elevation)
    }

    /// Does a rectangle (for example a pipe bundle or duct) of the given plan size fit in the shaft when
    /// laid with its sides along x and y and its centre at `centre`?
    pub fn contains_rect(&self, centre: P, w: f64, d: f64) -> bool {
        let (hx, hy) = (w / 2.0, d / 2.0);
        let corners = [[centre[0] - hx, centre[1] - hy], [centre[0] + hx, centre[1] - hy], [centre[0] + hx, centre[1] + hy], [centre[0] - hx, centre[1] + hy]];
        corners.iter().all(|c| point_in(&self.outline, *c))
            && (0..4).all(|i| !(0..self.outline.len()).any(|j| segs_cross(corners[i], corners[(i + 1) % 4], self.outline[j], self.outline[(j + 1) % self.outline.len()])))
    }
}

/// Shafts may not overlap in plan where they share levels.
pub fn check_shafts(shafts: &[Shaft], levels: &Levels) -> Result<(), ArchError> {
    if shafts.len() > MAX_SHAFTS { return Err(ArchError::TooMany("shafts")); }
    for s in shafts { s.check(levels)?; }
    let mut names: Vec<&str> = shafts.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) { return Err(ArchError::DuplicateName(w[0].to_string())); }
    for i in 0..shafts.len() { for j in (i + 1)..shafts.len() {
        let (a, b) = (&shafts[i], &shafts[j]);
        if a.from >= b.to || b.from >= a.to { continue; } // no common height
        if rings_overlap(&a.outline, &b.outline) { return Err(ArchError::BadValue { what: "shaft overlaps another shaft", name: b.name.clone() }); }
    } }
    Ok(())
}

fn orient(a: P, b: P, c: P) -> f64 { (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) }
/// Proper crossing only (touching ends do not count).
fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    ((o1 > 0.0) != (o2 > 0.0)) && ((o3 > 0.0) != (o4 > 0.0)) && o1 != 0.0 && o2 != 0.0 && o3 != 0.0 && o4 != 0.0
}
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
fn rings_overlap(a: &[P], b: &[P]) -> bool {
    let (n, m) = (a.len(), b.len());
    (0..n).any(|i| (0..m).any(|j| segs_cross(a[i], a[(i + 1) % n], b[j], b[(j + 1) % m])))
        || a.iter().any(|q| point_in(b, *q)) || b.iter().any(|q| point_in(a, *q))
}
