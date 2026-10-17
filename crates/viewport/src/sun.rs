//! The position of the sun for a place, a day and a time, for the Realistic style (NOAA's general solar position
//! equations, good to a fraction of a degree; no atmospheric refraction).
//!
//! Model coordinates: x east, y north, z up, which assumes project north is true north (no rotation yet).

use std::f64::consts::PI;

/// Where and when to put the sun.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    /// degrees, north positive
    pub latitude: f64,
    /// degrees, east positive
    pub longitude: f64,
    /// hours ahead of UTC of the clock the time is read on
    pub utc_offset: f64,
    pub month: u32,
    pub day: u32,
    /// hours on that clock, 0..24
    pub hour: f64,
}

impl Default for Sun {
    /// Phnom Penh, 21 March, 15:00 (an editable starting point, not a design value).
    fn default() -> Sun { Sun { latitude: 11.5564, longitude: 104.9282, utc_offset: 7.0, month: 3, day: 21, hour: 15.0 } }
}

/// The sun in the sky: degrees above the horizon, and clockwise from north.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position { pub elevation: f64, pub azimuth: f64 }

fn day_of_year(month: u32, day: u32) -> f64 {
    const CUM: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    (CUM[(month.clamp(1, 12) - 1) as usize] + day.clamp(1, 31)) as f64
}

impl Sun {
    pub fn position(&self) -> Position {
        let n = day_of_year(self.month, self.day);
        let g = 2.0 * PI / 365.0 * (n - 1.0 + (self.hour - 12.0) / 24.0);
        let eqtime = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
        let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin() - 0.002697 * (3.0 * g).cos() + 0.00148 * (3.0 * g).sin();
        let tst = self.hour * 60.0 + eqtime + 4.0 * self.longitude - 60.0 * self.utc_offset;
        let ha = (tst / 4.0 - 180.0).to_radians();
        let lat = self.latitude.to_radians();
        let sin_el = lat.sin() * decl.sin() + lat.cos() * decl.cos() * ha.cos();
        let elevation = sin_el.clamp(-1.0, 1.0).asin();
        // azimuth from the south, positive toward the west, then turned to clockwise from north
        let from_south = ha.sin().atan2(ha.cos() * lat.sin() - decl.tan() * lat.cos());
        let azimuth = (from_south + PI).rem_euclid(2.0 * PI);
        Position { elevation: elevation.to_degrees(), azimuth: azimuth.to_degrees() }
    }

    /// The unit vector toward the sun in model coordinates (z is negative at night).
    pub fn direction(&self) -> [f64; 3] {
        let p = self.position();
        let (el, az) = (p.elevation.to_radians(), p.azimuth.to_radians());
        [az.sin() * el.cos(), az.cos() * el.cos(), el.sin()]
    }

    /// How much of the sun's light reaches the ground: 0 at the horizon and below, 1 from 8 degrees up.
    pub fn strength(&self) -> f64 { (self.position().elevation / 8.0).clamp(0.0, 1.0) }
}
