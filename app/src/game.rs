//! Core gameplay: arcade flight model, cooldown-based weapons, endless enemy waves, ground targets.

use crate::input::Controls;
use crate::render::{quat_from_fwd_up, rotate_toward};
use crate::world::height_at;
use macroquad::prelude::*;
use macroquad::rand::gen_range;

// ------------------------------------------------------------------ tuning
pub const G: f32 = 9.81;
pub const MIN_SPEED: f32 = 60.0;
pub const STALL_SPEED: f32 = 95.0;
pub const MAX_SPEED: f32 = 470.0;
pub const IDLE_SPEED: f32 = 100.0; // target speed at 0 % throttle
pub const MIL_SPEED: f32 = 330.0; // target speed at 90 % throttle
pub const AB_SPEED: f32 = 420.0; // target speed at 100 % (afterburner)
pub const SPEED_RESPONSE: f32 = 0.35; // 1/s
pub const THROTTLE_RATE: f32 = 0.6; // per second
pub const PITCH_RATE_UP: f32 = 1.35; // rad/s
pub const PITCH_RATE_DOWN: f32 = 0.95;
pub const ROLL_RATE: f32 = 3.3;
pub const YAW_RATE: f32 = 0.35;
pub const BANK_TURN: f32 = 0.55; // auto-coordinated turn from bank (rad/s at 90 deg)
pub const CEILING: f32 = 15000.0;

pub const GUN_INTERVAL: f32 = 1.0 / 18.0;
pub const GUN_SPEED: f32 = 1100.0;
pub const GUN_LIFE: f32 = 1.4;
pub const GUN_DAMAGE: f32 = 8.0;
pub const GUN_SPREAD: f32 = 0.004;
pub const GUN_HEAT_PER_SHOT: f32 = 3.0;
pub const GUN_COOL_RATE: f32 = 32.0;
pub const GUN_OVERHEAT_RESUME: f32 = 35.0;

pub const FLARE_COOLDOWN: f32 = 5.0;
pub const FLARE_DECOY_CHANCE: f32 = 0.75;

pub const PLAYER_HP: f32 = 100.0;
pub const REGEN_DELAY: f32 = 6.0;
pub const REGEN_RATE: f32 = 3.0;
pub const START_LIVES: i32 = 3;
pub const EXTRA_LIFE_EVERY: i32 = 10000;
pub const RESPAWN_DELAY: f32 = 3.0;
pub const RESPAWN_SHIELD: f32 = 3.0;
pub const START_ALT: f32 = 2000.0;

pub const HIT_RADIUS_PLANE: f32 = 9.0;
pub const HIT_RADIUS_GROUND: f32 = 8.0;
pub const PROX_FUSE: f32 = 14.0;

pub struct WeaponSpec {
    pub name: &'static str,
    pub short: &'static str,
    pub cooldown: f32,
    pub lock_range: f32,
    pub lock_cone: f32, // radians half-angle
    pub lock_time: f32,
    pub speed: f32,
    pub turn: f32,
    pub life: f32,
    pub damage: f32,
    pub air: bool,
}

pub const WEAPONS: [WeaponSpec; 3] = [
    WeaponSpec { name: "IR MISSILE", short: "IR", cooldown: 1.2, lock_range: 3500.0, lock_cone: 0.44, lock_time: 0.6, speed: 700.0, turn: 3.5, life: 6.0, damage: 120.0, air: true },
    WeaponSpec { name: "RADAR MISSILE", short: "RDR", cooldown: 4.0, lock_range: 8000.0, lock_cone: 0.70, lock_time: 1.2, speed: 800.0, turn: 2.2, life: 12.0, damage: 120.0, air: true },
    WeaponSpec { name: "GROUND MISSILE", short: "AGM", cooldown: 3.0, lock_range: 6000.0, lock_cone: 0.61, lock_time: 0.8, speed: 520.0, turn: 2.5, life: 14.0, damage: 200.0, air: false },
];

pub struct EnemySpec {
    pub hp: f32,
    pub speed: f32,
    pub turn: f32,
    pub gun_range: f32,
    pub gun_interval: f32,
    pub gun_damage: f32,
    pub gun_spread: f32,
    pub msl_cd: (f32, f32),
    pub msl_range: f32,
    pub score: i32,
}

pub const FIGHTER: EnemySpec = EnemySpec { hp: 40.0, speed: 230.0, turn: 0.95, gun_range: 900.0, gun_interval: 0.125, gun_damage: 3.0, gun_spread: 0.020, msl_cd: (12.0, 18.0), msl_range: 3500.0, score: 100 };
pub const ACE: EnemySpec = EnemySpec { hp: 70.0, speed: 290.0, turn: 1.3, gun_range: 1000.0, gun_interval: 0.1, gun_damage: 4.0, gun_spread: 0.014, msl_cd: (8.0, 12.0), msl_range: 4000.0, score: 250 };
/// Difficulty setting: how the enemy pilots fly and shoot, and how forgiving the player's weapons are.
#[derive(Clone, Copy)]
pub struct Difficulty {
    pub name: &'static str,
    pub blurb: &'static str,
    // enemy pilots
    pub enemy_spread: f32,
    pub enemy_turn: f32,
    pub enemy_speed: f32,
    pub enemy_damage: f32,
    pub enemy_hp: f32,
    pub enemy_msl_cd: f32,
    pub enemy_evade: f32,
    pub enemy_count: i32,
    // player's weapons
    pub lock_time: f32,
    pub lock_cone: f32,
    pub cooldown: f32,
    pub missile_turn: f32,
    pub gun_hit_radius: f32,
    pub gun_assist: f32,
    pub fire_and_forget: bool,
    pub score: f32,
}

pub const DIFFICULTIES: [Difficulty; 3] = [
    Difficulty {
        name: "EASY",
        blurb: "SLOW, POOR-SHOOTING BOTS - FAST LOCKS, SHORT COOLDOWNS, GUN AIM ASSIST",
        enemy_spread: 2.0, enemy_turn: 0.75, enemy_speed: 0.88, enemy_damage: 0.5, enemy_hp: 0.7, enemy_msl_cd: 1.7, enemy_evade: 0.4, enemy_count: -1,
        lock_time: 0.4, lock_cone: 1.5, cooldown: 0.6, missile_turn: 1.4, gun_hit_radius: 15.0, gun_assist: 0.087, fire_and_forget: true, score: 0.5,
    },
    Difficulty {
        name: "NORMAL",
        blurb: "BALANCED BOTS - STANDARD LOCKS AND COOLDOWNS, LIGHT AIM ASSIST",
        enemy_spread: 1.0, enemy_turn: 1.0, enemy_speed: 1.0, enemy_damage: 1.0, enemy_hp: 1.0, enemy_msl_cd: 1.0, enemy_evade: 1.0, enemy_count: 0,
        lock_time: 1.0, lock_cone: 1.0, cooldown: 1.0, missile_turn: 1.0, gun_hit_radius: 9.0, gun_assist: 0.026, fire_and_forget: false, score: 1.0,
    },
    Difficulty {
        name: "HARD",
        blurb: "SHARP, AGGRESSIVE BOTS - SLOWER LOCKS, LONGER COOLDOWNS, NO ASSIST",
        enemy_spread: 0.65, enemy_turn: 1.15, enemy_speed: 1.08, enemy_damage: 1.4, enemy_hp: 1.3, enemy_msl_cd: 0.7, enemy_evade: 1.8, enemy_count: 1,
        lock_time: 1.3, lock_cone: 0.85, cooldown: 1.25, missile_turn: 0.9, gun_hit_radius: 7.0, gun_assist: 0.0, fire_and_forget: false, score: 1.5,
    },
];
pub const DEFAULT_DIFFICULTY: usize = 1;
/// Radar missile is selected when a flight starts.
pub const DEFAULT_WEAPON: usize = 1;

pub const ENEMY_MSL_SPEED: f32 = 560.0;
pub const ENEMY_MSL_TURN: f32 = 2.1;
pub const ENEMY_MSL_DAMAGE: f32 = 35.0;
pub const ENEMY_MSL_LIFE: f32 = 9.0;
pub const SAM_RANGE: f32 = 7000.0;
pub const SAM_MAX_ALT: f32 = 6000.0;
pub const SAM_CD: (f32, f32) = (10.0, 14.0);
pub const SAM_SPEED: f32 = 600.0;
pub const SAM_TURN: f32 = 2.0;
pub const SAM_LIFE: f32 = 12.0;

pub const WAVE_PAUSE: f32 = 4.0;
pub const SPAWN_INTERVAL: f32 = 2.5;
pub const SPAWN_DIST: (f32, f32) = (5000.0, 7000.0);
pub const GROUND_RELOCATE: f32 = 15000.0;

pub fn wave_quota(w: i32) -> i32 {
    4 + 2 * w
}
pub fn wave_concurrent(w: i32) -> usize {
    (2 + w).min(8) as usize
}
pub fn wave_ground_count(w: i32) -> usize {
    (4 + w / 2).min(8) as usize
}
pub fn wave_ace_chance(w: i32) -> f32 {
    if w < 3 {
        0.0
    } else {
        (0.15 * (w - 2) as f32).min(0.6)
    }
}
pub fn wave_speed_mult(w: i32) -> f32 {
    (1.0 + 0.03 * (w - 1) as f32).min(1.3)
}

// ------------------------------------------------------------------ entities

pub struct Player {
    pub pos: Vec3,
    pub rot: Quat,
    pub speed: f32,
    pub throttle: f32,
    pub hp: f32,
    pub alive: bool,
    pub respawn_t: f32,
    pub shield: f32,
    pub since_hit: f32,
    pub gun_t: f32,
    pub gun_heat: f32,
    pub overheated: bool,
    pub weapon: usize,
    pub cooldowns: [f32; 3],
    pub flare_cd: f32,
    pub flares_to_drop: i32,
    pub flare_drop_t: f32,
    pub target: Option<TargetRef>,
    pub lock_t: f32,
    pub locked: bool,
    pub g_load: f32,
    pub stalled: bool,
    pub hit_flash: f32,
    pub shots_fired: u32,
    pub shots_hit: u32,
    pub missiles_fired: u32,
}

impl Player {
    pub fn new() -> Self {
        Player {
            pos: vec3(0.0, START_ALT, -3000.0),
            rot: Quat::IDENTITY,
            speed: 250.0,
            throttle: 0.6,
            hp: PLAYER_HP,
            alive: true,
            respawn_t: 0.0,
            shield: RESPAWN_SHIELD,
            since_hit: 100.0,
            gun_t: 0.0,
            gun_heat: 0.0,
            overheated: false,
            weapon: DEFAULT_WEAPON,
            cooldowns: [0.0; 3],
            flare_cd: 0.0,
            flares_to_drop: 0,
            flare_drop_t: 0.0,
            target: None,
            lock_t: 0.0,
            locked: false,
            g_load: 1.0,
            stalled: false,
            hit_flash: 0.0,
            shots_fired: 0,
            shots_hit: 0,
            missiles_fired: 0,
        }
    }
    pub fn fwd(&self) -> Vec3 {
        self.rot * Vec3::Z
    }
    pub fn up(&self) -> Vec3 {
        self.rot * Vec3::Y
    }
    /// Right wing direction.
    pub fn right(&self) -> Vec3 {
        self.rot * Vec3::NEG_X
    }
    pub fn vel(&self) -> Vec3 {
        self.fwd() * self.speed
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TargetRef {
    Enemy(u32),
    Ground(u32),
    Remote(u32),
}

/// Single player, or an online match.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MatchMode {
    Solo,
    /// Players team up against the endless waves.
    Coop,
    /// Free-for-all: players can shoot each other, bots keep flying too.
    Dogfight,
}

pub const PVP_KILL_SCORE: i32 = 300;

/// Another player in an online match (state received over the network).
pub struct Remote {
    pub id: u32,
    pub name: String,
    pub pos: Vec3,
    pub rot: Quat,
    pub net_pos: Vec3,
    pub net_rot: Quat,
    pub speed: f32,
    pub alive: bool,
    pub shielded: bool,
    pub hp: f32,
    pub score: i32,
    pub kills: i32,
    pub deaths: i32,
    pub firing: bool,
    pub throttle: f32,
    pub missiles: Vec<(Vec3, Vec3)>,
    pub last_seen: f32,
    pub gun_t: f32,
    pub fresh: bool,
}

impl Remote {
    pub fn fwd(&self) -> Vec3 {
        self.rot * Vec3::Z
    }
    pub fn vel(&self) -> Vec3 {
        self.fwd() * self.speed
    }
}

/// Enemy missile seen by a client (simulated by the host).
pub struct ExtMissile {
    pub pos: Vec3,
    pub dir: Vec3,
    pub speed: f32,
    pub at_me: bool,
    pub smoke_t: f32,
}

/// Things the game needs to tell the other players.
#[derive(Clone, Debug)]
pub enum NetOut {
    /// client -> host: my shot hit a bot / ground target
    HitEnemy(u32, f32),
    HitGround(u32, f32),
    /// host -> all: a bot / ground target died (kind 'e' or 'g', id, killer pilot id, base points, where)
    Kill(char, u32, u32, i32, Vec3),
    /// host -> one pilot: bots hurt you
    Damage(u32, f32),
    /// dogfight: I hit another pilot
    HitPlayer(u32, f32),
    /// I was shot down (by pilot id, 0 = bots/terrain)
    Down(u32, Vec3),
    /// client -> host: I dropped flares
    Flares,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Ai {
    Attack,
    Evade,
    Extend,
    Dummy, // flies straight (self-test)
}

pub struct Enemy {
    pub net_pos: Vec3,
    pub net_rot: Quat,
    pub firing: bool,
    pub id: u32,
    pub ace: bool,
    pub pos: Vec3,
    pub rot: Quat,
    pub speed: f32,
    pub hp: f32,
    pub ai: Ai,
    pub ai_t: f32,
    pub evade_dir: Vec3,
    pub gun_t: f32,
    pub msl_cd: f32,
    pub flash: f32,
    pub alive: bool,
}

impl Enemy {
    pub fn fwd(&self) -> Vec3 {
        self.rot * Vec3::Z
    }
    pub fn spec(&self) -> &'static EnemySpec {
        if self.ace {
            &ACE
        } else {
            &FIGHTER
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum GroundKind {
    Sam,
    Tank,
    Hangar,
}

pub struct GroundTarget {
    pub id: u32,
    pub kind: GroundKind,
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: f32,
    pub alive: bool,
    pub fire_cd: f32,
    pub flash: f32,
}

impl GroundTarget {
    pub fn score(&self) -> i32 {
        match self.kind {
            GroundKind::Sam => 150,
            GroundKind::Tank => 75,
            GroundKind::Hangar => 50,
        }
    }
}

pub struct Bullet {
    /// drawn only (another computer decides what it hits)
    pub visual: bool,
    pub pos: Vec3,
    pub prev: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub player: bool,
    pub damage: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Homing {
    None,
    Enemy(u32),
    Ground(u32),
    Player,
    Remote(u32),
    Flare(u32),
}

pub struct Missile {
    pub pos: Vec3,
    pub dir: Vec3,
    pub speed: f32,
    pub max_speed: f32,
    pub turn: f32,
    pub life: f32,
    pub damage: f32,
    pub player: bool,
    pub target: Homing,
    pub smoke_t: f32,
}

pub struct Flare {
    pub id: u32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
}

pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub grow: f32,
    pub col: Color,
    pub col2: Color,
}

pub struct Message {
    pub text: String,
    pub t: f32,
    pub col: Color,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    PlayerHit,
    EnemyKilled,
    GroundKilled,
    MissileLaunch,
    Explosion,
}

pub struct Game {
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub ground: Vec<GroundTarget>,
    pub bullets: Vec<Bullet>,
    pub missiles: Vec<Missile>,
    pub flares: Vec<Flare>,
    pub particles: Vec<Particle>,
    pub messages: Vec<Message>,
    pub events: Vec<Event>,
    pub next_id: u32,
    pub score: i32,
    pub next_life_at: i32,
    pub lives: i32,
    pub wave: i32,
    pub wave_kills: i32,
    pub wave_pause: f32,
    pub spawn_t: f32,
    pub kills: i32,
    pub ground_kills: i32,
    pub time: f32,
    pub game_over: bool,
    pub spawning: bool,
    pub incoming: bool,
    pub cockpit: bool,
    pub diff: usize,
    // ---- online match
    pub mode: MatchMode,
    /// true when this computer runs the bots (single player, or the match host)
    pub authority: bool,
    pub my_id: u32,
    pub remotes: Vec<Remote>,
    pub out: Vec<NetOut>,
    pub ext_missiles: Vec<ExtMissile>,
    pub player_kills: i32,
    pub deaths: i32,
    pub last_attacker: u32,
}

impl Game {
    pub fn new(diff: usize) -> Self {
        let mut g = Game {
            player: Player::new(),
            enemies: Vec::new(),
            ground: Vec::new(),
            bullets: Vec::new(),
            missiles: Vec::new(),
            flares: Vec::new(),
            particles: Vec::new(),
            messages: Vec::new(),
            events: Vec::new(),
            next_id: 1,
            score: 0,
            next_life_at: EXTRA_LIFE_EVERY,
            lives: START_LIVES,
            wave: 1,
            wave_kills: 0,
            wave_pause: 0.0,
            spawn_t: 1.0,
            kills: 0,
            ground_kills: 0,
            time: 0.0,
            game_over: false,
            spawning: true,
            incoming: false,
            cockpit: true,
            diff: diff.min(DIFFICULTIES.len() - 1),
            mode: MatchMode::Solo,
            authority: true,
            my_id: 0,
            remotes: Vec::new(),
            out: Vec::new(),
            ext_missiles: Vec::new(),
            player_kills: 0,
            deaths: 0,
            last_attacker: 0,
        };
        g.msg("WAVE 1  -  ENGAGE", Color::new(1.0, 0.9, 0.3, 1.0));
        g
    }

    /// A new online match. The host runs the bots; everyone else mirrors them.
    pub fn new_match(diff: usize, mode: MatchMode, my_id: u32, host: bool) -> Self {
        let mut g = Game::new(diff);
        g.mode = mode;
        g.my_id = my_id;
        g.authority = host;
        g.messages.clear();
        let txt = match mode {
            MatchMode::Dogfight => "DOGFIGHT  -  EVERYONE IS A TARGET",
            _ => "CO-OP  -  WAVE 1  -  ENGAGE",
        };
        g.msg(txt, Color::new(1.0, 0.9, 0.3, 1.0));
        // spread the players out a little so they don't spawn inside each other
        let k = (my_id % 8) as f32;
        g.player.pos += vec3((k - 3.5) * 150.0, k * 40.0, 0.0);
        if !host {
            g.spawning = false;
        }
        g
    }

    pub fn online(&self) -> bool {
        self.mode != MatchMode::Solo
    }

    pub fn remote_name(&self, id: u32) -> String {
        if id == self.my_id {
            return "YOU".into();
        }
        self.remotes.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| format!("PILOT{}", id))
    }

    pub fn msg(&mut self, t: &str, col: Color) {
        self.messages.push(Message { text: t.to_string(), t: 3.0, col });
        if self.messages.len() > 4 {
            self.messages.remove(0);
        }
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn d(&self) -> &'static Difficulty {
        &DIFFICULTIES[self.diff]
    }

    /// Effective cooldown of a missile type at this difficulty.
    pub fn cooldown_of(&self, w: usize) -> f32 {
        WEAPONS[w].cooldown * self.d().cooldown
    }
    pub fn lock_time_of(&self, w: usize) -> f32 {
        WEAPONS[w].lock_time * self.d().lock_time
    }
    pub fn lock_cone_of(&self, w: usize) -> f32 {
        WEAPONS[w].lock_cone * self.d().lock_cone
    }

    /// Adds points scaled by the difficulty; returns what was added.
    pub fn add_score(&mut self, s: i32) -> i32 {
        let s = (s as f32 * self.d().score).round() as i32;
        self.score += s;
        while self.score >= self.next_life_at {
            self.lives += 1;
            self.next_life_at += EXTRA_LIFE_EVERY;
            self.msg("EXTRA LIFE", Color::new(0.4, 1.0, 0.4, 1.0));
        }
        s
    }

    // -------------------------------------------------------------- spawning

    pub fn spawn_enemy_at(&mut self, pos: Vec3, dir: Vec3, ace: bool, ai: Ai) -> u32 {
        let id = self.id();
        let spec = if ace { &ACE } else { &FIGHTER };
        let d = self.d();
        let speed = spec.speed * wave_speed_mult(self.wave) * d.enemy_speed * gen_range(0.95, 1.05);
        self.enemies.push(Enemy {
            net_pos: pos,
            net_rot: quat_from_fwd_up(dir, Vec3::Y),
            firing: false,
            id,
            ace,
            pos,
            rot: quat_from_fwd_up(dir, Vec3::Y),
            speed,
            hp: spec.hp * d.enemy_hp,
            ai,
            ai_t: 0.0,
            evade_dir: Vec3::Y,
            gun_t: 0.0,
            msl_cd: gen_range(spec.msl_cd.0, spec.msl_cd.1) * 0.6 * d.enemy_msl_cd,
            flash: 0.0,
            alive: true,
        });
        id
    }

    fn spawn_enemy(&mut self) {
        let p = &self.player;
        let ang = gen_range(0.0, std::f32::consts::TAU);
        let d = gen_range(SPAWN_DIST.0, SPAWN_DIST.1);
        let mut pos = p.pos + vec3(ang.cos() * d, 0.0, ang.sin() * d);
        pos.y = (p.pos.y + gen_range(-800.0, 800.0)).clamp(900.0, 9000.0);
        let gh = height_at(pos.x, pos.z);
        if pos.y < gh + 600.0 {
            pos.y = gh + 600.0;
        }
        let dir = (p.pos - pos).normalize();
        let ace = gen_range(0.0, 1.0) < wave_ace_chance(self.wave);
        self.spawn_enemy_at(pos, dir, ace, Ai::Attack);
    }

    fn place_ground(&mut self, idx: Option<usize>) {
        let p = &self.player;
        let hf = vec3(p.fwd().x, 0.0, p.fwd().z).normalize_or(Vec3::Z);
        let mut pos = Vec3::ZERO;
        for _ in 0..20 {
            let ang = gen_range(-1.2f32, 1.2);
            let dir = Quat::from_rotation_y(ang) * hf;
            let d = gen_range(3000.0, 9000.0);
            pos = p.pos + dir * d;
            pos.y = 0.0;
            if height_at(pos.x, pos.z) < 0.5 && height_at(pos.x + 60.0, pos.z + 60.0) < 0.5 {
                break;
            }
        }
        let r = gen_range(0.0, 1.0);
        let kind = if r < 0.35 {
            GroundKind::Sam
        } else if r < 0.75 {
            GroundKind::Tank
        } else {
            GroundKind::Hangar
        };
        let hp = match kind {
            GroundKind::Sam => 60.0,
            GroundKind::Tank => 40.0,
            GroundKind::Hangar => 90.0,
        };
        let id = self.id();
        let t = GroundTarget { id, kind, pos, yaw: gen_range(0.0, 6.28), hp, alive: true, fire_cd: gen_range(SAM_CD.0, SAM_CD.1) * 0.5 * self.d().enemy_msl_cd, flash: 0.0 };
        match idx {
            Some(i) => self.ground[i] = t,
            None => self.ground.push(t),
        }
    }

    fn manage_waves(&mut self, dt: f32) {
        if !self.spawning {
            return;
        }
        // completed wave?
        if self.wave_kills >= wave_quota(self.wave) && self.wave_pause <= 0.0 {
            let bonus = self.add_score(500 * self.wave);
            self.msg(&format!("WAVE {} CLEAR  +{}", self.wave, bonus), Color::new(0.4, 1.0, 0.5, 1.0));
            self.wave += 1;
            self.wave_kills = 0;
            self.wave_pause = WAVE_PAUSE;
        }
        if self.wave_pause > 0.0 {
            self.wave_pause -= dt;
            if self.wave_pause <= 0.0 {
                let w = self.wave;
                self.msg(&format!("WAVE {}  -  ENGAGE", w), Color::new(1.0, 0.9, 0.3, 1.0));
            }
            return;
        }
        // keep the sky busy: respawn up to the concurrent limit
        let alive = self.enemies.iter().filter(|e| e.alive).count();
        self.spawn_t -= dt;
        let limit = (wave_concurrent(self.wave) as i32 + self.d().enemy_count).clamp(1, 10) as usize;
        if alive < limit && self.spawn_t <= 0.0 {
            self.spawn_enemy();
            self.spawn_t = SPAWN_INTERVAL;
        }
        // ground targets follow the player around the endless map
        let want = wave_ground_count(self.wave);
        let alive_g = self.ground.iter().filter(|g| g.alive).count();
        if alive_g < want {
            let dead = self.ground.iter().position(|g| !g.alive);
            self.place_ground(dead);
        }
        let ppos = self.player.pos;
        for i in 0..self.ground.len() {
            let g = &self.ground[i];
            if g.alive && vec2(g.pos.x - ppos.x, g.pos.z - ppos.z).length() > GROUND_RELOCATE {
                self.place_ground(Some(i));
            }
        }
        // enemies that wandered too far are brought back in front
        for e in self.enemies.iter_mut() {
            if e.alive && e.ai != Ai::Dummy && e.pos.distance(ppos) > 16000.0 {
                let ang = gen_range(0.0, std::f32::consts::TAU);
                e.pos = ppos + vec3(ang.cos() * 6000.0, 0.0, ang.sin() * 6000.0);
                e.pos.y = (ppos.y).max(height_at(e.pos.x, e.pos.z) + 800.0);
                e.rot = quat_from_fwd_up(ppos - e.pos, Vec3::Y);
            }
        }
    }

    // -------------------------------------------------------------- update

    pub fn update(&mut self, c: &Controls, dt: f32) {
        self.time += dt;
        self.events.clear();
        if c.view {
            self.cockpit = !self.cockpit;
        }
        self.update_player(c, dt);
        self.update_targeting(c, dt);
        if self.authority {
            self.update_enemies(dt);
            self.update_ground(dt);
        } else {
            self.update_mirrored_world(dt);
        }
        self.update_remotes(dt);
        self.update_bullets(dt);
        self.update_missiles(dt);
        self.update_flares(dt);
        self.update_particles(dt);
        if self.authority {
            self.manage_waves(dt);
        }
        self.enemies.retain(|e| e.alive);
        for m in self.messages.iter_mut() {
            m.t -= dt;
        }
        self.messages.retain(|m| m.t > 0.0);
        self.incoming = self.missiles.iter().any(|m| m.target == Homing::Player) || self.ext_missiles.iter().any(|m| m.at_me);
    }

    /// Client side of an online match: bots and ground targets glide toward the host's latest snapshot.
    fn update_mirrored_world(&mut self, dt: f32) {
        let k = (8.0 * dt).min(1.0);
        let mut tracers = Vec::new();
        for e in self.enemies.iter_mut() {
            e.flash = (e.flash - dt * 4.0).max(0.0);
            let nf = e.net_rot * Vec3::Z;
            e.net_pos += nf * e.speed * dt;
            e.pos = e.pos.lerp(e.net_pos, k) + nf * e.speed * dt * (1.0 - k);
            e.rot = e.rot.slerp(e.net_rot, k).normalize();
            if e.firing {
                e.gun_t -= dt;
                if e.gun_t <= 0.0 {
                    e.gun_t = e.spec().gun_interval;
                    let f = e.fwd();
                    let o = e.pos + f * 8.0;
                    tracers.push(Bullet { visual: true, pos: o, prev: o, vel: f * (GUN_SPEED + e.speed), life: GUN_LIFE, player: false, damage: 0.0 });
                }
            }
        }
        self.bullets.extend(tracers);
        for g in self.ground.iter_mut() {
            g.flash = (g.flash - dt * 4.0).max(0.0);
        }
        let mut smoke = Vec::new();
        for m in self.ext_missiles.iter_mut() {
            m.pos += m.dir * m.speed * dt;
            m.smoke_t -= dt;
            if m.smoke_t <= 0.0 {
                m.smoke_t = 0.04;
                smoke.push(m.pos - m.dir * 2.0);
            }
        }
        for s in smoke {
            self.smoke_puff(s);
        }
    }

    /// Other players: smooth their reported movement, show their tracers and missile smoke.
    fn update_remotes(&mut self, dt: f32) {
        let k = (10.0 * dt).min(1.0);
        let mut tracers = Vec::new();
        let mut smoke = Vec::new();
        for r in self.remotes.iter_mut() {
            r.last_seen += dt;
            let nf = r.net_rot * Vec3::Z;
            r.net_pos += nf * r.speed * dt;
            if r.fresh {
                r.pos = r.net_pos;
                r.rot = r.net_rot;
                r.fresh = false;
            }
            r.pos = r.pos.lerp(r.net_pos, k) + nf * r.speed * dt * (1.0 - k);
            r.rot = r.rot.slerp(r.net_rot, k).normalize();
            if r.alive && r.firing {
                r.gun_t -= dt;
                if r.gun_t <= 0.0 {
                    r.gun_t = GUN_INTERVAL * 1.5;
                    let f = r.fwd();
                    let o = r.pos + f * 7.0;
                    tracers.push(Bullet { visual: true, pos: o, prev: o, vel: f * (GUN_SPEED + r.speed), life: GUN_LIFE, player: true, damage: 0.0 });
                }
            }
            for (mp, md) in r.missiles.iter_mut() {
                *mp += *md * 700.0 * dt;
                if gen_range(0.0, 1.0) < 0.5 {
                    smoke.push(*mp - *md * 2.0);
                }
            }
        }
        // drop players we haven't heard from for a while
        self.remotes.retain(|r| r.last_seen < 10.0);
        self.bullets.extend(tracers);
        for s in smoke {
            self.smoke_puff(s);
        }
    }

    fn smoke_puff(&mut self, at: Vec3) {
        self.particles.push(Particle {
            pos: at,
            vel: vec3(gen_range(-2.0, 2.0), gen_range(-1.0, 3.0), gen_range(-2.0, 2.0)),
            life: 2.0,
            max_life: 2.0,
            size: 2.0,
            grow: 3.0,
            col: Color::new(0.95, 0.95, 0.95, 0.85),
            col2: Color::new(0.75, 0.75, 0.78, 0.0),
        });
    }

    fn update_player(&mut self, c: &Controls, dt: f32) {
        if !self.player.alive {
            self.player.respawn_t -= dt;
            if self.player.respawn_t <= 0.0 && !self.game_over {
                if self.lives > 0 || self.online() {
                    let old = self.player.pos;
                    let heading = self.player.fwd();
                    self.player = Player::new();
                    self.player.pos = vec3(old.x, START_ALT.max(height_at(old.x, old.z) + 1500.0), old.z);
                    self.player.rot = quat_from_fwd_up(vec3(heading.x, 0.0, heading.z).normalize_or(Vec3::Z), Vec3::Y);
                    // clear threats on respawn
                    self.missiles.retain(|m| m.player);
                    self.msg("READY", Color::new(0.5, 1.0, 0.6, 1.0));
                } else {
                    self.game_over = true;
                }
            }
            return;
        }
        let p = &mut self.player;
        p.shield = (p.shield - dt).max(0.0);
        p.hit_flash = (p.hit_flash - dt * 2.0).max(0.0);
        p.since_hit += dt;
        if p.since_hit > REGEN_DELAY {
            p.hp = (p.hp + REGEN_RATE * dt).min(PLAYER_HP);
        }

        // throttle
        p.throttle = (p.throttle + c.throttle_delta * THROTTLE_RATE * dt).clamp(0.0, 1.0);
        if let Some(t) = c.throttle_set {
            p.throttle = t;
        }
        let target_speed = if p.throttle <= 0.9 {
            IDLE_SPEED + (MIL_SPEED - IDLE_SPEED) * (p.throttle / 0.9)
        } else {
            MIL_SPEED + (AB_SPEED - MIL_SPEED) * ((p.throttle - 0.9) / 0.1)
        };
        let fwd = p.fwd();
        p.speed += (target_speed - p.speed) * SPEED_RESPONSE * dt;
        p.speed -= G * fwd.y * dt * 0.9; // climbing bleeds speed, diving gains it
        p.speed = p.speed.clamp(MIN_SPEED, MAX_SPEED);

        // control authority falls off at low speed
        let auth = ((p.speed - 50.0) / 130.0).clamp(0.25, 1.0) * if p.speed > 380.0 { 0.85 } else { 1.0 };
        let pitch_rate = if c.pitch >= 0.0 { PITCH_RATE_UP } else { PITCH_RATE_DOWN };
        let pr = c.pitch * pitch_rate * auth;
        let rr = c.roll * ROLL_RATE * auth.max(0.5);
        let yr = c.yaw * YAW_RATE * auth;
        p.rot = (p.rot * Quat::from_rotation_x(-pr * dt) * Quat::from_rotation_z(rr * dt) * Quat::from_rotation_y(-yr * dt)).normalize();
        // bank produces a gentle turn (arcade coordination)
        let right = p.right();
        p.rot = (Quat::from_rotation_y(right.y * BANK_TURN * auth * dt) * p.rot).normalize();
        p.g_load = 1.0 + pr * p.speed / G;

        // stall: nose drops toward the ground
        p.stalled = p.speed < STALL_SPEED;
        if p.stalled {
            let f = p.fwd();
            let nf = rotate_toward(f, -Vec3::Y, 0.6 * dt);
            p.rot = quat_from_fwd_up(nf, p.up());
        }
        // above the ceiling the nose is pushed down
        if p.pos.y > CEILING {
            let f = p.fwd();
            if f.y > 0.0 {
                let nf = rotate_toward(f, vec3(f.x, 0.0, f.z), 0.8 * dt);
                p.rot = quat_from_fwd_up(nf, p.up());
            }
        }

        p.pos += p.fwd() * p.speed * dt;

        // terrain collision
        let gh = height_at(p.pos.x, p.pos.z);
        if p.pos.y < gh + 3.0 {
            p.pos.y = gh + 3.0;
            self.last_attacker = 0;
            self.kill_player("YOU CRASHED");
            return;
        }

        // gun
        let p = &mut self.player;
        p.gun_t -= dt;
        if p.overheated && p.gun_heat < GUN_OVERHEAT_RESUME {
            p.overheated = false;
        }
        let mut fire_gun = false;
        if c.gun && !p.overheated {
            if p.gun_t <= 0.0 {
                p.gun_t += GUN_INTERVAL;
                p.gun_heat += GUN_HEAT_PER_SHOT;
                if p.gun_heat >= 100.0 {
                    p.gun_heat = 100.0;
                    p.overheated = true;
                }
                fire_gun = true;
            }
        } else {
            p.gun_heat = (p.gun_heat - GUN_COOL_RATE * dt).max(0.0);
            p.gun_t = p.gun_t.max(0.0);
        }
        if fire_gun {
            let assist = self.d().gun_assist;
            let p = &self.player;
            let side = if self.player.shots_fired % 2 == 0 { 1.2 } else { -1.2 };
            let origin = p.pos + p.fwd() * 6.0 + p.right() * side - p.up() * 0.5;
            // aim assist: bend the stream toward the lead point of a nearby enemy close to the nose
            let mut aim = p.fwd();
            if assist > 0.0 {
                let mut best = assist;
                for e in self.enemies.iter().filter(|e| e.alive) {
                    let dist = e.pos.distance(origin);
                    if dist > GUN_SPEED * GUN_LIFE {
                        continue;
                    }
                    let t = dist / (GUN_SPEED + p.speed * 0.5);
                    let lead = e.pos + e.fwd() * e.speed * t;
                    let want = ((lead - origin) / t.max(0.01) - p.vel()).normalize_or(p.fwd());
                    let ang = p.fwd().dot(want).clamp(-1.0, 1.0).acos();
                    if ang < best {
                        best = ang;
                        aim = want;
                    }
                }
            }
            let spread = vec3(gen_range(-GUN_SPREAD, GUN_SPREAD), gen_range(-GUN_SPREAD, GUN_SPREAD), 0.0);
            let dir = (aim + p.rot * spread).normalize();
            let vel = dir * GUN_SPEED + p.vel();
            self.bullets.push(Bullet { visual: false, pos: origin, prev: origin, vel, life: GUN_LIFE, player: true, damage: GUN_DAMAGE });
            self.player.shots_fired += 1;
        }

        // weapon select
        let p = &mut self.player;
        if c.next_weapon {
            p.weapon = (p.weapon + 1) % WEAPONS.len();
            p.lock_t = 0.0;
            p.locked = false;
            p.target = None;
        }
        if let Some(w) = c.select_weapon {
            if w != p.weapon {
                p.weapon = w;
                p.lock_t = 0.0;
                p.locked = false;
                p.target = None;
            }
        }
        for cd in p.cooldowns.iter_mut() {
            *cd = (*cd - dt).max(0.0);
        }

        // missile launch
        if c.missile {
            self.try_fire_missile();
        }

        // flares
        let p = &mut self.player;
        p.flare_cd = (p.flare_cd - dt).max(0.0);
        if c.flare && p.flare_cd <= 0.0 {
            p.flare_cd = FLARE_COOLDOWN;
            p.flares_to_drop = 6;
            p.flare_drop_t = 0.0;
        }
        if p.flares_to_drop > 0 {
            p.flare_drop_t -= dt;
            if p.flare_drop_t <= 0.0 {
                p.flare_drop_t = 0.08;
                p.flares_to_drop -= 1;
                let side = if p.flares_to_drop % 2 == 0 { 1.0 } else { -1.0 };
                let vel = p.vel() * 0.6 + p.right() * side * 40.0 - p.up() * 30.0;
                let pos = p.pos - p.fwd() * 6.0;
                let id = self.id();
                self.flares.push(Flare { id, pos, vel, life: 3.0 });
                if !self.authority && self.player.flares_to_drop == 5 {
                    self.out.push(NetOut::Flares);
                }
                // decoy missiles chasing the player
                for m in self.missiles.iter_mut() {
                    if m.target == Homing::Player && gen_range(0.0, 1.0) < FLARE_DECOY_CHANCE / 3.0 {
                        m.target = Homing::Flare(id);
                    }
                }
            }
        }

        // engine smoke / afterburner glow particles (chase view)
        let p = &self.player;
        if p.throttle > 0.9 && gen_range(0.0, 1.0) < 0.5 {
            let back = p.pos - p.fwd() * 8.0;
            self.particles.push(Particle {
                pos: back,
                vel: p.vel() * 0.85,
                life: 0.15,
                max_life: 0.15,
                size: 1.4,
                grow: -4.0,
                col: Color::new(1.0, 0.7, 0.3, 1.0),
                col2: Color::new(1.0, 0.3, 0.1, 0.0),
            });
        }
    }

    /// Fires the selected missile if its cooldown is over. Returns true when launched.
    pub fn try_fire_missile(&mut self) -> bool {
        let p = &self.player;
        if !p.alive {
            return false;
        }
        let w = p.weapon;
        if p.cooldowns[w] > 0.0 {
            return false;
        }
        let spec = &WEAPONS[w];
        let d = self.d();
        // Easy: fire-and-forget - a missile follows the selected target even before full lock.
        let target = if p.locked || (d.fire_and_forget && p.lock_t > 0.0) {
            match p.target {
                Some(TargetRef::Enemy(id)) => Homing::Enemy(id),
                Some(TargetRef::Ground(id)) => Homing::Ground(id),
                Some(TargetRef::Remote(id)) => Homing::Remote(id),
                None => Homing::None,
            }
        } else {
            Homing::None
        };
        let side = if p.missiles_fired % 2 == 0 { 3.0 } else { -3.0 };
        let pos = p.pos + p.right() * side - p.up() * 1.0;
        let dir = p.fwd();
        let speed = p.speed;
        self.missiles.push(Missile {
            pos,
            dir,
            speed,
            max_speed: spec.speed + p.speed * 0.5,
            turn: spec.turn * d.missile_turn,
            life: spec.life,
            damage: spec.damage,
            player: true,
            target,
            smoke_t: 0.0,
        });
        let p = &mut self.player;
        p.cooldowns[w] = spec.cooldown * d.cooldown;
        p.missiles_fired += 1;
        self.events.push(Event::MissileLaunch);
        true
    }

    fn update_targeting(&mut self, c: &Controls, dt: f32) {
        if !self.player.alive {
            return;
        }
        let p = &self.player;
        let spec = &WEAPONS[p.weapon];
        let lock_cone = self.lock_cone_of(p.weapon);
        let lock_time = self.lock_time_of(p.weapon);
        let fwd = p.fwd();
        let ppos = p.pos;
        // candidates in range, sorted by angle off the nose
        let mut cands: Vec<(TargetRef, f32, f32)> = Vec::new();
        if spec.air {
            for e in &self.enemies {
                let to = e.pos - ppos;
                let d = to.length();
                let ang = fwd.dot(to / d).clamp(-1.0, 1.0).acos();
                if d < spec.lock_range * 1.6 {
                    cands.push((TargetRef::Enemy(e.id), ang, d));
                }
            }
            if self.mode == MatchMode::Dogfight {
                for r in self.remotes.iter().filter(|r| r.alive) {
                    let to = r.pos - ppos;
                    let d = to.length().max(0.1);
                    let ang = fwd.dot(to / d).clamp(-1.0, 1.0).acos();
                    if d < spec.lock_range * 1.6 {
                        cands.push((TargetRef::Remote(r.id), ang, d));
                    }
                }
            }
        } else {
            for g in self.ground.iter().filter(|g| g.alive) {
                let to = g.pos - ppos;
                let d = to.length();
                let ang = fwd.dot(to / d).clamp(-1.0, 1.0).acos();
                if d < spec.lock_range * 1.6 {
                    cands.push((TargetRef::Ground(g.id), ang, d));
                }
            }
        }
        cands.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let current = self.player.target.and_then(|t| cands.iter().find(|c| c.0 == t).copied());
        let mut new_target = current.map(|c| c.0);
        if c.next_target && !cands.is_empty() {
            let i = current.and_then(|cur| cands.iter().position(|c| c.0 == cur.0)).map(|i| (i + 1) % cands.len()).unwrap_or(0);
            new_target = Some(cands[i].0);
        }
        // auto-select the target closest to the nose when we have none (or it left the cone)
        let keep = current.map(|c| c.1 < lock_cone * 2.0).unwrap_or(false);
        if (new_target.is_none() || !keep) && !c.next_target {
            new_target = cands.first().filter(|c| c.1 < lock_cone * 1.5).map(|c| c.0).or(if keep { new_target } else { None });
        }
        let p = &mut self.player;
        if new_target != p.target {
            p.target = new_target;
            p.lock_t = 0.0;
            p.locked = false;
        }
        // lock-on progress
        if let Some(t) = p.target {
            let info = cands.iter().find(|c| c.0 == t).copied();
            if let Some((_, ang, d)) = info {
                if ang < lock_cone && d < spec.lock_range {
                    p.lock_t += dt;
                } else {
                    p.lock_t = (p.lock_t - dt * 2.0).max(0.0);
                }
                p.locked = p.lock_t >= lock_time;
            }
        } else {
            p.lock_t = 0.0;
            p.locked = false;
        }
    }

    pub fn target_pos(&self, t: TargetRef) -> Option<Vec3> {
        match t {
            TargetRef::Enemy(id) => self.enemies.iter().find(|e| e.id == id && e.alive).map(|e| e.pos),
            TargetRef::Ground(id) => self.ground.iter().find(|g| g.id == id && g.alive).map(|g| g.pos + Vec3::Y * 3.0),
            TargetRef::Remote(id) => self.remotes.iter().find(|r| r.id == id && r.alive).map(|r| r.pos),
        }
    }

    /// Everyone the bots can hunt: (pilot id, position, velocity, forward, can be hurt). Id 0 = this computer's player.
    fn pilots(&self) -> Vec<(u32, Vec3, Vec3, Vec3, bool)> {
        let p = &self.player;
        let mut v = vec![(0, p.pos, p.vel(), p.fwd(), p.alive && p.shield <= 0.0)];
        for r in self.remotes.iter().filter(|r| r.alive) {
            v.push((r.id, r.pos, r.vel(), r.fwd(), !r.shielded));
        }
        v
    }

    fn update_enemies(&mut self, dt: f32) {
        let pilots = self.pilots();
        let n = self.enemies.len();
        let positions: Vec<Vec3> = self.enemies.iter().map(|e| e.pos).collect();
        let mut new_bullets = Vec::new();
        let mut new_missiles = Vec::new();
        let wave = self.wave;
        let dd = *self.d();
        for i in 0..n {
            let e = &mut self.enemies[i];
            if !e.alive {
                continue;
            }
            let spec = e.spec();
            e.flash = (e.flash - dt * 4.0).max(0.0);
            e.ai_t -= dt;
            e.firing = false;
            // hunt the nearest living pilot
            let tgt = pilots
                .iter()
                .filter(|p| p.0 != 0 || self.player.alive)
                .min_by(|a, b| a.1.distance(e.pos).partial_cmp(&b.1.distance(e.pos)).unwrap())
                .copied()
                .unwrap_or((0, self.player.pos, self.player.vel(), self.player.fwd(), false));
            let (tid, ppos, pvel, pf, vulnerable) = tgt;
            let palive = tid != 0 || self.player.alive;
            let fwd = e.fwd();
            let to_p = ppos - e.pos;
            let dist = to_p.length();
            let mut desired = fwd;
            let turn = spec.turn * (1.0 + 0.04 * (wave - 1) as f32).min(1.35) * dd.enemy_turn;
            match e.ai {
                Ai::Dummy => {}
                Ai::Attack => {
                    if palive {
                        let t = (dist / (900.0 + e.speed)).min(3.0);
                        desired = (ppos + pvel * t * 0.7 - e.pos).normalize_or(fwd);
                        // player on our tail -> break away
                        if dist < 1300.0 && fwd.dot(to_p / dist) < -0.3 && pf.dot(-to_p / dist) > 0.85 && e.ai_t <= 0.0 && gen_range(0.0, 1.0) < 0.02 * dd.enemy_evade {
                            e.ai = Ai::Evade;
                            e.ai_t = gen_range(2.0, 4.0);
                            let side = fwd.cross(Vec3::Y).normalize_or(Vec3::X) * if gen_range(0.0, 1.0) < 0.5 { 1.0 } else { -1.0 };
                            e.evade_dir = (side + Vec3::Y * gen_range(-0.6, 0.8)).normalize();
                        }
                        // too close head-on -> extend and come back
                        if dist < 250.0 && e.ai_t <= 0.0 {
                            e.ai = Ai::Extend;
                            e.ai_t = gen_range(2.5, 4.0);
                        }
                    } else {
                        desired = (vec3(fwd.x, 0.0, fwd.z)).normalize_or(fwd);
                    }
                }
                Ai::Evade => {
                    desired = rotate_toward(fwd, e.evade_dir, 1.0);
                    if e.ai_t <= 0.0 {
                        e.ai = Ai::Attack;
                        e.ai_t = 2.0;
                    }
                }
                Ai::Extend => {
                    desired = (-to_p.normalize_or(fwd) + fwd * 1.5).normalize_or(fwd);
                    if e.ai_t <= 0.0 {
                        e.ai = Ai::Attack;
                        e.ai_t = 1.0;
                    }
                }
            }
            if e.ai != Ai::Dummy {
                // separation from other enemies
                for (j, op) in positions.iter().enumerate() {
                    if j != i {
                        let d = e.pos - *op;
                        let l = d.length();
                        if l < 200.0 && l > 0.1 {
                            desired += d / l * (1.0 - l / 200.0) * 0.8;
                        }
                    }
                }
                // terrain avoidance
                let ahead = e.pos + fwd * e.speed * 2.5;
                let clearance = e.pos.y - height_at(e.pos.x, e.pos.z).max(height_at(ahead.x, ahead.z));
                if clearance < 500.0 {
                    desired = (desired.normalize_or(fwd) + Vec3::Y * (2.0 - clearance / 250.0)).normalize();
                }
                if e.pos.y > 11000.0 {
                    desired.y = desired.y.min(-0.1);
                }
            }
            let desired = desired.normalize_or(fwd);
            let new_fwd = rotate_toward(fwd, desired, turn * dt);
            // bank into the turn
            let turn_vec = (new_fwd - fwd) / dt.max(1e-4);
            let up_target = (Vec3::Y + turn_vec * 1.8 / turn.max(0.1)).normalize_or(Vec3::Y);
            let cur_up = e.rot * Vec3::Y;
            let up = cur_up.lerp(up_target, (4.0 * dt).min(1.0));
            e.rot = quat_from_fwd_up(new_fwd, up);
            e.pos += new_fwd * e.speed * dt;
            let gh = height_at(e.pos.x, e.pos.z);
            if e.pos.y < gh + 5.0 {
                e.pos.y = gh + 5.0; // never crash on their own (keeps waves fair)
            }

            e.net_pos = e.pos;
            e.net_rot = e.rot;
            if !palive || e.ai == Ai::Dummy || !vulnerable {
                continue;
            }
            // guns
            e.gun_t -= dt;
            let lead_t = dist / GUN_SPEED;
            let aim = (ppos + pvel * lead_t - e.pos).normalize_or(fwd);
            let ang = new_fwd.dot(aim).clamp(-1.0, 1.0).acos();
            let acc = (1.0 - 0.05 * (wave - 1) as f32).max(0.6);
            if dist < spec.gun_range && ang < 0.11 && e.gun_t <= 0.0 {
                e.gun_t = spec.gun_interval;
                e.firing = true;
                let s = spec.gun_spread * acc * dd.enemy_spread;
                let dir = (aim + vec3(gen_range(-s, s), gen_range(-s, s), gen_range(-s, s))).normalize();
                let origin = e.pos + new_fwd * 8.0;
                new_bullets.push(Bullet { visual: false, pos: origin, prev: origin, vel: dir * GUN_SPEED + new_fwd * e.speed, life: GUN_LIFE, player: false, damage: spec.gun_damage * dd.enemy_damage });
            }
            // missiles
            e.msl_cd -= dt;
            if e.msl_cd <= 0.0 && dist < spec.msl_range && dist > 600.0 && new_fwd.dot(to_p / dist) > 0.85 {
                e.msl_cd = gen_range(spec.msl_cd.0, spec.msl_cd.1) * dd.enemy_msl_cd;
                new_missiles.push(Missile {
                    pos: e.pos + new_fwd * 4.0 - (e.rot * Vec3::Y) * 1.5,
                    dir: new_fwd,
                    speed: e.speed,
                    max_speed: ENEMY_MSL_SPEED,
                    turn: ENEMY_MSL_TURN * dd.enemy_turn,
                    life: ENEMY_MSL_LIFE,
                    damage: ENEMY_MSL_DAMAGE * dd.enemy_damage,
                    player: false,
                    target: if tid == 0 { Homing::Player } else { Homing::Remote(tid) },
                    smoke_t: 0.0,
                });
            }
        }
        self.bullets.extend(new_bullets);
        if !new_missiles.is_empty() {
            self.msg("MISSILE LAUNCH!", Color::new(1.0, 0.3, 0.2, 1.0));
        }
        self.missiles.extend(new_missiles);
    }

    fn update_ground(&mut self, dt: f32) {
        let pilots: Vec<(u32, Vec3, Vec3, Vec3, bool)> = self.pilots().into_iter().filter(|p| p.4).collect();
        let mut launches = Vec::new();
        let dd = *self.d();
        for g in self.ground.iter_mut().filter(|g| g.alive) {
            g.flash = (g.flash - dt * 4.0).max(0.0);
            if g.kind != GroundKind::Sam || pilots.is_empty() {
                continue;
            }
            let gp = g.pos;
            let near = pilots.iter().min_by(|a, b| a.1.distance(gp).partial_cmp(&b.1.distance(gp)).unwrap()).unwrap();
            let (tid, ppos) = (near.0, near.1);
            g.fire_cd -= dt;
            let d = vec2(ppos.x - g.pos.x, ppos.z - g.pos.z).length();
            if g.fire_cd <= 0.0 && d < SAM_RANGE && ppos.y < SAM_MAX_ALT {
                g.fire_cd = gen_range(SAM_CD.0, SAM_CD.1) * dd.enemy_msl_cd;
                let dir = (ppos - g.pos).normalize();
                let up_dir = (dir + Vec3::Y * 2.0).normalize();
                launches.push(Missile {
                    pos: g.pos + Vec3::Y * 6.0,
                    dir: up_dir,
                    speed: 80.0,
                    max_speed: SAM_SPEED,
                    turn: SAM_TURN * dd.enemy_turn,
                    life: SAM_LIFE,
                    damage: ENEMY_MSL_DAMAGE * dd.enemy_damage,
                    player: false,
                    target: if tid == 0 { Homing::Player } else { Homing::Remote(tid) },
                    smoke_t: 0.0,
                });
            }
        }
        if !launches.is_empty() {
            self.msg("SAM LAUNCH!", Color::new(1.0, 0.3, 0.2, 1.0));
        }
        self.missiles.extend(launches);
    }

    fn update_bullets(&mut self, dt: f32) {
        let mut hits_enemy: Vec<(u32, f32, Vec3)> = Vec::new();
        let mut hits_ground: Vec<(u32, f32, Vec3)> = Vec::new();
        let mut hits_player: Vec<(f32, Vec3)> = Vec::new();
        let mut hits_remote: Vec<(u32, f32, Vec3)> = Vec::new();
        let ppos = self.player.pos;
        let palive = self.player.alive;
        let hit_r = self.d().gun_hit_radius;
        let dogfight = self.mode == MatchMode::Dogfight;
        let authority = self.authority;
        for b in self.bullets.iter_mut() {
            b.prev = b.pos;
            b.pos += b.vel * dt;
            b.vel.y -= G * dt * 0.3;
            b.life -= dt;
            let seg = b.pos - b.prev;
            let seg_len2 = seg.length_squared().max(1e-6);
            let closest = |c: Vec3| -> f32 {
                let t = ((c - b.prev).dot(seg) / seg_len2).clamp(0.0, 1.0);
                (b.prev + seg * t).distance(c)
            };
            if b.visual {
                // drawn only
            } else if b.player {
                if dogfight {
                    for r in self.remotes.iter().filter(|r| r.alive && !r.shielded) {
                        if closest(r.pos) < hit_r {
                            hits_remote.push((r.id, b.damage, b.pos));
                            b.life = 0.0;
                            break;
                        }
                    }
                }
                let mut hit_enemy = None;
                if b.life > 0.0 {
                    hit_enemy = self.enemies.iter().filter(|e| e.alive).find(|e| closest(e.pos) < hit_r).map(|e| e.id);
                }
                if let Some(id) = hit_enemy {
                    hits_enemy.push((id, b.damage, b.pos));
                    b.life = 0.0;
                }
                if b.life > 0.0 {
                    for g in self.ground.iter().filter(|g| g.alive) {
                        if closest(g.pos + Vec3::Y * 2.0) < HIT_RADIUS_GROUND * hit_r / HIT_RADIUS_PLANE {
                            hits_ground.push((g.id, b.damage, b.pos));
                            b.life = 0.0;
                            break;
                        }
                    }
                }
            } else if palive && closest(ppos) < HIT_RADIUS_PLANE {
                hits_player.push((b.damage, b.pos));
                b.life = 0.0;
            } else if authority {
                // the host decides when bots hit the other pilots
                for r in self.remotes.iter().filter(|r| r.alive && !r.shielded) {
                    if closest(r.pos) < HIT_RADIUS_PLANE {
                        self.out.push(NetOut::Damage(r.id, b.damage));
                        b.life = 0.0;
                        break;
                    }
                }
            }
            if b.life > 0.0 && b.pos.y < height_at(b.pos.x, b.pos.z) {
                b.life = 0.0;
            }
        }
        self.bullets.retain(|b| b.life > 0.0);
        for (id, dmg, at) in hits_enemy {
            self.player.shots_hit += 1;
            self.spark(at);
            self.damage_enemy(id, dmg);
        }
        for (id, dmg, at) in hits_ground {
            self.player.shots_hit += 1;
            self.spark(at);
            self.damage_ground(id, dmg);
        }
        for (dmg, _) in hits_player {
            self.damage_player_from(dmg, 0);
        }
        for (id, dmg, at) in hits_remote {
            self.player.shots_hit += 1;
            self.spark(at);
            self.out.push(NetOut::HitPlayer(id, dmg));
        }
    }

    fn update_missiles(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let palive = self.player.alive;
        let mut detonate: Vec<(usize, Vec3)> = Vec::new();
        let mut smoke: Vec<Vec3> = Vec::new();
        for (i, m) in self.missiles.iter_mut().enumerate() {
            m.life -= dt;
            m.speed = (m.speed + 400.0 * dt).min(m.max_speed);
            let tpos = match m.target {
                Homing::Enemy(id) => self.enemies.iter().find(|e| e.id == id && e.alive).map(|e| (e.pos, e.fwd() * e.speed)),
                Homing::Ground(id) => self.ground.iter().find(|g| g.id == id && g.alive).map(|g| (g.pos + Vec3::Y * 2.0, Vec3::ZERO)),
                Homing::Player => {
                    if palive {
                        Some((ppos, self.player.vel()))
                    } else {
                        None
                    }
                }
                Homing::Remote(id) => self.remotes.iter().find(|r| r.id == id && r.alive).map(|r| (r.pos, r.vel())),
                Homing::Flare(id) => self.flares.iter().find(|f| f.id == id).map(|f| (f.pos, f.vel)),
                Homing::None => None,
            };
            if let Some((tp, tv)) = tpos {
                let to = tp - m.pos;
                let d = to.length();
                let lead = (d / m.speed.max(1.0)).min(2.0);
                let aim = (tp + tv * lead * 0.8 - m.pos).normalize_or(m.dir);
                // seeker gimbal limit: targets far off-boresight are lost
                if m.dir.dot(to / d.max(0.1)) < 0.3 && d > 200.0 {
                    m.target = Homing::None;
                } else {
                    m.dir = rotate_toward(m.dir, aim, m.turn * dt);
                }
                if d < PROX_FUSE {
                    detonate.push((i, m.pos));
                    continue;
                }
            } else if !m.player {
                m.target = Homing::None;
            }
            m.pos += m.dir * m.speed * dt;
            if m.pos.y < height_at(m.pos.x, m.pos.z) {
                detonate.push((i, m.pos));
                continue;
            }
            // proximity against everything for dumb-fired missiles
            if m.player && m.target == Homing::None {
                let rival_near = self.mode == MatchMode::Dogfight && self.remotes.iter().any(|r| r.alive && r.pos.distance(m.pos) < PROX_FUSE * 1.5);
                if rival_near || self.enemies.iter().any(|e| e.alive && e.pos.distance(m.pos) < PROX_FUSE * 1.5) || self.ground.iter().any(|g| g.alive && g.pos.distance(m.pos) < PROX_FUSE * 1.5) {
                    detonate.push((i, m.pos));
                    continue;
                }
            }
            if m.life <= 0.0 {
                detonate.push((i, m.pos));
                continue;
            }
            m.smoke_t -= dt;
            if m.smoke_t <= 0.0 {
                m.smoke_t = 0.03;
                smoke.push(m.pos - m.dir * 2.0);
            }
        }
        for s in smoke {
            self.particles.push(Particle {
                pos: s,
                vel: vec3(gen_range(-2.0, 2.0), gen_range(-1.0, 3.0), gen_range(-2.0, 2.0)),
                life: 2.2,
                max_life: 2.2,
                size: 2.0,
                grow: 3.0,
                col: Color::new(0.95, 0.95, 0.95, 0.85),
                col2: Color::new(0.75, 0.75, 0.78, 0.0),
            });
        }
        // resolve detonations (highest index first so removal is stable)
        detonate.sort_by(|a, b| b.0.cmp(&a.0));
        detonate.dedup_by_key(|d| d.0);
        for (i, at) in detonate {
            let m = self.missiles.remove(i);
            self.explosion(at, 0.6);
            let radius = 30.0;
            if m.player {
                let ids: Vec<(u32, f32)> = self.enemies.iter().filter(|e| e.alive && e.pos.distance(at) < radius).map(|e| (e.id, m.damage * (1.0 - e.pos.distance(at) / radius * 0.5))).collect();
                for (id, d) in ids {
                    self.damage_enemy(id, d);
                }
                let gids: Vec<(u32, f32)> = self.ground.iter().filter(|g| g.alive && g.pos.distance(at) < radius).map(|g| (g.id, m.damage)).collect();
                for (id, d) in gids {
                    self.damage_ground(id, d);
                }
                if self.mode == MatchMode::Dogfight {
                    let rids: Vec<u32> = self.remotes.iter().filter(|r| r.alive && !r.shielded && r.pos.distance(at) < radius).map(|r| r.id).collect();
                    for id in rids {
                        self.out.push(NetOut::HitPlayer(id, m.damage));
                    }
                }
            } else {
                if self.player.alive && self.player.pos.distance(at) < radius {
                    self.damage_player_from(m.damage, 0);
                }
                if self.authority {
                    let rids: Vec<u32> = self.remotes.iter().filter(|r| r.alive && !r.shielded && r.pos.distance(at) < radius).map(|r| r.id).collect();
                    for id in rids {
                        self.out.push(NetOut::Damage(id, m.damage));
                    }
                }
            }
        }
    }

    fn update_flares(&mut self, dt: f32) {
        for f in self.flares.iter_mut() {
            f.life -= dt;
            f.vel.y -= G * dt;
            f.vel *= 1.0 - 0.8 * dt;
            f.pos += f.vel * dt;
        }
        self.flares.retain(|f| f.life > 0.0);
    }

    fn update_particles(&mut self, dt: f32) {
        for p in self.particles.iter_mut() {
            p.life -= dt;
            p.pos += p.vel * dt;
            p.vel *= 1.0 - 0.6 * dt;
            p.size = (p.size + p.grow * dt).max(0.05);
        }
        self.particles.retain(|p| p.life > 0.0);
        if self.particles.len() > 1500 {
            let n = self.particles.len() - 1500;
            self.particles.drain(0..n);
        }
    }

    // -------------------------------------------------------------- damage

    fn spark(&mut self, at: Vec3) {
        for _ in 0..3 {
            self.particles.push(Particle {
                pos: at,
                vel: vec3(gen_range(-30.0, 30.0), gen_range(-30.0, 30.0), gen_range(-30.0, 30.0)),
                life: 0.25,
                max_life: 0.25,
                size: 1.5,
                grow: 0.0,
                col: Color::new(1.0, 0.95, 0.5, 1.0),
                col2: Color::new(1.0, 0.4, 0.1, 0.0),
            });
        }
    }

    pub fn explosion(&mut self, at: Vec3, scale: f32) {
        self.events.push(Event::Explosion);
        let n = (24.0 * scale) as i32 + 6;
        for _ in 0..n {
            let v = vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)).normalize_or(Vec3::Y) * gen_range(10.0, 60.0) * scale.max(0.5);
            self.particles.push(Particle {
                pos: at,
                vel: v,
                life: gen_range(0.5, 1.1),
                max_life: 1.1,
                size: gen_range(3.0, 7.0) * scale.max(0.5),
                grow: 6.0 * scale,
                col: Color::new(1.0, 0.85, 0.3, 1.0),
                col2: Color::new(0.8, 0.2, 0.05, 0.0),
            });
        }
        for _ in 0..(n / 2) {
            self.particles.push(Particle {
                pos: at + vec3(gen_range(-5.0, 5.0), gen_range(-5.0, 5.0), gen_range(-5.0, 5.0)),
                vel: vec3(gen_range(-8.0, 8.0), gen_range(2.0, 12.0), gen_range(-8.0, 8.0)),
                life: gen_range(2.0, 3.5),
                max_life: 3.5,
                size: gen_range(5.0, 9.0) * scale.max(0.5),
                grow: 5.0,
                col: Color::new(0.25, 0.22, 0.2, 0.9),
                col2: Color::new(0.45, 0.45, 0.45, 0.0),
            });
        }
    }

    pub fn damage_enemy(&mut self, id: u32, dmg: f32) {
        self.damage_enemy_by(id, dmg, 0);
    }

    /// `killer`: pilot id that fired (0 = this computer's player).
    pub fn damage_enemy_by(&mut self, id: u32, dmg: f32, killer: u32) {
        if !self.authority {
            // the host owns the bots: report the hit, show a flash meanwhile
            if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                e.flash = 1.0;
            }
            self.out.push(NetOut::HitEnemy(id, dmg));
            return;
        }
        let mut killed = None;
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id && e.alive) {
            e.hp -= dmg;
            e.flash = 1.0;
            if e.hp <= 0.0 {
                e.alive = false;
                killed = Some((e.pos, e.spec().score));
            }
        }
        if let Some((pos, s)) = killed {
            self.explosion(pos, 1.2);
            let base = s + 25 * (self.wave - 1);
            if self.online() {
                let k = if killer == 0 { self.my_id } else { killer };
                self.out.push(NetOut::Kill('e', id, k, base, pos));
            }
            if killer != 0 {
                self.wave_kills += 1;
                let name = self.remote_name(killer);
                self.msg(&format!("{} SPLASHED ONE", name), Color::new(0.6, 0.85, 1.0, 1.0));
                return;
            }
            let pts = self.add_score(base);
            self.kills += 1;
            self.wave_kills += 1;
            self.events.push(Event::EnemyKilled);
            self.msg(&format!("SPLASH ONE  +{}", pts), Color::new(0.5, 1.0, 0.5, 1.0));
        }
    }

    pub fn damage_ground(&mut self, id: u32, dmg: f32) {
        self.damage_ground_by(id, dmg, 0);
    }

    pub fn damage_ground_by(&mut self, id: u32, dmg: f32, killer: u32) {
        if !self.authority {
            if let Some(g) = self.ground.iter_mut().find(|g| g.id == id) {
                g.flash = 1.0;
            }
            self.out.push(NetOut::HitGround(id, dmg));
            return;
        }
        let mut killed = None;
        if let Some(g) = self.ground.iter_mut().find(|g| g.id == id && g.alive) {
            g.hp -= dmg;
            g.flash = 1.0;
            if g.hp <= 0.0 {
                g.alive = false;
                killed = Some((g.pos, g.score()));
            }
        }
        if let Some((pos, s)) = killed {
            self.explosion(pos + Vec3::Y * 4.0, 1.5);
            if self.online() {
                let k = if killer == 0 { self.my_id } else { killer };
                self.out.push(NetOut::Kill('g', id, k, s, pos));
            }
            if killer != 0 {
                return;
            }
            let s = self.add_score(s);
            self.ground_kills += 1;
            self.events.push(Event::GroundKilled);
            self.msg(&format!("TARGET DESTROYED  +{}", s), Color::new(0.5, 1.0, 0.5, 1.0));
        }
    }

    #[allow(dead_code)]
    pub fn damage_player(&mut self, dmg: f32) {
        self.damage_player_from(dmg, 0);
    }

    /// `attacker`: pilot id in a dogfight, 0 for bots, SAMs and terrain.
    pub fn damage_player_from(&mut self, dmg: f32, attacker: u32) {
        let p = &mut self.player;
        if !p.alive || p.shield > 0.0 {
            return;
        }
        self.last_attacker = attacker;
        let p = &mut self.player;
        p.hp -= dmg;
        p.since_hit = 0.0;
        p.hit_flash = 1.0;
        self.events.push(Event::PlayerHit);
        if p.hp <= 0.0 {
            self.kill_player("SHOT DOWN");
        }
    }

    fn kill_player(&mut self, why: &str) {
        let pos = self.player.pos;
        self.player.alive = false;
        self.player.hp = 0.0;
        self.player.respawn_t = RESPAWN_DELAY;
        self.explosion(pos, 1.6);
        if self.online() {
            // online matches never end: respawn as often as you like
            self.deaths += 1;
            let by = self.last_attacker;
            self.out.push(NetOut::Down(by, pos));
            let txt = if by != 0 { format!("SHOT DOWN BY {}", self.remote_name(by)) } else { why.to_string() };
            self.msg(&txt, Color::new(1.0, 0.35, 0.25, 1.0));
            self.last_attacker = 0;
            return;
        }
        self.lives -= 1;
        let txt = if self.lives > 0 { format!("{}  -  {} LEFT", why, self.lives) } else { why.to_string() };
        self.msg(&txt, Color::new(1.0, 0.35, 0.25, 1.0));
    }
}
