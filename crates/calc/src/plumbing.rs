//! Plumbing demand: loading units turned into a design flow by a curve the caller supplies (BS EN 806-3, CIBSE Guide G
//! or any other code). The table's values are the owner's data; this crate ships none.

/// Design flow against the sum of loading units, from a table.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadingCurve {
    /// (loading units, flow in m3/s), ascending in both
    points: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CurveError {
    /// fewer than two rows
    TooShort,
    /// the line number (1-based) and what is wrong with it
    Line(usize, &'static str),
}

impl LoadingCurve {
    /// From points (loading units, flow in m3/s). Both must rise strictly, the loading units from zero or more.
    pub fn new(points: Vec<(f64, f64)>) -> Result<Self, CurveError> {
        if points.len() < 2 { return Err(CurveError::TooShort); }
        for (k, p) in points.iter().enumerate() {
            if !(p.0.is_finite() && p.1.is_finite() && p.0 >= 0.0 && p.1 >= 0.0) { return Err(CurveError::Line(k + 1, "a value is negative or not a number")); }
            if k > 0 && !(p.0 > points[k - 1].0 && p.1 >= points[k - 1].1) { return Err(CurveError::Line(k + 1, "the table must rise")); }
        }
        Ok(Self { points })
    }

    /// From CSV text: `loading_units,flow_l_s` (a header row, blank lines and `#` comments are allowed).
    /// The flow column is in litres per second, as the codes print it.
    pub fn from_csv(text: &str) -> Result<Self, CurveError> {
        let mut points = Vec::new();
        let mut lines = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let cells: Vec<&str> = line.split([',', ';']).map(str::trim).collect();
            if cells.len() != 2 { return Err(CurveError::Line(n + 1, "two columns are expected")); }
            match (cells[0].parse::<f64>(), cells[1].parse::<f64>()) {
                (Ok(lu), Ok(l_s)) => { points.push((lu, l_s / 1000.0)); lines.push(n + 1); }
                _ if points.is_empty() => continue, // the header
                _ => return Err(CurveError::Line(n + 1, "not a number")),
            }
        }
        // an error names the line of the file, not the row's number among the numbers
        Self::new(points).map_err(|e| match e { CurveError::Line(k, why) => CurveError::Line(lines[k - 1], why), other => other })
    }

    /// The highest loading-unit sum the table covers.
    pub fn limit(&self) -> f64 { self.points[self.points.len() - 1].0 }

    /// Flow for a loading-unit sum, m3/s; None above the table. Between rows the curve is a straight line on log-log
    /// axes (exact for the power laws these tables follow) and a straight line where a row is at zero.
    pub fn flow(&self, units: f64) -> Option<f64> {
        if !(units.is_finite() && units >= 0.0) || units > self.limit() * (1.0 + 1e-12) { return None; }
        if units == 0.0 { return Some(0.0); }
        let first = self.points[0];
        if units <= first.0 {
            // below the first row: in proportion, from nothing
            return Some(first.1 * units / first.0.max(f64::MIN_POSITIVE));
        }
        let k = self.points.partition_point(|p| p.0 < units).clamp(1, self.points.len() - 1);
        let ((x0, y0), (x1, y1)) = (self.points[k - 1], self.points[k]);
        if x0 > 0.0 && y0 > 0.0 {
            Some(y0 * (y1 / y0).powf((units / x0).ln() / (x1 / x0).ln()))
        } else {
            Some(y0 + (y1 - y0) * (units - x0) / (x1 - x0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // test data only (a made-up q = 0.1 * LU^0.6 l/s), not any code's table
    fn table() -> &'static str { "# made up\nloading_units,flow_l_s\n1,0.1\n10,0.398107\n100,1.584893\n1000,6.309573\n" }

    #[test]
    fn a_table_is_read_and_a_power_law_is_followed_between_rows() {
        let c = LoadingCurve::from_csv(table()).unwrap();
        assert_eq!(c.limit(), 1000.0);
        let q = c.flow(31.6227766).unwrap() * 1000.0;
        assert!((q - 0.1 * 31.6227766f64.powf(0.6)).abs() < 1e-3, "{q}");
        assert!((c.flow(100.0).unwrap() - 1.584893e-3).abs() < 1e-9);
        assert_eq!(c.flow(0.0), Some(0.0));
        assert!((c.flow(0.5).unwrap() - 0.05e-3).abs() < 1e-12, "below the first row it is in proportion");
    }

    #[test]
    fn above_the_table_or_nonsense_gives_nothing() {
        let c = LoadingCurve::from_csv(table()).unwrap();
        assert_eq!(c.flow(1000.5), None);
        assert_eq!(c.flow(-1.0), None);
        assert_eq!(c.flow(f64::NAN), None);
    }

    #[test]
    fn bad_tables_are_refused_with_the_line() {
        assert_eq!(LoadingCurve::from_csv("1,0.1"), Err(CurveError::TooShort));
        assert_eq!(LoadingCurve::from_csv("lu,q\n1,0.1\n1,0.2"), Err(CurveError::Line(3, "the table must rise")));
        assert_eq!(LoadingCurve::from_csv("lu,q\n1,0.1\n2,x"), Err(CurveError::Line(3, "not a number")));
        assert_eq!(LoadingCurve::from_csv("1,0.1,3\n"), Err(CurveError::Line(1, "two columns are expected")));
        assert_eq!(LoadingCurve::new(vec![(1.0, 0.1), (2.0, 0.05)]), Err(CurveError::Line(2, "the table must rise")));
    }
}
