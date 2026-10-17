//! How a parameter is shown and typed: in the unit the project chose for its kind of quantity. Values are kept in
//! the program's own units (metres, degrees, seconds, kilograms, kelvin and what is built from them); this only
//! changes how they are written and read. A number typed without a unit is taken in the unit shown, as in Revit.
use crate::expr::*;

/// (symbol, how many program units one of it is, what it measures)
pub const UNITS: &[(&str, f64, Dim)] = &[
    ("mm", 0.001, LENGTH),
    ("cm", 0.01, LENGTH),
    ("m", 1.0, LENGTH),
    ("km", 1000.0, LENGTH),
    ("in", 0.0254, LENGTH),
    ("ft", 0.3048, LENGTH),
    ("mm2", 1e-6, AREA),
    ("cm2", 1e-4, AREA),
    ("m2", 1.0, AREA),
    ("ft2", 0.09290304, AREA),
    ("L", 0.001, VOLUME),
    ("m3", 1.0, VOLUME),
    ("ft3", 0.028316846592, VOLUME),
    ("deg", 1.0, ANGLE),
    ("rad", 180.0 / std::f64::consts::PI, ANGLE),
    ("s", 1.0, TIME),
    ("min", 60.0, TIME),
    ("h", 3600.0, TIME),
    ("g", 0.001, MASS),
    ("kg", 1.0, MASS),
    ("t", 1000.0, MASS),
    ("K", 1.0, TEMPERATURE),
    ("m3/s", 1.0, FLOW),
    ("L/s", 0.001, FLOW),
    ("m3/h", 1.0 / 3600.0, FLOW),
    ("N", 1.0, FORCE),
    ("kN", 1000.0, FORCE),
    ("Pa", 1.0, PRESSURE),
    ("kPa", 1000.0, PRESSURE),
    ("bar", 100000.0, PRESSURE),
    ("J", 1.0, ENERGY),
    ("kJ", 1000.0, ENERGY),
    ("W", 1.0, POWER),
    ("kW", 1000.0, POWER),
    ("W/m2K", 1.0, U_VALUE),
];

#[derive(Debug, Clone, PartialEq)]
pub enum UnitError {
    Unknown(String),
    /// the unit measures something else than the value does
    WrongKind {
        unit: String,
        wanted: String,
    },
    NotANumber(String),
}

impl std::fmt::Display for UnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitError::Unknown(u) => write!(f, "unknown unit '{u}'"),
            UnitError::WrongKind { unit, wanted } => {
                write!(f, "'{unit}' cannot be used for {wanted}")
            }
            UnitError::NotANumber(t) => write!(f, "'{t}' is not a number"),
        }
    }
}
impl std::error::Error for UnitError {}

fn find(symbol: &str) -> Option<(f64, Dim)> {
    UNITS
        .iter()
        .find(|(s, _, _)| *s == symbol)
        .map(|(_, f, d)| (*f, *d))
}

/// The unit chosen for each kind of quantity. A kind without a choice is shown in the first unit of the table
/// that measures it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayUnits {
    chosen: Vec<(Dim, &'static str)>,
}

impl DisplayUnits {
    /// Millimetres for lengths and the program's own units for the rest, the usual way for a metric building project.
    pub fn metric() -> DisplayUnits {
        let mut d = DisplayUnits::default();
        for (dim, s) in [
            (LENGTH, "mm"),
            (AREA, "m2"),
            (VOLUME, "m3"),
            (ANGLE, "deg"),
            (FLOW, "L/s"),
            (PRESSURE, "Pa"),
            (POWER, "kW"),
        ] {
            d.set(dim, s).expect("table units");
        }
        d
    }
    /// Choose the unit shown for a kind of quantity.
    pub fn set(&mut self, dim: Dim, symbol: &str) -> Result<(), UnitError> {
        let Some((s, _, d)) = UNITS.iter().find(|(s, _, _)| *s == symbol) else {
            return Err(UnitError::Unknown(symbol.into()));
        };
        if *d != dim {
            return Err(UnitError::WrongKind {
                unit: symbol.into(),
                wanted: dim.name(),
            });
        }
        self.chosen.retain(|(k, _)| *k != dim);
        self.chosen.push((dim, s));
        Ok(())
    }
    /// The symbol a kind of quantity is shown in; `None` for a dimension the table has no unit for.
    pub fn symbol(&self, dim: Dim) -> Option<&'static str> {
        self.chosen
            .iter()
            .find(|(k, _)| *k == dim)
            .map(|(_, s)| *s)
            .or_else(|| UNITS.iter().find(|(_, _, d)| *d == dim).map(|(s, _, _)| *s))
    }
    /// A value as text with its unit, such as "1200 mm"; trailing zeros are dropped. A dimension without a unit is
    /// written as a plain number.
    pub fn format(&self, value: f64, dim: Dim, decimals: usize) -> String {
        let Some(sym) = self.symbol(dim) else {
            return trim(value, decimals);
        };
        let (f, _) = find(sym).expect("table unit");
        format!("{} {sym}", trim(value / f, decimals))
    }
    /// A number typed for a value of this kind, with a unit or without. Without one it is in the unit shown.
    pub fn parse(&self, text: &str, dim: Dim) -> Result<f64, UnitError> {
        let t = text.trim();
        let cut = t
            .char_indices()
            .find(|(i, c)| {
                c.is_alphabetic() && !(*c == 'e' || *c == 'E')
                    || (*c == 'e' || *c == 'E')
                        && !t[*i + 1..]
                            .starts_with(|n: char| n.is_ascii_digit() || n == '-' || n == '+')
            })
            .map_or(t.len(), |(i, _)| i);
        let (num, unit) = (t[..cut].trim(), t[cut..].trim());
        let x: f64 = num
            .parse()
            .map_err(|_| UnitError::NotANumber(text.trim().into()))?;
        if !x.is_finite() {
            return Err(UnitError::NotANumber(text.trim().into()));
        }
        if unit.is_empty() {
            let f = self.symbol(dim).and_then(find).map_or(1.0, |(f, _)| f);
            return Ok(x * f);
        }
        let (f, d) = find(unit).ok_or_else(|| UnitError::Unknown(unit.into()))?;
        if d != dim {
            return Err(UnitError::WrongKind {
                unit: unit.into(),
                wanted: dim.name(),
            });
        }
        Ok(x * f)
    }
}

fn trim(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}
