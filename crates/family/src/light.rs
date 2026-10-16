//! Lighting data of a luminaire family: what a lighting program such as DIALux needs to know about the fitting,
//! beyond its place in the model. The position, the turn and the mounting height come from where the luminaire is
//! placed; the data below comes from the family author (or the manufacturer's data sheet). No standard values are
//! shipped: every number is typed in, and every field is optional until a calculation needs it.
use crate::{clean_text, FamilyError};

/// How the luminaire is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mounting {
    #[default]
    Recessed,
    Surface,
    Suspended,
    Wall,
    Free,
}

impl Mounting {
    pub fn name(self) -> &'static str {
        match self {
            Mounting::Recessed => "recessed",
            Mounting::Surface => "surface",
            Mounting::Suspended => "suspended",
            Mounting::Wall => "wall",
            Mounting::Free => "free",
        }
    }
    pub fn from_name(s: &str) -> Option<Mounting> {
        [Mounting::Recessed, Mounting::Surface, Mounting::Suspended, Mounting::Wall, Mounting::Free].into_iter().find(|m| m.name() == s)
    }
    /// the value of `Pset_LightFixtureTypeCommon.LightFixtureMountingType`
    pub fn ifc(self) -> &'static str {
        match self {
            Mounting::Recessed => "RECESSED",
            Mounting::Surface => "SURFACE",
            Mounting::Suspended => "SUSPENDED",
            Mounting::Wall => "OTHER",
            Mounting::Free => "FREESTANDING",
        }
    }
}

/// Lighting data of a family whose category is `LightFixture`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LightingDef {
    /// the photometric file the program reads, by name: an IES (`.ies`) or Eulumdat (`.ldt`) file kept with the project
    pub photometry: Option<String>,
    /// manufacturer and article number, so that a lighting program can find the luminaire in a catalogue
    pub manufacturer: Option<String>,
    pub article: Option<String>,
    /// luminous flux of the luminaire in lumens
    pub flux_lm: Option<f64>,
    /// electrical power in watts, driver included
    pub power_w: Option<f64>,
    /// correlated colour temperature in kelvin
    pub cct_k: Option<f64>,
    /// colour rendering index, 0 to 100
    pub cri: Option<f64>,
    /// ingress protection code as printed, for example "IP20"
    pub ip: Option<String>,
    /// share of the flux used, 0 to 1 (1 = full output)
    pub dimming: Option<f64>,
    pub mounting: Mounting,
}

pub const MAX_LIGHT_FLUX: f64 = 1.0e7;
pub const MAX_LIGHT_POWER: f64 = 1.0e6;

impl LightingDef {
    pub(crate) fn check(&self) -> Result<(), FamilyError> {
        let bad = |why: &'static str| Err(FamilyError::BadMeta(why));
        for (what, v) in [("photometry file", &self.photometry), ("manufacturer", &self.manufacturer), ("article number", &self.article), ("IP code", &self.ip)] {
            if let Some(s) = v {
                if s.trim().is_empty() {
                    return bad("a lighting text is empty");
                }
                clean_text(what, s, 128)?;
            }
        }
        if let Some(p) = &self.photometry {
            let l = p.to_ascii_lowercase();
            if !(l.ends_with(".ies") || l.ends_with(".ldt")) || p.contains(['/', '\\']) {
                return bad("the photometry must be the name of an .ies or .ldt file, without a folder");
            }
        }
        let pos = |v: Option<f64>, max: f64| v.is_none_or(|x| x.is_finite() && x > 0.0 && x <= max);
        if !pos(self.flux_lm, MAX_LIGHT_FLUX) {
            return bad("the luminous flux must be above 0 and at most 10 000 000 lm");
        }
        if !pos(self.power_w, MAX_LIGHT_POWER) {
            return bad("the power must be above 0 and at most 1 000 000 W");
        }
        if !pos(self.cct_k, 100_000.0) {
            return bad("the colour temperature must be above 0 and at most 100 000 K");
        }
        if !self.cri.is_none_or(|x| x.is_finite() && (0.0..=100.0).contains(&x)) {
            return bad("the colour rendering index is between 0 and 100");
        }
        if !self.dimming.is_none_or(|x| x.is_finite() && x > 0.0 && x <= 1.0) {
            return bad("the dimming level is above 0 and at most 1");
        }
        Ok(())
    }

    /// What a lighting calculation still lacks: the names of the fields that are empty and needed.
    /// A photometric file (or a manufacturer and article) says how the light is spread; the flux and power scale it.
    pub fn missing_for_calculation(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if self.photometry.is_none() && !(self.manufacturer.is_some() && self.article.is_some()) {
            m.push("photometry file or manufacturer and article");
        }
        if self.flux_lm.is_none() {
            m.push("luminous flux");
        }
        if self.power_w.is_none() {
            m.push("power");
        }
        m
    }
}
