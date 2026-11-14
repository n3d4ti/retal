//! Multiplayer menus: the match list, creating a match (public or private with a password,
//! co-op or dogfight, difficulty) and joining by code. Works with mouse, keyboard and touch.

use crate::game::{MatchMode, DIFFICULTIES};
use crate::hud::*;
use crate::mp::{Mp, MpEvent};
use crate::net::NetStatus;
use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Main,
    Create,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Field {
    Name,
    Server,
    MatchName,
    Password,
    JoinCode,
    JoinPassword(String),
}

pub struct TextEdit {
    pub field: Field,
    pub label: String,
    pub buf: String,
    pub secret: bool,
}

pub struct Lobby {
    pub view: View,
    pub match_name: String,
    pub public: bool,
    pub password: String,
    pub mode: MatchMode,
    pub edit: Option<TextEdit>,
    pub error: Option<(String, f32)>,
    pub note: String,
    pub embedded_server: bool,
    pub tried_embedded: bool,
    pub scroll: usize,
}

pub enum Action {
    None,
    Back,
}

impl Lobby {
    pub fn new() -> Self {
        Lobby {
            view: View::Main,
            match_name: String::new(),
            public: true,
            password: String::new(),
            mode: MatchMode::Coop,
            edit: None,
            error: None,
            note: String::new(),
            embedded_server: false,
            tried_embedded: false,
            scroll: 0,
        }
    }

    pub fn show_error(&mut self, e: &str) {
        self.error = Some((e.to_string(), 5.0));
    }
}

fn bscale(sw: f32, sh: f32) -> f32 {
    ui_scale(sw, sh).max(0.85)
}

/// Draws a button and reports whether it was tapped/clicked this frame.
pub fn button(r: Rect, label: &str, on: bool, accent: Color, tap: Option<Vec2>, s: f32) -> bool {
    let fill = if on { Color::new(accent.r * 0.35, accent.g * 0.35, accent.b * 0.35, 0.95) } else { Color::new(0.02, 0.06, 0.04, 0.85) };
    draw_rectangle(r.x, r.y, r.w, r.h, fill);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, if on { 3.0 } else { 1.5 }, if on { accent } else { Color::new(accent.r, accent.g, accent.b, 0.6) });
    let fs = (r.h * 0.5).min(22.0 * s);
    let d = measure_text(label, None, fs as u16, 1.0);
    let fs = if d.width > r.w - 10.0 { fs * (r.w - 10.0) / d.width } else { fs };
    text_c(label, r.x + r.w * 0.5, r.y + r.h * 0.5 + fs * 0.35, fs, WHITE);
    tap.map(|p| r.contains(p)).unwrap_or(false)
}

fn row_of(n: usize, y: f32, h: f32, sw: f32, s: f32) -> Vec<Rect> {
    let gap = 10.0 * s;
    let total = (sw - 40.0 * s).min(900.0 * s);
    let w = (total - gap * (n as f32 - 1.0)) / n as f32;
    let x0 = (sw - total) * 0.5;
    (0..n).map(|i| Rect::new(x0 + i as f32 * (w + gap), y, w, h)).collect()
}

/// Starts editing a text field: the browser shows its own prompt (phone keyboard), the desktop an in-game box.
fn begin_edit(lb: &mut Lobby, mp: &mut Mp, field: Field, label: &str, current: &str, secret: bool) {
    if let Some(res) = crate::net::prompt_text(label, current) {
        if let Some(text) = res {
            commit(lb, mp, field, text);
        }
        return;
    }
    while get_char_pressed().is_some() {}
    lb.edit = Some(TextEdit { field, label: label.to_string(), buf: current.to_string(), secret });
}

fn commit(lb: &mut Lobby, mp: &mut Mp, field: Field, text: String) {
    let text = text.trim().to_string();
    match field {
        Field::Name => {
            if !text.is_empty() {
                mp.set_name(&text);
            }
        }
        Field::Server => {
            if !text.is_empty() {
                mp.url = text;
                mp.connect();
            }
        }
        Field::MatchName => lb.match_name = text.chars().take(20).collect(),
        Field::Password => lb.password = text.chars().take(32).collect(),
        Field::JoinCode => {
            let code = text.to_uppercase();
            if code.is_empty() {
                return;
            }
            let known = mp.rooms.iter().find(|r| r.code == code).cloned();
            match known {
                Some(r) if !r.locked => mp.join(&code, ""),
                _ => begin_edit(lb, mp, Field::JoinPassword(code.clone()), &format!("PASSWORD FOR MATCH {}", code), "", true),
            }
        }
        Field::JoinPassword(code) => mp.join(&code, &text),
    }
}

/// Desktop text box (keyboard). Returns true while it is open (it takes all input).
fn text_box(lb: &mut Lobby, mp: &mut Mp, sw: f32, sh: f32, tap: Option<Vec2>) -> bool {
    let s = bscale(sw, sh);
    let mut done: Option<bool> = None;
    if let Some(ed) = lb.edit.as_mut() {
        while let Some(ch) = get_char_pressed() {
            if !ch.is_control() && ed.buf.chars().count() < 32 {
                ed.buf.push(ch);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            ed.buf.pop();
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            done = Some(true);
        }
        if is_key_pressed(KeyCode::Escape) {
            done = Some(false);
        }
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
        let w = (sw - 40.0 * s).min(620.0 * s);
        let r = Rect::new((sw - w) * 0.5, sh * 0.3, w, 150.0 * s);
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.02, 0.08, 0.05, 0.97));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, hud_green());
        text_c(&ed.label, sw * 0.5, r.y + 30.0 * s, 20.0 * s, hud_green());
        let shown = if ed.secret { "*".repeat(ed.buf.chars().count()) } else { ed.buf.clone() };
        let caret = if (get_time() * 2.0).fract() < 0.5 { "_" } else { " " };
        draw_rectangle_lines(r.x + 16.0 * s, r.y + 46.0 * s, r.w - 32.0 * s, 40.0 * s, 1.5, hud_dim());
        text_c(&format!("{}{}", shown, caret), sw * 0.5, r.y + 74.0 * s, 26.0 * s, WHITE);
        let b = row_of(2, r.y + 100.0 * s, 38.0 * s, r.w.min(sw), s);
        let off = r.x - b[0].x + 16.0 * s;
        let b0 = Rect::new(b[0].x + off, b[0].y, (r.w - 42.0 * s) * 0.5, b[0].h);
        let b1 = Rect::new(b0.x + b0.w + 10.0 * s, b[0].y, b0.w, b[0].h);
        if button(b0, "CANCEL (ESC)", false, hud_red(), tap, s) {
            done = Some(false);
        }
        if button(b1, "OK (ENTER)", true, hud_green(), tap, s) {
            done = Some(true);
        }
    }
    match done {
        Some(ok) => {
            let ed = lb.edit.take().unwrap();
            if ok {
                commit(lb, mp, ed.field, ed.buf);
            }
            true
        }
        None => lb.edit.is_some(),
    }
}

/// One frame of the multiplayer menus. `tap` is where the player tapped/clicked this frame.
pub fn frame(lb: &mut Lobby, mp: &mut Mp, diff_sel: &mut usize, tap: Option<Vec2>, sw: f32, sh: f32, dt: f32) -> Action {
    let s = bscale(sw, sh);
    let us = ui_scale(sw, sh);
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.03, 0.08, 0.72));
    if let Some((_, t)) = lb.error.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            lb.error = None;
        }
    }
    let editing = lb.edit.is_some();
    let tap_main = if editing { None } else { tap };
    let green = hud_green();
    let amber = hud_amber();
    let mut action = Action::None;

    let top = 30.0 * us.max(0.7);
    let h = 38.0 * s;
    match lb.view {
        View::Main => {
            text_c("MULTIPLAYER", sw * 0.5, top, 34.0 * us.max(0.7), Color::new(1.0, 0.85, 0.35, 1.0));
            let status = match mp.status() {
                NetStatus::Open if mp.ready() => format!("CONNECTED TO {}", mp.url),
                NetStatus::Open | NetStatus::Connecting => format!("CONNECTING TO {} ...", mp.url),
                NetStatus::Closed => format!("NO SERVER AT {}  -  {}", mp.url, mp.net.error),
                NetStatus::Idle => "OFFLINE".to_string(),
            };
            text_c(&status, sw * 0.5, top + 20.0 * s, 15.0 * s, if mp.ready() { hud_dim() } else { amber });
            if !lb.note.is_empty() {
                text_c(&lb.note, sw * 0.5, top + 36.0 * s, 13.0 * s, Color::new(0.6, 0.85, 1.0, 1.0));
            }
            let r1 = row_of(2, top + 44.0 * s, h, sw, s);
            if button(r1[0], &format!("NAME: {}  (N)", mp.name), false, green, tap_main, s) || (!editing && is_key_pressed(KeyCode::N)) {
                let n = mp.name.clone();
                begin_edit(lb, mp, Field::Name, "YOUR PILOT NAME", &n, false);
            }
            if button(r1[1], "SERVER ADDRESS", false, green, tap_main, s) {
                let u = mp.url.clone();
                begin_edit(lb, mp, Field::Server, "SERVER ADDRESS (ws://host:port/ws)", &u, false);
            }

            // match list
            let list_y = top + 44.0 * s + h + 12.0 * s;
            let bottom_y = sh - h - 14.0 * s;
            let row_h = 36.0 * s;
            let rows_fit = (((bottom_y - 8.0 * s - list_y) / (row_h + 4.0 * s)).floor() as usize).max(1);
            let list_w = (sw - 40.0 * s).min(900.0 * s);
            let lx = (sw - list_w) * 0.5;
            if !mp.ready() {
                text_c(if mp.status() == NetStatus::Closed { "CAN'T REACH THE MATCH SERVER - TAP RECONNECT OR CHANGE THE ADDRESS" } else { "CONNECTING..." }, sw * 0.5, list_y + 30.0 * s, 17.0 * s, amber);
            } else if mp.rooms.is_empty() {
                text_c(if mp.got_list { "NO MATCHES YET - CREATE ONE!" } else { "LOOKING FOR MATCHES..." }, sw * 0.5, list_y + 30.0 * s, 18.0 * s, hud_dim());
            } else {
                if lb.scroll >= mp.rooms.len() {
                    lb.scroll = 0;
                }
                let rooms = mp.rooms.clone();
                for (i, r) in rooms.iter().enumerate().skip(lb.scroll).take(rows_fit) {
                    let k = i - lb.scroll;
                    let rr = Rect::new(lx, list_y + k as f32 * (row_h + 4.0 * s), list_w, row_h);
                    let full = r.players >= r.max;
                    let mode = if r.mode == MatchMode::Dogfight { "DOGFIGHT" } else { "CO-OP" };
                    let lock = if r.locked { "PRIVATE" } else { "PUBLIC" };
                    let label = format!(
                        "{}. {}   [{}]   {}   {}   {}/{}   {}",
                        k + 1,
                        r.name,
                        r.code,
                        mode,
                        DIFFICULTIES[r.diff.min(2)].name,
                        r.players,
                        r.max,
                        lock
                    );
                    let accent = if r.locked { amber } else { green };
                    let key = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9].get(k).copied();
                    let pressed = !editing && key.map(is_key_pressed).unwrap_or(false);
                    if (button(rr, &label, false, accent, tap_main, s) || pressed) && !full {
                        if r.locked {
                            begin_edit(lb, mp, Field::JoinPassword(r.code.clone()), &format!("PASSWORD FOR \"{}\"", r.name), "", true);
                        } else {
                            mp.join(&r.code, "");
                        }
                    }
                }
                if rooms.len() > rows_fit {
                    let more = Rect::new(lx + list_w - 120.0 * s, bottom_y - 4.0 * s - 26.0 * s, 120.0 * s, 26.0 * s);
                    if button(more, "MORE...", false, green, tap_main, s) {
                        lb.scroll += rows_fit;
                    }
                }
            }

            let b = row_of(4, bottom_y, h, sw, s);
            if button(b[0], "BACK (ESC)", false, hud_red(), tap_main, s) || (!editing && is_key_pressed(KeyCode::Escape)) {
                action = Action::Back;
            }
            let reconnect = !mp.ready();
            if button(b[1], if reconnect { "RECONNECT" } else { "REFRESH (R)" }, false, green, tap_main, s) || (!editing && is_key_pressed(KeyCode::R)) {
                if reconnect {
                    mp.connect();
                } else {
                    mp.refresh();
                }
            }
            if button(b[2], "JOIN BY CODE (J)", false, green, tap_main, s) || (!editing && is_key_pressed(KeyCode::J)) {
                if mp.ready() {
                    begin_edit(lb, mp, Field::JoinCode, "MATCH CODE (4 LETTERS)", "", false);
                }
            }
            if button(b[3], "CREATE MATCH (C)", true, amber, tap_main, s) || (!editing && is_key_pressed(KeyCode::C)) {
                if mp.ready() {
                    lb.view = View::Create;
                    if lb.match_name.is_empty() {
                        lb.match_name = format!("{}'S MATCH", mp.name);
                    }
                }
            }
        }
        View::Create => {
            text_c("CREATE MATCH", sw * 0.5, top, 30.0 * us.max(0.7), Color::new(1.0, 0.85, 0.35, 1.0));
            let mut y = top + 12.0 * s;
            let step = h + 8.0 * s;
            let r = row_of(1, y, h, sw, s);
            if button(r[0], &format!("MATCH NAME: {}", lb.match_name), false, green, tap_main, s) {
                let n = lb.match_name.clone();
                begin_edit(lb, mp, Field::MatchName, "MATCH NAME", &n, false);
            }
            y += step;
            let r = row_of(2, y, h, sw, s);
            if button(r[0], "PUBLIC - ANYONE CAN JOIN", lb.public, green, tap_main, s) {
                lb.public = true;
            }
            if button(r[1], "PRIVATE - PASSWORD", !lb.public, amber, tap_main, s) {
                lb.public = false;
                if lb.password.is_empty() {
                    begin_edit(lb, mp, Field::Password, "MATCH PASSWORD", "", true);
                }
            }
            y += step;
            if !lb.public {
                let r = row_of(1, y, h, sw, s);
                let shown = if lb.password.is_empty() { "(TAP TO SET)".to_string() } else { lb.password.clone() };
                if button(r[0], &format!("PASSWORD: {}", shown), false, amber, tap_main, s) {
                    let p = lb.password.clone();
                    begin_edit(lb, mp, Field::Password, "MATCH PASSWORD", &p, false);
                }
                y += step;
            }
            let r = row_of(2, y, h, sw, s);
            if button(r[0], "CO-OP VS ENEMY WAVES", lb.mode == MatchMode::Coop, Color::new(0.4, 0.8, 1.0, 1.0), tap_main, s) {
                lb.mode = MatchMode::Coop;
            }
            if button(r[1], "DOGFIGHT - ALL VS ALL", lb.mode == MatchMode::Dogfight, Color::new(1.0, 0.4, 0.9, 1.0), tap_main, s) {
                lb.mode = MatchMode::Dogfight;
            }
            y += step + 26.0 * us.max(0.85);
            // difficulty of the bots in this match
            if !editing {
                if let Some(p) = tap_main {
                    for (i, rr) in crate::diff_rects(sw, sh, y).iter().enumerate() {
                        if rr.contains(p) {
                            *diff_sel = i;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Left) {
                    *diff_sel = (*diff_sel + 2) % 3;
                }
                if is_key_pressed(KeyCode::Right) {
                    *diff_sel = (*diff_sel + 1) % 3;
                }
            }
            crate::draw_picker(*diff_sel, sw, sh, y);

            let bottom_y = sh - h - 14.0 * s;
            let b = row_of(2, bottom_y, h, sw, s);
            if button(b[0], "BACK (ESC)", false, hud_red(), tap_main, s) || (!editing && is_key_pressed(KeyCode::Escape)) {
                lb.view = View::Main;
            }
            if button(b[1], "START MATCH (ENTER)", true, amber, tap_main, s) || (!editing && is_key_pressed(KeyCode::Enter)) {
                if !lb.public && lb.password.is_empty() {
                    lb.show_error("A PRIVATE MATCH NEEDS A PASSWORD");
                } else {
                    let (n, pw, m) = (lb.match_name.clone(), lb.password.clone(), lb.mode);
                    mp.create(&n, lb.public, &pw, m, *diff_sel);
                }
            }
        }
    }
    if let Some((e, _)) = &lb.error {
        let w = measure_text(e, None, (20.0 * s) as u16, 1.0).width + 30.0 * s;
        draw_rectangle(sw * 0.5 - w * 0.5, sh * 0.5 - 24.0 * s, w, 36.0 * s, Color::new(0.3, 0.02, 0.02, 0.92));
        text_c(e, sw * 0.5, sh * 0.5, 20.0 * s, Color::new(1.0, 0.6, 0.5, 1.0));
    }
    text_box(lb, mp, sw, sh, tap);
    action
}

/// Starts a match server inside this program (desktop only) so friends on the same network can join.
#[cfg(not(target_arch = "wasm32"))]
pub fn start_embedded_server(lb: &mut Lobby, mp: &mut Mp) {
    lb.tried_embedded = true;
    let web = crate::server::find_web_dir();
    match crate::server::start(&format!("0.0.0.0:{}", crate::server::DEFAULT_PORT), web) {
        Ok(addr) => {
            lb.embedded_server = true;
            lb.note = format!("HOSTING A SERVER ON THIS COMPUTER (PORT {}) - FRIENDS USE ws://<YOUR-IP>:{}/ws OR OPEN http://<YOUR-IP>:{}/", addr.port(), addr.port(), addr.port());
            mp.connect();
        }
        Err(_) => {}
    }
}

#[cfg(target_arch = "wasm32")]
pub fn start_embedded_server(lb: &mut Lobby, _mp: &mut Mp) {
    lb.tried_embedded = true;
}

/// Handles lobby events; returns a joined match when one starts.
pub fn handle_events(lb: &mut Lobby, mp: &mut Mp, evs: Vec<MpEvent>) -> Option<crate::mp::Joined> {
    let mut joined = None;
    for e in evs {
        match e {
            MpEvent::Joined(j) => {
                lb.view = View::Main;
                joined = Some(j);
            }
            MpEvent::Error(e) => lb.show_error(&e),
            MpEvent::Closed(e) => {
                // the desktop game starts its own server if none is running on this computer
                let local = mp.url.contains("127.0.0.1") || mp.url.contains("localhost");
                if local && !lb.tried_embedded && !mp.welcomed_before_close {
                    start_embedded_server(lb, mp);
                } else {
                    lb.show_error(&e);
                }
            }
            MpEvent::Connected => {}
        }
    }
    joined
}
