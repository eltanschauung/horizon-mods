//! What the translated spell cards (`gen_cards.rs`) run on: the original's types as small Rust
//! stand-ins, Java's arithmetic where it differs from Rust's, and `Cx`, through which a card reaches
//! its user, its target, the shots of its level and the world.
//!
//! The things a card sets going that are not shots (a Mini-Hakkero, the Frost King's familiars,
//! Sakuya's watch, a gap) are not entities either: they live in `HELPERS` and the server ticks them
//! here, as the originals' `onUpdate` did.

#![allow(dead_code, clippy::too_many_arguments)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mod_api::ServerHost;

use crate::pattern::Fire;
use crate::sdk::{SpellHost, block_id};
pub use crate::shot::ShotData;
use crate::shot::{Anchor, DAMAGE, SIZE, Shot, form, laser, shot_full};
pub use crate::vec::Rng;
use crate::vec::{V3, angle as look_angle, rotate as turn, v3};

pub type SD = ShotData;
pub type List = Vec<EH>;

/// A translated piece the translator could not port: never reached.
#[macro_export]
macro_rules! unported {
    ($s:expr) => {
        unreachable!("{}", $s)
    };
}
#[allow(unused_imports)]
pub use crate::unported;

// ---------------------------------------------------------------------------------------------- handles

/// A shot of the level being ticked, by its place in the level's list (`EntityTHShot`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SH {
    #[default]
    None,
    I(usize),
}

/// Any entity a card can be handed: a living thing, a shot, one of its helpers, or the card.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EH {
    #[default]
    None,
    Ent(Ent),
    Shot(SH),
    New(u32),
    Card,
}

pub fn eh_shot(e: EH) -> SH {
    match e {
        EH::Shot(s) => s,
        _ => SH::None,
    }
}

/// An axis-aligned box (`AxisAlignedBB`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bx {
    pub min: V3,
    pub max: V3,
}

impl Bx {
    pub const ZERO: Bx = Bx {
        min: V3::ZERO,
        max: V3::ZERO,
    };

    pub fn around(at: V3, half_width: f64, height: f64) -> Bx {
        Bx {
            min: v3(at.x - half_width, at.y, at.z - half_width),
            max: v3(at.x + half_width, at.y + height, at.z + half_width),
        }
    }
    /// `addCoord`: stretched the way of `(x, y, z)`.
    pub fn add_coord(self, x: f64, y: f64, z: f64) -> Bx {
        let mut b = self;
        if x < 0.0 {
            b.min.x += x
        } else {
            b.max.x += x
        }
        if y < 0.0 {
            b.min.y += y
        } else {
            b.max.y += y
        }
        if z < 0.0 {
            b.min.z += z
        } else {
            b.max.z += z
        }
        b
    }
    pub fn expand(self, x: f64, y: f64, z: f64) -> Bx {
        Bx {
            min: v3(self.min.x - x, self.min.y - y, self.min.z - z),
            max: v3(self.max.x + x, self.max.y + y, self.max.z + z),
        }
    }
    pub fn contract(self, x: f64, y: f64, z: f64) -> Bx {
        self.expand(-x, -y, -z)
    }
    pub fn offset(self, x: f64, y: f64, z: f64) -> Bx {
        Bx {
            min: self.min + v3(x, y, z),
            max: self.max + v3(x, y, z),
        }
    }
    pub fn copy(self) -> Bx {
        self
    }
    pub fn contains(self, p: V3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }
    pub fn meets(self, o: Bx) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
            && self.min.z <= o.max.z
            && self.max.z >= o.min.z
    }
}

/// An item, by id (`""` for none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Item(pub &'static str);

/// A block: the cards only ever ask what it is not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Blk;

/// A living thing (or what a card treats as one) as it stood when first asked about this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ent {
    pub id: u32,
    pub present: bool,
    /// One of the card's helpers, by its id; 0 for a real entity.
    pub helper: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub eye: f32,
    pub height: f32,
    pub width: f32,
    pub dead: bool,
    pub health: f32,
    pub max_health: f32,
    pub mx: f64,
    pub my: f64,
    pub mz: f64,
    pub ticks: i32,
    pub wet: bool,
    pub sneaking: bool,
    pub on_ground: bool,
    pub living: bool,
    pub player: bool,
    pub monster: bool,
    pub passive: bool,
    pub bx: Bx,
    pub held: Item,
}

impl Ent {
    pub const NONE: Ent = Ent {
        id: 0,
        present: false,
        helper: 0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
        yaw: 0.0,
        pitch: 0.0,
        eye: 0.0,
        height: 0.0,
        width: 0.0,
        dead: false,
        health: 0.0,
        max_health: 0.0,
        mx: 0.0,
        my: 0.0,
        mz: 0.0,
        ticks: 0,
        wet: false,
        sneaking: false,
        on_ground: false,
        living: false,
        player: false,
        monster: false,
        passive: false,
        bx: Bx::ZERO,
        held: Item(""),
    };

    pub fn pos(&self) -> V3 {
        v3(self.x, self.y, self.z)
    }
    /// `getLookVec`.
    pub fn look(&self) -> V3 {
        look_angle(self.yaw, self.pitch)
    }
    pub fn dist(&self, x: f64, y: f64, z: f64) -> f64 {
        (self.pos() - v3(x, y, z)).length()
    }
    pub fn dist_to(&self, o: &Ent) -> f32 {
        self.dist(o.x, o.y, o.z) as f32
    }
    pub fn dist_sq_to(&self, o: &Ent) -> f64 {
        let d = self.pos() - o.pos();
        d.x * d.x + d.y * d.y + d.z * d.z
    }
    fn same(&self, o: &Ent) -> bool {
        self.present && o.present && self.id == o.id && self.helper == o.helper
    }
}

/// What a ray met (`MovingObjectPosition`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    pub present: bool,
    pub entity: EH,
    /// The face struck, as Minecraft numbers them: 0 down, 1 up, 2 north, 3 south, 4 west, 5 east.
    pub side: i32,
    pub at: V3,
    pub bx: i32,
    pub by: i32,
    pub bz: i32,
}

impl Hit {
    pub const NONE: Hit = Hit {
        present: false,
        entity: EH::None,
        side: -1,
        at: V3::ZERO,
        bx: 0,
        by: 0,
        bz: 0,
    };
}

/// `LaserData`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LD {
    pub form: i32,
    pub color: i32,
    pub width: f32,
    pub length: f32,
    pub damage: f32,
    pub delay: i32,
    pub end: i32,
    pub special: i32,
}

pub fn ld(
    form: i32,
    color: i32,
    width: f32,
    length: f32,
    damage: f32,
    delay: i32,
    end: i32,
    special: i32,
) -> LD {
    LD {
        form,
        color,
        width,
        length,
        damage,
        delay,
        end,
        special,
    }
}

/// `ShotData.shot(form, color, size, damage, delay, end, special)`.
pub fn sd(form: i32, color: i32, size: f32, damage: f32, delay: i32, end: i32, special: i32) -> SD {
    shot_full(
        form.rem_euclid(32) as u8,
        color.rem_euclid(16) as u8,
        size,
        damage,
        delay,
        end,
        special,
    )
}

/// `ShotData.shot(form, color, delay, end, special)`: the form's own size and damage.
pub fn sd_form(form: i32, color: i32, delay: i32, end: i32, special: i32) -> SD {
    let f = form.rem_euclid(32) as usize;
    sd(form, color, SIZE[f], DAMAGE[f], delay, end, special)
}

// ---------------------------------------------------------------------------------------------- Java

pub fn jsin(a: f32) -> f32 {
    a.sin()
}
pub fn jcos(a: f32) -> f32 {
    a.cos()
}
pub fn jdiv_i32(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { a.wrapping_div(b) }
}
pub fn jdivrem_i32(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { a.wrapping_rem(b) }
}
pub fn jdiv_i64(a: i64, b: i64) -> i64 {
    if b == 0 { 0 } else { a.wrapping_div(b) }
}
pub fn jdivrem_i64(a: i64, b: i64) -> i64 {
    if b == 0 { 0 } else { a.wrapping_rem(b) }
}
/// `Vec3.normalize`: zero for a vector too short to have a way.
pub fn jnorm(v: V3) -> V3 {
    let l = v.length();
    if l < 1e-4 { V3::ZERO } else { v * (1.0 / l) }
}
pub fn jlen(v: V3) -> f64 {
    v.length()
}
pub fn jlen_sq(v: V3) -> f64 {
    v.x * v.x + v.y * v.y + v.z * v.z
}
pub fn dot(a: V3, b: V3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
pub fn cross(a: V3, b: V3) -> V3 {
    v3(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}
/// `Vec3.rotateAroundX`.
pub fn rot_x(v: V3, a: f32) -> V3 {
    let (s, c) = (f64::from(a.sin()), f64::from(a.cos()));
    v3(v.x, v.y * c + v.z * s, v.z * c - v.y * s)
}
/// `Vec3.rotateAroundY`.
pub fn rot_y(v: V3, a: f32) -> V3 {
    let (s, c) = (f64::from(a.sin()), f64::from(a.cos()));
    v3(v.x * c + v.z * s, v.y, v.z * c - v.x * s)
}
/// `Vec3.rotateAroundZ`.
pub fn rot_z(v: V3, a: f32) -> V3 {
    let (s, c) = (f64::from(a.sin()), f64::from(a.cos()));
    v3(v.x * c + v.y * s, v.y * c - v.x * s, v.z)
}
pub fn wrap_deg(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}
pub fn wrap_deg_d(a: f64) -> f64 {
    let mut a = a % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}
pub fn jsignum(a: f64) -> f64 {
    if a > 0.0 {
        1.0
    } else if a < 0.0 {
        -1.0
    } else {
        a
    }
}
/// `MathHelper.getRandomIntegerInRange`.
pub fn rand_range(r: &mut Rng, a: i32, b: i32) -> i32 {
    if a >= b { a } else { r.next_int(b - a + 1) + a }
}

/// A fresh seed, as `new Random()` takes one.
pub fn seed() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x5DEE_CE66);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    t ^ N.fetch_add(0x9E37_79B9_7F4A_7C15, std::sync::atomic::Ordering::Relaxed)
}

/// `java.util.Random`'s calls, on the engine's generator.
pub trait JRandom {
    fn next_int(&mut self, n: i32) -> i32;
    fn next_any_int(&mut self) -> i32;
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;
    fn next_boolean(&mut self) -> bool;
    fn next_gaussian(&mut self) -> f64;
}

impl JRandom for Rng {
    fn next_int(&mut self, n: i32) -> i32 {
        if n <= 0 { 0 } else { self.int(n as u32) as i32 }
    }
    fn next_any_int(&mut self) -> i32 {
        (self.int(u32::MAX) as i64 - i64::from(i32::MAX)) as i32
    }
    fn next_float(&mut self) -> f32 {
        self.float()
    }
    fn next_double(&mut self) -> f64 {
        self.double()
    }
    fn next_boolean(&mut self) -> bool {
        self.int(2) == 1
    }
    fn next_gaussian(&mut self) -> f64 {
        let (u, v) = (self.double().max(1e-12), self.double());
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }
}

// ---------------------------------------------------------------------------------------------- helpers

/// An argument a helper (`NewE`) was made with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg {
    N(f64),
    B(bool),
    V(V3),
    E(Ent),
    S(SD),
    L(LD),
    I(Item),
    Nil,
}

/// One of the things a card makes with `new`: a familiar, a potion effect, a mob to summon. A handle,
/// as a Java reference is: what it points at lives in `HELPERS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NewE {
    pub id: u32,
}

#[derive(Clone, Debug)]
pub struct Helper {
    pub kind: &'static str,
    pub args: Vec<Arg>,
    pub spawned: bool,
    pub dead: bool,
    pub dimension: String,
    /// Its user, by entity id.
    pub user: u32,
    pub by_player: bool,
    pub pos: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub ticks: i32,
    /// Its fields, as the original's subclass keeps them.
    pub f: [f64; 6],
    pub fields: Vec<(&'static str, f64)>,
}

struct Helpers {
    next: u32,
    all: HashMap<u32, Helper>,
}

static HELPERS: LazyLock<Mutex<Helpers>> = LazyLock::new(|| {
    Mutex::new(Helpers {
        next: 1,
        all: HashMap::new(),
    })
});

fn with_helpers<T>(f: impl FnOnce(&mut Helpers) -> T) -> T {
    let mut h = HELPERS.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut h)
}

impl NewE {
    pub fn new(kind: &'static str, args: Vec<Arg>) -> NewE {
        with_helpers(|h| {
            let id = h.next;
            h.next = h.next.wrapping_add(1).max(1);
            h.all.insert(
                id,
                Helper {
                    kind,
                    args,
                    spawned: false,
                    dead: false,
                    dimension: String::new(),
                    user: 0,
                    by_player: false,
                    pos: V3::ZERO,
                    yaw: 0.0,
                    pitch: 0.0,
                    ticks: 0,
                    f: [0.0; 6],
                    fields: Vec::new(),
                },
            );
            NewE { id }
        })
    }
    fn get(&self) -> Option<Helper> {
        with_helpers(|h| h.all.get(&self.id).cloned())
    }
    fn change(&self, f: impl FnOnce(&mut Helper)) {
        with_helpers(|h| {
            if let Some(x) = h.all.get_mut(&self.id) {
                f(x)
            }
        })
    }
    pub fn f(&self, name: &str) -> f64 {
        self.get()
            .and_then(|h| h.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
            .unwrap_or(0.0)
    }
    pub fn set(&self, name: &'static str, v: f64) {
        self.change(|h| {
            h.fields.retain(|(n, _)| *n != name);
            h.fields.push((name, v));
        })
    }
    pub fn set_v(&self, name: &'static str, v: V3) {
        self.set(name, v.x);
    }
    pub fn set_any(&self, _name: &'static str) {}
    /// A method called on it before it is spawned: only `setLocationAndAngles` matters.
    pub fn call(&self, name: &str, args: Vec<Arg>) {
        if name == "func_70012_b" || name == "func_70107_b" {
            let n: Vec<f64> = args
                .iter()
                .filter_map(|a| if let Arg::N(v) = a { Some(*v) } else { None })
                .collect();
            if n.len() >= 3 {
                self.change(|h| {
                    h.pos = v3(n[0], n[1], n[2]);
                    h.yaw = n.get(3).copied().unwrap_or(0.0) as f32;
                    h.pitch = n.get(4).copied().unwrap_or(0.0) as f32;
                });
            }
        }
    }
}

fn arg_n(args: &[Arg], i: usize) -> f64 {
    args.iter()
        .filter(|a| matches!(a, Arg::N(_)))
        .nth(i)
        .map(|a| if let Arg::N(v) = a { *v } else { 0.0 })
        .unwrap_or(0.0)
}
fn arg_e(args: &[Arg], i: usize) -> Ent {
    args.iter()
        .filter_map(|a| if let Arg::E(e) = a { Some(*e) } else { None })
        .nth(i)
        .unwrap_or(Ent::NONE)
}
fn arg_v(args: &[Arg], i: usize) -> V3 {
    args.iter()
        .filter_map(|a| if let Arg::V(v) = a { Some(*v) } else { None })
        .nth(i)
        .unwrap_or(V3::ZERO)
}
fn arg_b(args: &[Arg], i: usize) -> bool {
    args.iter()
        .filter_map(|a| if let Arg::B(v) = a { Some(*v) } else { None })
        .nth(i)
        .unwrap_or(false)
}

/// The helpers set going in a level, by id.
pub fn helpers_in(dimension: &str) -> Vec<u32> {
    with_helpers(|h| {
        let mut v: Vec<u32> = h
            .all
            .iter()
            .filter(|(_, x)| x.spawned && !x.dead && x.dimension == dimension)
            .map(|(id, _)| *id)
            .collect();
        v.sort_unstable();
        v
    })
}

/// Forgets the helpers that are done, and those made but never set going.
pub fn sweep_helpers() {
    with_helpers(|h| h.all.retain(|_, x| x.spawned && !x.dead));
}

/// A potion's 1.7.10 number, as a name.
fn potion(n: i32) -> &'static str {
    const P: [&str; 24] = [
        "",
        "speed",
        "slowness",
        "haste",
        "mining_fatigue",
        "strength",
        "instant_health",
        "instant_damage",
        "jump_boost",
        "nausea",
        "regeneration",
        "resistance",
        "fire_resistance",
        "water_breathing",
        "invisibility",
        "blindness",
        "night_vision",
        "hunger",
        "weakness",
        "poison",
        "wither",
        "health_boost",
        "absorption",
        "saturation",
    ];
    P.get(n as usize).copied().unwrap_or("")
}

fn uuid_text(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

// ---------------------------------------------------------------------------------------------- the level

/// A level as a tick sees it: its shots, and the game.
pub struct World<'h> {
    pub host: &'h mut dyn ServerHost,
    pub dimension: String,
    pub shots: Vec<Shot>,
    pub next_id: u32,
    /// Where shot sounds were asked for this tick.
    pub sounds: Vec<V3>,
    cache: RefCell<HashMap<u32, Ent>>,
    bodies: Option<Vec<(u32, String, [f64; 3])>>,
    pub players: Vec<(u32, u128)>,
}

impl<'h> World<'h> {
    pub fn new(
        host: &'h mut dyn ServerHost,
        dimension: &str,
        shots: Vec<Shot>,
        next_id: u32,
    ) -> World<'h> {
        let players = host
            .casters()
            .iter()
            .filter(|p| p.dimension == dimension)
            .map(|p| (p.entity_id as u32, p.uuid))
            .collect();
        World {
            host,
            dimension: dimension.to_owned(),
            shots,
            next_id,
            sounds: Vec::new(),
            cache: RefCell::new(HashMap::new()),
            bodies: None,
            players,
        }
    }

    /// Its shots, and the next shot id, when the tick is done.
    pub fn finish(self) -> (Vec<Shot>, u32) {
        (self.shots, self.next_id)
    }

    pub fn give_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    /// A living thing, as it is now (cached for the tick).
    pub fn ent(&self, id: u32) -> Ent {
        if id == 0 {
            return Ent::NONE;
        }
        if let Some(e) = self.cache.borrow().get(&id) {
            return *e;
        }
        let e = self.look_up(id);
        self.cache.borrow_mut().insert(id, e);
        e
    }

    pub fn forget(&self, id: u32) {
        self.cache.borrow_mut().remove(&id);
    }

    fn look_up(&self, id: u32) -> Ent {
        let host: &dyn ServerHost = &*self.host;
        let Some(r) = host.entity(id) else {
            return Ent::NONE;
        };
        let Some(p) = r.position(host) else {
            return Ent::NONE;
        };
        let rot = r.rotation(host).unwrap_or([0.0; 2]);
        let b = r.bounding_box(host).unwrap_or([p, p]);
        let kind = r.kind(host).map(|key| key.id(host)).unwrap_or_default();
        let living = host.living(r).is_some();
        let player = kind == "minecraft:player";
        let v = r.velocity(host).unwrap_or([0.0; 3]);
        let passive = crate::passive(&kind);
        let mut health = 20.0;
        if player
            && let Some(pl) = host
                .casters()
                .into_iter()
                .find(|pl| pl.entity_id as u32 == id)
        {
            health = pl.health;
        }
        Ent {
            id,
            present: true,
            helper: 0,
            x: p[0],
            y: p[1],
            z: p[2],
            yaw: rot[0],
            pitch: rot[1],
            eye: r.eye_height(host).unwrap_or(0.0) as f32,
            height: (b[1][1] - b[0][1]) as f32,
            width: (b[1][0] - b[0][0]) as f32,
            dead: !living && kind.is_empty(),
            health,
            max_health: 20.0,
            mx: v[0],
            my: v[1],
            mz: v[2],
            ticks: 1000,
            wet: r.is_in_water(host).unwrap_or(false),
            sneaking: r.is_crouching(host).unwrap_or(false),
            on_ground: r.on_ground(host).unwrap_or(false),
            living,
            player,
            monster: living && !player && !passive,
            passive,
            bx: Bx {
                min: V3::from_array(b[0]),
                max: V3::from_array(b[1]),
            },
            held: Item(""),
        }
    }

    /// The living things of the level, with where they stand.
    fn bodies(&mut self) -> &[(u32, String, [f64; 3])] {
        if self.bodies.is_none() {
            let dim = self.dimension.clone();
            self.bodies = Some(crate::sdk::entities(self.host, &dim));
        }
        self.bodies.as_deref().unwrap_or(&[])
    }

    pub fn helper_ent(&self, id: u32) -> Ent {
        with_helpers(|h| h.all.get(&id).cloned())
            .map(|x| helper_ent(id, &x))
            .unwrap_or(Ent::NONE)
    }

    pub fn shot_index(&self, id: u32) -> Option<usize> {
        self.shots.iter().position(|s| s.id == id && !s.dead)
    }

    fn uuid_of(&mut self, id: u32) -> Option<u128> {
        if let Some((_, u)) = self.players.iter().find(|(p, _)| *p == id) {
            return Some(*u);
        }
        let r = self.host.entity(id)?;
        r.uuid(self.host)
    }

    fn command(&mut self, line: &str) {
        let dim = self.dimension.clone();
        let _ = self
            .host
            .commands()
            .run_command(&format!("execute in {dim} run {line}"));
    }
}

fn helper_ent(id: u32, h: &Helper) -> Ent {
    let (w, ht) = match h.kind {
        "EntityPendulum" => (1.0, 2.0),
        "EntitySakuyaWatch" => (1.0, 1.0),
        "EntityMiniHakkero" | "EntityMiniHakkeroJTG" => (0.4, 0.4),
        _ => (0.2, 0.2),
    };
    let pos = h.pos;
    Ent {
        id: 0x8000_0000 | id,
        present: true,
        helper: id,
        x: pos.x,
        y: pos.y,
        z: pos.z,
        yaw: h.yaw,
        pitch: h.pitch,
        height: ht,
        width: w,
        dead: h.dead,
        health: 1.0,
        max_health: 1.0,
        ticks: h.ticks,
        bx: Bx::around(pos, f64::from(w) / 2.0, f64::from(ht)),
        ..Ent::NONE
    }
}

// ---------------------------------------------------------------------------------------------- Cx

/// What a card (or a shot's script, or a helper) sees: `THSpellCard`'s fields and the world.
pub struct Cx<'a, 'h> {
    pub w: &'a mut World<'h>,
    pub time: i32,
    pub level: i32,
    pub end: i32,
    pub need: i32,
    pub number: i32,
    pub user: Ent,
    pub target: Ent,
    /// Where the card floats.
    pub card: V3,
    pub rand: Rng,
    /// `THKaguyaConfig.danmakuLevel`.
    pub danmaku_level: i32,
    /// Who the shots it fires belong to.
    pub owner: u32,
    pub by_player: bool,
    /// `setEndTime`, if called.
    pub new_end: Option<i32>,
    scratch: Shot,
}

fn blank_shot() -> Shot {
    Shot::new(
        V3::ZERO,
        v3(0.0, 0.0, 1.0),
        0.0,
        v3(0.0, 1.0, 0.0),
        0.0,
        0,
        0.0,
        0.0,
        0.0,
        V3::ZERO,
        shot_full(0, 0, 0.0, 0.0, 0, 0, 0),
    )
}

impl<'a, 'h> Cx<'a, 'h> {
    pub fn new(w: &'a mut World<'h>, user: Ent, target: Ent, card: V3, seed: u64) -> Cx<'a, 'h> {
        Cx {
            w,
            time: 0,
            level: 2,
            end: 0,
            need: 0,
            number: 0,
            owner: user.id,
            by_player: user.player,
            user,
            target,
            card,
            rand: Rng::new(seed),
            danmaku_level: 2,
            new_end: None,
            scratch: blank_shot(),
        }
    }

    // ---- where things are

    pub fn eye_y(e: &Ent) -> f64 {
        let p = f64::from(e.pitch / 180.0 * 3.141_593);
        e.y + f64::from(e.eye) + p.sin() * -0.5 + p.cos() * -0.5
    }
    pub fn pos_user(&self) -> V3 {
        v3(self.user.x, Self::eye_y(&self.user), self.user.z)
    }
    /// `pos_User(angle, distance)`: its height only rises by the direction's own, as written.
    pub fn pos_user_toward(&self, a: V3, distance: f64) -> V3 {
        let a = a.normalize();
        v3(
            self.user.x + a.x * distance,
            Self::eye_y(&self.user) + a.y,
            self.user.z + a.z * distance,
        )
    }
    pub fn pos_target(&self) -> V3 {
        v3(self.target.x, Self::eye_y(&self.target), self.target.z)
    }
    pub fn target_mid_y(&self) -> f64 {
        self.target.y + f64::from(self.target.height) / 2.0
    }
    pub fn mid(&self, e: &Ent) -> V3 {
        v3(e.x, e.y + f64::from(e.height) / 2.0, e.z)
    }
    /// The spell card itself, floating beside its user and looking where they look.
    pub fn card_ent(&self) -> Ent {
        Ent {
            id: 0xFFFF_FFFE,
            present: true,
            x: self.card.x,
            y: self.card.y,
            z: self.card.z,
            yaw: self.user.yaw,
            pitch: self.user.pitch,
            width: 0.5,
            height: 0.5,
            bx: Bx::around(self.card, 0.25, 0.5),
            ..Ent::NONE
        }
    }

    // ---- shots

    pub fn sh(&self, h: SH) -> &Shot {
        match h {
            SH::I(i) if i < self.w.shots.len() => &self.w.shots[i],
            _ => &self.scratch,
        }
    }
    pub fn shm(&mut self, h: SH) -> &mut Shot {
        match h {
            SH::I(i) if i < self.w.shots.len() => {
                let s = &mut self.w.shots[i];
                s.dirty = true;
                s
            }
            _ => {
                self.scratch = blank_shot();
                &mut self.scratch
            }
        }
    }
    pub fn sh_owner(&self, h: SH) -> Ent {
        self.w.ent(self.sh(h).owner)
    }
    /// A shot as an entity: where it is, which way it goes.
    pub fn sh_ent(&self, h: SH) -> Ent {
        let s = self.sh(h);
        let half = f64::from(s.size) / 2.0;
        Ent {
            id: 0x4000_0000 | s.id,
            present: matches!(h, SH::I(_)),
            x: s.pos.x,
            y: s.pos.y,
            z: s.pos.z,
            yaw: s.yaw,
            pitch: s.pitch,
            mx: s.motion.x,
            my: s.motion.y,
            mz: s.motion.z,
            ticks: s.ticks,
            dead: s.dead,
            width: s.size,
            height: s.size,
            bx: Bx {
                min: s.pos - v3(half, half, half),
                max: s.pos + v3(half, half, half),
            },
            ..Ent::NONE
        }
    }
    pub fn sh_target(&self, h: SH) -> Ent {
        self.w.ent(self.sh(h).target)
    }
    pub fn kill_shot(&mut self, h: SH) {
        if let SH::I(i) = h
            && let Some(s) = self.w.shots.get_mut(i)
        {
            s.dead = true;
        }
    }
    fn push(&mut self, mut s: Shot) -> SH {
        s.id = self.w.give_id();
        s.owner = self.owner;
        s.by_player = self.by_player;
        s.card = self.by_player;
        s.dirty = true;
        self.w.shots.push(s);
        SH::I(self.w.shots.len() - 1)
    }
    fn push_all(&mut self, shots: Vec<Shot>) -> SH {
        let mut last = SH::None;
        for s in shots {
            last = self.push(s);
        }
        last
    }
    /// `reCreate`: the same shot again, new.
    pub fn recreate(&mut self, h: SH) -> SH {
        if !matches!(h, SH::I(_)) {
            return SH::None;
        }
        let o = self.sh(h).clone();
        let data = shot_full(o.form, o.color, o.size, o.damage, o.delay, o.end, o.special);
        let s = Shot::new(
            o.pos,
            o.angle,
            o.slope,
            o.rotate,
            o.rot_speed,
            o.rot_end,
            o.first_speed,
            o.limit,
            o.accel,
            o.gravity,
            data,
        );
        let owner = self.owner;
        self.owner = o.owner;
        let r = self.push(s);
        self.owner = owner;
        r
    }

    // ---- entities

    pub fn eh_ent(&self, e: EH) -> Ent {
        match e {
            EH::Ent(e) => e,
            EH::Shot(s) => self.sh_ent(s),
            EH::New(id) => self.w.helper_ent(id),
            EH::Card => self.card_ent(),
            EH::None => Ent::NONE,
        }
    }
    pub fn newe_ent(&self, n: &NewE) -> Ent {
        self.w.helper_ent(n.id)
    }
    pub fn eh_newe(&self, e: EH) -> NewE {
        match e {
            EH::New(id) => NewE { id },
            _ => NewE::default(),
        }
    }
    pub fn eh_is(&self, e: EH, kind: &str) -> bool {
        match e {
            EH::New(id) => with_helpers(|h| h.all.get(&id).is_some_and(|x| x.kind == kind)),
            _ => false,
        }
    }
    pub fn eh_is_laser(&self, e: EH) -> bool {
        matches!(e, EH::Shot(s) if self.sh(s).laser != laser::NONE)
    }
    pub fn same(&self, a: EH, b: EH) -> bool {
        match (a, b) {
            (EH::Shot(x), EH::Shot(y)) => x == y && x != SH::None,
            (EH::Card, EH::Card) => true,
            (EH::None, _) | (_, EH::None) => false,
            _ => self.eh_ent(a).same(&self.eh_ent(b)),
        }
    }
    pub fn target_of(&self, _e: &Ent) -> Ent {
        Ent::NONE
    }
    pub fn owner_of(&self, _e: &Ent) -> Ent {
        Ent::NONE
    }

    /// `getEntitiesWithinAABBExcludingEntity`: living things, shots and helpers within `b`.
    pub fn entities_in(&mut self, b: Bx) -> List {
        let mut out = List::new();
        let ids: Vec<u32> = self
            .w
            .bodies()
            .iter()
            .filter(|(_, _, p)| b.contains(V3::from_array(*p)))
            .map(|(id, _, _)| *id)
            .collect();
        for id in ids {
            let e = self.w.ent(id);
            if e.present {
                out.push(EH::Ent(e));
            }
        }
        for (i, s) in self.w.shots.iter().enumerate() {
            if !s.dead && b.contains(s.pos) {
                out.push(EH::Shot(SH::I(i)));
            }
        }
        for id in helpers_in(&self.w.dimension) {
            let e = self.w.helper_ent(id);
            if b.meets(e.bx) {
                out.push(EH::New(id));
            }
        }
        out
    }

    pub fn kill_eh(&mut self, e: EH) {
        match e {
            EH::Shot(s) => self.kill_shot(s),
            EH::New(id) => NewE { id }.change(|h| h.dead = true),
            EH::Ent(e) => self.kill_ent(e),
            _ => {}
        }
    }
    pub fn kill_ent(&mut self, e: Ent) {
        if e.helper != 0 {
            NewE { id: e.helper }.change(|h| h.dead = true);
        } else if e.present && !e.player {
            let dim = self.w.dimension.clone();
            let _ = self
                .w
                .host
                .hurt_in(&dim, e.id, f32::MAX, "minecraft:generic_kill", None);
        }
    }
    pub fn move_ent(&mut self, e: Ent, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) {
        if e.helper != 0 {
            NewE { id: e.helper }.change(|h| {
                h.pos = v3(x, y, z);
                h.yaw = yaw;
                h.pitch = pitch;
            });
            return;
        }
        if !e.present {
            return;
        }
        if let Some(r) = self.w.host.entity(e.id) {
            let _ = r.teleport_to(self.w.host, [x, y, z], Some([yaw, pitch]));
        }
        self.w.forget(e.id);
    }
    pub fn add_velocity(&mut self, e: Ent, x: f64, y: f64, z: f64) {
        if e.present
            && e.helper == 0
            && let Some(r) = self.w.host.entity(e.id)
        {
            let _ = r.push(self.w.host, [x, y, z]);
        }
    }
    pub fn hurt(&mut self, e: Ent, amount: f32) -> bool {
        if !e.present || e.helper != 0 {
            return false;
        }
        let dim = self.w.dimension.clone();
        let owner = (self.owner != 0).then_some(self.owner);
        let amount = if self.by_player {
            amount * crate::shot::CARD_DAMAGE
        } else {
            amount
        };
        self.w
            .host
            .hurt_in(&dim, e.id, amount, "minecraft:thrown", owner)
    }
    /// `addPotionEffect(new PotionEffect(id, ticks, level))`.
    pub fn effect(&mut self, e: Ent, p: NewE) {
        let Some(h) = p.get() else { return };
        let name = potion(arg_n(&h.args, 0) as i32);
        if name.is_empty() || !e.present || e.helper != 0 {
            return;
        }
        let secs = (arg_n(&h.args, 1) / 20.0).ceil().max(1.0) as i32;
        let amp = arg_n(&h.args, 2) as i32;
        if let Some(u) = self.w.uuid_of(e.id) {
            self.w.command(&format!(
                "effect give {} minecraft:{name} {secs} {amp}",
                uuid_text(u)
            ));
        }
    }
    pub fn clear_effects(&mut self, e: Ent) {
        if e.present
            && e.helper == 0
            && let Some(u) = self.w.uuid_of(e.id)
        {
            self.w.command(&format!("effect clear {}", uuid_text(u)));
        }
    }
    pub fn has_effect(&self, _e: Ent, _p: ()) -> bool {
        false
    }
    pub fn no_effect(&self) -> NewE {
        NewE::default()
    }
    pub fn set_fire(&mut self, e: Ent, secs: i32) {
        if e.present
            && e.helper == 0
            && let Some(u) = self.w.uuid_of(e.id)
        {
            self.w.command(&format!(
                "data merge entity {} {{Fire:{}s}}",
                uuid_text(u),
                secs * 20
            ));
        }
    }

    /// Sets a helper (or a summoned mob, or a shot made with `new`) going.
    pub fn spawn(&mut self, n: NewE) -> bool {
        let Some(h) = n.get() else { return false };
        let a = &h.args;
        match h.kind {
            "EntityMusouFuuin" => {
                // A homing orb that swells as it goes (`EntityMusouFuuin`).
                let size = arg_n(a, 6) as f32;
                let data = shot_full(
                    form::BIG_LIGHT,
                    self.rand.int(7) as u8,
                    0.1,
                    arg_n(a, 7) as f32,
                    0,
                    40,
                    special::MUSOU,
                );
                let mut s = Shot::new(
                    arg_v(a, 0),
                    arg_v(a, 1),
                    arg_n(a, 0) as f32,
                    arg_v(a, 2),
                    arg_n(a, 1) as f32,
                    arg_n(a, 2) as i32,
                    arg_n(a, 3),
                    arg_n(a, 4),
                    arg_n(a, 5),
                    arg_v(a, 3),
                    data,
                );
                s.target = arg_e(a, 2).id;
                s.set_length = f64::from(size);
                self.owner = arg_e(a, 0).id.max(self.owner);
                self.push(s);
                n.change(|h| h.dead = true);
                true
            }
            "EntityNuclearShot" => {
                let data = a
                    .iter()
                    .find_map(|x| if let Arg::S(d) = x { Some(*d) } else { None })
                    .unwrap_or(sd_form(30, 0, 0, 80, 0));
                let s = Shot::new(
                    arg_v(a, 0),
                    arg_v(a, 1),
                    arg_n(a, 0) as f32,
                    arg_v(a, 2),
                    arg_n(a, 1) as f32,
                    arg_n(a, 2) as i32,
                    arg_n(a, 3).max(0.2),
                    arg_n(a, 4).max(0.2),
                    arg_n(a, 5),
                    arg_v(a, 3),
                    data,
                );
                self.push(s);
                n.change(|h| h.dead = true);
                true
            }
            "EntityItem" => {
                let item = a
                    .iter()
                    .find_map(|x| if let Arg::I(i) = x { Some(i.0) } else { None })
                    .unwrap_or("");
                if !item.is_empty() {
                    self.w.command(&format!(
                        "summon item {} {} {} {{Item:{{id:\"{item}\",count:1}}}}",
                        arg_n(a, 0),
                        arg_n(a, 1),
                        arg_n(a, 2)
                    ));
                }
                n.change(|h| h.dead = true);
                true
            }
            "EntityCreeper" | "EntityDanmakuCreeper" | "EntityChicken" => {
                let kind = if h.kind == "EntityChicken" {
                    "chicken"
                } else {
                    "creeper"
                };
                let p = h.pos;
                self.w
                    .command(&format!("summon minecraft:{kind} {} {} {}", p.x, p.y, p.z));
                n.change(|h| h.dead = true);
                true
            }
            "PotionEffect" | "PacketSmoothCamera" => false,
            _ => {
                let user = arg_e(a, 0);
                let dim = self.w.dimension.clone();
                let (by_player, owner) = (
                    self.by_player,
                    if user.present { user.id } else { self.owner },
                );
                n.change(|x| {
                    x.spawned = true;
                    x.dimension = dim;
                    x.user = owner;
                    x.by_player = by_player;
                });
                helper_start(self, n);
                true
            }
        }
    }

    // ---- the world

    /// `rayTraceBlocks`: the first solid block between `a` and `b`.
    pub fn ray(&mut self, a: V3, b: V3) -> Hit {
        let d = b - a;
        let len = d.length();
        if len < 1e-9 {
            return Hit::NONE;
        }
        let Some(level) = self.w.host.level(&self.w.dimension) else {
            return Hit::NONE;
        };
        let steps = (len * 4.0).ceil() as i32;
        let mut last = [a.x.floor() as i32, a.y.floor() as i32, a.z.floor() as i32];
        for i in 0..=steps {
            let p = a + d * (f64::from(i) / f64::from(steps));
            let at = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
            if i > 0 && at == last {
                continue;
            }
            if block_id(self.w.host, level, at).is_some_and(|blk| !crate::passable(&blk)) {
                let side = if at[1] < last[1] {
                    1
                } else if at[1] > last[1] {
                    0
                } else if at[2] < last[2] {
                    3
                } else if at[2] > last[2] {
                    2
                } else if at[0] < last[0] {
                    5
                } else {
                    4
                };
                return Hit {
                    present: true,
                    entity: EH::None,
                    side,
                    at: p,
                    bx: at[0],
                    by: at[1],
                    bz: at[2],
                };
            }
            last = at;
        }
        Hit::NONE
    }
    pub fn block(&self, _x: i32, _y: i32, _z: i32) -> Blk {
        Blk
    }
    /// `Calendar.get`: 2 the month (from 0), 5 the day of the month, 11 the hour (UTC).
    pub fn calendar(&self, field: i32) -> i32 {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let days = secs.div_euclid(86_400);
        // Howard Hinnant's civil-from-days.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        match field {
            2 => (month - 1) as i32,
            5 => day as i32,
            11 => (secs.rem_euclid(86_400) / 3600) as i32,
            _ => 0,
        }
    }
    /// `spellcardplusmodLib.HinaSpellCard`: took Floating Hina Dolls as its price. The doll is not
    /// an item here, so the card's levels are its price.
    pub fn hina_card(&mut self, _n: i32) {}
    pub fn set_end(&mut self, end: i32) {
        self.new_end = Some(end);
        self.end = end;
    }

    /// `homing(level)`: turns a shot up to `level` degrees toward the living thing it best faces
    /// within 24 blocks.
    pub fn homing(&mut self, h: SH, level: f32) {
        self.homing_range(h, 24.0, level)
    }
    pub fn homing_range(&mut self, h: SH, range: f64, level: f32) {
        if !matches!(h, SH::I(_)) {
            return;
        }
        let s = self.sh(h).clone();
        let b = Bx {
            min: s.pos - v3(range, range, range),
            max: s.pos + v3(range, range, range),
        };
        let mut best: Option<(f64, V3)> = None;
        for e in self.entities_in(b) {
            let EH::Ent(e) = e else { continue };
            if !e.living || e.passive || e.id == s.owner || e.dead {
                continue;
            }
            let at = v3(e.x, e.y + f64::from(e.eye), e.z);
            let to = jnorm(at - s.pos);
            let span = angle_between(s.angle, to).abs();
            let value = (at - s.pos).length() * half_abs_sin(span / 180.0 * std::f32::consts::PI);
            if best.is_none_or(|(v, _)| value < v) {
                best = Some((value, to));
            }
        }
        if let Some((_, to)) = best {
            let axis = cross(s.angle, to);
            let a = angle_between(s.angle, to).clamp(-level, level);
            let m = self.shm(h);
            m.angle = turn(axis, m.angle, a);
            m.accelerate_now();
        }
    }

    pub fn play_shot_sound_1(&mut self) {
        let p = self.pos_user();
        self.w.sounds.push(p);
    }

    // ---- makers: `THShotLib.create*`, the user and source dropped

    fn fire(&mut self, f: impl FnOnce(&mut Fire)) -> SH {
        let mut out = Vec::new();
        f(&mut Fire {
            out: &mut out,
            rng: &mut self.rand,
        });
        self.push_all(out)
    }

    pub fn create_shot_13(
        &mut self,
        pos: V3,
        angle: V3,
        slope: f32,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
    ) -> SH {
        self.fire(|f| {
            f.shot_turning(
                pos, angle, slope, rotate, rot_speed, rot_end, first, limit, accel, gravity, d,
            )
        })
    }
    pub fn create_shot_10(
        &mut self,
        pos: V3,
        angle: V3,
        slope: f32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
    ) -> SH {
        self.fire(|f| f.shot(pos, angle, slope, first, limit, accel, gravity, d))
    }
    pub fn create_shot_8(
        &mut self,
        pos: V3,
        angle: V3,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
    ) -> SH {
        self.create_shot_10(pos, angle, 0.0, first, limit, accel, gravity, d)
    }
    pub fn create_shot_5(&mut self, pos: V3, angle: V3, speed: f64, d: SD) -> SH {
        self.create_shot_10(pos, angle, 0.0, speed, speed, 0.0, V3::ZERO, d)
    }
    pub fn create_wide_shot_15(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        wide: f32,
        distance: f64,
        base: f32,
    ) {
        self.fire(|f| {
            f.wide(
                pos, angle, rot_speed, rot_end, first, limit, accel, gravity, d, way, wide,
                distance, base,
            )
        });
    }
    pub fn create_wide_shot_14(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        wide: f32,
        distance: f64,
    ) {
        self.create_wide_shot_15(
            pos, angle, rot_speed, rot_end, first, limit, accel, gravity, d, way, wide, distance,
            0.0,
        )
    }
    pub fn create_wide_shot_12(
        &mut self,
        pos: V3,
        angle: V3,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        wide: f32,
        distance: f64,
    ) {
        self.create_wide_shot_15(
            pos, angle, 0.0, 9999, first, limit, accel, gravity, d, way, wide, distance, 0.0,
        )
    }
    pub fn create_circle_shot_14(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        distance: f64,
        base: f32,
    ) {
        self.fire(|f| {
            f.circle(
                pos, angle, rot_speed, rot_end, first, limit, accel, gravity, d, way, distance,
                base,
            )
        });
    }
    pub fn create_circle_shot_13(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        distance: f64,
    ) {
        self.create_circle_shot_14(
            pos, angle, rot_speed, rot_end, first, limit, accel, gravity, d, way, distance, 0.0,
        )
    }
    pub fn create_circle_shot_12(
        &mut self,
        pos: V3,
        angle: V3,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        distance: f64,
        base: f32,
    ) {
        self.create_circle_shot_14(
            pos, angle, 0.0, 9999, first, limit, accel, gravity, d, way, distance, base,
        )
    }
    pub fn create_ring_shot_15(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        distance: f64,
        span: f32,
        base: f32,
    ) {
        self.fire(|f| {
            f.ring(
                pos, angle, rot_speed, rot_end, first, limit, accel, gravity, d, way, distance,
                span, base,
            )
        });
    }
    pub fn create_random_ring_shot_14(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        num: i32,
        distance: f64,
        span: f32,
    ) {
        self.fire(|f| {
            f.random_ring(
                pos, angle, rot_speed, rot_end, first, limit, accel, gravity, d, num, distance,
                span,
            )
        });
    }
    pub fn create_random_ring_shot_10(
        &mut self,
        pos: V3,
        angle: V3,
        first: f64,
        limit: f64,
        accel: f64,
        d: SD,
        num: i32,
        distance: f64,
        span: f32,
    ) {
        self.create_random_ring_shot_14(
            pos,
            angle,
            0.0,
            9999,
            first,
            limit,
            accel,
            V3::ZERO,
            d,
            num,
            distance,
            span,
        )
    }
    pub fn create_sphere_shot_16(
        &mut self,
        pos: V3,
        angle: V3,
        slope: f32,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        distance: f64,
        base: f32,
    ) {
        self.fire(|f| {
            f.sphere(
                pos,
                angle,
                slope,
                rotate,
                rot_speed,
                rot_end,
                first,
                limit,
                accel,
                gravity,
                d,
                way.max(0) as usize,
                distance,
                base,
            )
        });
    }
    pub fn create_sphere_shot_13(
        &mut self,
        pos: V3,
        angle: V3,
        slope: f32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
        way: i32,
        distance: f64,
        base: f32,
    ) {
        self.create_sphere_shot_16(
            pos,
            angle,
            slope,
            v3(0.0, 1.0, 0.0),
            0.0,
            9999,
            first,
            limit,
            accel,
            gravity,
            d,
            way,
            distance,
            base,
        )
    }
    pub fn create_nuclear_shot_13(
        &mut self,
        pos: V3,
        angle: V3,
        slope: f32,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        d: SD,
    ) -> SH {
        self.create_shot_13(
            pos,
            angle,
            slope,
            rotate,
            rot_speed,
            rot_end,
            first.max(0.2),
            limit.max(0.2),
            accel,
            gravity,
            d,
        )
    }

    fn laser_shot(
        &mut self,
        pos: V3,
        angle: V3,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        l: LD,
        kind: u8,
    ) -> Shot {
        let mut color = l.color;
        if color == 8 {
            color = self.rand.next_int(8);
        } else if color == 9 {
            color = self.rand.next_int(7);
        }
        // The original packs form and colour as `form * 8 + colour`: a colour of 24 is form 3.
        let id = l.form * 8 + color;
        let data = shot_full(
            (id / 8).rem_euclid(32) as u8,
            id.rem_euclid(8) as u8,
            l.width,
            l.damage,
            l.delay,
            l.end,
            l.special,
        );
        let mut s = Shot::new(
            pos, angle, 0.0, rotate, rot_speed, rot_end, first, limit, accel, gravity, data,
        );
        s.laser = kind;
        s.max_length = f64::from(l.length);
        s
    }
    pub fn create_laser_a_9(
        &mut self,
        pos: V3,
        angle: V3,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        l: LD,
    ) -> SH {
        let s = self.laser_shot(
            pos,
            angle,
            v3(0.0, 1.0, 0.0),
            0.0,
            9999,
            first,
            limit,
            accel,
            gravity,
            l,
            laser::FREE,
        );
        self.push(s)
    }
    pub fn create_wide_laser_a_12(
        &mut self,
        pos: V3,
        angle: V3,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        l: LD,
        way: i32,
        wide: f32,
        distance: f64,
    ) {
        let yaw = crate::vec::yaw_of(angle.x, angle.z);
        let pitch = crate::vec::pitch_of(angle.x, angle.y, angle.z);
        let axis = crate::vec::vec_from_angle(-yaw, -pitch + 90.0, 1.0);
        let mut a = -wide / 2.0;
        let span = if way > 1 {
            wide / (way - 1) as f32
        } else {
            0.0
        };
        for _ in 0..way.max(0) {
            let v = turn(axis, angle, a);
            self.create_laser_a_9(pos + v * distance, v, first, limit, accel, gravity, l);
            a += span;
        }
    }
    /// `createLaserB`: a laser held by `set`, `set_length` along its way from it.
    pub fn create_laser_b_11(
        &mut self,
        _setting: EH,
        pos: V3,
        angle: V3,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        l: LD,
        set: EH,
        set_length: f64,
        y_offset: f64,
    ) -> SH {
        let mut s = self.laser_shot(
            pos,
            angle,
            rotate,
            rot_speed,
            rot_end,
            0.0,
            0.0,
            0.0,
            V3::ZERO,
            l,
            laser::SET,
        );
        s.anchor = match set {
            EH::Ent(e) if e.helper != 0 => Anchor::Helper(e.helper),
            EH::Ent(e) => Anchor::Entity(e.id),
            EH::New(id) => Anchor::Helper(id),
            EH::Shot(SH::I(i)) => self
                .w
                .shots
                .get(i)
                .map(|x| Anchor::Shot(x.id))
                .unwrap_or(Anchor::None),
            EH::Card => Anchor::Card(self.card),
            _ => Anchor::None,
        };
        s.set_length = set_length;
        s.y_offset = y_offset;
        self.push(s)
    }
}

/// `THShotLib.getVectorAndVectorAngle`.
fn angle_between(a: V3, b: V3) -> f32 {
    let l = a.length() * b.length();
    if l < 1e-12 {
        return 0.0;
    }
    wrap_deg(((dot(a, b) / l).clamp(-1.0, 1.0).acos() / std::f64::consts::PI * 180.0) as f32)
}

/// `THShotLib.halfAbsSin`.
fn half_abs_sin(a: f32) -> f64 {
    let s = f64::from(a.sin()).abs();
    if a.abs() > std::f32::consts::FRAC_PI_2 {
        2.0 - s
    } else {
        s
    }
}

/// The engine's own scripted specials, beyond the cards'.
pub mod special {
    /// `EntityMusouFuuin`'s orb: swells, slows for fifteen ticks, then homes in.
    pub const MUSOU: i32 = 901;
}

/// One tick of a Musou Fuuin orb.
pub fn musou(cx: &mut Cx, h: SH) {
    let s = cx.sh(h).clone();
    let target = cx.w.ent(s.target);
    let m = cx.shm(h);
    if f64::from(m.size) < m.set_length {
        m.size += 0.1;
    }
    if s.ticks < 15 {
        m.motion = m.motion * 0.8;
        return;
    }
    m.limit = 0.6;
    m.accel = 0.03;
    if !target.present || target.dead {
        m.target = 0;
        return;
    }
    let to = jnorm(v3(
        target.x - s.pos.x,
        target.y + f64::from(target.eye) - s.pos.y,
        target.z - s.pos.z,
    ));
    let a = angle_between(s.angle, to).clamp(-12.0, 12.0);
    m.angle = turn(cross(s.angle, to), m.angle, a);
}

// ---------------------------------------------------------------------------------------------- helpers' ticks

fn ring_point(user: &Ent, yaw: f32, turn_angle: f32, range: f64) -> V3 {
    // `getRotationVectorFromAngle(yaw, 0, angle, range)`: round the user, level.
    let a = f64::from(yaw + turn_angle) / 180.0 * std::f64::consts::PI;
    v3(user.x - a.sin() * range, user.y, user.z + a.cos() * range)
}

/// What a helper does as it is set going.
fn helper_start(cx: &mut Cx, n: NewE) {
    let Some(h) = n.get() else { return };
    let user = cx.w.ent(h.user);
    let a = &h.args;
    match h.kind {
        "EntityMiniHakkero" => {
            // Before its user's eyes, its Master Spark held to it from the start.
            let (y, p) = (
                f64::from(user.yaw / 180.0 * 3.141_593),
                f64::from(user.pitch / 180.0 * 3.141_593),
            );
            let pos = v3(
                user.x - y.sin() * p.cos(),
                user.y - p.sin() + f64::from(user.eye) - 0.1,
                user.z + y.cos() * p.cos(),
            );
            n.change(|x| {
                x.pos = pos;
                x.yaw = user.yaw;
                x.pitch = user.pitch;
            });
            let look = look_angle(user.yaw, user.pitch);
            cx.owner = h.user;
            cx.create_laser_b_11(
                EH::New(n.id),
                pos,
                look,
                v3(0.0, 1.0, 0.0),
                0.0,
                9999,
                ld(0, 24, 4.2, 60.0, 8.0, 30, 120, 0),
                EH::New(n.id),
                1.0,
                0.0,
            );
        }
        "EntityMiniHakkeroJTG" => {
            let look = user.look();
            let pos = v3(
                user.x - look.x,
                user.y - look.y + f64::from(user.eye) - 0.1,
                user.z - look.z,
            );
            n.change(|x| {
                x.pos = pos;
                x.yaw = user.yaw;
                x.pitch = user.pitch;
                x.f = [
                    arg_n(a, 0),
                    arg_n(a, 1),
                    f64::from(u8::from(arg_b(a, 1))),
                    f64::from(u8::from(arg_b(a, 2))),
                    0.0,
                    0.0,
                ];
            });
        }
        "EntityFrostKing" => {
            let (pitch, turn_angle) = (arg_n(a, 0), arg_n(a, 1));
            let pos = ring_point(&user, user.yaw, turn_angle as f32, 1.0) + v3(0.0, 1.0, 0.0);
            n.change(|x| {
                x.pos = pos;
                x.yaw = user.yaw;
                x.f = [turn_angle, pitch, 0.0, 0.0, 0.0, 0.0];
            });
        }
        "EntityPendulum" => {
            let (turn_angle, range) = (arg_n(a, 0), arg_n(a, 1));
            let pos = ring_point(&user, 0.0, turn_angle as f32, range);
            n.change(|x| {
                x.pos = pos;
                x.yaw = user.yaw;
                x.f = [turn_angle, range, 0.0, 0.0, 0.0, 0.0];
            });
        }
        "EntitySakuyaWatch" => {
            let end = if a.iter().filter(|x| matches!(x, Arg::N(_))).count() >= 2 {
                arg_n(a, 1)
            } else {
                60.0
            };
            n.change(|x| {
                x.pos = v3(user.x, user.y + 1.2, user.z);
                x.f = [arg_n(a, 0), end, 0.0, 0.0, 0.0, 0.0];
            });
        }
        "EntitySukima" => {
            let (x, y, z, yaw) = (arg_n(a, 0), arg_n(a, 1), arg_n(a, 2), arg_n(a, 3) as f32);
            n.change(|h| {
                h.pos = v3(x, y, z);
                h.yaw = yaw + 180.0;
                h.f = [arg_n(a, 4), 0.0, 0.0, 0.0, 0.0, 0.0];
            });
        }
        _ => {}
    }
}

/// One tick of every helper of a level.
pub fn tick_helpers(w: &mut World, seed: u64) {
    for id in helpers_in(&w.dimension) {
        let n = NewE { id };
        let Some(h) = n.get() else { continue };
        let user = w.ent(h.user);
        let mut cx = Cx::new(w, user, Ent::NONE, h.pos, seed ^ u64::from(id));
        cx.owner = h.user;
        cx.by_player = h.by_player;
        n.change(|x| x.ticks += 1);
        let t = h.ticks + 1;
        if !user.present || user.dead {
            n.change(|x| x.dead = true);
            continue;
        }
        match h.kind {
            "EntityMiniHakkero" => {
                if t >= 109 {
                    n.change(|x| x.dead = true);
                }
            }
            "EntityMiniHakkeroJTG" => {
                let [delay, end, explode, double, ..] = h.f;
                if f64::from(t) >= delay + end {
                    n.change(|x| x.dead = true);
                } else if t == 1 {
                    let data = ld(
                        0,
                        24,
                        4.2,
                        60.0,
                        8.0,
                        delay as i32,
                        end as i32,
                        if explode > 0.0 { 70 } else { 0 },
                    );
                    let looks = if double > 0.0 {
                        vec![
                            look_angle(h.yaw + 10.0, h.pitch),
                            look_angle(h.yaw - 10.0, h.pitch),
                        ]
                    } else {
                        vec![user.look()]
                    };
                    for look in looks {
                        cx.create_laser_b_11(
                            EH::New(id),
                            h.pos,
                            look,
                            v3(0.0, 1.0, 0.0),
                            0.0,
                            9999,
                            data,
                            EH::New(id),
                            1.0,
                            0.0,
                        );
                    }
                }
            }
            "EntityFrostKing" => {
                let [turn_angle, pitch, ..] = h.f;
                if t >= 160 {
                    n.change(|x| x.dead = true);
                    continue;
                }
                let range = if t <= 20 { f64::from(t) * 0.25 } else { 5.0 };
                let pos = if t <= 20 {
                    ring_point(&user, user.yaw, turn_angle as f32, range) + v3(0.0, 1.0, 0.0)
                } else {
                    h.pos
                };
                n.change(|x| {
                    x.pos = pos;
                    if t <= 20 {
                        x.yaw = user.yaw;
                        x.pitch = user.pitch;
                    }
                });
                if t >= 25 && t % 2 == 0 {
                    let spin = if turn_angle >= 0.0 { -2.0 } else { 2.0 };
                    let dir = look_angle(h.yaw, pitch as f32);
                    cx.create_circle_shot_14(
                        pos,
                        dir,
                        0.0,
                        0,
                        0.5,
                        0.0,
                        0.0,
                        V3::ZERO,
                        sd(10, 1, 0.1, 7.0, 0, 80, 0),
                        2,
                        0.0,
                        turn_angle as f32 + t as f32 * spin,
                    );
                    cx.play_shot_sound_1();
                }
            }
            "EntityPendulum" => {
                let [turn_angle, mut range, ..] = h.f;
                if t >= 200 {
                    n.change(|x| x.dead = true);
                    continue;
                }
                let goal = if user.sneaking { 1.3 } else { 3.0 };
                if range >= goal {
                    range -= 0.1;
                }
                if range <= goal {
                    range += 0.1;
                }
                let pos = ring_point(&user, 0.0, turn_angle as f32, range);
                n.change(|x| {
                    x.pos = pos;
                    x.yaw = user.yaw;
                    x.f[0] = turn_angle - 4.0;
                    x.f[1] = range;
                });
                // It stops others' shots and pushes away what it touches.
                let reach = Bx::around(pos, 0.5, 2.0).expand(0.2, 1.0, 0.2);
                for e in cx.entities_in(reach) {
                    match e {
                        EH::Shot(s) if cx.sh(s).owner != h.user => cx.kill_shot(s),
                        EH::Ent(e) if e.living && e.id != h.user => {
                            let away = jnorm(e.pos() - pos);
                            cx.add_velocity(e, away.x * 0.3, away.y * 0.1, away.z * 0.3);
                        }
                        _ => {}
                    }
                }
            }
            "EntitySakuyaWatch" => {
                let end = h.f[1];
                if f64::from(t) > end || user.sneaking && t > 10 {
                    n.change(|x| x.dead = true);
                    continue;
                }
                n.change(|x| x.pos = v3(user.x, user.y + 1.2, user.z));
                // Time stops for everything within forty blocks but its user: shots wait.
                let around = Bx::around(h.pos, 0.5, 1.0).expand(40.0, 40.0, 40.0);
                for s in cx.w.shots.iter_mut() {
                    if !s.dead && around.contains(s.pos) {
                        s.frozen = 2;
                    }
                }
                for e in cx.entities_in(around) {
                    if let EH::Ent(e) = e
                        && e.living
                        && e.id != h.user
                        && let Some(r) = cx.w.host.entity(e.id)
                    {
                        let _ = r.set_velocity(cx.w.host, [0.0, e.my.min(0.0), 0.0]);
                    }
                }
            }
            "EntitySukima" => {
                let color = h.f[0] as i32;
                if color == 17 {
                    if t == 5 {
                        let dir = look_angle(h.yaw - 180.0, h.pitch);
                        cx.create_laser_a_9(
                            h.pos,
                            dir,
                            0.1,
                            1.4,
                            0.1,
                            V3::ZERO,
                            ld(0, 4, 0.1, 4.0, 8.0, 0, 40, 0),
                        );
                    }
                    if t > 9 {
                        n.change(|x| x.dead = true);
                    }
                } else if t > 24 {
                    n.change(|x| x.dead = true);
                }
            }
            _ => n.change(|x| x.dead = true),
        }
    }
}

/// Where a set laser's holder is, if it still is.
pub fn anchor_pos(w: &World, a: Anchor) -> Option<V3> {
    match a {
        Anchor::None => None,
        Anchor::Entity(id) => {
            let e = w.ent(id);
            (e.present && !e.dead).then(|| e.pos())
        }
        Anchor::Helper(id) => with_helpers(|h| h.all.get(&id).filter(|x| !x.dead).map(|x| x.pos)),
        Anchor::Shot(id) => w.shot_index(id).map(|i| w.shots[i].pos),
        Anchor::Card(p) => Some(p),
    }
}
