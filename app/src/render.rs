//! Flat-shaded polygon rendering: a triangle batcher with per-vertex distance fog,
//! procedural aircraft / ground-target models and world-to-screen projection.

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

pub const FOG_START: f32 = 4000.0;
pub const FOG_END: f32 = 16000.0;

pub fn horizon_color() -> Color {
    Color::new(0.72, 0.80, 0.90, 1.0)
}

pub fn light_dir() -> Vec3 {
    vec3(0.35, 0.85, 0.40).normalize()
}

/// Simple Lambert term, `two_sided` for thin plates (wings, fins).
pub fn shade(base: Color, n: Vec3, two_sided: bool) -> Color {
    let d = n.dot(light_dir());
    let d = if two_sided { d.abs() } else { d.max(0.0) };
    let k = 0.50 + 0.55 * d;
    Color::new(
        (base.r * k).min(1.0),
        (base.g * k).min(1.0),
        (base.b * k).min(1.0),
        base.a,
    )
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub struct Batch {
    verts: Vec<Vertex>,
    idx: Vec<u16>,
    pub cam: Vec3,
    pub fog: bool,
    pub fog_start: f32,
    pub fog_end: f32,
    pub tris_drawn: usize,
}

impl Batch {
    pub fn new() -> Self {
        Batch {
            verts: Vec::with_capacity(8192),
            idx: Vec::with_capacity(4800),
            cam: Vec3::ZERO,
            fog: true,
            fog_start: FOG_START,
            fog_end: FOG_END,
            tris_drawn: 0,
        }
    }

    fn fogged(&self, p: Vec3, c: Color) -> Color {
        if !self.fog {
            return c;
        }
        let d = (p - self.cam).length();
        let t = ((d - self.fog_start) / (self.fog_end - self.fog_start)).clamp(0.0, 1.0);
        mix(c, horizon_color(), t * t * (3.0 - 2.0 * t))
    }

    fn room(&mut self, v: usize, i: usize) {
        if self.verts.len() + v > 8000 || self.idx.len() + i > 4800 {
            self.flush();
        }
    }

    fn v(&self, p: Vec3, c: Color) -> Vertex {
        Vertex {
            position: p,
            uv: Vec2::ZERO,
            color: self.fogged(p, c).into(),
            normal: Vec4::ZERO,
        }
    }

    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Color) {
        self.room(3, 3);
        let base = self.verts.len() as u16;
        let va = self.v(a, col);
        let vb = self.v(b, col);
        let vc = self.v(c, col);
        self.verts.extend_from_slice(&[va, vb, vc]);
        self.idx.extend_from_slice(&[base, base + 1, base + 2]);
        self.tris_drawn += 1;
    }

    /// Triangle with per-vertex colours.
    #[allow(dead_code)]
    pub fn tri3(&mut self, a: Vec3, b: Vec3, c: Vec3, ca: Color, cb: Color, cc: Color) {
        self.room(3, 3);
        let base = self.verts.len() as u16;
        let va = self.v(a, ca);
        let vb = self.v(b, cb);
        let vc = self.v(c, cc);
        self.verts.extend_from_slice(&[va, vb, vc]);
        self.idx.extend_from_slice(&[base, base + 1, base + 2]);
        self.tris_drawn += 1;
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Color) {
        self.tri(a, b, c, col);
        self.tri(a, c, d, col);
    }

    /// A camera-facing square (sprites: tracers, flares, explosions, smoke).
    pub fn billboard(&mut self, p: Vec3, size: f32, right: Vec3, up: Vec3, col: Color) {
        let r = right * size;
        let u = up * size;
        self.quad(p - r - u, p + r - u, p + r + u, p - r + u, col);
    }

    /// A thin camera-facing streak from `a` to `b`.
    pub fn streak(&mut self, a: Vec3, b: Vec3, width: f32, col: Color) {
        let dir = b - a;
        let to_cam = (self.cam - a).normalize_or_zero();
        let side = dir.cross(to_cam).normalize_or_zero() * width;
        self.quad(a - side, a + side, b + side, b - side, col);
    }

    pub fn flush(&mut self) {
        if self.idx.is_empty() {
            return;
        }
        let mesh = Mesh {
            vertices: std::mem::take(&mut self.verts),
            indices: std::mem::take(&mut self.idx),
            texture: None,
        };
        draw_mesh(&mesh);
        let Mesh {
            mut vertices,
            mut indices,
            ..
        } = mesh;
        vertices.clear();
        indices.clear();
        self.verts = vertices;
        self.idx = indices;
    }
}

/// One polygon of a model in local space (forward +Z, up +Y, left +X).
#[derive(Clone)]
pub struct Face {
    pub p: [Vec3; 3],
    pub col: Color,
    pub n: Vec3,
    pub two_sided: bool,
}

pub struct Model {
    pub faces: Vec<Face>,
}

impl Model {
    fn new() -> Self {
        Model { faces: Vec::new() }
    }

    /// Solid face; normal is flipped to point away from `inside`.
    fn solid(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Color, inside: Vec3) {
        let mut n = (b - a).cross(c - a).normalize_or_zero();
        let centroid = (a + b + c) / 3.0;
        if n.dot(centroid - inside) < 0.0 {
            n = -n;
        }
        self.faces.push(Face {
            p: [a, b, c],
            col,
            n,
            two_sided: false,
        });
    }

    fn plate(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Color) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        self.faces.push(Face {
            p: [a, b, c],
            col,
            n,
            two_sided: true,
        });
    }

    fn plate4(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Color) {
        self.plate(a, b, c, col);
        self.plate(a, c, d, col);
    }

    /// Mirror-symmetric plate: given for the left side (+X), adds the right.
    fn plate4_sym(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Color) {
        self.plate4(a, b, c, d, col);
        let m = |v: Vec3| vec3(-v.x, v.y, v.z);
        self.plate4(m(a), m(b), m(c), m(d), col);
    }

    fn plate_sym(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Color) {
        self.plate(a, b, c, col);
        let m = |v: Vec3| vec3(-v.x, v.y, v.z);
        self.plate(m(a), m(b), m(c), col);
    }

    /// A fuselage made of rings (each ring: top, left, bottom, right).
    fn body(&mut self, rings: &[(f32, f32, f32, f32)], nose: Vec3, tail_z: f32, col: Color, belly: Color) {
        // ring: (z, half_width, top, bottom)
        let ring = |r: &(f32, f32, f32, f32)| {
            [
                vec3(0.0, r.2, r.0),
                vec3(r.1, 0.0, r.0),
                vec3(0.0, -r.3, r.0),
                vec3(-r.1, 0.0, r.0),
            ]
        };
        let first = ring(&rings[0]);
        for k in 0..4 {
            let c = if k == 1 || k == 2 { belly } else { col };
            let c = if k == 0 || k == 3 { col } else { c };
            self.solid(nose, first[k], first[(k + 1) % 4], c, vec3(0.0, 0.0, rings[0].0));
        }
        for w in rings.windows(2) {
            let a = ring(&w[0]);
            let b = ring(&w[1]);
            let mid = vec3(0.0, 0.0, (w[0].0 + w[1].0) * 0.5);
            for k in 0..4 {
                let c = if k == 1 || k == 2 { belly } else { col };
                self.solid(a[k], a[(k + 1) % 4], b[(k + 1) % 4], c, mid);
                self.solid(a[k], b[(k + 1) % 4], b[k], c, mid);
            }
        }
        let last = ring(rings.last().unwrap());
        let tail = vec3(0.0, 0.0, tail_z);
        for k in 0..4 {
            self.solid(last[k], last[(k + 1) % 4], tail, Color::new(0.15, 0.15, 0.17, 1.0), vec3(0.0, 0.0, tail_z + 2.0));
        }
    }

    /// Box between two corners.
    fn boxed(&mut self, lo: Vec3, hi: Vec3, col: Color) {
        let c = (lo + hi) * 0.5;
        let p = |x: f32, y: f32, z: f32| vec3(if x > 0.0 { hi.x } else { lo.x }, if y > 0.0 { hi.y } else { lo.y }, if z > 0.0 { hi.z } else { lo.z });
        let quads = [
            [p(-1., 1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(-1., 1., 1.)],
            [p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.)],
            [p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)],
            [p(-1., -1., -1.), p(1., -1., -1.), p(1., 1., -1.), p(-1., 1., -1.)],
            [p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(1., -1., 1.)],
            [p(-1., -1., -1.), p(-1., 1., -1.), p(-1., 1., 1.), p(-1., -1., 1.)],
        ];
        for q in quads.iter() {
            self.solid(q[0], q[1], q[2], col, c);
            self.solid(q[0], q[2], q[3], col, c);
        }
    }
}

/// Player jet: twin-tail delta with canards, grey-blue.
pub fn player_jet() -> Model {
    let mut m = Model::new();
    let body = Color::new(0.55, 0.62, 0.72, 1.0);
    let belly = Color::new(0.68, 0.72, 0.78, 1.0);
    let wing = Color::new(0.50, 0.57, 0.66, 1.0);
    let dark = Color::new(0.30, 0.34, 0.40, 1.0);
    m.body(
        &[(5.0, 0.75, 0.75, 0.55), (1.5, 1.1, 1.0, 0.75), (-3.0, 1.15, 0.85, 0.7), (-7.0, 0.9, 0.6, 0.55)],
        vec3(0.0, 0.0, 9.0),
        -7.6,
        body,
        belly,
    );
    // canopy
    let canopy = Color::new(0.10, 0.18, 0.30, 1.0);
    let c_top = vec3(0.0, 1.35, 2.2);
    m.solid(vec3(0.0, 0.7, 5.2), vec3(0.55, 0.85, 2.6), c_top, canopy, vec3(0.0, 0.5, 2.5));
    m.solid(vec3(0.0, 0.7, 5.2), c_top, vec3(-0.55, 0.85, 2.6), canopy, vec3(0.0, 0.5, 2.5));
    m.solid(vec3(0.55, 0.85, 2.6), vec3(0.0, 1.0, -0.5), c_top, canopy, vec3(0.0, 0.5, 1.5));
    m.solid(vec3(-0.55, 0.85, 2.6), c_top, vec3(0.0, 1.0, -0.5), canopy, vec3(0.0, 0.5, 1.5));
    // main delta wings
    m.plate4_sym(vec3(1.0, 0.0, 2.2), vec3(6.8, -0.15, -3.8), vec3(6.8, -0.15, -5.2), vec3(1.0, 0.0, -5.6), wing);
    // canards
    m.plate_sym(vec3(0.9, 0.2, 5.0), vec3(2.8, 0.25, 3.4), vec3(0.9, 0.2, 3.4), wing);
    // stabilisers
    m.plate_sym(vec3(0.9, 0.0, -5.6), vec3(3.6, -0.1, -7.6), vec3(0.9, 0.0, -7.6), wing);
    // twin canted fins
    m.plate4_sym(vec3(0.75, 0.6, -3.8), vec3(1.6, 3.8, -6.0), vec3(1.7, 3.8, -7.1), vec3(0.8, 0.5, -7.3), dark);
    // intakes
    m.boxed(vec3(0.9, -0.7, -1.5), vec3(1.5, 0.0, 2.0), dark);
    m.boxed(vec3(-1.5, -0.7, -1.5), vec3(-0.9, 0.0, 2.0), dark);
    m
}

/// Enemy fighter: swept wing, single tall fin, desert-red camouflage.
pub fn enemy_jet(ace: bool) -> Model {
    let mut m = Model::new();
    let body = if ace { Color::new(0.20, 0.20, 0.22, 1.0) } else { Color::new(0.62, 0.30, 0.22, 1.0) };
    let belly = if ace { Color::new(0.35, 0.33, 0.33, 1.0) } else { Color::new(0.72, 0.62, 0.50, 1.0) };
    let wing = if ace { Color::new(0.28, 0.27, 0.28, 1.0) } else { Color::new(0.55, 0.42, 0.26, 1.0) };
    let mark = if ace { Color::new(0.85, 0.75, 0.10, 1.0) } else { Color::new(0.85, 0.15, 0.10, 1.0) };
    m.body(
        &[(4.5, 0.6, 0.6, 0.55), (1.0, 0.95, 1.05, 0.75), (-4.0, 1.0, 0.75, 0.65), (-7.5, 0.8, 0.55, 0.5)],
        vec3(0.0, -0.1, 8.0),
        -8.2,
        body,
        belly,
    );
    let canopy = Color::new(0.15, 0.25, 0.20, 1.0);
    m.solid(vec3(0.0, 0.6, 4.2), vec3(0.5, 0.8, 1.6), vec3(0.0, 1.35, 1.2), canopy, vec3(0.0, 0.5, 2.0));
    m.solid(vec3(0.0, 0.6, 4.2), vec3(0.0, 1.35, 1.2), vec3(-0.5, 0.8, 1.6), canopy, vec3(0.0, 0.5, 2.0));
    // swept wings
    m.plate4_sym(vec3(0.95, 0.1, 0.5), vec3(6.2, -0.2, -4.2), vec3(6.2, -0.2, -5.3), vec3(0.95, 0.1, -3.6), wing);
    // wingtip markings
    m.plate4_sym(vec3(5.0, -0.17, -3.2), vec3(6.2, -0.21, -4.2), vec3(6.2, -0.21, -5.3), vec3(5.0, -0.17, -4.9), mark);
    // tail planes
    m.plate_sym(vec3(0.8, 0.0, -5.5), vec3(3.4, -0.1, -7.9), vec3(0.8, 0.0, -7.9), wing);
    // single fin
    m.plate4(vec3(0.0, 0.7, -3.6), vec3(0.0, 4.4, -6.6), vec3(0.0, 4.4, -7.8), vec3(0.0, 0.5, -7.9), body);
    m.plate(vec3(0.0, 3.2, -6.0), vec3(0.0, 4.4, -6.6), vec3(0.0, 4.4, -7.8), mark);
    m
}

pub fn missile_model() -> Model {
    let mut m = Model::new();
    let c = Color::new(0.92, 0.92, 0.88, 1.0);
    m.body(&[(1.2, 0.12, 0.12, 0.12), (-1.4, 0.12, 0.12, 0.12)], vec3(0.0, 0.0, 1.7), -1.5, c, c);
    let f = Color::new(0.5, 0.5, 0.5, 1.0);
    m.plate_sym(vec3(0.1, 0.0, -0.8), vec3(0.45, 0.0, -1.4), vec3(0.1, 0.0, -1.4), f);
    m.plate(vec3(0.0, 0.1, -0.8), vec3(0.0, 0.45, -1.4), vec3(0.0, 0.1, -1.4), f);
    m.plate(vec3(0.0, -0.1, -0.8), vec3(0.0, -0.45, -1.4), vec3(0.0, -0.1, -1.4), f);
    m
}

/// Ground targets.
pub fn tank_model() -> Model {
    let mut m = Model::new();
    let g = Color::new(0.38, 0.42, 0.25, 1.0);
    let d = Color::new(0.25, 0.27, 0.18, 1.0);
    m.boxed(vec3(-2.0, 0.0, -3.5), vec3(2.0, 1.4, 3.5), g);
    m.boxed(vec3(-1.3, 1.4, -1.6), vec3(1.3, 2.4, 1.4), d);
    m.boxed(vec3(-0.15, 1.8, 1.4), vec3(0.15, 2.1, 5.0), d);
    m
}

pub fn sam_model() -> Model {
    let mut m = Model::new();
    let g = Color::new(0.45, 0.45, 0.32, 1.0);
    let w = Color::new(0.85, 0.85, 0.80, 1.0);
    m.boxed(vec3(-2.2, 0.0, -5.0), vec3(2.2, 2.0, 5.0), g);
    m.boxed(vec3(-1.8, 2.0, 2.0), vec3(1.8, 3.4, 4.6), g);
    // launch rails (tilted)
    for x in [-1.2f32, 0.0, 1.2] {
        m.solid(vec3(x - 0.3, 2.2, -4.0), vec3(x + 0.3, 2.2, -4.0), vec3(x, 5.5, 1.0), w, vec3(x, 2.0, -2.0));
        m.solid(vec3(x - 0.3, 2.2, -4.0), vec3(x, 5.5, 1.0), vec3(x - 0.3, 2.6, -4.2), w, vec3(x + 1.0, 2.0, -2.0));
    }
    // radar
    m.plate4(vec3(-2.0, 3.4, 3.0), vec3(2.0, 3.4, 3.0), vec3(2.0, 6.0, 3.6), vec3(-2.0, 6.0, 3.6), Color::new(0.6, 0.6, 0.5, 1.0));
    m
}

pub fn hangar_model() -> Model {
    let mut m = Model::new();
    let w = Color::new(0.62, 0.60, 0.55, 1.0);
    let r = Color::new(0.45, 0.30, 0.25, 1.0);
    m.boxed(vec3(-14.0, 0.0, -20.0), vec3(14.0, 8.0, 20.0), w);
    // pitched roof
    let mid = vec3(0.0, 4.0, 0.0);
    m.solid(vec3(-14.0, 8.0, -20.0), vec3(0.0, 13.0, -20.0), vec3(0.0, 13.0, 20.0), r, mid);
    m.solid(vec3(-14.0, 8.0, -20.0), vec3(0.0, 13.0, 20.0), vec3(-14.0, 8.0, 20.0), r, mid);
    m.solid(vec3(14.0, 8.0, -20.0), vec3(0.0, 13.0, -20.0), vec3(0.0, 13.0, 20.0), r, mid);
    m.solid(vec3(14.0, 8.0, -20.0), vec3(0.0, 13.0, 20.0), vec3(14.0, 8.0, 20.0), r, mid);
    m.solid(vec3(-14.0, 8.0, 20.0), vec3(14.0, 8.0, 20.0), vec3(0.0, 13.0, 20.0), w, mid);
    m.solid(vec3(-14.0, 8.0, -20.0), vec3(14.0, 8.0, -20.0), vec3(0.0, 13.0, -20.0), w, mid);
    m.boxed(vec3(-9.0, 0.0, 20.0), vec3(9.0, 6.5, 20.3), Color::new(0.2, 0.2, 0.22, 1.0));
    m
}

/// Same model with every colour multiplied (other players' jets).
pub fn tinted(mut m: Model, tint: Color) -> Model {
    for f in m.faces.iter_mut() {
        f.col = Color::new((f.col.r * tint.r).min(1.0), (f.col.g * tint.g).min(1.0), (f.col.b * tint.b).min(1.0), f.col.a);
    }
    m
}

/// Draw a model with transform (position, rotation, scale) and optional flash tint.
pub fn draw_model(b: &mut Batch, m: &Model, pos: Vec3, rot: Quat, scale: f32, flash: f32) {
    let white = Color::new(1.0, 0.9, 0.6, 1.0);
    for f in &m.faces {
        let n = rot * f.n;
        let mut c = shade(f.col, n, f.two_sided);
        if flash > 0.0 {
            c = mix(c, white, flash);
        }
        let a = pos + rot * (f.p[0] * scale);
        let bb = pos + rot * (f.p[1] * scale);
        let cc = pos + rot * (f.p[2] * scale);
        b.tri(a, bb, cc, c);
    }
}

/// Project a world point to screen pixels; None when behind the camera.
pub fn project(view_proj: &Mat4, p: Vec3, sw: f32, sh: f32) -> Option<Vec2> {
    let clip = *view_proj * vec4(p.x, p.y, p.z, 1.0);
    if clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(vec2((ndc.x + 1.0) * 0.5 * sw, (1.0 - ndc.y) * 0.5 * sh))
}

/// Builds an orientation from forward and approximate up (local fwd = +Z, up = +Y, left = +X).
pub fn quat_from_fwd_up(fwd: Vec3, up: Vec3) -> Quat {
    let f = fwd.normalize_or_zero();
    let mut x = up.cross(f);
    if x.length_squared() < 1e-6 {
        x = Vec3::Y.cross(f);
        if x.length_squared() < 1e-6 {
            x = Vec3::X;
        }
    }
    let x = x.normalize();
    let y = f.cross(x).normalize();
    Quat::from_mat3(&Mat3::from_cols(x, y, f)).normalize()
}

/// Rotate vector `from` toward `to` by at most `max_angle` radians.
pub fn rotate_toward(from: Vec3, to: Vec3, max_angle: f32) -> Vec3 {
    let a = from.normalize_or_zero();
    let b = to.normalize_or_zero();
    let dot = a.dot(b).clamp(-1.0, 1.0);
    let ang = dot.acos();
    if ang <= max_angle || ang < 1e-5 {
        return b;
    }
    let mut axis = a.cross(b);
    if axis.length_squared() < 1e-8 {
        axis = if a.y.abs() < 0.9 { a.cross(Vec3::Y) } else { a.cross(Vec3::X) };
    }
    Quat::from_axis_angle(axis.normalize(), max_angle) * a
}
