//! Editing in an elevation or section view, as Revit does: the view is a plane, so a change is a distance along it and
//! a distance up. Doors, windows and openings slide in their wall; walls, beams, slabs and stairs also change their
//! height offset; a mirror is about a vertical line; a rotation turns about an axis square to the view. Each function
//! gives the commands of one undoable step; the model is not changed until they are applied.
use super::{Category, Command, Document, ElementId, Geometry, ViewRef};
use crate::edit::Xf;

type P = [f64; 2];

/// Where a view stands in the plan: `origin` is the start of its line and `dir` the unit vector along which its u grows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewFrame { pub origin: P, pub dir: P }

impl ViewFrame {
    /// The frame of a view whose line runs from `a` to `b`; none when the line has no length.
    pub fn from_line(a: P, b: P) -> Option<ViewFrame> {
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        (l > 1e-9).then(|| ViewFrame { origin: a, dir: [(b[0] - a[0]) / l, (b[1] - a[1]) / l] })
    }
    /// The u of a point of the plan.
    pub fn u(&self, p: P) -> f64 { (p[0] - self.origin[0]) * self.dir[0] + (p[1] - self.origin[1]) * self.dir[1] }
    /// The point of the plan on the view line at `u`.
    pub fn at(&self, u: f64) -> P { [self.origin[0] + self.dir[0] * u, self.origin[1] + self.dir[1] * u] }
}

/// An edge picked in a view: a vertical one at `u`, or a horizontal one at height `z`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewEdge { Vertical(f64), Horizontal(f64) }

/// Whether moving an element up and down changes its own height offset.
fn moves_up(c: Category) -> bool { matches!(c, Category::Wall | Category::Beam | Category::Slab | Category::Stair) }

impl Document {
    /// The direction of the wall that hosts `id`, and the u of the wall's start in the frame.
    fn host_in_view(&self, host: ElementId, f: ViewFrame) -> Result<(P, f64, f64), String> {
        let Some(Geometry::Linear { start, end, .. }) = self.get(host).and_then(|w| w.geometry) else { return Err("its wall is gone".into()) };
        let len = (end[0] - start[0]).hypot(end[1] - start[1]);
        let w = [(end[0] - start[0]) / len, (end[1] - start[1]) / len];
        Ok((w, w[0] * f.dir[0] + w[1] * f.dir[1], f.u(start)))
    }

    /// Moves or copies `ids` in a view: `du` along it and `dz` up. Returns the commands and, for a copy, the numbers
    /// of the copies. What stands in a moved wall goes with it.
    pub fn view_move_commands(&self, ids: &[ElementId], f: ViewFrame, du: f64, dz: f64, copy: bool) -> Result<(Vec<Command>, Vec<ElementId>), String> {
        if !(du.is_finite() && dz.is_finite()) { return Err("not a distance".into()); }
        let mut ops = Vec::new();
        let (mut plan, mut stuck) = (Vec::new(), false);
        let mut next = self.next_id().0;
        let mut made = Vec::new();
        for &id in ids {
            let e = self.get(id).ok_or("an element is gone")?;
            // what stands in a wall that moves with it follows by itself
            if matches!(e.geometry, Some(Geometry::InWall { host, .. }) if ids.contains(&host)) { continue; }
            match e.geometry {
                Some(Geometry::InWall { host, along_m }) => {
                    // a wall at an angle to the view: the insert slides along the wall, so that its drawing moves by `du`
                    let (_, c, _) = self.host_in_view(host, f)?;
                    if c.abs() < 0.05 { return Err(format!("{} is in a wall seen nearly end on in this view; use a view that looks at the wall", e.name)); }
                    let along = along_m + du / c;
                    if copy {
                        let mut n = e.clone();
                        n.id = ElementId(next); next += 1;
                        n.geometry = Some(Geometry::InWall { host, along_m: along });
                        n.elevation_m += dz;
                        made.push(n.id);
                        ops.push(Command::Restore(Box::new(n)));
                    } else {
                        if du != 0.0 { ops.push(Command::SetGeometry { id, geometry: Geometry::InWall { host, along_m: along } }); }
                        if dz != 0.0 { ops.push(Command::MoveElevation { id, elevation_m: e.elevation_m + dz }); }
                    }
                }
                _ => {
                    if copy || du != 0.0 { plan.push(id); }
                    if dz != 0.0 && !moves_up(e.category) { stuck = true; }
                }
            }
        }
        if !plan.is_empty() {
            let xf = Xf::Move([f.dir[0] * du, f.dir[1] * du]);
            if copy {
                let (mut copies, firsts) = self.copy_commands(&plan, xf)?;
                for (orig, new) in plan.iter().zip(&firsts) {
                    if dz == 0.0 || !self.get(*orig).is_some_and(|e| moves_up(e.category)) { continue; }
                    for op in &mut copies { if let Command::Restore(e) = op { if e.id == *new { e.elevation_m += dz; } } }
                }
                ops.extend(copies);
                made.extend(firsts);
            } else {
                ops.extend(self.transform_commands(&plan, xf)?);
            }
        }
        if !copy {
            for &id in ids {
                let Some(e) = self.get(id) else { continue };
                if dz != 0.0 && moves_up(e.category) && !matches!(e.geometry, Some(Geometry::InWall { .. })) { ops.push(Command::MoveElevation { id, elevation_m: e.elevation_m + dz }); }
            }
        }
        if ops.is_empty() { return Err(if stuck { "this cannot be moved up or down in a view; change its offset or its levels in Properties".into() } else { "nothing changes".into() }); }
        Ok((ops, made))
    }

    /// Like `view_move_commands`, for the view `view`: the tags of the moved elements in that view go with them, and a
    /// tag or a dimension of the view that is itself in `ids` moves by (`du`, `dz`) on its own.
    pub fn view_move_in(&self, ids: &[ElementId], f: ViewFrame, view: ViewRef, du: f64, dz: f64, copy: bool) -> Result<(Vec<Command>, Vec<ElementId>), String> {
        let is_note = |i: &ElementId| self.get(*i).is_some_and(|e| matches!(e.geometry, Some(Geometry::ViewTag { .. } | Geometry::ViewDimension { .. })));
        let (notes, rest): (Vec<ElementId>, Vec<ElementId>) = ids.iter().copied().partition(is_note);
        let shifted = |g: Geometry| match g {
            Geometry::ViewTag { host, view, at, anchor } => Geometry::ViewTag { host, view, at: [at[0] + du, at[1] + dz], anchor: [anchor[0] + du, anchor[1] + dz] },
            Geometry::ViewDimension { view, a, b, at } => Geometry::ViewDimension { view, a: [a[0] + du, a[1] + dz], b: [b[0] + du, b[1] + dz], at: [at[0] + du, at[1] + dz] },
            other => other,
        };
        let (mut ops, mut made) = if rest.is_empty() { (Vec::new(), Vec::new()) } else { self.view_move_commands(&rest, f, du, dz, copy)? };
        let here = |g: Option<Geometry>| matches!(g, Some(Geometry::ViewTag { view: v, .. } | Geometry::ViewDimension { view: v, .. }) if v == view);
        if copy {
            // the tags that came with the copies of elements are copies in this view too: they go with them
            for op in &mut ops { if let Command::Restore(e) = op { if here(e.geometry) { e.geometry = e.geometry.map(shifted); } } }
            let next = ops.iter().filter_map(|o| if let Command::Restore(e) = o { Some(e.id.0 + 1) } else { None }).max().unwrap_or(0).max(self.next_id().0);
            for (id, number) in notes.into_iter().zip(next..) {
                let mut n = self.get(id).ok_or("an annotation is gone")?.clone();
                n.id = ElementId(number);
                n.geometry = n.geometry.map(shifted);
                made.push(n.id);
                ops.push(Command::Restore(Box::new(n)));
            }
        } else {
            let moving = self.with_dependents(&rest);
            for t in self.elements() {
                let follows = matches!(t.geometry, Some(Geometry::ViewTag { host, .. }) if moving.contains(&host)) && here(t.geometry) && !notes.contains(&t.id);
                if follows || notes.contains(&t.id) { if let Some(g) = t.geometry { ops.push(Command::SetGeometry { id: t.id, geometry: shifted(g) }); } }
            }
        }
        if ops.is_empty() { return Err("nothing changes".into()); }
        Ok((ops, made))
    }

    /// Mirrors `ids` about the vertical line at `u_axis` of a view, keeping the originals when `copy`.
    pub fn view_mirror_commands(&self, ids: &[ElementId], f: ViewFrame, u_axis: f64, copy: bool) -> Result<(Vec<Command>, Vec<ElementId>), String> {
        let mut ops = Vec::new();
        let (mut plan, mut made) = (Vec::new(), Vec::new());
        let mut next = self.next_id().0;
        for &id in ids {
            let e = self.get(id).ok_or("an element is gone")?;
            if matches!(e.geometry, Some(Geometry::InWall { host, .. }) if ids.contains(&host)) { continue; }
            match e.geometry {
                Some(Geometry::InWall { host, along_m }) => {
                    // an insert mirrored on its own stays in its wall, on the other side of the axis
                    let (_, c, u_start) = self.host_in_view(host, f)?;
                    if c.abs() < 0.05 { return Err(format!("{} is in a wall seen nearly end on in this view", e.name)); }
                    let s = (u_axis - u_start) / c;
                    let along = 2.0 * s - along_m;
                    if copy {
                        let mut n = e.clone();
                        n.id = ElementId(next); next += 1;
                        n.geometry = Some(Geometry::InWall { host, along_m: along });
                        made.push(n.id);
                        ops.push(Command::Restore(Box::new(n)));
                    } else { ops.push(Command::SetGeometry { id, geometry: Geometry::InWall { host, along_m: along } }); }
                }
                _ => plan.push(id),
            }
        }
        if !plan.is_empty() {
            // a vertical line of the view is a line across it in the plan
            let (p, q) = (f.at(u_axis), [f.at(u_axis)[0] - f.dir[1], f.at(u_axis)[1] + f.dir[0]]);
            let xf = Xf::Mirror(p, q);
            if copy { let (o, n) = self.copy_commands(&plan, xf)?; ops.extend(o); made.extend(n); }
            else { ops.extend(self.transform_commands(&plan, xf)?); }
        }
        if ops.is_empty() { return Err("nothing changes".into()); }
        Ok((ops, made))
    }

    /// Turns `ids` by `angle` (radians, counter-clockwise as the view is seen) about the axis square to the view, through
    /// the middle of each. As in Revit, walls do not rotate in an elevation, and a door or window cannot without its
    /// wall. A beam or brace parallel to the view tilts: its slope changes about its middle.
    pub fn view_rotate_commands(&self, ids: &[ElementId], f: ViewFrame, angle: f64) -> Result<Vec<Command>, String> {
        if !angle.is_finite() || angle.abs() < 1e-9 { return Err("nothing changes".into()); }
        let mut ops = Vec::new();
        for &id in ids {
            let e = self.get(id).ok_or("an element is gone")?;
            match (e.category, e.geometry) {
                (Category::Wall, _) => return Err(format!("{} cannot rotate in an elevation or section: walls do not rotate in these views", e.name)),
                (_, Some(Geometry::InWall { .. })) => return Err(format!("{} cannot rotate without its host wall", e.name)),
                (Category::Beam, Some(Geometry::Linear { start, end, .. })) => {
                    let len = (end[0] - start[0]).hypot(end[1] - start[1]);
                    let w = [(end[0] - start[0]) / len, (end[1] - start[1]) / len];
                    if (w[0] * f.dir[1] - w[1] * f.dir[0]).abs() > 0.02 { return Err(format!("{} is not parallel to this view, so it cannot be turned in it", e.name)); }
                    let s = (w[0] * f.dir[0] + w[1] * f.dir[1]).signum();
                    let old = e.slope_ratio.unwrap_or(0.0);
                    let turned = old.atan() + s * angle;
                    if turned.abs() > 80f64.to_radians() { return Err(format!("{} would be steeper than 80 degrees", e.name)); }
                    let slope = turned.tan();
                    let mid_z = e.elevation_m + old * len / 2.0;
                    ops.push(Command::SetSlope { id, slope_ratio: slope });
                    ops.push(Command::MoveElevation { id, elevation_m: mid_z - slope * len / 2.0 });
                }
                _ => return Err(format!("{} cannot be turned in a view yet; turn it in a plan view", e.name)),
            }
        }
        if ops.is_empty() { return Err("nothing selected".into()); }
        Ok(ops)
    }

    /// Aligns the edge `picked` of element `id` with the `reference` edge of a view. A wall end goes to the reference
    /// (the wall gets longer or shorter, and walls joined there follow); the top or the base of a wall moves (the wall
    /// gets taller or lower); anything else moves, as Revit moves a window to a line.
    pub fn view_align_commands(&self, id: ElementId, f: ViewFrame, picked: ViewEdge, reference: ViewEdge) -> Result<Vec<Command>, String> {
        let e = self.get(id).ok_or("an element is gone")?;
        match (picked, reference) {
            (ViewEdge::Vertical(from), ViewEdge::Vertical(to)) => {
                let du = to - from;
                if du.abs() < 1e-9 { return Err("it is already aligned".into()); }
                if let (Category::Wall, Some(Geometry::Linear { start, end, .. })) = (e.category, e.geometry) {
                    let len = (end[0] - start[0]).hypot(end[1] - start[1]);
                    let w = [(end[0] - start[0]) / len, (end[1] - start[1]) / len];
                    let parallel = (w[0] * f.dir[1] - w[1] * f.dir[0]).abs() <= 0.02;
                    let (us, ue) = (f.u(start), f.u(end));
                    let (ds, de) = ((us - from).abs(), (ue - from).abs());
                    if parallel && ds.min(de) < 0.1 {
                        let at_start = ds <= de;
                        let old = if at_start { start } else { end };
                        return self.wall_end_commands(id, at_start, [old[0] + f.dir[0] * du, old[1] + f.dir[1] * du]);
                    }
                }
                self.view_move_commands(&[id], f, du, 0.0, false).map(|r| r.0)
            }
            (ViewEdge::Horizontal(from), ViewEdge::Horizontal(to)) => {
                let dz = to - from;
                if dz.abs() < 1e-9 { return Err("it is already aligned".into()); }
                if let (Category::Wall, Some(Geometry::Linear { level, .. })) = (e.category, e.geometry) {
                    let base = self.get(level).map_or(0.0, |l| l.elevation_m) + e.elevation_m;
                    let top = base + e.height_m;
                    if (from - top).abs() < 0.02 {
                        if e.height_m + dz < 0.05 { return Err("the wall would be left with no height".into()); }
                        return Ok(vec![Command::Resize { id, width_m: e.width_m, height_m: e.height_m + dz }]);
                    }
                    if (from - base).abs() < 0.02 {
                        if e.height_m - dz < 0.05 { return Err("the wall would be left with no height".into()); }
                        return Ok(vec![Command::MoveElevation { id, elevation_m: e.elevation_m + dz }, Command::Resize { id, width_m: e.width_m, height_m: e.height_m - dz }]);
                    }
                }
                self.view_move_commands(&[id], f, 0.0, dz, false).map(|r| r.0)
            }
            _ => Err("pick a vertical edge to align with a vertical one, or a horizontal edge with a horizontal one".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FamilySpec;

    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }
    fn door_in_wall(d: &mut Document, at: f64) -> ElementId {
        let spec = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        d.place_family_in_wall(ElementId(20), at, &spec).unwrap()
    }
    fn along(d: &Document, id: ElementId) -> f64 { match d.get(id).unwrap().geometry { Some(Geometry::InWall { along_m, .. }) => along_m, g => panic!("{g:?}") } }
    fn run(d: &mut Document, ops: Vec<Command>) { d.transact(d.revision(), &ops).unwrap(); }
    // looking north from the south of the sample: u grows with x
    fn south() -> ViewFrame { ViewFrame::from_line([0.0, -2.0], [8.0, -2.0]).unwrap() }

    #[test]
    fn things_move_in_a_view_along_it_and_up_and_a_door_slides_in_its_wall() {
        let mut d = Document::spatial_sample();
        let door = door_in_wall(&mut d, 3.0);
        let (a0, sill0) = (along(&d, door), d.get(door).unwrap().elevation_m);
        let (ops, _) = d.view_move_commands(&[door], south(), 1.0, 0.2, false).unwrap();
        run(&mut d, ops);
        assert!(near(along(&d, door), a0 + 1.0) && near(d.get(door).unwrap().elevation_m, sill0 + 0.2));
        d.undo().unwrap();
        // seen from the north (u grows against x) the same drag to the right slides it the other way
        let north = ViewFrame::from_line([8.0, 8.0], [0.0, 8.0]).unwrap();
        let (ops, _) = d.view_move_commands(&[door], north, 1.0, 0.0, false).unwrap();
        run(&mut d, ops);
        assert!(near(along(&d, door), a0 - 1.0));
        d.undo().unwrap();
        // a door pushed out of its wall is refused by the model
        let (ops, _) = d.view_move_commands(&[door], south(), 50.0, 0.0, false).unwrap();
        assert!(d.transact(d.revision(), &ops).is_err());
        // the wall moves along the view and up, and its door goes with it; chosen together the door is not moved twice
        let (ops, _) = d.view_move_commands(&[ElementId(20), door], south(), 2.0, 0.3, false).unwrap();
        run(&mut d, ops);
        assert!(near(d.get(ElementId(20)).unwrap().elevation_m, 0.3) && near(along(&d, door), a0));
        match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { start, .. }) => assert!(near(start[0], 2.0)), g => panic!("{g:?}") }
        d.undo().unwrap();
        // a column goes sideways only; up and down alone is refused
        let col = d.place_column(ElementId(10), ElementId(11), [4.0, 4.0], &crate::ColumnSpec { width_m: 0.3, depth_m: 0.3, base_offset_m: 0.0, section: crate::Section::Rectangle, material: crate::Material::Concrete }).unwrap();
        assert!(d.view_move_commands(&[col], south(), 0.0, 0.5, false).is_err());
        assert!(d.view_move_commands(&[col], south(), 1.0, 0.5, false).is_ok());
    }

    #[test]
    fn a_door_in_a_wall_at_an_angle_to_the_view_slides_so_that_its_drawing_moves_by_the_drag() {
        let mut d = Document::spatial_sample();
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        // a wall at 60 degrees to the view line (cos = 0.5): a drag of 0.5 m along the view is 1 m along the wall
        let w = d.place_wall(level, [10.0, 0.0], [10.0 + 10.0 * 0.5, 10.0 * 0.75f64.sqrt()], 0.2, 3.0, 0.0).unwrap();
        let spec = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        let door = d.place_family_in_wall(w, 4.0, &spec).unwrap();
        let (ops, _) = d.view_move_commands(&[door], south(), 0.5, 0.0, false).unwrap();
        run(&mut d, ops);
        assert!(near(along(&d, door), 5.0), "{}", along(&d, door));
        // a wall seen almost end on has no place for a slide
        let n = d.place_wall(level, [20.0, 0.0], [20.0, 6.0], 0.2, 3.0, 0.0).unwrap();
        let door2 = d.place_family_in_wall(n, 3.0, &spec).unwrap();
        assert!(d.view_move_commands(&[door2], south(), 0.5, 0.0, false).is_err());
    }

    #[test]
    fn a_copy_in_a_view_keeps_the_original_and_a_copied_door_stands_in_the_same_wall() {
        let mut d = Document::spatial_sample();
        let door = door_in_wall(&mut d, 3.0);
        let count = d.elements().count();
        let (ops, new) = d.view_move_commands(&[door], south(), 2.0, 0.0, true).unwrap();
        run(&mut d, ops);
        assert_eq!((d.elements().count(), new.len()), (count + 1, 1));
        assert!(near(along(&d, door), 3.0) && near(along(&d, new[0]), 5.0));
        // a beam copied 2 m along and 1 m up
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let beam = d.place_beam(level, &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.5)).unwrap();
        let (ops, new) = d.view_move_commands(&[beam], south(), 2.0, 1.0, true).unwrap();
        run(&mut d, ops);
        let c = d.get(new[0]).unwrap();
        assert!(near(c.elevation_m, 3.5) && matches!(c.geometry, Some(Geometry::Linear { start, .. }) if near(start[0], 3.0)));
        assert!(near(d.get(beam).unwrap().elevation_m, 2.5));
    }

    #[test]
    fn mirroring_in_a_view_is_about_a_vertical_line() {
        let mut d = Document::spatial_sample();
        let door = door_in_wall(&mut d, 3.0);
        // a door mirrored on its own about u = 4 (the middle of the 8 m wall) goes to 5 m, and a copy leaves it
        let (ops, new) = d.view_mirror_commands(&[door], south(), 4.0, true).unwrap();
        run(&mut d, ops);
        assert!(near(along(&d, door), 3.0) && near(along(&d, new[0]), 5.0));
        let (ops, _) = d.view_mirror_commands(&[door], south(), 4.0, false).unwrap();
        run(&mut d, ops);
        assert!(near(along(&d, door), 5.0));
        // a column mirrors across the line in the plan
        let col = d.place_column(ElementId(10), ElementId(11), [1.0, 4.0], &crate::ColumnSpec { width_m: 0.3, depth_m: 0.3, base_offset_m: 0.0, section: crate::Section::Rectangle, material: crate::Material::Concrete }).unwrap();
        let (ops, new) = d.view_mirror_commands(&[col], south(), 4.0, true).unwrap();
        run(&mut d, ops);
        assert!(matches!(d.get(new[0]).unwrap().geometry, Some(Geometry::Point { at, .. }) if near(at[0], 7.0) && near(at[1], 4.0)));
    }

    #[test]
    fn rotating_in_a_view_follows_revit_walls_and_inserts_do_not_turn_and_a_beam_tilts_about_its_middle() {
        let mut d = Document::spatial_sample();
        let door = door_in_wall(&mut d, 3.0);
        let wall_err = d.view_rotate_commands(&[ElementId(20)], south(), 0.3).unwrap_err();
        assert!(wall_err.contains("walls do not rotate"), "{wall_err}");
        assert!(d.view_rotate_commands(&[door], south(), 0.3).unwrap_err().contains("without its host wall"));
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let beam = d.place_beam(level, &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.5)).unwrap();
        // 45 degrees counter-clockwise as seen from the south (towards +u): the east end rises, the middle stays at 2.5 m
        let ops = d.view_rotate_commands(&[beam], south(), std::f64::consts::FRAC_PI_4).unwrap();
        run(&mut d, ops);
        let b = d.get(beam).unwrap();
        assert!(near(b.slope_ratio.unwrap(), 1.0) && near(b.elevation_m, 2.5 - 2.0), "{:?} {}", b.slope_ratio, b.elevation_m);
        // seen from the north the east end is on the left: a counter-clockwise turn there lowers it again
        let north = ViewFrame::from_line([8.0, 8.0], [0.0, 8.0]).unwrap();
        let ops = d.view_rotate_commands(&[beam], north, std::f64::consts::FRAC_PI_4).unwrap();
        run(&mut d, ops);
        assert!(d.get(beam).unwrap().slope_ratio.unwrap().abs() < 1e-9);
        assert!(near(d.get(beam).unwrap().elevation_m, 2.5));
        // a beam across the view cannot be turned in it
        let across = d.place_beam(level, &crate::BeamSpec::level([2.0, 1.0], [2.0, 5.0], 0.2, 0.4, 2.5)).unwrap();
        assert!(d.view_rotate_commands(&[across], south(), 0.2).unwrap_err().contains("not parallel"));
    }

    #[test]
    fn align_in_a_view_moves_an_insert_a_wall_end_or_the_top_of_a_wall_onto_the_reference() {
        let mut d = Document::spatial_sample();
        let door = door_in_wall(&mut d, 3.0);
        // a vertical edge of the door onto u = 5: the door slides 1 m (its edge was at 3 - half its width... any edge: the distance counts)
        let ops = d.view_align_commands(door, south(), ViewEdge::Vertical(2.0), ViewEdge::Vertical(3.0)).unwrap();
        run(&mut d, ops);
        assert!(near(along(&d, door), 4.0));
        d.undo().unwrap();
        // the end of a wall goes to the reference: the wall gets longer, and the walls joined there follow
        let line = |d: &Document| match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { start, end, .. }) => (start, end), g => panic!("{g:?}") };
        let (s0, e0) = line(&d);
        let ops = d.view_align_commands(ElementId(20), south(), ViewEdge::Vertical(e0[0]), ViewEdge::Vertical(e0[0] + 1.5)).unwrap();
        run(&mut d, ops);
        assert!(near(line(&d).1[0], e0[0] + 1.5) && near(line(&d).0[0], s0[0]));
        d.undo().unwrap();
        // the top of a wall onto a line 0.5 m higher: the wall is 0.5 m taller; its base onto a line 0.2 m up: lower by 0.2
        let h0 = d.get(ElementId(20)).unwrap().height_m;
        let ops = d.view_align_commands(ElementId(20), south(), ViewEdge::Horizontal(h0), ViewEdge::Horizontal(h0 + 0.5)).unwrap();
        run(&mut d, ops);
        assert!(near(d.get(ElementId(20)).unwrap().height_m, h0 + 0.5));
        d.undo().unwrap();
        let ops = d.view_align_commands(ElementId(20), south(), ViewEdge::Horizontal(0.0), ViewEdge::Horizontal(0.2)).unwrap();
        run(&mut d, ops);
        let w = d.get(ElementId(20)).unwrap();
        assert!(near(w.elevation_m, 0.2) && near(w.height_m, h0 - 0.2));
        // an insert goes up as a whole, like a window to a line; mixed edges are refused
        let ops = d.view_align_commands(door, south(), ViewEdge::Horizontal(2.1), ViewEdge::Horizontal(2.4)).unwrap();
        run(&mut d, ops);
        assert!(near(d.get(door).unwrap().elevation_m, 0.3));
        assert!(d.view_align_commands(door, south(), ViewEdge::Horizontal(1.0), ViewEdge::Vertical(1.0)).is_err());
    }

    #[test]
    fn tags_and_dimensions_belong_to_one_view_follow_their_element_there_and_go_with_what_they_are_on() {
        let mut d = Document::spatial_sample();
        let east = ViewRef::Elevation(0);
        let tag = d.place_view_tag(ElementId(20), east, [3.0, 4.0], [2.0, 1.5]).unwrap();
        let dim = d.place_view_dimension(east, [1.0, 2.0], [4.0, 2.0], [2.5, 3.0]).unwrap();
        // a vertical dimension is allowed, a slanted one and one in a view that does not exist are not
        assert!(d.place_view_dimension(east, [1.0, 0.0], [1.0, 3.0], [2.0, 1.0]).is_ok());
        assert!(d.place_view_dimension(east, [1.0, 0.0], [2.0, 3.0], [2.0, 1.0]).is_err());
        assert!(d.place_view_dimension(ViewRef::Section(ElementId(999)), [1.0, 2.0], [4.0, 2.0], [2.5, 3.0]).is_err());
        assert!(d.place_view_tag(ElementId(999), east, [0.0, 0.0], [0.0, 0.0]).is_err());
        assert!(!d.tag_text(tag).is_empty());
        // the tag is not drawn in the plan: it has no level of its own plan symbol, only its host's
        // moving the wall in this view carries the tag of that view with it, not the tag of another view
        let other = d.place_view_tag(ElementId(20), ViewRef::Elevation(1), [3.0, 4.0], [2.0, 1.5]).unwrap();
        let (ops, _) = d.view_move_in(&[ElementId(20)], south(), east, 1.0, 0.5, false).unwrap();
        run(&mut d, ops);
        assert!(matches!(d.get(tag).unwrap().geometry, Some(Geometry::ViewTag { at, anchor, .. }) if near(at[0], 4.0) && near(at[1], 4.5) && near(anchor[0], 3.0) && near(anchor[1], 2.0)));
        assert!(matches!(d.get(other).unwrap().geometry, Some(Geometry::ViewTag { at, .. }) if near(at[0], 3.0) && near(at[1], 4.0)));
        // a dimension moves on its own, and copies of it and of a tag are made on the same view
        let (ops, made) = d.view_move_in(&[dim, tag], south(), east, 0.5, 0.0, true).unwrap();
        run(&mut d, ops);
        assert_eq!(made.len(), 2);
        assert!(matches!(d.get(made[0]).unwrap().geometry, Some(Geometry::ViewDimension { a, .. }) if near(a[0], 1.5)) || matches!(d.get(made[1]).unwrap().geometry, Some(Geometry::ViewDimension { a, .. }) if near(a[0], 1.5)));
        // saved and read back unchanged
        let (back, _) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        for id in [tag, dim, other] { assert_eq!(back.iter().find(|e| e.id == id).unwrap().geometry, d.get(id).unwrap().geometry); }
        // deleting the wall takes its tags (in every view), deleting a section line takes what was drawn in its view
        let sec = d.place_section(ElementId(10), [0.0, -2.0], [8.0, -2.0], 20.0).unwrap();
        let in_sec = d.place_view_dimension(ViewRef::Section(sec), [1.0, 2.0], [4.0, 2.0], [2.5, 3.0]).unwrap();
        let ops = d.delete_commands(&[sec]).unwrap();
        run(&mut d, ops);
        assert!(d.get(in_sec).is_none() && d.get(dim).is_some());
        let ops = d.delete_commands(&[ElementId(20)]).unwrap();
        run(&mut d, ops);
        assert!(d.get(tag).is_none() && d.get(other).is_none() && d.get(dim).is_some());
    }
}
