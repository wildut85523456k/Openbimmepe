//! Preset angle snaps for drawing ducts, pipes and trays.
//!
//! Two modes, as in Revit: a fixed increment (every 15 degrees, say) or a list of specific angles.
//! Plan angles are measured counter-clockwise from the +X axis; a listed angle is mirrored into all four
//! quadrants (45 means 45, 135, 225 and 315). Vertical angles are measured up from the horizontal, from
//! -90 to 90; a listed angle is mirrored up and down. A project holds the default; a session can override
//! it without changing the project.

/// How an angle is snapped.
#[derive(Clone, Debug, PartialEq)]
pub enum SnapMode {
    /// leave the angle as drawn
    Off,
    /// nearest multiple of this many degrees: more than 0, at most 90, and it must divide 360 evenly (and 90 for
    /// the vertical angle) so the grid is the same all round
    Increment(f64),
    /// nearest of these angles (degrees, at most 64 of them)
    Angles(Vec<f64>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapSettings {
    pub plan: SnapMode,
    pub vertical: SnapMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapError {
    /// an increment must be more than 0, at most 90 degrees, and divide 360 (plan) or 90 (vertical) evenly
    Increment,
    /// a list needs 1 to 64 finite angles; plan angles from 0 to 360, vertical angles from 0 to 90
    List,
}

pub const MAX_ANGLES: usize = 64;

impl SnapMode {
    pub fn check(&self, vertical: bool) -> Result<(), SnapError> {
        match self {
            SnapMode::Off => Ok(()),
            SnapMode::Increment(s) => {
                // the steps must fit evenly round the circle (plan) or up to the vertical (vertical), so the grid is the same everywhere
                let whole = |x: f64| (x - x.round()).abs() < 1e-9;
                let span = if vertical { 90.0 } else { 360.0 };
                if s.is_finite() && *s > 0.0 && *s <= 90.0 && whole(span / s) { Ok(()) } else { Err(SnapError::Increment) }
            }
            SnapMode::Angles(v) => {
                let hi = if vertical { 90.0 } else { 360.0 };
                if !v.is_empty() && v.len() <= MAX_ANGLES && v.iter().all(|a| a.is_finite() && (0.0..=hi).contains(a)) { Ok(()) } else { Err(SnapError::List) }
            }
        }
    }

    /// Snap a plan angle (degrees). The result is in (-180, 180]. For an increment, a half-way angle goes away from
    /// zero; for a list, a tie goes to the angle nearer the X axis (then to the positive one).
    pub fn snap_plan(&self, deg: f64) -> f64 {
        let a = norm180(deg);
        match self {
            SnapMode::Off => a,
            SnapMode::Increment(s) => norm180((a / s).round() * s),
            SnapMode::Angles(list) => {
                let mut best: Option<(f64, f64)> = None; // (distance, angle)
                for &l in list {
                    for c in [l, 180.0 - l, 180.0 + l, -l] {
                        let c = norm180(c);
                        let d = norm180(a - c).abs();
                        best = match best {
                            Some((bd, bc)) if bd < d - 1e-12 || ((bd - d).abs() <= 1e-12 && (bc.abs(), -bc) <= (c.abs(), -c)) => Some((bd, bc)),
                            _ => Some((d, c)),
                        };
                    }
                }
                best.map(|b| b.1).unwrap_or(a)
            }
        }
    }

    /// Snap a vertical angle (degrees, -90 to 90). Ties go to the angle nearer the horizontal.
    pub fn snap_vertical(&self, deg: f64) -> f64 {
        let a = deg.clamp(-90.0, 90.0);
        match self {
            SnapMode::Off => a,
            SnapMode::Increment(s) => ((a / s).round() * s).clamp(-90.0, 90.0),
            SnapMode::Angles(list) => {
                let mut best: Option<(f64, f64)> = None;
                for &l in list {
                    for c in [l, -l] {
                        let d = (a - c).abs();
                        best = match best {
                            Some((bd, bc)) if bd < d - 1e-12 || ((bd - d).abs() <= 1e-12 && bc.abs() <= c.abs()) => Some((bd, bc)),
                            _ => Some((d, c)),
                        };
                    }
                }
                best.map(|b| b.1).unwrap_or(a)
            }
        }
    }
}

impl SnapSettings {
    pub fn check(&self) -> Result<(), SnapError> {
        self.plan.check(false)?;
        self.vertical.check(true)
    }
    pub fn off() -> SnapSettings {
        SnapSettings { plan: SnapMode::Off, vertical: SnapMode::Off }
    }
}

/// The snaps in force: the project's default, unless this session overrides it.
#[derive(Clone, Debug, PartialEq)]
pub struct Snaps {
    pub project: SnapSettings,
    pub session: Option<SnapSettings>,
}

impl Snaps {
    pub fn new(project: SnapSettings) -> Snaps {
        Snaps { project, session: None }
    }
    pub fn active(&self) -> &SnapSettings {
        self.session.as_ref().unwrap_or(&self.project)
    }
    /// Override for this session only. The project default is untouched.
    pub fn set_session(&mut self, s: SnapSettings) -> Result<(), SnapError> {
        s.check()?;
        self.session = Some(s);
        Ok(())
    }
    pub fn clear_session(&mut self) {
        self.session = None;
    }
    /// Change the project default (kept in the project).
    pub fn set_project(&mut self, s: SnapSettings) -> Result<(), SnapError> {
        s.check()?;
        self.project = s;
        Ok(())
    }
}

fn norm180(a: f64) -> f64 {
    let mut x = a % 360.0;
    if x <= -180.0 {
        x += 360.0;
    } else if x > 180.0 {
        x -= 360.0;
    }
    x
}
