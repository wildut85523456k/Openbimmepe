//! Windows: glazing assembly (panes and gaps), frame and the whole-window U-value.
//! Whole window: Uw = (Ag Ug + Af Uf + lg psi) / (Ag + Af) where Ag is the glass area seen through the frame,
//! Af the frame area (frame and bars), lg the length of the glass edges and psi the extra heat flow along them.
//! Frame U-values and psi are entered from the maker's data or a calculation; no values are built in.

use crate::layers::Library;
use crate::{check_name, fin, ArchError};

pub const MAX_PANES: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct Pane { pub material: String, pub thickness: f64 }

/// A gap between panes; `r` is its thermal resistance (m2 K/W) from the maker's data or a calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct Gap { pub thickness: f64, pub r: f64 }

#[derive(Debug, Clone, PartialEq)]
pub struct Glazing {
    pub name: String,
    pub panes: Vec<Pane>,
    /// exactly one fewer than the panes
    pub gaps: Vec<Gap>,
    pub r_se: f64,
    pub r_si: f64,
    /// if the maker declares a centre-of-glass U-value it is used instead of the one built from the layers
    pub declared_ug: Option<f64>,
    /// total solar energy transmittance, 0..1, if known
    pub g_value: Option<f64>,
    /// visible light transmittance, 0..1, if known
    pub light_transmittance: Option<f64>,
}

impl Glazing {
    pub fn check(&self, lib: &Library) -> Result<(), ArchError> {
        check_name(&self.name)?;
        let bad = |what: &'static str| ArchError::BadValue { what, name: self.name.clone() };
        if self.panes.is_empty() || self.panes.len() > MAX_PANES { return Err(bad("number of panes")); }
        if self.gaps.len() + 1 != self.panes.len() { return Err(bad("number of gaps (one fewer than the panes)")); }
        for p in &self.panes {
            fin(p.thickness, "pane thickness")?;
            if !(0.001..=0.05).contains(&p.thickness) { return Err(bad("pane thickness")); }
            if lib.get(&p.material).is_none() { return Err(ArchError::UnknownMaterial(p.material.clone())); }
        }
        for g in &self.gaps {
            fin(g.thickness, "gap thickness")?; fin(g.r, "gap resistance")?;
            if !(0.001..=0.1).contains(&g.thickness) { return Err(bad("gap thickness")); }
            if g.r <= 0.0 || g.r > 10.0 { return Err(bad("gap resistance")); }
        }
        for r in [self.r_se, self.r_si] {
            fin(r, "surface resistance")?;
            if !(0.0..=10.0).contains(&r) { return Err(bad("surface resistance")); }
        }
        if let Some(u) = self.declared_ug { fin(u, "declared Ug")?; if u <= 0.0 || u > 20.0 { return Err(bad("declared Ug")); } }
        for (v, w) in [(self.g_value, "g value"), (self.light_transmittance, "light transmittance")] {
            if let Some(x) = v { fin(x, w)?; if !(0.0..=1.0).contains(&x) { return Err(bad(w)); } }
        }
        Ok(())
    }

    /// Total thickness of panes and gaps.
    pub fn thickness(&self) -> f64 { self.panes.iter().map(|p| p.thickness).sum::<f64>() + self.gaps.iter().map(|g| g.thickness).sum::<f64>() }

    /// Centre-of-glass U-value, W/(m2 K): the declared value, or 1 / (surface resistances + panes + gaps).
    pub fn ug(&self, lib: &Library) -> Result<f64, ArchError> {
        self.check(lib)?;
        if let Some(u) = self.declared_ug { return Ok(u); }
        let mut r = self.r_se + self.r_si;
        for p in &self.panes { r += p.thickness / lib.get(&p.material).ok_or_else(|| ArchError::UnknownMaterial(p.material.clone()))?.conductivity; }
        for g in &self.gaps { r += g.r; }
        if r <= 0.0 { return Err(ArchError::BadValue { what: "total resistance", name: self.name.clone() }); }
        Ok(1.0 / r)
    }
}

/// Frame data from the maker.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub name: String,
    /// frame U-value, W/(m2 K)
    pub uf: f64,
    /// linear thermal transmittance at the glass edge (spacer), W/(m K)
    pub psi: f64,
}

impl Frame {
    pub fn check(&self) -> Result<(), ArchError> {
        check_name(&self.name)?;
        fin(self.uf, "Uf")?; fin(self.psi, "psi")?;
        if self.uf <= 0.0 || self.uf > 20.0 { return Err(ArchError::BadValue { what: "Uf", name: self.name.clone() }); }
        if !(0.0..=1.0).contains(&self.psi) { return Err(ArchError::BadValue { what: "psi", name: self.name.clone() }); }
        Ok(())
    }
}

/// Window outline: overall size, frame width all round, and vertical bars between equal panes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowGeometry { pub width: f64, pub height: f64, pub frame_width: f64, pub bars: u32, pub bar_width: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowResult {
    /// overall area
    pub area: f64,
    /// glass area seen
    pub ag: f64,
    /// frame and bar area
    pub af: f64,
    /// length of glass edges
    pub lg: f64,
    pub uw: f64,
    /// frame share of the area, 0..1
    pub frame_fraction: f64,
}

pub fn window_u(g: WindowGeometry, ug: f64, frame: &Frame) -> Result<WindowResult, ArchError> {
    frame.check()?;
    for v in [g.width, g.height, g.frame_width, g.bar_width, ug] { fin(v, "window value")?; }
    let bad = |what: &'static str| ArchError::BadValue { what, name: frame.name.clone() };
    if ug <= 0.0 || ug > 20.0 { return Err(bad("Ug")); }
    if g.width <= 0.0 || g.height <= 0.0 || g.frame_width <= 0.0 || g.bar_width < 0.0 || g.bars > 100 { return Err(bad("window size")); }
    let n = g.bars as f64;
    let pane_w = (g.width - 2.0 * g.frame_width - n * g.bar_width) / (n + 1.0);
    let pane_h = g.height - 2.0 * g.frame_width;
    if pane_w < 0.01 || pane_h < 0.01 { return Err(bad("window size (no glass left)")); }
    let area = g.width * g.height;
    let ag = (n + 1.0) * pane_w * pane_h;
    let lg = (n + 1.0) * 2.0 * (pane_w + pane_h);
    let af = area - ag;
    let uw = (ag * ug + af * frame.uf + lg * frame.psi) / area;
    Ok(WindowResult { area, ag, af, lg, uw, frame_fraction: af / area })
}
