//! Envelope quantities of wall chains: for every segment the compass direction it faces, its outside face
//! area, the openings cut in it and the net opaque area. These feed the heating and cooling load modules.
//! Flat-topped walls only; sloped tops are refused rather than guessed.

use crate::{fin, ring_area, ArchError};
use openbim_geom::WallChain;

pub const SECTOR_NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side { Left, Right, Outside }

#[derive(Debug, Clone, PartialEq)]
pub struct EnvSegment {
    pub segment: usize,
    /// compass direction the outside face looks towards, degrees clockwise from true north, 0 <= az < 360
    pub azimuth: f64,
    /// index into `SECTOR_NAMES`
    pub sector: usize,
    /// outside face length x wall height
    pub gross_area: f64,
    /// doors and windows cut in this segment
    pub opening_area: f64,
    pub net_area: f64,
}

pub fn sector_of(azimuth: f64) -> usize { (((azimuth + 22.5) / 45.0).floor() as i64).rem_euclid(8) as usize }

/// Envelope of one wall chain. `exterior` says which side of the travel direction is outside; for a closed
/// chain `Side::Outside` means away from the area the chain encloses. `north` is the angle by which true north
/// lies counter-clockwise from the +y axis of the plan.
pub fn wall_envelope(chain: &WallChain, exterior: Side, north: f64) -> Result<Vec<EnvSegment>, ArchError> {
    fin(north, "north angle")?;
    if chain.tops.iter().any(|t| t.is_some()) { return Err(ArchError::Unsupported("sloped wall tops in an envelope")); }
    let plans = chain.plans().map_err(ArchError::Chain)?;
    let right = match exterior {
        Side::Left => false,
        Side::Right => true,
        Side::Outside => {
            if !chain.closed { return Err(ArchError::NeedClosedChain); }
            ring_area(&chain.points) > 0.0 // counter-clockwise: outside is on the right
        }
    };
    let mut out = Vec::with_capacity(plans.len());
    for (i, p) in plans.iter().enumerate() {
        let (a, b) = if right { (p.r0, p.r1) } else { (p.l0, p.l1) };
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        // normal pointing out: right of travel is (dy, -dx), left is (-dy, dx)
        let n = if right { [dy, -dx] } else { [-dy, dx] };
        let az = (n[0].atan2(n[1]).to_degrees() + north).rem_euclid(360.0);
        let az = if az >= 360.0 { 0.0 } else { az }; // rem_euclid can round a tiny negative angle up to 360
        let gross = len * chain.height;
        let mut open = 0.0;
        for o in chain.openings.get(i).into_iter().flatten() { open += o.width * o.height; }
        for q in chain.poly_openings.get(i).into_iter().flatten() { open += ring_area(q).abs(); }
        if open > gross { return Err(ArchError::BadValue { what: "opening area", name: format!("segment {i}") }); }
        out.push(EnvSegment { segment: i, azimuth: az, sector: sector_of(az), gross_area: gross, opening_area: open, net_area: gross - open });
    }
    Ok(out)
}

/// Totals per compass sector: (net opaque area, opening area).
pub fn by_sector(segs: &[EnvSegment]) -> [(f64, f64); 8] {
    let mut t = [(0.0, 0.0); 8];
    for s in segs { t[s.sector].0 += s.net_area; t[s.sector].1 += s.opening_area; }
    t
}
