//! The classic bosses' spell cards: the cards Touhou Items Mod's and Journey To Gensokyo's bosses
//! declared that the port had left out, ported from their `THSC_*` classes line for line. What a card
//! does besides firing (a gap opened, time stopped, a sound) it asks of the server through `Fx`.

use crate::cards::{Card, Ctx};
use crate::mobs::aim;
use crate::pattern::{Fire, laser, rotate_default};
use crate::shot::{DAMAGE, RAINBOW, SIZE, form, shot, shot_full, special};
use crate::vec::{V3, angle, cross, rotate, rotation_vector_from_angle, v3, vec_from_angle};

/// What a card asks of the world, beyond its shots.
#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    /// Flying-glowworm Nest's gap: at the first of these spots with a clear way from its user's
    /// feet, a gap facing `yaw`, which fires a laser five ticks on.
    Gap { tries: Vec<V3>, yaw: f32 },
    /// Killing Doll's stopped time (`EntitySakuyaWatch`, mode 1): every shot within forty blocks hangs
    /// still for this many ticks.
    TimeStop(i32),
    /// Killing Doll in the stopped time: a quarter of the user's hanging knives are thrown again.
    Redirect,
    /// A sound at the user: its id, volume and pitch.
    Sound(&'static str, f32, f32),
}

const SHOOT: Fx = Fx::Sound("minecraft:entity.arrow.shoot", 2.0, 0.8);
const SPARK: Fx = Fx::Sound("minecraft:entity.warden.sonic_boom", 3.0, 1.0);

/// The number (as the bosses declare them), name and length of each classic card. Journey To
/// Gensokyo's own cards are numbered 1300 and on here, since SPELLCARD PLUS used its 300s.
pub fn info(card: Card) -> (u32, &'static str, i32) {
    match card {
        Card::MusouFuuin => (0, "Spirit Sign \"Fantasy Seal\"", 90),
        Card::MasterSpark => (1, "Love Sign \"Master Spark\"", 109),
        Card::NijuuKokushichou => (5, "Sinister Spirits \"Double Black Death Butterfly\"", 200),
        Card::SatuzinDoll => (8, "Maid Secret Skill \"Killing Doll\"", 110),
        Card::HikouchuuNest => (10, "Fantasy Nest \"Flying-glowworm Nest\"", 160),
        Card::StardustReverie => (12, "Magic Sign \"Stardust Reverie\"", 120),
        Card::MiracleFruit => (14, "Miracle \"Miracle Fruit\"", 60),
        Card::YouryokuSpoiler => (16, "Youkai Extermination \"Bewitching Power Spoiler\"", 80),
        Card::IcicleFall => (19, "Ice Sign \"Icicle Fall\"", 180),
        Card::NonDirectionalLaser => (25, "Love Sign \"Non-Directional Laser\"", 103),
        Card::MoonlightRay => (28, "Moon Sign \"Moonlight Ray\"", 60),
        Card::LittleBugStorm => (34, "Wriggle Sign \"Little Bug Storm\"", 150),
        Card::MeteorOnEarth => (35, "Firefly Sign \"Meteor on Earth\"", 120),
        Card::GagoujiCyclone => (40, "Thunder Arrow \"Gagouji's Cyclone\"", 120),
        Card::DoubleSpark => (1300, "Loving Heart \"Double Spark\"", 110),
        Card::CurseOfDreamsAndReality => (1302, "Boundary \"Curse of Dreams and Reality\"", 160),
        Card::MeshOfLightAndDarkness => (1303, "Boundary \"Mesh of Light and Darkness\"", 150),
        _ => (u32::MAX, "", 0),
    }
}

/// The stars Master Spark and Double Spark spray from the card: the direction of the one `deg`
/// round the card's face (`angleY` is 0 in both, so the terms it scales are left out).
fn spark_star(c: &Ctx, look_at: V3, deg: f64) -> V3 {
    let rad = |d: f64| d.to_radians();
    let cy = rad(f64::from(c.card_yaw));
    let cp = rad(f64::from(c.card_pitch) + 90.0);
    let a = rad(deg);
    let x = a.cos() * look_at.x + a.sin() * cy.cos();
    let y = -a.cos() * cp.sin();
    let z = a.cos() * look_at.z + a.sin() * cy.sin();
    v3(x, y, z).normalize()
}

/// A Master Spark hakkero's laser (`EntityMiniHakkero`, `EntityMiniHakkeroJTG`): `laser(24, 4.2, 60,
/// 8, 30, end)`, one block out from the hakkero, which stays where it was set.
fn hakkero_laser(fire: &mut Fire, at: V3, look: V3, end: i32) {
    fire.laser_b(at + look, look, rotate_default(), 0.0, 9999, laser(24, 4.2, 60.0, 8.0, 30, end, 0), None);
}

/// One tick of a classic card.
pub fn tick(card: Card, c: &Ctx, state: &mut [f32; 2], fire: &mut Fire, fx: &mut Vec<Fx>) {
    let t = c.time;
    let level = c.level;
    let u = &c.user;
    match card {
        Card::MusouFuuin => {
            if t % 2 == 0 && (15..49).contains(&t) && c.target_alive {
                // `EntityMusouFuuin`: born a tenth across, growing to two; the server steers it.
                let s = shot_full(form::SMALL, ((t / 2) % 7) as u8, 0.1, 9.0, 0, 40, special::FANTASY_ORB);
                fire.shot_turning(c.pos_user(), angle(t as f32 * 33.0, 0.0), 0.0, rotate_default(), 0.0, 9999, 2.0, 0.0, -0.1, V3::ZERO, s);
                if t == 16 {
                    fx.push(Fx::Sound("minecraft:block.beacon.activate", 2.0, 1.4));
                }
            }
        }
        Card::MasterSpark => {
            if t == 1 {
                // The hakkero stands before its user's eyes as the card is used; it is gone at 109.
                let (yaw, pitch) = (f64::from(u.yaw).to_radians(), f64::from(u.pitch).to_radians());
                let at = v3(
                    u.pos.x - yaw.sin() * pitch.cos(),
                    u.pos.y - pitch.sin() + u.eye_height - 0.1,
                    u.pos.z + yaw.cos() * pitch.cos(),
                );
                hakkero_laser(fire, at, u.look(), 108);
            }
            if t == 30 {
                fx.push(SPARK);
            }
            if (30..99).contains(&t) {
                let cy = f64::from(c.card_yaw).to_radians();
                let cp = f64::from(c.card_pitch).to_radians();
                let cp90 = (f64::from(c.card_pitch) + 90.0).to_radians();
                let look_at = v3(-cy.sin() * cp90.cos(), -cp90.sin(), cy.cos() * cp90.cos());
                let mut deg = f64::from(t as f32 * 6.0);
                let g = 0.034 + 0.03 * (deg.to_radians()).sin();
                let gravity = v3(-cy.sin() * cp.cos(), -cp.sin(), cy.cos() * cp.cos()) * g;
                for _ in 0..7 {
                    let s = shot_full(form::STAR, (t % 7) as u8, 0.4, 8.0, 0, 55, 0);
                    fire.shot(c.card, spark_star(c, look_at, deg), 0.0, 0.2, 0.3, 0.05, gravity, s);
                    deg += f64::from(51.42857f32);
                }
            }
        }
        Card::NijuuKokushichou => {
            if t % 70 == 10 && t <= 100 {
                for i in 0..30 {
                    let speed = fire.rng.double() * 2.0 + 0.5;
                    let a = vec_from_angle(fire.rng.float() * 360.0, fire.rng.float() * 180.0 - 90.0, 1.0);
                    let s = shot_full(form::BUTTERFLY, (i % 2) as u8, SIZE[7], 8.0, 1, 120, special::KOKUSHICHOU);
                    fire.circle(c.pos_user(), a, 0.0, 9999, speed, 0.0, 1.0, V3::ZERO, s, 8, 0.2, 0.0);
                }
                fx.push(SHOOT);
            }
        }
        Card::SatuzinDoll => {
            if (5..15).contains(&t) {
                let s = shot_full(form::KNIFE, 1, SIZE[28], 12.0, 3, 80, 0);
                fire.sphere(c.pos_user(), u.look(), 0.0, rotate_default(), 0.0, 9999, 0.4, 0.4, 0.0, V3::ZERO, s, 32, 1.0, t as f32 * 6.0);
            }
            if t > 14 && t < 24 && t % 4 == 0 {
                let s = shot_full(form::KNIFE, 0, SIZE[28], 12.0, 3, 80, 0);
                for turn in [0.0, 5.0] {
                    fire.circle(c.pos_user(), vec_from_angle(u.yaw + turn, u.pitch, 1.0), 0.0, 9999, 0.4, 0.4, 0.0, V3::ZERO, s, 9, 1.0, 0.0);
                }
                fx.push(SHOOT);
            }
            if t == 36 {
                fx.push(Fx::TimeStop(60));
                fx.push(Fx::Sound("minecraft:block.bell.use", 2.0, 0.6));
            }
            if t > 44 && t < 68 && t % 8 == 3 {
                fx.push(Fx::Redirect);
            }
        }
        Card::HikouchuuNest => {
            // Four tries at a spot one to six blocks off, behind and beside the card's way.
            let mut tries = Vec::with_capacity(4);
            for _ in 0..4 {
                let length = fire.rng.double() * 5.0 + 1.0;
                let xz = f64::from((c.card_yaw + 90.0) / 180.0 * std::f32::consts::PI + fire.rng.float() * std::f32::consts::PI);
                let y = f64::from(fire.rng.float() * std::f32::consts::PI - std::f32::consts::FRAC_PI_2);
                tries.push(u.pos + v3(xz.sin() * y.cos(), y.sin(), xz.cos() * y.cos()) * length);
            }
            fx.push(Fx::Gap { tries, yaw: u.yaw });
        }
        Card::StardustReverie => {
            if t == 5 {
                let way = [4, 4, 7, 9, 10][level.clamp(0, 4) as usize];
                let s = shot_full(form::STAR, RAINBOW, 1.2, 7.0, 2, 120, special::STARDUST);
                fire.circle(c.pos_user(), u.look(), 0.0, 9999, 0.3, 0.3, 0.0, V3::ZERO, s, way, 0.2, 0.0);
                fx.push(SHOOT);
            }
        }
        Card::MiracleFruit => {
            if t >= 5 && t % 30 == 6 {
                let a = rotation_vector_from_angle(u.yaw, u.pitch, 0.0, 1.0);
                let s = shot_full(form::OVAL, 0, SIZE[16], 7.0, 3, 20, special::MIRACLE_FRUIT);
                let base = fire.rng.float() * 360.0;
                fire.circle(c.pos_user(), a, 0.0, 9999, 2.0, 0.1, -0.2, V3::ZERO, s, 8, 0.2, base);
                fx.push(SHOOT);
            }
        }
        Card::YouryokuSpoiler => {
            let base = if t < 20 {
                Some(t as f32 * 3.0)
            } else if t > 40 && t < 60 {
                Some(-t as f32 * 3.0)
            } else {
                None
            };
            if let Some(base) = base
                && c.target_alive
            {
                fx.push(Fx::Sound("minecraft:block.portal.ambient", 2.0, 1.9));
                let color = fire.rng.int(7) as u8;
                let s = shot_full(form::SCALE, color, 0.3, 3.0, 6, 70, special::SPOILER);
                fire.circle(c.target, vec_from_angle(c.card_yaw, c.card_pitch, 1.0), 0.0, 9999, 2.0, 0.2, -0.1, V3::ZERO, s, 7, 1.5, base);
            }
        }
        Card::IcicleFall => {
            if t % 5 == 0 {
                let a = ((t as f32 * 13.0 / 180.0 * 3.141593).sin()) * (30.0 + level as f32 * 30.0) + 90.0;
                let num = 4 + level * 2;
                for i in 1..num {
                    let fi = f64::from(i);
                    for (yaw, sp) in [(u.yaw + a, special::ICICLE_LEFT), (u.yaw - a, special::ICICLE_RIGHT)] {
                        let s = shot_full(form::CRYSTAL, 1, 0.5, 6.0, 0, num, sp);
                        fire.shot_turning(c.pos_user(), vec_from_angle(yaw, -30.0, 1.0), 0.0, rotate_default(), 0.0, 9999, fi, 0.0, -fi * 0.1, V3::ZERO, s);
                    }
                }
            }
        }
        Card::NonDirectionalLaser => {
            if t == 3 || t == 63 {
                fx.push(SPARK);
                let turn = if t == 3 { -4.0 } else { 4.0 };
                let look = u.look();
                let axis = vec_from_angle(u.yaw, u.pitch - 90.0, 1.0);
                for i in 0..5 {
                    let way = rotate(axis, look, i as f32 * 72.0);
                    let at = u.pos + way * 1.5 + v3(0.0, 1.0, 0.0);
                    fire.laser_b(at, way, axis, turn, 9999, laser(i, 0.7, 20.8, 7.0, 25, 60, 0), Some((1.5, 1.0)));
                }
            }
        }
        Card::MoonlightRay => {
            if t == 3 {
                fx.push(SPARK);
                let look = u.look();
                let over = vec_from_angle(u.yaw, u.pitch + 90.0, 1.0);
                let up = (c.user_height / 2.0) as f32;
                for i in 0..7 {
                    let way = rotate(look, over, i as f32 * (360.0 / 7.0));
                    let axis = cross(look, way);
                    let l = laser(1, 0.8 + level as f32 * 0.2, 40.0, 6.0, 10, 29 + level * 5, 0);
                    fire.laser_b(u.pos + way + v3(0.0, f64::from(up), 0.0), way, axis, -2.0, 9999, l, Some((1.0, up)));
                }
            }
        }
        Card::LittleBugStorm => {
            if t < 60 {
                state[0] += 2.7;
                state[1] += 2.7;
                let a = angle(state[0], state[1]);
                let sp = if (t / 15) % 2 == 0 { special::BUG_STORM } else { special::BUG_STORM_2 };
                let s = shot_full(form::TINY, 7, SIZE[1], 2.0, 0, 50, sp);
                let accel = -0.05 - f64::from(t) * 0.001;
                fire.sphere(c.pos_user(), a, 1.0, rotate_default(), 0.0, 9999, 1.0, 0.0, accel, V3::ZERO, s, (1 + level * 2) as usize, 0.3, 1.4);
                if t % 3 == 0 {
                    fx.push(SHOOT);
                }
            }
        }
        Card::MeteorOnEarth => {
            if t == 5 || t == 65 {
                let way = [0, 5, 7, 4, 6][level.clamp(0, 4) as usize];
                let turn = if t == 5 { 2.0 } else { -2.0 };
                let a = c.at_target();
                let s = shot(form::FAMILIAR, 2, 0, 60, special::METEOR);
                let base = fire.rng.float() * 360.0;
                fire.ring(c.pos_user(), a, turn, 60, 0.3, 0.3, 1.0, V3::ZERO, s, way, 0.5, 30.0, base);
                if level >= 3 {
                    let base = fire.rng.float() * 360.0;
                    fire.ring(c.pos_user(), a, turn, 60, 0.3, 0.3, 1.0, V3::ZERO, s, 8, 0.5, 70.0, base);
                }
                fx.push(SHOOT);
            }
        }
        Card::GagoujiCyclone => {
            let a = c.at_target();
            let lf = f64::from(level);
            let side = vec_from_angle(u.yaw, u.pitch + 90.0, 1.0);
            let arrow = |fire: &mut Fire, base: f32| {
                fire.ring(c.pos_user(), a, 0.0, 9999, 1.0, 1.0, 0.0, V3::ZERO, shot(form::ARROW, 3, 0, 50, special::GAGOUJI), 9, 1.0, 80.0, base);
                if level >= 2 {
                    let s = shot_full(form::ARROW, 3, SIZE[18], 5.0, 0, 60, special::GAGOUJI);
                    fire.shot_turning(c.pos_user(), a, 0.0, side, 0.0, 99999, 1.0, 0.0, -0.1, V3::ZERO, s);
                }
            };
            if t % 120 <= 40 && t % 2 == 0 {
                arrow(fire, t as f32 / 2.0);
            }
            if t % 120 >= 60 && t % 120 < 100 && t % 2 == 0 {
                arrow(fire, -t as f32 / 2.0);
            }
            if level == 4 && t % 120 > 30 && t % 120 <= 90 {
                fire.ring(c.pos_user(), a, 0.0, 99999, 1.0, 0.0, -0.1, V3::ZERO, shot(form::ARROW, 0, 0, 60, special::GAGOUJI), 3, 1.0, 10.0, t as f32 / 2.0);
            }
            if t % 20 == 0 {
                let s = shot_full(form::CIRCLE, 3, SIZE[4], 5.0, 0, 90, 0);
                let limit = 0.15 + lf * 0.05;
                fire.ring(c.pos_user(), a, 0.0, 99999, 1.2, limit, -0.1, V3::ZERO, s, 8 + level * 4, 1.0, 70.0, 0.0);
                fire.ring(c.pos_user(), a, 0.0, 99999, 1.2, limit, -0.1, V3::ZERO, s, 4 + level * 2, 1.0, 20.0, 0.0);
                if level >= 3 {
                    fire.shot_turning(c.pos_user(), a, 0.0, rotate_default(), 0.0, 99999, 1.2, limit, -0.1, V3::ZERO, s);
                }
                fx.push(SHOOT);
            }
        }
        Card::DoubleSpark => {
            if t == 1 {
                // `EntityMiniHakkeroJTG`, double: behind its user's eyes, two lasers ten degrees apart.
                let look = u.look();
                let at = v3(u.pos.x - look.x, u.pos.y - look.y + u.eye_height - 0.1, u.pos.z - look.z);
                for turn in [10.0, -10.0] {
                    hakkero_laser(fire, at, vec_from_angle(u.yaw + turn, u.pitch, 1.0), 120);
                }
            }
            if t == 30 {
                fx.push(SPARK);
            }
            if (30..=100).contains(&t) {
                let speed = f64::from(fire.rng.int(5) as i32 - 2);
                let look_at = angle(c.card_yaw, c.card_pitch + 90.0);
                let mut deg = f64::from(t) * speed;
                let g = 0.034 + 0.03 * f64::from((deg.to_radians() as f32).sin());
                let gravity = vec_from_angle(c.card_yaw, c.card_pitch, g);
                let s1 = shot_full(form::STAR, (t % 7) as u8, 0.5, 8.0, 0, 55, 0);
                let s2 = shot_full(form::STAR, (t % 7) as u8, 0.35, 8.0, 0, 100, 0);
                for i in 0..12 {
                    let a = spark_star(c, look_at, deg);
                    if i % 2 == 0 {
                        fire.shot(c.card, a, 0.0, 0.1, 0.5, 0.15, gravity, s1);
                    }
                    fire.shot(c.card, a, 0.0, 0.1, 0.6, 0.1, gravity, s2);
                    deg += 30.0;
                }
            }
        }
        Card::CurseOfDreamsAndReality => {
            if t == 1 || t == 50 {
                fx.push(SHOOT);
                let (seek, split) = if t == 1 { (45.0, -45.0) } else { (-45.0, 45.0) };
                let orb = |sp| shot_full(form::BIG, 1, SIZE[30], 3.0, 0, 15, sp);
                fire.shot(c.pos_user(), angle(u.yaw + seek, u.pitch), 0.0, 0.35, 0.35, 0.0, V3::ZERO, orb(special::CURSE_ORB));
                fire.shot(c.pos_user(), angle(u.yaw + split, u.pitch), 0.0, 0.35, 0.35, 0.0, V3::ZERO, orb(special::CURSE_SPLIT));
            }
        }
        Card::MeshOfLightAndDarkness => {
            let (first, second) = (t == 1, t == 50);
            if first || second {
                fx.push(SHOOT);
                // At 50 the colours change sides.
                let (cl, cr) = if first { (1, 0) } else { (0, 1) };
                let l = angle(u.yaw + 45.0, u.pitch);
                let r = angle(u.yaw - 45.0, u.pitch);
                fire.shot(c.pos_user(), l, 0.0, 0.55, 0.55, 0.0, V3::ZERO, shot(form::BIG, cl, 0, 80, 0));
                fire.shot(c.pos_user(), r, 0.0, 0.55, 0.55, 0.0, V3::ZERO, shot(form::BIG, cr, 0, 80, 0));
                for i in 1..20 {
                    let sp = 0.2 * f64::from(i) * 0.15;
                    let span = 180.0 / i as f32;
                    fire.random_ring(c.pos_user(), l, 0.0, 9999, sp, sp, 0.0, V3::ZERO, shot(form::MEDIUM, cl, 0, 80, 0), 2, 0.2, span);
                    fire.random_ring(c.pos_user(), r, 0.0, 9999, sp, sp, 0.0, V3::ZERO, shot(form::MEDIUM, cr, 0, 80, 0), 2, 0.2, span);
                }
                for i in 1..5 {
                    let (first, limit) = (0.3 * f64::from(i) * 0.25, 0.23 * f64::from(i) * 0.25);
                    let span = (90 / i) as f32;
                    let big = |color| shot_full(form::LIGHT, color, 0.5, DAMAGE[5] * 2.0, 0, 80, 0);
                    fire.random_ring(c.pos_user(), l, 0.0, 9999, first, limit, 0.0, V3::ZERO, big(cl), 2, 0.2, span);
                    fire.random_ring(c.pos_user(), r, 0.0, 9999, first, limit, 0.0, V3::ZERO, big(cr), 2, 0.2, span);
                }
            }
            // The mesh: shots left hanging along the two ways, each holding a laser.
            let step = if t > 1 && t < 20 { Some(t) } else if t > 50 && t < 70 { Some(t - 50) } else { None };
            if let Some(k) = step
                && t % 2 == 0
            {
                fx.push(SHOOT);
                let (ca, cb) = if t < 50 { (0, 1) } else { (1, 0) };
                let d = 2.0 * f64::from(k);
                let out = |yaw: f32| c.pos_user() + angle(u.yaw + yaw, u.pitch) * d;
                for (from, toward, color) in [(45.0, 135.0, ca), (45.0, -45.0, ca), (-45.0, -135.0, cb), (-45.0, 45.0, cb)] {
                    let way = aim(angle(u.yaw + toward, u.pitch), 30.0, fire.rng);
                    let s = shot_full(form::MEDIUM, color, 0.5, 8.0, 0, 60, special::MESH);
                    fire.shot(out(from), way, 0.0, 0.0, 0.0, 0.0, V3::ZERO, s);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Actor, CLASSIC, card_position};
    use crate::shot::Shot;
    use crate::vec::Rng;

    fn run(card: Card) -> (usize, Vec<Fx>) {
        let user = Actor { pos: v3(0.0, 64.0, 0.0), eye_height: 1.62, yaw: 20.0, pitch: 5.0 };
        let mut out: Vec<Shot> = Vec::new();
        let mut fx = Vec::new();
        let mut rng = Rng::new(7);
        let mut state = [0.0; 2];
        for time in 0..card.info().end {
            let ctx = Ctx {
                time,
                level: 2,
                user,
                target: v3(3.0, 65.0, 16.0),
                target_alive: true,
                card: card_position(&user),
                card_yaw: user.yaw,
                card_pitch: user.pitch,
                user_height: 1.8,
            };
            card.tick(&ctx, &mut state, &mut Fire { out: &mut out, rng: &mut rng }, &mut fx);
        }
        assert!(out.iter().all(|s| s.pos.length().is_finite() && s.motion.length().is_finite() && s.angle.length().is_finite()), "{card:?}");
        (out.len(), fx)
    }

    #[test]
    fn every_classic_card_does_something() {
        for card in CLASSIC {
            let (n, fx) = run(card);
            assert!(n > 0 || !fx.is_empty(), "{card:?} did nothing");
        }
    }

    #[test]
    fn classic_cards_fire_what_their_scripts_say() {
        // Fantasy Seal: an orb every other tick from 15 to 48.
        assert_eq!(run(Card::MusouFuuin).0, 17);
        // Moonlight Ray: seven held lasers.
        assert_eq!(run(Card::MoonlightRay).0, 7);
        // Non-Directional Laser: five, twice.
        assert_eq!(run(Card::NonDirectionalLaser).0, 10);
        // Killing Doll stops time once and throws knives again three times.
        let (_, fx) = run(Card::SatuzinDoll);
        assert_eq!(fx.iter().filter(|f| matches!(f, Fx::TimeStop(60))).count(), 1);
        assert_eq!(fx.iter().filter(|f| **f == Fx::Redirect).count(), 3);
        // The nest tries for a gap every tick.
        assert_eq!(run(Card::HikouchuuNest).1.iter().filter(|f| matches!(f, Fx::Gap { .. })).count(), 160);
    }

    #[test]
    fn the_sparks_are_held_lasers() {
        let user = Actor { pos: v3(0.0, 64.0, 0.0), eye_height: 1.62, yaw: 0.0, pitch: 0.0 };
        let mut out: Vec<Shot> = Vec::new();
        let mut rng = Rng::new(1);
        let ctx = Ctx { time: 1, level: 2, user, target: v3(0.0, 65.0, 10.0), target_alive: true, card: card_position(&user), card_yaw: 0.0, card_pitch: 0.0, user_height: 1.8 };
        tick(Card::DoubleSpark, &ctx, &mut [0.0; 2], &mut Fire { out: &mut out, rng: &mut rng }, &mut Vec::new());
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|s| s.is_laser() && s.form == crate::shot::LASER_SPARK && s.laser == 60.0 && s.speed() == 0.0));
    }
}
