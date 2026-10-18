//! Rooms, as Revit makes them: a room is a point in an enclosed region of a level, and its boundary is the faces of the
//! walls (and columns) around it. The boundary is worked out from the walls every time it is asked for, so a room
//! follows its walls when they move, and a region that is not closed holds no room.
use super::{Category, Command, Document, EditError, ElementId, Geometry};

type P = [f64; 2];

/// The floor of a room: its outline, and the outlines of what stands in it (columns, free walls).
#[derive(Clone, Debug, PartialEq)]
pub struct RoomShape {
    pub outline: Vec<P>,
    pub holes: Vec<Vec<P>>,
}

fn signed_area(p: &[P]) -> f64 { (0..p.len()).map(|i| { let (a, b) = (p[i], p[(i + 1) % p.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0 }

fn inside(poly: &[P], q: P) -> bool {
    let mut c = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] { c = !c; }
        j = i;
    }
    c
}

impl RoomShape {
    /// The floor area, holes taken out, in square metres.
    pub fn area(&self) -> f64 { signed_area(&self.outline).abs() - self.holes.iter().map(|h| signed_area(h).abs()).sum::<f64>() }
    /// The length of the boundary, holes included, in metres.
    pub fn perimeter(&self) -> f64 {
        std::iter::once(&self.outline).chain(self.holes.iter()).map(|r| (0..r.len()).map(|i| { let (a, b) = (r[i], r[(i + 1) % r.len()]); (b[0] - a[0]).hypot(b[1] - a[1]) }).sum::<f64>()).sum()
    }
    /// Whether `q` is in the room (inside the outline and outside the holes).
    pub fn contains(&self, q: P) -> bool { inside(&self.outline, q) && !self.holes.iter().any(|h| inside(h, q)) }
}

impl Document {
    /// The loops that bound what is not wall on `level`: counter-clockwise around a block of walls, clockwise around a
    /// closed space between them.
    fn void_loops(&self, level: ElementId) -> Vec<Vec<P>> {
        // the solids, each counter-clockwise
        let mut solids: Vec<Vec<P>> = Vec::new();
        for e in self.elements() {
            match (e.category, e.geometry) {
                (Category::Wall, Some(Geometry::Linear { level: l, .. })) if l == level => { if let Some(q) = self.wall_plan(e.id) { solids.push(q.to_vec()); } }
                (Category::Column, Some(Geometry::Point { at, level: l, .. })) if l == level => {
                    solids.push(e.profile().outline().into_iter().map(|p| [at[0] + p[0], at[1] + p[1]]).collect());
                }
                _ => {}
            }
        }
        for s in &mut solids { if signed_area(s) < 0.0 { s.reverse(); } }
        // the vertices, merged when they are within 10 micrometres
        let mut verts: Vec<P> = Vec::new();
        let index = |p: P, verts: &mut Vec<P>| -> usize {
            if let Some(i) = verts.iter().position(|v| (v[0] - p[0]).abs() < 1e-5 && (v[1] - p[1]).abs() < 1e-5) { return i; }
            verts.push(p);
            verts.len() - 1
        };
        let rings: Vec<Vec<usize>> = solids.iter().map(|s| s.iter().map(|p| index(*p, &mut verts)).collect()).collect();
        // the directed edges, split at the vertices that lie on them
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for r in &rings {
            for k in 0..r.len() {
                let (a, b) = (r[k], r[(k + 1) % r.len()]);
                if a == b { continue; }
                let (pa, pb) = (verts[a], verts[b]);
                let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
                let l2 = dx * dx + dy * dy;
                let mut cuts: Vec<(f64, usize)> = verts.iter().enumerate().filter_map(|(i, v)| {
                    if i == a || i == b { return None; }
                    let t = ((v[0] - pa[0]) * dx + (v[1] - pa[1]) * dy) / l2;
                    let off = ((v[0] - pa[0]) * -dy + (v[1] - pa[1]) * dx) / l2.sqrt();
                    (t > 1e-9 && t < 1.0 - 1e-9 && off.abs() < 1e-5).then_some((t, i))
                }).collect();
                cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
                let mut from = a;
                for (_, i) in cuts { edges.push((from, i)); from = i; }
                edges.push((from, b));
            }
        }
        // an edge that a neighbour runs along the other way is inside the walls
        let mut kept: Vec<(usize, usize)> = Vec::new();
        for e in edges {
            if let Some(pos) = kept.iter().position(|k| *k == (e.1, e.0)) { kept.swap_remove(pos); } else { kept.push(e); }
        }
        // trace: at a vertex take the edge first met turning counter-clockwise from the way back
        let mut used = vec![false; kept.len()];
        let mut loops = Vec::new();
        for s in 0..kept.len() {
            if used[s] { continue; }
            let mut ring = Vec::new();
            let mut cur = s;
            loop {
                used[cur] = true;
                let (a, b) = kept[cur];
                ring.push(verts[a]);
                let back = (verts[a][1] - verts[b][1]).atan2(verts[a][0] - verts[b][0]);
                let next = (0..kept.len()).filter(|&k| kept[k].0 == b && !used[k] || (k == s && kept[k].0 == b)).min_by(|&x, &y| {
                    let turn = |k: usize| { let t = (verts[kept[k].1][1] - verts[b][1]).atan2(verts[kept[k].1][0] - verts[b][0]); let mut d = t - back; while d <= 1e-12 { d += std::f64::consts::TAU; } d };
                    turn(x).total_cmp(&turn(y))
                });
                match next { Some(k) if k != s => cur = k, _ => break }
            }
            if ring.len() >= 3 { loops.push(ring); }
        }
        loops
    }

    /// The floor of the closed space on `level` that holds `p`, bounded by the faces of the walls and columns around it.
    pub fn room_region_at(&self, level: ElementId, p: P) -> Result<RoomShape, String> {
        let loops = self.void_loops(level);
        let outline = loops.iter().filter(|l| signed_area(l) < -1e-9 && inside(l, p)).min_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs()))
            .ok_or("the point is not in a closed space: the walls around it do not enclose it")?.clone();
        let mut outline = outline;
        outline.reverse();
        let holes = loops.iter().filter(|l| signed_area(l) > 1e-9 && l.iter().all(|q| inside(&outline, *q))).cloned().collect();
        Ok(RoomShape { outline, holes })
    }

    /// The floor of the room `id`, from the walls around its point as they are now.
    pub fn room_shape(&self, id: ElementId) -> Result<RoomShape, String> {
        match self.get(id).and_then(|e| e.geometry) {
            Some(Geometry::Room { at, level }) => self.room_region_at(level, at),
            _ => Err("not a room".into()),
        }
    }

    /// Places a room at `at` on `level`: the closed space that holds the point. A space has one room.
    pub fn place_room(&mut self, level: ElementId, at: P) -> Result<ElementId, String> {
        let shape = self.room_region_at(level, at)?;
        if let Some(other) = self.elements().find(|e| matches!(e.geometry, Some(Geometry::Room { at: o, level: l }) if l == level && shape.contains(o))) {
            return Err(format!("{} is in this space already", other.name));
        }
        let id = self.next_id();
        let n = self.elements().filter(|e| e.category == Category::Room).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Room, name: format!("Room {n}"), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Room { at, level } },
        ]).map_err(|e: EditError| format!("{e:?}"))?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 1e-6 }

    #[test]
    fn a_room_is_the_closed_space_inside_the_wall_faces_and_follows_its_walls() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        // the sample is a 8 x 6 room of 200 mm walls (axes) with an inner wall at x = 4; open it: use a fresh closed box far away
        let ids = d.place_walls(level, &[([100.0, 0.0], [106.0, 0.0]), ([106.0, 0.0], [106.0, 4.0]), ([106.0, 4.0], [100.0, 4.0]), ([100.0, 4.0], [100.0, 0.0])], 0.2, 3.0, 0.0, None).unwrap();
        let shape = d.room_region_at(level, [103.0, 2.0]).unwrap();
        // inside faces: 5.8 x 3.8
        assert!(near(shape.area(), 5.8 * 3.8) && near(shape.perimeter(), 2.0 * (5.8 + 3.8)), "{} {:?}", shape.area(), shape.outline);
        assert!(shape.holes.is_empty() && shape.contains([103.0, 2.0]) && !shape.contains([99.0, 2.0]));
        // a point in a wall, and a point outside, are not in a room
        assert!(d.room_region_at(level, [100.0, 2.0]).is_err());
        assert!(d.room_region_at(level, [90.0, 2.0]).unwrap_err().contains("closed"));
        // placing it: one room per space
        let room = d.place_room(level, [101.5, 1.0]).unwrap();
        assert!(d.place_room(level, [103.0, 2.0]).unwrap_err().contains("already"));
        assert!(near(d.room_shape(room).unwrap().area(), 5.8 * 3.8));
        // an inner wall that divides the space makes the room smaller, and a second room fits on the other side
        let wall = d.place_wall(level, [103.0, 0.0], [103.0, 4.0], 0.2, 3.0, 0.0).unwrap();
        assert!(near(d.room_shape(room).unwrap().area(), 2.8 * 3.8), "{}", d.room_shape(room).unwrap().area());
        let second = d.place_room(level, [105.0, 2.0]).unwrap();
        assert!(near(d.room_shape(second).unwrap().area(), 2.8 * 3.8));
        // a column that stands in the room is a hole
        let top = ElementId(11);
        d.place_column(level, top, [101.5, 2.5], &crate::ColumnSpec { width_m: 0.4, depth_m: 0.4, base_offset_m: 0.0, section: crate::Section::Rectangle, material: crate::Material::Concrete }).unwrap();
        let s = d.room_shape(room).unwrap();
        assert_eq!(s.holes.len(), 1);
        assert!(near(s.area(), 2.8 * 3.8 - 0.16), "{}", s.area());
        assert!(!s.contains([101.5, 2.5]));
        // the native records carry the room and bring it back
        let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        let mut back = Document::new();
        back.transact(back.revision(), &crate::native::restore_commands(els, site)).unwrap();
        assert!(near(back.room_shape(room).unwrap().area(), s.area()));
        // opening the box (deleting a wall) leaves no closed space
        let ops = d.delete_commands(&[ids[3], wall]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(d.room_shape(room).is_err());
        d.undo().unwrap();
        assert!(d.room_shape(room).is_ok());
        // a wall moved: the room follows
        let ops = d.transform_commands(&[wall], crate::Xf::Move([1.0, 0.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(near(d.room_shape(room).unwrap().area(), 3.8 * 3.8 - 0.16), "{}", d.room_shape(room).unwrap().area());
    }
}
