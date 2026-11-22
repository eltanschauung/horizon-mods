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
}

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

/// What a laser is: none, one that grows as it flies (`EntityTHLaser`), or one held by something
/// (`EntityTHSetLaser`), which the server keeps where its holder is.
pub mod laser {
    pub const NONE: u8 = 0;
    pub const FREE: u8 = 1;
    pub const SET: u8 = 2;
}

/// What a set laser is held by.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Anchor {
    #[default]
    None,
    /// A living thing, by entity id.
    Entity(u32),
    /// One of the things a card set going (`NewE`), by its id.
    Helper(u32),
    /// Another shot, by id.
    Shot(u32),
    /// The spell card itself: where it floats.
    Card(V3),
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
    /// Its tilt about its own course, in degrees, for the shapes that show one.
    pub slope: f32,
    /// Ticks since it was fired (`ticksExisted`).
    pub ticks: i32,
    /// `animationCount`: below zero while it waits out its delay, then its age.
    pub count: i32,
    pub last_time: i32,
    /// `rotationYaw` and `rotationPitch`, as the shot's own `updateAngle` keeps them.
    pub yaw: f32,
    pub pitch: f32,
    /// `shotSpeed`: the speed it was fired at, or last set to.
    pub first_speed: f64,
    pub delay: i32,
    /// A laser's kind, and its length now and at most.
    pub laser: u8,
    pub length: f64,
    pub max_length: f64,
    // ---- The server's own: never sent ----
    /// Its user, by entity id.
    pub owner: u32,
    pub by_player: bool,
    /// Fired by a player's spell card: half damage.
    pub card: bool,
    /// Gone at the end of this tick (`setDead`).
    pub dead: bool,
    /// Changed by a card's script: clients must be told again.
    pub dirty: bool,
    /// What a set laser is held by, how far along its way from it, and how far above.
    pub anchor: Anchor,
    pub set_length: f64,
    pub y_offset: f64,
    /// What a homing shot follows (`EntityMusouFuuin.target`), by entity id.
    pub target: u32,
    /// Ticks it stays still for, in stopped time (Sakuya's watch).
    pub frozen: u8,
}

/// What a tick's first half found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Its time is up.
    Gone,
    /// Still appearing: it does not move or hit.
    Waiting,
    /// It moves: a script may steer it before it does.
    Moving,
}

/// What a player's spell card does of its damage.
pub const CARD_DAMAGE: f32 = 0.5;

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
        let mut s = Shot {
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
            slope,
            ticks: 0,
            count: -data.delay,
            last_time: 0,
            yaw: 0.0,
            pitch: 0.0,
            first_speed,
            delay: data.delay,
            laser: laser::NONE,
            length: 0.0,
            max_length: 0.0,
            owner: 0,
            by_player: false,
            card: false,
            dead: false,
            dirty: false,
            anchor: Anchor::None,
            set_length: 0.0,
            y_offset: 0.0,
            target: 0,
            frozen: 0,
        };
        s.face_angle();
        s
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
        match self.begin() {
            Phase::Gone => false,
            Phase::Waiting => true,
            Phase::Moving => {
                self.special_motion();
                self.finish();
                true
            }
        }
    }

    /// The tick up to its special movement: turning and speeding up.
    pub fn begin(&mut self) -> Phase {
        self.ticks += 1;
        self.prev = self.pos;
        if self.dead || self.count > self.end {
            return Phase::Gone;
        }
        if self.count < 0 {
            self.count += 1;
            self.last_time = self.ticks;
            return Phase::Waiting;
        }
        self.count = self.last_time;
        if self.rot_speed != 0.0 {
            self.set_rotation_yaw(self.rot_speed);
        }
        self.face_motion();
        self.accelerate();
        Phase::Moving
    }

    /// The tick after its special movement: gravity, then the move.
    pub fn finish(&mut self) {
        self.motion = self.motion + self.gravity;
        if self.rot_end < self.last_time {
            self.rot_speed = 0.0;
        }
        self.pos = self.pos + self.motion;
        self.last_time = self.ticks;
        match self.laser {
            laser::FREE => self.length = (self.length + self.speed() * 2.0).min(self.max_length),
            laser::SET => self.length = (self.length + 6.0).min(self.max_length),
            _ => {}
        }
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

    /// `shotAcceleration()`, called by a script.
    pub fn accelerate_now(&mut self) {
        self.accelerate();
    }

    /// The special movements the engine itself knows; a card's own are run by the server.
    pub fn special_motion(&mut self) {
        match self.special {
            special::FALL => self.motion.y -= 0.02,
            special::FREEZE if self.ticks >= 20 => {
                self.limit = 0.3;
                self.accel = 0.03;
                self.special = 0;
            }
            _ => {}
        }
    }

    /// `updateAngle`: yaw and pitch from the way it moves.
    fn face_motion(&mut self) {
        let m = self.motion;
        if m.x == 0.0 && m.y == 0.0 && m.z == 0.0 {
            return;
        }
        let f = (m.x * m.x + m.z * m.z).sqrt();
        self.yaw = (m.x.atan2(m.z) * 180.0 / std::f64::consts::PI) as f32;
        self.pitch = (m.y.atan2(f) * 180.0 / std::f64::consts::PI) as f32;
    }

    /// `updateYawAndPitch`: yaw and pitch from the way it faces.
    fn face_angle(&mut self) {
        let a = self.angle;
        let f = (a.x * a.x + a.z * a.z).sqrt();
        self.yaw = (a.x.atan2(a.z) * 180.0 / std::f64::consts::PI) as f32;
        self.pitch = (a.y.atan2(f) * 180.0 / std::f64::consts::PI) as f32;
    }

    pub fn update_angle(&mut self) {
        self.face_motion();
    }

    /// `setShotRotationYaw`: turns its facing about its axis, and its motion with it.
    pub fn set_rotation_yaw(&mut self, degrees: f32) {
        self.angle = rotate(self.rotate, self.angle, degrees);
        self.update_motion();
    }

    /// `updateMotion`: its speed, along the way it faces.
    pub fn update_motion(&mut self) {
        let speed = self.speed();
        self.motion = self.angle * speed;
    }

    /// `setVector`: faces the way its yaw and pitch say.
    pub fn set_vector(&mut self) {
        let (y, p) = (f64::from(self.yaw / 180.0 * std::f32::consts::PI), f64::from(self.pitch / 180.0 * std::f32::consts::PI));
        self.angle = v3(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
        self.update_motion();
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.first_speed = speed;
        self.motion = self.angle * speed;
    }

    pub fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.pos = v3(x, y, z);
    }

    /// `bound`, in short: back the way it came, slowed, on the axis it struck along. Its count of
    /// bounces is not kept.
    pub fn bound(&mut self, rate: f64, _count: i32) {
        let m = self.motion;
        let (ax, ay, az) = (m.x.abs(), m.y.abs(), m.z.abs());
        if ay >= ax && ay >= az {
            self.motion.y *= -rate;
        } else if ax >= az {
            self.motion.x *= -rate;
        } else {
            self.motion.z *= -rate;
        }
        self.angle = self.motion.normalize();
    }

    /// Its damage, as `getShotDamageWithDifficulty` gives a mob's: a player's shot does its own.
    pub fn damage_for(&self, by_player: bool, difficulty: &str) -> f32 {
        if by_player {
            return if self.card { self.damage * CARD_DAMAGE } else { self.damage };
        }
        match difficulty {
            "easy" => self.damage * 0.7,
            "hard" => self.damage * 1.5,
            _ => self.damage,
        }
    }

    // ---- On the wire: a client is told a shot once, as it is fired ----

    pub const WIRE: usize = 4 + 3 * 8 + 3 * 4 * 5 + 4 * 8 + 3 + 4 * 3;

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
            out.extend((self.length as f32).to_le_bytes());
        }
        out.extend(self.rot_speed.to_le_bytes());
        out.extend(self.rot_end.to_le_bytes());
        out.extend(self.size.to_le_bytes());
        out.extend(self.slope.to_le_bytes());
        out.extend(self.end.to_le_bytes());
        out.extend(self.special.to_le_bytes());
        out.extend(self.ticks.to_le_bytes());
        out.extend(self.count.to_le_bytes());
        out.extend([self.form, self.color, self.laser]);
        out.extend(self.last_time.to_le_bytes());
        out.extend((self.max_length as f32).to_le_bytes());
        out.extend(self.yaw.to_le_bytes());
    }

    pub fn decode(data: &[u8]) -> Option<(Shot, &[u8])> {
        let mut r = Reader(data);
        let id = r.u32()?;
        let pos = v3(r.f64()?, r.f64()?, r.f64()?);
        let mut vecs = [V3::ZERO; 4];
        for v in &mut vecs {
            *v = v3(f64::from(r.f32()?), f64::from(r.f32()?), f64::from(r.f32()?));
        }
        let (limit, accel, length) = (f64::from(r.f32()?), f64::from(r.f32()?), f64::from(r.f32()?));
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
        let laser = r.u8()?;
        let last_time = r.i32()?;
        let max_length = f64::from(r.f32()?);
        let yaw = r.f32()?;
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
            slope,
            ticks,
            count,
            last_time,
            yaw,
            pitch: 0.0,
            first_speed: 0.0,
            delay: 0,
            laser,
            length,
            max_length,
            owner: 0,
            by_player: false,
            card: false,
            dead: false,
            dirty: false,
            anchor: Anchor::None,
            set_length: 0.0,
            y_offset: 0.0,
            target: 0,
            frozen: 0,
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
