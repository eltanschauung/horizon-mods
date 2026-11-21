//! A danmaku bullet: `EntityTHShot`'s state and its tick, without the entity. The server owns every
//! shot and settles what it hits; a client is told each shot once, when it is fired, and runs the
//! same motion itself to draw it, so a pattern of hundreds costs one message, not hundreds of entities.

use crate::vec::{V3, rotate, v3};

/// `THShotLib.SIZE`: each form's diameter, in blocks.
pub const SIZE: [f32; 32] = [
    0.3, 0.15, 0.5, 0.3, 0.3, 0.3, 0.15, 0.3, 0.25, 0.5, 0.15, 0.15, 0.5, 0.15, 0.15, 0.6, 0.5, 1.0, 0.15, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.4, 0.3, 1.0, 0.9, 0.9,
];

/// `THShotLib.DAMAGE`: each form's damage.
pub const DAMAGE: [f32; 32] = [
    2.4, 1.0, 2.8, 5.0, 2.0, 3.4, 1.6, 2.8, 1.6, 3.0, 1.2, 1.8, 3.2, 2.6, 1.0, 6.8, 3.0, 1.0, 3.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0, 2.6, 4.0, 5.0, 5.0,
];

/// The forms, by `DanmakuConstants`' numbers; the rest are drawn and fired as the engine grows.
#[allow(dead_code)]
pub mod form {
    pub const SMALL: u8 = 0;
    pub const TINY: u8 = 1;
    pub const MEDIUM: u8 = 2;
    pub const PEARL: u8 = 3;
    pub const CIRCLE: u8 = 4;
    pub const LIGHT: u8 = 5;
    pub const SCALE: u8 = 6;
    pub const BUTTERFLY: u8 = 7;
    pub const SMALL_STAR: u8 = 8;
    pub const STAR: u8 = 9;
    pub const RICE: u8 = 10;
    pub const CRYSTAL: u8 = 11;
    pub const HEART: u8 = 12;
    pub const KUNAI: u8 = 13;
    pub const TALISMAN: u8 = 14;
    pub const BIG_LIGHT: u8 = 15;
    pub const OVAL: u8 = 16;
    pub const FAMILIAR: u8 = 17;
    pub const ARROW: u8 = 18;
    /// A shot that is never seen and never hits: an emitter for the shots it fires.
    pub const HIDDEN: u8 = 19;
    /// A gap (`EntitySukima`), drawn as one; it never hits.
    pub const GAP: u8 = 20;
    pub const PHANTOM: u8 = 26;
    pub const AMULET: u8 = 27;
    pub const KNIFE: u8 = 28;
    pub const WIND: u8 = 29;
    pub const BIG: u8 = 30;
    pub const KISHITU: u8 = 31;
}

/// Colours 0 to 7, then the two that pick: `RANDOM` one of the eight, `RAINBOW` one of the first seven.
pub const RANDOM: u8 = 8;
pub const RAINBOW: u8 = 9;

/// Special movements, by `DanmakuConstants`' numbers and the spell cards that register their own.
pub mod special {
    /// Falls: two hundredths of a block a tick, every tick.
    pub const FALL: i32 = 30;
    /// Knocks what it hits up and away (Yasaka's Divine Wind).
    pub const WIND: i32 = 180;
    /// Perfect Freeze's frozen shot: still, then off again from its twentieth tick.
    pub const FREEZE: i32 = 900;
    /// Little Bug Storm's: at its end it turns 70 degrees and sets off again (3451 the other way).
    pub const BUG_LEFT: i32 = 3450;
    pub const BUG_RIGHT: i32 = 3451;
    /// Gagouji Cyclone's: at its end it turns at its shooter's target. The server does it.
    pub const SEEK_TARGET: i32 = 4020;
    /// Journey To Gensokyo's seed: at twenty ticks a fast thin light (its laser trail is to come).
    pub const SEED: i32 = 11000;
    /// Bad Fortune's: at ten ticks a fan and four rice shots (30609) or a ring of 36 (30610) burst
    /// from it; at forty it sets off; at a hundred it is gone. The server fires the bursts.
    pub const MISFORTUNE_FAN: i32 = 30609;
    pub const MISFORTUNE_RING: i32 = 30610;
    /// Bad Fortune's mark: at its end a ring of talismans along its shooter's look. The server fires it.
    pub const MISFORTUNE_MARK: i32 = 30611;

    // ---- Touhou Items Mod's spell cards, for the classic bosses ----
    /// Double Black Death Butterfly's: slows to a hover, sets off at 25 ticks, curls ten degrees a
    /// tick until 50 (one way by an even colour, the other by an odd one).
    pub const KOKUSHICHOU: i32 = 500;
    /// Stardust Reverie's big star: it turns two degrees a tick and sheds small stars. The server
    /// sheds them.
    pub const STARDUST: i32 = 1200;
    /// Miracle Fruit's oval: at its end it bursts into eight lines of six. The server bursts it.
    pub const MIRACLE_FRUIT: i32 = 1400;
    /// Youryoku Spoiler's scale: from twenty ticks it flies back at its shooter (1601 once it has
    /// hit). The server steers it.
    pub const SPOILER: i32 = 1600;
    pub const SPOILER_BACK: i32 = 1601;
    /// Icicle Fall's: at its end it turns square (1901 the other way) and falls.
    pub const ICICLE_LEFT: i32 = 1900;
    pub const ICICLE_RIGHT: i32 = 1901;
    /// Meteor on Earth's familiar sheds lights (3500) that set off at thirty ticks (3510). The server
    /// sheds them.
    pub const METEOR: i32 = 3500;
    pub const METEOR_LIGHT: i32 = 3510;
    /// Gagouji Cyclone's arrow: stops, then zigzags at ninety degrees every ten ticks.
    pub const GAGOUJI: i32 = 4000;
    /// Fantasy Seal's orb (`EntityMusouFuuin`): it grows, slows for fifteen ticks, then homes on its
    /// target, twelve degrees a tick at most. The server steers it.
    pub const FANTASY_ORB: i32 = 11110;
    /// Killing Doll's knife, left hanging by the stopped time: the server sends a quarter of them
    /// at the target again.
    pub const KILLING_DOLL: i32 = 800;
    /// Little Bug Storm's: at its end it turns square and darts out (3401), then flies again as a
    /// bug light (3402), blinking, then sets off as a rice shot; 3410 to 3412 turn the other way.
    pub const BUG_STORM: i32 = 3400;
    pub const BUG_STORM_2: i32 = 3410;
    /// Curse of Dreams and Reality's yin-yang orbs: at its end one leaves an emitter (11201) that
    /// sprays shots (11202) which turn at the target at their end; the other (11210) bursts into
    /// spheres. The server fires them.
    pub const CURSE_ORB: i32 = 11200;
    pub const CURSE_EMIT: i32 = 11201;
    pub const CURSE_SEEK: i32 = 11202;
    pub const CURSE_SPLIT: i32 = 11210;
    /// Mesh of Light and Darkness's hanging shot: it sets a laser along its way at its first tick.
    /// The server sets it.
    pub const MESH: i32 = 11300;
    /// Flying-glowworm Nest's gap: at five ticks it fires a laser. The server fires it.
    pub const GAP: i32 = 10017;
}

/// A laser's look, by its form (`LaserData.color / 8`): a plain beam, or Master Spark's.
pub const LASER_SPARK: u8 = 3;

/// `ShotData`: what a pattern asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotData {
    pub form: u8,
    pub color: u8,
    pub size: f32,
    pub damage: f32,
    pub delay: i32,
    pub end: i32,
    pub special: i32,
}

/// `ShotData.shot(form, color, size, damage, delay, end, special)`.
pub fn shot_full(form: u8, color: u8, size: f32, damage: f32, delay: i32, end: i32, special: i32) -> ShotData {
    ShotData { form: form % 32, color: color % 16, size, damage, delay, end, special }
}

/// `ShotData.shot(form, color, delay, end, special)`, with the form's own size and damage.
pub fn shot(form: u8, color: u8, delay: i32, end: i32, special: i32) -> ShotData {
    let f = usize::from(form % 32);
    shot_full(form, color, SIZE[f], DAMAGE[f], delay, end, special)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub id: u32,
    pub pos: V3,
    /// Where it was a tick ago, to draw it between ticks.
    pub prev: V3,
    pub motion: V3,
    /// The way it faces: speeding up and slowing down go along this, gravity does not turn it.
    pub angle: V3,
    /// The axis it turns about, `rot_speed` degrees a tick, until tick `rot_end`.
    pub rotate: V3,
    pub rot_speed: f32,
    pub rot_end: i32,
    pub limit: f64,
    pub accel: f64,
    pub gravity: V3,
    pub form: u8,
    pub color: u8,
    pub size: f32,
    pub damage: f32,
    pub end: i32,
    pub special: i32,
    /// Ticks it waited before it first moved; it waits again if it is fired anew.
    pub delay: i32,
    /// Its tilt about its own course, in degrees, for the shapes that show one.
    pub slope: f32,
    /// Ticks since it was fired (`ticksExisted`).
    pub ticks: i32,
    /// `animationCount`: below zero while it waits out its delay, then its age.
    pub count: i32,
    last_time: i32,
    /// A laser's length now, and the most it reaches; both 0 for a shot that is not a laser.
    pub laser: f32,
    pub laser_max: f32,
    /// A laser held by its owner (`EntityTHSetLaser`): this far out along its way from the owner's
    /// feet, and this far up. The server moves it with the owner and sends it again each tick.
    pub anchor: Option<(f32, f32)>,
    /// Ticks it hangs still in stopped time (Killing Doll), not ageing.
    pub hold: i32,
    /// Drawn in the classic bosses' finer shapes.
    pub fancy: bool,
}

impl Shot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pos: V3,
        angle: V3,
        slope: f32,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        first_speed: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        data: ShotData,
    ) -> Shot {
        Shot {
            id: 0,
            pos,
            prev: pos,
            motion: angle * first_speed,
            angle,
            rotate,
            rot_speed,
            rot_end,
            limit,
            accel,
            gravity,
            form: data.form,
            color: data.color,
            size: data.size,
            damage: data.damage,
            end: data.end,
            special: data.special,
            delay: data.delay,
            slope,
            ticks: 0,
            count: -data.delay,
            last_time: 0,
            laser: 0.0,
            laser_max: 0.0,
            anchor: None,
            hold: 0,
            fancy: false,
        }
    }

    /// Whether it is a laser.
    pub fn is_laser(&self) -> bool {
        self.laser_max > 0.0
    }

    pub fn speed(&self) -> f64 {
        self.motion.length()
    }

    /// Whether it can hit anything yet: not while it is still appearing.
    pub fn solid(&self) -> bool {
        self.count >= 1
    }

    /// One tick of `EntityTHShot.onUpdate`, without the hit test, which the server makes after it
    /// along the way the shot went. False once the shot has had its time.
    pub fn step(&mut self) -> bool {
        if self.hold > 0 {
            self.hold -= 1;
            self.prev = self.pos;
            return true;
        }
        self.ticks += 1;
        self.prev = self.pos;
        if self.count > self.end {
            return false;
        }
        if matches!(self.special, special::MISFORTUNE_FAN | special::MISFORTUNE_RING) && self.ticks >= 100 {
            return false;
        }
        if self.count < 0 {
            self.count += 1;
            self.last_time = self.ticks;
            return true;
        }
        self.count = self.last_time;
        if self.rot_speed != 0.0 {
            self.angle = rotate(self.rotate, self.angle, self.rot_speed);
            self.motion = self.angle * self.speed();
        }
        if self.renew() {
            return true;
        }
        self.accelerate();
        self.special_motion();
        self.motion = self.motion + self.gravity;
        if self.rot_end < self.last_time {
            self.rot_speed = 0.0;
        }
        self.pos = self.pos + self.motion;
        self.last_time = self.ticks;
        // A fired laser (`createLaserA`) grows twice its speed a tick; a held one (`EntityTHSetLaser`),
        // born whole, grows back six blocks a tick after a wall cut it short.
        if self.is_laser() {
            let speed = self.speed() as f32;
            let grow = if speed > 1e-6 { speed * 2.0 } else { 6.0 };
            self.laser = (self.laser + grow).min(self.laser_max);
        }
        true
    }

    /// `setShotRotationYaw`: turned `degrees` about its axis at once, its speed kept.
    pub fn turn(&mut self, degrees: f32) {
        let speed = self.speed();
        self.angle = rotate(self.rotate, self.angle, degrees);
        self.motion = self.angle * speed;
    }

    /// `setSpeed`: `speed` along the way it faces.
    pub fn set_speed(&mut self, speed: f64) {
        self.motion = self.angle * speed;
    }

    /// `shotAcceleration`: towards `limit`, along the way it faces.
    fn accelerate(&mut self) {
        let a = self.angle;
        if self.accel > 0.0 && self.speed() < self.limit {
            self.motion = self.motion + a * self.accel;
            if self.speed() > self.limit {
                self.motion = a * self.limit;
            }
        } else if self.accel < 0.0 && self.speed() > self.limit {
            if self.accel.abs() > self.speed() - self.limit {
                self.motion = a * self.limit;
            } else {
                self.motion = self.motion + a * self.accel;
            }
        }
    }

    /// `reCreate`: fired anew from where it is, at `speed` along the way it faces.
    pub fn restart(&mut self, speed: f64) {
        self.motion = self.angle * speed;
        self.ticks = 0;
        self.count = -self.delay;
        self.last_time = 0;
    }

    /// Little Bug Storm's stages, each ending in `reCreate`.
    fn bug_storm(&mut self) -> bool {
        let s = self.special;
        if self.ticks != self.end || !(special::BUG_STORM..=special::BUG_STORM + 2).contains(&s) && !(special::BUG_STORM_2..=special::BUG_STORM_2 + 2).contains(&s) {
            return false;
        }
        let second = s >= special::BUG_STORM_2;
        match s % 10 {
            0 => {
                self.color = 7;
                self.size = SIZE[0];
                self.special += 1;
                self.end = 20;
                self.angle = rotate(self.rotate, self.angle, 90.0);
                self.limit = 0.0;
                self.accel = -1.0;
                self.restart(3.0);
            }
            1 => {
                self.form = form::LIGHT;
                self.color = 3;
                self.damage = 4.0;
                self.size = SIZE[1];
                self.special += 1;
                self.restart(3.0);
            }
            _ => {
                self.form = form::RICE;
                self.color = if second { 2 } else { 3 };
                self.damage = 6.0;
                self.size = SIZE[10] * 2.0;
                self.special = 0;
                self.angle = rotate(self.rotate, self.angle, if second { 135.0 } else { -45.0 });
                let yaw = crate::vec::yaw_of(self.angle.x, self.angle.z);
                self.angle.y = f64::from((yaw * 23.0 / 180.0 * 3.141593).sin()) * 30.0 * 0.01;
                self.angle = self.angle.normalize();
                self.limit = 0.5;
                self.accel = 0.02;
                self.end = 80;
                self.restart(0.01);
            }
        }
        true
    }

    /// The specials that end a shot by firing it anew from where it is (`reCreate`).
    fn renew(&mut self) -> bool {
        if self.bug_storm() {
            return true;
        }
        let turn = match self.special {
            special::BUG_LEFT => 70.0,
            special::BUG_RIGHT => -70.0,
            _ => return false,
        };
        if self.ticks != self.end {
            return false;
        }
        let speed = self.speed();
        self.special = 0;
        self.end = 80;
        self.angle = rotate(self.rotate, self.angle, turn);
        self.limit = 0.5;
        self.accel = 0.03;
        self.motion = self.angle * speed;
        self.ticks = 0;
        self.count = -self.delay;
        self.last_time = 0;
        true
    }

    fn special_motion(&mut self) {
        match self.special {
            special::FALL => self.motion.y -= 0.02,
            special::MISFORTUNE_FAN | special::MISFORTUNE_RING if self.ticks == 40 => {
                self.limit = 0.5;
                self.accel = 0.1;
            }
            // Night Bird's: at twenty ticks it turns square and sets off (the other way with end 181).
            32501 if self.ticks == 20 => {
                let turn = if self.end == 181 { -90.0 } else { 90.0 };
                self.limit = 0.2;
                self.accel = 0.1;
                self.angle = rotate(self.rotate, self.angle, turn);
            }
            special::SEED if self.ticks == 20 => {
                self.accel = 0.05;
                self.limit = 10.0;
                self.size = 0.1;
                self.form = form::LIGHT;
                self.motion = self.angle * self.speed();
            }
            special::FREEZE if self.ticks >= 20 => {
                self.limit = 0.3;
                self.accel = 0.03;
                self.special = 0;
            }
            special::KOKUSHICHOU => {
                if self.ticks < 25 {
                    self.motion = self.motion * 0.9;
                } else if self.ticks == 25 {
                    self.limit = 2.0;
                    self.accel = 0.03;
                } else if self.ticks < 50 {
                    self.turn(if self.color % 2 == 0 { 10.0 } else { -10.0 });
                }
            }
            special::STARDUST => self.turn(2.0),
            // The bug light blinks, small white and big yellow, every three ticks.
            s if (s == special::BUG_STORM + 2 || s == special::BUG_STORM_2 + 2) && self.ticks % 3 == 0 => {
                if self.color == 3 {
                    self.color = 7;
                    self.size = SIZE[1];
                } else {
                    self.color = 3;
                    self.size = SIZE[1] * 2.0;
                }
            }
            special::ICICLE_LEFT | special::ICICLE_RIGHT
                if (self.special == special::ICICLE_LEFT && self.ticks >= self.end) || self.ticks == self.end =>
            {
                self.turn(if self.special == special::ICICLE_LEFT { 90.0 } else { -90.0 });
                self.end = 120;
                self.gravity.y = -0.03;
                self.accel = 0.01;
                self.limit = 10.0;
                self.motion = self.angle * self.speed();
                self.special = 0;
            }
            special::METEOR_LIGHT if self.ticks == 30 => self.accel = 0.03,
            special::GAGOUJI => {
                if self.ticks == 1 {
                    self.turn(45.0);
                    self.accel = -0.099;
                    self.limit = 0.0;
                }
                if self.ticks % 20 == 9 {
                    self.turn(-90.0);
                    self.set_speed(1.0);
                } else if self.ticks % 20 == 19 {
                    self.turn(90.0);
                    self.set_speed(1.0);
                }
            }
            special::FANTASY_ORB => {
                // Its growing: the server's steering does the rest.
                if self.size < 2.0 {
                    self.size = (self.size + 0.1).min(2.0);
                }
                if self.ticks < 15 {
                    self.motion = self.motion * 0.8;
                }
            }
            _ => {}
        }
    }

    /// Its damage, as `getShotDamageWithDifficulty` gives a mob's: a player's shot does its own.
    pub fn damage_for(&self, by_player: bool, difficulty: &str) -> f32 {
        if by_player {
            return self.damage;
        }
        match difficulty {
            "easy" => self.damage * 0.7,
            "hard" => self.damage * 1.5,
            _ => self.damage,
        }
    }

    // ---- On the wire: a client is told a shot once, as it is fired ----

    pub const WIRE: usize = 4 + 3 * 8 + 3 * 4 * 5 + 4 * 8 + 2 + 4 * 2 + 4 * 2 + 4 + 1;

    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend(self.id.to_le_bytes());
        for v in self.pos.array() {
            out.extend(v.to_le_bytes());
        }
        for v in [self.motion, self.angle, self.rotate, self.gravity] {
            for c in v.array() {
                out.extend((c as f32).to_le_bytes());
            }
        }
        for v in [v3(self.limit, self.accel, 0.0)] {
            out.extend((v.x as f32).to_le_bytes());
            out.extend((v.y as f32).to_le_bytes());
            out.extend(0f32.to_le_bytes());
        }
        out.extend(self.rot_speed.to_le_bytes());
        out.extend(self.rot_end.to_le_bytes());
        out.extend(self.size.to_le_bytes());
        out.extend(self.slope.to_le_bytes());
        out.extend(self.end.to_le_bytes());
        out.extend(self.special.to_le_bytes());
        out.extend(self.ticks.to_le_bytes());
        out.extend(self.count.to_le_bytes());
        out.extend([self.form, self.color]);
        out.extend(self.last_time.to_le_bytes());
        out.extend(self.delay.to_le_bytes());
        out.extend(self.laser.to_le_bytes());
        out.extend(self.laser_max.to_le_bytes());
        out.extend(self.hold.to_le_bytes());
        out.push(u8::from(self.fancy));
    }

    pub fn decode(data: &[u8]) -> Option<(Shot, &[u8])> {
        let mut r = Reader(data);
        let id = r.u32()?;
        let pos = v3(r.f64()?, r.f64()?, r.f64()?);
        let mut vecs = [V3::ZERO; 4];
        for v in &mut vecs {
            *v = v3(f64::from(r.f32()?), f64::from(r.f32()?), f64::from(r.f32()?));
        }
        let (limit, accel, _) = (f64::from(r.f32()?), f64::from(r.f32()?), r.f32()?);
        let rot_speed = r.f32()?;
        let rot_end = r.i32()?;
        let size = r.f32()?;
        let slope = r.f32()?;
        let end = r.i32()?;
        let special = r.i32()?;
        let ticks = r.i32()?;
        let count = r.i32()?;
        let form = r.u8()?;
        let color = r.u8()?;
        let last_time = r.i32()?;
        let delay = r.i32()?;
        let (laser, laser_max) = (r.f32()?, r.f32()?);
        let hold = r.i32()?;
        let fancy = r.u8()? != 0;
        let [motion, angle, rotate, gravity] = vecs;
        let shot = Shot {
            id,
            pos,
            prev: pos,
            motion,
            angle,
            rotate,
            rot_speed,
            rot_end,
            limit,
            accel,
            gravity,
            form,
            color,
            size,
            damage: 0.0,
            end,
            special,
            delay,
            slope,
            ticks,
            count,
            last_time,
            laser,
            laser_max,
            anchor: None,
            hold,
            fancy,
        };
        Some((shot, r.0))
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take().map(f32::from_le_bytes)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }
}

/// What the server tells clients: shots fired, and shots gone before their time.
#[derive(Debug, Default, PartialEq)]
pub struct Batch {
    pub fired: Vec<Shot>,
    pub gone: Vec<u32>,
}

impl Batch {
    /// Messages of at most `limit` bytes each.
    pub fn encode(&self, limit: usize) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut message = vec![0u8];
        for shot in &self.fired {
            if message.len() + 1 + Shot::WIRE + 16 > limit {
                out.push(std::mem::replace(&mut message, vec![0u8]));
            }
            message.push(1);
            shot.encode(&mut message);
        }
        for id in &self.gone {
            if message.len() + 5 > limit {
                out.push(std::mem::replace(&mut message, vec![0u8]));
            }
            message.push(2);
            message.extend(id.to_le_bytes());
        }
        if message.len() > 1 {
            out.push(message);
        }
        out
    }

    pub fn decode(mut data: &[u8]) -> Batch {
        let mut batch = Batch::default();
        if data.first() != Some(&0) {
            return batch;
        }
        data = &data[1..];
        while let Some((&kind, rest)) = data.split_first() {
            match kind {
                1 => match Shot::decode(rest) {
                    Some((shot, rest)) => {
                        batch.fired.push(shot);
                        data = rest;
                    }
                    None => break,
                },
                2 => match rest.split_first_chunk::<4>() {
                    Some((id, rest)) => {
                        batch.gone.push(u32::from_le_bytes(*id));
                        data = rest;
                    }
                    None => break,
                },
                _ => break,
            }
        }
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::angle;

    fn plain(speed: f64, limit: f64, accel: f64, data: ShotData) -> Shot {
        Shot::new(V3::ZERO, angle(0.0, 0.0), 0.0, v3(0.0, 1.0, 0.0), 0.0, 9999, speed, limit, accel, V3::ZERO, data)
    }

    #[test]
    fn a_shot_waits_out_its_delay_then_flies() {
        let mut s = plain(0.5, 0.5, 0.0, shot(form::SMALL, 0, 3, 80, 0));
        for _ in 0..3 {
            assert!(s.step());
            assert_eq!(s.pos, V3::ZERO);
            assert!(!s.solid());
        }
        assert!(s.step());
        assert!((s.pos.z - 0.5).abs() < 1e-9);
    }

    #[test]
    fn it_speeds_up_to_its_limit_and_no_further() {
        let mut s = plain(0.2, 0.7, 0.1, shot(form::SMALL, 0, 0, 80, 0));
        for _ in 0..20 {
            s.step();
        }
        assert!((s.speed() - 0.7).abs() < 1e-6, "{}", s.speed());
    }

    #[test]
    fn it_is_gone_after_its_end() {
        let mut s = plain(0.1, 0.1, 0.0, shot(form::SMALL, 0, 0, 10, 0));
        let lived = (0..100).take_while(|_| s.step()).count();
        assert_eq!(lived, 12);
    }

    #[test]
    fn a_frozen_shot_sets_off_again_at_twenty_ticks() {
        let mut s = plain(1e-4, 1e-4, 0.0, shot_full(form::CIRCLE, 7, 0.3, 4.0, 0, 120, special::FREEZE));
        for _ in 0..19 {
            s.step();
        }
        assert!(s.speed() < 1e-3);
        for _ in 0..12 {
            s.step();
        }
        assert_eq!(s.special, 0);
        assert!((s.speed() - 0.3).abs() < 1e-6, "{}", s.speed());
    }

    #[test]
    fn a_turning_shot_keeps_its_speed() {
        let mut s =
            Shot::new(V3::ZERO, angle(0.0, 0.0), 0.0, v3(0.0, 1.0, 0.0), 10.0, 9999, 0.3, 0.3, 0.0, V3::ZERO, shot(0, 0, 0, 80, 0));
        for _ in 0..9 {
            s.step();
        }
        assert!((s.speed() - 0.3).abs() < 1e-6);
        assert!((s.angle.x - 1.0).abs() < 1e-6, "{:?}", s.angle);
    }

    #[test]
    fn shots_cross_the_wire_unchanged_enough_to_fly_the_same() {
        let mut s = plain(0.4, 0.9, 0.05, shot(form::STAR, 3, 2, 60, special::FALL));
        s.id = 77;
        s.step();
        let batch = Batch { fired: vec![s.clone()], gone: vec![5, 6] };
        let messages = batch.encode(30_000);
        assert_eq!(messages.len(), 1);
        let back = Batch::decode(&messages[0]);
        assert_eq!(back.gone, vec![5, 6]);
        let mut theirs = back.fired[0].clone();
        assert_eq!(theirs.id, 77);
        for _ in 0..40 {
            s.step();
            theirs.step();
        }
        assert!((s.pos - theirs.pos).length() < 1e-3, "{:?} {:?}", s.pos, theirs.pos);
    }

    #[test]
    fn a_fired_laser_grows_twice_its_speed_a_tick_to_its_length() {
        let mut s = plain(0.5, 0.5, 0.0, shot(form::SMALL, 0, 0, 80, 0));
        s.laser_max = 4.0;
        s.step();
        assert!((s.laser - 1.0).abs() < 1e-6);
        for _ in 0..10 {
            s.step();
        }
        assert_eq!(s.laser, 4.0);
        let batch = Batch { fired: vec![s.clone()], gone: vec![] };
        let back = Batch::decode(&batch.encode(30_000)[0]);
        assert_eq!((back.fired[0].laser, back.fired[0].laser_max), (4.0, 4.0));
    }

    #[test]
    fn a_held_shot_waits_out_its_hold_then_goes_on() {
        let mut s = plain(0.5, 0.5, 0.0, shot(form::SMALL, 0, 0, 80, 0));
        s.step();
        let (at, ticks) = (s.pos, s.ticks);
        s.hold = 3;
        for _ in 0..3 {
            assert!(s.step());
        }
        assert_eq!((s.pos, s.ticks), (at, ticks));
        s.step();
        assert!(s.pos != at);
    }

    #[test]
    fn a_bug_storm_shot_goes_through_its_three_stages() {
        let mut s = plain(1.0, 0.0, -0.05, shot_full(form::TINY, 7, SIZE[1], 2.0, 0, 50, special::BUG_STORM));
        for _ in 0..50 {
            s.step();
        }
        assert_eq!((s.special, s.end, s.ticks), (special::BUG_STORM + 1, 20, 0));
        for _ in 0..20 {
            s.step();
        }
        assert_eq!((s.special, s.form), (special::BUG_STORM + 2, form::LIGHT));
        for _ in 0..20 {
            s.step();
        }
        assert_eq!((s.special, s.form, s.end), (0, form::RICE, 80));
        assert!(s.angle.length().is_finite());
    }

    #[test]
    fn icicles_turn_square_and_fall_at_their_end() {
        let mut s = plain(1.0, 0.0, -0.1, shot_full(form::CRYSTAL, 1, 0.5, 6.0, 0, 5, special::ICICLE_LEFT));
        for _ in 0..5 {
            s.step();
        }
        assert_eq!(s.special, 0);
        assert_eq!(s.end, 120);
        assert!(s.gravity.y < 0.0);
    }

    #[test]
    fn a_big_batch_is_split_into_messages_that_fit() {
        let fired: Vec<Shot> = (0..1000).map(|i| {
            let mut s = plain(0.1, 0.1, 0.0, shot(0, 0, 0, 80, 0));
            s.id = i;
            s
        }).collect();
        let messages = Batch { fired, gone: vec![] }.encode(30_000);
        assert!(messages.iter().all(|m| m.len() <= 30_000));
        let total: usize = messages.iter().map(|m| Batch::decode(m).fired.len()).sum();
        assert_eq!(total, 1000);
    }
}
