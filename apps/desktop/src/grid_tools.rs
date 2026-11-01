//! Grid datum alignment, re-derived from standard CAD behaviour (as in Revit and open-source BIM
//! editors). The **bubble line** of a grid end is the line through that end perpendicular to the grid.
//! When a grid end is drawn or dragged close to the bubble line of a parallel grid's end, it snaps onto
//! that line so the two grids come out aligned; a guide is drawn from the reference end to the snapped
//! point to show the alignment. Nothing here touches the document: it is pure geometry over `(start, end)`
//! pairs, so the same helper drives the create tool, the end grips and the whole-line move.

/// Two directions are parallel when they agree to within `deg` degrees, counted either way along a line
/// (so a grid and its reverse are parallel). A zero direction is never parallel to anything.
pub(crate) fn directions_parallel(u: [f64; 2], v: [f64; 2], deg: f64) -> bool {
    let [ux, uy] = unit(u);
    let [vx, vy] = unit(v);
    if (ux, uy) == (0.0, 0.0) || (vx, vy) == (0.0, 0.0) {
        return false;
    }
    // |cross| = |sin(angle between)|; within `deg` when it is at most sin(deg).
    (ux * vy - uy * vx).abs() <= deg.to_radians().sin()
}

fn unit(p: [f64; 2]) -> [f64; 2] {
    let len = p[0].hypot(p[1]);
    if len == 0.0 { [0.0, 0.0] } else { [p[0] / len, p[1] / len] }
}

/// A grid end snapped onto the bubble line of another grid's end.
pub(crate) struct EndAlignment {
    /// The snapped position of the moving end.
    pub point: [f64; 2],
    /// The end of the other grid its bubble line it landed on (for the guide).
    pub reference: [f64; 2],
}

/// Snap the grid end near `raw` onto the bubble line of a parallel grid's start or end, within `tol`
/// (model units). `others` are the other grids as `[start, end]`.
///
/// - with the grid's other end `fixed` (drawing the far point, or dragging one end), the moving end is
///   placed where the line through `fixed` parallel to a grid meets that grid's bubble line, so the grid
///   comes out parallel with its end lined up; only alignments ahead of `fixed`, in the way the pointer
///   lies, are considered;
/// - without `fixed`, the end goes onto the nearest bubble line.
///
/// The closest candidate wins; `None` when none is within `tol`.
pub(crate) fn align_end(others: &[[[f64; 2]; 2]], fixed: Option<[f64; 2]>, raw: [f64; 2], tol: f64) -> Option<EndAlignment> {
    if !tol.is_finite() || tol <= 0.0 || !raw[0].is_finite() || !raw[1].is_finite() {
        return None;
    }
    let mut best: Option<(EndAlignment, f64)> = None;
    for end in others {
        let dir = unit([end[1][0] - end[0][0], end[1][1] - end[0][1]]);
        if (dir[0], dir[1]) == (0.0, 0.0) { continue; }
        for e in [end[0], end[1]] {
            let cand = match fixed {
                Some(f) => {
                    // Along the other grid, oriented the way the pointer is from `fixed`.
                    let along = (raw[0] - f[0]) * dir[0] + (raw[1] - f[1]) * dir[1];
                    let d = if along < 0.0 { [-dir[0], -dir[1]] } else { dir };
                    let t = (e[0] - f[0]) * d[0] + (e[1] - f[1]) * d[1];
                    if t <= tol { continue; }
                    [f[0] + d[0] * t, f[1] + d[1] * t]
                }
                None => {
                    // The point of the bubble line through `e` (perpendicular to `dir`) closest to `raw`.
                    let proj = (raw[0] - e[0]) * dir[0] + (raw[1] - e[1]) * dir[1];
                    [raw[0] - dir[0] * proj, raw[1] - dir[1] * proj]
                }
            };
            let dist = (cand[0] - raw[0]).hypot(cand[1] - raw[1]);
            if cand[0].is_finite() && cand[1].is_finite() && dist <= tol
                && best.as_ref().is_none_or(|(_, b)| dist < *b)
            {
                best = Some((EndAlignment { point: cand, reference: e }, dist));
            }
        }
    }
    best.map(|(a, _)| a)
}

/// The `[start, end]` pairs of every grid in the document, for the alignment helper.
pub(crate) fn grid_ends(doc: &openbim_document::Document) -> Vec<[[f64; 2]; 2]> {
    doc.elements()
        .filter_map(|el| match el.geometry {
            Some(openbim_document::Geometry::Grid { start, end }) => Some([start, end]),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Grid A runs vertically at x=0 (ends (0,0) and (0,10)). Its bubble lines are horizontal
    // (perpendicular to the vertical grid) through y=0 and y=10.

    #[test]
    fn parallel_detection() {
        assert!(directions_parallel([0.0, 1.0], [0.0, -1.0], 1.0), "a line is parallel to its reverse");
        assert!(directions_parallel([0.0, 1.0], [0.0, 1.0], 1.0));
        assert!(!directions_parallel([0.0, 1.0], [1.0, 0.0], 1.0), "perpendicular is not parallel");
        assert!(!directions_parallel([0.0, 0.0], [1.0, 0.0], 1.0), "a zero direction is never parallel");
    }

    #[test]
    fn free_end_snaps_onto_nearest_bubble_line() {
        // A grid B parallel to A, being drawn; its far end near y=10 should land on A's y=10 bubble line.
        let others = [[[0.0, 0.0], [0.0, 10.0]]];
        let a = align_end(&others, None, [3.0, 9.7], 0.5).expect("aligns to the bubble line");
        assert!((a.point[1] - 10.0).abs() < 1e-9, "end lands on the y=10 bubble line: {:?}", a.point);
        assert!((a.reference[1] - 10.0).abs() < 1e-9, "reference is A's far end: {:?}", a.reference);
        // Too far away: no alignment.
        assert!(align_end(&others, None, [3.0, 5.0], 0.5).is_none(), "mid-span is not a bubble line");
    }

    #[test]
    fn fixed_end_stays_parallel_and_lines_up() {
        // Grid B fixed at (5, 0), being pulled toward A's ends; the moving end should land where the
        // line through (5,0) parallel to A (vertical) meets a bubble line: (5, 10), lined up with A's end.
        let others = [[[0.0, 0.0], [0.0, 10.0]]];
        let a = align_end(&others, Some([5.0, 0.0]), [5.2, 9.8], 0.5).expect("aligns while parallel");
        assert!((a.point[0] - 5.0).abs() < 1e-9 && (a.point[1] - 10.0).abs() < 1e-9, "parallel, end lined up: {:?}", a.point);
        assert!((a.reference[1] - 10.0).abs() < 1e-9, "reference is A's far end");
    }
}


/// Next auto name for a horizontal grid: the smallest positive integer not already used by a numeric grid name.
pub fn next_grid_number(doc: &openbim_document::Document) -> String {
    let mut max: u64 = 0;
    for g in doc.elements() {
        if g.category != openbim_document::Category::Grid { continue; }
        if let Ok(n) = g.name.parse::<u64>() { if n > max { max = n; } }
    }
    (max + 1).to_string()
}

/// Next auto name for a vertical grid: the letter after the last lettered grid (A, B, …, Z, AA, …).
pub fn next_grid_letter(doc: &openbim_document::Document) -> String {
    let mut best: Option<String> = None;
    for g in doc.elements() {
        if g.category != openbim_document::Category::Grid { continue; }
        let n = &g.name;
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_uppercase()) {
            match &best {
                None => best = Some(n.clone()),
                Some(b) if n > b => best = Some(n.clone()),
                _ => {}
            }
        }
    }
    match best {
        None => "A".to_string(),
        Some(b) => {
            let mut chars: Vec<u8> = b.bytes().collect();
            let mut i = chars.len();
            loop {
                if i == 0 { chars.insert(0, b'A'); break; }
                i -= 1;
                if chars[i] == b'Z' { chars[i] = b'A'; }
                else { chars[i] += 1; break; }
            }
            String::from_utf8(chars).unwrap_or_default()
        }
    }
}
