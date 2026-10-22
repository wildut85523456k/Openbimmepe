//! Stable parameter IDs with English and French labels, and their IFC mapping.
#![forbid(unsafe_code)]

use super::{canonical, Dimension, Unit};
use std::fmt;

/// A stable parameter identity, e.g. `duct.width`.
///
/// The ID never changes when a label is renamed, so a saved graph or script keeps
/// working (04-visual-automation.md, rule 8). A label is accepted in its place,
/// with a warning, like Dynamo's get-parameter-by-name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParameterId(&'static str);

impl ParameterId {
    pub const fn new(id: &'static str) -> ParameterId {
        ParameterId(id)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl AsRef<str> for ParameterId {
    fn as_ref(&self) -> &str {
        self.0
    }
}

/// Where a parameter is written in IFC: a property set and a property of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IfcRef {
    pub pset: &'static str,
    pub property: &'static str,
}

/// One parameter: stable ID, English and French labels, dimension, unit and IFC
/// property-set group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameter {
    pub id: ParameterId,
    pub en: &'static str,
    pub fr: &'static str,
    pub dimension: Dimension,
    /// The unit the parameter is shown in.
    pub unit: Unit,
    /// The shared-parameter group that names the IFC property set this parameter is
    /// written to. The project's pset map (`crates/ifc/src/pset_map.rs`) resolves the
    /// group to a concrete IFC property set and property; empty when no mapping is
    /// agreed yet, which is the state the pset map ships in.
    pub ifc_group: &'static str,
    /// Default value, in SI.
    pub default: Option<f64>,
    /// Lower bound, in SI.
    pub min: Option<f64>,
    /// Upper bound, in SI.
    pub max: Option<f64>,
}

impl Parameter {
    /// A parameter shown in its dimension's canonical unit.
    pub const fn new(
        id: ParameterId,
        en: &'static str,
        fr: &'static str,
        dimension: Dimension,
        ifc_group: &'static str,
    ) -> Parameter {
        Parameter {
            id,
            en,
            fr,
            dimension,
            unit: canonical(dimension),
            ifc_group,
            default: None,
            min: None,
            max: None,
        }
    }

    /// A parameter shown in a specific unit.
    pub const fn with_unit(
        id: ParameterId,
        en: &'static str,
        fr: &'static str,
        unit: Unit,
        ifc_group: &'static str,
    ) -> Parameter {
        Parameter {
            id,
            en,
            fr,
            dimension: unit.dimension,
            unit,
            ifc_group,
            default: None,
            min: None,
            max: None,
        }
    }

    /// The IFC property set and property this parameter is written to, resolved from
    /// `map`: pairs of (group, property set). `None` when the parameter's group has
    /// no agreed property set.
    pub fn ifc_target(&self, map: &[(&'static str, &'static str)]) -> Option<IfcRef> {
        let pset = map
            .iter()
            .find(|(group, _)| *group == self.ifc_group)
            .map(|(_, pset)| *pset)?;
        Some(IfcRef {
            pset,
            property: self.en,
        })
    }
}

/// How a parameter was named: by its stable ID, or by a label (accepted, with a
/// warning).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolve<'a> {
    /// The stable ID was given.
    Id(&'a Parameter),
    /// A label was given; the parameter it names is returned. Callers report a
    /// warning, like Dynamo's get-parameter-by-name.
    ByLabel {
        parameter: &'a Parameter,
        label: &'a str,
    },
}

impl<'a> Resolve<'a> {
    /// The parameter this name resolved to, whichever way it was matched.
    pub fn parameter(&self) -> &'a Parameter {
        match self {
            Self::Id(p) | Self::ByLabel { parameter: p, .. } => p,
        }
    }
}

/// The name is not a parameter ID or a known label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveError {
    pub given: String,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no parameter with ID or label {:?}", self.given)
    }
}

impl std::error::Error for ResolveError {}

/// The parameters the app knows: a stable ID, English and French labels, dimension,
/// unit and IFC group for each.
pub struct ParameterRegistry {
    params: &'static [Parameter],
}

impl ParameterRegistry {
    pub const fn new(params: &'static [Parameter]) -> ParameterRegistry {
        ParameterRegistry { params }
    }

    /// The parameters of the built-in set.
    pub fn builtin() -> ParameterRegistry {
        ParameterRegistry::new(BUILTIN_PARAMETERS)
    }

    /// The parameter with this stable ID, if there is one.
    pub fn find(&self, id: &str) -> Option<&'static Parameter> {
        self.params.iter().find(|p| p.id.as_str() == id)
    }

    /// The parameter named by an ID or by an English or French label. A label is
    /// accepted with a [`Resolve::ByLabel`] result; an unknown name is an error.
    pub fn resolve(&self, given: &str) -> Result<Resolve<'static>, ResolveError> {
        if let Some(p) = self.find(given) {
            return Ok(Resolve::Id(p));
        }
        if let Some(p) = self.params.iter().find(|p| p.en == given) {
            return Ok(Resolve::ByLabel {
                parameter: p,
                label: p.en,
            });
        }
        if let Some(p) = self.params.iter().find(|p| p.fr == given) {
            return Ok(Resolve::ByLabel {
                parameter: p,
                label: p.fr,
            });
        }
        Err(ResolveError {
            given: given.to_string(),
        })
    }

    /// Every registered parameter.
    pub fn all(&self) -> &'static [Parameter] {
        self.params
    }
}

/// The built-in parameter set. The IDs and labels are stable; the IFC groups are
/// resolved through the project's pset map, which ships without mappings.
pub static BUILTIN_PARAMETERS: &[Parameter] = &[
    Parameter::new(
        ParameterId::new("element.width"),
        "Width",
        "Largeur",
        Dimension::Length,
        "",
    ),
    Parameter::new(
        ParameterId::new("element.height"),
        "Height",
        "Hauteur",
        Dimension::Length,
        "",
    ),
    Parameter::new(
        ParameterId::new("element.elevation"),
        "Elevation",
        "Élévation",
        Dimension::Length,
        "",
    ),
    Parameter::new(
        ParameterId::new("element.slope"),
        "Slope",
        "Pente",
        Dimension::Slope,
        "",
    ),
    Parameter::new(
        ParameterId::new("element.spacing"),
        "Spacing",
        "Espacement",
        Dimension::Length,
        "",
    ),
    Parameter::with_unit(
        ParameterId::new("element.angle"),
        "Angle",
        "Angle",
        Unit::linear("deg", Dimension::Angle, std::f64::consts::PI / 180.0),
        "",
    ),
    Parameter::new(
        ParameterId::new("wall.thickness"),
        "Thickness",
        "Épaisseur",
        Dimension::Length,
        "Pset_WallCommon",
    ),
    Parameter::with_unit(
        ParameterId::new("duct.width"),
        "Width",
        "Largeur",
        Unit::linear("mm", Dimension::Length, 0.001),
        "",
    ),
    Parameter::with_unit(
        ParameterId::new("duct.height"),
        "Height",
        "Hauteur",
        Unit::linear("mm", Dimension::Length, 0.001),
        "",
    ),
    Parameter::with_unit(
        ParameterId::new("duct.diameter"),
        "Diameter",
        "Diamètre",
        Unit::linear("mm", Dimension::Length, 0.001),
        "",
    ),
    Parameter::new(
        ParameterId::new("duct.velocity"),
        "Air velocity",
        "Vitesse d'air",
        Dimension::Velocity,
        "",
    ),
    Parameter::new(
        ParameterId::new("duct.flow"),
        "Air flow",
        "Débit d'air",
        Dimension::VolumeFlow,
        "",
    ),
    Parameter::new(
        ParameterId::new("duct.pressure"),
        "Pressure",
        "Pression",
        Dimension::Pressure,
        "",
    ),
    Parameter::new(
        ParameterId::new("duct.working_pressure"),
        "Working pressure",
        "Pression de service",
        Dimension::Pressure,
        "",
    ),
    Parameter::with_unit(
        ParameterId::new("pipe.diameter"),
        "Diameter",
        "Diamètre",
        Unit::linear("mm", Dimension::Length, 0.001),
        "",
    ),
    Parameter::new(
        ParameterId::new("pipe.flow"),
        "Flow",
        "Débit",
        Dimension::VolumeFlow,
        "",
    ),
];

/// The built-in parameter registry.
pub fn builtin() -> ParameterRegistry {
    ParameterRegistry::builtin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_resolves_to_the_stable_id_with_a_warning() {
        let r = builtin();
        // by ID: exact, no warning
        match r.resolve("duct.width").unwrap() {
            Resolve::Id(p) => assert_eq!(p.id.as_str(), "duct.width"),
            other => panic!("expected an exact ID match, got {other:?}"),
        }
        // by English label: accepted, with a warning
        match r.resolve("Width").unwrap() {
            Resolve::ByLabel { parameter, label } => {
                assert_eq!(parameter.id.as_str(), "element.width");
                assert_eq!(label, "Width");
            }
            other => panic!("expected a label match, got {other:?}"),
        }
        // by French label: accepted, with a warning
        match r.resolve("Largeur").unwrap() {
            Resolve::ByLabel { parameter, .. } => {
                assert_eq!(parameter.id.as_str(), "element.width")
            }
            other => panic!("expected a French label match, got {other:?}"),
        }
        // an unknown name is refused
        assert!(r.resolve("nope").is_err());
    }

    #[test]
    fn ids_are_stable_and_unique_and_dimensions_are_set() {
        let r = builtin();
        let mut ids = std::collections::BTreeSet::new();
        for p in r.all() {
            assert!(ids.insert(p.id.as_str()), "duplicate parameter ID {}", p.id);
            assert!(
                !p.en.is_empty() && !p.fr.is_empty(),
                "{} needs both labels",
                p.id
            );
            assert_eq!(
                p.unit.dimension, p.dimension,
                "{} unit does not match its dimension",
                p.id
            );
        }
        assert_eq!(
            r.find("element.width").unwrap().dimension,
            Dimension::Length
        );
        assert_eq!(
            r.find("duct.velocity").unwrap().dimension,
            Dimension::Velocity
        );
    }

    #[test]
    fn an_ifc_group_resolves_through_the_project_map() {
        let r = builtin();
        let wall = r.find("wall.thickness").unwrap();
        assert_eq!(
            wall.ifc_target(&[("Pset_WallCommon", "Pset_WallCommon")]),
            Some(IfcRef {
                pset: "Pset_WallCommon",
                property: "Thickness"
            })
        );
        assert_eq!(wall.ifc_target(&[]), None, "no map: no IFC target");
    }
}
