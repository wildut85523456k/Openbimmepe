//! Spike S2: a parametric family solver.
//!
//! A family is a set of named values. An *input* has a default and a scope (type or instance) and is a
//! number, a whole number, a yes/no flag, a choice from a list, a free text label, or a material name.
//! Everything else is a *formula* over other numeric names. A *plane* is a formula tagged with an axis.
//! A choice also gives a yes/no value for each option (`swing.left`), so formulas can branch on it.
//! Solids are boxes between planes; a solid may repeat along one axis (mullions, louvres) and may take
//! its material from a material, choice or text value. Other families may be nested inside a family,
//! their inputs tied to formulas of the parent. Checks are formulas that must be true. Types are named
//! sets of input values, and an instance may override instance-scope inputs only.
//!
//! Solving is a single pass in dependency order, so the result never depends on the order the author
//! listed things, and a family that has a cycle, an unknown name or a plane on the wrong axis is
//! rejected when it is compiled. All lengths are metres.

pub mod expr;
pub mod mep;
pub mod forms;
pub mod shared;
pub mod catalogue;
pub mod library;

use expr::{Expr, ANG, LEN, NUM};
pub use expr::ExprError;
pub use expr::{Dim, Want, ANGLE, AREA, ENERGY, FLOW, FORCE, LENGTH, MASS, NUMBER, POWER, PRESSURE, TEMPERATURE, TIME, U_VALUE, VOLUME};
pub use shared::{SharedParam, SharedSet};
pub use forms::{Form, FormDef, FormGeom, FormKind, Loop, ProfileDef};
pub use mep::{Category, Connector, ConnectorDef, Dir, Domain, FlowDir, Host, MateError, PlanDef, PlanShape, Pose, Resize, ShapeDef, System};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

pub const MAX_VALUES: usize = 10_000;
pub const MAX_SOLIDS: usize = 1_000;
pub const MAX_ARRAY: f64 = 1_000.0;
/// Most boxes one placement may produce, so repeated solids cannot exhaust memory.
pub const MAX_BOXES: usize = 20_000;
/// Most nested family instances one placement may create (counts repeats and levels).
pub const MAX_NESTED_INSTANCES: usize = 20_000;
/// Deepest nesting of families inside families.
pub const MAX_NEST_DEPTH: usize = 8;
pub const MAX_TEXT: usize = 256;
pub const MIN_SIZE: f64 = 1e-6;
/// Name of the repeat index inside the formulas of a nested family (0, 1, 2 ...).
/// Straight sides of an arched opening are either absent or at least this long (metres).
pub const MIN_ARCH_SIDE: f64 = 1e-3;
/// An arch with less rise than this is refused (metres); use a rectangular head instead.
pub const MIN_ARCH_RISE: f64 = 1e-3;
/// Number of straight pieces that approximate the arc of an arched head.
pub const ARCH_SEGMENTS: usize = 32;
pub const INDEX_NAME: &str = "_i";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis { X, Y, Z }
impl Axis { fn i(self) -> usize { match self { Axis::X => 0, Axis::Y => 1, Axis::Z => 2 } } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope { Type, Instance }

#[derive(Debug, Clone)]
pub enum ValueKind {
    Input { default: f64, min: f64, max: f64, integer: bool, scope: Scope },
    Flag { default: bool, scope: Scope },
    Choice { options: Vec<String>, default: String, scope: Scope },
    Text { default: String, scope: Scope },
    Material { default: String, scope: Scope },
    Formula(String),
}

#[derive(Debug, Clone)]
pub struct ValueDef {
    pub name: String,
    pub kind: ValueKind,
    pub axis: Option<Axis>,
    /// what the value measures. An input is a length unless it is a count or says otherwise; a formula takes the
    /// dimension of its result, and when one is given here the formula must produce it.
    pub dim: Option<Dim>,
}

impl ValueDef {
    fn new(name: &str, kind: ValueKind) -> Self { ValueDef { name: name.into(), kind, axis: None, dim: None } }
    pub fn input(name: &str, default: f64, min: f64, max: f64, scope: Scope) -> Self {
        Self::new(name, ValueKind::Input { default, min, max, integer: false, scope })
    }
    /// An angle in degrees.
    pub fn angle(name: &str, default: f64, min: f64, max: f64, scope: Scope) -> Self { Self::input(name, default, min, max, scope).with_dim(ANGLE) }
    /// A number that is not a length (a ratio, a factor).
    pub fn number(name: &str, default: f64, min: f64, max: f64, scope: Scope) -> Self { Self::input(name, default, min, max, scope).with_dim(NUMBER) }
    pub fn with_dim(mut self, d: Dim) -> Self { self.dim = Some(d); self }
    pub fn count(name: &str, default: f64, min: f64, max: f64, scope: Scope) -> Self {
        Self::new(name, ValueKind::Input { default, min, max, integer: true, scope })
    }
    pub fn flag(name: &str, default: bool, scope: Scope) -> Self { Self::new(name, ValueKind::Flag { default, scope }) }
    pub fn choice(name: &str, options: &[&str], default: &str, scope: Scope) -> Self {
        Self::new(name, ValueKind::Choice { options: options.iter().map(|s| s.to_string()).collect(), default: default.into(), scope })
    }
    pub fn text(name: &str, default: &str, scope: Scope) -> Self { Self::new(name, ValueKind::Text { default: default.into(), scope }) }
    pub fn material(name: &str, default: &str, scope: Scope) -> Self { Self::new(name, ValueKind::Material { default: default.into(), scope }) }
    pub fn formula(name: &str, f: &str) -> Self { Self::new(name, ValueKind::Formula(f.into())) }
    pub fn plane(name: &str, axis: Axis, f: &str) -> Self { ValueDef { name: name.into(), kind: ValueKind::Formula(f.into()), axis: Some(axis), dim: None } }
}

#[derive(Debug, Clone)]
pub struct ArrayDef { pub axis: Axis, pub count: String, pub bar: String }

#[derive(Debug, Clone)]
pub struct SolidDef {
    pub name: String,
    pub x: (String, String), pub y: (String, String), pub z: (String, String),
    pub array: Option<ArrayDef>,
    pub visible: Option<String>,
    /// name of a material, choice or text value that gives this solid its material
    pub material: Option<String>,
    pub rotate: Option<RotateDef>,
}

/// Turn a solid (or a nested family) about a vertical line through (x, y) of the family frame. Angle in degrees, positive counter-clockwise seen from above.
#[derive(Debug, Clone)]
pub struct RotateDef { pub angle: String, pub pivot: (String, String) }
impl RotateDef {
    pub fn new(angle: &str, px: &str, py: &str) -> Self { RotateDef { angle: angle.into(), pivot: (px.into(), py.into()) } }
}

impl SolidDef {
    pub fn new(name: &str, x: (&str, &str), y: (&str, &str), z: (&str, &str)) -> Self {
        let t = |p: (&str, &str)| (p.0.to_string(), p.1.to_string());
        SolidDef { name: name.into(), x: t(x), y: t(y), z: t(z), array: None, visible: None, material: None, rotate: None }
    }
    pub fn array(mut self, axis: Axis, count: &str, bar: &str) -> Self { self.array = Some(ArrayDef { axis, count: count.into(), bar: bar.into() }); self }
    pub fn visible_if(mut self, f: &str) -> Self { self.visible = Some(f.into()); self }
    pub fn material(mut self, value_name: &str) -> Self { self.material = Some(value_name.into()); self }
    pub fn rotate_about(mut self, angle: &str, px: &str, py: &str) -> Self { self.rotate = Some(RotateDef::new(angle, px, py)); self }
}

#[derive(Debug, Clone)]
pub struct CheckDef { pub name: String, pub expr: String }

#[derive(Debug, Clone)]
pub struct OpeningDef { pub width: String, pub height: String, pub sill: String, pub rise: Option<String> }

impl OpeningDef {
    /// A rectangular opening: `height` is measured from the sill to the head.
    pub fn new(width: &str, height: &str, sill: &str) -> Self { OpeningDef { width: width.into(), height: height.into(), sill: sill.into(), rise: None } }
    /// Make the head a segmental arch of the given rise. `height` then reaches the crown of the arch.
    pub fn arched(mut self, rise: &str) -> Self { self.rise = Some(rise.into()); self }
}

#[derive(Debug, Clone, Default)]
pub struct TypeDef {
    pub name: String,
    /// numbers, whole numbers and yes/no (0 or 1)
    pub values: Vec<(String, f64)>,
    /// choices, texts and materials
    pub texts: Vec<(String, String)>,
}

/// Where a nested family's text, choice or material value comes from.
#[derive(Debug, Clone)]
pub enum TextSrc { Param(String), Literal(String) }

/// Another family placed inside this one. Its inputs are tied to formulas (numbers, yes/no) or to
/// values of this family (choice, text, material). Offsets are formulas; the repeat index is `_i`.
#[derive(Debug, Clone)]
pub struct NestedDef {
    pub name: String,
    pub family: String,
    pub type_name: Option<String>,
    pub at: (String, String, String),
    pub values: Vec<(String, String)>,
    pub texts: Vec<(String, TextSrc)>,
    pub count: Option<String>,
    pub visible: Option<String>,
    /// turns the placed nested family (after `at`) about a vertical line through the pivot, given in the parent's frame
    pub rotate: Option<RotateDef>,
}
impl NestedDef {
    pub fn new(name: &str, family: &str, at: (&str, &str, &str)) -> Self {
        NestedDef { name: name.into(), family: family.into(), type_name: None, at: (at.0.into(), at.1.into(), at.2.into()),
                    values: vec![], texts: vec![], count: None, visible: None, rotate: None }
    }
    pub fn set(mut self, child_input: &str, formula: &str) -> Self { self.values.push((child_input.into(), formula.into())); self }
    pub fn set_text(mut self, child_input: &str, src: TextSrc) -> Self { self.texts.push((child_input.into(), src)); self }
    pub fn repeat(mut self, count: &str) -> Self { self.count = Some(count.into()); self }
    pub fn visible_if(mut self, f: &str) -> Self { self.visible = Some(f.into()); self }
    pub fn rotate_about(mut self, angle: &str, px: &str, py: &str) -> Self { self.rotate = Some(RotateDef::new(angle, px, py)); self }
}

/// A turn about the vertical axis plus a shift in plan: x' = R(angle) x + t. Heights are not changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xf { pub angle: f64, pub t: [f64; 2] }
impl Xf {
    pub const IDENTITY: Xf = Xf { angle: 0.0, t: [0.0, 0.0] };
    pub fn translate(x: f64, y: f64) -> Xf { Xf { angle: 0.0, t: [x, y] } }
    pub fn about(angle: f64, px: f64, py: f64) -> Xf {
        let (s, c) = (expr::sin_deg(angle), expr::cos_deg(angle));
        Xf { angle, t: [px - (c * px - s * py), py - (s * px + c * py)] }
    }
    /// unit vectors of the turned x and y axes
    pub fn axes(&self) -> ([f64; 2], [f64; 2]) { let (s, c) = (expr::sin_deg(self.angle), expr::cos_deg(self.angle)); ([c, s], [-s, c]) }
    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] { let (s, c) = (expr::sin_deg(self.angle), expr::cos_deg(self.angle)); [c * p[0] - s * p[1] + self.t[0], s * p[0] + c * p[1] + self.t[1]] }
    /// A turn of 0 (or a whole number of full turns) is only a shift: fold it into the box and report no turn.
    pub fn settle(self, b: &mut Box3) -> Option<Xf> {
        if self.angle.rem_euclid(360.0) != 0.0 { return Some(self); }
        for a in 0..2 { b.min[a] += self.t[a]; b.max[a] += self.t[a]; }
        None
    }
    /// do `self` first, then `outer`: R2 (R1 x + t1) + t2
    pub fn then(&self, outer: &Xf) -> Xf { Xf { angle: self.angle + outer.angle, t: outer.apply(self.t) } }
}

#[derive(Debug, Clone, Default)]
pub struct Family {
    pub name: String,
    pub values: Vec<ValueDef>,
    pub solids: Vec<SolidDef>,
    pub nested: Vec<NestedDef>,
    pub checks: Vec<CheckDef>,
    pub opening: Option<OpeningDef>,
    pub types: Vec<TypeDef>,
    /// what the family is (decides the IFC class)
    pub category: Category,
    /// what it is placed on; None = the category's usual host
    pub host: Option<Host>,
    /// (classification system, code) pairs typed in by the author, for example a Uniclass or OmniClass code
    pub classification: Vec<(String, String)>,
    pub connectors: Vec<ConnectorDef>,
    pub plan: Vec<PlanDef>,
    /// names of shared parameters this family uses; they come before its own values
    pub shared: Vec<String>,
    /// extrusions, revolves and sweeps
    pub forms: Vec<FormDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FamilyError {
    TooMany(&'static str),
    DuplicateName(String),
    BadName(String),
    BadInput { name: String, why: &'static str },
    Formula { name: String, err: ExprError },
    Cycle(Vec<String>),
    UnknownPlane { solid: String, plane: String },
    WrongAxis { solid: String, plane: String },
    UnknownType(String),
    UnknownValue(String),
    NotAnInput(String),
    WrongScope(String),
    OutOfRange { name: String, value: f64 },
    NotWhole { name: String, value: f64 },
    NotYesNo { name: String, value: f64 },
    NotFinite(String),
    BadChoice { name: String, value: String },
    BadText { name: String, why: &'static str },
    WrongKind { name: String, expected: &'static str },
    TextInFormula(String),
    CheckFailed(String),
    CheckError { name: String, err: ExprError },
    Degenerate { solid: String },
    BadArch(&'static str),
    ArrayTooDense { solid: String },
    ArrayCount { solid: String },
    UnknownFamily(String),
    NestedLoop(Vec<String>),
    NestTooDeep,
    BadNested { nested: String, why: &'static str },
    Nested { path: String, err: Box<FamilyError> },
    BadMeta(&'static str),
    Shared { name: String, why: &'static str },
    Form { name: String, why: &'static str },
    UnknownShared(String),
    Connector { name: String, why: &'static str },
    Plan { index: usize, why: &'static str },
}

impl fmt::Display for FamilyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FamilyError::*;
        match self {
            TooMany(w) => write!(f, "too many {w}"),
            DuplicateName(n) => write!(f, "name '{n}' is used twice"),
            BadName(n) => write!(f, "'{n}' is not a valid name"),
            BadInput { name, why } => write!(f, "input '{name}': {why}"),
            Formula { name, err } => write!(f, "formula for '{name}': {err}"),
            Cycle(c) => write!(f, "formulas depend on each other in a loop: {}", c.join(", ")),
            UnknownPlane { solid, plane } => write!(f, "solid '{solid}' uses unknown plane '{plane}'"),
            WrongAxis { solid, plane } => write!(f, "solid '{solid}' uses plane '{plane}' on the wrong axis"),
            UnknownType(t) => write!(f, "unknown type '{t}'"),
            UnknownValue(n) => write!(f, "unknown value '{n}'"),
            NotAnInput(n) => write!(f, "'{n}' is calculated and cannot be set"),
            WrongScope(n) => write!(f, "'{n}' is a type value and cannot be changed on one instance"),
            OutOfRange { name, value } => write!(f, "'{name}' = {value} is outside its allowed range"),
            NotWhole { name, value } => write!(f, "'{name}' = {value} must be a whole number"),
            NotYesNo { name, value } => write!(f, "'{name}' = {value} must be yes (1) or no (0)"),
            NotFinite(n) => write!(f, "'{n}' is not a finite number"),
            BadChoice { name, value } => write!(f, "'{value}' is not one of the choices of '{name}'"),
            BadText { name, why } => write!(f, "'{name}': {why}"),
            WrongKind { name, expected } => write!(f, "'{name}' is not a {expected} value"),
            TextInFormula(n) => write!(f, "text or material value '{n}' cannot be used in a formula"),
            CheckFailed(n) => write!(f, "rule '{n}' is not satisfied"),
            CheckError { name, err } => write!(f, "rule '{name}': {err}"),
            BadArch(why) => write!(f, "arched opening: {why}"),
            Degenerate { solid } => write!(f, "solid '{solid}' has no thickness in at least one direction"),
            ArrayTooDense { solid } => write!(f, "solid '{solid}': repeated bars do not fit in their cells"),
            ArrayCount { solid } => write!(f, "solid '{solid}': repeat count must be a whole number from 0 to {MAX_ARRAY}"),
            UnknownFamily(n) => write!(f, "nested family '{n}' is not in the library"),
            NestedLoop(c) => write!(f, "families contain each other in a loop: {}", c.join(" > ")),
            NestTooDeep => write!(f, "families are nested more than {MAX_NEST_DEPTH} levels deep"),
            BadNested { nested, why } => write!(f, "nested family '{nested}': {why}"),
            Nested { path, err } => write!(f, "in {path}: {err}"),
            BadMeta(why) => write!(f, "family data: {why}"),
            Form { name, why } => write!(f, "form '{name}': {why}"),
            Shared { name, why } => write!(f, "shared parameter '{name}': {why}"),
            UnknownShared(n) => write!(f, "shared parameter '{n}' is not in the shared set"),
            Connector { name, why } => write!(f, "connector '{name}': {why}"),
            Plan { index, why } => write!(f, "plan symbol {index}: {why}"),
        }
    }
}
impl std::error::Error for FamilyError {}

#[derive(Debug, Clone, PartialEq)]
enum Spec {
    Num { default: f64, min: f64, max: f64, integer: bool },
    Flag { default: bool },
    Choice { options: Vec<String>, default: usize },
    Text { default: String },
    Material { default: String },
}
#[derive(Debug, Clone, PartialEq)]
struct InputInfo { spec: Spec, scope: Scope }

struct CSolid {
    name: String,
    span: [(usize, usize); 3],
    array: Option<(usize, Expr, Expr)>,
    visible: Option<Expr>,
    material: Option<usize>,
    rotate: Option<[Expr; 3]>,
}

enum TextFrom { Slot(usize), Literal(String) }

struct CNested {
    name: String,
    child: Compiled,
    type_name: Option<String>,
    at: [Expr; 3],
    values: Vec<(usize, Expr)>,
    texts: Vec<(usize, TextFrom)>,
    count: Option<Expr>,
    visible: Option<Expr>,
    rotate: Option<[Expr; 3]>,
    /// index of the first of this nested family's solids in the flattened solid list
    solid_offset: usize,
    form_offset: usize,
}

/// a type: name, numeric overrides by slot, text overrides by slot
type CompiledType = (String, Vec<(usize, f64)>, Vec<(usize, String)>);

/// Data of a compiled family. Use it through `Compiled`.
#[doc(hidden)]
pub struct Inner {
    name: String,
    names: Vec<String>,
    index: HashMap<String, usize>,
    nvalues: usize,
    inputs: Vec<Option<InputInfo>>,
    /// per choice value: (slot of the choice, slot of its first option flag)
    choice_flags: Vec<(usize, usize)>,
    formulas: Vec<Option<Expr>>,
    /// what each slot measures
    dims: Vec<Dim>,
    /// slots of calculated numbers that are not planes, in declaration order
    derived: Vec<usize>,
    order: Vec<usize>,
    solids: Vec<CSolid>,
    solid_names: Vec<String>,
    nested: Vec<CNested>,
    checks: Vec<(String, Expr)>,
    opening: Option<(Expr, Expr, Expr, Option<Expr>)>,
    types: Vec<CompiledType>,
    depth: usize,
    category: Category,
    host: Host,
    classification: Vec<(String, String)>,
    connectors: Vec<mep::CConnector>,
    plan: Vec<mep::CPlan>,
    shared_ids: Vec<(String, u128)>,
    forms: Vec<forms::CForm>,
    form_names: Vec<String>,
}

/// A family that has passed every structural check. Solving it cannot hit a missing name or a loop.
/// Cheap to clone: clones share the compiled data.
#[derive(Clone)]
pub struct Compiled(Arc<Inner>);
impl Deref for Compiled { type Target = Inner; fn deref(&self) -> &Inner { &self.0 } }

/// Names start with a letter (any language) or underscore, then letters, digits, underscores and single spaces
/// ("Rough Width", "Largeur_allège"); at most 64 characters, no space at the end.
pub(crate) fn valid_name(n: &str) -> bool {
    let mut c = n.chars();
    matches!(c.next(), Some(ch) if ch.is_alphabetic() || ch == '_')
        && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_' || ch == ' ')
        && !n.contains("  ")
        && !n.ends_with(' ')
        && n.chars().count() <= 64
}

pub(crate) fn clean_text(name: &str, s: &str, max: usize) -> Result<(), FamilyError> {
    if s.chars().count() > max { return Err(FamilyError::BadText { name: name.into(), why: "text is too long" }); }
    if s.chars().any(|c| c.is_control()) { return Err(FamilyError::BadText { name: name.into(), why: "text contains a control character" }); }
    Ok(())
}

impl Family {
    /// Compile a family that has no nested families (or only ones listed in `library`).
    pub fn compile(&self) -> Result<Compiled, FamilyError> {
        compile_library(std::slice::from_ref(self)).map(|mut v| v.remove(0))
    }
}

/// Compile a set of families that may nest each other by name. The result is in the same order.
pub fn compile_library(families: &[Family]) -> Result<Vec<Compiled>, FamilyError> { compile_library_with(families, None) }

/// Like `compile_library`, with a shared parameter set that families name in `Family::shared`.
pub fn compile_library_shared(families: &[Family], shared: &SharedSet) -> Result<Vec<Compiled>, FamilyError> {
    shared.check()?;
    compile_library_with(families, Some(shared))
}

fn compile_library_with(families: &[Family], shared: Option<&SharedSet>) -> Result<Vec<Compiled>, FamilyError> {
    let mut by_name: HashMap<&str, usize> = HashMap::new();
    for (i, f) in families.iter().enumerate() {
        if by_name.insert(f.name.as_str(), i).is_some() { return Err(FamilyError::DuplicateName(f.name.clone())); }
    }
    let mut done: Vec<Option<Compiled>> = vec![None; families.len()];
    fn get(i: usize, families: &[Family], by_name: &HashMap<&str, usize>, done: &mut Vec<Option<Compiled>>, stack: &mut Vec<usize>, shared: Option<&SharedSet>) -> Result<Compiled, FamilyError> {
        if let Some(c) = &done[i] { return Ok(c.clone()); }
        if let Some(p) = stack.iter().position(|&s| s == i) {
            let mut path: Vec<String> = stack[p..].iter().map(|&s| families[s].name.clone()).collect();
            path.push(families[i].name.clone());
            return Err(FamilyError::NestedLoop(path));
        }
        if stack.len() >= MAX_NEST_DEPTH { return Err(FamilyError::NestTooDeep); }
        stack.push(i);
        let mut kids: HashMap<String, Compiled> = HashMap::new();
        for n in &families[i].nested {
            if kids.contains_key(&n.family) { continue; }
            let j = *by_name.get(n.family.as_str()).ok_or_else(|| FamilyError::UnknownFamily(n.family.clone()))?;
            let c = get(j, families, by_name, done, stack, shared)?;
            kids.insert(n.family.clone(), c);
        }
        stack.pop();
        let c = compile_one(&families[i], &kids, shared)?;
        done[i] = Some(c.clone());
        Ok(c)
    }
    let mut out = Vec::new();
    for i in 0..families.len() { out.push(get(i, families, &by_name, &mut done, &mut Vec::new(), shared)?); }
    Ok(out)
}

fn compile_one(fam: &Family, kids: &HashMap<String, Compiled>, shared: Option<&SharedSet>) -> Result<Compiled, FamilyError> {
    // shared parameters come first, in the order the family names them
    let mut shared_ids = Vec::new();
    let expanded;
    let fam = if fam.shared.is_empty() { fam } else {
        let set = shared.ok_or_else(|| FamilyError::UnknownShared(fam.shared[0].clone()))?;
        let mut vals = Vec::with_capacity(fam.shared.len() + fam.values.len());
        for n in &fam.shared {
            let p = set.get(n).ok_or_else(|| FamilyError::UnknownShared(n.clone()))?;
            shared_ids.push((n.clone(), p.id));
            vals.push(p.def.clone());
        }
        vals.extend(fam.values.iter().cloned());
        expanded = Family { values: vals, ..fam.clone() };
        &expanded
    };
    if fam.values.len() > MAX_VALUES { return Err(FamilyError::TooMany("values")); }
    if fam.solids.len() > MAX_SOLIDS { return Err(FamilyError::TooMany("solids")); }
    if fam.nested.len() > MAX_SOLIDS { return Err(FamilyError::TooMany("nested families")); }
    // slots: the values in order, then one yes/no flag per choice option
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    for (i, v) in fam.values.iter().enumerate() {
        if !valid_name(&v.name) || v.name == INDEX_NAME { return Err(FamilyError::BadName(v.name.clone())); }
        if index.insert(v.name.clone(), i).is_some() { return Err(FamilyError::DuplicateName(v.name.clone())); }
        names.push(v.name.clone());
    }
    let nvalues = fam.values.len();
    let mut choice_flags = Vec::new();
    for (i, v) in fam.values.iter().enumerate() {
        if let ValueKind::Choice { options, .. } = &v.kind {
            if options.is_empty() || options.len() > 256 { return Err(FamilyError::BadInput { name: v.name.clone(), why: "a choice needs 1 to 256 options" }); }
            let first = names.len();
            for o in options {
                if !valid_name(o) { return Err(FamilyError::BadName(o.clone())); }
                let full = format!("{}.{}", v.name, o);
                if index.insert(full.clone(), names.len()).is_some() { return Err(FamilyError::DuplicateName(full)); }
                names.push(full);
            }
            choice_flags.push((i, first));
        }
    }
    if names.len() > MAX_VALUES * 2 { return Err(FamilyError::TooMany("values")); }
    let n = names.len();
    let mut inputs: Vec<Option<InputInfo>> = vec![None; n];
    let mut formula_src: Vec<Option<&str>> = vec![None; n];
    for (i, v) in fam.values.iter().enumerate() {
        let bad = |why: &'static str| FamilyError::BadInput { name: v.name.clone(), why };
        if v.axis.is_some() && !matches!(v.kind, ValueKind::Formula(_)) { return Err(bad("only a formula can be a plane")); }
        match &v.kind {
            ValueKind::Input { default, min, max, integer, scope } => {
                if !(default.is_finite() && min.is_finite() && max.is_finite()) { return Err(bad("numbers must be finite")); }
                if min > max { return Err(bad("minimum is above maximum")); }
                if default < min || default > max { return Err(bad("default is outside the range")); }
                if *integer && default.fract() != 0.0 { return Err(bad("default must be whole")); }
                inputs[i] = Some(InputInfo { spec: Spec::Num { default: *default, min: *min, max: *max, integer: *integer }, scope: *scope });
            }
            ValueKind::Flag { default, scope } => inputs[i] = Some(InputInfo { spec: Spec::Flag { default: *default }, scope: *scope }),
            ValueKind::Choice { options, default, scope } => {
                let d = options.iter().position(|o| o == default).ok_or_else(|| bad("default is not one of the options"))?;
                inputs[i] = Some(InputInfo { spec: Spec::Choice { options: options.clone(), default: d }, scope: *scope });
            }
            ValueKind::Text { default, scope } => {
                clean_text(&v.name, default, MAX_TEXT)?;
                inputs[i] = Some(InputInfo { spec: Spec::Text { default: default.clone() }, scope: *scope });
            }
            ValueKind::Material { default, scope } => {
                clean_text(&v.name, default, 128)?;
                inputs[i] = Some(InputInfo { spec: Spec::Material { default: default.clone() }, scope: *scope });
            }
            ValueKind::Formula(f) => formula_src[i] = Some(f.as_str()),
        }
    }
    let res = |s: &str| index.get(s).copied();
    // text and material values carry no number, so a formula may not read them
    let is_text = |slot: usize| matches!(inputs[slot].as_ref().map(|t| &t.spec), Some(Spec::Text { .. }) | Some(Spec::Material { .. }));
    // what each slot measures: inputs say so, calculated values are worked out below, in dependency order
    let dims: std::cell::RefCell<Vec<Dim>> = std::cell::RefCell::new(vec![NUMBER; n]);
    for (i, v) in fam.values.iter().enumerate() {
        if let ValueKind::Input { integer, .. } = &v.kind {
            dims.borrow_mut()[i] = v.dim.unwrap_or(if *integer { NUMBER } else { LENGTH });
        } else if !matches!(v.kind, ValueKind::Formula(_)) && v.dim.is_some_and(|d| d != NUMBER) {
            return Err(FamilyError::BadInput { name: v.name.clone(), why: "only a number input or a formula has a dimension" });
        }
    }
    let parse_raw = |label: &str, src: &str, extra: bool| -> Result<Expr, FamilyError> {
        let e = Expr::parse(src, &|s| if extra && s == INDEX_NAME { Some(n) } else { res(s) }).map_err(|err| FamilyError::Formula { name: label.into(), err })?;
        for &d in e.deps() { if d < n && is_text(d) { return Err(FamilyError::TextInFormula(names[d].clone())); } }
        Ok(e)
    };
    // a formula in a place that needs a certain dimension: the units must fit
    let parse = |label: &str, src: &str, extra: bool, want: Want| -> Result<Expr, FamilyError> {
        let e = parse_raw(label, src, extra)?;
        let d = dims.borrow();
        e.check(&|slot| d.get(slot).copied().unwrap_or(NUMBER), want).map_err(|err| FamilyError::Formula { name: label.into(), err })?;
        Ok(e)
    };
    let mut formulas: Vec<Option<Expr>> = vec![None; n];
    for i in 0..n { if let Some(src) = formula_src[i] { formulas[i] = Some(parse_raw(&names[i], src, false)?); } }
    // dependency order (Kahn). Inputs and choice flags have no formula.
    let mut indeg = vec![0usize; n];
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, f) in formulas.iter().enumerate() {
        if let Some(e) = f { for &d in e.deps() { if formulas[d].is_some() { indeg[i] += 1; users[d].push(i); } } }
    }
    let mut ready: Vec<usize> = (0..n).filter(|&i| formulas[i].is_some() && indeg[i] == 0).collect();
    let mut order = Vec::new();
    while let Some(i) = ready.pop() {
        order.push(i);
        for &u in &users[i] { indeg[u] -= 1; if indeg[u] == 0 { ready.push(u); } }
    }
    let nform = formulas.iter().filter(|f| f.is_some()).count();
    if order.len() != nform {
        let mut cyc: Vec<String> = (0..n).filter(|&i| formulas[i].is_some() && indeg[i] > 0).map(|i| names[i].clone()).collect();
        cyc.sort();
        return Err(FamilyError::Cycle(cyc));
    }
    for &i in &order {
        let v = &fam.values[i];
        let want = if v.axis.is_some() { LEN } else { v.dim.map_or(Want::Any, Want::Is) };
        let d = {
            let d = dims.borrow();
            formulas[i].as_ref().expect("a formula").check(&|slot| d.get(slot).copied().unwrap_or(NUMBER), want)
        };
        dims.borrow_mut()[i] = d.map_err(|err| FamilyError::Formula { name: names[i].clone(), err })?;
    }
    let dims_v: Vec<Dim> = dims.borrow().clone();
    let plane = |solid: &str, p: &str, axis: Axis| -> Result<usize, FamilyError> {
        let i = *index.get(p).ok_or_else(|| FamilyError::UnknownPlane { solid: solid.into(), plane: p.into() })?;
        if i >= nvalues || fam.values[i].axis != Some(axis) { return Err(FamilyError::WrongAxis { solid: solid.into(), plane: p.into() }); }
        Ok(i)
    };
    let material_slot = |owner: &str, m: &str| -> Result<usize, FamilyError> {
        let i = *index.get(m).ok_or_else(|| FamilyError::UnknownValue(m.into()))?;
        match inputs.get(i).and_then(|t| t.as_ref()).map(|t| &t.spec) {
            Some(Spec::Material { .. }) | Some(Spec::Text { .. }) | Some(Spec::Choice { .. }) => Ok(i),
            _ => Err(FamilyError::WrongKind { name: format!("{owner}.{m}"), expected: "material, choice or text" }),
        }
    };
    let mut solids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for s in &fam.solids {
        if !valid_name(&s.name) { return Err(FamilyError::BadName(s.name.clone())); }
        if !seen.insert(s.name.clone()) { return Err(FamilyError::DuplicateName(s.name.clone())); }
        let span = [
            (plane(&s.name, &s.x.0, Axis::X)?, plane(&s.name, &s.x.1, Axis::X)?),
            (plane(&s.name, &s.y.0, Axis::Y)?, plane(&s.name, &s.y.1, Axis::Y)?),
            (plane(&s.name, &s.z.0, Axis::Z)?, plane(&s.name, &s.z.1, Axis::Z)?),
        ];
        let pe = |part: &str, f: &str, w: Want| parse(&format!("{}.{}", s.name, part), f, false, w);
        let array = match &s.array { Some(a) => Some((a.axis.i(), pe("count", &a.count, NUM)?, pe("bar", &a.bar, LEN)?)), None => None };
        let visible = match &s.visible { Some(v) => Some(pe("visible", v, NUM)?), None => None };
        let material = match &s.material { Some(m) => Some(material_slot(&s.name, m)?), None => None };
        let rotate = match &s.rotate { Some(r) => Some([pe("rotate.angle", &r.angle, ANG)?, pe("rotate.x", &r.pivot.0, LEN)?, pe("rotate.y", &r.pivot.1, LEN)?]), None => None };
        solids.push(CSolid { name: s.name.clone(), span, array, visible, material, rotate });
    }
    let mut checks = Vec::new();
    for c in &fam.checks { checks.push((c.name.clone(), parse(&c.name, &c.expr, false, NUM)?)); }
    let opening = match &fam.opening {
        Some(o) => Some((
            parse("opening.width", &o.width, false, LEN)?, parse("opening.height", &o.height, false, LEN)?, parse("opening.sill", &o.sill, false, LEN)?,
            match &o.rise { Some(r) => Some(parse("opening.rise", r, false, LEN)?), None => None },
        )),
        None => None,
    };
    // nested families
    let mut solid_names: Vec<String> = solids.iter().map(|s| s.name.clone()).collect();
    let mut form_names: Vec<String> = fam.forms.iter().map(|f| f.name.clone()).collect();
    let mut nested = Vec::new();
    let mut nseen = std::collections::HashSet::new();
    let mut depth = 0;
    for nd in &fam.nested {
        if !valid_name(&nd.name) { return Err(FamilyError::BadName(nd.name.clone())); }
        if !nseen.insert(nd.name.clone()) || seen.contains(&nd.name) { return Err(FamilyError::DuplicateName(nd.name.clone())); }
        let child = kids.get(&nd.family).ok_or_else(|| FamilyError::UnknownFamily(nd.family.clone()))?.clone();
        let bad = |why: &'static str| FamilyError::BadNested { nested: nd.name.clone(), why };
        if let Some(t) = &nd.type_name { if !child.types.iter().any(|(nm, _, _)| nm == t) { return Err(bad("the nested family has no such type")); } }
        let lbl = |part: &str| format!("{}.{}", nd.name, part);
        let at = [parse(&lbl("x"), &nd.at.0, true, LEN)?, parse(&lbl("y"), &nd.at.1, true, LEN)?, parse(&lbl("z"), &nd.at.2, true, LEN)?];
        let mut values = Vec::new();
        let mut set_seen = std::collections::HashSet::new();
        for (cn, src) in &nd.values {
            let slot = child.slot(cn).ok_or_else(|| bad("a tied input does not exist in the nested family"))?;
            if !set_seen.insert(slot) { return Err(bad("an input is tied twice")); }
            match child.inputs[slot].as_ref().map(|t| &t.spec) {
                Some(Spec::Num { .. }) | Some(Spec::Flag { .. }) => {}
                _ => return Err(bad("a number can only be tied to a number or yes/no input")),
            }
            values.push((slot, parse(&lbl(cn), src, true, Want::Is(child.dims[slot]))?));
        }
        let mut texts = Vec::new();
        for (cn, src) in &nd.texts {
            let slot = child.slot(cn).ok_or_else(|| bad("a tied input does not exist in the nested family"))?;
            if !set_seen.insert(slot) { return Err(bad("an input is tied twice")); }
            let spec = match child.inputs[slot].as_ref().map(|t| &t.spec) { Some(s) => s, None => return Err(bad("a text can only be tied to a choice, text or material input")) };
            if matches!(spec, Spec::Num { .. } | Spec::Flag { .. }) { return Err(bad("a text can only be tied to a choice, text or material input")); }
            match src {
                TextSrc::Param(p) => {
                    let ps = *index.get(p).ok_or_else(|| FamilyError::UnknownValue(p.clone()))?;
                    if !matches!(inputs.get(ps).and_then(|t| t.as_ref()).map(|t| &t.spec), Some(Spec::Choice { .. }) | Some(Spec::Text { .. }) | Some(Spec::Material { .. })) {
                        return Err(bad("a text must come from a choice, text or material value"));
                    }
                    texts.push((slot, TextFrom::Slot(ps)));
                }
                TextSrc::Literal(l) => {
                    match spec {
                        Spec::Choice { options, .. } => if !options.contains(l) { return Err(bad("the fixed text is not a choice of the nested input")); },
                        Spec::Material { .. } => clean_text(cn, l, 128)?,
                        _ => clean_text(cn, l, MAX_TEXT)?,
                    }
                    texts.push((slot, TextFrom::Literal(l.clone())));
                }
            }
        }
        let count = match &nd.count { Some(c) => Some(parse(&lbl("count"), c, true, NUM)?), None => None };
        let visible = match &nd.visible { Some(v) => Some(parse(&lbl("visible"), v, true, NUM)?), None => None };
        let rotate = match &nd.rotate { Some(r) => Some([parse(&lbl("rotate.angle"), &r.angle, true, ANG)?, parse(&lbl("rotate.x"), &r.pivot.0, true, LEN)?, parse(&lbl("rotate.y"), &r.pivot.1, true, LEN)?]), None => None };
        let offset = solid_names.len();
        let form_offset = form_names.len();
        for fnm in &child.form_names { form_names.push(format!("{}/{}", nd.name, fnm)); }
        if form_names.len() > forms::MAX_FORMS * 10 { return Err(FamilyError::TooMany("forms including nested families")); }
        for sn in &child.solid_names { solid_names.push(format!("{}/{}", nd.name, sn)); }
        if solid_names.len() > MAX_SOLIDS { return Err(FamilyError::TooMany("solids including nested families")); }
        depth = depth.max(child.depth + 1);
        nested.push(CNested { name: nd.name.clone(), child, type_name: nd.type_name.clone(), at, values, texts, count, visible, rotate, solid_offset: offset, form_offset });
    }
    if depth > MAX_NEST_DEPTH { return Err(FamilyError::NestTooDeep); }
    let mut types = Vec::new();
    for t in &fam.types {
        let mut nums = Vec::new();
        for (nm, v) in &t.values {
            let i = *index.get(nm).ok_or_else(|| FamilyError::UnknownValue(nm.clone()))?;
            match inputs[i].as_ref().map(|t| &t.spec) {
                None => return Err(FamilyError::NotAnInput(nm.clone())),
                Some(Spec::Num { .. }) | Some(Spec::Flag { .. }) => nums.push((i, *v)),
                Some(_) => return Err(FamilyError::WrongKind { name: nm.clone(), expected: "number or yes/no" }),
            }
        }
        let mut txts = Vec::new();
        for (nm, v) in &t.texts {
            let i = *index.get(nm).ok_or_else(|| FamilyError::UnknownValue(nm.clone()))?;
            match inputs[i].as_ref().map(|t| &t.spec) {
                None => return Err(FamilyError::NotAnInput(nm.clone())),
                Some(Spec::Num { .. }) | Some(Spec::Flag { .. }) => return Err(FamilyError::WrongKind { name: nm.clone(), expected: "choice, text or material" }),
                Some(_) => txts.push((i, v.clone())),
            }
        }
        types.push((t.name.clone(), nums, txts));
    }
    // category, hosting, classification, connectors and plan symbol
    let host = fam.host.unwrap_or(if fam.opening.is_some() { Host::Wall } else { fam.category.default_host() });
    if matches!(fam.category, Category::Door | Category::Window) && host != Host::Wall { return Err(FamilyError::BadMeta("a door or window must be hosted by a wall")); }
    if fam.opening.is_some() && host != Host::Wall { return Err(FamilyError::BadMeta("a family that cuts an opening must be hosted by a wall")); }
    if fam.classification.len() > mep::MAX_CLASSIFICATIONS { return Err(FamilyError::TooMany("classification entries")); }
    for (sys, code) in &fam.classification {
        if sys.trim().is_empty() || code.trim().is_empty() { return Err(FamilyError::BadMeta("a classification needs a system name and a code")); }
        clean_text("classification", sys, 64)?;
        clean_text("classification", code, 64)?;
    }
    for (i, (s, _)) in fam.classification.iter().enumerate() { if fam.classification[..i].iter().any(|(o, _)| o == s) { return Err(FamilyError::BadMeta("a classification system is listed twice")); } }
    let (connectors, plan) = mep::compile(&fam.connectors, &fam.plan, &parse)?;
    let forms = forms::compile(&fam.forms, &parse, &material_slot)?;
    let derived: Vec<usize> = (0..nvalues).filter(|&i| formulas[i].is_some() && fam.values[i].axis.is_none()).collect();
    Ok(Compiled(Arc::new(Inner { name: fam.name.clone(), names, index, nvalues, inputs, choice_flags, formulas, dims: dims_v, derived, order, solids, solid_names, nested, checks, opening, types, depth, category: fam.category, host, classification: fam.classification.clone(), connectors, plan, shared_ids, forms, form_names })))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box3 { pub min: [f64; 3], pub max: [f64; 3] }
impl Box3 {
    pub fn volume(&self) -> f64 { (0..3).map(|k| self.max[k] - self.min[k]).product() }
    /// True when the interiors share volume (touching faces do not count).
    pub fn overlaps(&self, o: &Box3, tol: f64) -> bool { (0..3).all(|k| self.min[k] < o.max[k] - tol && o.min[k] < self.max[k] - tol) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub values: Vec<f64>,
    /// (solid index, box); see `Inner::solid_name`. A repeated solid gives one entry per bar.
    /// Solids of nested families follow the family's own solids.
    pub solids: Vec<(usize, Box3)>,
    /// for each entry of `solids`: index into `strings` of its material, if it has one
    pub materials: Vec<Option<u32>>,
    /// for each entry of `solids`: the turn and shift of the box in plan (None = as is); the box itself is given before the turn
    pub xf: Vec<Option<Xf>>,
    pub strings: Vec<String>,
    /// text and material values: (value slot, text)
    pub texts: Vec<(usize, String)>,
    /// width, height, sill of the wall opening, if the family cuts one.
    pub opening: Option<[f64; 3]>,
    /// rise of the arched head of the opening (0 = rectangular head)
    pub opening_rise: f64,
    /// solved MEP connectors in the family's own frame (hidden ones left out)
    pub connectors: Vec<Connector>,
    /// solved plan symbol
    pub plan: Vec<PlanShape>,
    /// solved extrusions, revolves and sweeps; `Form::index` refers to `Inner::form_name`
    pub forms: Vec<Form>,
    /// for each entry of `forms`: index into `strings` of its material, if it has one
    pub form_materials: Vec<Option<u32>>,
}

impl Instance {
    /// Material name of the solid entry `k`.
    pub fn material_of(&self, k: usize) -> Option<&str> { self.materials[k].map(|i| self.strings[i as usize].as_str()) }
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(i) = self.strings.iter().position(|x| x == s) { return i as u32; }
        self.strings.push(s.to_string());
        (self.strings.len() - 1) as u32
    }
}

#[derive(Debug, Clone, Default)]
pub struct Request<'a> {
    pub type_name: Option<&'a str>,
    /// numbers, whole numbers and yes/no (0 or 1) by name
    pub overrides: Vec<(&'a str, f64)>,
    /// choices, texts and materials by name
    pub texts: Vec<(&'a str, &'a str)>,
}

/// Inputs already resolved to slots.
struct Resolved<'a> {
    type_name: Option<&'a str>,
    nums: Vec<(usize, f64)>,
    texts: Vec<(usize, String)>,
}

impl Inner {
    pub fn name(&self) -> &str { &self.name }
    pub fn category(&self) -> Category { self.category }
    /// (name, identity) of the shared parameters this family uses
    pub fn shared_ids(&self) -> &[(String, u128)] { &self.shared_ids }
    pub fn host(&self) -> Host { self.host }
    /// Names of the family's types, in the order the author gave them.
    pub fn type_names(&self) -> Vec<&str> { self.types.iter().map(|t| t.0.as_str()).collect() }
    pub fn classification(&self) -> &[(String, String)] { &self.classification }
    pub fn slot(&self, name: &str) -> Option<usize> { self.index.get(name).copied() }
    /// What a value measures: a length, an area, an angle, a plain number. Calculated values take it from their formula.
    pub fn value_dim(&self, name: &str) -> Option<Dim> { self.slot(name).map(|i| self.dims[i]) }
    pub fn value(&self, inst: &Instance, name: &str) -> Option<f64> { self.slot(name).map(|i| inst.values[i]) }

    /// Text of a choice, text or material value in `inst`.
    pub fn text<'a>(&'a self, inst: &'a Instance, name: &str) -> Option<&'a str> {
        let i = self.slot(name)?;
        match &self.inputs.get(i)?.as_ref()?.spec {
            Spec::Choice { options, .. } => options.get(inst.values[i] as usize).map(|s| s.as_str()),
            Spec::Text { .. } | Spec::Material { .. } => inst.texts.iter().find(|(s, _)| *s == i).map(|(_, t)| t.as_str()),
            _ => None,
        }
    }
    /// Scope of an input value, or None if `name` is unknown or calculated.
    pub fn input_scope(&self, name: &str) -> Option<Scope> { self.slot(name).and_then(|i| self.inputs.get(i)?.as_ref()).map(|t| t.scope) }
    /// (name, value) of every number and yes/no input in `inst`, in definition order.
    pub fn input_values<'a>(&'a self, inst: &Instance) -> Vec<(&'a str, f64)> {
        (0..self.nvalues).filter(|&i| matches!(self.inputs[i].as_ref().map(|t| &t.spec), Some(Spec::Num { .. }) | Some(Spec::Flag { .. }))).map(|i| (self.names[i].as_str(), inst.values[i])).collect()
    }
    /// (name, text) of every choice, text and material input in `inst`, in definition order.
    pub fn input_texts<'a>(&'a self, inst: &'a Instance) -> Vec<(&'a str, &'a str)> {
        (0..self.nvalues).filter_map(|i| self.text(inst, &self.names[i]).map(|t| (self.names[i].as_str(), t))).collect()
    }
    /// Calculated numbers that are not planes (areas, U-values, counts), by name, as solved for `inst`.
    pub fn derived_values<'a>(&'a self, inst: &Instance) -> Vec<(&'a str, f64)> {
        self.derived.iter().map(|&i| (self.names[i].as_str(), inst.values[i])).collect()
    }
    pub fn solid_name(&self, solid: usize) -> &str { &self.solid_names[solid] }
    pub fn form_name(&self, form: usize) -> &str { &self.form_names[form] }

    pub fn solve(&self, req: &Request) -> Result<Instance, FamilyError> {
        let mut r = Resolved { type_name: req.type_name, nums: Vec::with_capacity(req.overrides.len()), texts: Vec::with_capacity(req.texts.len()) };
        for &(nm, v) in &req.overrides {
            let i = self.slot(nm).ok_or_else(|| FamilyError::UnknownValue(nm.into()))?;
            let inp = self.inputs.get(i).and_then(|t| t.as_ref()).ok_or_else(|| FamilyError::NotAnInput(nm.into()))?;
            if inp.scope != Scope::Instance { return Err(FamilyError::WrongScope(nm.into())); }
            if !matches!(inp.spec, Spec::Num { .. } | Spec::Flag { .. }) { return Err(FamilyError::WrongKind { name: nm.into(), expected: "number or yes/no" }); }
            r.nums.push((i, v));
        }
        for &(nm, v) in &req.texts {
            let i = self.slot(nm).ok_or_else(|| FamilyError::UnknownValue(nm.into()))?;
            let inp = self.inputs.get(i).and_then(|t| t.as_ref()).ok_or_else(|| FamilyError::NotAnInput(nm.into()))?;
            if inp.scope != Scope::Instance { return Err(FamilyError::WrongScope(nm.into())); }
            if matches!(inp.spec, Spec::Num { .. } | Spec::Flag { .. }) { return Err(FamilyError::WrongKind { name: nm.into(), expected: "choice, text or material" }); }
            r.texts.push((i, v.to_string()));
        }
        let mut budget = MAX_NESTED_INSTANCES;
        self.run(&r, &mut budget)
    }

    fn run(&self, r: &Resolved, budget: &mut usize) -> Result<Instance, FamilyError> {
        let n = self.names.len();
        let mut vals = vec![0.0; n + 1]; // the last slot holds the repeat index for nested families
        let mut texts: Vec<(usize, String)> = Vec::new();
        #[allow(clippy::needless_range_loop)] // i indexes inputs, vals and names together
        for i in 0..self.nvalues {
            if let Some(inp) = &self.inputs[i] {
                match &inp.spec {
                    Spec::Num { default, .. } => vals[i] = *default,
                    Spec::Flag { default } => vals[i] = if *default { 1.0 } else { 0.0 },
                    Spec::Choice { default, .. } => vals[i] = *default as f64,
                    Spec::Text { default } | Spec::Material { default } => texts.push((i, default.clone())),
                }
            }
        }
        let set_text = |texts: &mut Vec<(usize, String)>, vals: &mut [f64], slot: usize, v: &str| -> Result<(), FamilyError> {
            let name = &self.names[slot];
            match &self.inputs[slot].as_ref().expect("checked input").spec {
                Spec::Choice { options, .. } => {
                    let k = options.iter().position(|o| o == v).ok_or_else(|| FamilyError::BadChoice { name: name.clone(), value: v.into() })?;
                    vals[slot] = k as f64;
                }
                Spec::Text { .. } => { clean_text(name, v, MAX_TEXT)?; texts.iter_mut().find(|(s, _)| *s == slot).expect("text slot").1 = v.to_string(); }
                Spec::Material { .. } => { clean_text(name, v, 128)?; texts.iter_mut().find(|(s, _)| *s == slot).expect("text slot").1 = v.to_string(); }
                _ => unreachable!("numeric slots are never set from text"),
            }
            Ok(())
        };
        // layer 1: type values, layer 2: the request's values (later entries win)
        if let Some(t) = r.type_name {
            let (_, nums, txts) = self.types.iter().find(|(nm, _, _)| nm == t).ok_or_else(|| FamilyError::UnknownType(t.into()))?;
            for &(i, v) in nums { vals[i] = v; }
            for (i, v) in txts { set_text(&mut texts, &mut vals, *i, v)?; }
        }
        for &(i, v) in &r.nums { vals[i] = v; }
        for (i, v) in &r.texts { set_text(&mut texts, &mut vals, *i, v)?; }
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.nvalues {
            if let Some(inp) = &self.inputs[i] {
                let v = vals[i];
                let name = &self.names[i];
                match &inp.spec {
                    Spec::Num { min, max, integer, .. } => {
                        if !v.is_finite() { return Err(FamilyError::NotFinite(name.clone())); }
                        if v < *min || v > *max { return Err(FamilyError::OutOfRange { name: name.clone(), value: v }); }
                        if *integer && v.fract() != 0.0 { return Err(FamilyError::NotWhole { name: name.clone(), value: v }); }
                    }
                    Spec::Flag { .. } => {
                        if !v.is_finite() { return Err(FamilyError::NotFinite(name.clone())); }
                        if v != 0.0 && v != 1.0 { return Err(FamilyError::NotYesNo { name: name.clone(), value: v }); }
                    }
                    _ => {}
                }
            }
        }
        for &(c, first) in &self.choice_flags {
            if let Some(InputInfo { spec: Spec::Choice { options, .. }, .. }) = &self.inputs[c] {
                for k in 0..options.len() { vals[first + k] = if vals[c] as usize == k { 1.0 } else { 0.0 }; }
            }
        }
        for &i in &self.order {
            let e = self.formulas[i].as_ref().expect("ordered slots are formulas");
            vals[i] = e.eval(&vals).map_err(|err| FamilyError::Formula { name: self.names[i].clone(), err })?;
        }
        for (nm, e) in &self.checks {
            match e.eval(&vals) {
                Ok(v) if v != 0.0 => {}
                Ok(_) => return Err(FamilyError::CheckFailed(nm.clone())),
                Err(err) => return Err(FamilyError::CheckError { name: nm.clone(), err }),
            }
        }
        let mut inst = Instance { values: Vec::new(), solids: Vec::with_capacity(self.solids.len() + 8), materials: Vec::with_capacity(self.solids.len() + 8), xf: Vec::with_capacity(self.solids.len() + 8), strings: Vec::new(), texts: Vec::new(), opening: None, opening_rise: 0.0, connectors: Vec::new(), plan: Vec::new(), forms: Vec::new(), form_materials: Vec::new() };
        // own solids
        for (si, s) in self.solids.iter().enumerate() {
            if let Some(v) = &s.visible {
                let x = v.eval(&vals).map_err(|err| FamilyError::Formula { name: format!("{}.visible", s.name), err })?;
                if x == 0.0 { continue; }
            }
            let mut b = Box3 { min: [0.0; 3], max: [0.0; 3] };
            for k in 0..3 { b.min[k] = vals[s.span[k].0]; b.max[k] = vals[s.span[k].1]; }
            if (0..3).any(|k| b.max[k] - b.min[k] <= MIN_SIZE) { return Err(FamilyError::Degenerate { solid: s.name.clone() }); }
            let mat = match s.material {
                None => None,
                Some(slot) => {
                    let t = match &self.inputs[slot].as_ref().expect("material slot is an input").spec {
                        Spec::Choice { options, .. } => options[vals[slot] as usize].clone(),
                        _ => texts.iter().find(|(x, _)| *x == slot).expect("text slot").1.clone(),
                    };
                    Some(inst.intern(&t))
                }
            };
            let xf = match &s.rotate {
                None => None,
                Some(r) => {
                    let e = |k: usize, part: &str| r[k].eval(&vals).map_err(|err| FamilyError::Formula { name: format!("{}.rotate.{}", s.name, part), err });
                    Some(Xf::about(e(0, "angle")?, e(1, "x")?, e(2, "y")?))
                }
            };
            let (b, xf) = match xf { Some(x) => { let mut b = b; let r = x.settle(&mut b); (b, r) } None => (b, None) };
            match &s.array {
                None => { inst.solids.push((si, b)); inst.materials.push(mat); inst.xf.push(xf); }
                Some((axis, count, bar)) => {
                    let c = count.eval(&vals).map_err(|err| FamilyError::Formula { name: format!("{}.count", s.name), err })?;
                    if c.fract() != 0.0 || !(0.0..=MAX_ARRAY).contains(&c) { return Err(FamilyError::ArrayCount { solid: s.name.clone() }); }
                    let w = bar.eval(&vals).map_err(|err| FamilyError::Formula { name: format!("{}.bar", s.name), err })?;
                    let cnt = c as usize;
                    if cnt == 0 { continue; }
                    let cell = (b.max[*axis] - b.min[*axis]) / (cnt as f64 + 1.0);
                    if w <= MIN_SIZE || w >= cell { return Err(FamilyError::ArrayTooDense { solid: s.name.clone() }); }
                    if inst.solids.len() + cnt > MAX_BOXES { return Err(FamilyError::TooMany("boxes in one placement")); }
                    for k in 1..=cnt {
                        let centre = b.min[*axis] + cell * k as f64;
                        let mut bb = b; bb.min[*axis] = centre - w / 2.0; bb.max[*axis] = centre + w / 2.0;
                        inst.solids.push((si, bb)); inst.materials.push(mat); inst.xf.push(xf);
                    }
                }
            }
        }
        // own forms (extrusions, revolves, sweeps)
        for fm in forms::solve_forms(&self.forms, &vals)? {
            let mat = match self.forms[fm.index].material {
                None => None,
                Some(slot) => {
                    let t = match &self.inputs[slot].as_ref().expect("material slot is an input").spec {
                        Spec::Choice { options, .. } => options[vals[slot] as usize].clone(),
                        _ => texts.iter().find(|(x, _)| *x == slot).expect("text slot").1.clone(),
                    };
                    Some(inst.intern(&t))
                }
            };
            inst.forms.push(fm);
            inst.form_materials.push(mat);
        }
        // nested families
        for nd in &self.nested {
            if let Some(v) = &nd.visible {
                let x = v.eval(&vals).map_err(|err| FamilyError::Formula { name: format!("{}.visible", nd.name), err })?;
                if x == 0.0 { continue; }
            }
            let reps = match &nd.count {
                None => 1usize,
                Some(c) => {
                    let c = c.eval(&vals).map_err(|err| FamilyError::Formula { name: format!("{}.count", nd.name), err })?;
                    if c.fract() != 0.0 || !(0.0..=MAX_ARRAY).contains(&c) { return Err(FamilyError::BadNested { nested: nd.name.clone(), why: "repeat count must be a whole number from 0 to 1000" }); }
                    c as usize
                }
            };
            for rep in 0..reps {
                if *budget == 0 { return Err(FamilyError::TooMany("nested family instances")); }
                *budget -= 1;
                vals[n] = rep as f64;
                let fe = |part: &str, e: &Expr, vals: &[f64]| e.eval(vals).map_err(|err| FamilyError::Formula { name: format!("{}.{}", nd.name, part), err });
                let at = [fe("x", &nd.at[0], &vals)?, fe("y", &nd.at[1], &vals)?, fe("z", &nd.at[2], &vals)?];
                let mut cr = Resolved { type_name: nd.type_name.as_deref(), nums: Vec::with_capacity(nd.values.len()), texts: Vec::with_capacity(nd.texts.len()) };
                for (slot, e) in &nd.values { cr.nums.push((*slot, fe(&nd.child.names[*slot], e, &vals)?)); }
                for (slot, src) in &nd.texts {
                    let t = match src {
                        TextFrom::Literal(l) => l.clone(),
                        TextFrom::Slot(ps) => match &self.inputs[*ps].as_ref().expect("text source is an input").spec {
                            Spec::Choice { options, .. } => options[vals[*ps] as usize].clone(),
                            _ => texts.iter().find(|(x, _)| x == ps).expect("text slot").1.clone(),
                        },
                    };
                    cr.texts.push((*slot, t));
                }
                let ci = nd.child.run(&cr, budget).map_err(|err| FamilyError::Nested { path: nd.name.clone(), err: Box::new(err) })?;
                if inst.solids.len() + ci.solids.len() > MAX_BOXES { return Err(FamilyError::TooMany("boxes in one placement")); }
                let turn = match &nd.rotate {
                    None => None,
                    Some(r) => {
                        let e = |k: usize, part: &str| fe(part, &r[k], &vals);
                        Some(Xf::about(e(0, "rotate.angle")?, e(1, "rotate.x")?, e(2, "rotate.y")?))
                    }
                };
                // forms of the nested family, moved and turned like its solids
                if inst.forms.len() + ci.forms.len() > forms::MAX_FORMS_TOTAL { return Err(FamilyError::TooMany("forms in one placement")); }
                for (k, fm) in ci.forms.iter().enumerate() {
                    let mut x = fm.xf.then(&Xf::translate(at[0], at[1]));
                    if let Some(t) = turn { x = x.then(&t); }
                    let m = match ci.form_materials[k] { Some(i) => Some(inst.intern(&ci.strings[i as usize])), None => None };
                    inst.forms.push(Form { index: nd.form_offset + fm.index, geom: fm.geom.clone(), outer: fm.outer.clone(), holes: fm.holes.clone(), xf: x, dz: fm.dz + at[2] });
                    inst.form_materials.push(m);
                }
                // connectors of the nested family, moved and turned like its solids
                if inst.connectors.len() + ci.connectors.len() > mep::MAX_CONNECTORS_TOTAL { return Err(FamilyError::TooMany("connectors in one placement")); }
                if inst.plan.len() + ci.plan.len() > mep::MAX_PLAN_TOTAL { return Err(FamilyError::TooMany("plan symbol lines in one placement")); }
                for s in &ci.plan {
                    let mv = |x: f64, y: f64| { let (x, y) = (x + at[0], y + at[1]); match turn { Some(tf) => { let p = tf.apply([x, y]); (p[0], p[1]) } None => (x, y) } };
                    inst.plan.push(match *s {
                        PlanShape::Line([x1, y1, x2, y2]) => { let (a, b) = (mv(x1, y1), mv(x2, y2)); PlanShape::Line([a.0, a.1, b.0, b.1]) }
                        PlanShape::Circle { c, r } => { let p = mv(c[0], c[1]); PlanShape::Circle { c: [p.0, p.1], r } }
                    });
                }
                for c in &ci.connectors {
                    let mut c = c.clone();
                    c.name = if nd.count.is_some() { format!("{}#{}/{}", nd.name, rep, c.name) } else { format!("{}/{}", nd.name, c.name) };
                    c.pos = [c.pos[0] + at[0], c.pos[1] + at[1], c.pos[2] + at[2]];
                    if let Some(tf) = turn {
                        let p = tf.apply([c.pos[0], c.pos[1]]);
                        c.pos[0] = p[0]; c.pos[1] = p[1];
                        let (s, co) = (expr::sin_deg(tf.angle), expr::cos_deg(tf.angle));
                        c.dir = [co * c.dir[0] - s * c.dir[1], s * c.dir[0] + co * c.dir[1], c.dir[2]];
                    }
                    inst.connectors.push(c);
                }
                for (k, (si, b)) in ci.solids.iter().enumerate() {
                    let mut bb = *b;
                    bb.min[2] += at[2]; bb.max[2] += at[2];
                    let mut x = ci.xf[k].unwrap_or(Xf::IDENTITY).then(&Xf::translate(at[0], at[1]));
                    if let Some(t) = turn { x = x.then(&t); }
                    let xf = x.settle(&mut bb);
                    inst.solids.push((nd.solid_offset + si, bb));
                    let m = match ci.materials[k] { Some(i) => Some(inst.intern(&ci.strings[i as usize])), None => None };
                    inst.materials.push(m);
                    inst.xf.push(xf);
                }
            }
        }
        inst.opening = match &self.opening {
            Some((w, h, s, r)) => {
                let f = |nm: &str, e: &Expr| e.eval(&vals).map_err(|err| FamilyError::Formula { name: format!("opening.{nm}"), err });
                let (w, h, s) = (f("width", w)?, f("height", h)?, f("sill", s)?);
                if let Some(r) = r {
                    let rise = f("rise", r)?;
                    check_arch(w, h, rise)?;
                    inst.opening_rise = rise;
                }
                Some([w, h, s])
            }
            None => None,
        };
        let mut own = mep::solve_connectors(&self.connectors, &vals)?;
        own.append(&mut inst.connectors);
        inst.connectors = own;
        let mut own = mep::solve_plan(&self.plan, &vals)?;
        own.append(&mut inst.plan);
        inst.plan = own;
        vals.truncate(n);
        inst.values = vals;
        inst.texts = texts;
        Ok(inst)
    }

    /// Solve many placements in parallel. The answer for request i is at position i, whatever the thread count.
    pub fn solve_many(&self, reqs: &[Request]) -> Vec<Result<Instance, FamilyError>> {
        reqs.par_iter().map(|r| self.solve(r)).collect()
    }
}

fn check_arch(w: f64, h: f64, rise: f64) -> Result<(), FamilyError> {
    if rise < 0.0 { return Err(FamilyError::BadArch("rise cannot be negative")); }
    if rise == 0.0 { return Ok(()); }
    if rise < MIN_ARCH_RISE { return Err(FamilyError::BadArch("rise is too small; use rise 0 for a flat head")); }
    if rise > w / 2.0 { return Err(FamilyError::BadArch("rise cannot be more than half the width")); }
    // the straight part under the arch: none at all, or at least 1 mm (a negative length means rise > height)
    let side = h - rise;
    if side != 0.0 && side < MIN_ARCH_SIDE { return Err(FamilyError::BadArch("the rise must equal the opening height or leave straight sides of at least 1 mm")); }
    Ok(())
}

impl Instance {
    /// The outline of the opening in (s, z) coordinates of the wall segment, counter-clockwise, for an opening
    /// whose left edge is at `offset`. Rectangular heads give 4 points; arched heads give the straight part
    /// plus `ARCH_SEGMENTS` pieces of a circular arc. None if the family cuts no opening.
    pub fn opening_outline(&self, offset: f64) -> Option<Vec<[f64; 2]>> {
        let [w, h, sill] = self.opening?;
        let rise = self.opening_rise;
        let (l, r, top) = (offset, offset + w, sill + h);
        if rise == 0.0 { return Some(vec![[l, sill], [r, sill], [r, top], [l, top]]); }
        let spring = top - rise;
        let half = w / 2.0;
        let radius = (half * half + rise * rise) / (2.0 * rise);
        let alpha = (half / radius).asin();
        let (zc, c) = (top - radius, offset + half);
        let mut pts = vec![[l, sill], [r, sill]];
        if spring > sill { pts.push([r, spring]); }
        // arc from the right springing point over the crown to the left one
        for j in 1..ARCH_SEGMENTS {
            let phi = alpha - 2.0 * alpha * (j as f64) / (ARCH_SEGMENTS as f64);
            if j * 2 == ARCH_SEGMENTS { pts.push([c, top]); } else { pts.push([c + radius * phi.sin(), zc + radius * phi.cos()]); }
        }
        if spring > sill { pts.push([l, spring]); }
        Some(pts)
    }
}
