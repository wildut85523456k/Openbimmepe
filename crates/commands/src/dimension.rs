//! Physical dimensions carried by a [`Quantity`](super::Quantity).
#![forbid(unsafe_code)]

use std::fmt;

/// A physical dimension. Every value carried through the command API is a
/// quantity: an SI number plus one of these. Two quantities may only be added or
/// subtracted when they share a dimension; anything else is a
/// [`DimensionMismatch`](super::DimensionMismatch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// A length, in metres.
    Length,
    /// An area, in square metres.
    Area,
    /// A volume, in cubic metres.
    Volume,
    /// An angle, in radians.
    Angle,
    /// A dimensionless ratio.
    Ratio,
    /// A slope, rise over run. Dimensionless like a ratio but kept distinct so a
    /// slope cannot be silently added to another dimensionless quantity.
    Slope,
    /// A length over a time, in metres per second.
    Velocity,
    /// A volume over a time, in cubic metres per second.
    VolumeFlow,
    /// A mass over a time, in kilograms per second.
    MassFlow,
    /// A frequency, in hertz.
    Frequency,
    /// A mass, in kilograms.
    Mass,
    /// A temperature, in kelvin.
    Temperature,
    /// A pressure, in pascals.
    Pressure,
    /// A force, in newtons.
    Force,
    /// A power, in watts.
    Power,
    /// A rotational speed, in revolutions per minute.
    RotationalSpeed,
}

impl Dimension {
    /// The canonical display unit of this dimension.
    pub fn unit(self) -> &'static str {
        match self {
            Self::Length => "m",
            Self::Area => "m²",
            Self::Volume => "m³",
            Self::Angle => "rad",
            Self::Ratio => "ratio",
            Self::Slope => "slope",
            Self::Velocity => "m/s",
            Self::VolumeFlow => "m³/s",
            Self::MassFlow => "kg/s",
            Self::Frequency => "Hz",
            Self::Mass => "kg",
            Self::Temperature => "°C",
            Self::Pressure => "Pa",
            Self::Force => "N",
            Self::Power => "W",
            Self::RotationalSpeed => "rpm",
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.unit())
    }
}
