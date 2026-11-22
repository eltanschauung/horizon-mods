//! `EntityDanmakuMob.onUpdate` as a goal: hover above the ground, find the player in range, face them,
//! keep at its attack distance, and run its attack while it can see them. A goal has no host, so what
//! it fires waits in `FIRED`, and the spell cards it declares in `CASTS`, for the level's tick.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mod_api::{EntityRef, GoalSpec, Host, LevelRef, LivingRef, MobRef, ServerHost};

use crate::attack::Atk;
use crate::mobs::{self, Pattern};
use crate::pattern::Fire;
use crate::shot::Shot;
use crate::vec::{Rng, V3, v3, vec_from_angle};

/// Shots fired this tick, by their mob.
pub static FIRED: Mutex<Vec<(u32, Shot, bool)>> = Mutex::new(Vec::new());
/// Spell cards declared this tick: the mob, the card.
pub static CASTS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

/// A boss's phase as the death hook last set it, for its goal to take up.
#[derive(Clone, Copy, Debug)]
pub struct Boss {
    pub phase: i32,
    pub counter: Option<i32>,
    pub invincible: i32,
    pub ai: Option<mobs::Ai>,
}

pub static BOSSES: LazyLock<Mutex<HashMap<u32, Boss>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Each fighting mob's health and phase as its goal last saw them, for the classic bosses' bars: the
/// server has no other way to a mob's health.
pub static STATUS: LazyLock<Mutex<HashMap<u32, (f32, i32)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn status(id: u32) -> Option<(f32, i32)> {
    STATUS.lock().ok().and_then(|s| s.get(&id).copied())
}

/// Bosses that caught one of their own Bewitching Power Spoiler scales coming back: regeneration and
/// strength, as the original gave.
pub static BUFFS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Whether a mob fires at the original's full rate: the old crow tengu and the classic bosses.
pub fn full_rate(id: &str) -> bool {
    id == "old_crow_tengu" || id.starts_with("classic_") || id == "yukari_great_doll"
}

/// The phase a mob is in, for the death hook: its start until a phase ends.
pub fn phase_of(id: u32, spec: &mobs::Mob) -> i32 {
    BOSSES
        .lock()
        .ok()
        .and_then(|b| b.get(&id).map(|b| b.phase))
        .unwrap_or_else(|| mobs::start_of(spec, id))
}

/// Whether the mob can't be hurt just now (a phase's first ticks).
pub fn invincible(id: u32) -> bool {
    BOSSES
        .lock()
        .ok()
        .and_then(|b| b.get(&id).map(|b| b.invincible > 0))
        .unwrap_or(false)
}

/// IDs used by the existing boss and projectile logic.
pub static DECOYS: LazyLock<Mutex<std::collections::HashSet<u32>>> =
    LazyLock::new(Default::default);
pub static OURS: LazyLock<Mutex<std::collections::HashSet<u32>>> = LazyLock::new(Default::default);
pub static ATTACKING: LazyLock<Mutex<std::collections::HashSet<u32>>> =
    LazyLock::new(Default::default);

pub fn ours(id: u32) -> bool {
    OURS.lock().is_ok_and(|ours| ours.contains(&id))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Who {
    Mob(u32),
    Player,
}

static STATES: LazyLock<Mutex<HashMap<u32, (Danmaku, i32)>>> = LazyLock::new(Default::default);
static LAST_HURT: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(Default::default);

pub fn pending(id: u32) {
    if let Ok(mut states) = STATES.lock() {
        states.remove(&id);
    }
    if let Ok(mut hurt) = LAST_HURT.lock() {
        hurt.remove(&id);
    }
}

pub fn hurt_by(victim: u32, attacker: i32) {
    if let Ok(attacker) = u32::try_from(attacker)
        && attacker != 0
        && let Ok(mut hits) = LAST_HURT.lock()
    {
        hits.insert(victim, attacker);
    }
}

struct MobFrame {
    entity: EntityRef,
    id: u32,
    position: V3,
    velocity: V3,
    eye_height: f64,
    age: i32,
    health: f32,
    yaw: f32,
    head_pitch: f32,
    no_gravity: bool,
    removed: bool,
    persistent: bool,
    target: Option<Who>,
    buff: bool,
}

impl MobFrame {
    fn eye_y(&self) -> f64 {
        self.position.y + self.eye_height
    }
}

struct PlayerView {
    living: LivingRef,
    position: V3,
    look: V3,
    eye: f64,
    middle: f64,
    sneaking: bool,
}

fn configure(host: &mut dyn ServerHost, mob: MobRef, spec: &mobs::Mob) {
    for goal in mob.goals(host).unwrap_or_default() {
        let _ = mob.remove_goal(host, &goal.id);
    }
    for goal in mob.target_goals(host).unwrap_or_default() {
        let _ = mob.remove_target_goal(host, &goal.id);
    }
    mob.add_goal(host, 0, &GoalSpec::Float);
    if spec.npc {
        mob.add_goal(
            host,
            5,
            &GoalSpec::Stroll {
                speed: 0.5,
                interval: 120,
                water_avoiding: None,
                swim: false,
                flying: false,
            },
        );
        mob.add_goal(
            host,
            6,
            &GoalSpec::LookAtPlayer {
                range: 8.0,
                probability: 0.02,
                only_horizontal: false,
                mobs: false,
                only: None,
            },
        );
    }
}

pub fn tick_level(host: &mut dyn ServerHost, level: LevelRef) {
    let players: Vec<_> = level
        .online_players(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|player| player.game_mode(host) != Some(mod_api::GameMode::Spectator))
        .filter_map(|player| {
            let entity = player.entity();
            let living = host.living(entity)?;
            if living.health(host)? <= 0.0 {
                return None;
            }
            let position = V3::from_array(entity.position(host)?);
            let [yaw, pitch] = entity.rotation(host)?;
            let bounds = crate::bounds(host, entity)?;
            Some(PlayerView {
                living,
                position,
                look: crate::vec::angle(yaw, pitch),
                eye: position.y + entity.eye_height(host)?,
                middle: (bounds[0][1] + bounds[1][1]) / 2.0,
                sneaking: player.is_secondary_use_active(host) == Some(true),
            })
        })
        .collect();
    let entities = level.all_entities(host).unwrap_or_default();
    let ours_here: std::collections::HashSet<u32> = entities
        .iter()
        .filter_map(|entity| crate::is_ours(&entity.kind(host)?.id(host)).then_some(entity.id()))
        .collect();
    if let Ok(mut ours) = OURS.lock() {
        ours.extend(ours_here.iter().copied());
    }

    for entity in entities
        .iter()
        .copied()
        .filter(|entity| ours_here.contains(&entity.id()))
    {
        let (Some(mob), Some(living), Some(kind), Some(position), Some(velocity)) = (
            host.mob(entity),
            host.living(entity),
            entity.kind(host),
            entity.position(host),
            entity.velocity(host),
        ) else {
            continue;
        };
        let kind = kind.id(host);
        let Some(spec) = mobs::by_kind(&kind) else {
            continue;
        };
        let Some(health) = living.health(host) else {
            continue;
        };
        if health <= 0.0 {
            continue;
        }
        let [yaw, pitch] = entity.rotation(host).unwrap_or_default();
        let mut frame = MobFrame {
            entity,
            id: entity.id(),
            position: V3::from_array(position),
            velocity: V3::from_array(velocity),
            eye_height: entity.eye_height(host).unwrap_or(1.62),
            age: entity.tick_count(host).unwrap_or(0),
            health,
            yaw,
            head_pitch: pitch,
            no_gravity: spec.ai.flying_height > 0,
            removed: false,
            persistent: mob.persistence_required(host).unwrap_or(false),
            target: None,
            buff: false,
        };
        let closest = players.iter().min_by(|a, b| {
            (a.position - frame.position)
                .length()
                .total_cmp(&(b.position - frame.position).length())
        });
        if let Ok(mut states) = STATES.lock() {
            let (state, ticks) = states
                .entry(frame.id)
                .or_insert_with(|| (Danmaku::new(spec, frame.id), 0));
            if *ticks == 0 {
                configure(host, mob, spec);
            }
            *ticks += 1;
            if spec.id == "cursed_decoy_doll" {
                frame.velocity.x = 0.0;
                frame.velocity.z = 0.0;
                frame.removed = *ticks > 180;
                if let Ok(mut decoys) = DECOYS.lock() {
                    if frame.removed {
                        decoys.remove(&frame.id);
                    } else {
                        decoys.insert(frame.id);
                    }
                }
            } else if spec.id == "yin_yang_orb" {
                frame.no_gravity = true;
                if let Some(player) = closest {
                    let [yaw, pitch] = player.living.entity().rotation(host).unwrap_or_default();
                    let radius = if player.sneaking { 1.0 } else { 3.0 };
                    let radial = crate::vec::rotation_vector_from_angle(
                        yaw,
                        pitch,
                        -*ticks as f32 * 7.5,
                        radius,
                    );
                    frame.position = player.position + v3(radial.x, 0.8, radial.z);
                    frame.yaw = yaw;
                    frame.velocity = V3::ZERO;
                } else {
                    frame.removed = true;
                }
            } else if !spec.npc {
                state.tick(&mut frame, host, level, closest);
            }
        }
        if frame.removed {
            entity.discard(host);
            crate::forget(entity.id());
            pending(entity.id());
            continue;
        }
        if frame.health <= 0.0 {
            entity.hurt(
                host,
                "minecraft:on_fire",
                health.max(1.0) * 10.0,
                None,
                None,
            );
            continue;
        }
        if frame.buff {
            crate::apply_effect(host, entity, "minecraft:regeneration", 400, 1);
            crate::apply_effect(host, entity, "minecraft:strength", 400, 1);
        }
        if !spec.npc {
            entity.set_no_gravity(host, frame.no_gravity);
            entity.set_velocity(host, frame.velocity.array());
            if spec.id == "yin_yang_orb" {
                entity.teleport_to(
                    host,
                    frame.position.array(),
                    Some([frame.yaw, frame.head_pitch]),
                );
            } else {
                entity.set_rotation(host, frame.yaw, frame.head_pitch);
            }
            let target = match frame.target {
                Some(Who::Mob(id)) => host.entity(id).and_then(|e| host.living(e)),
                Some(Who::Player) => closest.map(|player| player.living),
                None => None,
            };
            mob.set_target(host, target);
        }
    }
    redirect_hostiles(host, level, &entities, &ours_here);
}

/// Vanilla's monster species with a targeting selector.
fn monster(kind: &str) -> bool {
    let Some(id) = kind.strip_prefix("minecraft:") else {
        return false;
    };
    matches!(
        id,
        "blaze"
            | "bogged"
            | "breeze"
            | "cave_spider"
            | "creaking"
            | "drowned"
            | "elder_guardian"
            | "ender_dragon"
            | "endermite"
            | "evoker"
            | "ghast"
            | "guardian"
            | "hoglin"
            | "husk"
            | "illusioner"
            | "magma_cube"
            | "parched"
            | "phantom"
            | "piglin_brute"
            | "pillager"
            | "ravager"
            | "shulker"
            | "silverfish"
            | "skeleton"
            | "slime"
            | "spider"
            | "stray"
            | "vex"
            | "vindicator"
            | "warden"
            | "witch"
            | "wither"
            | "wither_skeleton"
            | "zoglin"
            | "zombie"
            | "zombie_villager"
            | "giant"
            | "camel_husk"
            | "zombie_nautilus"
    )
}

fn redirect_hostiles(
    host: &mut dyn ServerHost,
    level: LevelRef,
    entities: &[EntityRef],
    ours: &std::collections::HashSet<u32>,
) {
    let priority: Vec<_> = entities
        .iter()
        .copied()
        .filter_map(|entity| {
            let id = entity.id();
            if !ours.contains(&id) {
                return None;
            }
            let decoy = DECOYS.lock().is_ok_and(|decoys| decoys.contains(&id));
            let active = ATTACKING
                .lock()
                .is_ok_and(|attacking| attacking.contains(&id));
            if !decoy && !active {
                return None;
            }
            let living = host.living(entity)?;
            if living.health(host)? <= 0.0 {
                return None;
            }
            Some((living, V3::from_array(entity.position(host)?), decoy))
        })
        .collect();
    for &entity in entities {
        if ours.contains(&entity.id()) {
            continue;
        }
        let (Some(mob), Some(kind)) = (host.mob(entity), entity.kind(host)) else {
            continue;
        };
        let id = kind.id(host);
        if id == "minecraft:creeper" {
            if mob
                .target(host)
                .is_some_and(|target| ours.contains(&target.entity().id()))
            {
                mob.set_target(host, None);
            }
            continue;
        }
        if priority.is_empty()
            || matches!(
                id.as_str(),
                "minecraft:enderman" | "minecraft:zombified_piglin" | "minecraft:piglin"
            )
            || !monster(&id)
        {
            continue;
        }
        let Some(at) = entity.position(host).map(V3::from_array) else {
            continue;
        };
        let near = priority
            .iter()
            .filter_map(|&(target, pos, decoy)| {
                let distance = (pos - at).length();
                (distance <= if decoy { 40.0 } else { 16.0 })
                    .then_some((target, distance, decoy, pos))
            })
            .filter(|&(_, _, decoy, pos)| {
                decoy || level.line_of_sight(host, at.array(), pos.array()) == Some(true)
            })
            .min_by(|a, b| b.2.cmp(&a.2).then(a.1.total_cmp(&b.1)));
        if let Some((target, _, _, _)) = near {
            mob.set_target(host, Some(target));
        }
    }
}

/// How much of the original's rate of fire the mobs keep: their attacks, and the spell cards they
/// declare, advance on this share of ticks, so every pattern keeps its shape, only slower.
pub const PACE: f32 = 0.35;

const DESPAWN_FAR: f64 = 128.0;
const DESPAWN_NEAR: f64 = 32.0;
/// `THKaguyaConfig.danmakuLevel`: 1 easy, 2 normal, 3 hard, 4 lunatic. Miko changes it.
pub static LEVEL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(2);

pub fn level() -> i32 {
    LEVEL.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub struct Danmaku {
    spec: &'static mobs::Mob,
    ai: mobs::Ai,
    phase: i32,
    pattern: Pattern,
    span: i32,
    target: bool,
    counter: i32,
    lost_target: i32,
    move_vec: V3,
    move_timer: i32,
    move_timer_max: i32,
    stop: bool,
    mem: [i32; 4],
    rng: Rng,
    /// Toward the next tick its attack advances on (`PACE`).
    pace: f32,
    /// A mob, not one of ours, that hurt it: it fights that back first.
    revenge: Option<u32>,
}

/// Whom one of our mobs fights: the player, or a mob that hurt it.
struct Foe {
    who: Who,
    at: V3,
    eye: f64,
    middle: f64,
}

/// How far one of our mobs goes after a mob that hurt it.
const REVENGE_RANGE: f64 = 40.0;

impl Danmaku {
    pub fn new(spec: &'static mobs::Mob, id: u32) -> Danmaku {
        let mut rng = Rng::new(u64::from(id).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x7A11);
        let level = &mobs::FAIRY_PATTERNS[(level() - 1) as usize];
        let mut pattern = level[rng.int(level.len() as u32) as usize];
        if spec.id == "ice_fairy" {
            pattern.shot = pattern.shot / 8 * 8 + 5;
        }
        let phase = mobs::start_of(spec, id);
        let ai = spec
            .start_ai
            .iter()
            .find(|(p, _)| *p == phase)
            .map_or(spec.ai, |(_, a)| *a);
        Danmaku {
            spec,
            ai,
            phase,
            pattern,
            span: pattern.span,
            target: false,
            counter: 0,
            lost_target: 0,
            move_vec: V3::ZERO,
            move_timer: 0,
            move_timer_max: 0,
            stop: false,
            mem: [0; 4],
            rng,
            pace: 0.0,
            revenge: None,
        }
    }

    fn glide(&mut self, dir: V3, speed: f64, time: i32) {
        self.move_vec = dir * speed;
        self.move_timer = time;
        self.move_timer_max = time;
    }

    /// Take up a phase the death hook set.
    fn follow_boss(&mut self, id: u32) {
        let Ok(mut bosses) = BOSSES.lock() else {
            return;
        };
        let Some(b) = bosses.get_mut(&id) else { return };
        if b.invincible > 0 {
            b.invincible -= 1;
        }
        if b.phase != self.phase {
            self.phase = b.phase;
            self.stop = false;
        }
        if let Some(c) = b.counter.take() {
            self.counter = c;
        }
        if let Some(a) = b.ai.take() {
            self.ai = a;
        }
    }
}

fn air(host: &dyn Host, level: LevelRef, x: i32, y: i32, z: i32) -> bool {
    crate::block_id(host, level, [x, y, z]).is_some_and(|block| block.ends_with("air"))
}

/// `getPosYFromEye`.
fn shot_y(eye_y: f64, pitch: f32) -> f64 {
    let p = f64::from(pitch).to_radians();
    eye_y + p.sin() * -0.5 + p.cos() * -0.5
}

impl Danmaku {
    fn tick(
        &mut self,
        m: &mut MobFrame,
        host: &dyn Host,
        world: LevelRef,
        player: Option<&PlayerView>,
    ) {
        if m.health <= 0.0 {
            return;
        }
        self.follow_boss(m.id);
        if let Ok(mut o) = OURS.lock() {
            o.insert(m.id);
        }
        if self.spec.classic() {
            if let Ok(mut s) = STATUS.lock() {
                s.insert(m.id, (m.health, self.phase));
            }
            let buffed = BUFFS.lock().is_ok_and(|mut b| {
                let had = b.contains(&m.id);
                b.retain(|id| *id != m.id);
                had
            });
            if buffed {
                m.buff = true;
            }
        }
        if !m.persistent
            && let Some(p) = player.map(|p| p.position)
        {
            let d = ((p.x - m.position.x).powi(2)
                + (p.y - m.position.y).powi(2)
                + (p.z - m.position.z).powi(2))
            .sqrt();
            if d > DESPAWN_FAR || (d > DESPAWN_NEAR && self.rng.int(800) == 0) {
                m.removed = true;
                return;
            }
        }
        let (x, y, z) = (
            m.position.x.floor() as i32,
            m.position.y.floor() as i32,
            m.position.z.floor() as i32,
        );
        // A phantom fades in the sun.
        if self.spec.burns
            && crate::daytime(world.day_time(host).unwrap_or_default())
            && world.can_see_sky(host, [x, y + 1, z]) == Some(true)
            && self.rng.float() * 30.0 < 1.2
        {
            m.health = 0.0;
            return;
        }
        m.no_gravity = self.ai.flying_height > 0;

        if self.move_timer > 0 {
            let rate = (f64::from(self.move_timer) / f64::from(self.move_timer_max)
                * std::f64::consts::FRAC_PI_2)
                .cos();
            m.velocity.x = self.move_vec.x * rate;
            m.velocity.z = self.move_vec.z * rate;
            m.velocity.y += self.move_vec.y * rate * 0.1;
            self.move_timer -= 1;
        } else {
            m.velocity.x *= 0.5;
            m.velocity.z *= 0.5;
        }
        if self.stop {
            m.velocity.x = 0.0;
            m.velocity.y = 0.0;
            m.velocity.z = 0.0;
        }

        let attacker = LAST_HURT
            .lock()
            .ok()
            .and_then(|mut hits| hits.remove(&m.id));
        if let Some(id) = attacker
            && let Some(entity) = host.entity(id)
            && let Some(kind) = entity.kind(host).map(|kind| kind.id(host))
        {
            if kind == "minecraft:player" {
                self.target = true;
            } else if !ours(id) {
                self.revenge = Some(id);
            }
        }
        let revenge = self.revenge.and_then(|id| {
            let entity = host.entity(id)?;
            let living = host.living(entity)?;
            let at = V3::from_array(entity.position(host)?);
            let bounds = crate::bounds(host, entity)?;
            if living.health(host)? <= 0.0 || (at - m.position).length() > REVENGE_RANGE {
                return None;
            }
            Some(Foe {
                who: Who::Mob(id),
                at,
                eye: at.y + entity.eye_height(host).unwrap_or(1.62),
                middle: (bounds[0][1] + bounds[1][1]) / 2.0,
            })
        });
        if revenge.is_none() {
            self.revenge = None;
        }
        if let Some(p) = player
            && !self.target
        {
            let d = ((p.position.x - m.position.x).powi(2)
                + (p.position.y - m.position.y).powi(2)
                + (p.position.z - m.position.z).powi(2))
            .sqrt();
            if d <= self.ai.detection_distance {
                self.target = true;
            }
        }
        if player.is_none() {
            self.target = false;
        }
        // While it fights it holds the player as its target, so the dolls and hostile mobs know it.
        m.target = match &revenge {
            Some(f) => Some(f.who),
            None => self.target.then_some(Who::Player),
        };
        if let Ok(mut attacking) = ATTACKING.lock() {
            if self.target || revenge.is_some() {
                attacking.insert(m.id);
            } else {
                attacking.remove(&m.id);
            }
        }

        if self.ai.flying_height > 0 && !self.stop {
            let mut height = 0;
            while height < 8 && air(host, world, x, y - height, z) {
                height += 1;
            }
            let foe_y = revenge
                .as_ref()
                .map(|f| f.at.y)
                .or_else(|| player.filter(|_| self.target).map(|p| p.position.y));
            if let Some(foe_y) = foe_y
                && height > self.ai.flying_height
            {
                m.velocity.y += (foe_y - m.position.y) * 6.0e-4;
            } else if height < self.ai.flying_height {
                m.velocity.y += 0.006;
            }
        }

        let foe = revenge.or_else(|| {
            player.filter(|_| self.target).map(|p| Foe {
                who: Who::Player,
                at: v3(p.position.x, p.position.y, p.position.z),
                eye: p.eye,
                middle: p.middle,
            })
        });
        let Some(p) = foe else { return };
        let eye = m.eye_y();
        let (dx, dy, dz) = (p.at.x - m.position.x, p.eye - eye, p.at.z - m.position.z);
        let yaw = (-(dx.atan2(dz)) as f32).to_degrees();
        let pitch = (-(dy.atan2((dx * dx + dz * dz).sqrt())) as f32).to_degrees();
        if !self.stop {
            m.yaw = yaw;
            m.head_pitch = pitch;
        }

        // The attack goes on only on its share of ticks.
        // The old crow tengu and the classic bosses fire at the original's full rate.
        self.pace += if full_rate(self.spec.id) { 1.0 } else { PACE };
        let advance = self.pace >= 1.0;
        if advance {
            self.pace -= 1.0;
        }
        let sees = world.line_of_sight(
            host,
            [m.position.x, eye, m.position.z],
            [p.at.x, p.eye, p.at.z],
        ) == Some(true);
        if sees && !advance {
            self.lost_target = 0;
        } else if sees {
            self.lost_target = 0;
            let wet = crate::block_id(host, world, [x, y, z])
                .is_some_and(|block| block.contains("water"));
            if self.counter >= 0 && self.phase != 127 {
                let mut fired = Vec::new();
                let mut atk = Atk {
                    c: self.counter,
                    level: level(),
                    phase: self.phase,
                    pos: v3(m.position.x, shot_y(eye, m.head_pitch), m.position.z),
                    feet: v3(m.position.x, m.position.y, m.position.z),
                    yaw: m.yaw,
                    pitch: m.head_pitch,
                    ticks: m.age as i32,
                    target: Some(v3(p.at.x, p.middle, p.at.z)),
                    fire: Fire {
                        out: &mut fired,
                        rng: &mut self.rng,
                    },
                    span: self.span,
                    pattern: self.pattern,
                    glide: None,
                    stop: None,
                    card: None,
                    sound: false,
                    wet,
                    mem: &mut self.mem,
                };
                (self.spec.attack)(&mut atk);
                let (c, span, glide, stop, card, sound) =
                    (atk.c, atk.span, atk.glide, atk.stop, atk.card, atk.sound);
                self.counter = c;
                self.span = span;
                if let Some((dir, speed, time)) = glide {
                    self.glide(dir, speed, time);
                }
                if let Some(s) = stop {
                    self.stop = s;
                }
                if let Some(card) = card
                    && let Ok(mut q) = CASTS.lock()
                {
                    q.push((m.id, card));
                }
                if !fired.is_empty()
                    && let Ok(mut q) = FIRED.lock()
                {
                    let mut first = sound;
                    for s in fired {
                        q.push((m.id, s, std::mem::take(&mut first)));
                    }
                }
            }
            self.counter += 1;
            if self.spec.fairy_counter && self.counter > self.span {
                self.counter = 0;
            }
        } else {
            self.lost_target += 1;
            if self.lost_target > 200 {
                self.target = false;
            }
        }

        if self.stop {
            return;
        }
        if self.move_timer <= 0 && self.lost_target > 0 {
            let dir = vec_from_angle(self.rng.float() + 360.0, self.rng.float() * 360.0, 1.0);
            self.glide(dir, self.ai.speed, 30);
        }
        if self.move_timer <= 0 && self.ai.attack_distance > 0.0 {
            let d = (dx * dx + (p.at.y - m.position.y).powi(2) + dz * dz).sqrt();
            let a = self.ai.attack_distance;
            if d < a {
                self.glide(
                    vec_from_angle(yaw + 180.0, pitch, 1.0),
                    self.ai.speed * 0.3 * (1.0 - d / a),
                    15,
                );
            } else {
                self.glide(
                    vec_from_angle(yaw, pitch, 1.0),
                    self.ai.speed * 0.5 * (d / a),
                    15,
                );
            }
        }
    }
}
