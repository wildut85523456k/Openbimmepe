//! Where the project is on the earth, the way Revit keeps it.
//!
//! Revit has three points and one coordinate system. This follows it:
//!
//! * the **internal origin**: the (0, 0, 0) of the model's own coordinates. It never moves.
//! * the **project base point**: a point of the model, the reference the building is dimensioned from. Levels can be
//!   measured from it. The angle between project north (the model's Y axis) and true north is kept with it.
//! * the **survey point**: another point of the model, the one the surveyor's coordinates are given for (a boundary
//!   marker, say). The **shared coordinates** are the surveyor's system: a point of the model has shared coordinates
//!   (easting, northing, elevation) that follow from where the survey point is, what its shared coordinates are, and
//!   the angle to true north.
//!
//! The author gives: the map by name; the survey point (where it is in the model and its shared coordinates);
//! the project base point (where it is in the model); the angle to true north; the latitude, longitude and elevation
//! of the site; and the coordinate base for export. The export writes an `IfcProjectedCRS` and `IfcMapConversion`,
//! the true north of the model context, and the site's latitude, longitude and elevation. Like Revit's exporter, it
//! writes the model relative to the chosen coordinate base: the internal origin, the project base point, the survey
//! point or the shared coordinates (the model then sits in the surveyor's system, rotated to it).
//!
//! The sign of the angle is this program's own: positive when project north is turned counter-clockwise from true
//! north. It has to be checked against Revit's before files are exchanged. Nothing here is a standard value: the map's
//! name is text the author gives, and no projection arithmetic is done.
#[derive(Debug, Clone, PartialEq)]
pub struct Crs {
    /// the name the surveyor gives the map, for example an authority code with its number
    pub name: String,
    pub geodetic_datum: Option<String>,
    pub vertical_datum: Option<String>,
    pub map_projection: Option<String>,
    pub map_zone: Option<String>,
}

/// What the exported file's own origin is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateBase {
    InternalOrigin,
    ProjectBasePoint,
    SurveyPoint,
    /// the model is written in the shared coordinates, turned to true north
    SharedCoordinates,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Georef {
    pub crs: Crs,
    /// the project base point, in the model's coordinates
    pub project_base_point: [f64; 3],
    /// the survey point, in the model's coordinates
    pub survey_point: [f64; 3],
    /// the shared coordinates of the survey point: easting, northing, elevation
    pub survey_shared: [f64; 3],
    /// degrees, project north turned counter-clockwise from true north
    pub angle_to_true_north_deg: f64,
    /// scale of the map projection (map length over ground length); 1 when the survey is on the ground
    pub scale: f64,
    pub coordinate_base: CoordinateBase,
    /// decimal degrees, north positive
    pub latitude: f64,
    /// decimal degrees, east positive
    pub longitude: f64,
    /// elevation of the site above the vertical datum, metres
    pub elevation: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeorefError {
    NoCrsName,
    NotFinite(&'static str),
    OutOfRange(&'static str),
    NotInvertible,
}

impl Georef {
    pub fn check(&self) -> Result<(), GeorefError> {
        if self.crs.name.trim().is_empty() {
            return Err(GeorefError::NoCrsName);
        }
        let scalars = [(self.angle_to_true_north_deg, "angle to true north"), (self.scale, "scale"), (self.latitude, "latitude"), (self.longitude, "longitude"), (self.elevation, "elevation")];
        for (v, n) in scalars {
            if !v.is_finite() {
                return Err(GeorefError::NotFinite(n));
            }
        }
        for (arr, n) in [(self.project_base_point, "project base point"), (self.survey_point, "survey point"), (self.survey_shared, "survey point shared coordinates")] {
            if arr.iter().any(|v| !v.is_finite()) {
                return Err(GeorefError::NotFinite(n));
            }
            // the largest map coordinate is a few million metres, so anything beyond a billion is a slip
            if arr.iter().any(|v| v.abs() > 1e9) {
                return Err(GeorefError::OutOfRange(n));
            }
        }
        if self.elevation.abs() > 1e9 {
            return Err(GeorefError::OutOfRange("elevation"));
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(GeorefError::OutOfRange("latitude"));
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(GeorefError::OutOfRange("longitude"));
        }
        if self.scale <= 0.0 || self.scale > 1e3 {
            return Err(GeorefError::NotInvertible);
        }
        // the model is moved rigidly into the shared coordinates, so a map scale has nowhere to go
        if self.coordinate_base == CoordinateBase::SharedCoordinates && self.scale != 1.0 {
            return Err(GeorefError::OutOfRange("scale with the shared coordinates as base"));
        }
        Ok(())
    }

    /// The model's X axis on the map as (abscissa, ordinate): the cosine and sine of the angle it makes with map east.
    /// Project north is turned by the angle from true north, and so is the X axis from east.
    pub fn x_axis(&self) -> (f64, f64) {
        cos_sin(self.angle_to_true_north_deg)
    }

    /// Direction of true north in the model's plan: (x, y) of a unit vector.
    pub fn true_north(&self) -> (f64, f64) {
        let (c, s) = cos_sin(self.angle_to_true_north_deg);
        // map north (0, 1) turned back by the angle
        (s, c)
    }

    /// Shared coordinates of the model's internal origin.
    pub fn origin_shared(&self) -> [f64; 3] {
        let (a, b) = self.x_axis();
        let p = self.survey_point;
        [
            self.survey_shared[0] - self.scale * (a * p[0] - b * p[1]),
            self.survey_shared[1] - self.scale * (b * p[0] + a * p[1]),
            self.survey_shared[2] - self.scale * p[2],
        ]
    }

    /// A point of the model as shared coordinates (easting, northing, elevation).
    pub fn to_shared(&self, p: [f64; 3]) -> [f64; 3] {
        let (a, b) = self.x_axis();
        let o = self.origin_shared();
        [o[0] + self.scale * (a * p[0] - b * p[1]), o[1] + self.scale * (b * p[0] + a * p[1]), o[2] + self.scale * p[2]]
    }

    /// Shared coordinates as a point of the model: the inverse of [`Georef::to_shared`].
    pub fn to_model(&self, m: [f64; 3]) -> [f64; 3] {
        let (a, b) = self.x_axis();
        let o = self.origin_shared();
        let (e, n) = ((m[0] - o[0]) / self.scale, (m[1] - o[1]) / self.scale);
        [a * e + b * n, -b * e + a * n, (m[2] - o[2]) / self.scale]
    }

    /// The shared coordinates of the project base point (what Revit shows in its properties).
    pub fn project_base_point_shared(&self) -> [f64; 3] {
        self.to_shared(self.project_base_point)
    }

    /// Where the file's own origin is in the model, and the turn of its axes from the model's: the placement of the
    /// site (which carries the whole model) is the opposite of it.
    /// Returns (origin in model coordinates, X-axis angle of the file against the model's X axis in degrees).
    pub fn base(&self) -> ([f64; 3], f64) {
        match self.coordinate_base {
            CoordinateBase::InternalOrigin => ([0.0; 3], 0.0),
            CoordinateBase::ProjectBasePoint => (self.project_base_point, 0.0),
            CoordinateBase::SurveyPoint => (self.survey_point, 0.0),
            CoordinateBase::SharedCoordinates => ([0.0; 3], -self.angle_to_true_north_deg),
        }
    }

    /// What the `IfcMapConversion` says: the map coordinates of the file's origin and the direction of the file's X axis on the map.
    /// With the shared coordinates as base, the file is already in the map's frame and the conversion is the identity.
    pub fn map_conversion(&self) -> ([f64; 3], (f64, f64)) {
        match self.coordinate_base {
            CoordinateBase::SharedCoordinates => ([0.0; 3], (1.0, 0.0)),
            _ => (self.to_shared(self.base().0), self.x_axis()),
        }
    }

    /// The true north in the file's plan.
    pub fn file_true_north(&self) -> (f64, f64) {
        match self.coordinate_base {
            CoordinateBase::SharedCoordinates => (0.0, 1.0),
            _ => self.true_north(),
        }
    }

    /// Where a model point lies in the exported file (before the scale of the map is applied).
    pub fn to_file(&self, p: [f64; 3]) -> [f64; 3] {
        match self.coordinate_base {
            CoordinateBase::SharedCoordinates => {
                let (a, b) = self.x_axis();
                let o = self.origin_shared();
                [o[0] + a * p[0] - b * p[1], o[1] + b * p[0] + a * p[1], o[2] + p[2]]
            }
            _ => {
                let o = self.base().0;
                [p[0] - o[0], p[1] - o[1], p[2] - o[2]]
            }
        }
    }
}

/// Decimal degrees as the four whole numbers an `IfcCompoundPlaneAngleMeasure` holds: degrees, minutes, seconds and
/// millionths of a second, all carrying the sign.
pub fn compound_angle(deg: f64) -> [i64; 4] {
    let sign = if deg < 0.0 { -1 } else { 1 };
    // work in whole millionths of a second so that nothing carries by accident
    let total = (deg.abs() * 3600.0 * 1e6).round() as i64;
    let micro = total % 1_000_000;
    let secs = total / 1_000_000;
    [sign * (secs / 3600), sign * ((secs / 60) % 60), sign * (secs % 60), sign * micro]
}

/// Cosine and sine of an angle in degrees, exact at the quarter turns (where the radian arithmetic leaves 1e-16).
fn cos_sin(deg: f64) -> (f64, f64) {
    let d = deg.rem_euclid(360.0);
    if d == 0.0 {
        (1.0, 0.0)
    } else if d == 90.0 {
        (0.0, 1.0)
    } else if d == 180.0 {
        (-1.0, 0.0)
    } else if d == 270.0 {
        (0.0, -1.0)
    } else {
        let a = deg.to_radians();
        (a.cos(), a.sin())
    }
}
