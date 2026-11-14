//! Endless procedural terrain: patchwork fields, rivers of colour and polygon mountains.
//! Everything is derived from a hash of the tile coordinates, so the world never ends.

use crate::render::{mix, shade, Batch};
use macroquad::prelude::*;

pub const CELL: f32 = 500.0; // ground patch size (m)
pub const TILE: f32 = 2400.0; // mountain placement tile (m)
pub const GRID_RADIUS: i32 = 34; // ground cells drawn around the camera

pub fn hash2(x: i32, z: i32, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (z as u32).wrapping_mul(0xd8163841) ^ salt.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    h
}

pub fn hf(x: i32, z: i32, salt: u32) -> f32 {
    (hash2(x, z, salt) & 0xffff) as f32 / 65535.0
}

#[derive(Clone, Copy)]
pub struct Mountain {
    pub c: Vec2,
    pub r: f32,
    pub h: f32,
    pub sides: u32,
    pub rot: f32,
    pub snow: bool,
}

pub fn mountain_in_tile(tx: i32, tz: i32) -> Option<Mountain> {
    if tx.abs() <= 1 && tz.abs() <= 1 {
        return None; // keep the start area open
    }
    if hf(tx, tz, 1) > 0.42 {
        return None;
    }
    let r = 500.0 + hf(tx, tz, 2) * 700.0;
    let h = 300.0 + hf(tx, tz, 3) * 1300.0;
    let margin = r;
    let cx = tx as f32 * TILE + margin + hf(tx, tz, 4) * (TILE - 2.0 * margin).max(1.0);
    let cz = tz as f32 * TILE + margin + hf(tx, tz, 5) * (TILE - 2.0 * margin).max(1.0);
    Some(Mountain {
        c: vec2(cx, cz),
        r,
        h,
        sides: 5 + (hash2(tx, tz, 6) % 3),
        rot: hf(tx, tz, 7) * 6.28,
        snow: h > 1000.0,
    })
}

/// Ground height at (x, z): 0 on the plains, conical inside mountains.
pub fn height_at(x: f32, z: f32) -> f32 {
    let tx = (x / TILE).floor() as i32;
    let tz = (z / TILE).floor() as i32;
    let mut best = 0.0f32;
    for dx in -1..=1 {
        for dz in -1..=1 {
            if let Some(m) = mountain_in_tile(tx + dx, tz + dz) {
                let d = vec2(x, z).distance(m.c);
                // inscribed radius of the polygon base keeps collision fair
                let ri = m.r * (std::f32::consts::PI / m.sides as f32).cos();
                if d < ri {
                    best = best.max(m.h * (1.0 - d / ri));
                }
            }
        }
    }
    best
}

fn cell_color(cx: i32, cz: i32) -> Color {
    let v = hf(cx, cz, 11);
    let palette = [
        Color::new(0.32, 0.52, 0.22, 1.0),
        Color::new(0.38, 0.58, 0.25, 1.0),
        Color::new(0.27, 0.45, 0.20, 1.0),
        Color::new(0.55, 0.52, 0.28, 1.0),
        Color::new(0.62, 0.50, 0.25, 1.0),
        Color::new(0.30, 0.48, 0.28, 1.0),
    ];
    let mut c = palette[(v * palette.len() as f32) as usize % palette.len()];
    // a meandering river along a sine path
    let x = cx as f32 * CELL;
    let river_z = (x * 0.00035).sin() * 6000.0 + 3500.0;
    let z = cz as f32 * CELL;
    if (z - river_z).abs() < CELL * 0.6 {
        c = Color::new(0.20, 0.35, 0.60, 1.0);
    }
    c
}

/// Phones draw fewer ground patches (shorter, foggier view) to keep the frame rate up.
pub const GRID_RADIUS_MOBILE: i32 = 22;

pub fn draw_ground(b: &mut Batch, cam: Vec3, radius: i32) {
    let ccx = (cam.x / CELL).floor() as i32;
    let ccz = (cam.z / CELL).floor() as i32;
    let r = radius;
    for gz in -r..r {
        for gx in -r..r {
            let cx = ccx + gx;
            let cz = ccz + gz;
            let x0 = cx as f32 * CELL;
            let z0 = cz as f32 * CELL;
            let col = cell_color(cx, cz);
            let a = vec3(x0, 0.0, z0);
            let bb = vec3(x0 + CELL, 0.0, z0);
            let c = vec3(x0 + CELL, 0.0, z0 + CELL);
            let d = vec3(x0, 0.0, z0 + CELL);
            b.quad(a, bb, c, d, col);
        }
    }
    // runway near the origin: the home airfield
    let rw = Color::new(0.30, 0.30, 0.32, 1.0);
    b.quad(vec3(-30.0, 0.3, -1200.0), vec3(30.0, 0.3, -1200.0), vec3(30.0, 0.3, 1200.0), vec3(-30.0, 0.3, 1200.0), rw);
    let mut z = -1100.0;
    while z < 1100.0 {
        b.quad(vec3(-1.5, 0.5, z), vec3(1.5, 0.5, z), vec3(1.5, 0.5, z + 40.0), vec3(-1.5, 0.5, z + 40.0), WHITE);
        z += 90.0;
    }
}

pub fn draw_mountains(b: &mut Batch, cam: Vec3, r: i32) {
    let tx = (cam.x / TILE).floor() as i32;
    let tz = (cam.z / TILE).floor() as i32;
    let rock = Color::new(0.48, 0.42, 0.36, 1.0);
    let rock2 = Color::new(0.40, 0.38, 0.34, 1.0);
    let snow = Color::new(0.92, 0.93, 0.96, 1.0);
    for dz in -r..=r {
        for dx in -r..=r {
            if let Some(m) = mountain_in_tile(tx + dx, tz + dz) {
                let peak = vec3(m.c.x, m.h, m.c.y);
                let n = m.sides as usize;
                for i in 0..n {
                    let a0 = m.rot + i as f32 / n as f32 * std::f32::consts::TAU;
                    let a1 = m.rot + (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
                    let p0 = vec3(m.c.x + a0.cos() * m.r, 0.0, m.c.y + a0.sin() * m.r);
                    let p1 = vec3(m.c.x + a1.cos() * m.r, 0.0, m.c.y + a1.sin() * m.r);
                    let mut nrm = (p1 - p0).cross(peak - p0).normalize_or_zero();
                    if nrm.y < 0.0 {
                        nrm = -nrm;
                    }
                    let base = if i % 2 == 0 { rock } else { rock2 };
                    if m.snow {
                        // lower band in rock, snowy cap above 65 %
                        let k = 0.62;
                        let q0 = p0.lerp(peak, k);
                        let q1 = p1.lerp(peak, k);
                        let c = shade(base, nrm, false);
                        b.quad(p0, p1, q1, q0, c);
                        b.tri(q0, q1, peak, shade(snow, nrm, false));
                    } else {
                        b.tri(p0, p1, peak, shade(base, nrm, false));
                    }
                }
            }
        }
    }
}

/// Sky gradient drawn in screen space before the 3D pass. The horizon line follows the camera roll/pitch.
pub fn draw_sky(view_proj: &Mat4, cam: Vec3, fwd: Vec3, sw: f32, sh: f32) {
    let top = Color::new(0.10, 0.22, 0.52, 1.0);
    let mid = Color::new(0.38, 0.55, 0.82, 1.0);
    let hor = crate::render::horizon_color();
    clear_background(top);
    // Project two far points on the horizon to find its screen line.
    let hf = vec3(fwd.x, 0.0, fwd.z).normalize_or(Vec3::Z);
    let side = hf.cross(Vec3::Y);
    let far = 60000.0;
    let p0 = crate::render::project(view_proj, cam + hf * far + side * far * 0.6 - Vec3::Y * cam.y, sw, sh);
    let p1 = crate::render::project(view_proj, cam + hf * far - side * far * 0.6 - Vec3::Y * cam.y, sw, sh);
    // "up" direction on screen: project a point above the horizon.
    let pu = crate::render::project(view_proj, cam + hf * far + Vec3::Y * far * 0.5, sw, sh);
    if let (Some(a), Some(bp)) = (p0, p1) {
        let dir = (bp - a).normalize_or(vec2(1.0, 0.0));
        let mut nrm = vec2(-dir.y, dir.x);
        if let Some(u) = pu {
            if nrm.dot(u - a) < 0.0 {
                nrm = -nrm;
            }
        }
        // Draw gradient bands as long quads parallel to the horizon.
        let big = (sw + sh) * 2.0;
        let center = a + dir * ((vec2(sw, sh) * 0.5 - a).dot(dir));
        let bands = [(0.0, hor), (sh * 0.12, mid), (sh * 0.9, top)];
        for i in 0..bands.len() - 1 {
            let (d0, c0) = bands[i];
            let (d1, c1) = bands[i + 1];
            let s = 6;
            for k in 0..s {
                let t0 = k as f32 / s as f32;
                let t1 = (k + 1) as f32 / s as f32;
                let o0 = center + nrm * (d0 + (d1 - d0) * t0);
                let o1 = center + nrm * (d0 + (d1 - d0) * t1);
                let col = mix(c0, c1, (t0 + t1) * 0.5);
                let q = [o0 - dir * big, o0 + dir * big, o1 + dir * big, o1 - dir * big];
                draw_triangle(q[0], q[1], q[2], col);
                draw_triangle(q[0], q[2], q[3], col);
            }
        }
        // below the horizon (ground fills it anyway, but avoid gaps at extreme angles)
        let o0 = center;
        let o1 = center - nrm * big;
        let q = [o0 - dir * big, o0 + dir * big, o1 + dir * big, o1 - dir * big];
        draw_triangle(q[0], q[1], q[2], hor);
        draw_triangle(q[0], q[2], q[3], hor);
    } else {
        // Looking straight up or down: plain colour.
        let c = if fwd.y > 0.0 { top } else { hor };
        clear_background(c);
    }
}
