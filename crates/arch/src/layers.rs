//! Layered assemblies (wall, floor and roof types) and their thermal transmittance.
//! Every number (conductivity, surface resistances, air-gap resistance) is entered by the user or taken
//! from a project library; none is built into the program.

use crate::{check_name, fin, ArchError};

pub const MAX_LAYERS: usize = 64;
pub const MAX_MATERIALS: usize = 10_000;
/// Total thickness of one assembly, metres.
pub const MAX_THICKNESS: f64 = 5.0;
pub const MAX_CONDUCTIVITY: f64 = 1_000.0;
pub const MAX_RESISTANCE: f64 = 100.0;
pub const MAX_DENSITY: f64 = 30_000.0;
pub const MAX_SPECIFIC_HEAT: f64 = 100_000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub name: String,
    /// thermal conductivity, W/(m K)
    pub conductivity: f64,
    /// density, kg/m3, if known (needed later for thermal mass)
    pub density: Option<f64>,
    /// specific heat capacity, J/(kg K), if known
    pub specific_heat: Option<f64>,
    /// free text group such as "Masonry" or "Insulation" for the picker
    pub category: String,
}

impl Material {
    /// A material with only a conductivity.
    pub fn new(name: &str, conductivity: f64) -> Material { Material { name: name.into(), conductivity, density: None, specific_heat: None, category: String::new() } }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Layer {
    /// a solid layer of a library material
    Solid { material: String, thickness: f64 },
    /// a layer given by its thermal resistance, m2 K/W (air gap, cavity, membrane)
    Resistance { name: String, thickness: f64, r: f64 },
}

impl Layer {
    pub fn thickness(&self) -> f64 { match self { Layer::Solid { thickness, .. } | Layer::Resistance { thickness, .. } => *thickness } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyKind { Wall, Floor, Roof }

/// Layers run from the outside (or top) to the inside (or bottom).
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    pub name: String,
    pub kind: AssemblyKind,
    pub layers: Vec<Layer>,
    /// outside surface resistance, m2 K/W
    pub r_se: f64,
    /// inside surface resistance, m2 K/W
    pub r_si: f64,
}

/// Materials by name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Library { materials: Vec<Material> }

impl Library {
    pub fn new(materials: Vec<Material>) -> Result<Library, ArchError> {
        if materials.len() > MAX_MATERIALS { return Err(ArchError::TooMany("materials")); }
        for m in &materials {
            check_name(&m.name)?;
            fin(m.conductivity, "conductivity")?;
            if m.conductivity <= 0.0 || m.conductivity > MAX_CONDUCTIVITY { return Err(ArchError::BadValue { what: "conductivity", name: m.name.clone() }); }
            if let Some(d) = m.density { fin(d, "density")?; if d <= 0.0 || d > MAX_DENSITY { return Err(ArchError::BadValue { what: "density", name: m.name.clone() }); } }
            if let Some(c) = m.specific_heat { fin(c, "specific heat")?; if c <= 0.0 || c > MAX_SPECIFIC_HEAT { return Err(ArchError::BadValue { what: "specific heat", name: m.name.clone() }); } }
            if m.category.chars().count() > crate::MAX_NAME || m.category.chars().any(|c| c.is_control()) { return Err(ArchError::BadName(m.category.clone())); }
        }
        let mut names: Vec<&str> = materials.iter().map(|m| m.name.as_str()).collect();
        names.sort_unstable();
        if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) { return Err(ArchError::DuplicateName(w[0].to_string())); }
        Ok(Library { materials })
    }
    pub fn get(&self, name: &str) -> Option<&Material> { self.materials.iter().find(|m| m.name == name) }
}

impl Assembly {
    pub fn check(&self, lib: &Library) -> Result<(), ArchError> {
        check_name(&self.name)?;
        if self.layers.is_empty() { return Err(ArchError::BadValue { what: "layer count", name: self.name.clone() }); }
        if self.layers.len() > MAX_LAYERS { return Err(ArchError::TooMany("layers")); }
        for r in [self.r_se, self.r_si] {
            fin(r, "surface resistance")?;
            if !(0.0..=MAX_RESISTANCE).contains(&r) { return Err(ArchError::BadValue { what: "surface resistance", name: self.name.clone() }); }
        }
        let mut total = 0.0;
        for l in &self.layers {
            let t = l.thickness();
            fin(t, "layer thickness")?;
            if t <= 0.0 { return Err(ArchError::BadValue { what: "layer thickness", name: self.name.clone() }); }
            total += t;
            match l {
                Layer::Solid { material, .. } => { if lib.get(material).is_none() { return Err(ArchError::UnknownMaterial(material.clone())); } }
                Layer::Resistance { name, r, .. } => {
                    check_name(name)?;
                    fin(*r, "layer resistance")?;
                    if *r <= 0.0 || *r > MAX_RESISTANCE { return Err(ArchError::BadValue { what: "layer resistance", name: name.clone() }); }
                }
            }
        }
        if total > MAX_THICKNESS { return Err(ArchError::BadValue { what: "total thickness", name: self.name.clone() }); }
        Ok(())
    }

    /// Total thickness; this is the thickness a wall chain of this type gets.
    pub fn thickness(&self) -> f64 { self.layers.iter().map(|l| l.thickness()).sum() }

    /// Thermal resistance of layer `i`, m2 K/W.
    pub fn layer_r(&self, i: usize, lib: &Library) -> Result<f64, ArchError> {
        match self.layers.get(i) {
            Some(Layer::Solid { material, thickness }) => Ok(thickness / lib.get(material).ok_or_else(|| ArchError::UnknownMaterial(material.clone()))?.conductivity),
            Some(Layer::Resistance { r, .. }) => Ok(*r),
            None => Err(ArchError::BadValue { what: "layer number", name: self.name.clone() }),
        }
    }

    /// Total resistance including both surfaces, m2 K/W.
    pub fn r_total(&self, lib: &Library) -> Result<f64, ArchError> {
        self.check(lib)?;
        let mut r = self.r_se + self.r_si;
        for i in 0..self.layers.len() { r += self.layer_r(i, lib)?; }
        if r <= 0.0 { return Err(ArchError::BadValue { what: "total resistance", name: self.name.clone() }); }
        Ok(r)
    }

    /// Thermal transmittance U = 1 / R_total, W/(m2 K).
    pub fn u_value(&self, lib: &Library) -> Result<f64, ArchError> { Ok(1.0 / self.r_total(lib)?) }
}

/// One layer of an assembly with its figures, for the wall-type editor.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerResult {
    pub label: String,
    pub thickness: f64,
    pub r: f64,
    /// share of the total resistance, 0..1
    pub share: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Breakdown {
    pub layers: Vec<LayerResult>,
    pub r_se: f64,
    pub r_si: f64,
    pub r_total: f64,
    pub u: f64,
    pub thickness: f64,
}

impl Assembly {
    /// Everything the wall-type editor shows after each change of material or thickness.
    pub fn breakdown(&self, lib: &Library) -> Result<Breakdown, ArchError> {
        let r_total = self.r_total(lib)?;
        let mut layers = Vec::with_capacity(self.layers.len());
        for (i, l) in self.layers.iter().enumerate() {
            let r = self.layer_r(i, lib)?;
            let label = match l { Layer::Solid { material, .. } => material.clone(), Layer::Resistance { name, .. } => name.clone() };
            layers.push(LayerResult { label, thickness: l.thickness(), r, share: r / r_total });
        }
        Ok(Breakdown { layers, r_se: self.r_se, r_si: self.r_si, r_total, u: 1.0 / r_total, thickness: self.thickness() })
    }

    /// Thickness that solid layer `i` needs for the assembly to reach `target_u`, all other layers unchanged.
    /// Refused if the other layers already give a lower U, if layer `i` is not a solid layer, or if the result is
    /// not a usable thickness (above 5 m, or the total above 5 m).
    pub fn thickness_for_u(&self, i: usize, target_u: f64, lib: &Library) -> Result<f64, ArchError> {
        fin(target_u, "target U")?;
        if target_u <= 0.0 { return Err(ArchError::BadValue { what: "target U", name: self.name.clone() }); }
        let material = match self.layers.get(i) {
            Some(Layer::Solid { material, .. }) => material,
            _ => return Err(ArchError::BadValue { what: "layer number", name: self.name.clone() }),
        };
        let lambda = lib.get(material).ok_or_else(|| ArchError::UnknownMaterial(material.clone()))?.conductivity;
        let mut others = self.r_se + self.r_si;
        for j in 0..self.layers.len() { if j != i { others += self.layer_r(j, lib)?; } }
        let need = 1.0 / target_u - others;
        if need <= 0.0 { return Err(ArchError::BadValue { what: "target U (already met without this layer)", name: self.name.clone() }); }
        let t = need * lambda;
        let rest: f64 = self.thickness() - self.layers[i].thickness();
        if t > MAX_THICKNESS || rest + t > MAX_THICKNESS { return Err(ArchError::BadValue { what: "thickness needed", name: self.name.clone() }); }
        Ok(t)
    }

    /// Give a wall chain the thickness of this assembly.
    pub fn apply_to(&self, chain: &mut openbim_geom::WallChain, lib: &Library) -> Result<(), ArchError> {
        self.check(lib)?;
        chain.thickness = self.thickness();
        Ok(())
    }
}

// ------------------------------------------------------------------ the material list as a text file
// One line per material, tab separated: name, conductivity, density, specific heat, category.
// Empty density or specific heat means "not given". Lines starting with # are comments.

impl Library {
    pub fn all(&self) -> &[Material] { &self.materials }

    pub fn to_text(&self) -> String {
        let mut s = String::from("# name\tconductivity W/(m K)\tdensity kg/m3\tspecific heat J/(kg K)\tcategory\n");
        for m in &self.materials {
            let o = |v: Option<f64>| v.map(|x| format!("{x}")).unwrap_or_default();
            s.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", m.name, m.conductivity, o(m.density), o(m.specific_heat), m.category));
        }
        s
    }

    /// Read a list written by `to_text` or typed by hand. The error names the line.
    pub fn from_text(text: &str) -> Result<Library, (usize, ArchError)> {
        let mut v = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') { continue; }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 2 || f.len() > 5 { return Err((n + 1, ArchError::BadValue { what: "number of columns", name: line.chars().take(40).collect() })); }
            let num = |s: &str, what: &'static str| -> Result<f64, (usize, ArchError)> { s.trim().parse::<f64>().map_err(|_| (n + 1, ArchError::BadValue { what, name: f[0].to_string() })) };
            let opt = |i: usize, what: &'static str| -> Result<Option<f64>, (usize, ArchError)> {
                match f.get(i).map(|s| s.trim()) { None | Some("") => Ok(None), Some(s) => num(s, what).map(Some) }
            };
            v.push(Material {
                name: f[0].to_string(),
                conductivity: num(f[1], "conductivity")?,
                density: opt(2, "density")?,
                specific_heat: opt(3, "specific heat")?,
                category: f.get(4).map(|s| s.trim().to_string()).unwrap_or_default(),
            });
            if v.len() > MAX_MATERIALS { return Err((n + 1, ArchError::TooMany("materials"))); }
        }
        Library::new(v).map_err(|e| (0, e))
    }
}
