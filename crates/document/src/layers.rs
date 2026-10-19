//! Layered wall types, as Revit's compound walls are: a wall type has a list of layers from the exterior face to the
//! interior face (function, material, thickness, whether the layer wraps round the ends and openings) and a core, the
//! run of layers between the two core boundaries. The thickness of the type, and of every wall of the type, is the sum
//! of the layers, so nothing else in the document needs to know that a wall has layers: they only add to what is drawn,
//! to where the Location Line puts the wall and to what is written to IFC.
//!
//! The exterior is on the left of the direction in which a wall runs (draw clockwise and it faces out).
use crate::{Category, Command, Document, EditError, Element, ElementId};

pub const MAX_LAYERS: usize = 64;

/// What a layer is for. The order is the order of Revit's list; the priority decides which layer runs through the other
/// wall's layers where two walls join (structure first, finishes last).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerFunction {
    Structure,
    Substrate,
    ThermalAir,
    Membrane,
    Finish1,
    Finish2,
}

impl LayerFunction {
    pub const ALL: [LayerFunction; 6] = [Self::Structure, Self::Substrate, Self::ThermalAir, Self::Membrane, Self::Finish1, Self::Finish2];
    pub fn name(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Substrate => "Substrate",
            Self::ThermalAir => "Thermal/Air Layer",
            Self::Membrane => "Membrane Layer",
            Self::Finish1 => "Finish 1",
            Self::Finish2 => "Finish 2",
        }
    }
    /// The short code written in records.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Structure => "S",
            Self::Substrate => "B",
            Self::ThermalAir => "T",
            Self::Membrane => "M",
            Self::Finish1 => "F1",
            Self::Finish2 => "F2",
        }
    }
    pub(crate) fn from_code(code: &str) -> Option<Self> { Self::ALL.into_iter().find(|f| f.code() == code) }
}

/// One layer of a wall type.
#[derive(Clone, Debug, PartialEq)]
pub struct WallLayer {
    pub material: String,
    pub thickness_m: f64,
    pub function: LayerFunction,
    /// The layer turns round the ends of the wall and the edges of its openings.
    pub wraps: bool,
}

impl WallLayer {
    pub fn new(material: &str, thickness_m: f64, function: LayerFunction) -> Self { Self { material: material.into(), thickness_m, function, wraps: false } }
}

/// Where the line a wall is drawn along runs in its thickness, in the order of Revit's Location Line list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationLine {
    WallCentreline,
    CoreCentreline,
    FinishFaceExterior,
    FinishFaceInterior,
    CoreFaceExterior,
    CoreFaceInterior,
}

impl LocationLine {
    pub const ALL: [LocationLine; 6] = [Self::WallCentreline, Self::CoreCentreline, Self::FinishFaceExterior, Self::FinishFaceInterior, Self::CoreFaceExterior, Self::CoreFaceInterior];
    pub fn name(self) -> &'static str {
        match self {
            Self::WallCentreline => "Wall Centerline",
            Self::CoreCentreline => "Core Centerline",
            Self::FinishFaceExterior => "Finish Face: Exterior",
            Self::FinishFaceInterior => "Finish Face: Interior",
            Self::CoreFaceExterior => "Core Face: Exterior",
            Self::CoreFaceInterior => "Core Face: Interior",
        }
    }
    /// The same line seen from the other side of the wall.
    pub fn flipped(self) -> Self {
        match self {
            Self::FinishFaceExterior => Self::FinishFaceInterior,
            Self::FinishFaceInterior => Self::FinishFaceExterior,
            Self::CoreFaceExterior => Self::CoreFaceInterior,
            Self::CoreFaceInterior => Self::CoreFaceExterior,
            other => other,
        }
    }
}

/// The sum of the thicknesses, in the order of the layers (so that it is the same number wherever it is added up).
pub fn thickness(layers: &[WallLayer]) -> f64 { layers.iter().map(|l| l.thickness_m).sum() }

/// The first and last layer of the core; without core boundaries the whole wall is the core.
pub fn core_range(layers: &[WallLayer], core: Option<(usize, usize)>) -> (usize, usize) { core.unwrap_or((0, layers.len().saturating_sub(1))) }

/// Whether the layers make a wall: at least one, none thick enough to be silly, a core on layers of the wall that
/// holds structure and no membrane.
pub fn check(layers: &[WallLayer], core: Option<(usize, usize)>) -> Result<(), String> {
    if layers.is_empty() { return Err("a wall type needs at least one layer".into()); }
    if layers.len() > MAX_LAYERS { return Err(format!("more than {MAX_LAYERS} layers")); }
    for l in layers {
        if l.material.trim().is_empty() || l.material.len() > 64 || l.material.chars().any(char::is_control) { return Err("a layer needs a material name of up to 64 characters".into()); }
        if !l.thickness_m.is_finite() || l.thickness_m < 0.0 || l.thickness_m > 5.0 || (l.thickness_m == 0.0 && l.function != LayerFunction::Membrane) {
            return Err(format!("layer '{}' needs a thickness above 0 and up to 5 m (only a membrane layer may have none)", l.material));
        }
    }
    let (a, b) = core_range(layers, core);
    if a > b || b >= layers.len() { return Err("the core boundaries are not on layers of the wall".into()); }
    let inside = &layers[a..=b];
    if inside.iter().any(|l| l.function == LayerFunction::Membrane) { return Err("a membrane layer cannot be in the core".into()); }
    if !inside.iter().any(|l| l.function == LayerFunction::Structure) { return Err("the core needs a structure layer".into()); }
    if thickness(layers) <= 0.0 { return Err("the wall has no thickness".into()); }
    Ok(())
}

/// Distances from the exterior face to the core's exterior face and to its interior face.
pub fn core_faces(layers: &[WallLayer], core: Option<(usize, usize)>) -> (f64, f64) {
    let (a, b) = core_range(layers, core);
    let before: f64 = layers[..a.min(layers.len())].iter().map(|l| l.thickness_m).sum();
    let inside: f64 = layers.get(a..=b).map_or(0.0, |s| s.iter().map(|l| l.thickness_m).sum());
    (before, before + inside)
}

/// How far the centre of a wall lies to the left of the line it is drawn along, for a Location Line (negative: to the
/// right). The line is `pos` from the exterior face, the centre is half the thickness from it. A wall of no layers is
/// all core and all finish, so the core lines are the finish lines. `flipped` reads the line from the other side.
pub fn centre_shift(thickness_m: f64, layers: &[WallLayer], core: Option<(usize, usize)>, loc: LocationLine, flipped: bool) -> f64 {
    let loc = if flipped { loc.flipped() } else { loc };
    let (c0, c1) = if layers.is_empty() { (0.0, thickness_m) } else { core_faces(layers, core) };
    let pos = match loc {
        LocationLine::WallCentreline => thickness_m / 2.0,
        LocationLine::CoreCentreline => (c0 + c1) / 2.0,
        LocationLine::FinishFaceExterior => 0.0,
        LocationLine::FinishFaceInterior => thickness_m,
        LocationLine::CoreFaceExterior => c0,
        LocationLine::CoreFaceInterior => c1,
    };
    pos - thickness_m / 2.0
}

impl Element {
    /// A wall type with layers.
    pub fn is_layered(&self) -> bool { self.category == Category::Type && !self.layers.is_empty() }
}

impl Document {
    /// A new wall type made of `layers`, `height_m` high, ready to be drawn with. Its thickness is the sum of the layers.
    pub fn add_layered_wall_type(&mut self, name: &str, height_m: f64, layers: Vec<WallLayer>, core: Option<(usize, usize)>) -> Result<ElementId, EditError> {
        let name = name.trim();
        if name.is_empty() || !height_m.is_finite() || height_m <= 0.0 || check(&layers, core).is_err() { return Err(EditError::InvalidElement); }
        if self.types_for(Category::Wall).iter().any(|t| t.name == name) { return Err(EditError::InvalidElement); }
        let id = self.next_id();
        let ops = vec![
            Command::Create { id, category: Category::Type, name: name.into(), width_m: thickness(&layers), height_m, elevation_m: 0.0 },
            Command::SetTypeFor { id, category: Category::Wall },
            Command::SetLayers { id, layers, core },
        ];
        self.transact(self.revision, &ops)?;
        Ok(id)
    }

    /// Gives the wall type `type_id` these layers (none: it goes back to a plain thickness, the one it has) and every
    /// wall of the type the new thickness, in one transaction. Walls keep their line: their faces move.
    pub fn set_wall_layers(&mut self, type_id: ElementId, layers: Vec<WallLayer>, core: Option<(usize, usize)>) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type && t.type_for == Some(Category::Wall)).ok_or(EditError::NotFound(type_id))?;
        if !layers.is_empty() && check(&layers, core).is_err() { return Err(EditError::InvalidElement); }
        let width = if layers.is_empty() { ty.width_m } else { thickness(&layers) };
        let mut ops = vec![Command::SetLayers { id: type_id, layers, core }];
        for id in self.instances_of(type_id) {
            let h = self.get(id).map_or(0.0, |w| w.height_m);
            ops.push(Command::Resize { id, width_m: width, height_m: h });
        }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// Everything the Edit Type window can change in a wall type, in one transaction: its name, its height, and its
    /// layers (none: a plain type `plain_thickness_m` thick). Every wall of the type follows.
    pub fn edit_wall_type(&mut self, type_id: ElementId, name: &str, height_m: f64, layers: Vec<WallLayer>, core: Option<(usize, usize)>, plain_thickness_m: f64) -> Result<(), EditError> {
        let ty = self.get(type_id).filter(|t| t.category == Category::Type && t.type_for == Some(Category::Wall)).ok_or(EditError::NotFound(type_id))?;
        let name = name.trim();
        if name.is_empty() || !height_m.is_finite() || height_m <= 0.0 { return Err(EditError::InvalidElement); }
        if !layers.is_empty() && check(&layers, core).is_err() { return Err(EditError::InvalidElement); }
        if layers.is_empty() && !(plain_thickness_m.is_finite() && plain_thickness_m > 0.0) { return Err(EditError::InvalidElement); }
        let width = if layers.is_empty() { plain_thickness_m } else { thickness(&layers) };
        let mut ops = Vec::new();
        if ty.name != name { ops.push(Command::Rename { id: type_id, name: name.into() }); }
        if !layers.is_empty() || ty.is_layered() { ops.push(Command::SetLayers { id: type_id, layers, core }); }
        ops.push(Command::Resize { id: type_id, width_m: width, height_m });
        for id in self.instances_of(type_id) { ops.push(Command::Resize { id, width_m: width, height_m }); }
        self.transact(self.revision, &ops)?;
        Ok(())
    }

    /// The layers and core of the type of the wall `id`, if its type has any.
    pub fn wall_layers(&self, id: ElementId) -> Option<(&[WallLayer], Option<(usize, usize)>)> {
        let ty = self.get(self.get(id)?.type_id?)?;
        ty.is_layered().then_some((ty.layers.as_slice(), ty.core))
    }

    /// The lines a plan draws inside a layered wall: where each layer ends, along every segment of the wall, with the
    /// two core boundaries marked (they are drawn heavier). Nothing for a wall of a plain type.
    pub fn wall_layer_lines(&self, id: ElementId) -> Vec<([f64; 2], [f64; 2], bool)> {
        let Some((layers, core)) = self.wall_layers(id) else { return Vec::new() };
        let Some(plans) = self.wall_chain(id).and_then(|c| c.plans().ok()) else { return Vec::new() };
        let total = thickness(layers);
        let (a, b) = core_range(layers, core);
        let mut out = Vec::new();
        let mut at = 0.0;
        for (i, l) in layers.iter().enumerate().take(layers.len() - 1) {
            at += l.thickness_m;
            let f = at / total;
            // the line between layer i and i + 1 is a core boundary when the core starts or ends there
            let boundary = core.is_some() && (i + 1 == a || i == b);
            for p in &plans {
                let point = |l: [f64; 2], r: [f64; 2]| [l[0] + (r[0] - l[0]) * f, l[1] + (r[1] - l[1]) * f];
                out.push((point(p.l0, p.r0), point(p.l1, p.r1), boundary));
            }
        }
        out
    }

    /// How far the centre of the wall type `type_id` (none: a plain wall `thickness_m` thick) lies to the left of a line
    /// drawn along `loc`.
    pub fn location_shift(&self, type_id: Option<ElementId>, thickness_m: f64, loc: LocationLine, flipped: bool) -> f64 {
        match type_id.and_then(|t| self.get(t)).filter(|t| t.is_layered()) {
            Some(t) => centre_shift(t.width_m, &t.layers, t.core, loc, flipped),
            None => centre_shift(thickness_m, &[], None, loc, flipped),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LayerFunction::*;

    pub(crate) fn cavity() -> Vec<WallLayer> {
        vec![
            WallLayer::new("Render", 0.02, Finish1),
            WallLayer::new("Brick", 0.1, Structure),
            WallLayer::new("Insulation", 0.05, ThermalAir),
            WallLayer::new("Block", 0.15, Structure),
            WallLayer::new("Plaster", 0.015, Finish2),
        ]
    }

    #[test]
    fn layers_are_checked_like_revit_checks_a_compound_wall() {
        assert!(check(&cavity(), None).is_ok() && check(&cavity(), Some((1, 3))).is_ok());
        assert!(check(&[], None).is_err());
        assert!(check(&cavity(), Some((3, 1))).is_err() && check(&cavity(), Some((1, 9))).is_err());
        assert!(check(&cavity(), Some((0, 0))).is_err(), "the core needs structure");
        let mut membrane = cavity();
        membrane[2].function = Membrane;
        assert!(check(&membrane, Some((1, 3))).is_err(), "no membrane in the core");
        assert!(check(&membrane, Some((1, 1))).is_ok());
        membrane[2].thickness_m = 0.0;
        assert!(check(&membrane, Some((1, 1))).is_ok(), "a membrane may have no thickness");
        let mut zero = cavity();
        zero[0].thickness_m = 0.0;
        assert!(check(&zero, None).is_err());
        zero[0].thickness_m = f64::NAN;
        assert!(check(&zero, None).is_err());
        let mut nameless = cavity();
        nameless[1].material = "  ".into();
        assert!(check(&nameless, None).is_err());
        assert!(check(&vec![WallLayer::new("x", 0.1, Structure); 65], None).is_err());
    }

    #[test]
    fn the_location_line_puts_the_centre_of_the_wall_where_revit_does() {
        let l = cavity();
        let t = thickness(&l);
        assert!((t - 0.335).abs() < 1e-12);
        // core (layers 1..=3) runs from 0.02 to 0.32 of 0.335
        let shift = |loc, flip| centre_shift(t, &l, Some((1, 3)), loc, flip);
        assert!(shift(LocationLine::WallCentreline, false).abs() < 1e-12);
        assert!((shift(LocationLine::FinishFaceExterior, false) + t / 2.0).abs() < 1e-12, "the line is on the exterior face, the centre is half a wall to its right");
        assert!((shift(LocationLine::FinishFaceInterior, false) - t / 2.0).abs() < 1e-12);
        assert!((shift(LocationLine::CoreFaceExterior, false) - (0.02 - t / 2.0)).abs() < 1e-12);
        assert!((shift(LocationLine::CoreFaceInterior, false) - (0.32 - t / 2.0)).abs() < 1e-12);
        assert!((shift(LocationLine::CoreCentreline, false) - (0.17 - t / 2.0)).abs() < 1e-12, "the core is not in the middle of this wall");
        // flipped, the exterior lines are the interior ones
        assert_eq!(shift(LocationLine::FinishFaceExterior, true), shift(LocationLine::FinishFaceInterior, false));
        assert_eq!(shift(LocationLine::CoreFaceInterior, true), shift(LocationLine::CoreFaceExterior, false));
        assert_eq!(shift(LocationLine::CoreCentreline, true), shift(LocationLine::CoreCentreline, false));
        // a plain wall: the core is the wall
        for loc in LocationLine::ALL {
            let plain = centre_shift(0.2, &[], None, loc, false);
            let expect = match loc { LocationLine::FinishFaceExterior | LocationLine::CoreFaceExterior => -0.1, LocationLine::FinishFaceInterior | LocationLine::CoreFaceInterior => 0.1, _ => 0.0 };
            assert!((plain - expect).abs() < 1e-12, "{loc:?}");
        }
    }

    #[test]
    fn a_layered_type_is_as_thick_as_its_layers_and_so_are_its_walls() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(crate::Geometry::Linear { level, .. }) => level, _ => panic!() };
        let t = d.add_layered_wall_type("Cavity 335", 3.0, cavity(), Some((1, 3))).unwrap();
        assert!((d.get(t).unwrap().width_m - 0.335).abs() < 1e-12 && d.get(t).unwrap().is_layered());
        assert!(d.add_layered_wall_type("Cavity 335", 3.0, cavity(), None).is_err(), "the name is taken");
        assert!(d.add_layered_wall_type("cavity 335", 3.0, cavity(), None).is_err(), "names differ only in case");
        assert!(d.add_layered_wall_type("Bad", 3.0, vec![], None).is_err());
        let w = d.place_wall_typed(level, [20.0, 0.0], [26.0, 0.0], 0.1, 2.0, 0.0, &[], Some(t)).unwrap();
        assert!((d.get(w).unwrap().width_m - 0.335).abs() < 1e-12 && d.get(w).unwrap().height_m == 3.0);
        let (l, core) = d.wall_layers(w).unwrap();
        assert_eq!((l.len(), core), (5, Some((1, 3))));
        // a wall's own thickness cannot be set while it follows the type
        assert!(d.transact(d.revision(), &[Command::Resize { id: w, width_m: 0.3, height_m: 3.0 }]).is_err());
        // changing a layer changes the type and every wall of it in one step
        let revision = d.revision();
        let mut thicker = cavity();
        thicker[2].thickness_m = 0.1;
        d.set_wall_layers(t, thicker, Some((1, 3))).unwrap();
        assert_eq!(d.revision(), revision + 1);
        assert!((d.get(t).unwrap().width_m - 0.385).abs() < 1e-12 && (d.get(w).unwrap().width_m - 0.385).abs() < 1e-12);
        // layers that do not make a wall change nothing
        assert!(d.set_wall_layers(t, vec![WallLayer::new("Foam", 0.1, Membrane)], None).is_err());
        assert!((d.get(w).unwrap().width_m - 0.385).abs() < 1e-12);
        assert!(d.undo().unwrap());
        assert!((d.get(w).unwrap().width_m - 0.335).abs() < 1e-12 && (d.get(t).unwrap().width_m - 0.335).abs() < 1e-12);
        // taking the layers away leaves a plain type of the same thickness
        d.set_wall_layers(t, vec![], None).unwrap();
        assert!(!d.get(t).unwrap().is_layered() && (d.get(w).unwrap().width_m - 0.335).abs() < 1e-12 && d.wall_layers(w).is_none());
        // only walls' types hold layers
        let bt = {
            let b = d.place_beam(level, &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.3, 0.5, 2.8)).unwrap();
            d.create_type(b, "B").unwrap()
        };
        assert!(d.set_wall_layers(bt, cavity(), None).is_err());
        assert!(d.transact(d.revision(), &[Command::SetLayers { id: w, layers: cavity(), core: None }]).is_err(), "not on a wall");
    }

    #[test]
    fn the_edit_type_window_changes_name_height_and_layers_of_the_type_and_its_walls_in_one_step() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(crate::Geometry::Linear { level, .. }) => level, _ => panic!() };
        let t = d.add_layered_wall_type("Cavity", 3.0, cavity(), Some((1, 3))).unwrap();
        let other = d.add_wall_type("Plain", 0.2, 3.0).unwrap();
        let w = d.place_wall_typed(level, [20.0, 0.0], [26.0, 0.0], 0.1, 2.0, 0.0, &[], Some(t)).unwrap();
        let revision = d.revision();
        let mut l = cavity();
        l.remove(0);
        d.edit_wall_type(t, " Cavity 315 ", 3.5, l.clone(), Some((0, 2)), 0.0).unwrap();
        assert_eq!(d.revision(), revision + 1);
        let (ty, wall) = (d.get(t).unwrap(), d.get(w).unwrap());
        assert_eq!((ty.name.as_str(), ty.layers.clone(), ty.core, wall.height_m), ("Cavity 315", l, Some((0, 2)), 3.5));
        assert!((wall.width_m - 0.315).abs() < 1e-12 && wall.width_m == ty.width_m);
        // a name that is taken, layers that are not a wall, or a bad height change nothing
        assert!(d.edit_wall_type(t, "Plain", 3.0, cavity(), None, 0.0).is_err());
        assert!(d.edit_wall_type(t, "x", 3.0, vec![WallLayer::new("Foam", 0.1, Membrane)], None, 0.0).is_err());
        assert!(d.edit_wall_type(t, "x", 0.0, cavity(), None, 0.0).is_err());
        assert_eq!(d.revision(), revision + 1);
        // without layers it is a plain type of the thickness asked for
        d.edit_wall_type(t, "Cavity 315", 3.5, vec![], None, 0.25).unwrap();
        assert!(!d.get(t).unwrap().is_layered() && d.get(t).unwrap().core.is_none() && d.get(w).unwrap().width_m == 0.25);
        assert!(d.edit_wall_type(t, "Cavity 315", 3.5, vec![], None, 0.0).is_err());
        // a plain type can be given layers
        d.edit_wall_type(other, "Plain", 3.0, cavity(), None, 0.0).unwrap();
        assert!((d.get(other).unwrap().width_m - 0.335).abs() < 1e-12 && d.get(other).unwrap().is_layered());
        assert!(d.undo().unwrap() && !d.get(other).unwrap().is_layered());
    }

    #[test]
    fn a_plan_draws_a_line_where_each_layer_ends_counted_from_the_exterior_on_the_left() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(crate::Geometry::Linear { level, .. }) => level, _ => panic!() };
        let t = d.add_layered_wall_type("Cavity 335", 3.0, cavity(), Some((1, 3))).unwrap();
        let w = d.place_wall_typed(level, [20.0, 0.0], [26.0, 0.0], 0.1, 2.0, 0.0, &[], Some(t)).unwrap();
        let lines = d.wall_layer_lines(w);
        assert_eq!(lines.len(), 4, "five layers, four lines");
        // the wall runs along +x about y = 0: the exterior face (left) is at y = +0.1675, the render is 20 mm thick
        let ys: Vec<f64> = lines.iter().map(|l| l.0[1]).collect();
        let expect = [0.1675 - 0.02, 0.1675 - 0.12, 0.1675 - 0.17, 0.1675 - 0.32];
        for (y, e) in ys.iter().zip(expect) { assert!((y - e).abs() < 1e-9, "{ys:?}"); }
        assert!(lines.iter().all(|l| (l.0[1] - l.1[1]).abs() < 1e-9 && (l.1[0] - l.0[0] - 6.0).abs() < 1e-9));
        // the core (layers 1..=3) starts after the render and ends before the plaster
        assert_eq!(lines.iter().map(|l| l.2).collect::<Vec<_>>(), [true, false, false, true]);
        let plain = d.add_wall_type("Plain", 0.2, 3.0).unwrap();
        let p = d.place_wall_typed(level, [20.0, 3.0], [26.0, 3.0], 0.1, 2.0, 0.0, &[], Some(plain)).unwrap();
        assert!(d.wall_layer_lines(p).is_empty());
    }

    #[test]
    fn layers_of_a_type_are_kept_by_duplicating_it_and_by_the_records() {
        let mut d = Document::spatial_sample();
        let t = d.add_layered_wall_type("Cavity", 3.0, cavity(), Some((1, 3))).unwrap();
        let copy = d.duplicate_type(t, "Cavity copy").unwrap();
        assert_eq!((d.get(copy).unwrap().layers.clone(), d.get(copy).unwrap().core), (cavity(), Some((1, 3))));
        let lines = crate::native::records(&d);
        let (els, site) = crate::native::parse_records(&lines).unwrap();
        let mut back = Document::new();
        back.transact(0, &crate::native::restore_commands(els, site)).unwrap();
        let norm = |d: &Document| d.elements().map(|e| Element { revision: 0, ..e.clone() }).collect::<Vec<_>>();
        assert_eq!(norm(&d), norm(&back));
        // a material with every kind of character
        let mut odd = cavity();
        odd[0].material = "Render; 2 coats | a=b, c:d".into();
        odd[0].wraps = true;
        let o = d.add_layered_wall_type("Odd", 3.0, odd.clone(), None).unwrap();
        let (els, _) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        assert_eq!(els.iter().find(|e| e.id == o).unwrap().layers, odd);
    }
}
