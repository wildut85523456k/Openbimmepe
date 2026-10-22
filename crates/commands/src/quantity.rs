//! A quantity: an SI number plus a dimension.
#![forbid(unsafe_code)]

use super::{by_symbol, canonical, Dimension, Unit};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A physical quantity: an SI value plus the [`Dimension`] it measures.
///
/// Every value that crosses the command and query API is a `Quantity`, so a
/// dimension mismatch is a value error rather than a silent unit mix-up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    si: f64,
    dimension: Dimension,
}

/// Two quantities could not be combined because their dimensions differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DimensionMismatch {
    pub left: Dimension,
    pub right: Dimension,
}

impl fmt::Display for DimensionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "dimension mismatch: cannot combine {} and {}",
            self.left, self.right
        )
    }
}

impl std::error::Error for DimensionMismatch {}

/// Why a quantity could not be read from text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    NoNumber(String),
    UnknownUnit(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty quantity"),
            Self::NoNumber(s) => write!(f, "no number in {s:?}"),
            Self::UnknownUnit(s) => write!(f, "unknown unit {s:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl Quantity {
    /// A quantity from its canonical SI value.
    pub const fn from_si(dimension: Dimension, si: f64) -> Quantity {
        Quantity { si, dimension }
    }

    /// A quantity of `value` expressed in `unit`.
    pub fn of(unit: Unit, value: f64) -> Quantity {
        Quantity {
            si: unit.to_si(value),
            dimension: unit.dimension,
        }
    }

    /// Read a quantity from text: a number and a unit, as `6 m/s`, `-3.5 °C` or
    /// `90°`. The unit may be any symbol in [`UNITS`](super::UNITS); `°` is an
    /// alias for `deg`.
    pub fn parse(text: &str) -> Result<Quantity, ParseError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ParseError::Empty);
        }
        let bytes = text.as_bytes();
        let mut i = 0;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let mut seen_digit = false;
        let mut seen_dot = false;
        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_digit() {
                seen_digit = true;
                i += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                i += 1;
            } else if (c == b'e' || c == b'E') && seen_digit {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                break;
            } else {
                break;
            }
        }
        if !seen_digit {
            return Err(ParseError::NoNumber(text.to_string()));
        }
        let number: f64 = text[..i]
            .parse()
            .map_err(|_| ParseError::NoNumber(text.to_string()))?;
        let symbol = text[i..].trim();
        let symbol = if symbol == "°" { "deg" } else { symbol };
        let unit = by_symbol(symbol).ok_or_else(|| ParseError::UnknownUnit(symbol.to_string()))?;
        Ok(Quantity::of(*unit, number))
    }

    /// The value in this quantity's canonical unit.
    pub fn value(self) -> f64 {
        canonical(self.dimension).from_si(self.si)
    }

    /// The value in `unit`, which must be of the same dimension.
    pub fn value_in(self, unit: Unit) -> Result<f64, DimensionMismatch> {
        self.require(unit.dimension)?;
        Ok(unit.from_si(self.si))
    }

    /// The canonical SI value.
    pub fn si(self) -> f64 {
        self.si
    }

    /// The dimension this quantity measures.
    pub fn dimension(self) -> Dimension {
        self.dimension
    }

    /// `self + other`, or a [`DimensionMismatch`] when the dimensions differ.
    pub fn checked_add(self, other: Quantity) -> Result<Quantity, DimensionMismatch> {
        self.require(other.dimension)?;
        Ok(Quantity::from_si(self.dimension, self.si + other.si))
    }

    /// `self - other`, or a [`DimensionMismatch`] when the dimensions differ.
    pub fn checked_sub(self, other: Quantity) -> Result<Quantity, DimensionMismatch> {
        self.require(other.dimension)?;
        Ok(Quantity::from_si(self.dimension, self.si - other.si))
    }

    /// Whether `other` measures the same dimension.
    pub fn same_dimension(self, other: Quantity) -> bool {
        self.dimension == other.dimension
    }

    fn require(self, dimension: Dimension) -> Result<(), DimensionMismatch> {
        if self.dimension == dimension {
            Ok(())
        } else {
            Err(DimensionMismatch {
                left: self.dimension,
                right: dimension,
            })
        }
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value(), self.dimension)
    }
}

/// A quantity is carried as `{"value": <number>, "unit": "<symbol>"}`, matching
/// the MCP wire form (04-visual-automation.md, rule 7).
impl Serialize for Quantity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let unit = canonical(self.dimension);
        let mut s = serializer.serialize_struct("Quantity", 2)?;
        s.serialize_field("value", &unit.from_si(self.si))?;
        s.serialize_field("unit", unit.symbol)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            value: f64,
            unit: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        let unit = by_symbol(&raw.unit)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown unit {:?}", raw.unit)))?;
        Ok(Quantity::of(*unit, raw.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dimension_mismatch_is_an_error() {
        let length = Quantity::of(*by_symbol("m").unwrap(), 3.0);
        let area = Quantity::of(*by_symbol("m²").unwrap(), 2.0);
        assert_eq!(
            length.checked_add(area),
            Err(DimensionMismatch {
                left: Dimension::Length,
                right: Dimension::Area
            })
        );
        assert_eq!(
            area.checked_sub(length),
            Err(DimensionMismatch {
                left: Dimension::Area,
                right: Dimension::Length
            })
        );
        // the same dimension combines
        assert_eq!(
            length
                .checked_add(Quantity::from_si(Dimension::Length, 2.0))
                .map(|q| q.si()),
            Ok(5.0)
        );
        assert!(!length.same_dimension(area));
    }

    #[test]
    fn units_convert_to_si_and_back() {
        let mm = Quantity::of(*by_symbol("mm").unwrap(), 2540.0);
        let m = Quantity::of(*by_symbol("m").unwrap(), 2.54);
        assert!((mm.si() - m.si()).abs() < 1e-9);
        assert!((mm.value_in(*by_symbol("m").unwrap()).unwrap() - 2.54).abs() < 1e-9);
        // The canonical display unit of a dimension is the SI one; a client that
        // wants another unit (a duct width in millimetres) asks for it by name.
        assert!((mm.value() - 2.54).abs() < 1e-9, "a length shows in metres");
        assert!((mm.value_in(*by_symbol("mm").unwrap()).unwrap() - 2540.0).abs() < 1e-9);
    }

    #[test]
    fn text_is_read_and_written_in_the_canonical_unit() {
        let q = Quantity::parse("6 m/s").unwrap();
        assert_eq!(q.dimension(), Dimension::Velocity);
        assert!((q.si() - 6.0).abs() < 1e-9);
        assert_eq!(Quantity::parse("212 °F").unwrap().si(), 373.15);
        assert_eq!(
            Quantity::parse("90°").unwrap().si(),
            std::f64::consts::PI / 2.0
        );
        assert_eq!(Quantity::parse("1.5e3 mm").unwrap().si(), 1.5);
        assert_eq!(
            Quantity::parse("bogus"),
            Err(ParseError::NoNumber("bogus".into()))
        );
        assert_eq!(
            Quantity::parse("6 furlong").unwrap_err(),
            ParseError::UnknownUnit("furlong".into())
        );
        assert_eq!(Quantity::parse("  "), Err(ParseError::Empty));
    }

    #[test]
    fn the_wire_form_is_value_and_unit() {
        let q = Quantity::parse("6 m/s").unwrap();
        let json = serde_json::to_string(&q).unwrap();
        assert_eq!(json, r#"{"value":6.0,"unit":"m/s"}"#);
        let back: Quantity = serde_json::from_str(&json).unwrap();
        assert!((back.si() - 6.0).abs() < 1e-9);
        // the canonical unit of a dimension is what is written
        assert_eq!(
            serde_json::to_string(&Quantity::from_si(Dimension::Length, 1.5)).unwrap(),
            r#"{"value":1.5,"unit":"m"}"#
        );
    }
}
