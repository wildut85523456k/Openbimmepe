//! The fittings catalogue: which fitting exists for which system, shape and size.
//!
//! Each `FittingType` covers a class (elbow, tee, tap, transition), optionally one system, a section shape and a
//! range of sizes (the larger section dimension, metres), plus the dimensions that define it. When the router needs
//! a fitting it takes the first type in the list that fits, as in Revit's routing preferences, so the order is the
//! author's order of preference. Names and part codes are the author's; nothing is shipped.
use crate::AngleSet;
use openbim_family::mep::{Domain, System};
use openbim_place::penetrations::RunSize;

pub const MAX_TYPES: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FittingClass {
    Elbow,
    Tee,
    Tap,
    Transition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeFilter {
    Round,
    Rect,
    Any,
}

impl ShapeFilter {
    fn fits(self, s: RunSize) -> bool {
        matches!((self, s), (ShapeFilter::Any, _) | (ShapeFilter::Round, RunSize::Round { .. }) | (ShapeFilter::Rect, RunSize::Rect { .. }))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FittingType {
    pub name: String,
    /// manufacturer part code or similar; free text, may be empty
    pub part_code: String,
    pub class: FittingClass,
    pub domain: Domain,
    /// None = any system of the domain
    pub system: Option<System>,
    pub shape: ShapeFilter,
    /// larger section dimension of the run (for a tee the main run), inclusive, metres
    pub size_min: f64,
    pub size_max: f64,
    /// tees and taps only: the branch section's larger dimension, inclusive
    pub branch_min: f64,
    pub branch_max: f64,
    /// elbow: bend angles; tee and tap: branch angles (an angle and its supplement both count); transition: unused
    pub angles: AngleSet,
    /// elbow: bend radius = this x the larger section dimension
    pub radius_factor: f64,
    /// tee: half length along the run = this x the larger dimension of the run
    pub half_factor: f64,
    /// tee and tap: branch length = this x the larger dimension of the run
    pub stub_factor: f64,
    /// transition: largest included angle, degrees
    pub transition_angle_deg: f64,
    /// transition: shortest length, metres
    pub min_len: f64,
}

impl FittingType {
    /// A blank type of a class for a domain, covering every size and shape, with neutral dimensions. Use the
    /// builder methods to set what the author wants.
    pub fn new(name: &str, class: FittingClass, domain: Domain) -> FittingType {
        FittingType {
            name: name.into(),
            part_code: String::new(),
            class,
            domain,
            system: None,
            shape: ShapeFilter::Any,
            size_min: 0.0,
            size_max: 1000.0,
            branch_min: 0.0,
            branch_max: 1000.0,
            angles: AngleSet::Any,
            radius_factor: 1.0,
            half_factor: 1.0,
            stub_factor: 1.0,
            transition_angle_deg: 30.0,
            min_len: 0.0,
        }
    }
    pub fn code(mut self, c: &str) -> Self {
        self.part_code = c.into();
        self
    }
    pub fn system(mut self, s: System) -> Self {
        self.system = Some(s);
        self
    }
    pub fn shape(mut self, s: ShapeFilter) -> Self {
        self.shape = s;
        self
    }
    pub fn sizes(mut self, min: f64, max: f64) -> Self {
        self.size_min = min;
        self.size_max = max;
        self
    }
    pub fn branch_sizes(mut self, min: f64, max: f64) -> Self {
        self.branch_min = min;
        self.branch_max = max;
        self
    }
    pub fn angles(mut self, a: AngleSet) -> Self {
        self.angles = a;
        self
    }
    pub fn radius(mut self, factor: f64) -> Self {
        self.radius_factor = factor;
        self
    }
    pub fn tee_lengths(mut self, half: f64, stub: f64) -> Self {
        self.half_factor = half;
        self.stub_factor = stub;
        self
    }
    pub fn transition(mut self, angle_deg: f64, min_len: f64) -> Self {
        self.transition_angle_deg = angle_deg;
        self.min_len = min_len;
        self
    }

    fn base_fits(&self, class: FittingClass, domain: Domain, system: System, size: RunSize) -> bool {
        let d = crate::ref_dim(size);
        self.class == class
            && self.domain == domain
            && self.system.is_none_or(|s| s.compatible(system))
            && self.shape.fits(size)
            && d >= self.size_min - 1e-9
            && d <= self.size_max + 1e-9
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogueError {
    TooMany,
    BadName,
    BadRange,
    BadDimension,
    BadAngles,
    DuplicateName,
}

/// What `pick` found wrong, in order of how far it got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickError {
    /// no type of this class for this domain, system, shape and size
    NoType,
    /// types fit the size but none has this angle
    NoAngle,
    /// types fit the run but none takes this branch size
    NoBranchSize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalogue {
    pub types: Vec<FittingType>,
}

impl Catalogue {
    pub fn new(types: Vec<FittingType>) -> Catalogue {
        Catalogue { types }
    }

    pub fn check(&self) -> Result<(), CatalogueError> {
        if self.types.len() > MAX_TYPES {
            return Err(CatalogueError::TooMany);
        }
        let range = |a: f64, b: f64| a.is_finite() && b.is_finite() && a >= 0.0 && a <= b && b <= 1000.0;
        for (i, t) in self.types.iter().enumerate() {
            if t.name.is_empty() || t.name.chars().count() > 64 || t.name.chars().any(|c| c.is_control()) || t.part_code.chars().count() > 64 || t.part_code.chars().any(|c| c.is_control()) {
                return Err(CatalogueError::BadName);
            }
            if self.types[..i].iter().any(|o| o.name == t.name) {
                return Err(CatalogueError::DuplicateName);
            }
            if !range(t.size_min, t.size_max) || !range(t.branch_min, t.branch_max) {
                return Err(CatalogueError::BadRange);
            }
            let pos = |v: f64| v.is_finite() && v > 0.0 && v <= 1000.0;
            let nonneg = |v: f64| v.is_finite() && (0.0..=1000.0).contains(&v);
            if !pos(t.radius_factor) || !nonneg(t.half_factor) || !nonneg(t.stub_factor) || !nonneg(t.min_len) || !(t.transition_angle_deg.is_finite() && t.transition_angle_deg > 0.0 && t.transition_angle_deg < 180.0) {
                return Err(CatalogueError::BadDimension);
            }
            if let AngleSet::List(v) = &t.angles {
                if v.is_empty() || v.len() > 64 || v.iter().any(|a| !(a.is_finite() && *a > 0.0 && *a < 180.0)) {
                    return Err(CatalogueError::BadAngles);
                }
            }
        }
        Ok(())
    }

    /// The first elbow that has this bend angle for a run of this size.
    pub fn pick_elbow(&self, domain: Domain, system: System, size: RunSize, angle: f64) -> Result<&FittingType, PickError> {
        let mut any = false;
        for t in &self.types {
            if t.base_fits(FittingClass::Elbow, domain, system, size) {
                any = true;
                if t.angles.allows(angle) {
                    return Ok(t);
                }
            }
        }
        Err(if any { PickError::NoAngle } else { PickError::NoType })
    }

    /// The first tee or tap (`class`) for a run of this size and a branch of this size leaving at this angle.
    pub fn pick_junction(&self, class: FittingClass, domain: Domain, system: System, main: RunSize, branch: RunSize, angle: f64) -> Result<&FittingType, PickError> {
        let (mut any, mut any_branch) = (false, false);
        for t in &self.types {
            if t.base_fits(class, domain, system, main) {
                any = true;
                let b = crate::ref_dim(branch);
                if t.shape_branch_fits(branch) && b >= t.branch_min - 1e-9 && b <= t.branch_max + 1e-9 {
                    any_branch = true;
                    if t.angles.allows(angle) || t.angles.allows(180.0 - angle) {
                        return Ok(t);
                    }
                }
            }
        }
        Err(if !any {
            PickError::NoType
        } else if !any_branch {
            PickError::NoBranchSize
        } else {
            PickError::NoAngle
        })
    }

    /// The first transition that covers both sizes.
    pub fn pick_transition(&self, domain: Domain, system: System, from: RunSize, to: RunSize) -> Result<&FittingType, PickError> {
        self.types
            .iter()
            .find(|t| t.base_fits(FittingClass::Transition, domain, system, from) && t.base_fits(FittingClass::Transition, domain, system, to))
            .ok_or(PickError::NoType)
    }
}

impl FittingType {
    fn shape_branch_fits(&self, s: RunSize) -> bool {
        self.shape.fits(s)
    }
}
