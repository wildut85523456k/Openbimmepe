//! Cross-sections of structural members: the outline in the plane across the member, x across and y up,
//! centred on the member's line. Beams sweep one along their length and columns stand one on their footprint.

/// A section a member can have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Profile {
    /// a rectangle `width` across and `depth` high
    Rect { width: f64, depth: f64 },
    Circle { diameter: f64 },
    /// a symmetric I or H: flanges `width` wide and `flange` thick, a web `web` thick, `depth` high overall
    I { width: f64, depth: f64, web: f64, flange: f64 },
}

/// Segments of the polygon that stands for a circle in a mesh.
pub const CIRCLE_SEGMENTS: usize = 32;

impl Profile {
    pub fn check(&self) -> Result<(), &'static str> {
        let ok = |v: f64| v.is_finite() && v > 0.0 && v <= 1e3;
        match *self {
            Profile::Rect { width, depth } if ok(width) && ok(depth) => Ok(()),
            Profile::Circle { diameter } if ok(diameter) => Ok(()),
            Profile::I { width, depth, web, flange } if ok(width) && ok(depth) && ok(web) && ok(flange) => {
                if web >= width {
                    Err("the web of an I-section must be thinner than its flanges are wide")
                } else if 2.0 * flange >= depth {
                    Err("the two flanges of an I-section must leave room for the web")
                } else {
                    Ok(())
                }
            }
            _ => Err("a section needs positive, finite sizes"),
        }
    }

    /// Width across and height of the section's box.
    pub fn bounds(&self) -> (f64, f64) {
        match *self {
            Profile::Rect { width, depth } | Profile::I { width, depth, .. } => (width, depth),
            Profile::Circle { diameter } => (diameter, diameter),
        }
    }

    /// The exact area of the section.
    pub fn area(&self) -> f64 {
        match *self {
            Profile::Rect { width, depth } => width * depth,
            Profile::Circle { diameter } => std::f64::consts::PI * diameter * diameter / 4.0,
            Profile::I { width, depth, web, flange } => 2.0 * width * flange + (depth - 2.0 * flange) * web,
        }
    }

    /// The outline, counter-clockwise. A circle is a polygon of [`CIRCLE_SEGMENTS`] corners on the circle.
    pub fn outline(&self) -> Vec<[f64; 2]> {
        match *self {
            Profile::Rect { width: w, depth: d } => vec![[-w / 2.0, -d / 2.0], [w / 2.0, -d / 2.0], [w / 2.0, d / 2.0], [-w / 2.0, d / 2.0]],
            Profile::Circle { diameter } => (0..CIRCLE_SEGMENTS)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / CIRCLE_SEGMENTS as f64;
                    [diameter / 2.0 * a.cos(), diameter / 2.0 * a.sin()]
                })
                .collect(),
            Profile::I { width: b, depth: d, web: tw, flange: tf } => vec![
                [-b / 2.0, -d / 2.0], [b / 2.0, -d / 2.0], [b / 2.0, -d / 2.0 + tf], [tw / 2.0, -d / 2.0 + tf],
                [tw / 2.0, d / 2.0 - tf], [b / 2.0, d / 2.0 - tf], [b / 2.0, d / 2.0], [-b / 2.0, d / 2.0],
                [-b / 2.0, d / 2.0 - tf], [-tw / 2.0, d / 2.0 - tf], [-tw / 2.0, -d / 2.0 + tf], [-b / 2.0, -d / 2.0 + tf],
            ],
        }
    }
}

/// The area of a polygon, positive when it runs counter-clockwise.
pub fn polygon_area(p: &[[f64; 2]]) -> f64 {
    (0..p.len()).map(|i| { let (a, b) = (p[i], p[(i + 1) % p.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0
}
