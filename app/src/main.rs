//! RETAL — an endless arcade jet-combat game in the spirit of late-80s cockpit flight sims.
//! Infinite weapons on cooldown timers, endless respawning enemy waves.
//! Runs natively and in the browser (WebAssembly); on phones it flies by tilting the device.

mod game;
mod hud;
mod input;
mod lobby;
mod mp;
mod net;
mod render;
#[cfg(not(target_arch = "wasm32"))]
mod server;
mod world;
mod wsproto;

use game::*;
use hud::*;
use input::*;
use macroquad::prelude::*;
use render::*;

const TITLE_PNG: &[u8] = include_bytes!("../assets/generated/title_small.png");

#[derive(PartialEq, Clone, Copy)]
enum State {
    Title,
    Playing,
    Paused,
    GameOver,
    Lobby,
}

#[derive(Default)]
struct Args {
    screenshot: Option<String>,
    self_test: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < v.len() {
        match v[i].as_str() {
            "--screenshot" if i + 1 < v.len() => {
                a.screenshot = Some(v[i + 1].clone());
                i += 1;
            }
            "--self-test" if i + 1 < v.len() => {
                a.self_test = Some(v[i + 1].clone());
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    a
}

/// Game steps per drawn frame during --self-test.
const SELF_TEST_SUBSTEPS: u32 = 20;

fn window_conf() -> Conf {
    let testing = std::env::args().any(|a| a == "--self-test" || a == "--screenshot");
    Conf {
        platform: miniquad::conf::Platform {
            // checks must not wait for the screen refresh (a covered window may stop refreshing)
            swap_interval: if testing { Some(0) } else { None },
            ..Default::default()
        },
        window_title: "Retal".to_string(),
        window_width: if std::env::args().any(|a| a == "--touch-ui") { 844 } else { 1280 },
        window_height: if std::env::args().any(|a| a == "--touch-ui") { 390 } else { 720 },
        high_dpi: false,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    }
}

/// Camera for the current view (cockpit, chase, or a slow orbit when the player is down / on the title).
fn make_view(g: &Game, chase_eye: &mut Vec3, title_t: Option<f32>) -> (Camera3D, View) {
    let sw = screen_width();
    let sh = screen_height();
    let p = &g.player;
    let (eye, target, up, fov) = if let Some(t) = title_t {
        let a = t * 0.12;
        let c = p.pos;
        let eye = c + vec3(a.cos() * 40.0, 8.0 + (t * 0.3).sin() * 4.0, a.sin() * 40.0);
        (eye, c, Vec3::Y, 50.0f32)
    } else if !p.alive {
        let a = g.time * 0.3;
        let eye = p.pos + vec3(a.cos() * 160.0, 60.0, a.sin() * 160.0);
        (eye, p.pos, Vec3::Y, 60.0)
    } else if g.cockpit {
        let eye = p.pos + p.up() * 1.2 + p.fwd() * 4.5;
        (eye, eye + p.fwd() * 100.0, p.up(), 55.0)
    } else {
        let want = p.pos - p.fwd() * 38.0 + p.up() * 9.0;
        let k = 0.25;
        *chase_eye = chase_eye.lerp(want, k);
        if chase_eye.distance(want) > 80.0 {
            *chase_eye = want;
        }
        (*chase_eye, p.pos + p.fwd() * 30.0, p.up(), 60.0)
    };
    let cam = Camera3D {
        position: eye,
        target,
        up,
        fovy: fov.to_radians(),
        z_near: 1.0,
        z_far: 22000.0,
        ..Default::default()
    };
    let vp = cam.matrix();
    let fwd = (target - eye).normalize_or(Vec3::Z);
    let right = fwd.cross(up).normalize_or(Vec3::X);
    let up2 = right.cross(fwd).normalize_or(Vec3::Y);
    (cam, View { vp, eye, fwd, up: up2, right, sw, sh })
}

struct Models {
    player: Model,
    enemy: Model,
    ace: Model,
    missile: Model,
    tank: Model,
    sam: Model,
    hangar: Model,
    wingman: Model,
    rival: Model,
}

fn draw_world(g: &Game, cam: &Camera3D, v: &View, b: &mut Batch, m: &Models, show_player: bool, mobile: bool) {
    world::draw_sky(&v.vp, v.eye, v.fwd, v.sw, v.sh);
    set_camera(cam);
    b.cam = v.eye;
    b.fog = true;
    // phones: shorter view distance, fog closes in earlier
    let radius = if mobile { world::GRID_RADIUS_MOBILE } else { world::GRID_RADIUS };
    b.fog_end = radius as f32 * world::CELL * 0.95;
    b.fog_start = b.fog_end * 0.25;
    world::draw_ground(b, v.eye, radius);
    b.flush();
    world::draw_mountains(b, v.eye, if mobile { 5 } else { 8 });
    for gt in &g.ground {
        if !gt.alive || gt.pos.distance(v.eye) > 14000.0 {
            continue;
        }
        let model = match gt.kind {
            GroundKind::Sam => &m.sam,
            GroundKind::Tank => &m.tank,
            GroundKind::Hangar => &m.hangar,
        };
        draw_model(b, model, gt.pos, Quat::from_rotation_y(gt.yaw), 1.0, gt.flash);
    }
    for e in &g.enemies {
        let d = e.pos.distance(v.eye);
        // distant aircraft are drawn a little larger so they stay visible on small screens
        let sc = 1.0 + ((d - 1500.0) / 3000.0).clamp(0.0, 1.5);
        draw_model(b, if e.ace { &m.ace } else { &m.enemy }, e.pos, e.rot, sc, e.flash);
    }
    if show_player && g.player.alive {
        draw_model(b, &m.player, g.player.pos, g.player.rot, 1.0, g.player.hit_flash * 0.5);
    }
    for ms in &g.missiles {
        draw_model(b, &m.missile, ms.pos, quat_from_fwd_up(ms.dir, Vec3::Y), 1.6, 0.0);
    }
    // other players in an online match
    let other = if g.mode == MatchMode::Dogfight { &m.rival } else { &m.wingman };
    for r in g.remotes.iter() {
        if r.alive {
            let d = r.pos.distance(v.eye);
            let sc = 1.0 + ((d - 1500.0) / 3000.0).clamp(0.0, 1.5);
            draw_model(b, other, r.pos, r.rot, sc, 0.0);
        }
        for (mp, md) in &r.missiles {
            draw_model(b, &m.missile, *mp, quat_from_fwd_up(*md, Vec3::Y), 1.6, 0.0);
        }
    }
    for em in &g.ext_missiles {
        draw_model(b, &m.missile, em.pos, quat_from_fwd_up(em.dir, Vec3::Y), 1.6, 0.0);
    }
    b.flush();
    // effects without fog
    b.fog = false;
    let r = v.right;
    let u = v.up;
    for bl in &g.bullets {
        let col = if bl.player { Color::new(1.0, 0.95, 0.4, 1.0) } else { Color::new(1.0, 0.45, 0.2, 1.0) };
        let dist = bl.pos.distance(v.eye);
        if dist < 30.0 {
            continue; // just left the muzzle: too close to the eye to draw nicely
        }
        let travelled = (GUN_LIFE - bl.life) * bl.vel.length();
        let len = 25.0f32.min(travelled * 0.5);
        let tail = bl.pos - bl.vel.normalize_or_zero() * len;
        b.streak(tail, bl.pos, 0.35 + dist * 0.0008, col);
    }
    for f in &g.flares {
        b.billboard(f.pos, 3.0, r, u, Color::new(1.0, 1.0, 0.85, 1.0));
        b.billboard(f.pos, 7.0, r, u, Color::new(1.0, 0.6, 0.2, 0.35));
    }
    for ms in &g.missiles {
        b.billboard(ms.pos - ms.dir * 2.5, 1.6, r, u, Color::new(1.0, 0.8, 0.4, 1.0));
    }
    for p in &g.particles {
        let k = 1.0 - p.life / p.max_life;
        let c = mix(p.col, p.col2, k);
        b.billboard(p.pos, p.size, r, u, c);
    }
    b.flush();
    set_default_camera();
}

struct SelfTest {
    #[allow(dead_code)]
    path: String,
    frame: u32,
    p0: Vec3,
    q0: Quat,
    thr0: f32,
    move_result: Option<(bool, String)>,
    dummy1: u32,
    dummy2: u32,
    shots_before: u32,
    hits_before: u32,
    missiles_before: u32,
    launched: bool,
    second_blocked: Option<bool>,
    kills_at_gun_end: i32,
    respawn_check_start: f32,
    hud: HudFlags,
    default_weapon_ok: bool,
    locked_at_launch: bool,
}

fn level_player(g: &mut Game) {
    let f = g.player.fwd();
    g.player.rot = quat_from_fwd_up(vec3(f.x, 0.0, f.z).normalize_or(Vec3::Z), Vec3::Y);
    if g.player.pos.y < 1500.0 {
        g.player.pos.y = 2000.0;
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = parse_args();
    let title_tex = Texture2D::from_file_with_format(TITLE_PNG, Some(ImageFormat::Png));
    title_tex.set_filter(FilterMode::Linear);

    let models = Models {
        player: player_jet(),
        enemy: enemy_jet(false),
        ace: enemy_jet(true),
        missile: missile_model(),
        tank: tank_model(),
        sam: sam_model(),
        hangar: hangar_model(),
        wingman: tinted(player_jet(), Color::new(0.75, 1.0, 1.35, 1.0)),
        rival: tinted(player_jet(), Color::new(1.35, 0.75, 1.1, 1.0)),
    };
    let mut batch = Batch::new();
    let mut inp = InputState::new();
    // --touch-ui: show the phone controls on a computer (for previews and testing)
    if std::env::args().any(|a| a == "--touch-ui") {
        inp.touch_mode = true;
    }
    let mut diff_sel = DEFAULT_DIFFICULTY;
    {
        // --difficulty easy|normal|hard (optional, the menus can change it too)
        let a: Vec<String> = std::env::args().collect();
        if let Some(i) = a.iter().position(|x| x == "--difficulty") {
            if let Some(v) = a.get(i + 1) {
                if let Some(k) = DIFFICULTIES.iter().position(|d| d.name.eq_ignore_ascii_case(v)) {
                    diff_sel = k;
                }
            }
        }
    }
    let mut g = Game::new(diff_sel);
    // ---- multiplayer: server address (--server ws://host:port/ws, or the page's own server in a browser)
    let server_url = {
        let a: Vec<String> = std::env::args().collect();
        a.iter().position(|x| x == "--server").and_then(|i| a.get(i + 1).cloned()).unwrap_or_else(net::default_server)
    };
    let pilot_name = format!("PILOT{}", macroquad::rand::gen_range(10, 99));
    let mut mpx = mp::Mp::new(server_url, pilot_name);
    let mut lb = lobby::Lobby::new();
    let mut state = State::Title;
    let mut chase_eye = g.player.pos;
    let mut hi_score = 0;
    let mut t_total = 0.0f32;
    let mut frame: u32 = 0;
    let mut over_t = 0.0f32;
    let fixed = args.screenshot.is_some() || args.self_test.is_some();
    let mut st: Option<SelfTest> = args.self_test.clone().map(|p| SelfTest {
        path: p,
        frame: 0,
        p0: Vec3::ZERO,
        q0: Quat::IDENTITY,
        thr0: 0.0,
        move_result: None,
        dummy1: 0,
        dummy2: 0,
        shots_before: 0,
        hits_before: 0,
        missiles_before: 0,
        launched: false,
        second_blocked: None,
        kills_at_gun_end: 0,
        respawn_check_start: 0.0,
        hud: HudFlags::default(),
        default_weapon_ok: false,
        locked_at_launch: false,
    });
    // title backdrop: player jet flying level
    g.player.pos = vec3(0.0, 1200.0, -2000.0);

    // --title / --lobby / --create with --screenshot preview menus instead of a fight
    let lobby_preview = std::env::args().any(|a| a == "--lobby" || a == "--create");
    let title_preview = std::env::args().any(|a| a == "--title") || lobby_preview;
    if lobby_preview {
        state = State::Lobby;
        mpx.connect();
        if std::env::args().any(|a| a == "--create") {
            lb.view = lobby::View::Create;
            lb.public = false;
            lb.password = "skyfire".into();
            lb.match_name = "FRIDAY NIGHT".into();
        }
    }
    if args.screenshot.is_some() && !title_preview {
        rand::srand(7);
        state = State::Playing;
        g = Game::new(diff_sel);
        g.messages.clear();
        let p = g.player.pos;
        let f = g.player.fwd();
        let side = g.player.right();
        g.spawn_enemy_at(p + f * 700.0 + side * 60.0 + Vec3::Y * 25.0, f, false, Ai::Dummy);
        g.spawn_enemy_at(p + f * 2200.0 - side * 500.0 + Vec3::Y * 150.0, -side, false, Ai::Attack);
        g.spawn_enemy_at(p + f * 3800.0 + side * 900.0 + Vec3::Y * 300.0, f, true, Ai::Attack);
        g.player.shield = 100.0;
    }

    loop {
        let dt = if fixed { 1.0 / 60.0 } else { get_frame_time().min(0.05) };
        t_total += dt;
        frame += 1;
        let sw = screen_width();
        let sh = screen_height();
        let mut c = inp.poll(sw, sh, state == State::Playing);
        let tapped = inp.last_touch_tap || is_mouse_button_pressed(MouseButton::Left);

        let mut st_done = false;
        // The self-test advances several fixed game steps per drawn frame, so it finishes quickly
        // even when the window is covered and draws slowly.
        let substeps = if st.is_some() { SELF_TEST_SUBSTEPS } else { 1 };
        for _sub in 0..substeps {
        if st_done {
            break;
        }
        // ---------------- self-test script ----------------
        if let Some(t) = st.as_mut() {
            t.frame += 1;
            let tt = t.frame as f32 / 60.0;
            c = Controls::default();
            if t.frame == 1 {
                rand::srand(42);
                g = Game::new(diff_sel);
                g.spawning = false;
                state = State::Playing;
                inp.tilt_calibrated = false;
                t.p0 = g.player.pos;
                t.q0 = g.player.rot;
                t.thr0 = g.player.throttle;
            }
            if tt < 1.5 {
                c.pitch = 0.6;
                c.throttle_delta = 1.0;
            } else if tt < 2.5 {
                c.roll = 0.6;
            }
            if t.frame == 150 {
                let d = g.player.pos.distance(t.p0);
                let turned = g.player.rot.angle_between(t.q0);
                let ok = d > 300.0 && turned > 0.2 && g.player.throttle > t.thr0;
                t.move_result = Some((ok, format!("flew {:.0} m in 2.5 s, attitude changed {:.0} deg, throttle {:.0}% -> {:.0}%", d, turned.to_degrees(), t.thr0 * 100.0, g.player.throttle * 100.0)));
                level_player(&mut g);
                g.enemies.clear();
                let p = g.player.pos;
                let f = g.player.fwd();
                t.dummy1 = g.spawn_enemy_at(p + f * 350.0, f, false, Ai::Dummy);
                let spd = g.player.speed;
                g.enemies.last_mut().unwrap().speed = spd;
                t.shots_before = g.player.shots_fired;
                t.hits_before = g.player.shots_hit;
            }
            if (150..240).contains(&t.frame) {
                c.gun = true;
            }
            if t.frame == 240 {
                t.kills_at_gun_end = g.kills;
                level_player(&mut g);
                t.default_weapon_ok = g.player.weapon == DEFAULT_WEAPON;
                g.bullets.clear(); // the missile test target must fall to the missile, not stray rounds
                let p = g.player.pos;
                let f = g.player.fwd();
                t.dummy2 = g.spawn_enemy_at(p + f * 2500.0, f, false, Ai::Dummy);
                let spd = g.player.speed;
                g.enemies.last_mut().unwrap().speed = spd;
                t.missiles_before = g.player.missiles_fired;
            }
            if t.frame == 330 {
                t.locked_at_launch = g.player.locked;
                t.launched = g.try_fire_missile();
            }
            if t.frame == 333 {
                // infinite payload, but governed by the cooldown timer
                t.second_blocked = Some(!g.try_fire_missile());
            }
            if t.frame == 560 {
                g.spawning = true;
                g.spawn_t = 0.5;
                t.respawn_check_start = g.time;
            }
            if t.frame >= 760 {
                st_done = true;
            }
        }

        if args.screenshot.is_some() && !title_preview {
            // scripted: level flight, then a short gun burst so tracers show in the picture
            c = Controls::default();
            c.gun = frame >= 92;
        }

        // ---------------- difficulty picker (title, pause, game over) ----------------
        let mut tap_used = false;
        let picker_live = matches!(state, State::Title | State::GameOver) || (state == State::Paused && !g.online());
        if picker_live && st.is_none() && args.screenshot.is_none() {
            if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                diff_sel = (diff_sel + DIFFICULTIES.len() - 1) % DIFFICULTIES.len();
            }
            if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                diff_sel = (diff_sel + 1) % DIFFICULTIES.len();
            }
            if tapped {
                let y = picker_y(state, sw, sh);
                for (i, r) in diff_rects(sw, sh, y).iter().enumerate() {
                    if r.contains(inp.last_tap_pos) {
                        diff_sel = i;
                        tap_used = true;
                    }
                }
            }
            if state == State::Paused {
                g.diff = diff_sel; // takes effect right away
            }
        }
        let tapped = tapped && !tap_used;

        // ---------------- state machine ----------------
        match state {
            State::Title => {
                g.player.pos += g.player.fwd() * 120.0 * dt;
                g.time += dt;
                let [solo_r, multi_r] = title_buttons(sw, sh);
                let tap_at = |r: Rect| tapped && r.contains(inp.last_tap_pos);
                if c.confirm || tap_at(solo_r) {
                    g = Game::new(diff_sel);
                    state = State::Playing;
                    inp.tilt_calibrated = false; // current phone angle becomes "level"
                    chase_eye = g.player.pos;
                } else if tap_at(multi_r) || is_key_pressed(KeyCode::M) {
                    state = State::Lobby;
                    lb.view = lobby::View::Main;
                    if !mpx.ready() {
                        mpx.connect();
                    }
                }
            }
            State::Lobby => {
                g.player.pos += g.player.fwd() * 120.0 * dt;
                g.time += dt;
                let evs = mpx.poll(None, dt);
                if let Some(j) = lobby::handle_events(&mut lb, &mut mpx, evs) {
                    let host = j.host == j.my_id;
                    g = Game::new_match(j.diff, j.mode, j.my_id, host);
                    diff_sel = j.diff;
                    state = State::Playing;
                    inp.tilt_calibrated = false;
                    chase_eye = g.player.pos;
                }
            }
            State::Playing if g.online() => {
                let evs = mpx.poll(Some(&mut g), dt);
                let mut kicked = None;
                for e in evs {
                    if let mp::MpEvent::Closed(why) = e {
                        kicked = Some(why);
                    }
                }
                if let Some(why) = kicked {
                    lb.show_error(&why);
                    g = Game::new(diff_sel);
                    state = State::Lobby;
                } else {
                    if c.pause {
                        state = State::Paused;
                    }
                    if c.recenter {
                        inp.recenter_tilt();
                    }
                    g.update(&c, dt);
                    mpx.sync(&mut g, dt);
                }
            }
            State::Paused if g.online() => {
                // the match keeps running while this menu is open
                let evs = mpx.poll(Some(&mut g), dt);
                let closed = evs.iter().any(|e| matches!(e, mp::MpEvent::Closed(_)));
                g.update(&Controls::default(), dt);
                mpx.sync(&mut g, dt);
                if closed {
                    lb.show_error("THE MATCH ENDED");
                    g = Game::new(diff_sel);
                    state = State::Lobby;
                } else if (tapped && leave_rect(sw, sh).contains(inp.last_tap_pos)) || is_key_pressed(KeyCode::L) {
                    mpx.leave();
                    g = Game::new(diff_sel);
                    state = State::Lobby;
                } else if tapped && pause_toggles_tap(&mut inp, sw, sh) {
                    // invert / tilt sensitivity changed
                } else if c.pause || c.confirm || tapped {
                    state = State::Playing;
                }
            }
            State::Playing => {
                if c.pause {
                    state = State::Paused;
                } else {
                    if c.recenter {
                        inp.recenter_tilt();
                    }
                    g.update(&c, dt);
                    if g.game_over {
                        hi_score = hi_score.max(g.score);
                        state = State::GameOver;
                        over_t = 0.0;
                    }
                }
            }
            State::Paused => {
                if tapped && pause_toggles_tap(&mut inp, sw, sh) {
                    // invert / tilt sensitivity changed
                } else if c.pause || c.confirm || tapped {
                    state = State::Playing;
                }
            }
            State::GameOver => {
                over_t += dt;
                g.update(&Controls::default(), dt);
                if over_t > 1.0 && (c.confirm || tapped) {
                    g = Game::new(diff_sel);
                    state = State::Playing;
                    inp.tilt_calibrated = false;
                }
            }
        }
        } // substeps

        // ---------------- draw ----------------
        let title_t = if matches!(state, State::Title | State::Lobby) { Some(t_total) } else { None };
        let (cam, view) = make_view(&g, &mut chase_eye, title_t);
        let show_player = title_t.is_some() || !g.cockpit || !g.player.alive;
        draw_world(&g, &cam, &view, &mut batch, &models, show_player, inp.touch_mode);

        let mut flags = HudFlags::default();
        match state {
            State::Title => {
                draw_title(&title_tex, &inp, hi_score, t_total);
                draw_picker(diff_sel, sw, sh, picker_y(state, sw, sh));
                let [solo_r, multi_r] = title_buttons(sw, sh);
                let bs = ui_scale(sw, sh).max(0.85);
                lobby::button(solo_r, if inp.touch_mode { "FLY SOLO" } else { "FLY SOLO (ENTER)" }, true, hud_green(), None, bs);
                lobby::button(multi_r, if inp.touch_mode { "MULTIPLAYER" } else { "MULTIPLAYER (M)" }, true, hud_amber(), None, bs);
            }
            State::Lobby => {
                let tap = if tapped { Some(inp.last_tap_pos) } else { None };
                if let lobby::Action::Back = lobby::frame(&mut lb, &mut mpx, &mut diff_sel, tap, sw, sh, dt) {
                    mpx.disconnect();
                    state = State::Title;
                }
            }
            State::Playing | State::Paused | State::GameOver => {
                draw_hud(&g, &view, &inp, &mut flags, t_total);
                if state == State::Paused && g.online() {
                    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
                    let s = ui_scale(sw, sh);
                    let bs = s.max(0.85);
                    if let Some(j) = &mpx.joined {
                        let kind = if j.mode == MatchMode::Dogfight { "DOGFIGHT" } else { "CO-OP" };
                        let access = if j.public { "PUBLIC" } else { "PRIVATE (PASSWORD)" };
                        text_c(&format!("{}  -  CODE {}  -  {}  -  {}", j.name, j.code, kind, access), sw * 0.5, 40.0 * s, 20.0 * s, hud_green());
                        text_c(if j.host == mpx.my_id { "YOU ARE THE HOST - THE MATCH ENDS IF YOU LEAVE" } else { "THE MATCH KEEPS RUNNING WHILE THIS MENU IS OPEN" }, sw * 0.5, 62.0 * s, 15.0 * s, hud_dim());
                    }
                    draw_scoreboard(&g, sw * 0.5 + 115.0 * s, 74.0 * s, s);
                    text_c(if inp.touch_mode { "TAP TO RESUME" } else { "P / ESC / ENTER TO RESUME" }, sw * 0.5, sh * 0.45 + 40.0 * s, 22.0 * s, hud_green());
                    draw_pause_toggles(&inp, sw, sh);
                    lobby::button(leave_rect(sw, sh), if inp.touch_mode { "LEAVE MATCH" } else { "LEAVE MATCH (L)" }, true, hud_red(), None, bs);
                } else if state == State::Paused {
                    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
                    let s = ui_scale(sw, sh);
                    text_c("PAUSED", sw * 0.5, sh * 0.45, 48.0 * s, hud_green());
                    text_c(if inp.touch_mode { "TAP TO RESUME" } else { "P / ESC / ENTER TO RESUME" }, sw * 0.5, sh * 0.45 + 40.0 * s, 22.0 * s, hud_green());
                    draw_pause_toggles(&inp, sw, sh);
                    draw_picker(diff_sel, sw, sh, picker_y(state, sw, sh));
                }
                if state == State::GameOver {
                    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
                    let s = ui_scale(sw, sh);
                    text_c("GAME OVER", sw * 0.5, sh * 0.38, 56.0 * s, hud_red());
                    text_c(&format!("SCORE {}    WAVE {}    KILLS {}", g.score, g.wave, g.kills + g.ground_kills), sw * 0.5, sh * 0.38 + 48.0 * s, 26.0 * s, hud_green());
                    text_c(&format!("BEST {}", hi_score), sw * 0.5, sh * 0.38 + 80.0 * s, 22.0 * s, hud_amber());
                    if (t_total * 2.0).fract() < 0.7 {
                        text_c(if inp.touch_mode { "TAP TO FLY AGAIN" } else { "PRESS ENTER TO FLY AGAIN" }, sw * 0.5, sh * 0.38 + 130.0 * s, 26.0 * s, WHITE);
                    }
                    draw_picker(diff_sel, sw, sh, picker_y(state, sw, sh));
                }
            }
        }

        // ---------------- self-test bookkeeping ----------------
        if let Some(t) = st.as_mut() {
            let f = &mut t.hud;
            f.speed |= flags.speed;
            f.altitude |= flags.altitude;
            f.heading |= flags.heading;
            f.score |= flags.score;
            f.health |= flags.health;
            f.weapon |= flags.weapon;
            f.radar |= flags.radar;
            f.lives |= flags.lives;
            f.wave |= flags.wave;
            if t.frame >= 400 && !f.touch_buttons {
                // touch HUD: draw the phone controls once to make sure they render
                let mut tf = HudFlags::default();
                draw_touch_controls(&g, &inp, sw, sh, ui_scale(sw, sh), &mut tf);
                f.touch_buttons = tf.touch_buttons;
            }
        }

        if st_done {
            if let Some(t) = st.take() {
                write_self_test(&t, &g);
            }
            break;
        }

        if let Some(path) = &args.screenshot {
            if frame == 85 {
                g.player.throttle = 0.95;
            }
            if frame == 120 {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(dir) = std::path::Path::new(path).parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    get_screen_data().export_png(path);
                }
                let _ = path;
                break;
            }
        }

        next_frame().await;
    }
}

/// Pause screen toggles: INVERT PITCH, and on phones the tilt sensitivity next to it.
fn pause_toggle_rects(sw: f32, sh: f32, touch: bool) -> (Rect, Option<Rect>) {
    let r = invert_rect(sw, sh);
    if !touch {
        return (r, None);
    }
    let gap = 8.0;
    let half = (r.w - gap) * 0.5;
    (Rect::new(r.x, r.y, half, r.h), Some(Rect::new(r.x + half + gap, r.y, half, r.h)))
}

fn pause_toggles_tap(inp: &mut InputState, sw: f32, sh: f32) -> bool {
    let (inv, sens) = pause_toggle_rects(sw, sh, inp.touch_mode);
    if inv.contains(inp.last_tap_pos) {
        inp.invert_pitch = !inp.invert_pitch;
        return true;
    }
    if let Some(r) = sens {
        if r.contains(inp.last_tap_pos) {
            inp.tilt_sens = (inp.tilt_sens + 1) % TILT_SENS_NAMES.len();
            return true;
        }
    }
    false
}

fn draw_pause_toggles(inp: &InputState, sw: f32, sh: f32) {
    let s = ui_scale(sw, sh);
    let (inv, sens) = pause_toggle_rects(sw, sh, inp.touch_mode);
    let mut boxes = vec![(inv, format!("{}: {}", if sens.is_some() { "INVERT" } else { "INVERT PITCH" }, if inp.invert_pitch { "ON" } else { "OFF" }))];
    if let Some(r) = sens {
        boxes.push((r, format!("TILT: {}", TILT_SENS_NAMES[inp.tilt_sens])));
    }
    for (r, lab) in boxes {
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.1, 0.04, 0.8));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, hud_green());
        let fs = (22.0 * s).max(13.0);
        text_c(&lab, r.x + r.w * 0.5, r.y + r.h * 0.5 + fs * 0.35, fs, hud_green());
    }
}

/// Title screen: [FLY SOLO] [MULTIPLAYER]
fn title_buttons(sw: f32, sh: f32) -> [Rect; 2] {
    let s = ui_scale(sw, sh).max(0.85);
    let w = 230.0 * s;
    let h = 42.0 * s;
    let y = sh - h - 12.0 * s;
    let gap = 16.0 * s;
    [Rect::new(sw * 0.5 - w - gap * 0.5, y, w, h), Rect::new(sw * 0.5 + gap * 0.5, y, w, h)]
}

/// Online pause menu: leave the match.
fn leave_rect(sw: f32, sh: f32) -> Rect {
    let r = invert_rect(sw, sh);
    let s = ui_scale(sw, sh).max(0.85);
    Rect::new(r.x, r.y + r.h + 12.0 * s, r.w, 44.0 * s)
}

/// Where the difficulty picker sits on each menu screen.
fn picker_y(state: State, sw: f32, sh: f32) -> f32 {
    let s = ui_scale(sw, sh);
    match state {
        State::Paused => {
            let r = invert_rect(sw, sh);
            r.y + r.h + 34.0 * s
        }
        State::GameOver => sh * 0.38 + 165.0 * s,
        _ => sh * 0.16 + 92.0 * s,
    }
}

fn diff_rects(sw: f32, sh: f32, y: f32) -> [Rect; 3] {
    let s = ui_scale(sw, sh).max(0.85); // finger-sized on phones
    let w = 150.0 * s;
    let h = 44.0 * s;
    let gap = 14.0 * s;
    let x0 = sw * 0.5 - (w * 3.0 + gap * 2.0) * 0.5;
    [0, 1, 2].map(|i| Rect::new(x0 + i as f32 * (w + gap), y, w, h))
}

/// Three tappable boxes: EASY / NORMAL / HARD, with a one-line description.
fn draw_picker(sel: usize, sw: f32, sh: f32, y: f32) {
    let s = ui_scale(sw, sh).max(0.85);
    let rects = diff_rects(sw, sh, y);
    text_c("ENEMY DIFFICULTY  (TAP OR LEFT / RIGHT)", sw * 0.5, y - 8.0 * s, 17.0 * s, Color::new(0.85, 0.92, 1.0, 1.0));
    for (i, r) in rects.iter().enumerate() {
        let on = i == sel;
        let col = match i {
            0 => Color::new(0.4, 1.0, 0.5, 1.0),
            1 => Color::new(1.0, 0.85, 0.3, 1.0),
            _ => Color::new(1.0, 0.4, 0.3, 1.0),
        };
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(col.r * 0.25, col.g * 0.25, col.b * 0.25, if on { 0.95 } else { 0.6 }));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if on { 4.0 } else { 1.5 }, if on { col } else { Color::new(col.r, col.g, col.b, 0.5) });
        text_c(DIFFICULTIES[i].name, r.x + r.w * 0.5, r.y + r.h * 0.66, 24.0 * s, if on { WHITE } else { Color::new(0.8, 0.8, 0.8, 0.8) });
    }
    let last = rects[2];
    let bw = last.x + last.w - rects[0].x;
    draw_rectangle(rects[0].x, y + rects[0].h + 4.0 * s, bw, 22.0 * s, Color::new(0.0, 0.03, 0.06, 0.7));
    text_c(DIFFICULTIES[sel].blurb, sw * 0.5, y + rects[0].h + 20.0 * s, 15.0 * s, Color::new(0.75, 1.0, 0.8, 1.0));
}

/// Tappable "invert pitch" toggle on the pause screen.
fn invert_rect(sw: f32, sh: f32) -> Rect {
    let s = ui_scale(sw, sh);
    Rect::new(sw * 0.5 - 170.0 * s, sh * 0.45 + 70.0 * s, 340.0 * s, 46.0 * s)
}

fn draw_title(tex: &Texture2D, inp: &InputState, hi: i32, t: f32) {
    let sw = screen_width();
    let sh = screen_height();
    let s = ui_scale(sw, sh);
    // cover-fit the art
    let tw = tex.width();
    let th = tex.height();
    let k = (sw / tw).max(sh / th);
    let w = tw * k;
    let h = th * k;
    draw_texture_ex(tex, (sw - w) * 0.5, (sh - h) * 0.5, Color::new(1.0, 1.0, 1.0, 0.92), DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() });
    draw_rectangle(0.0, 0.0, sw, sh * 0.32, Color::new(0.0, 0.02, 0.08, 0.35));
    let ty = sh * 0.16;
    text_c("RETAL", sw * 0.5 + 3.0, ty + 3.0, 84.0 * s, Color::new(0.0, 0.0, 0.0, 0.7));
    text_c("RETAL", sw * 0.5, ty, 84.0 * s, Color::new(1.0, 0.85, 0.35, 1.0));
    text_c("ENDLESS ARCADE AIR COMBAT", sw * 0.5, ty + 34.0 * s, 24.0 * s, Color::new(0.85, 0.92, 1.0, 1.0));
    if hi > 0 {
        text(&format!("BEST {}", hi), 16.0 * s, sh - 16.0 * s, 22.0 * s, hud_amber());
    }
    let panel_y = sh * 0.66;
    draw_rectangle(sw * 0.5 - 330.0 * s, panel_y - 28.0 * s, 660.0 * s, 150.0 * s, Color::new(0.0, 0.03, 0.06, 0.72));
    let lines: &[&str] = if inp.touch_mode {
        &[
            "TILT PHONE TO FLY  (HOLD IT LEVEL WHEN YOU TAP START)",
            "GUN = HOLD   MSL = FIRE MISSILE   WPN = SWITCH MISSILE",
            "FLR = FLARES   LEFT SLIDER = THROTTLE   LEVEL = RE-CENTRE TILT",
            "NO TILT SENSOR? DRAG ON THE LEFT SIDE TO STEER",
        ]
    } else {
        &[
            "ARROWS / WASD  PITCH & ROLL     Q E  RUDDER     SHIFT / CTRL  THROTTLE",
            "SPACE / LEFT MOUSE  GUN     ENTER / M / RIGHT MOUSE  MISSILE",
            "TAB or 1-3  WEAPON     T  TARGET     F  FLARES     V  VIEW     P  PAUSE",
            "I  INVERT PITCH     WEAPONS NEVER RUN OUT - WATCH THE COOLDOWNS",
        ]
    };
    if inp.touch_mode {
        let (txt, col) = match tilt_state() {
            1 => ("TILT SENSOR: ON", hud_green()),
            2 => ("TILT SENSOR: BLOCKED - ALLOW MOTION ACCESS AND RELOAD, OR DRAG TO STEER", hud_amber()),
            3 => ("NO TILT SENSOR - DRAG ON THE LEFT SIDE TO STEER", hud_amber()),
            _ => ("TILT SENSOR: WAITING... (TILT THE PHONE)", hud_dim()),
        };
        text_c(txt, sw * 0.5, panel_y - 36.0 * s, (17.0 * s).max(13.0), col);
    }
    for (i, l) in lines.iter().enumerate() {
        text_c(l, sw * 0.5, panel_y + i as f32 * 26.0 * s, 18.0 * s, Color::new(0.75, 1.0, 0.8, 1.0));
    }
    if (t * 2.0).fract() < 0.7 {
        let msg = "";
        text_c(msg, sw * 0.5, sh * 0.93, 32.0 * s, WHITE);
    }
}

fn write_self_test(t: &SelfTest, g: &Game) {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    let (mok, mdet) = t.move_result.clone().unwrap_or((false, "move phase never ran".into()));
    steps.push(("move".into(), mok, mdet));

    let shots = g.player.shots_fired - t.shots_before;
    let hits = g.player.shots_hit - t.hits_before;
    let d1_dead = !g.enemies.iter().any(|e| e.id == t.dummy1);
    let d2_dead = !g.enemies.iter().any(|e| e.id == t.dummy2);
    let blocked = t.second_blocked.unwrap_or(false);
    let act_ok = shots > 0 && hits > 0 && d1_dead && t.launched && blocked && d2_dead && t.default_weapon_ok && t.locked_at_launch;
    steps.push((
        "act".into(),
        act_ok,
        format!(
            "gun fired {} rounds, {} hit, gun target destroyed: {}; default weapon is the radar missile: {}, locked: {}, launched: {}, second launch blocked by {:.1}s cooldown: {}, missile target destroyed: {}; total kills {}",
            shots, hits, d1_dead, t.default_weapon_ok, t.locked_at_launch, t.launched, g.cooldown_of(DEFAULT_WEAPON), blocked, d2_dead, g.kills
        ),
    ));

    let h = &t.hud;
    let hud_ok = h.speed && h.altitude && h.heading && h.score && h.health && h.weapon && h.radar && h.lives && h.wave;
    steps.push((
        "hud".into(),
        hud_ok,
        format!(
            "speed {}, altitude {}, heading {}, score {}, hull {}, weapons/cooldowns {}, radar {}, lives {}, wave {}",
            h.speed, h.altitude, h.heading, h.score, h.health, h.weapon, h.radar, h.lives, h.wave
        ),
    ));

    let alive = g.enemies.iter().filter(|e| e.alive).count();
    let waves_ok = alive > 0 && g.time - t.respawn_check_start > 1.0;
    // difficulty: the same weapons/enemies under EASY and HARD
    let mut easy = Game::new(0);
    let mut hard = Game::new(2);
    let fwd = easy.player.fwd();
    let pe = easy.player.pos;
    let ide = easy.spawn_enemy_at(pe + fwd * 3000.0, fwd, false, Ai::Dummy);
    let ph = hard.player.pos;
    let idh = hard.spawn_enemy_at(ph + fwd * 3000.0, fwd, false, Ai::Dummy);
    let hp_e = easy.enemies.iter().find(|e| e.id == ide).map(|e| e.hp).unwrap_or(0.0);
    let hp_h = hard.enemies.iter().find(|e| e.id == idh).map(|e| e.hp).unwrap_or(0.0);
    let diff_ok = easy.cooldown_of(DEFAULT_WEAPON) < hard.cooldown_of(DEFAULT_WEAPON)
        && easy.lock_time_of(DEFAULT_WEAPON) < hard.lock_time_of(DEFAULT_WEAPON)
        && easy.d().gun_hit_radius > hard.d().gun_hit_radius
        && hp_e < hp_h
        && easy.d().enemy_spread > hard.d().enemy_spread;
    steps.push((
        "difficulty".into(),
        diff_ok,
        format!(
            "EASY vs HARD: radar missile cooldown {:.1}s vs {:.1}s, lock time {:.2}s vs {:.2}s, gun hit radius {:.0} m vs {:.0} m, enemy hp {:.0} vs {:.0}, enemy gun spread x{:.2} vs x{:.2}",
            easy.cooldown_of(DEFAULT_WEAPON), hard.cooldown_of(DEFAULT_WEAPON), easy.lock_time_of(DEFAULT_WEAPON), hard.lock_time_of(DEFAULT_WEAPON),
            easy.d().gun_hit_radius, hard.d().gun_hit_radius, hp_e, hp_h, easy.d().enemy_spread, hard.d().enemy_spread
        ),
    ));

    steps.push(("waves".into(), waves_ok, format!("after the test targets died, {} new enemies spawned within {:.1}s (endless respawn)", alive, g.time - t.respawn_check_start)));

    // phone controls: tilt mapping + on-screen buttons hit-testing (synthetic input)
    let (r, _) = tilt_to_stick((0.30, 0.0), (0.0, 0.0));
    let (_, pch) = tilt_to_stick((0.0, -0.30), (0.0, 0.0));
    let lay = touch_layout(screen_width(), screen_height());
    let hit = |b: Btn| lay.buttons.iter().find(|x| x.kind == b).map(|x| lay.buttons.iter().filter(|y| x.pos.distance(y.pos) < y.r * 1.25).all(|y| y.kind == b)).unwrap_or(false);
    let touch_ok = r > 0.5 && pch < -0.5 && hit(Btn::Gun) && hit(Btn::Missile) && h.touch_buttons;
    steps.push((
        "phone".into(),
        touch_ok,
        format!("tilt 17 deg right -> roll {:.2}, tilt 17 deg forward -> pitch {:.2}; GUN and MSL buttons hit-test cleanly; touch controls drawn: {}", r, pch, h.touch_buttons),
    ));

    #[cfg(not(target_arch = "wasm32"))]
    {
        let (ok, detail) = mp_self_test();
        steps.push(("multiplayer".into(), ok, detail));
    }

    let mut out = String::from("{\"version\": 1, \"steps\": [");
    for (i, (s, p, d)) in steps.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("{{\"step\": \"{}\", \"passed\": {}, \"detail\": \"{}\"}}", s, p, json_escape(d)));
    }
    out.push_str("]}\n");
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(dir) = std::path::Path::new(&t.path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&t.path, &out);
    }
    println!("{}", out);
}

/// Online play, end to end on this computer: server, private match with password, a guest joining,
/// the host's bots appearing for the guest, the guest's hit counting on the host, and a public match.
#[cfg(not(target_arch = "wasm32"))]
fn mp_self_test() -> (bool, String) {
    use std::time::{Duration, Instant};
    let addr = match server::start("127.0.0.1:0", server::find_web_dir()) {
        Ok(a) => a,
        Err(e) => return (false, format!("server did not start: {}", e)),
    };
    let url = format!("ws://{}/ws", addr);
    let mut a = mp::Mp::new(url.clone(), "HOST".into());
    let mut b = mp::Mp::new(url.clone(), "GUEST".into());
    a.connect();
    b.connect();
    let mut log: Vec<String> = Vec::new();

    // pump both sides until `done` or timeout
    fn pump(xs: &mut [&mut mp::Mp], secs: f32, mut done: impl FnMut(&[&mut mp::Mp], &Vec<(usize, mp::MpEvent)>) -> bool) -> (bool, Vec<(usize, mp::MpEvent)>) {
        let start = Instant::now();
        let mut evs = Vec::new();
        while start.elapsed().as_secs_f32() < secs {
            for (i, x) in xs.iter_mut().enumerate() {
                for e in x.poll(None, 0.016) {
                    evs.push((i, e));
                }
            }
            if done(xs, &evs) {
                return (true, evs);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        (false, evs)
    }

    let (ok, _) = pump(&mut [&mut a, &mut b], 4.0, |x, _| x[0].ready() && x[1].ready());
    if !ok {
        return (false, "clients could not connect to the test server".into());
    }
    log.push("2 pilots connected".into());

    a.create("TEST MATCH", false, "secret", MatchMode::Coop, 1);
    let (ok, evs) = pump(&mut [&mut a, &mut b], 3.0, |_, e| e.iter().any(|(i, ev)| *i == 0 && matches!(ev, mp::MpEvent::Joined(_))));
    if !ok {
        return (false, "host could not create a match".into());
    }
    let code = evs.iter().find_map(|(_, e)| if let mp::MpEvent::Joined(j) = e { Some(j.code.clone()) } else { None }).unwrap_or_default();
    log.push(format!("host created private match {}", code));

    b.got_list = false;
    b.refresh();
    let c2 = code.clone();
    let (ok, _) = pump(&mut [&mut a, &mut b], 3.0, |x, _| x[1].got_list && x[1].rooms.iter().any(|r| r.code == c2 && r.locked));
    if !ok {
        return (false, "guest did not see the private match in the list".into());
    }
    log.push("listed as PRIVATE".into());

    b.join(&code, "wrong");
    let (ok, _) = pump(&mut [&mut a, &mut b], 3.0, |_, e| e.iter().any(|(i, ev)| *i == 1 && matches!(ev, mp::MpEvent::Error(m) if m.contains("PASSWORD"))));
    if !ok {
        return (false, "a wrong password was not refused".into());
    }
    log.push("wrong password refused".into());

    b.join(&code, "secret");
    let (ok, _) = pump(&mut [&mut a, &mut b], 3.0, |_, e| e.iter().any(|(i, ev)| *i == 1 && matches!(ev, mp::MpEvent::Joined(_))));
    if !ok {
        return (false, "guest could not join with the right password".into());
    }
    log.push("joined with password".into());

    // run both games: host owns a bot, guest must see it and its hit must count on the host
    let mut ga = Game::new_match(1, MatchMode::Coop, a.my_id, true);
    let mut gb = Game::new_match(1, MatchMode::Coop, b.my_id, false);
    ga.spawning = false;
    let p = ga.player.pos;
    let f = ga.player.fwd();
    let bot = ga.spawn_enemy_at(p + f * 2000.0, f, false, Ai::Dummy);
    let dt = 1.0 / 60.0;
    let none = Controls::default();
    let step = |a: &mut mp::Mp, b: &mut mp::Mp, ga: &mut Game, gb: &mut Game| {
        a.poll(Some(ga), dt);
        b.poll(Some(gb), dt);
        ga.update(&none, dt);
        gb.update(&none, dt);
        a.sync(ga, dt);
        b.sync(gb, dt);
        std::thread::sleep(Duration::from_millis(4));
    };
    let start = Instant::now();
    while start.elapsed().as_secs_f32() < 4.0 && !(gb.enemies.iter().any(|e| e.id == bot) && gb.remotes.iter().any(|r| r.id == a.my_id) && ga.remotes.iter().any(|r| r.id == b.my_id)) {
        step(&mut a, &mut b, &mut ga, &mut gb);
    }
    let sees_bot = gb.enemies.iter().any(|e| e.id == bot);
    let sees_each_other = gb.remotes.iter().any(|r| r.id == a.my_id) && ga.remotes.iter().any(|r| r.id == b.my_id);
    if !sees_bot || !sees_each_other {
        return (false, format!("{}; guest sees host bot: {}, pilots see each other: {}", log.join(", "), sees_bot, sees_each_other));
    }
    log.push("guest sees the host's bot and both pilots see each other".into());

    gb.damage_enemy(bot, 1000.0);
    let start = Instant::now();
    while start.elapsed().as_secs_f32() < 4.0 && gb.kills == 0 {
        step(&mut a, &mut b, &mut ga, &mut gb);
    }
    let host_killed = !ga.enemies.iter().any(|e| e.id == bot);
    if gb.kills != 1 || !host_killed || gb.score <= 0 {
        return (false, format!("{}; guest hit: bot destroyed on host {}, guest credited {} kill(s), score {}", log.join(", "), host_killed, gb.kills, gb.score));
    }
    log.push(format!("guest's hit destroyed the host's bot and scored {}", gb.score));

    // public match: anyone joins without a password
    b.leave();
    b.create("OPEN SKIES", true, "", MatchMode::Dogfight, 1);
    let (ok, evs) = pump(&mut [&mut a, &mut b], 3.0, |_, e| e.iter().any(|(i, ev)| *i == 1 && matches!(ev, mp::MpEvent::Joined(_))));
    let pcode = evs.iter().find_map(|(i, e)| if let (1, mp::MpEvent::Joined(j)) = (i, e) { Some(j.code.clone()) } else { None }).unwrap_or_default();
    if !ok {
        return (false, format!("{}; could not create a public match", log.join(", ")));
    }
    let mut c = mp::Mp::new(url, "THIRD".into());
    c.connect();
    let (ok, _) = pump(&mut [&mut c], 3.0, |x, _| x[0].ready());
    if !ok {
        return (false, format!("{}; third pilot could not connect", log.join(", ")));
    }
    c.join(&pcode, "");
    let (ok, _) = pump(&mut [&mut c, &mut b], 3.0, |_, e| e.iter().any(|(i, ev)| *i == 0 && matches!(ev, mp::MpEvent::Joined(_))));
    if !ok {
        return (false, format!("{}; could not join the public match without a password", log.join(", ")));
    }
    log.push("anyone can join a public dogfight match without a password".into());

    // dogfight: the third pilot shoots down the match host
    let mut gh = Game::new_match(1, MatchMode::Dogfight, b.my_id, true);
    let mut gc = Game::new_match(1, MatchMode::Dogfight, c.my_id, false);
    gh.spawning = false;
    let start = Instant::now();
    while start.elapsed().as_secs_f32() < 4.0 && !(gc.remotes.iter().any(|r| r.id == b.my_id) && gh.remotes.iter().any(|r| r.id == c.my_id)) {
        step(&mut b, &mut c, &mut gh, &mut gc);
    }
    gh.player.shield = 0.0;
    gc.out.push(NetOut::HitPlayer(b.my_id, 500.0));
    let start = Instant::now();
    while start.elapsed().as_secs_f32() < 4.0 && gc.player_kills == 0 {
        step(&mut b, &mut c, &mut gh, &mut gc);
    }
    if gc.player_kills != 1 || gh.deaths != 1 {
        return (false, format!("{}; dogfight shoot-down: victim deaths {}, shooter credited {}", log.join(", "), gh.deaths, gc.player_kills));
    }
    log.push(format!("dogfight: a pilot shot down another and scored {}", gc.score));
    a.disconnect();
    b.disconnect();
    c.disconnect();
    (true, log.join(", "))
}
