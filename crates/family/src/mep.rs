//! S2f: family category, hosting, classification, MEP connectors and 2D plan symbols.
//!
//! Units follow the rest of the engine: lengths in metres. Connector flow is m3/s for ducts and
//! pipes and watts for electrical connectors; pressure drop is in pascals; voltage in volts.
//! No standard values are shipped here: every size, flow and code comes from the family author.
use crate::expr::{cos_deg, sin_deg, Expr};
use crate::FamilyError;

pub const MAX_CONNECTORS: usize = 64;
pub const MAX_PLAN: usize = 200;
/// Most connectors in one placement, nested families included.
pub const MAX_CONNECTORS_TOTAL: usize = 2000;
/// Most plan symbol shapes in one placement, nested families included.
pub const MAX_PLAN_TOTAL: usize = 5000;
pub const MAX_CLASSIFICATIONS: usize = 8;
/// Largest connector size or plan length accepted (metres).
pub const MAX_DIM: f64 = 100.0;

/// What the family is. Decides the IFC class and the default hosting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    #[default]
    Generic,
    Door,
    Window,
    Furniture,
    AirTerminal,
    Fan,
    Damper,
    Pump,
    Boiler,
    Chiller,
    Coil,
    UnitaryEquipment,
    Tank,
    Valve,
    SanitaryTerminal,
    LightFixture,
    DistributionBoard,
    Outlet,
    SwitchingDevice,
    Sprinkler,
    DuctFitting,
    PipeFitting,
    CableFitting,
}

impl Category {
    /// The IFC4 entity name used on export.
    pub fn ifc_class(self) -> &'static str {
        use Category::*;
        match self {
            Generic => "IfcBuildingElementProxy",
            Door => "IfcDoor",
            Window => "IfcWindow",
            Furniture => "IfcFurniture",
            AirTerminal => "IfcAirTerminal",
            Fan => "IfcFan",
            Damper => "IfcDamper",
            Pump => "IfcPump",
            Boiler => "IfcBoiler",
            Chiller => "IfcChiller",
            Coil => "IfcCoil",
            UnitaryEquipment => "IfcUnitaryEquipment",
            Tank => "IfcTank",
            Valve => "IfcValve",
            SanitaryTerminal => "IfcSanitaryTerminal",
            LightFixture => "IfcLightFixture",
            DistributionBoard => "IfcElectricDistributionBoard",
            Outlet => "IfcOutlet",
            SwitchingDevice => "IfcSwitchingDevice",
            Sprinkler => "IfcFireSuppressionTerminal",
            DuctFitting => "IfcDuctFitting",
            PipeFitting => "IfcPipeFitting",
            CableFitting => "IfcCableCarrierFitting",
        }
    }
    /// Where a family of this category goes when the author does not say.
    pub fn default_host(self) -> Host {
        use Category::*;
        match self {
            Door | Window => Host::Wall,
            AirTerminal | LightFixture | Sprinkler => Host::Ceiling,
            Outlet | SwitchingDevice => Host::Wall,
            _ => Host::Free,
        }
    }
    /// All categories, for menus and tests.
    pub const ALL: [Category; 23] = {
        use Category::*;
        [Generic, Door, Window, Furniture, AirTerminal, Fan, Damper, Pump, Boiler, Chiller, Coil, UnitaryEquipment, Tank, Valve, SanitaryTerminal, LightFixture, DistributionBoard, Outlet, SwitchingDevice, Sprinkler, DuctFitting, PipeFitting, CableFitting]
    };
}

/// What a family is placed on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Host {
    #[default]
    Free,
    Wall,
    Floor,
    Ceiling,
    /// any surface of another element
    Face,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Duct,
    Pipe,
    CableTray,
    Conduit,
    Electrical,
}

impl Domain {
    /// IfcDistributionPortTypeEnum value.
    pub fn ifc_port_type(self) -> &'static str {
        match self {
            Domain::Duct => "DUCT",
            Domain::Pipe => "PIPE",
            Domain::CableTray | Domain::Conduit => "CABLECARRIER",
            Domain::Electrical => "CABLE",
        }
    }
}

/// The system a connector belongs to. Two connectors join only if their systems match (or one is Other).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    SupplyAir,
    ReturnAir,
    ExhaustAir,
    OutsideAir,
    ChilledWaterSupply,
    ChilledWaterReturn,
    HeatingWaterSupply,
    HeatingWaterReturn,
    CondenserWaterSupply,
    CondenserWaterReturn,
    DomesticCold,
    DomesticHot,
    Sanitary,
    Vent,
    StormWater,
    Condensate,
    FireProtection,
    Gas,
    Power,
    Data,
    Other,
}

impl System {
    pub fn domain_ok(self, d: Domain) -> bool {
        use System::*;
        match self {
            Other => true,
            SupplyAir | ReturnAir | ExhaustAir | OutsideAir => d == Domain::Duct,
            Power | Data => matches!(d, Domain::Electrical | Domain::CableTray | Domain::Conduit),
            _ => d == Domain::Pipe,
        }
    }
    /// IfcDistributionSystemEnum value.
    pub fn ifc_system(self) -> &'static str {
        use System::*;
        match self {
            SupplyAir | ReturnAir | OutsideAir => "VENTILATION",
            ExhaustAir => "EXHAUST",
            ChilledWaterSupply | ChilledWaterReturn => "CHILLEDWATER",
            HeatingWaterSupply | HeatingWaterReturn => "HEATING",
            CondenserWaterSupply | CondenserWaterReturn => "CONDENSERWATER",
            DomesticCold => "DOMESTICCOLDWATER",
            DomesticHot => "DOMESTICHOTWATER",
            Sanitary => "WASTEWATER",
            Vent => "VENT",
            StormWater => "STORMWATER",
            Condensate => "DRAINAGE",
            FireProtection => "FIREPROTECTION",
            Gas => "GAS",
            Power => "ELECTRICAL",
            Data => "DATA",
            Other => "NOTDEFINED",
        }
    }
    pub fn compatible(self, o: System) -> bool {
        self == o || self == System::Other || o == System::Other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowDir {
    In,
    Out,
    Both,
}

impl FlowDir {
    /// IfcFlowDirectionEnum value.
    pub fn ifc(self) -> &'static str {
        match self {
            FlowDir::In => "SINK",
            FlowDir::Out => "SOURCE",
            FlowDir::Both => "SOURCEANDSINK",
        }
    }
    fn joins(self, o: FlowDir) -> bool {
        !matches!((self, o), (FlowDir::In, FlowDir::In) | (FlowDir::Out, FlowDir::Out))
    }
}

/// The way a connector faces, in the family's own axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    PlusX,
    MinusX,
    PlusY,
    MinusY,
    PlusZ,
    MinusZ,
}

impl Dir {
    pub fn vec(self) -> [f64; 3] {
        match self {
            Dir::PlusX => [1.0, 0.0, 0.0],
            Dir::MinusX => [-1.0, 0.0, 0.0],
            Dir::PlusY => [0.0, 1.0, 0.0],
            Dir::MinusY => [0.0, -1.0, 0.0],
            Dir::PlusZ => [0.0, 0.0, 1.0],
            Dir::MinusZ => [0.0, 0.0, -1.0],
        }
    }
}

/// Size of a connector as the author writes it (formulas).
#[derive(Clone, Debug)]
pub enum ShapeDef {
    Round(String),
    Rect(String, String),
    /// voltage (V) and number of poles (1 to 4)
    Electrical { voltage: String, poles: String },
}

#[derive(Clone, Debug)]
pub struct ConnectorDef {
    pub name: String,
    pub domain: Domain,
    pub system: System,
    pub flow_dir: FlowDir,
    pub at: (String, String, String),
    pub dir: Dir,
    pub shape: ShapeDef,
    /// m3/s (duct, pipe) or W (electrical)
    pub flow: Option<String>,
    /// Pa; not for electrical connectors
    pub pressure_drop: Option<String>,
    pub visible: Option<String>,
    /// azimuth and elevation in degrees; when set they replace `dir` (45 degree fittings, slopes)
    pub aim: Option<(String, String)>,
    /// turn of a rectangular connector about its own axis, degrees (0 = default up)
    pub roll: Option<String>,
    /// electrical only: power factor, more than 0 and at most 1 (default 1)
    pub power_factor: Option<String>,
}

impl ConnectorDef {
    pub fn new(name: &str, domain: Domain, system: System, flow_dir: FlowDir, at: (&str, &str, &str), dir: Dir, shape: ShapeDef) -> Self {
        ConnectorDef { name: name.into(), domain, system, flow_dir, at: (at.0.into(), at.1.into(), at.2.into()), dir, shape, flow: None, pressure_drop: None, visible: None, aim: None, roll: None, power_factor: None }
    }
    pub fn flow(mut self, f: &str) -> Self {
        self.flow = Some(f.into());
        self
    }
    pub fn pressure_drop(mut self, f: &str) -> Self {
        self.pressure_drop = Some(f.into());
        self
    }
    pub fn visible_if(mut self, f: &str) -> Self {
        self.visible = Some(f.into());
        self
    }
    /// Face along azimuth (counter-clockwise from +X in plan) and elevation (up from the horizontal), in degrees.
    pub fn aim(mut self, azimuth: &str, elevation: &str) -> Self {
        self.aim = Some((azimuth.into(), elevation.into()));
        self
    }
    pub fn roll(mut self, degrees: &str) -> Self {
        self.roll = Some(degrees.into());
        self
    }
    pub fn power_factor(mut self, f: &str) -> Self {
        self.power_factor = Some(f.into());
        self
    }
}

/// A 2D symbol drawn in plan, in the family's own x and y.
#[derive(Clone, Debug)]
pub enum PlanDef {
    Line { a: (String, String), b: (String, String), visible: Option<String> },
    Circle { c: (String, String), r: String, visible: Option<String> },
}

impl PlanDef {
    pub fn line(a: (&str, &str), b: (&str, &str)) -> Self {
        PlanDef::Line { a: (a.0.into(), a.1.into()), b: (b.0.into(), b.1.into()), visible: None }
    }
    pub fn circle(c: (&str, &str), r: &str) -> Self {
        PlanDef::Circle { c: (c.0.into(), c.1.into()), r: r.into(), visible: None }
    }
    pub fn visible_if(mut self, f: &str) -> Self {
        match &mut self {
            PlanDef::Line { visible, .. } | PlanDef::Circle { visible, .. } => *visible = Some(f.into()),
        }
        self
    }
}

// ---------- compiled ----------

pub(crate) type Parse<'a> = &'a dyn Fn(&str, &str, bool) -> Result<Expr, FamilyError>;

pub(crate) struct CConnector {
    name: String,
    domain: Domain,
    system: System,
    flow_dir: FlowDir,
    at: [Expr; 3],
    dir: Dir,
    shape: CShape,
    flow: Option<Expr>,
    pdrop: Option<Expr>,
    visible: Option<Expr>,
    aim: Option<[Expr; 2]>,
    roll: Option<Expr>,
    pf: Option<Expr>,
}
enum CShape {
    Round(Expr),
    Rect(Expr, Expr),
    Elec(Expr, Expr),
}
pub(crate) enum CPlan {
    Line([Expr; 4], Option<Expr>),
    Circle([Expr; 3], Option<Expr>),
}

fn bad(name: &str, why: &'static str) -> FamilyError {
    FamilyError::Connector { name: name.into(), why }
}

pub(crate) fn compile(conns: &[ConnectorDef], plan: &[PlanDef], parse: Parse) -> Result<(Vec<CConnector>, Vec<CPlan>), FamilyError> {
    if conns.len() > MAX_CONNECTORS {
        return Err(FamilyError::TooMany("connectors"));
    }
    if plan.len() > MAX_PLAN {
        return Err(FamilyError::TooMany("plan symbol lines"));
    }
    let mut out: Vec<CConnector> = Vec::new();
    for c in conns {
        if !crate::valid_name(&c.name) {
            return Err(FamilyError::BadName(c.name.clone()));
        }
        if out.iter().any(|o| o.name == c.name) {
            return Err(FamilyError::DuplicateName(c.name.clone()));
        }
        if !c.system.domain_ok(c.domain) {
            return Err(bad(&c.name, "the system does not belong to this kind of connector"));
        }
        let lbl = |p: &str| format!("connector {}.{}", c.name, p);
        let p = |part: &str, s: &str| parse(&lbl(part), s, false);
        let shape = match (&c.shape, c.domain) {
            (ShapeDef::Round(d), Domain::Duct | Domain::Pipe | Domain::Conduit) => CShape::Round(p("diameter", d)?),
            (ShapeDef::Rect(w, h), Domain::Duct | Domain::CableTray) => CShape::Rect(p("width", w)?, p("height", h)?),
            (ShapeDef::Electrical { voltage, poles }, Domain::Electrical) => CShape::Elec(p("voltage", voltage)?, p("poles", poles)?),
            _ => return Err(bad(&c.name, "the shape does not fit this kind of connector")),
        };
        if c.domain == Domain::Electrical && c.pressure_drop.is_some() {
            return Err(bad(&c.name, "an electrical connector has no pressure drop"));
        }
        if c.power_factor.is_some() && c.domain != Domain::Electrical {
            return Err(bad(&c.name, "only an electrical connector has a power factor"));
        }
        if c.roll.is_some() && !matches!(c.shape, ShapeDef::Rect(..)) {
            return Err(bad(&c.name, "only a rectangular connector can be rolled"));
        }
        out.push(CConnector {
            name: c.name.clone(),
            domain: c.domain,
            system: c.system,
            flow_dir: c.flow_dir,
            at: [p("x", &c.at.0)?, p("y", &c.at.1)?, p("z", &c.at.2)?],
            dir: c.dir,
            shape,
            flow: match &c.flow { Some(f) => Some(p("flow", f)?), None => None },
            pdrop: match &c.pressure_drop { Some(f) => Some(p("pressure_drop", f)?), None => None },
            visible: match &c.visible { Some(f) => Some(p("visible", f)?), None => None },
            aim: match &c.aim { Some((a, e)) => Some([p("azimuth", a)?, p("elevation", e)?]), None => None },
            roll: match &c.roll { Some(f) => Some(p("roll", f)?), None => None },
            pf: match &c.power_factor { Some(f) => Some(p("power_factor", f)?), None => None },
        });
    }
    let mut cp = Vec::new();
    for (i, d) in plan.iter().enumerate() {
        let p = |part: &str, s: &str| parse(&format!("plan {i}.{part}"), s, false);
        let vis = |v: &Option<String>| -> Result<Option<Expr>, FamilyError> { match v { Some(f) => Ok(Some(p("visible", f)?)), None => Ok(None) } };
        cp.push(match d {
            PlanDef::Line { a, b, visible } => CPlan::Line([p("x1", &a.0)?, p("y1", &a.1)?, p("x2", &b.0)?, p("y2", &b.1)?], vis(visible)?),
            PlanDef::Circle { c, r, visible } => CPlan::Circle([p("cx", &c.0)?, p("cy", &c.1)?, p("r", r)?], vis(visible)?),
        });
    }
    Ok((out, cp))
}

// ---------- solved ----------

/// Solved size of a connector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Round { diameter: f64 },
    Rect { width: f64, height: f64 },
    Electrical { voltage: f64, poles: u8 },
}

/// A solved connector in the family's own frame, or (after `world`) in the model.
#[derive(Clone, Debug, PartialEq)]
pub struct Connector {
    pub name: String,
    pub domain: Domain,
    pub system: System,
    pub flow_dir: FlowDir,
    pub pos: [f64; 3],
    pub dir: [f64; 3],
    /// the direction of the height of a rectangular connector (unit, square to `dir`)
    pub up: [f64; 3],
    pub size: Size,
    /// m3/s, or real power in W for an electrical connector
    pub flow: f64,
    pub pressure_drop: f64,
    /// electrical only; 1 elsewhere
    pub power_factor: f64,
}

impl Connector {
    /// Apparent power in VA of an electrical connector (real power over power factor).
    pub fn apparent_power(&self) -> f64 {
        self.flow / self.power_factor
    }
    /// Line current in A: single pole S/V; two or more poles are taken as three-phase S/(sqrt(3) V) when `poles` is 3 or 4, else S/V.
    pub fn current(&self) -> Option<f64> {
        match self.size {
            Size::Electrical { voltage, poles } => {
                let s = self.apparent_power();
                Some(if poles >= 3 { s / (3f64.sqrt() * voltage) } else { s / voltage })
            }
            _ => None,
        }
    }
}

/// A 2D symbol shape in the family's plan frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanShape {
    Line([f64; 4]),
    Circle { c: [f64; 2], r: f64 },
}

fn flag(e: &Option<Expr>, vals: &[f64], label: &str) -> Result<bool, FamilyError> {
    match e {
        None => Ok(true),
        Some(e) => Ok(e.eval(vals).map_err(|err| FamilyError::Formula { name: label.into(), err })? != 0.0),
    }
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
/// Default up for a connector facing `d`: +Z, or +Y when `d` is vertical, made square to `d`; then turned by `roll` degrees about `d`.
pub fn up_for(d: [f64; 3], roll: f64) -> [f64; 3] {
    let base = if d[0].abs() < 1e-9 && d[1].abs() < 1e-9 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
    let k = base[0] * d[0] + base[1] * d[1] + base[2] * d[2];
    let u = unit([base[0] - k * d[0], base[1] - k * d[1], base[2] - k * d[2]]);
    let w = cross(d, u);
    let (s, c) = (sin_deg(roll), cos_deg(roll));
    unit([c * u[0] + s * w[0], c * u[1] + s * w[1], c * u[2] + s * w[2]])
}

pub(crate) fn solve_connectors(cs: &[CConnector], vals: &[f64]) -> Result<Vec<Connector>, FamilyError> {
    let mut out = Vec::with_capacity(cs.len());
    for c in cs {
        if !flag(&c.visible, vals, &format!("connector {}.visible", c.name))? {
            continue;
        }
        let ev = |part: &str, e: &Expr| e.eval(vals).map_err(|err| FamilyError::Formula { name: format!("connector {}.{}", c.name, part), err });
        let pos = [ev("x", &c.at[0])?, ev("y", &c.at[1])?, ev("z", &c.at[2])?];
        if pos.iter().any(|v| !v.is_finite() || v.abs() > MAX_DIM) {
            return Err(bad(&c.name, "the position is not a finite point within 100 m"));
        }
        let dim = |part: &str, e: &Expr| -> Result<f64, FamilyError> {
            let v = ev(part, e)?;
            if !(v > 0.0 && v <= MAX_DIM) {
                return Err(bad(&c.name, "a size must be more than 0 and at most 100"));
            }
            Ok(v)
        };
        let size = match &c.shape {
            CShape::Round(d) => Size::Round { diameter: dim("diameter", d)? },
            CShape::Rect(w, h) => Size::Rect { width: dim("width", w)?, height: dim("height", h)? },
            CShape::Elec(v, p) => {
                let poles = ev("poles", p)?;
                if poles.fract() != 0.0 || !(1.0..=4.0).contains(&poles) {
                    return Err(bad(&c.name, "poles must be a whole number from 1 to 4"));
                }
                let volts = ev("voltage", v)?;
                if !(volts > 0.0 && volts <= 1.0e6) {
                    return Err(bad(&c.name, "voltage must be more than 0 and at most 1,000,000"));
                }
                Size::Electrical { voltage: volts, poles: poles as u8 }
            }
        };
        let nonneg = |part: &str, e: &Option<Expr>| -> Result<f64, FamilyError> {
            match e {
                None => Ok(0.0),
                Some(e) => {
                    let v = ev(part, e)?;
                    if !(v >= 0.0 && v.is_finite()) {
                        return Err(bad(&c.name, "flow and pressure drop cannot be negative or infinite"));
                    }
                    Ok(v)
                }
            }
        };
        let dir = match &c.aim {
            None => c.dir.vec(),
            Some([a, e]) => {
                let (az, el) = (ev("azimuth", a)?, ev("elevation", e)?);
                if !(az.is_finite() && el.is_finite() && el.abs() <= 90.0) {
                    return Err(bad(&c.name, "elevation must be from -90 to 90 degrees"));
                }
                unit([cos_deg(el) * cos_deg(az), cos_deg(el) * sin_deg(az), sin_deg(el)])
            }
        };
        let roll = match &c.roll {
            None => 0.0,
            Some(r) => {
                let v = ev("roll", r)?;
                if !v.is_finite() {
                    return Err(bad(&c.name, "roll is not a finite angle"));
                }
                v
            }
        };
        let power_factor = match &c.pf {
            None => 1.0,
            Some(f) => {
                let v = ev("power_factor", f)?;
                if !(v > 0.0 && v <= 1.0) {
                    return Err(bad(&c.name, "power factor must be more than 0 and at most 1"));
                }
                v
            }
        };
        out.push(Connector {
            name: c.name.clone(),
            domain: c.domain,
            system: c.system,
            flow_dir: c.flow_dir,
            pos,
            dir,
            up: up_for(dir, roll),
            size,
            flow: nonneg("flow", &c.flow)?,
            pressure_drop: nonneg("pressure_drop", &c.pdrop)?,
            power_factor,
        });
    }
    Ok(out)
}

pub(crate) fn solve_plan(ps: &[CPlan], vals: &[f64]) -> Result<Vec<PlanShape>, FamilyError> {
    let mut out = Vec::with_capacity(ps.len());
    for (i, p) in ps.iter().enumerate() {
        let ev = |part: &str, e: &Expr| -> Result<f64, FamilyError> {
            let v = e.eval(vals).map_err(|err| FamilyError::Formula { name: format!("plan {i}.{part}"), err })?;
            if !v.is_finite() || v.abs() > MAX_DIM {
                return Err(FamilyError::Plan { index: i, why: "a coordinate is not a finite number within 100 m" });
            }
            Ok(v)
        };
        match p {
            CPlan::Line(e, vis) => {
                if !flag(vis, vals, &format!("plan {i}.visible"))? {
                    continue;
                }
                let l = [ev("x1", &e[0])?, ev("y1", &e[1])?, ev("x2", &e[2])?, ev("y2", &e[3])?];
                if l[0] == l[2] && l[1] == l[3] {
                    return Err(FamilyError::Plan { index: i, why: "a line has no length" });
                }
                out.push(PlanShape::Line(l));
            }
            CPlan::Circle(e, vis) => {
                if !flag(vis, vals, &format!("plan {i}.visible"))? {
                    continue;
                }
                let (cx, cy, r) = (ev("cx", &e[0])?, ev("cy", &e[1])?, ev("r", &e[2])?);
                if r <= 0.0 {
                    return Err(FamilyError::Plan { index: i, why: "a circle needs a radius above 0" });
                }
                out.push(PlanShape::Circle { c: [cx, cy], r });
            }
        }
    }
    Ok(out)
}

// ---------- placing and joining ----------

/// Where a family instance sits in the model: position, and turn about the vertical axis (degrees, counter-clockwise).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub angle: f64,
}

impl Connector {
    /// This connector after placing the family.
    pub fn world(&self, p: &Pose) -> Connector {
        let (s, c) = (sin_deg(p.angle), cos_deg(p.angle));
        let rot = |v: [f64; 3]| [c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]];
        let r = rot(self.pos);
        let mut out = self.clone();
        out.pos = [r[0] + p.x, r[1] + p.y, r[2] + p.z];
        out.dir = rot(self.dir);
        out.up = rot(self.up);
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resize {
    /// sizes must be equal
    Strict,
    /// different sizes are accepted (a reducer or transition will be added)
    Allow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MateError {
    Domain,
    System,
    FlowDirection,
    Position,
    Facing,
    ShapeKind,
    Size,
    Voltage,
    Poles,
    /// rectangular sizes cannot line up: the heights are neither parallel nor square
    Orientation,
}

/// Can connector `a` join connector `b`? Both are in model coordinates. `tol` is the largest gap (metres).
pub fn mate(a: &Connector, b: &Connector, tol: f64, resize: Resize) -> Result<(), MateError> {
    if a.domain != b.domain {
        return Err(MateError::Domain);
    }
    if !a.system.compatible(b.system) {
        return Err(MateError::System);
    }
    if !a.flow_dir.joins(b.flow_dir) {
        return Err(MateError::FlowDirection);
    }
    let d = [a.pos[0] - b.pos[0], a.pos[1] - b.pos[1], a.pos[2] - b.pos[2]];
    if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > tol {
        return Err(MateError::Position);
    }
    let dot = a.dir[0] * b.dir[0] + a.dir[1] * b.dir[1] + a.dir[2] * b.dir[2];
    if dot > -1.0 + 1e-9 {
        return Err(MateError::Facing);
    }
    match (a.size, b.size) {
        (Size::Round { diameter: x }, Size::Round { diameter: y }) => {
            if (x - y).abs() > 1e-9 && resize == Resize::Strict {
                return Err(MateError::Size);
            }
        }
        (Size::Rect { width: w1, height: h1 }, Size::Rect { width: w2, height: h2 }) => {
            let c = (a.up[0] * b.up[0] + a.up[1] * b.up[1] + a.up[2] * b.up[2]).abs();
            let crossed = if c > 1.0 - 1e-6 {
                false
            } else if c < 1e-6 {
                true
            } else {
                return Err(MateError::Orientation);
            };
            let (w2, h2) = if crossed { (h2, w2) } else { (w2, h2) };
            if ((w1 - w2).abs() > 1e-9 || (h1 - h2).abs() > 1e-9) && resize == Resize::Strict {
                return Err(MateError::Size);
            }
        }
        (Size::Electrical { voltage: v1, poles: p1 }, Size::Electrical { voltage: v2, poles: p2 }) => {
            if (v1 - v2).abs() > 1e-9 {
                return Err(MateError::Voltage);
            }
            if p1 != p2 {
                return Err(MateError::Poles);
            }
        }
        _ => return Err(MateError::ShapeKind),
    }
    Ok(())
}
