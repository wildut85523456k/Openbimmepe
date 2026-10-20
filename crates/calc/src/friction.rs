use crate::Fluid;

/// A size from the catalogue the caller gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Size {
    /// what the designer reads: "DN50", "400 x 300"
    pub label: String,
    pub shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// internal diameter, metres
    Round { diameter: f64 },
    /// internal width and height, metres
    Rect { width: f64, height: f64 },
}

impl Size {
    pub fn round(label: &str, diameter: f64) -> Size { Size { label: label.into(), shape: Shape::Round { diameter } } }
    pub fn rect(label: &str, width: f64, height: f64) -> Size { Size { label: label.into(), shape: Shape::Rect { width, height } } }

    pub fn valid(&self) -> bool {
        match self.shape {
            Shape::Round { diameter } => diameter.is_finite() && diameter > 0.0,
            Shape::Rect { width, height } => width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0,
        }
    }

    /// Flow area, m2.
    pub fn area(&self) -> f64 {
        match self.shape {
            Shape::Round { diameter } => std::f64::consts::PI * diameter * diameter / 4.0,
            Shape::Rect { width, height } => width * height,
        }
    }

    /// The diameter of the round section that has the same friction at the same flow: the circle itself, or for a
    /// rectangle the equivalent diameter of Huebscher.
    pub fn friction_diameter(&self) -> f64 {
        match self.shape {
            Shape::Round { diameter } => diameter,
            Shape::Rect { width, height } => equivalent_diameter(width, height),
        }
    }
}

/// The diameter of the round duct that has the same friction and carries the same flow as a rectangular duct
/// (Huebscher): 1.30 (a b)^0.625 / (a + b)^0.25.
pub fn equivalent_diameter(a: f64, b: f64) -> f64 { 1.30 * (a * b).powf(0.625) / (a + b).powf(0.25) }

/// The Darcy friction factor from the Colebrook-White equation, solved by Newton's method on x = 1/sqrt(f).
/// `relative_roughness` is roughness over diameter. Laminar flow (Reynolds below 2300) is 64 / Re.
pub fn colebrook(reynolds: f64, relative_roughness: f64) -> f64 {
    if reynolds < 2300.0 { return 64.0 / reynolds.max(1e-9); }
    let re = reynolds.max(4000.0);
    let e = relative_roughness / 3.7;
    // the Swamee-Jain value is within 1 % and starts Newton close to the root
    let mut x = 1.0 / swamee_jain(re, relative_roughness).sqrt();
    for _ in 0..50 {
        // g(x) = x + 2 log10(e + 2.51 x / Re), g'(x) = 1 + 2 / ln 10 x 2.51 / Re / (e + 2.51 x / Re)
        let t = e + 2.51 * x / re;
        let g = x + 2.0 * t.log10();
        let dg = 1.0 + 2.0 / std::f64::consts::LN_10 * (2.51 / re) / t;
        let step = g / dg;
        x -= step;
        if step.abs() < 1e-13 * x.abs() { break; }
    }
    1.0 / (x * x)
}

/// The explicit approximation of Swamee and Jain, within about 1 % of Colebrook for Reynolds 5000 to 1e8.
pub fn swamee_jain(reynolds: f64, relative_roughness: f64) -> f64 {
    let t = (relative_roughness / 3.7 + 5.74 / reynolds.powf(0.9)).log10();
    0.25 / (t * t)
}

/// Friction pressure drop per metre (Pa/m), the velocity (m/s) and the Reynolds number of `flow` in `size`.
pub(crate) fn friction_per_metre(flow: f64, size: &Size, roughness: f64, fluid: &Fluid) -> (f64, f64, f64) {
    let d = size.friction_diameter();
    // equal friction and equal flow: the velocity in the equivalent round section
    let v_eq = flow / (std::f64::consts::PI * d * d / 4.0);
    let re = fluid.density * v_eq * d / fluid.viscosity;
    let f = colebrook(re, roughness / d);
    let per_m = f / d * fluid.density * v_eq * v_eq / 2.0;
    let v = flow / size.area();
    (per_m, v, re)
}

/// Hazen-Williams friction (Pa/m) and velocity (m/s) of `flow` in `size`, for sprinkler and fire-main hydraulics:
/// dp/L = rho g 10.67 Q^1.852 / (C^1.852 d^4.8704), SI units. Reynolds is not used, so 0 is returned for it.
pub(crate) fn hazen_williams_per_metre(flow: f64, size: &Size, c: f64, fluid: &Fluid) -> (f64, f64, f64) {
    let d = size.friction_diameter();
    let head_per_m = 10.67 * flow.powf(1.852) / (c.powf(1.852) * d.powf(4.8704));
    (head_per_m * fluid.density * 9.80665, flow / size.area(), 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazen_williams_matches_a_hand_calculation() {
        // 10 l/s in 100 mm, C 120: about 0.0221 m/m (Darcy at f 0.02 gives 0.0164 for a smoother pipe, C 120 is rougher)
        let (pa, v, _) = hazen_williams_per_metre(0.01, &Size::round("100", 0.1), 120.0, &Fluid::WATER);
        let head = 10.67 * 0.01f64.powf(1.852) / (120f64.powf(1.852) * 0.1f64.powf(4.8704));
        assert!((head - 0.0221).abs() < 0.0002, "{head}");
        assert!((pa - head * 998.0 * 9.80665).abs() < 1e-9 && (v - 1.2732).abs() < 1e-3);
    }

    #[test]
    fn laminar_flow_is_sixty_four_over_reynolds() {
        assert!((colebrook(1000.0, 0.001) - 0.064).abs() < 1e-12);
    }

    #[test]
    fn colebrook_solves_its_own_equation_and_swamee_jain_agrees_within_its_published_accuracy() {
        for re in [5.0e3, 2.0e4, 1.0e5, 5.0e5, 3.0e6, 5.0e7] {
            for rr in [0.0, 1e-5, 1e-4, 1e-3, 1e-2, 0.04] {
                let f = colebrook(re, rr);
                // 1/sqrt(f) = -2 log10(rr/3.7 + 2.51 / (Re sqrt(f)))
                let lhs = 1.0 / f.sqrt();
                let rhs = -2.0 * (rr / 3.7 + 2.51 / (re * f.sqrt())).log10();
                assert!((lhs - rhs).abs() < 1e-9, "Re {re} rr {rr}: {lhs} vs {rhs}");
                if rr > 0.0 || re < 1e8 {
                    let sj = swamee_jain(re, rr);
                    assert!((sj - f).abs() / f < 0.035, "Re {re} rr {rr}: Colebrook {f} Swamee-Jain {sj}");
                }
            }
        }
    }

    #[test]
    fn the_friction_factor_of_a_smooth_turbulent_pipe_matches_the_moody_chart() {
        // Re 1e5 smooth: f = 0.0180 (Blasius gives 0.0178)
        assert!((colebrook(1.0e5, 0.0) - 0.0180).abs() < 0.0003, "{}", colebrook(1.0e5, 0.0));
        // fully rough, relative roughness 0.01: f = 0.0379 (von Karman)
        assert!((colebrook(1.0e8, 0.01) - 0.0379).abs() < 0.0006, "{}", colebrook(1.0e8, 0.01));
    }

    #[test]
    fn a_square_duct_and_a_flat_duct_have_the_equivalent_diameters_of_huebscher() {
        assert!((equivalent_diameter(0.3, 0.3) - 0.3279).abs() < 0.0005, "{}", equivalent_diameter(0.3, 0.3));
        // a flat duct is less efficient than a square one of the same area
        assert!(equivalent_diameter(0.6, 0.15) < equivalent_diameter(0.3, 0.3));
    }

    #[test]
    fn a_pipe_loses_what_darcy_weisbach_says() {
        // 2 l/s of water in an inside diameter of 50 mm, steel, roughness 0.045 mm
        let size = Size::round("DN50", 0.05);
        let (per_m, v, re) = friction_per_metre(0.002, &size, 0.045e-3, &Fluid::WATER);
        assert!((v - 1.0186).abs() < 0.0005 && (re - 50_700.0).abs() < 500.0, "{v} {re}");
        let f = swamee_jain(re, 0.045e-3 / 0.05);
        let by_hand = f / 0.05 * 998.0 * v * v / 2.0;
        assert!((per_m - by_hand).abs() / by_hand < 0.02, "{per_m} vs {by_hand}");
        assert!((per_m - 254.0).abs() < 25.0, "about 254 Pa/m (Moody f 0.0245): {per_m}");
    }
}
