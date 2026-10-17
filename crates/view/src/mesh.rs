//! Triangle meshes of the model, one per element, for a 3D viewport.
//!
//! Every mesh is split into parts: runs of triangles that share a face kind (top, bottom, side, or for walls the
//! exterior and interior face) and a material, so the viewport can colour them and the pick buffer can name the
//! element. `Batch` joins the meshes into one set of buffers with an element index per triangle.
//!
//! `MeshCache` keeps the meshes between edits. It decides what to rebuild by comparing the inputs of each element
//! with the ones it meshed last time, so it does not matter how the scene was changed: only elements whose inputs
//! differ are meshed again, and meshing is done in parallel. Output order is walls, slabs, columns, beams, family items,
//! each in scene order, and does not depend on the number of threads.

use crate::{slab_holes, Element, Kind, ViewError};
use openbim_family::{forms::Loop, mep::up_for, Instance, System};
use openbim_geom::{Mesh, WallChain};
use openbim_ifc::{BeamItem, Frame, Scene};
use openbim_place::floors::Slab;
use openbim_place::penetrations::{Run, RunSize};
use openbim_route::{Fitting, FittingKind};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

type P = [f64; 2];

/// What a run of triangles is. Walls have an exterior face (left of the direction of travel, as in the wall type)
/// and an interior face; their ends and the sides of openings are `Edge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaceKind {
    Top,
    Bottom,
    Side,
    Exterior,
    Interior,
    Edge,
}

/// A run of triangles of one mesh with one face kind and one material.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub face: FaceKind,
    pub material: Option<String>,
    pub tris: Range<usize>,
}

/// The mesh of one element. Triangles face outward and are grouped by `parts`, which cover all of them.
#[derive(Debug, Clone)]
pub struct ElementMesh {
    pub element: Element,
    pub mesh: Mesh,
    pub parts: Vec<Part>,
    /// what could not be drawn of this element (for instance a revolve in a family)
    pub warnings: Vec<String>,
}

// ------------------------------------------------------------------ geometry

// ------------------------------------------------------------------ geometry

use openbim_family::meshing::{self, loop_points, rotate_vec, TriMesh};

fn to_mesh(t: TriMesh) -> Mesh {
    Mesh {
        verts: t.verts,
        tris: t.tris,
    }
}

/// A prism between heights z0 and z1 on `outer`, with vertical holes. None if the outline cannot be triangulated.
pub(crate) fn prism(outer: &[P], holes: &[Vec<P>], z0: f64, z1: f64) -> Option<Mesh> {
    meshing::prism(outer, holes, z0, z1).map(to_mesh)
}

fn skin(
    rings: &[Vec<Vec<V3>>],
    profile: &[Vec<P>],
    out: &dyn Fn(usize, usize, usize) -> V3,
    wrap: bool,
    cap_out: &dyn Fn(bool, V3) -> V3,
) -> Option<Mesh> {
    meshing::skin(rings, profile, out, wrap, cap_out).map(to_mesh)
}
fn dot3(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normal(m: &Mesh, t: [u32; 3]) -> [f64; 3] {
    let (a, b, c) = (
        m.verts[t[0] as usize],
        m.verts[t[1] as usize],
        m.verts[t[2] as usize],
    );
    let (u, v) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-12 {
        [0.0; 3]
    } else {
        [n[0] / l, n[1] / l, n[2] / l]
    }
}

fn vertical_kind(n: [f64; 3]) -> FaceKind {
    if n[2] > 0.5 {
        FaceKind::Top
    } else if n[2] < -0.5 {
        FaceKind::Bottom
    } else {
        FaceKind::Side
    }
}

/// Reorder the triangles of `mesh` so that those with the same (face kind, material) are one run.
/// `tag[i]` is the kind and the material of triangle i. Within a run the order is kept.
fn group(mesh: Mesh, tag: Vec<(FaceKind, Option<&str>)>) -> (Mesh, Vec<Part>) {
    let mut mats: Vec<Option<&str>> = Vec::new();
    let mut runs: BTreeMap<(FaceKind, usize), Vec<[u32; 3]>> = BTreeMap::new();
    for (t, (face, mat)) in mesh.tris.iter().zip(&tag) {
        let k = match mats.iter().position(|m| m == mat) {
            Some(k) => k,
            None => {
                mats.push(*mat);
                mats.len() - 1
            }
        };
        runs.entry((*face, k)).or_default().push(*t);
    }
    let mut tris = Vec::with_capacity(mesh.tris.len());
    let mut parts = Vec::new();
    for ((face, k), v) in runs {
        let start = tris.len();
        tris.extend(v);
        parts.push(Part {
            face,
            material: mats[k].map(str::to_string),
            tris: start..tris.len(),
        });
    }
    (
        Mesh {
            verts: mesh.verts,
            tris,
        },
        parts,
    )
}

fn plain(mesh: Mesh) -> (Mesh, Vec<Part>) {
    let tag = mesh
        .tris
        .iter()
        .map(|&t| (vertical_kind(normal(&mesh, t)), None))
        .collect();
    group(mesh, tag)
}

/// Exterior and interior material of a wall, and the material of its edges.
type WallMaterials = [Option<String>; 3];

fn wall_materials(scene: &Scene, wall_type: Option<usize>) -> WallMaterials {
    let Some(t) = wall_type.and_then(|k| scene.wall_types.get(k)) else {
        return [None, None, None];
    };
    let (c0, _) = t.core_range();
    [
        t.layers.first().map(|l| l.material.clone()),
        t.layers.last().map(|l| l.material.clone()),
        t.layers.get(c0).map(|l| l.material.clone()),
    ]
}

fn wall_mesh(
    chain: &WallChain,
    mats: &WallMaterials,
    voids: &[TriMesh],
) -> Result<(Mesh, Vec<Part>, Vec<String>), openbim_geom::GeomError> {
    let mut mesh = chain.chain_mesh()?;
    let mut warnings = Vec::new();
    if !voids.is_empty() {
        // what the hosted families cut out of the wall beyond its openings
        match meshing::cut(
            &TriMesh {
                verts: mesh.verts.clone(),
                tris: mesh.tris.clone(),
            },
            voids,
        ) {
            Some(t) => mesh = to_mesh(t),
            None => {
                warnings.push("a void of a hosted family could not be cut out of the wall".into())
            }
        }
    }
    let n = chain.points.len();
    let segs = if chain.closed { n } else { n.saturating_sub(1) };
    let mut tag: Vec<(FaceKind, Option<&str>)> = Vec::with_capacity(mesh.tris.len());
    for &t in &mesh.tris {
        let nm = normal(&mesh, t);
        let kind = if nm[2].abs() > 0.5 {
            if nm[2] > 0.0 {
                FaceKind::Top
            } else {
                FaceKind::Bottom
            }
        } else {
            // the face belongs to the segment whose centreline is nearest to its centre
            let c = [0, 1].map(|k| {
                (mesh.verts[t[0] as usize][k]
                    + mesh.verts[t[1] as usize][k]
                    + mesh.verts[t[2] as usize][k])
                    / 3.0
            });
            let mut best = (f64::MAX, [0.0, 0.0]);
            for i in 0..segs {
                let (a, b) = (chain.points[i], chain.points[(i + 1) % n]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let l2 = d[0] * d[0] + d[1] * d[1];
                if l2 <= 0.0 {
                    continue;
                }
                let s = (((c[0] - a[0]) * d[0] + (c[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0);
                let dist = (a[0] + d[0] * s - c[0]).hypot(a[1] + d[1] * s - c[1]);
                if dist < best.0 - 1e-9 {
                    best = (dist, [-d[1] / l2.sqrt(), d[0] / l2.sqrt()]);
                }
            }
            let side = nm[0] * best.1[0] + nm[1] * best.1[1];
            if side > 0.7 {
                FaceKind::Exterior
            } else if side < -0.7 {
                FaceKind::Interior
            } else {
                FaceKind::Edge
            }
        };
        let mat = match kind {
            FaceKind::Exterior => &mats[0],
            FaceKind::Interior => &mats[1],
            _ => &mats[2],
        };
        tag.push((kind, mat.as_deref()));
    }
    let (mesh, parts) = group(mesh, tag);
    Ok((mesh, parts, warnings))
}

/// Meshes of everything a family item builds (boxes, extrusions, revolves, sweeps, blends, voids cut out), moved into
/// the model, each triangle tagged with its material.
fn family_parts(frame: &Frame, inst: &Instance) -> (Mesh, Vec<Part>, Vec<String>) {
    let body = inst.body();
    let mut mesh = Mesh::default();
    let mut mat_of_tri: Vec<Option<&str>> = Vec::new();
    for piece in &body.pieces {
        let mat = piece
            .material
            .and_then(|i| inst.strings.get(i as usize))
            .map(|s| s.as_str());
        let base = mesh.verts.len() as u32;
        mesh.verts.extend(piece.mesh.verts.iter().map(|v| {
            let g = frame.point([v[0], v[1], 0.0]);
            [g[0], g[1], frame.origin[2] + v[2]]
        }));
        for t in &piece.mesh.tris {
            mesh.tris.push([t[0] + base, t[1] + base, t[2] + base]);
            mat_of_tri.push(mat);
        }
    }
    let tag = mesh
        .tris
        .iter()
        .zip(&mat_of_tri)
        .map(|(&t, &m)| (vertical_kind(normal(&mesh, t)), m))
        .collect();
    let (mesh, parts) = group(mesh, tag);
    (mesh, parts, body.warnings)
}

const CIRCLE_SIDES: usize = 32;

// ------------------------------------------------------------------ service networks

type V3 = [f64; 3];
fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross3(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit3(a: V3) -> V3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l == 0.0 {
        a
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// A direction square to `d` (the same rule as the IFC export uses for round sections).
fn perp(d: V3) -> V3 {
    let h = if d[0].abs() <= d[1].abs() && d[0].abs() <= d[2].abs() {
        [1.0, 0.0, 0.0]
    } else if d[1].abs() <= d[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    unit3(cross3(d, h))
}

/// A straight piece with the section `size` from `at` along the unit vector `d` for `len` metres: a cylinder
/// (an n-gon prism) or a box. Triangles face outward.
fn straight_piece(size: RunSize, at: V3, d: V3, len: f64) -> Option<Mesh> {
    if len.is_nan() || len <= 1e-9 {
        return None;
    }
    let (x, ring): (V3, Vec<[f64; 2]>) = match size {
        RunSize::Round { diameter } => (
            perp(d),
            loop_points(&Loop::Circle {
                c: [0.0, 0.0],
                r: diameter / 2.0,
            }),
        ),
        RunSize::Rect {
            width,
            height,
            roll,
        } => {
            let (w, h) = (width / 2.0, height / 2.0);
            (
                unit3(cross3(up_for(d, roll), d)),
                vec![[-w, -h], [w, -h], [w, h], [-w, h]],
            )
        }
    };
    let y = cross3(d, x);
    let n = ring.len() as u32;
    let mut m = Mesh::default();
    for k in 0..2 {
        let o = [
            at[0] + d[0] * len * k as f64,
            at[1] + d[1] * len * k as f64,
            at[2] + d[2] * len * k as f64,
        ];
        for q in &ring {
            m.verts.push([
                o[0] + x[0] * q[0] + y[0] * q[1],
                o[1] + x[1] * q[0] + y[1] * q[1],
                o[2] + x[2] * q[0] + y[2] * q[1],
            ]);
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        m.tris.push([i, j, j + n]);
        m.tris.push([i, j + n, i + n]);
    }
    for i in 1..n - 1 {
        m.tris.push([n, n + i, n + i + 1]); // far end, facing along d
        m.tris.push([0, i + 1, i]); // near end
    }
    if m.volume() < 0.0 {
        for t in &mut m.tris {
            t.swap(1, 2);
        }
    }
    Some(m)
}

fn run_mesh(r: &Run) -> Option<Mesh> {
    let d = sub3(r.b, r.a);
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    straight_piece(r.size, r.a, unit3(d), len)
}

/// An elbow as a bend: the section carried round the arc of radius `f.radius` between the tangent points of its two
/// legs. None when the fitting does not describe a consistent arc, so that the caller draws stubs instead.
fn elbow_bend(f: &Fitting) -> Option<Mesh> {
    if !matches!(f.kind, FittingKind::Elbow { .. })
        || f.ports.len() != 2
        || f.radius <= 1e-9
        || f.trim.len() != 2
        || f.trim.iter().any(|t| t.is_nan() || *t <= 1e-9)
    {
        return None;
    }
    let (u0, u1) = (unit3(f.ports[0]), unit3(f.ports[1]));
    let travel = [-u0[0], -u0[1], -u0[2]]; // direction of flow into the fitting along leg 0
    let c = dot3(travel, u1).clamp(-1.0, 1.0);
    let bend = c.acos();
    if !(1e-3..=std::f64::consts::PI - 1e-3).contains(&bend) {
        return None;
    }
    let axis = unit3(cross3(travel, u1));
    let t0 = [
        f.at[0] + u0[0] * f.trim[0],
        f.at[1] + u0[1] * f.trim[0],
        f.at[2] + u0[2] * f.trim[0],
    ];
    // the centre of the arc is toward the inside of the bend, square to the first leg
    let inside = unit3(cross3(axis, travel));
    let centre = [
        t0[0] + inside[0] * f.radius,
        t0[1] + inside[1] * f.radius,
        t0[2] + inside[2] * f.radius,
    ];
    // the arc has to end where the second leg's tangent point is
    let end = {
        let r = rotate_vec(sub3(t0, centre), axis, bend);
        [centre[0] + r[0], centre[1] + r[1], centre[2] + r[2]]
    };
    let t1 = [
        f.at[0] + u1[0] * f.trim[1],
        f.at[1] + u1[1] * f.trim[1],
        f.at[2] + u1[2] * f.trim[1],
    ];
    if (0..3).any(|k| (end[k] - t1[k]).abs() > 1e-6) {
        return None;
    }
    // section frame at the start: x and y as for a straight piece along `travel`
    let size = f.port_sizes[0];
    let (x0, ring): (V3, Vec<P>) = match size {
        RunSize::Round { diameter } => (
            perp(travel),
            loop_points(&Loop::Circle {
                c: [0.0, 0.0],
                r: diameter / 2.0,
            }),
        ),
        RunSize::Rect {
            width,
            height,
            roll,
        } => (
            unit3(cross3(up_for(travel, roll), travel)),
            vec![
                [-width / 2.0, -height / 2.0],
                [width / 2.0, -height / 2.0],
                [width / 2.0, height / 2.0],
                [-width / 2.0, height / 2.0],
            ],
        ),
    };
    let y0 = cross3(travel, x0);
    let steps = ((bend.to_degrees() / 7.5).ceil() as usize).clamp(2, 48);
    let ang = |j: usize| bend * j as f64 / steps as f64;
    let rings = vec![(0..=steps)
        .map(|j| {
            let (x, y) = (rotate_vec(x0, axis, ang(j)), rotate_vec(y0, axis, ang(j)));
            let o = rotate_vec(sub3(t0, centre), axis, ang(j));
            let o = [centre[0] + o[0], centre[1] + o[1], centre[2] + o[2]];
            ring.iter()
                .map(|q| {
                    [
                        o[0] + x[0] * q[0] + y[0] * q[1],
                        o[1] + x[1] * q[0] + y[1] * q[1],
                        o[2] + x[2] * q[0] + y[2] * q[1],
                    ]
                })
                .collect()
        })
        .collect::<Vec<Vec<V3>>>()];
    let out = |_r: usize, k: usize, j: usize| -> V3 {
        let (x, y) = (rotate_vec(x0, axis, ang(j)), rotate_vec(y0, axis, ang(j)));
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let right = [b[1] - a[1], -(b[0] - a[0])]; // for a counter-clockwise ring this points out
        [
            x[0] * right[0] + y[0] * right[1],
            x[1] * right[0] + y[1] * right[1],
            x[2] * right[0] + y[2] * right[1],
        ]
    };
    let end_dir = rotate_vec(travel, axis, bend);
    let cap_out = move |is_end: bool, _p: V3| -> V3 {
        if is_end {
            end_dir
        } else {
            [-travel[0], -travel[1], -travel[2]]
        }
    };
    skin(&rings, std::slice::from_ref(&ring), &out, false, &cap_out)
}

/// The points of a section on the plane through `o` spanned by `x` and `y`. A round section is a 32-gon; a
/// rectangular one has its corners only when `rect_corners` (when both ends of a loft are rectangles), otherwise it
/// is sampled at the same 32 angles as a round one so that the two ends of a loft pair up.
fn section_points(size: RunSize, o: V3, x: V3, y: V3, rect_corners: bool) -> Vec<V3> {
    let at = |q: P| {
        [
            o[0] + x[0] * q[0] + y[0] * q[1],
            o[1] + x[1] * q[0] + y[1] * q[1],
            o[2] + x[2] * q[0] + y[2] * q[1],
        ]
    };
    match size {
        RunSize::Round { diameter } => loop_points(&Loop::Circle {
            c: [0.0, 0.0],
            r: diameter / 2.0,
        })
        .into_iter()
        .map(at)
        .collect(),
        RunSize::Rect { width, height, .. } if rect_corners => [
            [-width / 2.0, -height / 2.0],
            [width / 2.0, -height / 2.0],
            [width / 2.0, height / 2.0],
            [-width / 2.0, height / 2.0],
        ]
        .into_iter()
        .map(at)
        .collect(),
        RunSize::Rect { width, height, .. } => (0..CIRCLE_SIDES)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / CIRCLE_SIDES as f64;
                let (c, s) = (a.cos(), a.sin());
                let r = (width / 2.0 / c.abs().max(1e-12)).min(height / 2.0 / s.abs().max(1e-12));
                at([r * c, r * s])
            })
            .collect(),
    }
}

/// A transition: the section changes linearly from one end to the other (a frustum, or a duct that changes size).
fn transition_loft(f: &Fitting) -> Option<Mesh> {
    if !matches!(f.kind, FittingKind::Transition) || f.ports.len() != 2 || f.trim.len() != 2 {
        return None;
    }
    let (u0, u1) = (unit3(f.ports[0]), unit3(f.ports[1]));
    let p0 = [
        f.at[0] + u0[0] * f.trim[0],
        f.at[1] + u0[1] * f.trim[0],
        f.at[2] + u0[2] * f.trim[0],
    ];
    let p1 = [
        f.at[0] + u1[0] * f.trim[1],
        f.at[1] + u1[1] * f.trim[1],
        f.at[2] + u1[2] * f.trim[1],
    ];
    let d = sub3(p1, p0);
    let len = dot3(d, d).sqrt();
    if len.is_nan() || len <= 1e-9 {
        return None;
    }
    let d = unit3(d);
    let frame = |size: RunSize| -> (V3, V3) {
        let x = match size {
            RunSize::Round { .. } => perp(d),
            RunSize::Rect { roll, .. } => unit3(cross3(up_for(d, roll), d)),
        };
        (x, cross3(d, x))
    };
    let both_rect = matches!(
        (f.port_sizes[0], f.port_sizes[1]),
        (RunSize::Rect { .. }, RunSize::Rect { .. })
    );
    let (x0, y0) = frame(f.port_sizes[0]);
    let (x1, y1) = frame(f.port_sizes[1]);
    let ring0 = section_points(f.port_sizes[0], p0, x0, y0, both_rect);
    let ring1 = section_points(f.port_sizes[1], p1, x1, y1, both_rect);
    let n = ring0.len();
    // the 2D shape of the end caps: the first ring in its own plane
    let flat: Vec<P> = ring0
        .iter()
        .map(|p| {
            let r = sub3(*p, p0);
            [dot3(r, x0), dot3(r, y0)]
        })
        .collect();
    let centres = [p0, p1];
    let rings = vec![vec![ring0, ring1]];
    let out = |_r: usize, k: usize, j: usize| -> V3 {
        let p = rings[0][j][k];
        let q = rings[0][j][(k + 1) % n];
        let m = [
            (p[0] + q[0]) / 2.0,
            (p[1] + q[1]) / 2.0,
            (p[2] + q[2]) / 2.0,
        ];
        sub3(m, centres[j])
    };
    let cap_out = move |is_end: bool, _p: V3| -> V3 {
        if is_end {
            d
        } else {
            [-d[0], -d[1], -d[2]]
        }
    };
    skin(&rings, &[flat], &out, false, &cap_out)
}

/// A fitting: an elbow with a consistent bend radius is a bend, a transition a loft; anything else is the straight stubs it takes from
/// each leg, as in the IFC export.
fn fitting_mesh(f: &Fitting) -> Option<Mesh> {
    if let Some(m) = elbow_bend(f).or_else(|| transition_loft(f)) {
        return Some(m);
    }
    let mut out = Mesh::default();
    for k in 0..f.ports.len() {
        let Some(m) = straight_piece(f.port_sizes[k], f.at, unit3(f.ports[k]), f.trim[k]) else {
            continue;
        };
        let base = out.verts.len() as u32;
        out.verts.extend(m.verts);
        out.tris.extend(
            m.tris
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
    }
    if out.tris.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// A section with a layer of insulation all round.
fn grow(size: RunSize, t: f64) -> RunSize {
    match size {
        RunSize::Round { diameter } => RunSize::Round {
            diameter: diameter + 2.0 * t,
        },
        RunSize::Rect {
            width,
            height,
            roll,
        } => RunSize::Rect {
            width: width + 2.0 * t,
            height: height + 2.0 * t,
            roll,
        },
    }
}

/// One part, one material: the system the piece belongs to.
fn one_part(mesh: Mesh, system: System) -> (Mesh, Vec<Part>) {
    let n = mesh.tris.len();
    (
        mesh,
        vec![Part {
            face: FaceKind::Side,
            material: Some(system.ifc_system().to_string()),
            tris: 0..n,
        }],
    )
}

// ------------------------------------------------------------------ inputs and cache

/// The inputs of one element's mesh, borrowed from the scene.
enum Src<'a> {
    /// a wall, and the voids of the families hosted in it, in model coordinates
    Wall(&'a WallChain, WallMaterials, Vec<TriMesh>),
    /// a slab, its holes, and the voids of free-standing families that reach it (a shaft), in model coordinates
    Slab(&'a Slab, Vec<Vec<P>>, Vec<TriMesh>),
    Column(&'a [P], f64, f64),
    /// a beam, and the voids of free-standing families that reach it, in model coordinates
    Beam(&'a BeamItem, Vec<TriMesh>),
    Family(&'a Frame, &'a Instance),
    Run(&'a Run),
    Fitting(&'a Fitting, System),
    /// a run or fitting with its size grown by the insulation: (thickness, material)
    InsulatedRun(&'a Run, f64, &'a str),
    InsulatedFitting(&'a Fitting, f64, &'a str),
}

/// The same inputs, kept by the cache.
enum Key {
    Wall(WallChain, WallMaterials, Vec<TriMesh>),
    Slab(Slab, Vec<Vec<P>>, Vec<TriMesh>),
    Column(Vec<P>, f64, f64),
    Beam(BeamItem, Vec<TriMesh>),
    Family(Frame, Instance),
    Run(Run),
    Fitting(Fitting, System),
    InsulatedRun(Run, f64, String),
    InsulatedFitting(Fitting, f64, String),
}

impl Src<'_> {
    fn same(&self, k: &Key) -> bool {
        match (self, k) {
            (Src::Wall(a, m, v), Key::Wall(b, n, w)) => *a == b && m == n && v == w,
            (Src::Slab(a, h, v), Key::Slab(b, g, w)) => *a == b && h == g && v == w,
            (Src::Beam(a, v), Key::Beam(b, w)) => *a == b && v == w,
            (Src::Column(a, z, t), Key::Column(b, y, u)) => *a == b.as_slice() && z == y && t == u,
            (Src::Family(a, i), Key::Family(b, j)) => *a == b && *i == j,
            (Src::Run(a), Key::Run(b)) => *a == b,
            (Src::Fitting(a, s), Key::Fitting(b, t)) => *a == b && s == t,
            (Src::InsulatedRun(a, x, m), Key::InsulatedRun(b, y, n)) => *a == b && x == y && m == n,
            (Src::InsulatedFitting(a, x, m), Key::InsulatedFitting(b, y, n)) => {
                *a == b && x == y && m == n
            }
            _ => false,
        }
    }
    fn own(&self) -> Key {
        match self {
            Src::Wall(c, m, v) => Key::Wall((*c).clone(), m.clone(), v.clone()),
            Src::Slab(s, h, v) => Key::Slab((*s).clone(), h.clone(), v.clone()),
            Src::Column(f, a, b) => Key::Column(f.to_vec(), *a, *b),
            Src::Beam(b, v) => Key::Beam((*b).clone(), v.clone()),
            Src::Family(f, i) => Key::Family(**f, (*i).clone()),
            Src::Run(r) => Key::Run((*r).clone()),
            Src::Fitting(f, s) => Key::Fitting((*f).clone(), *s),
            Src::InsulatedRun(r, t, m) => Key::InsulatedRun((*r).clone(), *t, m.to_string()),
            Src::InsulatedFitting(f, t, m) => {
                Key::InsulatedFitting((*f).clone(), *t, m.to_string())
            }
        }
    }
    fn build(&self, el: Element) -> Result<ElementMesh, ViewError> {
        let bad = || ViewError::Mesh { id: el.id };
        let mut warnings = Vec::new();
        let (mesh, parts) = match self {
            Src::Wall(c, m, v) => {
                let (m, p, w) =
                    wall_mesh(c, m, v).map_err(|err| ViewError::Wall { id: el.id, err })?;
                warnings = w;
                (m, p)
            }
            Src::Slab(s, holes, voids) => {
                let mut m = prism(&s.outline, holes, s.top - s.thickness, s.top).ok_or_else(bad)?;
                if !voids.is_empty() {
                    let whole = TriMesh {
                        verts: m.verts.clone(),
                        tris: m.tris.clone(),
                    };
                    match meshing::cut(&whole, voids) {
                        Some(t) => m = to_mesh(t),
                        None => warnings
                            .push("a void of a family could not be cut out of the slab".into()),
                    }
                }
                plain(m)
            }
            Src::Beam(b, voids) => {
                let mut m = beam_mesh(b).ok_or_else(bad)?;
                if !voids.is_empty() {
                    let whole = TriMesh { verts: m.verts.clone(), tris: m.tris.clone() };
                    match meshing::cut(&whole, voids) {
                        Some(t) => m = to_mesh(t),
                        None => warnings.push("a void of a family could not be cut out of the beam".into()),
                    }
                }
                plain(m)
            }
            Src::Column(f, z0, z1) => plain(prism(f, &[], *z0, *z1).ok_or_else(bad)?),
            Src::Family(f, i) => {
                let (m, p, w) = family_parts(f, i);
                warnings = w;
                (m, p)
            }
            Src::Run(r) => one_part(run_mesh(r).ok_or_else(bad)?, r.system),
            Src::Fitting(f, s) => match fitting_mesh(f) {
                Some(m) => one_part(m, *s),
                None => (Mesh::default(), Vec::new()),
            },
            Src::InsulatedRun(r, t, name) => {
                let mut g = (*r).clone();
                g.size = grow(g.size, *t);
                let m = run_mesh(&g).ok_or_else(bad)?;
                let n = m.tris.len();
                (
                    m,
                    vec![Part {
                        face: FaceKind::Side,
                        material: Some(name.to_string()),
                        tris: 0..n,
                    }],
                )
            }
            Src::InsulatedFitting(f, t, name) => {
                let mut g = (*f).clone();
                g.port_sizes = g.port_sizes.iter().map(|s| grow(*s, *t)).collect();
                match fitting_mesh(&g) {
                    Some(m) => {
                        let n = m.tris.len();
                        (
                            m,
                            vec![Part {
                                face: FaceKind::Side,
                                material: Some(name.to_string()),
                                tris: 0..n,
                            }],
                        )
                    }
                    None => (Mesh::default(), Vec::new()),
                }
            }
        };
        Ok(ElementMesh {
            element: el,
            mesh,
            parts,
            warnings,
        })
    }
}

/// Every element with its inputs. The number beside the element is the network it belongs to (0 for everything that is
/// not a service piece): piece numbers are only unique within one network.
/// The voids whose box meets the box `lo..hi`; the others cannot cut what is inside it.
fn reaching(voids: &[TriMesh], lo: [f64; 3], hi: [f64; 3]) -> Vec<TriMesh> {
    let meets = |m: &TriMesh| {
        (0..3).all(|k| {
            let a = m.verts.iter().map(|v| v[k]).fold(f64::INFINITY, f64::min);
            let b = m.verts.iter().map(|v| v[k]).fold(f64::NEG_INFINITY, f64::max);
            a < hi[k] && b > lo[k]
        })
    };
    voids.iter().filter(|m| meets(m)).cloned().collect()
}

fn slab_box(slab: &Slab) -> ([f64; 3], [f64; 3]) {
    let r = |k: usize| (slab.outline.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min), slab.outline.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max));
    ([r(0).0, r(1).0, slab.top - slab.thickness], [r(0).1, r(1).1, slab.top])
}

/// The box of a beam: eight corners around the centre line, twelve outward triangles.
fn beam_corners(b: &BeamItem) -> Option<[[f64; 3]; 8]> {
    let (_, side, up) = b.axes()?;
    let mut c = [[0.0; 3]; 8];
    for (i, p) in c.iter_mut().enumerate() {
        let (e, s, h) = (i >> 2, (i >> 1) & 1, i & 1);
        let base = if e == 0 { b.a } else { b.b };
        let (ws, wh) = ((s as f64 - 0.5) * b.width, (h as f64 - 0.5) * b.depth);
        *p = [base[0] + side[0] * ws + up[0] * wh, base[1] + side[1] * ws + up[1] * wh, base[2] + side[2] * ws + up[2] * wh];
    }
    Some(c)
}

fn beam_mesh(b: &BeamItem) -> Option<Mesh> {
    let c = beam_corners(b)?;
    let quads: [[u32; 4]; 6] = [[4, 6, 7, 5], [0, 1, 3, 2], [2, 3, 7, 6], [0, 4, 5, 1], [1, 5, 7, 3], [0, 2, 6, 4]];
    let tris = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
    Some(Mesh { verts: c.to_vec(), tris })
}

fn sources(scene: &Scene) -> Vec<(Element, u32, Src<'_>)> {
    let mut v = Vec::new();
    // the voids of families, moved into the model: those hosted in a wall cut that wall, the free-standing ones cut
    // the slabs they reach (a shaft)
    let mut wall_voids: Vec<Vec<TriMesh>> = vec![Vec::new(); scene.walls.len()];
    let mut free_voids: Vec<TriMesh> = Vec::new();
    for it in &scene.items {
        let list = match it.host {
            Some((wi, _, _)) => match wall_voids.get_mut(wi) {
                Some(l) => l,
                None => continue,
            },
            None => &mut free_voids,
        };
        for mut m in it.instance.voids() {
            for p in &mut m.verts {
                let g = it.frame.point([p[0], p[1], 0.0]);
                *p = [g[0], g[1], it.frame.origin[2] + p[2]];
            }
            list.push(m);
        }
    }
    for (wi, w) in scene.walls.iter().enumerate() {
        v.push((
            Element {
                kind: Kind::Wall,
                id: w.id,
            },
            0,
            Src::Wall(
                &w.chain,
                wall_materials(scene, w.wall_type),
                std::mem::take(&mut wall_voids[wi]),
            ),
        ));
    }
    for (i, s) in scene.slabs.iter().enumerate() {
        v.push((
            Element {
                kind: Kind::Slab,
                id: s.id,
            },
            0,
            Src::Slab(
                &s.slab,
                slab_holes(scene, i, &s.slab),
                { let (lo, hi) = slab_box(&s.slab); reaching(&free_voids, lo, hi) },
            ),
        ));
    }
    for b in &scene.beams {
        let voids = match beam_corners(b) {
            Some(c) => {
                let lo = [0, 1, 2].map(|k| c.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min));
                let hi = [0, 1, 2].map(|k| c.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max));
                reaching(&free_voids, lo, hi)
            }
            None => Vec::new(),
        };
        v.push((Element { kind: Kind::Beam, id: b.id }, 0, Src::Beam(b, voids)));
    }
    for c in &scene.columns {
        v.push((
            Element {
                kind: Kind::Column,
                id: c.id,
            },
            0,
            Src::Column(&c.footprint, c.base_z, c.base_z + c.height),
        ));
    }
    for it in &scene.items {
        v.push((
            Element {
                kind: Kind::Family,
                id: it.id,
            },
            0,
            Src::Family(&it.frame, &it.instance),
        ));
    }
    for (ni, net) in scene.networks.iter().enumerate() {
        for sg in &net.segments {
            v.push((
                Element {
                    kind: Kind::Segment,
                    id: sg.id,
                },
                ni as u32,
                Src::Run(&sg.run),
            ));
            if let Some(i) = net.insulation_of(sg.run.system) {
                v.push((
                    Element {
                        kind: Kind::Insulation,
                        id: sg.id,
                    },
                    ni as u32,
                    Src::InsulatedRun(&sg.run, i.thickness, &i.material),
                ));
            }
        }
        for nf in &net.fittings {
            v.push((
                Element {
                    kind: Kind::Fitting,
                    id: nf.id,
                },
                ni as u32,
                Src::Fitting(&nf.fitting, nf.system),
            ));
            if let Some(i) = net.insulation_of(nf.system) {
                v.push((
                    Element {
                        kind: Kind::Insulation,
                        id: nf.id,
                    },
                    ni as u32,
                    Src::InsulatedFitting(&nf.fitting, i.thickness, &i.material),
                ));
            }
        }
    }
    v
}

/// Meshes of every wall, slab, column and family solid in the scene.
pub fn scene_meshes(scene: &Scene) -> Result<Vec<ElementMesh>, ViewError> {
    sources(scene)
        .par_iter()
        .map(|(el, _, s)| s.build(*el))
        .collect::<Vec<_>>()
        .into_iter()
        .collect()
}

/// What `MeshCache::update` found.
#[derive(Debug, Clone)]
pub struct Update {
    /// every element's mesh, in scene order; unchanged ones are the same `Arc` as before
    pub meshes: Vec<Arc<ElementMesh>>,
    /// elements meshed again (new or changed)
    pub rebuilt: Vec<Element>,
    /// elements that were in the cache and are no longer in the scene
    pub removed: Vec<Element>,
}

/// Meshes kept between edits; see the module notes.
#[derive(Default)]
pub struct MeshCache {
    entries: HashMap<(Element, u32), (Key, Arc<ElementMesh>)>,
}

impl MeshCache {
    pub fn new() -> MeshCache {
        MeshCache::default()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bring the cache to the state of `scene`, meshing only what changed. On an error the cache is left as it was.
    pub fn update(&mut self, scene: &Scene) -> Result<Update, ViewError> {
        let src = sources(scene);
        let built: Vec<Option<Result<ElementMesh, ViewError>>> = src
            .par_iter()
            .map(|(el, own, s)| {
                if self
                    .entries
                    .get(&(*el, *own))
                    .is_some_and(|(k, _)| s.same(k))
                {
                    None
                } else {
                    Some(s.build(*el))
                }
            })
            .collect();
        let mut fresh = Vec::new();
        for b in built.iter().flatten() {
            if let Err(e) = b {
                return Err(e.clone());
            }
        }
        let mut meshes = Vec::with_capacity(src.len());
        let mut rebuilt = Vec::new();
        for ((el, own, s), b) in src.iter().zip(built) {
            match b {
                None => meshes.push(self.entries[&(*el, *own)].1.clone()),
                Some(r) => {
                    let m = Arc::new(r.expect("errors returned above"));
                    meshes.push(m.clone());
                    fresh.push(((*el, *own), s.own(), m));
                    rebuilt.push(*el);
                }
            }
        }
        let now: HashSet<(Element, u32)> = src.iter().map(|(e, o, _)| (*e, *o)).collect();
        let mut gone: Vec<(Element, u32)> = self
            .entries
            .keys()
            .filter(|e| !now.contains(e))
            .copied()
            .collect();
        gone.sort_by_key(|(e, o)| (*o, e.id, format!("{:?}", e.kind)));
        for e in &gone {
            self.entries.remove(e);
        }
        let removed: Vec<Element> = gone.into_iter().map(|(e, _)| e).collect();
        for (el, k, m) in fresh {
            self.entries.insert(el, (k, m));
        }
        Ok(Update {
            meshes,
            rebuilt,
            removed,
        })
    }
}

// ------------------------------------------------------------------ batch for the GPU

/// All meshes joined into one set of buffers, in single precision relative to `origin`.
#[derive(Debug, Clone, Default)]
pub struct Batch {
    /// position of (0, 0, 0) of `verts` in model coordinates
    pub origin: [f64; 3],
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// for each triangle: index into `elements`
    pub tri_element: Vec<u32>,
    pub tri_face: Vec<FaceKind>,
    /// for each triangle: index into `materials`, or `u32::MAX` for none
    pub tri_material: Vec<u32>,
    pub elements: Vec<Element>,
    /// material names, in order of first use
    pub materials: Vec<String>,
}

impl Batch {
    pub fn new(meshes: &[Arc<ElementMesh>]) -> Batch {
        let mut origin = [f64::MAX; 3];
        for m in meshes {
            for v in &m.mesh.verts {
                for k in 0..3 {
                    origin[k] = origin[k].min(v[k]);
                }
            }
        }
        if origin[0] == f64::MAX {
            origin = [0.0; 3];
        }
        let mut b = Batch {
            origin,
            ..Batch::default()
        };
        let mut mat_index: HashMap<&str, u32> = HashMap::new();
        for (ei, m) in meshes.iter().enumerate() {
            let base = b.verts.len() as u32;
            b.verts.extend(m.mesh.verts.iter().map(|v| {
                [
                    (v[0] - origin[0]) as f32,
                    (v[1] - origin[1]) as f32,
                    (v[2] - origin[2]) as f32,
                ]
            }));
            b.elements.push(m.element);
            for part in &m.parts {
                let mi = match &part.material {
                    None => u32::MAX,
                    Some(name) => *mat_index.entry(name.as_str()).or_insert_with(|| {
                        b.materials.push(name.clone());
                        b.materials.len() as u32 - 1
                    }),
                };
                for t in &m.mesh.tris[part.tris.clone()] {
                    b.tris.push([t[0] + base, t[1] + base, t[2] + base]);
                    b.tri_element.push(ei as u32);
                    b.tri_face.push(part.face);
                    b.tri_material.push(mi);
                }
            }
        }
        b
    }
}
