//! Where the project is on the earth.
//!
//! The model is drawn in metres from one internal origin: the project base point. The author says three things about
//! it, and nothing else is kept:
//!
//! * which map (coordinate reference system) the survey is in, by the name the surveyor gives it, and the map
//!   coordinates of the internal origin (easting, northing, height) with the direction of the model's X axis on the
//!   map (the grid angle) and a scale factor of the map projection;
//! * the direction of true north in the model's plan, which differs from grid north by the meridian convergence;
//! * the latitude, longitude and elevation of the site, for sun paths and weather.
//!
//! Revit's survey point is a point in this map; its project base point is the internal origin, so moving the
//! base point means moving the origin and changing the easting and northing here. Terrain and survey importers
//! read points in map coordinates and bring them in with [`Georef::to_local`]; the IFC export writes the same
//! numbers as an `IfcMapConversion` and an `IfcProjectedCRS`, the true north of the model context, and the
//! site's latitude and longitude. Nothing here is a standard value: the CRS name is text the author gives.
/// The map the survey is in. Only the names are kept; the projection arithmetic is the survey software's.
#[derive(Debug, Clone, PartialEq)]
pub struct Crs {
    /// the name the surveyor gives the map, for example an authority code with its number
    pub name: String,
    pub geodetic_datum: Option<String>,
    pub vertical_datum: Option<String>,
    pub map_projection: Option<String>,
    pub map_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Georef {
    pub crs: Crs,
    /// map coordinates of the project's internal origin, metres
    pub easting: f64,
    pub northing: f64,
    pub height: f64,
    /// direction of the model's X axis on the map, degrees counter-clockwise from map east
    pub grid_angle_deg: f64,
    /// scale of the map projection (map length over ground length), 1 when the survey is already on the ground
    pub scale: f64,
    /// direction of true north in the model's plan, degrees counter-clockwise from the model's Y axis
    pub true_north_deg: f64,
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
        for (v, n) in [(self.easting, "easting"), (self.northing, "northing"), (self.height, "height"), (self.grid_angle_deg, "grid angle"), (self.scale, "scale"), (self.true_north_deg, "true north"), (self.latitude, "latitude"), (self.longitude, "longitude"), (self.elevation, "elevation")] {
            if !v.is_finite() {
                return Err(GeorefError::NotFinite(n));
            }
        }
        // the largest map coordinate is a few million metres, so anything beyond a billion is a slip
        if [self.easting, self.northing, self.height, self.elevation].iter().any(|v| v.abs() > 1e9) {
            return Err(GeorefError::OutOfRange("map coordinate"));
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
        Ok(())
    }

    /// The model's X axis on the map as (abscissa, ordinate): the cosine and sine of the grid angle.
    pub fn x_axis(&self) -> (f64, f64) {
        cos_sin(self.grid_angle_deg)
    }

    /// Direction of true north in the model's plan: (x, y) of a unit vector.
    pub fn true_north(&self) -> (f64, f64) {
        let (c, s) = cos_sin(self.true_north_deg);
        (-s, c)
    }

    /// A point of the model as map coordinates (easting, northing, height).
    pub fn to_map(&self, p: [f64; 3]) -> [f64; 3] {
        let (a, b) = self.x_axis();
        [self.easting + self.scale * (a * p[0] - b * p[1]), self.northing + self.scale * (b * p[0] + a * p[1]), self.height + self.scale * p[2]]
    }

    /// A map point as model coordinates: the inverse of [`Georef::to_map`].
    pub fn to_local(&self, m: [f64; 3]) -> [f64; 3] {
        let (a, b) = self.x_axis();
        let (e, n) = ((m[0] - self.easting) / self.scale, (m[1] - self.northing) / self.scale);
        [a * e + b * n, -b * e + a * n, (m[2] - self.height) / self.scale]
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
