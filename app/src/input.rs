//! Player controls: keyboard + mouse on computers, tilt (gyroscope) + on-screen buttons on phones.

use macroquad::prelude::*;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn retal_tilt_ok() -> i32;
    fn retal_tilt_roll() -> f32;
    fn retal_tilt_pitch() -> f32;
    fn retal_is_touch() -> i32;
}

/// Tells the JS side which plugin version this build expects.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn retal_tilt_crate_version() -> u32 {
    1
}

/// Raw device tilt in radians: (roll, pitch). Roll > 0 = right edge down, pitch > 0 = top edge toward the player.
pub fn read_tilt() -> Option<(f32, f32)> {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        if retal_tilt_ok() != 0 {
            return Some((retal_tilt_roll(), retal_tilt_pitch()));
        }
    }
    None
}

pub fn platform_is_touch() -> bool {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        return retal_is_touch() != 0;
    }
    #[allow(unreachable_code)]
    false
}

/// Tuning for the phone tilt controls.
pub const TILT_FULL_ROLL: f32 = 0.45; // rad of phone tilt for full roll (~26 deg)
pub const TILT_FULL_PITCH: f32 = 0.40; // rad for full pitch (~23 deg)
pub const TILT_DEADZONE: f32 = 0.05; // rad (~3 deg)

pub fn tilt_to_stick(raw: (f32, f32), center: (f32, f32)) -> (f32, f32) {
    let f = |v: f32, full: f32| {
        let s = v.signum();
        let a = (v.abs() - TILT_DEADZONE).max(0.0) / (full - TILT_DEADZONE);
        (s * a).clamp(-1.0, 1.0)
    };
    let roll = f(raw.0 - center.0, TILT_FULL_ROLL);
    let pitch = f(raw.1 - center.1, TILT_FULL_PITCH);
    (roll, pitch)
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Controls {
    pub pitch: f32, // + nose up
    pub roll: f32,  // + right wing down
    pub yaw: f32,   // + nose right
    pub throttle_delta: f32,
    pub throttle_set: Option<f32>,
    pub gun: bool,
    pub missile: bool,
    pub next_weapon: bool,
    pub select_weapon: Option<usize>,
    pub next_target: bool,
    pub flare: bool,
    pub view: bool,
    pub pause: bool,
    pub recenter: bool,
    pub confirm: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Btn {
    Gun,
    Missile,
    Weapon,
    Flare,
    View,
    Pause,
    Tilt,
}

pub struct TouchButton {
    pub kind: Btn,
    pub pos: Vec2,
    pub r: f32,
    pub label: &'static str,
}

/// Screen layout of the on-screen controls (depends on screen size).
pub struct TouchLayout {
    pub buttons: Vec<TouchButton>,
    pub throttle_rect: Rect,
    pub stick_center: Vec2,
    pub stick_r: f32,
}

pub fn touch_layout(sw: f32, sh: f32) -> TouchLayout {
    let u = sw.min(sh);
    let big = u * 0.115;
    let small = u * 0.065;
    let gun = vec2(sw - big * 1.35, sh - big * 1.35);
    let msl = vec2(gun.x - big * 2.2, gun.y + big * 0.25);
    let buttons = vec![
        TouchButton { kind: Btn::Gun, pos: gun, r: big, label: "GUN" },
        TouchButton { kind: Btn::Missile, pos: msl, r: big * 0.85, label: "MSL" },
        TouchButton { kind: Btn::Weapon, pos: vec2(gun.x + big * 0.2, gun.y - big * 2.0), r: small, label: "WPN" },
        TouchButton { kind: Btn::Flare, pos: vec2(msl.x - big * 0.2, msl.y - big * 1.95), r: small, label: "FLR" },
        TouchButton { kind: Btn::View, pos: vec2(sw - small * 1.4, small * 2.9), r: small * 0.8, label: "VIEW" },
        TouchButton { kind: Btn::Pause, pos: vec2(sw - small * 1.4, small * 1.1), r: small * 0.8, label: "II" },
        TouchButton { kind: Btn::Tilt, pos: vec2(small * 1.4, u * 0.25), r: small * 0.8, label: "LEVEL" },
    ];
    let throttle_rect = Rect::new(u * 0.03, sh * 0.38, u * 0.10, sh * 0.52);
    TouchLayout {
        buttons,
        throttle_rect,
        stick_center: vec2(u * 0.13 + u * 0.22, sh - u * 0.25),
        stick_r: u * 0.16,
    }
}

/// Keeps touch state between frames (which finger holds what, tilt calibration, virtual stick).
pub struct InputState {
    pub touch_mode: bool,
    pub tilt_center: (f32, f32),
    pub tilt_calibrated: bool,
    pub tilt_live: bool,
    pub stick_finger: Option<u64>,
    pub stick_origin: Vec2,
    pub stick_vec: Vec2,
    pub held: Vec<Btn>,
    prev_held: Vec<Btn>,
    pub invert_pitch: bool,
    pub tilt_stick: (f32, f32),
    pub last_touch_tap: bool,
    pub last_tap_pos: Vec2,
}

impl InputState {
    pub fn new() -> Self {
        InputState {
            touch_mode: platform_is_touch(),
            tilt_center: (0.0, 0.0),
            tilt_calibrated: false,
            tilt_live: false,
            stick_finger: None,
            stick_origin: Vec2::ZERO,
            stick_vec: Vec2::ZERO,
            held: Vec::new(),
            prev_held: Vec::new(),
            invert_pitch: false,
            tilt_stick: (0.0, 0.0),
            last_touch_tap: false,
            last_tap_pos: Vec2::ZERO,
        }
    }

    pub fn recenter_tilt(&mut self) {
        if let Some(t) = read_tilt() {
            self.tilt_center = t;
            self.tilt_calibrated = true;
        }
    }

    fn pressed(&self, b: Btn) -> bool {
        self.held.contains(&b) && !self.prev_held.contains(&b)
    }

    /// Reads all devices and returns this frame's controls.
    pub fn poll(&mut self, sw: f32, sh: f32, in_flight: bool) -> Controls {
        let mut c = Controls::default();

        // ---------- keyboard ----------
        let k = |code| is_key_down(code);
        if k(KeyCode::Down) || k(KeyCode::S) {
            c.pitch += 1.0;
        }
        if k(KeyCode::Up) || k(KeyCode::W) {
            c.pitch -= 1.0;
        }
        if k(KeyCode::Right) || k(KeyCode::D) {
            c.roll += 1.0;
        }
        if k(KeyCode::Left) || k(KeyCode::A) {
            c.roll -= 1.0;
        }
        if k(KeyCode::E) {
            c.yaw += 1.0;
        }
        if k(KeyCode::Q) {
            c.yaw -= 1.0;
        }
        if k(KeyCode::LeftShift) || k(KeyCode::RightShift) || k(KeyCode::Equal) || k(KeyCode::KpAdd) || k(KeyCode::PageUp) {
            c.throttle_delta += 1.0;
        }
        if k(KeyCode::LeftControl) || k(KeyCode::RightControl) || k(KeyCode::Minus) || k(KeyCode::KpSubtract) || k(KeyCode::PageDown) {
            c.throttle_delta -= 1.0;
        }
        c.gun = k(KeyCode::Space) || is_mouse_button_down(MouseButton::Left) && !self.touch_mode;
        c.missile = is_key_pressed(KeyCode::Enter)
            || is_key_pressed(KeyCode::M)
            || (is_mouse_button_pressed(MouseButton::Right) && !self.touch_mode);
        c.next_weapon = is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::R);
        for (i, kc) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3].iter().enumerate() {
            if is_key_pressed(*kc) {
                c.select_weapon = Some(i);
            }
        }
        c.next_target = is_key_pressed(KeyCode::T);
        c.flare = is_key_pressed(KeyCode::F) || is_key_pressed(KeyCode::X);
        c.view = is_key_pressed(KeyCode::V) || is_key_pressed(KeyCode::C);
        c.pause = is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape);
        c.confirm = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space);
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            self.last_tap_pos = vec2(mx, my);
        }
        if is_key_pressed(KeyCode::I) {
            self.invert_pitch = !self.invert_pitch;
        }

        // ---------- touch ----------
        let ts = touches();
        if !ts.is_empty() {
            self.touch_mode = true;
        }
        self.prev_held = std::mem::take(&mut self.held);
        let layout = touch_layout(sw, sh);
        let mut stick_seen = false;
        self.last_touch_tap = false;
        for t in &ts {
            let ended = matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled);
            if t.phase == TouchPhase::Started {
                self.last_touch_tap = true;
                self.last_tap_pos = t.position;
            }
            if ended {
                if self.stick_finger == Some(t.id) {
                    self.stick_finger = None;
                }
                continue;
            }
            let p = t.position;
            if self.stick_finger == Some(t.id) {
                stick_seen = true;
                self.stick_vec = ((p - self.stick_origin) / layout.stick_r).clamp_length_max(1.0);
                continue;
            }
            let mut on_button = false;
            for b in &layout.buttons {
                if p.distance(b.pos) < b.r * 1.25 {
                    self.held.push(b.kind);
                    on_button = true;
                }
            }
            if on_button {
                continue;
            }
            let tr = layout.throttle_rect;
            let wide = Rect::new(tr.x - tr.w * 0.5, tr.y - 20.0, tr.w * 2.2, tr.h + 40.0);
            if wide.contains(p) {
                let v = 1.0 - (p.y - tr.y) / tr.h;
                c.throttle_set = Some(v.clamp(0.0, 1.0));
                continue;
            }
            // Virtual stick: only used when tilt is unavailable, anywhere on the left 60 % of the screen.
            if !self.tilt_live && in_flight && p.x < sw * 0.6 && t.phase == TouchPhase::Started && self.stick_finger.is_none() {
                self.stick_finger = Some(t.id);
                self.stick_origin = p;
                self.stick_vec = Vec2::ZERO;
                stick_seen = true;
            }
        }
        if !stick_seen && self.stick_finger.is_none() {
            self.stick_vec = Vec2::ZERO;
        }
        if self.held.contains(&Btn::Gun) {
            c.gun = true;
        }
        if self.pressed(Btn::Missile) {
            c.missile = true;
        }
        if self.pressed(Btn::Weapon) {
            c.next_weapon = true;
        }
        if self.pressed(Btn::Flare) {
            c.flare = true;
        }
        if self.pressed(Btn::View) {
            c.view = true;
        }
        if self.pressed(Btn::Pause) {
            c.pause = true;
        }
        if self.pressed(Btn::Tilt) {
            c.recenter = true;
        }
        if self.stick_finger.is_some() {
            c.roll += self.stick_vec.x;
            c.pitch += self.stick_vec.y; // drag down = pull up (like a real stick)
        }

        // ---------- tilt (gyroscope) ----------
        if let Some(raw) = read_tilt() {
            self.tilt_live = true;
            if !self.tilt_calibrated || c.recenter {
                self.tilt_center = raw;
                self.tilt_calibrated = true;
            }
            let (r, p) = tilt_to_stick(raw, self.tilt_center);
            self.tilt_stick = (r, p);
            if in_flight {
                c.roll += r;
                c.pitch += p;
            }
        }

        if self.invert_pitch {
            c.pitch = -c.pitch;
        }
        c.pitch = c.pitch.clamp(-1.0, 1.0);
        c.roll = c.roll.clamp(-1.0, 1.0);
        c.yaw = c.yaw.clamp(-1.0, 1.0);
        c.throttle_delta = c.throttle_delta.clamp(-1.0, 1.0);
        c
    }
}
