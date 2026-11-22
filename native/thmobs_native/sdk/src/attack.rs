//! Each mob's attacks, `danmakuPattern` and its helpers ported line for line. `Atk` gives them the
//! words the Java used: the attack counter `c`, the danmaku `level`, `pos()`, `look()`, the shot
//! helpers' overloads, `move_right` and the rest, and `use_spell_card`.

use crate::mobs::{self, Pattern};
use crate::pattern::{Fire, rotate_default};
use crate::shot::{SIZE, ShotData, shot, shot_full, special};
use crate::vec::{V3, angle, angle_random, rotation_vector_from_angle, vec_from_angle};

/// What an attack sees and may change, each tick it runs.
pub struct Atk<'a> {
    /// `attackCounter`.
    pub c: i32,
    /// `THKaguyaConfig.danmakuLevel`.
    pub level: i32,
    /// `getDanmakuPattern()`: the boss's phase, or a fairy's pattern number.
    pub phase: i32,
    /// `pos()`: a little below and before the eyes.
    pub pos: V3,
    pub feet: V3,
    pub yaw: f32,
    pub pitch: f32,
    /// `ticksExisted`.
    pub ticks: i32,
    /// The target's middle, when there is one.
    pub target: Option<V3>,
    pub fire: Fire<'a>,
    /// `danmakuSpan`: for the fairy kind, the counter goes back to 0 past it.
    pub span: i32,
    /// The registry pattern a fairy was born with.
    pub pattern: Pattern,
    /// A glide asked for: way, speed, ticks.
    pub glide: Option<(V3, f64, i32)>,
    /// `setStopStart` / `setStopEnd`.
    pub stop: Option<bool>,
    /// A spell card to declare.
    pub card: Option<u32>,
    pub sound: bool,
    /// `isWet`: in water or rain.
    pub wet: bool,
    /// What a mob keeps between ticks (Cirno's `random`).
    pub mem: &'a mut [i32; 4],
}

impl Atk<'_> {
    pub fn look(&self) -> V3 {
        angle(self.yaw, self.pitch)
    }
    pub fn angle(&self, yaw: f32, pitch: f32) -> V3 {
        angle(yaw, pitch)
    }
    pub fn angle_to_target(&self) -> Option<V3> {
        self.target.map(|t| (t - self.pos).normalize())
    }
    pub fn play_shot_sound(&mut self) {
        self.sound = true;
    }
    pub fn use_spell_card(&mut self, n: u32) {
        self.card = Some(n);
    }
    pub fn rf(&mut self) -> f32 {
        self.fire.rng.float()
    }
    pub fn rd(&mut self) -> f64 {
        self.fire.rng.double()
    }
    pub fn ri(&mut self, n: u32) -> u32 {
        self.fire.rng.int(n)
    }

    fn glide_to(&mut self, yaw: f32, speed: f64, time: i32) {
        self.glide = Some((vec_from_angle(yaw, self.pitch, 1.0), speed, time));
    }
    pub fn move_right(&mut self, speed: f64, time: i32) {
        self.glide_to(self.yaw + 90.0, speed, time);
    }
    pub fn move_left(&mut self, speed: f64, time: i32) {
        self.glide_to(self.yaw - 90.0, speed, time);
    }
    pub fn move_forward(&mut self, speed: f64, time: i32) {
        self.glide_to(self.yaw, speed, time);
    }
    pub fn move_back(&mut self, speed: f64, time: i32) {
        self.glide_to(self.yaw + 180.0, speed, time);
    }

    // ---- `THShotLib`'s overloads ----

    /// `createShot(user, pos, angle, speed, shot)`.
    pub fn shot(&mut self, pos: V3, a: V3, speed: f64, d: ShotData) {
        self.fire.shot(pos, a, 0.0, speed, speed, 0.0, V3::ZERO, d);
    }
    /// `createShot(user, pos, angle, first, limit, accel, shot)`.
    pub fn shot_acc(&mut self, pos: V3, a: V3, first: f64, limit: f64, accel: f64, d: ShotData) {
        self.fire.shot(pos, a, 0.0, first, limit, accel, V3::ZERO, d);
    }
    /// `createWideShot(user, pos, angle, speed, shot, way, wide)`.
    pub fn wide(&mut self, pos: V3, a: V3, speed: f64, d: ShotData, way: i32, wide: f32) {
        self.fire.wide(pos, a, 0.0, 9999, speed, speed, 0.0, V3::ZERO, d, way, wide, 0.0, 0.0);
    }
    /// `createWideShot(user, pos, angle, first, limit, accel, shot, way, wide)`.
    #[allow(clippy::too_many_arguments)]
    pub fn wide_acc(&mut self, pos: V3, a: V3, first: f64, limit: f64, accel: f64, d: ShotData, way: i32, wide: f32) {
        self.fire.wide(pos, a, 0.0, 9999, first, limit, accel, V3::ZERO, d, way, wide, 0.0, 0.0);
    }
    /// `createWideShot(user, source, pos, angle, first, limit, accel, gravity, shot, way, wide, distance)`.
    #[allow(clippy::too_many_arguments)]
    pub fn wide_far(&mut self, pos: V3, a: V3, first: f64, limit: f64, accel: f64, d: ShotData, way: i32, wide: f32, distance: f64) {
        self.fire.wide(pos, a, 0.0, 9999, first, limit, accel, V3::ZERO, d, way, wide, distance, 0.0);
    }
    /// `createCircleShot(user, pos, angle, speed, shot, way)`.
    pub fn circle(&mut self, pos: V3, a: V3, speed: f64, d: ShotData, way: i32) {
        self.fire.circle_simple(pos, a, speed, d, way);
    }
    /// `createCircleShot(user, pos, angle, first, limit, accel, shot, way)`.
    #[allow(clippy::too_many_arguments)]
    pub fn circle_acc(&mut self, pos: V3, a: V3, first: f64, limit: f64, accel: f64, d: ShotData, way: i32) {
        self.fire.circle(pos, a, 0.0, 9999, first, limit, accel, V3::ZERO, d, way, f64::from(d.size), 0.0);
    }
    /// `createRandomRingShot(user, pos, angle, speed, shot, num, span)`.
    pub fn random_ring(&mut self, pos: V3, a: V3, speed: f64, d: ShotData, num: i32, span: f32) {
        self.fire.random_ring(pos, a, 0.0, 9999, speed, speed, 0.0, V3::ZERO, d, num, 0.0, span);
    }
    /// `createSphereShot(user, pos, angle, speed, shot, way, base)`.
    pub fn sphere(&mut self, pos: V3, a: V3, speed: f64, d: ShotData, way: usize, base: f32) {
        self.fire.sphere(pos, a, 0.0, rotate_default(), 0.0, 9999, speed, speed, 0.0, V3::ZERO, d, way, 0.0, base);
    }
}

// ---- `ShotData.shot`'s overloads ----

/// `shot(form, color)`.
pub fn s2(form: u8, color: u8) -> ShotData {
    shot(form, color, 0, 80, 0)
}
/// `shot(form, color, delay)`.
pub fn s3(form: u8, color: u8, delay: i32) -> ShotData {
    shot(form, color, delay, 80, 0)
}
/// `shot(form, color, delay, end)`.
pub fn s4(form: u8, color: u8, delay: i32, end: i32) -> ShotData {
    shot(form, color, delay, end, 0)
}
/// `shot(form, color, size, damage)`.
pub fn sd(form: u8, color: u8, size: f32, damage: f32) -> ShotData {
    shot_full(form, color, size, damage, 0, 80, 0)
}
/// `shot(form, color, size, damage, delay, end)`.
pub fn sde(form: u8, color: u8, size: f32, damage: f32, delay: i32, end: i32) -> ShotData {
    shot_full(form, color, size, damage, delay, end, 0)
}

/// A mob's attack, run each tick it has its target in sight.
pub type Attack = fn(&mut Atk);

// ---- Touhou Items Mod ----

/// `EntityDanmakuCreeper`: close by, it hisses for thirty ticks, then bursts into firework
/// danmaku and waits three hundred ticks; it doesn't die of it.
pub fn hanabeeper(a: &mut Atk) {
    let near = a.target.is_some_and(|t| (t - a.feet).length() < 3.5);
    if a.mem[0] < 0 {
        a.mem[0] += 1;
        return;
    }
    if near || a.mem[0] > 0 {
        a.mem[0] += 1;
    }
    if a.mem[0] >= 30 {
        a.mem[0] = -300;
        let level = a.level;
        let size = 0.1 + level as f32 * 0.1;
        let way = [8, 12, 20, 32, 44][level as usize];
        let dir = angle_random(a.fire.rng);
        for (speed, color) in [(0.6, 2), (0.5, 0), (0.4, 3)] {
            let d = shot_full(5, color, size, 5.0, 0, 80, 0);
            a.fire.sphere(a.pos, dir, 0.0, rotate_default(), 0.0, 9999, speed, 0.01, 0.0, crate::pattern::gravity(-0.02), d, way, 0.0, 0.0);
        }
        a.play_shot_sound();
    }
}

/// No attack: the people of Gensokyo who don't fight.
pub fn none(_: &mut Atk) {}

/// `EntityTHFairy.danmakuPattern`: the registry pattern it was born with.
pub fn fairy(a: &mut Atk) {
    a.span = a.pattern.span;
    if a.c == 0 {
        a.play_shot_sound();
        mobs::fairy_volley(a.pattern, a.pos, a.look(), &mut a.fire);
    }
}

/// A Nether fairy of the zombie kind: slow, close, a light now and then.
pub fn zombie_fairy(a: &mut Atk) {
    a.pattern = Pattern { code: 0, span: 50 - a.level * 10, speed: 0.2, shot: 40 };
    fairy(a);
}

/// A Nether fairy: half are the ordinary kind, half the zombie kind.
pub fn nether_fairy(a: &mut Atk) {
    if a.phase == 2 { zombie_fairy(a) } else { fairy(a) }
}

pub fn cirno(a: &mut Atk) {
    let ang = a.angle(a.yaw, a.pitch);
    let level = a.level;
    let lf = f64::from(level);
    match a.phase {
        1 => {
            if a.c == 40 {
                let look = a.look();
                for i in 0..7 {
                    let s = shot_full(11, 5, 0.2, 4.0, i, 120, 0);
                    let base = a.rf() * 360.0;
                    a.fire.ring(a.pos, look, 0.0, 9999, 0.6 + lf * 0.15, 0.6 + lf * 0.15, 0.1, V3::ZERO, s, i + 2, 0.0, (i * level) as f32, base);
                }
                a.play_shot_sound();
            }
            if a.c >= 50 {
                a.c = 0;
            }
        }
        101 => {
            if a.c == 1 {
                a.use_spell_card(19);
            } else if a.c > 30 && a.c < 170 && a.c % 17 == 0 && level >= 2 {
                let size = if level >= 3 { SIZE[2] } else { SIZE[0] };
                a.fire.wide(a.pos, ang, 0.0, 9999, 0.3 + lf * 0.1, 0.5, 1.0, V3::ZERO, sde(0, 3, size, 4.0, 3, 80), 3, 70.0, 0.2, 0.0);
                a.play_shot_sound();
            } else if a.c >= 240 {
                a.c = 0;
            }
        }
        2 => {
            if a.c < 50 && a.c % 6 == 0 {
                a.circle(a.pos, ang, 0.2, s2(1, 7), 5 + level * 2);
                a.circle(a.pos, ang, 0.3 + lf * 0.1, s2(4, 1), 5 + level * 2);
                a.play_shot_sound();
            }
            // At 55, 65 and 75 a fan of three lasers: to come with the lasers.
            if a.c > 128 {
                a.c = 0;
            }
        }
        102 => {
            if a.c == 1 {
                a.mem[0] = a.ri(2) as i32;
                a.use_spell_card(9);
            } else if (70..=90).contains(&a.c) && a.c % 3 == 0 {
                let way = 4 + a.mem[0];
                for i in 0..3 + level {
                    a.fire.wide(a.pos, ang, 0.0, 9999, 0.3 + lf * 0.1, 0.5, 1.0, V3::ZERO, sde(4, 1, 0.3, 4.0, i, 80), way, 70.0, 0.2, 0.0);
                }
                a.play_shot_sound();
            } else if a.c >= 170 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

pub fn reimu(a: &mut Atk) {
    if a.phase == 101 {
        if a.c == 1 {
            a.use_spell_card(0);
        } else if a.c >= 140 {
            a.c = 0;
        }
    }
}

pub fn rumia(a: &mut Atk) {
    let level = a.level;
    let lf = f64::from(level);
    match a.phase {
        1 => {
            let ang = a.angle(a.yaw, a.pitch);
            if a.c == 1 || a.c == 14 {
                a.play_shot_sound();
                let color = if a.c == 1 { 1 } else { 0 };
                for i in 0..1 + level {
                    a.circle_acc(a.pos, ang, 2.0, 0.4, -0.3, s3(4, color, 10 + i), 12 + 3 * level);
                }
            } else if a.c == 24 {
                a.move_right(0.3, 20);
            } else if (62..67).contains(&a.c) {
                a.play_shot_sound();
                let i = a.c - 62;
                let turned = a.angle(a.yaw + i as f32 * 5.0, a.pitch);
                a.circle(a.pos, turned, 0.2 + lf * 0.1, s2(1, (a.c % 7) as u8), 5 + level * 2);
            } else if a.c == 67 {
                a.move_left(0.3, 20);
            }
            if a.c >= 127 {
                a.c = 0;
            }
        }
        101 => {
            if a.c == 1 {
                a.use_spell_card(28);
            } else if a.c < 60 && a.c % 9 == 0 {
                let ang = a.angle(a.yaw, a.pitch);
                a.circle(a.pos, ang, 0.1 + lf * 0.1, s2(1, 7), 15 + level * 3);
                a.play_shot_sound();
            } else if a.c >= 120 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

/// `THShotLib.pos_Random`: a point a unit out from `pos`, any way. (`range` was never used.)
fn pos_random(a: &mut Atk, pos: V3) -> V3 {
    pos + angle_random(a.fire.rng)
}

pub fn toziko(a: &mut Atk) {
    match a.phase {
        1 => {
            if a.c % 80 < 20 {
                a.play_shot_sound();
                for _ in 0..1 + a.level {
                    let at = pos_random(a, a.pos);
                    let dir = angle_random(a.fire.rng);
                    a.fire.shot(at, dir, 0.0, 0.3, 0.0, -0.03, V3::ZERO, shot(18, 1, 1, 20, special::SEEK_TARGET));
                }
            } else if a.c == 24 {
                let speed = a.rd() * 0.3 + 0.2;
                if a.ri(2) == 0 {
                    a.move_right(speed, 16);
                } else {
                    a.move_left(speed, 16);
                }
            }
            if a.c >= 80 {
                a.c = 0;
            }
        }
        101 => {
            if a.c == 1 {
                a.use_spell_card(40);
            } else if a.c >= 200 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

pub fn wriggle(a: &mut Atk) {
    let level = a.level;
    let lf = f64::from(level);
    match a.phase {
        1 => {
            let look = a.look();
            if a.c < 30 {
                let k = (a.c % 8) as f32 * 10.0;
                if a.c % 15 >= 1 && a.c % 15 < 8 {
                    let dir = rotation_vector_from_angle(a.yaw, a.pitch, k, 1.0);
                    a.play_shot_sound();
                    a.shot_acc(a.pos, dir, 1.0, 0.0, -0.2, shot(10, 3, 3, 15, special::BUG_LEFT));
                    if level >= 3 {
                        a.shot_acc(a.pos, dir, 1.0, 0.0, -0.4, shot(10, 3, 3, 15, special::BUG_LEFT));
                    }
                } else if a.c % 15 >= 8 {
                    let dir = rotation_vector_from_angle(a.yaw, a.pitch, -k, 1.0);
                    a.play_shot_sound();
                    a.shot_acc(a.pos, dir, 1.0, 0.0, -0.2, shot(10, 5, 3, 15, special::BUG_RIGHT));
                    if level >= 3 {
                        a.shot_acc(a.pos, dir, 1.0, 0.0, -0.4, shot(10, 5, 3, 15, special::BUG_RIGHT));
                    }
                }
            } else if a.c == 24 {
                a.move_right(0.3, 20);
            } else if (62..72).contains(&a.c) {
                a.play_shot_sound();
                let accel = f64::from(-(a.c - 61)) * 0.005;
                a.wide_acc(a.pos, look, 0.3 + 0.1 * lf, 0.3, accel, s3(0, 3, 10), 5, 90.0);
                // The original's move left at 67 sits behind this branch, so it never comes.
            }
            if a.c >= 147 {
                a.c = 0;
            }
        }
        101 => {
            if a.c % 15 == 1 && a.c < 120 {
                let (speed, way) = match level {
                    2 => (0.2, 16),
                    3 => (0.3, 24),
                    4 => (0.4, 32),
                    _ => (0.0, 0),
                };
                if way > 0 {
                    let look = a.look();
                    a.sphere(a.pos, look, speed, s2(2, 1), way, a.ticks as f32 * 13.0);
                }
            }
            if a.c == 1 {
                a.use_spell_card(35);
            } else if a.c >= 180 {
                a.c = 0;
            }
        }
        102 => {
            if a.c == 1 {
                a.use_spell_card(34);
            } else if a.c >= 260 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

// ---- Journey To Gensokyo ----

/// `angle_LimitRandom(angle(yaw, pitch), danmakuAccuracy)`.
fn aimed(a: &mut Atk) -> V3 {
    let look = a.look();
    mobs::aim(look, mobs::ACCURACY, a.fire.rng)
}

fn fan_rows(a: &mut Atk, base: V3, d: ShotData, speed: f64, num: i32, rows: &[f32]) {
    for (i, tilt) in rows.iter().enumerate() {
        let dir = if i == 0 { base } else { a.angle(a.yaw, a.pitch + tilt) };
        a.wide(a.pos, dir, speed, d, num, 20.0);
    }
}

fn level_span(level: i32) -> i32 {
    match level {
        1 => 30,
        2 => 20,
        3 => 15,
        4 => 10,
        _ => 20,
    }
}

/// `EntityTHFairyEnd`: its form, 1 to 3, picks the volley.
pub fn fairy_end(a: &mut Atk) {
    let ang = aimed(a);
    let level = a.level;
    match a.phase {
        1 => {
            a.span = 30;
            let d = shot(13, 1, 0, 80, 0);
            let speed = 0.4;
            let num = if level <= 2 { 3 } else { 1 };
            if a.c == 0 {
                fan_rows(a, ang, d, speed, num, &[0.0, 5.0, -5.0]);
                a.play_shot_sound();
            }
            if level == 3 && (a.c == 0 || a.c == 10 || a.c == 20) {
                fan_rows(a, ang, d, speed, 5, &[0.0, 5.0, 10.0, -5.0, -10.0]);
                a.play_shot_sound();
            }
            if level == 4 && a.c <= 4 {
                fan_rows(a, ang, d, speed, 7, &[0.0, 5.0, 10.0, -5.0, -10.0]);
                a.play_shot_sound();
            }
        }
        2 | 3 => {
            a.span = level_span(level);
            let d = shot(11, 0, 0, 80, special::SEED);
            let num = match level {
                1 => 1,
                2 => 3,
                _ => 5,
            };
            if a.c == 0 {
                a.play_shot_sound();
                if a.phase == 2 {
                    if level != 4 {
                        a.wide(a.pos, ang, 0.1, d, num, 20.0);
                    } else {
                        fan_rows(a, ang, d, 0.1, num, &[0.0, 10.0, -10.0]);
                    }
                } else {
                    a.fire.random_ring(a.pos, ang, 0.0, 9999, 0.1, 0.1, 0.0, V3::ZERO, d, num, 0.75, 0.0);
                }
            }
        }
        _ => {}
    }
}

pub fn sunflower_fairy_end(a: &mut Atk) {
    if a.phase == 1 {
        let ang = aimed(a);
        a.span = 10;
        if a.c == 0 {
            a.play_shot_sound();
            let level = a.level;
            a.fire.random_ring(a.pos, ang, 0.0, 0, 0.2, 0.4, 0.05, V3::ZERO, shot(9, 8, 0, 80, 0), level, 0.5, 20.0);
        }
    } else {
        fairy_end(a);
    }
}

fn crow_moves(a: &mut Atk, accurate: V3) {
    if a.c == 0 {
        let r = a.ri(4);
        if r == 0 {
            let s = a.rd() * 0.5 + 0.5;
            a.move_right(s, 12);
        } else if r == 1 {
            let s = a.rd() * 0.5 + 0.5;
            a.move_left(s, 12);
        } else if r == 2 {
            let s = a.rd() * 0.1 + 0.1;
            a.move_forward(s, 12);
        } else if a.ri(2) == 0 {
            a.shot(a.pos, accurate, f64::from(0.9f32), sd(29, 0, 0.8, 1.0));
            let s = a.rd() * 0.2 + 3.0;
            a.move_forward(s, 20);
            a.play_shot_sound();
        }
    }
}

pub fn tengu_crow(a: &mut Atk) {
    let accurate = a.look();
    let ang = aimed(a);
    let level = a.level;
    a.span = 12;
    let d = shot(0, 3, 0, 80, 0);
    if a.phase == 1 {
        let p = 10;
        let (mut span, mut num) = ((p as f32 + 1.0) * (level / 2) as f32, (p / 100 + 1) * (level / 2));
        if level == 1 {
            span = 1.0;
            num = 1;
        }
        if a.c % 6 == 0 {
            a.random_ring(a.pos, ang, 0.3, d, num, span);
            a.play_shot_sound();
        }
    } else if a.c % 8 == 0 {
        a.random_ring(a.pos, ang, 0.3, d, 1 + level, 90.0);
        a.play_shot_sound();
    }
    crow_moves(a, accurate);
    // Toned down: its dashes at half their speed.
    if let Some(g) = a.glide.as_mut() {
        g.1 *= 0.5;
    }
}

pub fn hell_raven(a: &mut Atk) {
    let level = a.level;
    if a.phase == 1 {
        a.span = 81;
        let ang = a.look();
        let speed = 0.25 * f64::from(level as f32);
        let d = shot(13, 1, 0, 60, 0);
        if (2..=60).contains(&a.c) {
            a.play_shot_sound();
            let times = if level == 1 { 2 } else { 1 };
            for _ in 0..times {
                a.sphere(a.pos, ang, speed, d, 6, 0.0);
            }
            if level >= 4 {
                a.fire.sphere(a.pos, ang, 0.0, rotate_default(), 0.0, 9999, speed, speed, 0.0, V3::ZERO, d, 6, 0.5, 0.0);
            }
        }
        if a.c == 1 {
            let s = a.rd() * 0.3 + 0.3;
            if a.ri(2) == 0 {
                a.move_right(s, 32);
            } else {
                a.move_left(s, 32);
            }
        }
    } else {
        a.span = 45;
        let ang = aimed(a);
        let d = shot(0, 1, 0, 80, 0);
        if a.c == 10 || a.c == 20 || a.c == 30 {
            a.play_shot_sound();
            for i in 0..1 + level {
                let i = f64::from(i);
                a.shot_acc(a.pos, ang, 0.5 * (1.0 + i / 2.0), 0.5 * (i / 2.0), 0.0, d);
            }
        }
        if a.c == 31 {
            let s = a.rd() * 0.5 + 0.3;
            if a.ri(2) == 0 {
                a.move_right(s, 20);
            } else {
                a.move_left(s, 20);
            }
        }
    }
}

// ---- SPELLCARD PLUS ----

/// Off at 70 degrees to one side or the other (`moveVec = getVecFromAngle(yaw ± 70, pitch)`).
fn dash_aside(a: &mut Atk, speed: f64, time: i32) {
    let side = if a.ri(2) == 0 { -1.0 } else { 1.0 };
    a.glide = Some((vec_from_angle(a.yaw + 70.0 * side, a.pitch, 1.0), speed, time));
}

/// Wakasagihime's two-way scale fans.
fn scale_fans(a: &mut Atk, base1: f32, base2: f32, end: i32, wide_rate: f32, way_rate: f64, first1: f64) {
    let level = a.level;
    let look = a.look();
    let d = shot_full(6, 1, SIZE[6], level as f32 + 2.0, 0, end, 0);
    let way = (f64::from(level) * way_rate) as i32;
    let wide = (20 * level) as f32 * wide_rate;
    a.fire.wide(a.pos, look, 0.0, 9999, first1, 9999.0, 0.0, V3::ZERO, d, way, wide, 0.0, base1);
    if first1 < 0.55 {
        a.fire.wide(a.pos, look, 0.0, 9999, 0.0, 0.5, 0.05, V3::ZERO, d, way, wide, 0.0, base2);
    }
    a.play_shot_sound();
}

fn scale_rain(a: &mut Atk, end: i32) {
    let level = a.level;
    if a.c > 100 && a.c % 10 / level == 0 {
        let look = a.look();
        let d = shot_full(0, 1, SIZE[0], level as f32 + 2.0, 0, end, 0);
        a.fire.random_ring(a.pos, look, 0.0, 9999, 0.2 + f64::from(level) * 0.2, 0.0, 0.0, V3::ZERO, d, 1, 0.0, 90.0);
        a.play_shot_sound();
    }
}

pub fn wakasagihime(a: &mut Atk) {
    let level = a.level;
    let c = a.c;
    match a.phase {
        1 => {
            if c > 30 && c < 43 {
                scale_fans(a, (c * 15 - 180) as f32, (c * 15) as f32 - 217.5, 80, 1.0, 1.0, 0.5);
            }
            if c == 101 || c == 201 {
                dash_aside(a, 0.3, if a.wet { 90 } else { 20 });
            }
            if c > 130 && c < 143 {
                scale_fans(a, (c * 15 - 240) as f32, (c * 15) as f32 - 277.5, 80, 1.0, 1.0, 0.5);
            }
            if c > 230 && c < 243 {
                scale_fans(a, (c * -15 + 300) as f32, (c * -15) as f32 + 337.5, 80, 1.0, 1.0, 0.5);
            }
            scale_rain(a, 80);
            if a.c == 90 - (level - 1) * 15 {
                a.c = 200;
            } else if a.c >= 290 - (level - 1) * 15 {
                a.c = 100;
            } else if a.c == 190 - (level - 1) * 15 {
                a.c = 200;
            }
        }
        2 => {
            if c < 40 {
                scale_fans(a, (c * 10 - 60) as f32, 0.0, 100, 4.5, 1.5, 0.6);
            }
            if c == 101 || c == 201 {
                dash_aside(a, 0.3, if a.wet { 99 } else { 20 });
            }
            if c > 130 && c < 170 {
                scale_fans(a, (c * 10 - 310) as f32, 0.0, 100, 4.5, 1.5, 0.6);
            }
            if c > 230 && c < 270 {
                scale_fans(a, (c * -10 + 300) as f32, 0.0, 100, 4.5, 1.5, 0.6);
            }
            scale_rain(a, 120);
            if a.c == 80 - (level - 1) * 7 {
                a.c = 200;
            } else if a.c >= 299 - (level - 1) * 7 {
                a.c = 100;
            } else if a.c == 199 - (level - 1) * 7 {
                a.c = 200;
            }
        }
        101 => {
            if a.wet {
                if c % 30 == 0 {
                    dash_aside(a, 0.6, 30);
                }
            } else if c == 1 {
                dash_aside(a, 0.6, 20);
            }
            if c == 30 {
                a.use_spell_card(if level <= 2 { 300 } else { 304 });
            } else if c >= 100 {
                a.c = 0;
            }
        }
        102 => {
            if c == 1 {
                a.use_spell_card(match level {
                    ..=2 => 301,
                    3 => 302,
                    _ => 305,
                });
            }
            if a.wet {
                if c % 30 == 0 {
                    dash_aside(a, 0.6, 30);
                }
            } else if [40, 70, 100, 130].contains(&c) {
                dash_aside(a, 0.3, 10);
            }
            if (level == 3 && c >= 250) || (level != 3 && c >= 120) {
                a.c = 0;
            }
        }
        103 => {
            if c == 1 {
                a.use_spell_card(303);
            }
            if a.wet {
                if c % 30 == 0 {
                    dash_aside(a, 1.0, 30);
                }
            } else if c == 40 {
                dash_aside(a, 0.6, 20);
            }
            if c >= 140 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

/// Hina's turning marks, two a tick: each a still talisman that bursts at its end.
fn hina_marks(a: &mut Atk) {
    let c = a.c;
    let dmg = 2.0 + a.level as f32;
    let look = a.look();
    let bases = if (200..=273).contains(&c) && c % 2 == 0 {
        Some(((c * -5) as f32, (-180 + c * -5) as f32))
    } else if c % 2 == 0 && c <= 73 {
        Some(((c * 5) as f32, (c * 5 + 180) as f32))
    } else {
        None
    };
    if let Some((b0, b1)) = bases {
        for (color, base) in [(0, b0), (1, b1)] {
            let d = shot_full(31, color, 0.1, dmg, 0, 1, special::MISFORTUNE_MARK);
            a.fire.circle(a.pos, look, 0.0, 0, 0.0, 0.0, 0.0, V3::ZERO, d, 1, 2.0, base);
        }
    }
    if c == 73 || c == 273 {
        dash_aside(a, 0.6, 30);
    }
    if a.c >= 300 {
        a.c = 0;
    } else if (100..=199).contains(&a.c) {
        a.c = 200;
    }
}

pub fn hina(a: &mut Atk) {
    let level = a.level;
    let dmg = 2.0 + level as f32;
    let ang = a.angle(a.yaw, a.pitch);
    match a.phase {
        1 => {
            let color = [0, 1, 2, 3][a.ri(4) as usize];
            let sp = if a.ri(2) == 0 { special::MISFORTUNE_FAN } else { special::MISFORTUNE_RING };
            let d = shot_full(30, color, 0.5, dmg, 0, 30609, sp);
            if a.c == 1 || a.c == 41 {
                let side = if a.c == 1 { 60.0 } else { -60.0 };
                a.glide = Some((vec_from_angle(a.yaw + side, a.pitch, 1.0), 1.0, 50));
                a.shot(a.pos, ang, 0.0, d);
            }
            if a.c >= 80 {
                a.c = 0;
            }
        }
        2 => {
            if a.c % 30 == 0 {
                let look = a.look();
                a.fire.circle(a.pos, look, 0.0, 0, 0.0, 0.5, 0.5, V3::ZERO, shot_full(31, 7, 0.1, dmg, 0, 60, 0), 8, 0.0, 0.0);
            }
            hina_marks(a);
        }
        3 => {
            if a.c % 40 == 0 {
                let look = a.look();
                for delay in [0, 2, 4, 6, 8] {
                    let base = a.ri(3) as f32;
                    a.fire.circle(a.pos, look, 0.0, 0, 0.0, 0.5, 0.5, V3::ZERO, shot_full(10, 0, 0.1, dmg, delay, 60, 0), 18, 0.0, base);
                }
            }
            hina_marks(a);
        }
        101 | 102 | 103 | 104 => {
            let n = (a.phase - 101) as usize;
            if a.c == 1 {
                let cards = match level {
                    ..=2 => [306, 307, 308, 309],
                    3 => [310, 311, 314, 315],
                    _ => [312, 313, 314, 315],
                };
                a.use_spell_card(cards[n]);
            }
            let (dash_at, end) = [(70, 100), (100, 140), (100, 160), (-1, 200)][n];
            if a.c == dash_at {
                dash_aside(a, 0.6, 30);
            }
            if a.c >= end {
                a.c = 0;
            }
        }
        _ => {}
    }
}

/// SPELLCARD PLUS's Cirno: Touhou Items Mod's at normal, with a third card after them.
pub fn cirno_plus(a: &mut Atk) {
    if a.phase != 103 {
        cirno(a);
        return;
    }
    let ang = a.angle(a.yaw, a.pitch);
    if a.level == 3 {
        if a.c == 1 {
            a.use_spell_card(336);
        }
        return;
    }
    if a.c % 5 == 0 {
        a.fire.random_ring(a.pos, ang, 0.0, 0, 0.5, 0.5, 0.0, V3::ZERO, shot_full(11, 1, 0.1, 7.0, 10, 100, 0), 1, 0.0, 30.0);
    }
    if a.c == 1 {
        a.use_spell_card(330);
    }
    if a.c % 30 == 0 {
        let side = if a.ri(2) == 0 { -1.0 } else { 1.0 };
        let turn = 60.0 + a.ri(60) as f32;
        a.glide = Some((vec_from_angle(a.yaw + turn * side, a.pitch, 1.0), 0.3, 20));
    } else if a.c >= 140 {
        a.c = 0;
    }
}

/// Off at 90 degrees to one side or the other.
fn dash_square(a: &mut Atk, speed: f64, time: i32) {
    let side = if a.ri(2) == 0 { -1.0 } else { 1.0 };
    a.glide = Some((vec_from_angle(a.yaw + 90.0 * side, a.pitch, 1.0), speed, time));
}

/// SPELLCARD PLUS's Rumia: Touhou Items Mod's first two phases, and four more.
pub fn rumia_plus(a: &mut Atk) {
    let level = a.level;
    let lv = level as f32;
    let c = a.c;
    match a.phase {
        1 | 101 => rumia(a),
        2 => {
            if c == 1 || c == 40 {
                dash_square(a, 0.5, 20);
            }
            if (5..=20).contains(&c) && c % 3 == 0 {
                a.play_shot_sound();
                let look = a.look();
                for i in 0..10 {
                    let d = shot_full(2, 0, SIZE[2], lv + 2.0, i + i, 80, 0);
                    a.fire.wide(a.pos, look, 0.0, 0, 1.5 - f64::from(i) * 0.1, 0.3, -0.08, V3::ZERO, d, level, (level * 5) as f32, 0.0, 0.0);
                }
            }
            if c == 60 {
                a.play_shot_sound();
                let look = a.look();
                for delay in [0, 10, 20, 30] {
                    let d = shot_full(0, 1, SIZE[0], lv, delay, 100, 0);
                    a.fire.circle(a.pos, look, 0.0, 0, 1.0, 1.0, 0.0, V3::ZERO, d, 6 + level, 0.0, (30 - level * 3) as f32);
                }
                for delay in [0, 10, 20, 30] {
                    let d = shot_full(2, 1, SIZE[2], lv + 2.0, delay, 100, 0);
                    a.fire.circle(a.pos, look, 0.0, 0, 0.7, 0.7, 0.0, V3::ZERO, d, 6 + level, 0.0, 0.0);
                }
            }
            if a.c >= 120 {
                a.c = 0;
            }
        }
        3 => {
            let look = a.look();
            let ang = a.angle(a.yaw, a.pitch);
            if c == 1 {
                dash_square(a, 0.5, 30);
            }
            if c == 10 {
                a.play_shot_sound();
                for i in 0..8 {
                    let d = shot_full(2, 5, SIZE[2], lv + 2.0, i * 3, 80, 0);
                    a.fire.wide(a.pos, look, 0.0, 0, 0.5, 0.0, 0.0, V3::ZERO, d, 1 + (level - 1) * 2, (level * 15) as f32, 0.0, 0.0);
                }
            }
            if (10..=27).contains(&c) && c % 3 == 0 {
                // These left lasers behind them; the lasers are to come.
                a.shot(a.pos, look, f64::from(0.001f32), shot_full(31, 1, 0.3, lv + 2.0, 0, 80, 32500));
            }
            if c == 95 {
                dash_square(a, 0.5, 20);
            }
            let ring = |a: &mut Atk, speed: f64, d: ShotData, times: i32| {
                a.play_shot_sound();
                for _ in 0..times {
                    a.circle_acc(a.pos, ang, speed, speed, 0.0, d, 28);
                }
            };
            if c == 120 || (c == 125 && level >= 4) {
                ring(a, 0.5, sde(16, 2, 0.3, lv + 2.0, 10, 80), 1 + level);
            }
            if c == 130 || (c == 135 && level >= 4) {
                ring(a, 0.7, sde(1, 2, 0.1, lv, 10, 80), 1);
            }
            if c == 140 || (c == 145 && level >= 4) {
                ring(a, 0.7, sde(16, 2, 0.3, lv + 2.0, 10, 80), 1);
            }
            if a.c >= 170 {
                a.c = 0;
            }
        }
        102 => {
            if c == 1 || (c == 5 && level >= 3) || (c == 10 && level >= 4) {
                a.use_spell_card(325);
            }
            if c == 140 {
                dash_square(a, 1.0, 20);
            } else if c >= 150 {
                a.c = 0;
            }
        }
        103 => {
            let ang = a.angle(a.yaw, a.pitch);
            if c == 1 {
                dash_square(a, 0.5, 20);
            }
            for (at, color) in [(25, 1), (45, 2), (65, 0)] {
                if c == at {
                    a.play_shot_sound();
                    let accels: &[f64] = if level >= 4 { &[-0.15, -0.1] } else { &[-0.15] };
                    for &acc in accels {
                        for (end, base) in [(180, 5.0), (181, -5.0)] {
                            let d = shot_full(10, color, 0.1, lv, 10, end, 32501);
                            a.fire.circle(a.pos, ang, 0.0, 0, 1.0, 0.001, acc, V3::ZERO, d, 14 + level, 0.0, base);
                        }
                    }
                }
            }
            if c == 70 {
                dash_square(a, 0.3, 40);
                a.use_spell_card(326);
            } else if c >= 120 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

// ---- Journey To Gensokyo's bosses ----

/// `playerNear`: a fan of small stars at a player within four blocks.
fn player_near(a: &mut Atk) {
    if let Some(t) = a.target
        && (t - a.pos).length() <= 5.0
    {
        let dir = (t - a.pos).normalize();
        a.wide(a.pos, dir, 0.6, s2(8, 0), 5, 30.0);
    }
}

/// One of Marisa's familiars: a star sprayer circling her, set off behind her.
fn marisa_familiar(a: &mut Atk, behind: f64, turn: f32, color: u8, special: i32) {
    let look = a.look();
    let axis = vec_from_angle(a.yaw, a.pitch - 90.0, 1.0);
    let dir = crate::vec::rotate(axis, look, 90.0);
    let at = a.pos + look * -behind;
    let d = shot(17, color, 0, 20 * a.level, special);
    a.fire.shot_turning(at, dir, 0.0, axis, turn, 9999, 0.5, 0.5, 0.0, V3::ZERO, d);
}

pub fn marisa(a: &mut Atk) {
    match a.phase {
        1 => {
            player_near(a);
            if a.c == 1 {
                marisa_familiar(a, 5.0, -7.0, 9, 11600);
            }
            if a.c >= 20 {
                a.c = 0;
            }
        }
        2 => {
            player_near(a);
            if a.c == 1 {
                marisa_familiar(a, 3.0, -5.0, 9, 11601);
            } else if a.c == 10 {
                marisa_familiar(a, 5.0, -8.0, 9, 11601);
            }
            if a.c >= 20 {
                a.c = 0;
            }
        }
        3 => {
            player_near(a);
            if a.c == 1 {
                marisa_familiar(a, 5.0, -7.0, 3, 11602);
            }
            if a.c >= 10 {
                a.c = 0;
            }
        }
        101 => {
            if a.c == 1 {
                a.use_spell_card(12);
            }
            if a.c >= 100 {
                a.c = 0;
            }
        }
        102 => {
            if a.ticks % 3 == 0 {
                let dir = a.angle(3.6 * a.c as f32, a.pitch);
                a.circle(a.pos, dir, 0.5, s4(9, 9, 0, 60), 4 * a.level);
            }
            if a.c == 1 {
                a.use_spell_card(25);
            }
            if a.c >= 110 {
                a.c = 0;
            }
        }
        103 => {
            if a.c == 1 {
                a.use_spell_card(1);
            }
            if a.c >= 110 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

/// Amulets in two rings, and a short line of three, as the hostile Reimu throws them.
fn reimu_amulets(a: &mut Atk, s1: f64, turn: f32, s2_: f64, stars: Option<u8>, size: f32) {
    let level = a.level;
    let look = a.look();
    a.circle(a.pos, look, s1, sd(27, 7, size, 0.5), 8 * level);
    let turned = a.angle(a.yaw + turn, a.pitch);
    a.circle(a.pos, turned, s2_, sd(27, 0, 0.2, 0.5), 8 * level);
    if let Some(color) = stars {
        let dir = a.angle(a.yaw + turn * 2.0, a.pitch);
        for i in 4..=6 {
            a.shot(a.pos, dir, 0.05 * f64::from(i), sd(27, color, 0.3, 0.5));
        }
    }
}

/// A yin-yang orb, falling and rolling (`EntityOnmyoudama`).
fn reimu_orb(a: &mut Atk, yaw: f32) {
    let dir = a.angle(yaw, a.pitch);
    let at = a.pos + dir * 2.0;
    let d = shot_full(30, 0, 1.2, 2.0, 0, 180, 43);
    a.fire.shot_turning(at, dir, 0.0, rotate_default(), 0.0, 9999, 0.5, 1.8, 0.0, crate::pattern::gravity(-0.03), d);
}

/// Journey To Gensokyo's hostile Reimu.
pub fn reimu_hostile(a: &mut Atk) {
    let level = a.level;
    let c = a.c;
    match a.phase {
        1 => {
            if c == 2 || c == 4 || c == 6 {
                reimu_amulets(a, 0.3, c as f32, 0.12, Some(7), 0.2);
            }
            if c < 30 && c % 2 == 0 {
                reimu_orb(a, a.yaw + (c * 12) as f32);
            }
            if c == 30 || c == 70 {
                let dir = angle_random(a.fire.rng);
                a.glide = Some((dir, 0.6, 10));
            }
            if c == 42 || c == 44 || c == 46 {
                reimu_amulets(a, 0.15, (c - 40) as f32, 0.25, Some(0), 0.2);
            }
            if c == 48 || c == 50 || c == 52 {
                reimu_amulets(a, 0.3, (c - 48) as f32, 0.12, None, 0.15);
            }
            if c < 70 && c > 40 && c % 2 == 0 {
                reimu_orb(a, a.yaw - (c * 12) as f32);
            }
            if a.c >= 80 {
                a.c = 0;
            }
        }
        101 => {
            if c == 1 {
                a.use_spell_card(0);
            }
            for k in 1..=8 {
                if c == 20 + k * (level / 2) {
                    for i in 1..=5 {
                        for j in -1..=1 {
                            a.play_shot_sound();
                            let dir = a.angle(a.yaw + j as f32 * 6.0 + (45 * k) as f32, a.pitch);
                            a.shot(a.pos, dir, 0.06 * f64::from(i), shot_full(27, 7, 0.2, 0.5, 0, 20, 11100));
                        }
                    }
                }
            }
            if a.c >= 100 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

// ---- The classic bosses: the originals' attacks with all they declared ----

/// Cirno as Touhou Items Mod had her: her second attack's fan of three lasers at 55, 65 and 75 too.
pub fn classic_cirno(a: &mut Atk) {
    cirno(a);
    if a.phase == 2 && matches!(a.c, 55 | 65 | 75) {
        let speed = 0.3 + f64::from(a.level) * 0.2;
        let look = a.look();
        a.fire.wide_laser_a(a.pos, look, speed, speed, 0.0, V3::ZERO, crate::pattern::laser(7, 0.1, 4.0, 4.0, 0, 80, 0), 3, 30.0, 0.0);
        a.play_shot_sound();
    }
}

/// Marisa as Journey To Gensokyo had her: after her third card, her final word, Double Spark
/// (`finalWord`, at 104). It came only to one who had used no spell card near her.
pub fn classic_marisa(a: &mut Atk) {
    marisa(a);
    if a.phase == 104 {
        if a.c == 1 {
            a.use_spell_card(1300);
        }
        if a.c >= 110 {
            a.c = 0;
        }
    }
}

/// A boss whose one card is all she does (`EntitySakuya`, `EntityMiko`): it at once, again every
/// 140 ticks.

/// A classic boss's spell-card phases from `first` on, one card each in turn (`cards[0]` in phase
/// `first`, `cards[1]` in the next): the card cast, and cast again a moment after it ends, until the
/// phase's health runs out. Whether `a` is in one of them.
fn card_run(a: &mut Atk, first: i32, cards: &[u32]) -> bool {
    let Some(&card) = usize::try_from(a.phase - first).ok().and_then(|i| cards.get(i)) else { return false };
    let end = crate::cards::Card::classic_by_number(card).map_or(110, |c| c.info().end);
    if a.c == 1 {
        a.use_spell_card(card);
    } else if a.c >= end + 30 {
        a.c = 0;
    }
    true
}

/// Classic Reimu: Fantasy Seal, then Bewitching Power Spoiler.
pub fn classic_reimu(a: &mut Atk) {
    card_run(a, 101, &[0, 16]);
}

/// Classic Reimu (Journey To Gensokyo): her fight and Fantasy Seal, then Bewitching Power Spoiler.
pub fn classic_reimu_hostile(a: &mut Atk) {
    if !card_run(a, 102, &[16]) {
        reimu_hostile(a);
    }
}

/// Classic Rumia: her fight and Moonlight Ray, then Double Black Death Butterfly.
pub fn classic_rumia(a: &mut Atk) {
    if !card_run(a, 102, &[5]) {
        rumia(a);
    }
}

/// Classic Tojiko: her fight and Gagouji's Cyclone, then Non-Directional Laser.
pub fn classic_toziko(a: &mut Atk) {
    if !card_run(a, 102, &[25]) {
        toziko(a);
    }
}

/// Sakuya: Eternal Meek, then Killing Doll. The original never let her fight; this is the fight it
/// had for her.
pub fn classic_sakuya(a: &mut Atk) {
    card_run(a, 101, &[24, 8]);
}

/// Miko: Gagouji's Cyclone, Bewitching Power Spoiler, then Fantasy Seal, the one the original gave her.
pub fn classic_miko(a: &mut Atk) {
    card_run(a, 101, &[40, 16, 0]);
}

/// Sanae: `danmaku01`, a five-pointed star of shots drawn one a tick and let go at once, then her
/// four cards (Miracle Fruit, Moses's Miracle, Yasaka's Divine Wind, Bewitching Power Spoiler).
pub fn classic_sanae(a: &mut Atk) {
    match a.phase {
        1 => {
            if a.c < 50 {
                a.stop = Some(true);
                let size = 2.0;
                let div = 10;
                let step = size / f64::from(div);
                let mut p = crate::vec::v3(0.0, size, 0.0);
                let mut ang = 4.39823f32;
                for i in 0..a.c {
                    p.x += f64::from(ang.cos()) * step;
                    p.y += f64::from(ang.sin()) * step;
                    if i % div == div - 1 {
                        ang += 2.5132744;
                    }
                }
                let look = a.look();
                let at = crate::vec::cross(look, p) + a.pos;
                a.fire.shot(at, look, 0.0, 0.0, 0.5, 0.03, V3::ZERO, shot(1, 1, 55 - a.c, 120, 0));
            } else if a.c == 50 {
                a.stop = Some(false);
                a.play_shot_sound();
            } else if a.c >= 100 {
                a.c = 0;
            }
        }
        101..=104 => {
            if a.c == 1 {
                a.use_spell_card([14, 17, 18, 16][(a.phase - 101) as usize]);
            } else if a.c >= 140 {
                a.c = 0;
            }
        }
        _ => {}
    }
}

fn kunai(color: u8, damage: f32, end: i32) -> ShotData {
    shot_full(13, color, SIZE[13], damage, 0, end, 0)
}

pub fn yukari(a: &mut Atk) {
    let level = a.level;
    let c = a.c;
    let look = a.look();
    match a.phase {
        1 | 2 => {
            let (c1, c2, s2_) = if a.phase == 1 { (4, 4, 0.31) } else { (4, 5, 0.4) };
            if c == 1 {
                a.play_shot_sound();
                a.circle(a.pos, look, 0.3, kunai(c1, 6.0, 60), 12 * level);
            } else if c == 2 {
                a.play_shot_sound();
                a.circle(a.pos, look, s2_, kunai(c2, 6.0, 60), 12 * level);
                a.circle(a.pos, look, 0.5, kunai(c2, 6.0, 60), 12 * level);
            }
            if a.c >= 10 {
                a.c = 0;
            }
        }
        3 => {
            if c == 1 {
                a.play_shot_sound();
                a.circle(a.pos, look, 0.3, kunai(5, 14.0, 60), 32 * level);
                a.circle(a.pos, look, 0.6, kunai(5, 14.0, 60), 32 * level);
            }
            if a.c >= 20 {
                a.c = 0;
            }
        }
        4 => {
            a.play_shot_sound();
            let (turn, color) = if c % 2 == 0 { ((c * 5) as f32, 2) } else { (-(c * 5) as f32, 5) };
            let dir = a.angle(a.yaw + turn, a.pitch);
            a.circle(a.pos, dir, 0.3, kunai(color, 6.0, 60), 2 * level);
            a.circle(a.pos, dir, 0.4, kunai(color, 6.0, 60), 2 * level);
            if a.c >= 60 {
                a.c = 0;
            }
        }
        5 => {
            if c % 3 == 0 {
                a.play_shot_sound();
                let dir = a.angle(a.yaw + (c * 5) as f32, a.pitch);
                for speed in [0.4, 0.5, 0.6] {
                    a.circle(a.pos, dir, speed, kunai(5, 9.0, 50), 4 * level);
                }
            }
            if a.c >= 60 {
                a.c = 0;
            }
        }
        101..=105 => {
            let n = (a.phase - 101) as usize;
            if c == 1 {
                a.use_spell_card([10, 302, 5, 303, 4][n]);
            }
            if a.c >= [170, 130, 150, 100, 300][n] {
                a.c = 0;
            }
        }
        _ => {}
    }
}
