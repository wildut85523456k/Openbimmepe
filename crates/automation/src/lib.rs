//! Independent typed automation. Evaluation is read-only; only a validated plan
//! can modify the document, through its existing atomic transaction API.
#![forbid(unsafe_code)]
use openbim_document::{Command, Document, EditError, ElementId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub mod protocol;
pub mod worker;
pub mod mcp;
pub mod mcp_model;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Property { Width, Height, Elevation, Slope }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type { Elements, Length, Ratio }
impl Property {
    pub fn value_type(self) -> Type {
        if self == Self::Slope { Type::Ratio } else { Type::Length }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum NodeKind {
    Selection { ids: Vec<u64> },
    Read { property: Property },
    Dimensions,
    Length { metres: f64 },
    Ratio { value: f64 },
    Slider { value:f64, min:f64, max:f64, step:f64, quantity:Type },
    /// Numeric expression applied to each input quantity in a separate Python process.
    Python { expression: String, quantity: Type },
    Set { property: Property },
}
impl NodeKind {
    pub fn inputs(&self) -> Vec<Type> {
        match self {
            Self::Read { .. } | Self::Dimensions => vec![Type::Elements],
            Self::Python { quantity, .. } => vec![*quantity],
            Self::Set { property } => vec![Type::Elements, property.value_type()],
            _ => vec![],
        }
    }
    pub fn output(&self) -> Type {
        match self {
            Self::Read { property } => property.value_type(),
            Self::Length { .. } | Self::Dimensions => Type::Length,
            Self::Ratio { .. } => Type::Ratio,
            Self::Python { quantity, .. } | Self::Slider { quantity,.. } => *quantity,
            _ => Type::Elements,
        }
    }
    /// IDs are persistent identifiers, independent of displayed names/order.
    pub fn outputs(&self) -> Vec<(u16, &'static str, Type)> {
        if matches!(self,Self::Dimensions) {
            vec![(0,"width",Type::Length),(1,"height",Type::Length),(2,"elevation",Type::Length)]
        } else { vec![(0,"result",self.output())] }
    }
    pub fn output_type(&self,id:u16)->Option<Type> {
        self.outputs().into_iter().find(|p|p.0==id).map(|p|p.2)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node { pub id: u64, pub kind: NodeKind, pub position: [f32; 2] }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    #[serde(default, skip_serializing_if="is_default_output")]
    pub output: u16,
    pub from: u64, pub to: u64, pub input: usize,
    /// Optional visual waypoint offset from the midpoint of the two ports.
    /// This changes the displayed route only, never graph execution.
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub route: Option<[i32; 2]>,
}
fn is_default_output(id:&u16)->bool { *id==0 }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub version: u32, pub nodes: Vec<Node>, pub wires: Vec<Wire>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub groups:Vec<Group>,
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group { pub id:u64, pub title:String, pub nodes:Vec<u64>, pub color:[u8;3] }
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    Version, DuplicateNode(u64), MissingNode(u64), InvalidInput(u64, usize),
    DuplicateInput(u64, usize), MissingInput(u64, usize), TypeMismatch(u64, usize),
    Cycle, InvalidGroup(u64), InvalidOutput(u64,u16), InvalidNumber(u64), MissingElement(u64), UnsupportedProperty(u64),
    InvalidPythonType(u64), PythonDisabled(u64), Worker(u64, String), Cancelled, GraphLimit, InvalidRoute(u64,usize),
    Cardinality(u64), ConflictingWrite(u64, Property), Document(EditError), Json(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value { Elements(Vec<u64>), Quantities(Vec<f64>) }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change { pub element: u64, pub property: Property, pub before: f64, pub after: f64 }
#[derive(Clone, Debug)]
pub struct Plan {
    // Keep the exact snapshot to reject plans applied to a different document
    // with the same revision as well as plans made stale by intervening edits.
    snapshot: Document,
    commands: Vec<Command>,
    changes: Vec<Change>,
    values: BTreeMap<u64, Value>,
    port_values: BTreeMap<(u64,u16),Value>,
}
impl Plan {
    pub fn changes(&self) -> &[Change] { &self.changes }
    pub fn values(&self) -> &BTreeMap<u64, Value> { &self.values }
    pub fn port_values(&self) -> &BTreeMap<(u64,u16),Value> { &self.port_values }
    pub fn apply(self, document: &mut Document) -> Result<u64, Error> {
        if *document != self.snapshot {
            return Err(Error::Document(EditError::RevisionConflict {
                expected: self.snapshot.revision(), actual: document.revision(),
            }));
        }
        if self.commands.is_empty() { return Ok(document.revision()); }
        document.transact(self.snapshot.revision(), &self.commands).map_err(Error::Document)
    }
}
impl Graph {
    pub fn example(ids: Vec<u64>) -> Self {
        Self { version: 1, groups:vec![], nodes: vec![
            Node { id: 1, kind: NodeKind::Selection { ids }, position: [20., 30.] },
            Node { id: 2, kind: NodeKind::Read { property: Property::Elevation }, position: [290., 30.] },
            Node { id: 3, kind: NodeKind::Length { metres: 3. }, position: [20., 210.] },
            Node { id: 4, kind: NodeKind::Set { property: Property::Elevation }, position: [560., 130.] },
        ], wires: vec![
            Wire { output:0, from: 1, to: 2, input: 0, route: None }, Wire { output:0, from: 1, to: 4, input: 0, route: None },
            Wire { output:0, from: 3, to: 4, input: 1, route: None },
        ] }
    }
    pub fn to_json(&self) -> Result<String, Error> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| Error::Json(e.to_string()))
    }
    pub fn from_json(json: &str) -> Result<Self, Error> {
        let graph: Self = serde_json::from_str(json).map_err(|e| Error::Json(e.to_string()))?;
        graph.validate()?;
        Ok(graph)
    }
    pub fn validate(&self) -> Result<Vec<u64>, Error> {
        if self.version != 1 { return Err(Error::Version); }
        if self.nodes.len()>10_000 || self.wires.len()>30_000 { return Err(Error::GraphLimit); }
        if self.groups.len()>1000 { return Err(Error::GraphLimit); }
        let mut nodes = BTreeMap::new();
        for n in &self.nodes {
            if nodes.insert(n.id, n).is_some() { return Err(Error::DuplicateNode(n.id)); }
            if n.position.iter().any(|v| !v.is_finite()) { return Err(Error::InvalidNumber(n.id)); }
            match n.kind {
                NodeKind::Length { metres } if !metres.is_finite() => return Err(Error::InvalidNumber(n.id)),
                NodeKind::Ratio { value } if !value.is_finite() => return Err(Error::InvalidNumber(n.id)),
                NodeKind::Slider { value,min,max,step,quantity } if
                    [value,min,max,step].iter().any(|v|!v.is_finite()) || min>=max || step<=0.0 ||
                    !(max-min).is_finite() || value<min || value>max || quantity==Type::Elements => return Err(Error::InvalidNumber(n.id)),
                NodeKind::Python { quantity: Type::Elements, .. } => return Err(Error::InvalidPythonType(n.id)),
                _ => {}
            }
        }
        let mut group_ids=BTreeSet::new();let mut members=BTreeSet::new();
        for g in &self.groups {
            if !group_ids.insert(g.id) || g.title.len()>256 || g.nodes.is_empty() ||
                g.nodes.iter().any(|id|!nodes.contains_key(id) || !members.insert(*id)) {
                return Err(Error::InvalidGroup(g.id));
            }
        }
        let mut inputs = BTreeSet::new();
        for w in &self.wires {
            if w.route.is_some_and(|offset|offset.iter().any(|v|i64::from(*v).abs()>100_000)) {
                return Err(Error::InvalidRoute(w.to,w.input));
            }
            let from = nodes.get(&w.from).ok_or(Error::MissingNode(w.from))?;
            let to = nodes.get(&w.to).ok_or(Error::MissingNode(w.to))?;
            let ports = to.kind.inputs();
            let expected = ports.get(w.input).ok_or(Error::InvalidInput(w.to, w.input))?;
            if !inputs.insert((w.to, w.input)) { return Err(Error::DuplicateInput(w.to, w.input)); }
            let actual=from.kind.output_type(w.output).ok_or(Error::InvalidOutput(w.from,w.output))?;
            if *expected != actual { return Err(Error::TypeMismatch(w.to, w.input)); }
        }
        for n in &self.nodes {
            for i in 0..n.kind.inputs().len() {
                if !inputs.contains(&(n.id, i)) { return Err(Error::MissingInput(n.id, i)); }
            }
        }
        let mut incoming: BTreeMap<u64,usize> = nodes.keys().map(|id|(*id,0)).collect();
        let mut outgoing: BTreeMap<u64,Vec<u64>> = BTreeMap::new();
        for w in &self.wires {
            *incoming.get_mut(&w.to).expect("validated target")+=1;
            outgoing.entry(w.from).or_default().push(w.to);
        }
        let mut ready: BTreeSet<u64> = incoming.iter().filter(|(_,count)|**count==0).map(|(id,_)|*id).collect();
        let mut order=Vec::new();
        while let Some(id)=ready.pop_first() {
            order.push(id);
            if let Some(targets)=outgoing.get(&id) {
                for target in targets {
                    let count=incoming.get_mut(target).expect("validated target");
                    *count-=1;
                    if *count==0 { ready.insert(*target); }
                }
            }
        }
        if order.len()!=nodes.len() { return Err(Error::Cycle); }
        Ok(order)
    }
    pub fn preview(&self, document: &Document) -> Result<Plan, Error> {
        self.preview_with(document, None, &AtomicBool::new(false))
    }
    pub fn preview_with(&self, document: &Document, python: Option<&worker::PythonWorker>, cancel: &AtomicBool) -> Result<Plan, Error> {
        let order = self.validate()?;
        let mut values = BTreeMap::new();
        let mut port_values = BTreeMap::new();
        let mut commands = Vec::new();
        let mut changes = Vec::new();
        let mut writes = BTreeSet::new();
        for id in order {
            if cancel.load(Ordering::Relaxed) { return Err(Error::Cancelled); }
            let node = self.nodes.iter().find(|n| n.id == id).expect("validated node");
            let input = |i| {
                let wire = self.wires.iter().find(|w| w.to == id && w.input == i).expect("validated input");
                port_values.get(&(wire.from,wire.output)).expect("topological input")
            };
            let value = match &node.kind {
                NodeKind::Selection { ids } => {
                    let mut seen = BTreeSet::new();
                    for eid in ids {
                        if document.get(ElementId(*eid)).is_none() { return Err(Error::MissingElement(*eid)); }
                    }
                    Value::Elements(ids.iter().copied().filter(|id| seen.insert(*id)).collect())
                }
                NodeKind::Length { metres } => Value::Quantities(vec![*metres]),
                NodeKind::Ratio { value } | NodeKind::Slider { value,.. } => Value::Quantities(vec![*value]),
                NodeKind::Python { expression, .. } => {
                    let Value::Quantities(numbers) = input(0) else { unreachable!() };
                    let python = python.ok_or(Error::PythonDisabled(id))?;
                    Value::Quantities(python.evaluate(expression, numbers, cancel).map_err(|e| {
                        if cancel.load(Ordering::Relaxed) { Error::Cancelled } else { Error::Worker(id, e) }
                    })?)
                }
                NodeKind::Dimensions => {
                    let Value::Elements(ids) = input(0) else { unreachable!() };
                    let ids=ids.clone();
                    for (port,property) in [(0,Property::Width),(1,Property::Height),(2,Property::Elevation)] {
                        port_values.insert((id,port),Value::Quantities(ids.iter().map(|eid|read(document,*eid,property)).collect::<Result<_,_>>()?));
                    }
                    port_values.get(&(id,0)).unwrap().clone()
                }
                NodeKind::Read { property } => {
                    let Value::Elements(ids) = input(0) else { unreachable!() };
                    Value::Quantities(ids.iter().map(|eid| read(document, *eid, *property)).collect::<Result<_, _>>()?)
                }
                NodeKind::Set { property } => {
                    let Value::Elements(ids) = input(0) else { unreachable!() };
                    let Value::Quantities(numbers) = input(1) else { unreachable!() };
                    if numbers.len() != 1 && numbers.len() != ids.len() { return Err(Error::Cardinality(id)); }
                    for (i, eid) in ids.iter().enumerate() {
                        // Multiple writers to the same property are ambiguous, even if values match.
                        if !writes.insert((*eid, *property as u8)) { return Err(Error::ConflictingWrite(*eid, *property)); }
                        let before = read(document, *eid, *property)?;
                        let after = numbers[if numbers.len() == 1 { 0 } else { i }];
                        if before == after { continue; }
                        let el = document.get(ElementId(*eid)).ok_or(Error::MissingElement(*eid))?;
                        commands.push(match property {
                            Property::Elevation => Command::MoveElevation { id: el.id, elevation_m: after },
                            Property::Slope => Command::SetSlope { id: el.id, slope_ratio: after },
                            // Width and height writes are merged below to avoid clobbering one another.
                            Property::Width | Property::Height => Command::Resize {
                                id: el.id, width_m: if *property == Property::Width { after } else { el.width_m },
                                height_m: if *property == Property::Height { after } else { el.height_m },
                            },
                        });
                        changes.push(Change { element: *eid, property: *property, before, after });
                    }
                    Value::Elements(ids.clone())
                }
            };
            port_values.insert((id,0),value.clone());
            values.insert(id, value);
        }
        // Compose independent dimension edits using all proposed changes.
        for cmd in &mut commands {
            if let Command::Resize { id, width_m, height_m } = cmd {
                for c in changes.iter().filter(|c| c.element == id.0) {
                    match c.property { Property::Width => *width_m = c.after, Property::Height => *height_m = c.after, _ => {} }
                }
            }
        }
        // Validate against the host's constraints before exposing a usable plan.
        if cancel.load(Ordering::Relaxed) { return Err(Error::Cancelled); }
        if !commands.is_empty() {
            let mut trial = document.clone();
            trial.transact(document.revision(), &commands).map_err(Error::Document)?;
        }
        Ok(Plan { snapshot: document.clone(), commands, changes, values, port_values })
    }
}
fn read(document: &Document, id: u64, property: Property) -> Result<f64, Error> {
    let el = document.get(ElementId(id)).ok_or(Error::MissingElement(id))?;
    Ok(match property {
        Property::Width => el.width_m, Property::Height => el.height_m,
        Property::Elevation => el.elevation_m,
        Property::Slope => el.slope_ratio.ok_or(Error::UnsupportedProperty(id))?,
    })
}
