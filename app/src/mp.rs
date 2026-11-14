//! Online play: talks to the Retal server's lobby, then keeps a match in sync.
//!
//! The match host runs the bots and ground targets and sends a world snapshot ~10 times a second.
//! Every player sends their own aircraft ~15 times a second. Hits on bots go to the host,
//! hits on other pilots (dogfight) go to that pilot, and kills are announced to everyone.

use crate::game::*;
use crate::net::{Net, NetStatus};
use crate::wsproto::clean_field;
use macroquad::prelude::*;
use std::collections::HashMap;

pub const STATE_RATE: f32 = 1.0 / 15.0;
pub const SNAPSHOT_RATE: f32 = 1.0 / 10.0;
pub const LIST_RATE: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct RoomInfo {
    pub code: String,
    pub name: String,
    pub players: u32,
    pub max: u32,
    pub mode: MatchMode,
    pub locked: bool,
    pub diff: usize,
}

#[derive(Clone, Debug)]
pub struct Joined {
    pub code: String,
    pub name: String,
    pub my_id: u32,
    pub host: u32,
    pub mode: MatchMode,
    pub diff: usize,
    pub public: bool,
}

#[derive(Debug)]
pub enum MpEvent {
    Connected,
    Joined(Joined),
    Error(String),
    Closed(String),
}

pub struct Mp {
    pub net: Net,
    pub url: String,
    pub name: String,
    pub my_id: u32,
    pub rooms: Vec<RoomInfo>,
    pub joined: Option<Joined>,
    pub peers: HashMap<u32, String>,
    pub welcomed: bool,
    pub welcomed_before_close: bool,
    pub got_list: bool,
    list_t: f32,
    state_t: f32,
    snap_t: f32,
    last_wave: i32,
}

fn f1(v: f32) -> String {
    format!("{:.1}", v)
}
fn f3(v: f32) -> String {
    format!("{:.3}", v)
}

pub fn mode_tag(m: MatchMode) -> &'static str {
    match m {
        MatchMode::Dogfight => "DOGFIGHT",
        _ => "COOP",
    }
}

fn parse_mode(s: &str) -> MatchMode {
    if s == "DOGFIGHT" {
        MatchMode::Dogfight
    } else {
        MatchMode::Coop
    }
}

impl Mp {
    pub fn new(url: String, name: String) -> Self {
        Mp {
            net: Net::new(),
            url,
            name,
            my_id: 0,
            rooms: Vec::new(),
            joined: None,
            peers: HashMap::new(),
            welcomed: false,
            welcomed_before_close: false,
            got_list: false,
            list_t: 0.0,
            state_t: 0.0,
            snap_t: 0.0,
            last_wave: 1,
        }
    }

    pub fn connect(&mut self) {
        self.net.connect(&self.url.clone());
        self.welcomed = false;
        self.got_list = false;
        self.rooms.clear();
        self.joined = None;
        self.peers.clear();
    }

    pub fn disconnect(&mut self) {
        self.net.close();
        self.welcomed = false;
        self.joined = None;
        self.peers.clear();
    }

    pub fn status(&self) -> NetStatus {
        self.net.status
    }

    pub fn ready(&self) -> bool {
        self.net.status == NetStatus::Open && self.welcomed
    }

    pub fn refresh(&mut self) {
        self.net.send("LIST");
        self.list_t = LIST_RATE;
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = clean_field(name, 16).to_uppercase();
        if self.net.status == NetStatus::Open {
            let n = self.name.clone();
            self.net.send(&format!("HELLO|{}", n));
        }
    }

    pub fn create(&mut self, name: &str, public: bool, password: &str, mode: MatchMode, diff: usize) {
        let pw = password.replace('|', "");
        self.net.send(&format!("CREATE|{}|{}|{}|{}|{}", clean_field(name, 20), if public { 1 } else { 0 }, pw, mode_tag(mode), diff));
    }

    pub fn join(&mut self, code: &str, password: &str) {
        self.net.send(&format!("JOIN|{}|{}", clean_field(code, 8).to_uppercase(), password.replace('|', "")));
    }

    pub fn leave(&mut self) {
        self.net.send("LEAVE");
        self.joined = None;
        self.peers.clear();
    }

    /// Reads everything the server sent. Game messages are applied to `g` when a match is running.
    pub fn poll(&mut self, mut g: Option<&mut Game>, dt: f32) -> Vec<MpEvent> {
        let mut ev = Vec::new();
        let prev = self.net.status;
        let msgs = self.net.poll();
        if prev != NetStatus::Open && self.net.status == NetStatus::Open {
            let n = self.name.clone();
            self.net.send(&format!("HELLO|{}", n));
        }
        if prev != NetStatus::Closed && self.net.status == NetStatus::Closed {
            let e = if self.net.error.is_empty() { "DISCONNECTED".to_string() } else { self.net.error.clone() };
            self.welcomed_before_close = self.welcomed;
            self.welcomed = false;
            self.joined = None;
            ev.push(MpEvent::Closed(e));
        }
        for m in msgs {
            let mut it = m.splitn(2, '|');
            let cmd = it.next().unwrap_or("");
            let rest = it.next().unwrap_or("");
            match cmd {
                "WELCOME" => {
                    self.my_id = rest.parse().unwrap_or(0);
                    if !self.welcomed {
                        self.welcomed = true;
                        ev.push(MpEvent::Connected);
                        self.refresh();
                    }
                }
                "ROOMS" => {
                    self.got_list = true;
                    self.rooms = rest
                        .split(';')
                        .filter(|r| !r.is_empty())
                        .filter_map(|r| {
                            let f: Vec<&str> = r.split('~').collect();
                            if f.len() < 7 {
                                return None;
                            }
                            Some(RoomInfo {
                                code: f[0].to_string(),
                                name: f[1].to_string(),
                                players: f[2].parse().unwrap_or(0),
                                max: f[3].parse().unwrap_or(8),
                                mode: parse_mode(f[4]),
                                locked: f[5] == "1",
                                diff: f[6].parse().unwrap_or(1),
                            })
                        })
                        .collect();
                }
                "JOINED" => {
                    let f: Vec<&str> = rest.split('|').collect();
                    if f.len() >= 7 {
                        let j = Joined {
                            code: f[0].to_string(),
                            my_id: f[1].parse().unwrap_or(0),
                            host: f[2].parse().unwrap_or(0),
                            mode: parse_mode(f[3]),
                            diff: f[4].parse().unwrap_or(1),
                            name: f[5].to_string(),
                            public: f.get(6).map(|v| *v == "1").unwrap_or(true),
                        };
                        self.my_id = j.my_id;
                        self.peers.clear();
                        self.joined = Some(j.clone());
                        self.last_wave = 1;
                        ev.push(MpEvent::Joined(j));
                    }
                }
                "PEER" => {
                    let mut f = rest.splitn(2, '|');
                    let id: u32 = f.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    let name = f.next().unwrap_or("").to_string();
                    if let Some(g) = g.as_deref_mut() {
                        g.msg(&format!("{} JOINED", name), Color::new(0.6, 0.85, 1.0, 1.0));
                    }
                    self.peers.insert(id, name);
                }
                "PEERLEFT" => {
                    let id: u32 = rest.parse().unwrap_or(0);
                    let name = self.peers.remove(&id).unwrap_or_default();
                    if let Some(g) = g.as_deref_mut() {
                        g.remotes.retain(|r| r.id != id);
                        g.msg(&format!("{} LEFT", name), Color::new(0.6, 0.85, 1.0, 1.0));
                    }
                }
                "CLOSED" => {
                    self.joined = None;
                    self.peers.clear();
                    ev.push(MpEvent::Closed(rest.to_string()));
                }
                "ERR" => ev.push(MpEvent::Error(rest.to_string())),
                "F" => {
                    let mut f = rest.splitn(2, '|');
                    let from: u32 = f.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    let payload = f.next().unwrap_or("");
                    if let Some(g) = g.as_deref_mut() {
                        self.apply(g, from, payload);
                    }
                }
                _ => {}
            }
        }
        // lobby list refresh
        if self.joined.is_none() && self.ready() {
            self.list_t -= dt;
            if self.list_t <= 0.0 {
                self.refresh();
            }
        }
        ev
    }

    /// Sends this computer's part of the match: own aircraft, host snapshot, hit/kill events.
    pub fn sync(&mut self, g: &mut Game, dt: f32) {
        if self.joined.is_none() || self.net.status != NetStatus::Open {
            g.out.clear();
            return;
        }
        let host_id = self.joined.as_ref().map(|j| j.host).unwrap_or(0);
        for o in std::mem::take(&mut g.out) {
            match o {
                NetOut::HitEnemy(id, d) => self.net.send(&format!("T|{}|H,{},{}", host_id, id, f1(d))),
                NetOut::HitGround(id, d) => self.net.send(&format!("T|{}|HG,{},{}", host_id, id, f1(d))),
                NetOut::Kill(k, id, killer, pts, p) => self.net.send(&format!("R|K,{},{},{},{},{},{},{}", k, id, killer, pts, f1(p.x), f1(p.y), f1(p.z))),
                NetOut::Damage(victim, d) => self.net.send(&format!("T|{}|D,{}", victim, f1(d))),
                NetOut::HitPlayer(victim, d) => self.net.send(&format!("T|{}|PH,{}", victim, f1(d))),
                NetOut::Down(by, p) => self.net.send(&format!("R|PK,{},{},{},{}", by, f1(p.x), f1(p.y), f1(p.z))),
                NetOut::Flares => self.net.send(&format!("T|{}|FL", host_id)),
            }
        }
        self.state_t -= dt;
        if self.state_t <= 0.0 {
            self.state_t = STATE_RATE;
            let msg = format!("R|{}", encode_state(g));
            self.net.send(&msg);
        }
        if g.authority {
            self.snap_t -= dt;
            if self.snap_t <= 0.0 {
                self.snap_t = SNAPSHOT_RATE;
                let msg = format!("R|{}", encode_snapshot(g));
                self.net.send(&msg);
            }
        }
    }

    fn apply(&mut self, g: &mut Game, from: u32, payload: &str) {
        let mut parts = payload.split(';');
        let head = parts.next().unwrap_or("");
        let f: Vec<&str> = head.split(',').collect();
        let num = |i: usize| -> f32 { f.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) };
        let id_at = |i: usize| -> u32 { f.get(i).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) };
        match f.first().copied().unwrap_or("") {
            "P" => {
                if f.len() < 16 {
                    return;
                }
                let name = self.peers.get(&from).cloned().unwrap_or_else(|| format!("PILOT{}", from));
                let idx = match g.remotes.iter().position(|r| r.id == from) {
                    Some(i) => i,
                    None => {
                        g.remotes.push(Remote {
                            id: from,
                            name,
                            pos: Vec3::ZERO,
                            rot: Quat::IDENTITY,
                            net_pos: Vec3::ZERO,
                            net_rot: Quat::IDENTITY,
                            speed: 0.0,
                            alive: false,
                            shielded: false,
                            hp: 100.0,
                            score: 0,
                            kills: 0,
                            deaths: 0,
                            firing: false,
                            throttle: 0.0,
                            missiles: Vec::new(),
                            last_seen: 0.0,
                            gun_t: 0.0,
                            fresh: true,
                        });
                        g.remotes.len() - 1
                    }
                };
                let r = &mut g.remotes[idx];
                let was_alive = r.alive;
                r.net_pos = vec3(num(1), num(2), num(3));
                r.net_rot = Quat::from_xyzw(num(4), num(5), num(6), num(7)).normalize();
                r.speed = num(8);
                let a = id_at(9);
                r.alive = a >= 1;
                r.shielded = a == 2;
                if r.alive && !was_alive {
                    r.fresh = true; // respawned: jump straight there
                }
                r.hp = num(10);
                r.score = num(11) as i32;
                r.kills = num(12) as i32;
                r.deaths = num(13) as i32;
                r.firing = id_at(14) == 1;
                r.throttle = num(15);
                r.last_seen = 0.0;
                r.missiles = parts
                    .filter(|m| !m.is_empty())
                    .filter_map(|m| {
                        let v: Vec<f32> = m.split(',').filter_map(|x| x.parse().ok()).collect();
                        if v.len() == 6 {
                            Some((vec3(v[0], v[1], v[2]), vec3(v[3], v[4], v[5]).normalize_or(Vec3::Z)))
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            "W" => {
                if g.authority {
                    return;
                }
                let wave = num(1) as i32;
                if wave != self.last_wave && wave > 0 {
                    self.last_wave = wave;
                    g.msg(&format!("WAVE {}  -  ENGAGE", wave), Color::new(1.0, 0.9, 0.3, 1.0));
                }
                g.wave = wave.max(1);
                g.wave_kills = num(2) as i32;
                let mut seen_e = Vec::new();
                let mut seen_g = Vec::new();
                g.ext_missiles.clear();
                for e in parts {
                    let v: Vec<&str> = e.split(',').collect();
                    let n = |i: usize| -> f32 { v.get(i).and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.0) };
                    match v.first().copied().unwrap_or("") {
                        "e" if v.len() >= 13 => {
                            let id = n(1) as u32;
                            seen_e.push(id);
                            let pos = vec3(n(3), n(4), n(5));
                            let rot = Quat::from_xyzw(n(6), n(7), n(8), n(9)).normalize();
                            match g.enemies.iter_mut().find(|x| x.id == id) {
                                Some(x) => {
                                    x.net_pos = pos;
                                    x.net_rot = rot;
                                    x.speed = n(10);
                                    x.flash = x.flash.max(n(11));
                                    x.firing = n(12) > 0.5;
                                }
                                None => {
                                    let ace = n(2) > 0.5;
                                    g.spawn_enemy_at(pos, rot * Vec3::Z, ace, Ai::Dummy);
                                    let x = g.enemies.last_mut().unwrap();
                                    x.id = id;
                                    x.rot = rot;
                                    x.net_rot = rot;
                                    x.speed = n(10);
                                }
                            }
                        }
                        "g" if v.len() >= 8 => {
                            let id = n(1) as u32;
                            seen_g.push(id);
                            let kind = match n(2) as i32 {
                                0 => GroundKind::Sam,
                                1 => GroundKind::Tank,
                                _ => GroundKind::Hangar,
                            };
                            let pos = vec3(n(3), n(4), n(5));
                            match g.ground.iter_mut().find(|x| x.id == id) {
                                Some(x) => {
                                    x.pos = pos;
                                    x.kind = kind;
                                    x.yaw = n(6);
                                    x.alive = true;
                                    x.flash = x.flash.max(n(7));
                                }
                                None => g.ground.push(GroundTarget { id, kind, pos, yaw: n(6), hp: 100.0, alive: true, fire_cd: 0.0, flash: 0.0 }),
                            }
                        }
                        "m" if v.len() >= 9 => {
                            g.ext_missiles.push(ExtMissile {
                                pos: vec3(n(1), n(2), n(3)),
                                dir: vec3(n(4), n(5), n(6)).normalize_or(Vec3::Z),
                                speed: n(7),
                                at_me: n(8) as u32 == g.my_id,
                                smoke_t: 0.0,
                            });
                        }
                        _ => {}
                    }
                }
                g.enemies.retain(|e| seen_e.contains(&e.id));
                g.ground.retain(|x| seen_g.contains(&x.id));
            }
            "H" => {
                if g.authority {
                    g.damage_enemy_by(id_at(1), num(2), from);
                }
            }
            "HG" => {
                if g.authority {
                    g.damage_ground_by(id_at(1), num(2), from);
                }
            }
            "K" => {
                // a bot or ground target died (sent by the host)
                let kind = f.get(1).copied().unwrap_or("e");
                let id = id_at(2);
                let killer = id_at(3);
                let pts = num(4) as i32;
                let pos = vec3(num(5), num(6), num(7));
                if !g.authority {
                    g.enemies.retain(|e| !(kind == "e" && e.id == id));
                    if kind == "g" {
                        if let Some(x) = g.ground.iter_mut().find(|x| x.id == id) {
                            x.alive = false;
                        }
                    }
                    g.explosion(pos, if kind == "g" { 1.5 } else { 1.2 });
                }
                if killer == g.my_id && !g.authority {
                    let got = g.add_score(pts);
                    if kind == "g" {
                        g.ground_kills += 1;
                        g.msg(&format!("TARGET DESTROYED  +{}", got), Color::new(0.5, 1.0, 0.5, 1.0));
                    } else {
                        g.kills += 1;
                        g.msg(&format!("SPLASH ONE  +{}", got), Color::new(0.5, 1.0, 0.5, 1.0));
                    }
                } else if killer != g.my_id && !g.authority && kind == "e" {
                    let n = g.remote_name(killer);
                    g.msg(&format!("{} SPLASHED ONE", n), Color::new(0.6, 0.85, 1.0, 1.0));
                }
            }
            "D" => g.damage_player_from(num(1), 0),
            "PH" => {
                if g.mode == MatchMode::Dogfight {
                    g.damage_player_from(num(1), from);
                }
            }
            "PK" => {
                // another pilot went down
                let killer = id_at(1);
                let pos = vec3(num(2), num(3), num(4));
                g.explosion(pos, 1.6);
                let victim = g.remote_name(from);
                if killer == g.my_id && g.my_id != 0 {
                    let got = g.add_score(PVP_KILL_SCORE);
                    g.player_kills += 1;
                    g.msg(&format!("YOU SHOT DOWN {}  +{}", victim, got), Color::new(0.5, 1.0, 0.5, 1.0));
                } else if killer != 0 {
                    let k = g.remote_name(killer);
                    g.msg(&format!("{} SHOT DOWN {}", k, victim), Color::new(1.0, 0.6, 0.9, 1.0));
                } else {
                    g.msg(&format!("{} WENT DOWN", victim), Color::new(0.8, 0.8, 0.8, 1.0));
                }
            }
            "FL" => {
                // a client dropped flares: give missiles chasing them a chance to go for a decoy
                if g.authority {
                    if let Some(r) = g.remotes.iter().find(|r| r.id == from) {
                        let (pos, vel) = (r.pos, r.vel() * 0.6);
                        g.next_id += 1;
                        let fid = g.next_id;
                        g.flares.push(Flare { id: fid, pos, vel, life: 3.0 });
                        for m in g.missiles.iter_mut() {
                            if m.target == Homing::Remote(from) && macroquad::rand::gen_range(0.0, 1.0) < FLARE_DECOY_CHANCE {
                                m.target = Homing::Flare(fid);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// This computer's aircraft (+ its missiles, for the others to draw).
pub fn encode_state(g: &Game) -> String {
    let p = &g.player;
    let alive = if !p.alive { 0 } else if p.shield > 0.0 { 2 } else { 1 };
    let firing = c_firing(g);
    let mut s = format!(
        "P,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        f1(p.pos.x),
        f1(p.pos.y),
        f1(p.pos.z),
        f3(p.rot.x),
        f3(p.rot.y),
        f3(p.rot.z),
        f3(p.rot.w),
        f1(p.speed),
        alive,
        p.hp.round(),
        g.score,
        g.kills + g.player_kills,
        g.deaths,
        if firing { 1 } else { 0 },
        format!("{:.2}", p.throttle)
    );
    for m in g.missiles.iter().filter(|m| m.player).take(8) {
        s.push_str(&format!(";{},{},{},{},{},{}", f1(m.pos.x), f1(m.pos.y), f1(m.pos.z), f3(m.dir.x), f3(m.dir.y), f3(m.dir.z)));
    }
    s
}

fn c_firing(g: &Game) -> bool {
    // the gun fired within the last couple of frames
    g.player.alive && g.player.gun_t > 0.0 && !g.player.overheated
}

/// Host world snapshot: wave, bots, ground targets and enemy missiles (with whom they chase).
pub fn encode_snapshot(g: &Game) -> String {
    let mut s = format!("W,{},{}", g.wave, g.wave_kills);
    for e in g.enemies.iter().filter(|e| e.alive) {
        s.push_str(&format!(
            ";e,{},{},{},{},{},{},{},{},{},{},{:.1},{}",
            e.id,
            if e.ace { 1 } else { 0 },
            f1(e.pos.x),
            f1(e.pos.y),
            f1(e.pos.z),
            f3(e.rot.x),
            f3(e.rot.y),
            f3(e.rot.z),
            f3(e.rot.w),
            f1(e.speed),
            e.flash,
            if e.firing || e.gun_t > 0.0 && e.gun_t < 0.3 { 1 } else { 0 }
        ));
    }
    for x in g.ground.iter().filter(|x| x.alive) {
        let k = match x.kind {
            GroundKind::Sam => 0,
            GroundKind::Tank => 1,
            GroundKind::Hangar => 2,
        };
        s.push_str(&format!(";g,{},{},{},{},{},{:.2},{:.1}", x.id, k, f1(x.pos.x), f1(x.pos.y), f1(x.pos.z), x.yaw, x.flash));
    }
    for m in g.missiles.iter().filter(|m| !m.player) {
        let t = match m.target {
            Homing::Player => g.my_id,
            Homing::Remote(id) => id,
            _ => 0,
        };
        s.push_str(&format!(";m,{},{},{},{},{},{},{},{}", f1(m.pos.x), f1(m.pos.y), f1(m.pos.z), f3(m.dir.x), f3(m.dir.y), f3(m.dir.z), f1(m.speed), t));
    }
    s
}
