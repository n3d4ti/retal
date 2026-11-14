//! Head-up display, cockpit panel (radar, weapons, damage) and the phone's on-screen controls.

use crate::game::*;
use crate::input::{touch_layout, Btn, InputState};
use crate::render::project;
use macroquad::prelude::*;

pub fn hud_green() -> Color {
    Color::new(0.35, 1.0, 0.45, 0.95)
}
pub fn hud_dim() -> Color {
    Color::new(0.35, 1.0, 0.45, 0.45)
}
pub fn hud_red() -> Color {
    Color::new(1.0, 0.3, 0.22, 1.0)
}
pub fn hud_amber() -> Color {
    Color::new(1.0, 0.8, 0.25, 1.0)
}

#[derive(Default, Clone, Copy, Debug)]
pub struct HudFlags {
    pub speed: bool,
    pub altitude: bool,
    pub heading: bool,
    pub score: bool,
    pub health: bool,
    pub weapon: bool,
    pub radar: bool,
    pub lives: bool,
    pub wave: bool,
    pub touch_buttons: bool,
}

pub struct View {
    pub vp: Mat4,
    pub eye: Vec3,
    pub fwd: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub sw: f32,
    pub sh: f32,
}

pub fn ui_scale(sw: f32, sh: f32) -> f32 {
    (sw.min(sh) / 640.0).clamp(0.55, 2.2)
}

pub fn text(t: &str, x: f32, y: f32, size: f32, col: Color) {
    draw_text(t, x, y, size, col);
}

pub fn text_c(t: &str, cx: f32, y: f32, size: f32, col: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(t, cx - d.width * 0.5, y, size, col);
}

pub fn text_r(t: &str, rx: f32, y: f32, size: f32, col: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(t, rx - d.width, y, size, col);
}

fn shadow_text_c(t: &str, cx: f32, y: f32, size: f32, col: Color) {
    text_c(t, cx + 2.0, y + 2.0, size, Color::new(0.0, 0.0, 0.0, 0.6));
    text_c(t, cx, y, size, col);
}

pub fn heading_deg(fwd: Vec3) -> f32 {
    let h = (-fwd.x).atan2(fwd.z).to_degrees();
    (h + 360.0) % 360.0
}

pub fn draw_hud(g: &Game, v: &View, inp: &InputState, flags: &mut HudFlags, t: f32) {
    let sw = v.sw;
    let sh = v.sh;
    let s = ui_scale(sw, sh);
    let fs = 20.0 * s;
    let green = hud_green();
    let p = &g.player;
    let touch = inp.touch_mode;
    let cx = sw * 0.5;
    let cy = sh * if g.cockpit && !touch { 0.42 } else { 0.47 };

    // damage flash vignette
    if p.hit_flash > 0.0 {
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.9, 0.1, 0.05, 0.28 * p.hit_flash));
    }

    if p.alive && g.cockpit {
        draw_pitch_ladder(v, cx, cy, s);
        // boresight cross
        let r = 10.0 * s;
        let bc = vec2(cx, cy);
        if let Some(b) = project(&v.vp, v.eye + p.fwd() * 2000.0, sw, sh) {
            let _ = bc;
            draw_line(b.x - r * 2.0, b.y, b.x - r * 0.6, b.y, 2.0, green);
            draw_line(b.x + r * 0.6, b.y, b.x + r * 2.0, b.y, 2.0, green);
            draw_line(b.x, b.y - r * 0.6, b.x, b.y - r * 1.6, 2.0, green);
            draw_circle_lines(b.x, b.y, 2.0, 1.5, green);
        }
    } else if p.alive {
        // chase view: small aiming reticle where the guns point
        if let Some(b) = project(&v.vp, p.pos + p.fwd() * 1500.0, sw, sh) {
            draw_circle_lines(b.x, b.y, 10.0 * s, 1.5, green);
            draw_circle(b.x, b.y, 2.0, green);
        }
    }

    draw_contacts(g, v, s, t);

    // ---- heading tape (top) ----
    let hd = heading_deg(p.fwd());
    let tape_w = (sw * 0.36).min(520.0 * s);
    let ty = 34.0 * s + if touch { 6.0 * s } else { 0.0 };
    let px_per_deg = tape_w / 60.0;
    let mut d = (hd / 5.0).floor() * 5.0 - 30.0;
    while d <= hd + 30.0 {
        let x = cx + (d - hd) * px_per_deg;
        let major = (d.rem_euclid(10.0)).abs() < 0.01;
        let h = if major { 10.0 } else { 5.0 } * s;
        draw_line(x, ty, x, ty + h, 1.5, green);
        if major {
            let dd = d.rem_euclid(360.0) as i32;
            let lab = match dd {
                0 => "N".to_string(),
                90 => "E".to_string(),
                180 => "S".to_string(),
                270 => "W".to_string(),
                _ => format!("{:02}", dd / 10),
            };
            text_c(&lab, x, ty - 4.0 * s, fs * 0.8, green);
        }
        d += 5.0;
    }
    draw_triangle(vec2(cx, ty + 12.0 * s), vec2(cx - 6.0 * s, ty + 20.0 * s), vec2(cx + 6.0 * s, ty + 20.0 * s), green);
    text_c(&format!("{:03}", hd.round() as i32 % 360), cx, ty + 36.0 * s, fs, green);
    flags.heading = true;

    // ---- speed (left) and altitude (right) boxes ----
    let bx_w = 84.0 * s;
    let bx_h = 30.0 * s;
    let lx = cx - (sw * 0.30).min(360.0 * s) - bx_w * 0.5;
    let rx = cx + (sw * 0.30).min(360.0 * s) - bx_w * 0.5;
    let knots = (p.speed * 1.944).round() as i32;
    let feet = (p.pos.y * 3.281).round() as i32;
    draw_rectangle_lines(lx, cy - bx_h * 0.5, bx_w, bx_h, 2.0, green);
    text_c(&format!("{}", knots), lx + bx_w * 0.5, cy + fs * 0.35, fs * 1.1, green);
    text_c("KTS", lx + bx_w * 0.5, cy + bx_h * 0.5 + fs * 0.9, fs * 0.75, hud_dim());
    flags.speed = true;
    draw_rectangle_lines(rx, cy - bx_h * 0.5, bx_w, bx_h, 2.0, green);
    text_c(&format!("{}", feet), rx + bx_w * 0.5, cy + fs * 0.35, fs * 1.1, green);
    text_c("FT", rx + bx_w * 0.5, cy + bx_h * 0.5 + fs * 0.9, fs * 0.75, hud_dim());
    flags.altitude = true;
    // radar altitude + G + throttle
    let agl = ((p.pos.y - crate::world::height_at(p.pos.x, p.pos.z)) * 3.281) as i32;
    text_c(&format!("R {}", agl.max(0)), rx + bx_w * 0.5, cy + bx_h * 0.5 + fs * 1.8, fs * 0.75, hud_dim());
    text_c(&format!("G {:.1}", p.g_load), lx + bx_w * 0.5, cy - bx_h * 0.5 - fs * 0.5, fs * 0.8, green);
    let thr = (p.throttle * 100.0).round() as i32;
    let ab = if p.throttle > 0.9 { " AB" } else { "" };
    text_c(&format!("THR {}%{}", thr, ab), lx + bx_w * 0.5, cy + bx_h * 0.5 + fs * 1.8, fs * 0.75, if p.throttle > 0.9 { hud_amber() } else { hud_dim() });

    // ---- score / wave / lives (top corners) ----
    let top_y = 26.0 * s;
    let left_x = if touch { 14.0 * s } else { 16.0 * s };
    text(&format!("SCORE {:07}", g.score), left_x, top_y, fs, green);
    flags.score = true;
    text(&format!("WAVE {}   KILLS {}/{}   {}", g.wave, g.wave_kills.min(wave_quota(g.wave)), wave_quota(g.wave), g.d().name), left_x, top_y + fs * 1.1, fs * 0.8, green);
    flags.wave = true;
    let lives_x = if touch { sw - 110.0 * s } else { sw - 16.0 * s };
    let mut lx2 = lives_x;
    if g.online() {
        // endless respawns online: show the tally instead of lives
        let mode = if g.mode == MatchMode::Dogfight { "DOGFIGHT" } else { "CO-OP" };
        text_r(&format!("{}  K {}  D {}", mode, g.kills + g.player_kills, g.deaths), lives_x, top_y, fs * 0.85, green);
        if !touch {
            draw_scoreboard(g, sw - 16.0 * s, top_y + fs * 1.0, s);
        }
    } else {
        for _ in 0..g.lives.max(0).min(6) {
            draw_jet_icon(lx2 - 10.0 * s, top_y - 6.0 * s, 8.0 * s, green);
            lx2 -= 22.0 * s;
        }
        if !touch {
            text_r(&format!("KILLS {}  GND {}", g.kills, g.ground_kills), sw - 16.0 * s, top_y + fs * 1.1, fs * 0.8, green);
        }
    }
    flags.lives = true;

    // ---- warnings ----
    let blink = (t * 4.0).fract() < 0.6;
    let mut wy = cy + 70.0 * s;
    if g.incoming && blink {
        shadow_text_c("MISSILE", cx, wy, fs * 1.4, hud_red());
        wy += fs * 1.4;
    }
    if p.alive {
        let agl_m = p.pos.y - crate::world::height_at(p.pos.x, p.pos.z);
        let fwd = p.fwd();
        if (agl_m < 300.0 && fwd.y < 0.0) || (agl_m + fwd.y * p.speed * 4.0) < 50.0 {
            if blink {
                shadow_text_c("PULL UP", cx, wy, fs * 1.4, hud_red());
            }
            wy += fs * 1.4;
        }
        if p.stalled && blink {
            shadow_text_c("STALL", cx, wy, fs * 1.2, hud_amber());
            wy += fs * 1.2;
        }
        if p.overheated {
            shadow_text_c("GUN OVERHEAT", cx, wy, fs, hud_amber());
        }
        if p.shield > 0.0 && blink {
            text_c("SHIELD", cx, cy - 60.0 * s, fs * 0.8, hud_dim());
        }
    }

    // ---- messages ----
    let mut my = sh * 0.25;
    for m in &g.messages {
        let a = m.t.min(1.0);
        let c = Color::new(m.col.r, m.col.g, m.col.b, a);
        shadow_text_c(&m.text, cx, my, fs * 1.15, c);
        my += fs * 1.3;
    }

    // ---- cockpit panel / instruments ----
    if g.cockpit && !touch {
        let ph = sh * 0.2;
        let py = sh - ph;
        draw_rectangle(0.0, py, sw, ph, Color::new(0.11, 0.12, 0.13, 1.0));
        draw_line(0.0, py, sw, py, 3.0, Color::new(0.25, 0.27, 0.3, 1.0));
        let r = ph * 0.42;
        draw_radar(g, vec2(cx, py + ph * 0.5), r, s, flags);
        let mfd_w = (sw * 0.28).min(380.0 * s);
        draw_weapons(g, Rect::new(cx - r - 20.0 * s - mfd_w, py + ph * 0.1, mfd_w, ph * 0.8), s, flags);
        draw_status(g, Rect::new(cx + r + 20.0 * s, py + ph * 0.1, mfd_w, ph * 0.8), s, flags);
    } else {
        // compact instruments for chase view and phones
        let r = 62.0 * s;
        let rc = if touch { vec2(sw * 0.5, sh - r - 12.0 * s) } else { vec2(sw - r - 16.0 * s, sh - r - 16.0 * s) };
        draw_radar(g, rc, r, s, flags);
        let w = 200.0 * s;
        let wr = if touch { Rect::new(cx - w * 0.5, ty + 50.0 * s, w, 70.0 * s) } else { Rect::new(16.0 * s, sh - 120.0 * s, w * 1.3, 104.0 * s) };
        if touch {
            draw_weapon_compact(g, wr, s, flags);
            draw_health_bar(g, Rect::new(left_x, top_y + fs * 1.6, 150.0 * s, 10.0 * s), s, flags);
        } else {
            draw_weapons(g, wr, s, flags);
            draw_health_bar(g, Rect::new(16.0 * s, sh - 140.0 * s, w * 1.3, 10.0 * s), s, flags);
        }
    }

    if touch {
        draw_touch_controls(g, inp, sw, sh, s, flags);
    }

    if !p.alive && !g.game_over {
        shadow_text_c(&format!("RESPAWN IN {:.0}", p.respawn_t.max(0.0).ceil()), cx, sh * 0.5, fs * 1.4, hud_amber());
    }
}

fn draw_jet_icon(x: f32, y: f32, r: f32, c: Color) {
    draw_triangle(vec2(x, y - r), vec2(x - r, y + r * 0.7), vec2(x + r, y + r * 0.7), c);
}

fn draw_pitch_ladder(v: &View, cx: f32, cy: f32, s: f32) {
    let green = hud_green();
    let hf = vec3(v.fwd.x, 0.0, v.fwd.z).normalize_or(Vec3::Z);
    let hr = hf.cross(Vec3::Y);
    let clip = Rect::new(cx - 260.0 * s, cy - 200.0 * s, 520.0 * s, 400.0 * s);
    let cur_pitch = v.fwd.y.clamp(-1.0, 1.0).asin().to_degrees();
    let mut a = ((cur_pitch / 10.0).round() as i32 - 3) * 10;
    let a_end = a + 60;
    while a <= a_end {
        if a < -90 || a > 90 {
            a += 10;
            continue;
        }
        let rad = (a as f32).to_radians();
        let dir = hf * rad.cos() + Vec3::Y * rad.sin();
        let c = v.eye + dir * 1000.0;
        let half = if a == 0 { 140.0 } else { 70.0 };
        let gap = if a == 0 { 30.0 } else { 22.0 };
        let p_l0 = project(&v.vp, c - hr * half, v.sw, v.sh);
        let p_l1 = project(&v.vp, c - hr * gap, v.sw, v.sh);
        let p_r0 = project(&v.vp, c + hr * gap, v.sw, v.sh);
        let p_r1 = project(&v.vp, c + hr * half, v.sw, v.sh);
        if let (Some(l0), Some(l1), Some(r0), Some(r1)) = (p_l0, p_l1, p_r0, p_r1) {
            let mid = (l1 + r0) * 0.5;
            if clip.contains(mid) {
                if a < 0 {
                    dashed(l0, l1, green);
                    dashed(r0, r1, green);
                } else {
                    draw_line(l0.x, l0.y, l1.x, l1.y, 2.0, green);
                    draw_line(r0.x, r0.y, r1.x, r1.y, 2.0, green);
                }
                if a != 0 {
                    // end ticks pointing toward the horizon
                    let along = (r1 - r0).normalize_or(vec2(1.0, 0.0));
                    let n = vec2(along.y, -along.x) * if a > 0 { -6.0 * s } else { 6.0 * s };
                    draw_line(l0.x, l0.y, l0.x + n.x, l0.y + n.y, 2.0, green);
                    draw_line(r1.x, r1.y, r1.x + n.x, r1.y + n.y, 2.0, green);
                    let lab = format!("{}", a.abs());
                    draw_text(&lab, r1.x + 4.0, r1.y + 5.0, 16.0 * s, green);
                    let d = measure_text(&lab, None, (16.0 * s) as u16, 1.0);
                    draw_text(&lab, l0.x - d.width - 4.0, l0.y + 5.0, 16.0 * s, green);
                }
            }
        }
        a += 10;
    }
}

fn dashed(a: Vec2, b: Vec2, c: Color) {
    let n = 4;
    for i in 0..n {
        if i % 2 == 0 {
            let p0 = a.lerp(b, i as f32 / n as f32);
            let p1 = a.lerp(b, (i + 1) as f32 / n as f32);
            draw_line(p0.x, p0.y, p1.x, p1.y, 2.0, c);
        }
    }
}

fn draw_contacts(g: &Game, v: &View, s: f32, t: f32) {
    let p = &g.player;
    let red = hud_red();
    // enemies: brackets
    for e in &g.enemies {
        if let Some(sp) = project(&v.vp, e.pos, v.sw, v.sh) {
            let d = e.pos.distance(v.eye);
            let r = (9.0 * s).max(900.0 * s / d.max(1.0) * 9.0).min(40.0 * s);
            let c = if e.ace { hud_amber() } else { red };
            corner_box(sp, r, c);
            if d > 1500.0 {
                draw_circle(sp.x, sp.y, 1.5 * s, c);
            }
        }
    }
    for gt in g.ground.iter().filter(|g| g.alive) {
        let d = gt.pos.distance(v.eye);
        if d < 12000.0 {
            if let Some(sp) = project(&v.vp, gt.pos, v.sw, v.sh) {
                let r = 6.0 * s;
                let c = if gt.kind == GroundKind::Sam { red } else { hud_amber() };
                draw_triangle_lines(vec2(sp.x, sp.y - r), vec2(sp.x - r, sp.y + r * 0.7), vec2(sp.x + r, sp.y + r * 0.7), 1.5, c);
            }
        }
    }
    // other pilots: blue wingmen in co-op, magenta rivals in a dogfight
    for r in g.remotes.iter().filter(|r| r.alive) {
        if let Some(sp) = project(&v.vp, r.pos, v.sw, v.sh) {
            let c = remote_color(g);
            let d = r.pos.distance(v.eye);
            let rr = (10.0 * s).max(900.0 * s / d.max(1.0) * 9.0).min(40.0 * s);
            draw_circle_lines(sp.x, sp.y, rr, 1.5, c);
            text_c(&r.name, sp.x, sp.y - rr - 4.0 * s, 15.0 * s, c);
            text_c(&format!("{:.1}", d / 1852.0), sp.x, sp.y + rr + 14.0 * s, 13.0 * s, c);
        }
    }
    for r in &g.remotes {
        for (mp, _) in &r.missiles {
            if let Some(sp) = project(&v.vp, *mp, v.sw, v.sh) {
                draw_circle(sp.x, sp.y, 2.0 * s, remote_color(g));
            }
        }
    }
    // incoming missiles
    for m in g.ext_missiles.iter().filter(|m| m.at_me) {
        if let Some(sp) = project(&v.vp, m.pos, v.sw, v.sh) {
            if (t * 6.0).fract() < 0.5 {
                draw_circle_lines(sp.x, sp.y, 7.0 * s, 2.0, red);
            }
        }
    }
    for m in g.missiles.iter().filter(|m| !m.player) {
        if let Some(sp) = project(&v.vp, m.pos, v.sw, v.sh) {
            if (t * 6.0).fract() < 0.5 {
                draw_circle_lines(sp.x, sp.y, 7.0 * s, 2.0, red);
            }
        }
    }
    // current target box + lock
    if !p.alive {
        return;
    }
    let _ = &WEAPONS[p.weapon];
    if let Some(tr) = p.target {
        if let Some(tp) = g.target_pos(tr) {
            let dist = tp.distance(p.pos);
            let green = hud_green();
            match project(&v.vp, tp, v.sw, v.sh) {
                Some(sp) if sp.x > 0.0 && sp.x < v.sw && sp.y > 0.0 && sp.y < v.sh => {
                    let r = 16.0 * s;
                    draw_rectangle_lines(sp.x - r, sp.y - r, r * 2.0, r * 2.0, 2.0, green);
                    let txt = format!("{:.1}", dist / 1852.0);
                    text_c(&txt, sp.x, sp.y + r + 16.0 * s, 16.0 * s, green);
                    if p.locked {
                        let rr = r * 1.5;
                        let col = if (t * 8.0).fract() < 0.5 { green } else { hud_amber() };
                        draw_line(sp.x, sp.y - rr, sp.x + rr, sp.y, 2.5, col);
                        draw_line(sp.x + rr, sp.y, sp.x, sp.y + rr, 2.5, col);
                        draw_line(sp.x, sp.y + rr, sp.x - rr, sp.y, 2.5, col);
                        draw_line(sp.x - rr, sp.y, sp.x, sp.y - rr, 2.5, col);
                        text_c("LOCK", sp.x, sp.y - rr - 6.0 * s, 16.0 * s, col);
                    } else if p.lock_t > 0.0 {
                        let frac = (p.lock_t / g.lock_time_of(p.weapon)).min(1.0);
                        let rr = r * (3.0 - 1.5 * frac);
                        draw_circle_lines(sp.x, sp.y, rr, 1.5, hud_dim());
                    }
                    // gun pipper (lead) for air targets in gun range
                    if let TargetRef::Enemy(id) = tr {
                        if dist < 1500.0 {
                            if let Some(e) = g.enemies.iter().find(|e| e.id == id) {
                                let tof = dist / (GUN_SPEED + p.speed * 0.2);
                                let lead = e.pos + e.fwd() * e.speed * tof - p.vel() * tof * 0.0;
                                if let Some(lp) = project(&v.vp, lead, v.sw, v.sh) {
                                    draw_circle_lines(lp.x, lp.y, 12.0 * s, 1.5, green);
                                    draw_circle(lp.x, lp.y, 2.0 * s, green);
                                }
                            }
                        }
                    }
                }
                _ => {
                    // arrow at the screen edge toward the target
                    let to = tp - v.eye;
                    let x = to.dot(v.right);
                    let y = to.dot(v.up);
                    let dir = vec2(x, -y).normalize_or(vec2(0.0, -1.0));
                    let c = vec2(v.sw * 0.5, v.sh * 0.47);
                    let rad = v.sw.min(v.sh) * 0.33;
                    let tip = c + dir * rad;
                    let side = vec2(-dir.y, dir.x) * 9.0 * s;
                    draw_triangle(tip, tip - dir * 18.0 * s + side, tip - dir * 18.0 * s - side, green);
                    text_c(&format!("{:.1}", dist / 1852.0), tip.x - dir.x * 32.0 * s, tip.y - dir.y * 32.0 * s + 5.0, 15.0 * s, green);
                }
            }
        }
    }
}

fn corner_box(c: Vec2, r: f32, col: Color) {
    let l = r * 0.45;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let p = vec2(c.x + sx * r, c.y + sy * r);
        draw_line(p.x, p.y, p.x - sx * l, p.y, 1.5, col);
        draw_line(p.x, p.y, p.x, p.y - sy * l, 1.5, col);
    }
}

pub fn draw_radar(g: &Game, c: Vec2, r: f32, s: f32, flags: &mut HudFlags) {
    let green = hud_green();
    draw_circle(c.x, c.y, r, Color::new(0.02, 0.1, 0.04, 0.85));
    draw_circle_lines(c.x, c.y, r, 2.0, green);
    draw_circle_lines(c.x, c.y, r * 0.5, 1.0, hud_dim());
    draw_line(c.x - r, c.y, c.x + r, c.y, 1.0, hud_dim());
    draw_line(c.x, c.y - r, c.x, c.y + r, 1.0, hud_dim());
    let range = 10000.0;
    let p = &g.player;
    let hf = vec3(p.fwd().x, 0.0, p.fwd().z).normalize_or(Vec3::Z);
    let hr = hf.cross(Vec3::Y);
    let to_screen = |w: Vec3| -> Option<Vec2> {
        let rel = w - p.pos;
        let v = vec2(rel.dot(hr), -rel.dot(hf)) / range * r;
        if v.length() > r {
            None
        } else {
            Some(c + v)
        }
    };
    for gt in g.ground.iter().filter(|g| g.alive) {
        if let Some(q) = to_screen(gt.pos) {
            let col = if gt.kind == GroundKind::Sam { hud_red() } else { hud_amber() };
            draw_rectangle(q.x - 2.0 * s, q.y - 2.0 * s, 4.0 * s, 4.0 * s, col);
        }
    }
    for e in &g.enemies {
        if let Some(q) = to_screen(e.pos) {
            let col = if e.ace { hud_amber() } else { hud_red() };
            draw_circle(q.x, q.y, 3.2 * s, col);
            let f = e.fwd();
            let d = vec2(f.dot(hr), -f.dot(hf)) * 7.0 * s;
            draw_line(q.x, q.y, q.x + d.x, q.y + d.y, 1.5, col);
        }
    }
    for m in &g.missiles {
        if let Some(q) = to_screen(m.pos) {
            draw_circle(q.x, q.y, 1.6 * s, if m.player { WHITE } else { hud_red() });
        }
    }
    for r in g.remotes.iter().filter(|r| r.alive) {
        if let Some(q) = to_screen(r.pos) {
            draw_jet_icon(q.x, q.y, 4.0 * s, remote_color(g));
        }
    }
    for m in &g.ext_missiles {
        if let Some(q) = to_screen(m.pos) {
            draw_circle(q.x, q.y, 1.6 * s, hud_red());
        }
    }
    draw_jet_icon(c.x, c.y, 5.0 * s, green);
    text_c("10 KM", c.x, c.y + r - 4.0 * s, 13.0 * s, hud_dim());
    flags.radar = true;
}

fn draw_weapons(g: &Game, rc: Rect, s: f32, flags: &mut HudFlags) {
    let green = hud_green();
    let p = &g.player;
    draw_rectangle(rc.x, rc.y, rc.w, rc.h, Color::new(0.02, 0.07, 0.03, 0.85));
    draw_rectangle_lines(rc.x, rc.y, rc.w, rc.h, 2.0, hud_dim());
    let fs = 16.0 * s;
    let row = rc.h / 4.0;
    // gun
    let gy = rc.y + row * 0.7;
    text("GUN", rc.x + 8.0 * s, gy, fs, if p.overheated { hud_amber() } else { green });
    bar(Rect::new(rc.x + rc.w * 0.42, gy - fs * 0.6, rc.w * 0.52, fs * 0.6), p.gun_heat / 100.0, if p.overheated { hud_red() } else { hud_amber() });
    for (i, w) in WEAPONS.iter().enumerate() {
        let y = rc.y + row * (i as f32 + 1.7);
        let sel = i == p.weapon;
        let col = if sel { green } else { hud_dim() };
        if sel {
            draw_rectangle_lines(rc.x + 3.0 * s, y - fs * 0.95, rc.w - 6.0 * s, fs * 1.25, 1.5, green);
        }
        text(&format!("{} {}", i + 1, w.name), rc.x + 8.0 * s, y, fs * 0.9, col);
        let cd = p.cooldowns[i];
        let frac = 1.0 - cd / g.cooldown_of(i);
        if cd <= 0.0 {
            text_r("RDY", rc.x + rc.w - 8.0 * s, y, fs * 0.9, col);
        } else {
            bar(Rect::new(rc.x + rc.w * 0.70, y - fs * 0.6, rc.w * 0.26, fs * 0.6), frac, hud_amber());
        }
    }
    flags.weapon = true;
}

fn draw_weapon_compact(g: &Game, rc: Rect, s: f32, flags: &mut HudFlags) {
    let p = &g.player;
    let w = &WEAPONS[p.weapon];
    let green = hud_green();
    let fs = 16.0 * s;
    let cd = p.cooldowns[p.weapon];
    let status = if cd <= 0.0 { if p.locked { "LOCK".to_string() } else { "RDY".to_string() } } else { format!("{:.1}s", cd) };
    text_c(&format!("{}  {}", w.name, status), rc.x + rc.w * 0.5, rc.y + fs, fs, if cd <= 0.0 { green } else { hud_amber() });
    bar(Rect::new(rc.x, rc.y + fs * 1.4, rc.w, fs * 0.45), 1.0 - cd / g.cooldown_of(p.weapon), green);
    text("GUN", rc.x, rc.y + fs * 2.9, fs * 0.8, green);
    bar(Rect::new(rc.x + 40.0 * s, rc.y + fs * 2.4, rc.w - 40.0 * s, fs * 0.45), p.gun_heat / 100.0, if p.overheated { hud_red() } else { hud_amber() });
    flags.weapon = true;
}

fn draw_status(g: &Game, rc: Rect, s: f32, flags: &mut HudFlags) {
    let green = hud_green();
    let p = &g.player;
    draw_rectangle(rc.x, rc.y, rc.w, rc.h, Color::new(0.02, 0.07, 0.03, 0.85));
    draw_rectangle_lines(rc.x, rc.y, rc.w, rc.h, 2.0, hud_dim());
    let fs = 16.0 * s;
    let row = rc.h / 4.0;
    let y = rc.y + row * 0.7;
    text("HULL", rc.x + 8.0 * s, y, fs, green);
    let hp = (p.hp / PLAYER_HP).clamp(0.0, 1.0);
    let col = if hp > 0.5 { green } else if hp > 0.25 { hud_amber() } else { hud_red() };
    bar(Rect::new(rc.x + rc.w * 0.35, y - fs * 0.6, rc.w * 0.6, fs * 0.6), hp, col);
    flags.health = true;
    let y2 = y + row;
    text("THROTTLE", rc.x + 8.0 * s, y2, fs * 0.9, green);
    bar(Rect::new(rc.x + rc.w * 0.45, y2 - fs * 0.6, rc.w * 0.5, fs * 0.6), p.throttle, if p.throttle > 0.9 { hud_amber() } else { green });
    let y3 = y2 + row;
    let fl = if p.flare_cd <= 0.0 { "FLARES RDY".to_string() } else { format!("FLARES {:.1}s", p.flare_cd) };
    text(&fl, rc.x + 8.0 * s, y3, fs * 0.9, if p.flare_cd <= 0.0 { green } else { hud_amber() });
    let y4 = y3 + row;
    let tgt = match p.target {
        Some(TargetRef::Enemy(_)) => "TGT AIR",
        Some(TargetRef::Ground(_)) => "TGT GND",
        Some(TargetRef::Remote(_)) => "TGT PILOT",
        None => "NO TGT",
    };
    text(tgt, rc.x + 8.0 * s, y4, fs * 0.9, green);
    if p.locked {
        text_r("LOCKED", rc.x + rc.w - 8.0 * s, y4, fs * 0.9, hud_amber());
    }
}

fn draw_health_bar(g: &Game, rc: Rect, s: f32, flags: &mut HudFlags) {
    let hp = (g.player.hp / PLAYER_HP).clamp(0.0, 1.0);
    let col = if hp > 0.5 { hud_green() } else if hp > 0.25 { hud_amber() } else { hud_red() };
    text("HULL", rc.x, rc.y - 2.0 * s, 14.0 * s, hud_dim());
    bar(Rect::new(rc.x + 40.0 * s, rc.y - rc.h, rc.w - 40.0 * s, rc.h), hp, col);
    flags.health = true;
}

fn bar(r: Rect, frac: f32, col: Color) {
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, hud_dim());
    draw_rectangle(r.x + 1.0, r.y + 1.0, (r.w - 2.0) * frac.clamp(0.0, 1.0), r.h - 2.0, col);
}

pub fn draw_touch_controls(g: &Game, inp: &InputState, sw: f32, sh: f32, s: f32, flags: &mut HudFlags) {
    let lay = touch_layout(sw, sh);
    let p = &g.player;
    for b in &lay.buttons {
        let held = inp.held.contains(&b.kind);
        let (fill, ring) = match b.kind {
            Btn::Gun => (Color::new(0.9, 0.3, 0.2, if held { 0.55 } else { 0.28 }), Color::new(1.0, 0.5, 0.4, 0.9)),
            Btn::Missile => {
                let ready = p.cooldowns[p.weapon] <= 0.0;
                (Color::new(0.95, 0.75, 0.2, if held { 0.55 } else if ready { 0.3 } else { 0.12 }), Color::new(1.0, 0.85, 0.4, if ready { 0.9 } else { 0.4 }))
            }
            _ => (Color::new(0.3, 0.9, 0.4, if held { 0.45 } else { 0.18 }), Color::new(0.4, 1.0, 0.5, 0.75)),
        };
        draw_circle(b.pos.x, b.pos.y, b.r, fill);
        draw_circle_lines(b.pos.x, b.pos.y, b.r, 2.5, ring);
        let fs = (b.r * 0.55).max(12.0);
        text_c(b.label, b.pos.x, b.pos.y + fs * 0.35, fs, WHITE);
        // cooldown sweep on the missile button
        if b.kind == Btn::Missile {
            let w = &WEAPONS[p.weapon];
            let cd = p.cooldowns[p.weapon];
            if cd > 0.0 {
                let frac = 1.0 - cd / g.cooldown_of(p.weapon);
                let n = 24;
                for i in 0..((n as f32 * frac) as i32) {
                    let a0 = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
                    let a1 = a0 + std::f32::consts::TAU / n as f32;
                    let rr = b.r + 5.0;
                    draw_line(b.pos.x + a0.cos() * rr, b.pos.y + a0.sin() * rr, b.pos.x + a1.cos() * rr, b.pos.y + a1.sin() * rr, 4.0, hud_amber());
                }
            }
            text_c(w.short, b.pos.x, b.pos.y + b.r + 16.0 * s, 15.0 * s, Color::new(1.0, 0.9, 0.5, 0.9));
        }
        if b.kind == Btn::Flare && p.flare_cd > 0.0 {
            text_c(&format!("{:.0}", p.flare_cd.ceil()), b.pos.x, b.pos.y + b.r + 14.0 * s, 13.0 * s, hud_amber());
        }
    }
    // throttle slider
    let tr = lay.throttle_rect;
    draw_rectangle(tr.x, tr.y, tr.w, tr.h, Color::new(0.0, 0.0, 0.0, 0.25));
    draw_rectangle_lines(tr.x, tr.y, tr.w, tr.h, 2.0, Color::new(0.4, 1.0, 0.5, 0.7));
    let ab_y = tr.y + tr.h * 0.1;
    draw_line(tr.x, ab_y, tr.x + tr.w, ab_y, 1.5, hud_amber());
    let ky = tr.y + tr.h * (1.0 - p.throttle);
    draw_rectangle(tr.x - 4.0, ky - 7.0 * s, tr.w + 8.0, 14.0 * s, if p.throttle > 0.9 { hud_amber() } else { hud_green() });
    text_c("THR", tr.x + tr.w * 0.5, tr.y + tr.h + 18.0 * s, 15.0 * s, hud_green());
    text_c("AB", tr.x + tr.w * 0.5, tr.y - 6.0 * s, 13.0 * s, hud_amber());
    // virtual stick (only shown when there's no tilt sensor)
    if !inp.tilt_live {
        let (c, r) = if inp.stick_finger.is_some() { (inp.stick_origin, lay.stick_r) } else { (lay.stick_center, lay.stick_r) };
        draw_circle_lines(c.x, c.y, r, 2.0, Color::new(0.4, 1.0, 0.5, 0.5));
        let k = c + inp.stick_vec * r;
        draw_circle(k.x, k.y, r * 0.35, Color::new(0.4, 1.0, 0.5, 0.35));
        if inp.stick_finger.is_none() {
            text_c("DRAG TO STEER", c.x, c.y + r + 18.0 * s, 14.0 * s, hud_dim());
        }
    } else {
        // tilt indicator
        let c = vec2(lay.buttons[6].pos.x + lay.buttons[6].r + 34.0 * s, lay.buttons[6].pos.y);
        let r = 22.0 * s;
        draw_circle_lines(c.x, c.y, r, 1.5, hud_dim());
        let k = c + vec2(inp.tilt_stick.0, inp.tilt_stick.1) * r;
        draw_circle(k.x, k.y, 4.0 * s, hud_green());
    }
    flags.touch_buttons = true;
}

pub fn remote_color(g: &Game) -> Color {
    if g.mode == MatchMode::Dogfight {
        Color::new(1.0, 0.4, 0.9, 1.0)
    } else {
        Color::new(0.4, 0.8, 1.0, 1.0)
    }
}

/// Online scoreboard: everyone in the match, best score first. Right-aligned at `rx`.
pub fn draw_scoreboard(g: &Game, rx: f32, y: f32, s: f32) {
    let mut rows: Vec<(String, i32, i32, i32, bool)> = vec![("YOU".into(), g.score, g.kills + g.player_kills, g.deaths, true)];
    for r in &g.remotes {
        rows.push((r.name.clone(), r.score, r.kills, r.deaths, false));
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let fs = 15.0 * s;
    let w = 230.0 * s;
    draw_rectangle(rx - w, y, w, fs * 1.25 * (rows.len() as f32 + 1.0) + 6.0 * s, Color::new(0.0, 0.05, 0.02, 0.55));
    text_r("PILOT          SCORE   K   D", rx - 6.0 * s, y + fs * 1.05, fs * 0.9, hud_dim());
    for (i, (n, sc, k, d, me)) in rows.iter().enumerate() {
        let c = if *me { hud_green() } else { remote_color(g) };
        let yy = y + fs * 1.25 * (i as f32 + 2.0);
        text(&n.chars().take(12).collect::<String>(), rx - w + 6.0 * s, yy, fs, c);
        text_r(&format!("{:6}  {:2}  {:2}", sc, k, d), rx - 6.0 * s, yy, fs, c);
    }
}
