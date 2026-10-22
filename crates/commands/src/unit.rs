//! Units and their conversion to and from SI.
#![forbid(unsafe_code)]

use super::Dimension;

/// One unit of a dimension, with the scale and offset that convert it to SI:
///
/// `to_si(x) = scale * x + offset`, and its inverse `(si - offset) / scale`.
///
/// Only temperature is affine (°C, °F); every other unit is linear (offset 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    pub symbol: &'static str,
    pub dimension: Dimension,
    pub scale: f64,
    pub offset: f64,
}

impl Unit {
    /// A linear unit: `to_si(x) = scale * x`.
    pub const fn linear(symbol: &'static str, dimension: Dimension, scale: f64) -> Unit {
        Unit {
            symbol,
            dimension,
            scale,
            offset: 0.0,
        }
    }

    /// An affine unit: `to_si(x) = scale * x + offset`.
    pub const fn affine(
        symbol: &'static str,
        dimension: Dimension,
        scale: f64,
        offset: f64,
    ) -> Unit {
        Unit {
            symbol,
            dimension,
            scale,
            offset,
        }
    }

    /// The value in SI of `value` expressed in this unit.
    pub fn to_si(self, value: f64) -> f64 {
        self.scale * value + self.offset
    }

    /// The value in this unit of the SI quantity `si`.
    pub fn from_si(self, si: f64) -> f64 {
        (si - self.offset) / self.scale
    }
}

/// The units the API knows, by symbol.
pub static UNITS: &[Unit] = &[
    // length
    Unit::linear("m", Dimension::Length, 1.0),
    Unit::linear("mm", Dimension::Length, 0.001),
    Unit::linear("cm", Dimension::Length, 0.01),
    Unit::linear("km", Dimension::Length, 1000.0),
    Unit::linear("in", Dimension::Length, 0.0254),
    Unit::linear("ft", Dimension::Length, 0.3048),
    // area
    Unit::linear("m²", Dimension::Area, 1.0),
    Unit::linear("mm²", Dimension::Area, 1e-6),
    Unit::linear("cm²", Dimension::Area, 1e-4),
    Unit::linear("ft²", Dimension::Area, 0.092_903_04),
    // volume
    Unit::linear("m³", Dimension::Volume, 1.0),
    Unit::linear("L", Dimension::Volume, 0.001),
    // angle
    Unit::linear("rad", Dimension::Angle, 1.0),
    Unit::linear("deg", Dimension::Angle, std::f64::consts::PI / 180.0),
    // dimensionless
    Unit::linear("ratio", Dimension::Ratio, 1.0),
    Unit::linear("slope", Dimension::Slope, 1.0),
    // velocity
    Unit::linear("m/s", Dimension::Velocity, 1.0),
    Unit::linear("km/h", Dimension::Velocity, 1000.0 / 3600.0),
    // volume flow
    Unit::linear("m³/s", Dimension::VolumeFlow, 1.0),
    Unit::linear("m³/h", Dimension::VolumeFlow, 1.0 / 3600.0),
    Unit::linear("L/s", Dimension::VolumeFlow, 0.001),
    // mass flow
    Unit::linear("kg/s", Dimension::MassFlow, 1.0),
    // frequency
    Unit::linear("Hz", Dimension::Frequency, 1.0),
    // mass
    Unit::linear("kg", Dimension::Mass, 1.0),
    Unit::linear("g", Dimension::Mass, 0.001),
    // temperature (affine)
    Unit::linear("K", Dimension::Temperature, 1.0),
    Unit::affine("°C", Dimension::Temperature, 1.0, 273.15),
    Unit::affine(
        "°F",
        Dimension::Temperature,
        5.0 / 9.0,
        273.15 - 32.0 * 5.0 / 9.0,
    ),
    // pressure
    Unit::linear("Pa", Dimension::Pressure, 1.0),
    Unit::linear("kPa", Dimension::Pressure, 1000.0),
    Unit::linear("bar", Dimension::Pressure, 100_000.0),
    // force
    Unit::linear("N", Dimension::Force, 1.0),
    Unit::linear("kN", Dimension::Force, 1000.0),
    // power
    Unit::linear("W", Dimension::Power, 1.0),
    Unit::linear("kW", Dimension::Power, 1000.0),
    // rotational speed
    Unit::linear("rpm", Dimension::RotationalSpeed, 1.0),
    Unit::linear("rps", Dimension::RotationalSpeed, 60.0),
];

/// The unit with this symbol, if the API knows it.
pub fn by_symbol(symbol: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|u| u.symbol == symbol)
}

/// The canonical display unit of a dimension.
pub const fn canonical(dimension: Dimension) -> Unit {
    match dimension {
        Dimension::Length => Unit::linear("m", Dimension::Length, 1.0),
        Dimension::Area => Unit::linear("m²", Dimension::Area, 1.0),
        Dimension::Volume => Unit::linear("m³", Dimension::Volume, 1.0),
        Dimension::Angle => Unit::linear("rad", Dimension::Angle, 1.0),
        Dimension::Ratio => Unit::linear("ratio", Dimension::Ratio, 1.0),
        Dimension::Slope => Unit::linear("slope", Dimension::Slope, 1.0),
        Dimension::Velocity => Unit::linear("m/s", Dimension::Velocity, 1.0),
        Dimension::VolumeFlow => Unit::linear("m³/s", Dimension::VolumeFlow, 1.0),
        Dimension::MassFlow => Unit::linear("kg/s", Dimension::MassFlow, 1.0),
        Dimension::Frequency => Unit::linear("Hz", Dimension::Frequency, 1.0),
        Dimension::Mass => Unit::linear("kg", Dimension::Mass, 1.0),
        Dimension::Temperature => Unit::affine("°C", Dimension::Temperature, 1.0, 273.15),
        Dimension::Pressure => Unit::linear("Pa", Dimension::Pressure, 1.0),
        Dimension::Force => Unit::linear("N", Dimension::Force, 1.0),
        Dimension::Power => Unit::linear("W", Dimension::Power, 1.0),
        Dimension::RotationalSpeed => Unit::linear("rpm", Dimension::RotationalSpeed, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_units_convert_through_kelvin() {
        let c = by_symbol("°C").unwrap();
        let f = by_symbol("°F").unwrap();
        let k = by_symbol("K").unwrap();
        // 100 °C boils, 212 °F, 373.15 K
        assert!((c.to_si(100.0) - 373.15).abs() < 1e-9);
        assert!((f.to_si(212.0) - 373.15).abs() < 1e-9);
        assert!((k.to_si(373.15) - 373.15).abs() < 1e-9);
        assert!((c.from_si(373.15) - 100.0).abs() < 1e-9);
        assert!((f.from_si(373.15) - 212.0).abs() < 1e-9);
    }

    #[test]
    fn every_dimension_names_a_unit_and_the_symbol_round_trips() {
        for d in [
            Dimension::Length,
            Dimension::Area,
            Dimension::Volume,
            Dimension::Angle,
            Dimension::Ratio,
            Dimension::Slope,
            Dimension::Velocity,
            Dimension::VolumeFlow,
            Dimension::MassFlow,
            Dimension::Frequency,
            Dimension::Mass,
            Dimension::Temperature,
            Dimension::Pressure,
            Dimension::Force,
            Dimension::Power,
            Dimension::RotationalSpeed,
        ] {
            let u = canonical(d);
            assert_eq!(u.dimension, d, "{d} names a unit of another dimension");
            assert_eq!(by_symbol(u.symbol), Some(&u), "{d} unit symbol not unique");
            let back = u.from_si(u.to_si(1.5));
            assert!((back - 1.5).abs() < 1e-9, "{d} does not round-trip: {back}");
        }
    }
}
