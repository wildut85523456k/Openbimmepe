//! The logic of a 3D view that does not need a window or a GPU: the camera, the way the mouse drives it (as in
//! Revit), the ViewCube, and a software renderer for the visual styles. The egui/wgpu viewport draws the same
//! `Batch` with the same camera; the software renderer is what the tests look at, and a fallback.
//!
//! Coordinates: metres, z up, as in the model. Screen: pixels, x right, y down.

mod edges;
mod raster;
pub use edges::{crease_edges, intersection_edges, Edge};
pub use raster::{base_colour, render, Image, Style};

type V = [f64; 3];

fn add(a: V, b: V) -> V { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub(a: V, b: V) -> V { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn mul(a: V, s: f64) -> V { [a[0] * s, a[1] * s, a[2] * s] }
fn dot(a: V, b: V) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(a: V) -> V { let l = dot(a, a).sqrt(); if l == 0.0 { a } else { mul(a, 1.0 / l) } }

const MAX_PITCH: f64 = 89.0_f64 * std::f64::consts::PI / 180.0;
const FOV: f64 = 45.0_f64 * std::f64::consts::PI / 180.0;

/// An orbit camera: it looks at `target` from `distance` away. `yaw` is the direction of the eye seen from the
/// target in plan (0 = from +x, counter-clockwise); `pitch` is its height angle above the horizon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub target: V,
    pub distance: f64,
    pub yaw: f64,
    pub pitch: f64,
}

/// Mouse buttons held during a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buttons { pub left: bool, pub middle: bool, pub right: bool }
/// Keyboard modifiers held during a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods { pub shift: bool, pub ctrl: bool }

/// What a drag does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drag { Orbit, Pan, Nothing }

impl Camera {
    /// The `{3D}` view of a new project: from the south-east, looking at `target`, at the isometric angle.
    pub fn default_3d(target: V, distance: f64) -> Camera {
        Camera { target, distance, yaw: -std::f64::consts::FRAC_PI_4, pitch: (1.0_f64 / 3.0_f64.sqrt()).asin() }
    }

    pub fn eye(&self) -> V {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        add(self.target, mul([cp * self.yaw.cos(), cp * self.yaw.sin(), sp], self.distance))
    }
    /// unit vector from the eye toward the target
    pub fn forward(&self) -> V { unit(sub(self.target, self.eye())) }
    pub fn right(&self) -> V { unit(cross(self.forward(), [0.0, 0.0, 1.0])) }
    pub fn up(&self) -> V { cross(self.right(), self.forward()) }

    /// Metres per pixel on the plane through the target.
    pub fn metres_per_pixel(&self, height_px: f64) -> f64 { 2.0 * self.distance * (FOV / 2.0).tan() / height_px }

    /// What a drag with these buttons does, as in Revit: middle button pans, Shift + middle orbits.
    pub fn drag_kind(b: Buttons, m: Mods) -> Drag {
        match (b.middle, m.shift) { (true, true) => Drag::Orbit, (true, false) => Drag::Pan, _ => Drag::Nothing }
    }

    pub fn orbit(&mut self, dx: f64, dy: f64) {
        const PER_PIXEL: f64 = 0.005;
        self.yaw -= dx * PER_PIXEL;
        self.pitch = (self.pitch + dy * PER_PIXEL).clamp(-MAX_PITCH, MAX_PITCH);
    }

    pub fn pan(&mut self, dx: f64, dy: f64, height_px: f64) {
        let s = self.metres_per_pixel(height_px);
        self.target = add(self.target, add(mul(self.right(), -dx * s), mul(self.up(), dy * s)));
    }

    /// Apply a drag; returns what it did.
    pub fn drag(&mut self, b: Buttons, m: Mods, dx: f64, dy: f64, height_px: f64) -> Drag {
        let k = Camera::drag_kind(b, m);
        match k { Drag::Orbit => self.orbit(dx, dy), Drag::Pan => self.pan(dx, dy, height_px), Drag::Nothing => {} }
        k
    }

    /// Wheel zoom toward the cursor: `factor` below 1 comes closer. The point under the cursor stays where it is.
    pub fn zoom_at(&mut self, cursor: [f64; 2], factor: f64, size: [f64; 2]) {
        let factor = factor.clamp(0.05, 20.0);
        let s = self.metres_per_pixel(size[1]);
        let off = [cursor[0] - size[0] / 2.0, cursor[1] - size[1] / 2.0];
        let p = add(self.target, add(mul(self.right(), off[0] * s), mul(self.up(), -off[1] * s)));
        self.target = add(self.target, mul(sub(p, self.target), 1.0 - factor));
        self.distance = (self.distance * factor).max(1e-3);
    }

    /// Zoom to fit a box (the model, or the selection), keeping the direction of view.
    pub fn fit(&mut self, min: V, max: V, aspect: f64) {
        self.target = mul(add(min, max), 0.5);
        let r = dot(sub(max, min), sub(max, min)).sqrt() / 2.0;
        let half = (FOV / 2.0).tan().min((FOV / 2.0).tan() * aspect);
        self.distance = (r / half * 1.05).max(1e-3);
    }

    /// Near and far planes for a scene `radius` metres across that the camera sits `distance` from.
    pub fn clip_range(&self, radius: f64) -> (f64, f64) {
        let far = (self.distance + radius * 2.0).max(1.0);
        ((far * 1e-4).max(self.distance * 0.01).min(self.distance * 0.5).max(1e-3), far)
    }

    /// View and projection as one column-major matrix for a GPU with clip depth 0..1 and y up (`wgpu`): a model point
    /// `p` (metres, relative to `origin`) goes to `m * [p, 1]`. The vertical field of view is fixed.
    pub fn view_proj(&self, aspect: f64, near: f64, far: f64, origin: V) -> [[f32; 4]; 4] {
        let (r, u, f) = (self.right(), self.up(), self.forward());
        let eye = sub(self.eye(), origin);
        // view space: x right, y up, looking down -z
        let view = [
            [r[0], u[0], -f[0], 0.0],
            [r[1], u[1], -f[1], 0.0],
            [r[2], u[2], -f[2], 0.0],
            [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
        ];
        let t = 1.0 / (FOV / 2.0).tan();
        let proj = [
            [t / aspect, 0.0, 0.0, 0.0],
            [0.0, t, 0.0, 0.0],
            [0.0, 0.0, far / (near - far), -1.0],
            [0.0, 0.0, near * far / (near - far), 0.0],
        ];
        let mut m = [[0.0f64; 4]; 4];
        for c in 0..4 { for rr in 0..4 { m[c][rr] = (0..4).map(|k| proj[k][rr] * view[c][k]).sum(); } }
        m.map(|col| col.map(|x| x as f32))
    }

    /// Pixel position and distance along the view direction of a model point; None behind the camera.
    pub fn project(&self, p: V, size: [f64; 2]) -> Option<[f64; 3]> {
        let rel = sub(p, self.eye());
        let z = dot(rel, self.forward());
        if z <= 0.01 { return None; }
        let f = size[1] / 2.0 / (FOV / 2.0).tan();
        Some([size[0] / 2.0 + dot(rel, self.right()) / z * f, size[1] / 2.0 - dot(rel, self.up()) / z * f, z])
    }
}

/// The ViewCube: 26 places to look from, the 6 faces, 12 edges and 8 corners of the cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CubeDir(pub [i8; 3]);

impl CubeDir {
    /// Named faces as Revit has them: the direction is where the eye sits.
    pub const TOP: CubeDir = CubeDir([0, 0, 1]);
    pub const BOTTOM: CubeDir = CubeDir([0, 0, -1]);
    pub const FRONT: CubeDir = CubeDir([0, -1, 0]); // looking north, as the south elevation
    pub const BACK: CubeDir = CubeDir([0, 1, 0]);
    pub const RIGHT: CubeDir = CubeDir([1, 0, 0]);
    pub const LEFT: CubeDir = CubeDir([-1, 0, 0]);

    /// All 26 directions in a fixed order.
    pub fn all() -> Vec<CubeDir> {
        let mut v = Vec::new();
        for z in -1..=1 { for y in -1..=1 { for x in -1..=1 { if (x, y, z) != (0, 0, 0) { v.push(CubeDir([x, y, z])); } } } }
        v
    }

    /// The cube place nearest to the direction the camera now looks from.
    pub fn nearest(cam: &Camera) -> CubeDir {
        let e = unit(sub(cam.eye(), cam.target));
        let mut best = (f64::MIN, CubeDir::TOP);
        for d in CubeDir::all() {
            let u = unit([d.0[0] as f64, d.0[1] as f64, d.0[2] as f64]);
            let c = dot(u, e);
            if c > best.0 + 1e-12 { best = (c, d); }
        }
        best.1
    }
}

impl Camera {
    /// Click on a ViewCube place: look from there, same target and distance.
    pub fn snap_to(&mut self, d: CubeDir) {
        let v = unit([d.0[0] as f64, d.0[1] as f64, d.0[2] as f64]);
        self.pitch = v[2].asin().clamp(-MAX_PITCH, MAX_PITCH);
        if v[0].abs() > 1e-9 || v[1].abs() > 1e-9 { self.yaw = v[1].atan2(v[0]); }
    }
}
