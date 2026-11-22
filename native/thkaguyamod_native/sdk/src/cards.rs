//! Spell cards: each a script that fires, tick by tick, from its user at its target. Ported from
//! Touhou Items Mod's `THSC_*` classes; the cards here fire plain danmaku only. Lasers, familiars and
//! the cards that summon things come with the parts of the engine they need.

use crate::pattern::{Fire, gravity, rotate_default};
use crate::shot::{SIZE, form, shot, shot_full, special};
use crate::vec::{V3, angle, angle_to, v3, vec_from_angle};

/// Who casts, as each tick finds them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Actor {
    /// Their feet.
    pub pos: V3,
    pub eye_height: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Actor {
    /// `THShotLib.getPosYFromEye`: a little below and before the eyes.
    pub fn shot_y(&self) -> f64 {
        let p = f64::from(self.pitch) / 180.0 * std::f64::consts::PI;
        self.pos.y + self.eye_height + p.sin() * -0.5 + p.cos() * -0.5
    }

    /// `getLookVec`.
    pub fn look(&self) -> V3 {
        angle(self.yaw, self.pitch)
    }
}

/// What a card sees each tick.
pub struct Ctx {
    pub time: i32,
    pub level: i32,
    pub user: Actor,
    /// The target's middle (`pos_Living`), or a point ahead of the user when there is none.
    pub target: V3,
    pub target_alive: bool,
    /// Where the card floats: beside its user, as it was cast.
    pub card: V3,
    pub card_yaw: f32,
}

impl Ctx {
    /// `pos_User()`.
    pub fn pos_user(&self) -> V3 {
        v3(self.user.pos.x, self.user.shot_y(), self.user.pos.z)
    }

    /// `pos_User(angle, distance)`: out along `angle` level with the user. Its height only rises by
    /// the direction's own height, not times `distance`: so it was written.
    pub fn pos_user_toward(&self, a: V3, distance: f64) -> V3 {
        let a = a.normalize();
        v3(self.user.pos.x + a.x * distance, self.user.shot_y() + a.y, self.user.pos.z + a.z * distance)
    }

    pub fn at_target(&self) -> V3 {
        angle_to(self.pos_user(), self.target)
    }
}

/// Where a card floats, given its user as cast (`EntitySpellCard`'s first position).
pub fn card_position(user: &Actor) -> V3 {
    let yaw = (user.yaw + 30.0) / 180.0 * 3.141593;
    let pitch = user.pitch / 180.0 * 3.141593;
    v3(
        user.pos.x - f64::from(yaw.sin()) * f64::from(pitch.cos()),
        user.pos.y - f64::from(pitch.sin()) + user.eye_height - 0.1,
        user.pos.z + f64::from(yaw.cos()) * f64::from(pitch.cos()),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    KashoNoEimin,
    MeteonicShower,
    NamiToTubuNoKyoukai,
    PerfectFreeze,
    KerochanFuuuNiMakezu,
    MosesMiracle,
    YasakaNoKamikaze,
    EternalMeek,
    HoukaKenran,
}

pub const CARDS: [Card; 9] = [
    Card::KashoNoEimin,
    Card::MeteonicShower,
    Card::NamiToTubuNoKyoukai,
    Card::PerfectFreeze,
    Card::KerochanFuuuNiMakezu,
    Card::MosesMiracle,
    Card::YasakaNoKamikaze,
    Card::EternalMeek,
    Card::HoukaKenran,
];

#[allow(dead_code)]
pub struct Info {
    /// The original's spell card number.
    pub number: u32,
    /// Its item, `thkaguyamod:<item>`.
    pub item: &'static str,
    /// The original's icon, `textures/items/spellCard/<icon>.png`.
    pub icon: &'static str,
    pub name: &'static str,
    /// Levels (or bomb items) it costs.
    pub need_level: i32,
    /// Ticks it lasts.
    pub end: i32,
}

impl Card {
    pub fn info(self) -> Info {
        let (number, item, icon, name, need_level, end) = match self {
            Card::KashoNoEimin => (2, "spell_card_kasho_no_eimin", "Kasho_no_Eimin", "Deadly Butterfly \"Everlasting Nap\"", 3, 150),
            Card::MeteonicShower => (3, "spell_card_meteonic_shower", "MeteonicShower", "Star Sign \"Meteonic Shower\"", 1, 60),
            Card::NamiToTubuNoKyoukai => (4, "spell_card_nami_to_tubu_no_kyoukai", "Nami_to_Tubu_no_Kyoukai", "Boundary Sign \"Boundary between Wave and Particle\"", 5, 320),
            Card::PerfectFreeze => (9, "spell_card_perfect_freeze", "PerfectFreeze", "Freeze Sign \"Perfect Freeze\"", 1, 110),
            Card::KerochanFuuuNiMakezu => (13, "spell_card_kerochan_fuuu_ni_makezu", "Kerochan_Fuuu_ni_Makezu", "Native God \"Kero-chan Braves the Wind and Rain\"", 2, 200),
            Card::MosesMiracle => (17, "spell_card_moses_miracle", "Moses_no_Kiseki", "Sea Opening \"Moses's Miracle\"", 2, 180),
            Card::YasakaNoKamikaze => (18, "spell_card_yasaka_no_kamikaze", "Yasaka_no_Kamikaze", "Great Miracle \"Yasaka's Divine Wind\"", 3, 270),
            Card::EternalMeek => (24, "spell_card_eternal_meek", "EternalMeek", "Conjuring \"Eternal Meek\"", 2, 50),
            Card::HoukaKenran => (29, "spell_card_houka_kenran", "HoukaKenran", "Flower Sign \"Gorgeous Sweet Flower\"", 2, 170),
        };
        Info { number, item, icon, name, need_level, end }
    }

    pub fn by_item(item: &str) -> Option<Card> {
        let name = item.strip_prefix("thkaguyamod:")?;
        CARDS.into_iter().find(|c| c.info().item == name)
    }

    /// One tick of the card. `state` is what a card keeps between ticks.
    pub fn tick(self, c: &Ctx, state: &mut [f32; 2], fire: &mut Fire) {
        let t = c.time;
        match self {
            Card::KashoNoEimin => {
                if t % 6 == 3 && t < 90 {
                    let a = if c.target_alive { c.at_target() } else { vec_from_angle(c.user.yaw, c.user.pitch, 1.0) };
                    let color = if t % 9 == 0 { 3 } else { 4 };
                    let base = fire.rng.float() * 360.0;
                    let s = shot_full(form::BUTTERFLY, color, SIZE[7], 6.0, 3, 70, 0);
                    fire.circle(c.pos_user(), a, 1.6, 30, 0.2, 0.7, 0.01, V3::ZERO, s, 15, 0.8, base);
                    fire.circle(c.pos_user(), a, -1.6, 30, 0.2, 0.7, 0.01, V3::ZERO, s, 15, 0.8, base);
                }
            }
            Card::MeteonicShower => {
                if t > 10 && t < 40 && t % 2 == 0 {
                    let a = if c.target_alive { c.at_target() } else { c.user.look() };
                    let color = [0, 2, 3, 4][fire.rng.int(4) as usize];
                    let size = 0.2 + fire.rng.float() * 0.4;
                    let damage = (size * 15.0) as i32 as f32;
                    let s = shot_full(form::STAR, color, size, damage, 3, 40, 0);
                    fire.random_ring(c.pos_user_toward(a, 0.7), a, 0.0, 9999, 0.4, 0.6, 0.1, V3::ZERO, s, 1, 0.0, 40.0);
                }
            }
            Card::NamiToTubuNoKyoukai => {
                if t == 0 {
                    *state = [0.0, 0.0];
                }
                if (5..280).contains(&t) {
                    let k = (t + 75) as f32 / 50.0;
                    state[0] += k.cos() * 17.0;
                    state[1] += (k.sin() * 3.14).sin() * 17.0;
                    let s = shot_full(form::LIGHT, 4, 0.3, 9.0, 0, 40, 0);
                    let a = vec_from_angle(state[0], state[1], 1.0);
                    fire.sphere(c.pos_user(), a, 0.0, rotate_default(), 0.0, 9999, 0.2, 0.6, 0.05, V3::ZERO, s, 12, 2.0, 0.0);
                }
            }
            Card::PerfectFreeze => {
                if (5..58).contains(&t) {
                    let n = (4 + c.level).min(10);
                    for _ in 0..n {
                        let a = angle(fire.rng.float() * 360.0, fire.rng.float() * 180.0 - 90.0);
                        let s = shot_full(form::CIRCLE, crate::shot::RAINBOW, SIZE[4], 4.0, 5, 120, 0);
                        fire.shot(c.pos_user_toward(a, 1.0), a, 0.0, f64::from(c.level) * 0.1, 0.5, 0.03, V3::ZERO, s);
                    }
                }
                // At 64 every shot about the user freezes white; the server does it, as it holds them.
            }
            Card::KerochanFuuuNiMakezu => {
                if t >= 2 {
                    let at = v3(c.card.x, c.card.y - 0.4, c.card.z);
                    if t < 140 {
                        for i in 0..4 {
                            let yaw = c.card_yaw + i as f32 * 90.0 + (t as f32 * 4.0).cos() * 45.0;
                            let pitch = (((t as f32) + i as f32 * 45.0) * 2.0 / 180.0 * 3.141593).cos() * 19.0 - 70.0;
                            let s = shot_full(form::LIGHT, 1, SIZE[5], 8.0, 10, 120, 0);
                            fire.shot(at, angle(yaw, pitch), 0.0, 0.5, 0.5, 0.0, gravity(f64::from(-0.01f32)), s);
                        }
                    }
                    for _ in 0..2 {
                        let speed = fire.rng.double() * 0.2 + 0.45;
                        let a = angle(fire.rng.float() * 360.0, fire.rng.float() * 14.0 - 75.0);
                        let s = shot_full(form::RICE, 5, 0.15, 9.0, 4, 120, 0);
                        fire.shot(at, a, 0.0, speed, speed, 0.0, gravity(f64::from(-0.018f32)), s);
                    }
                }
            }
            Card::MosesMiracle => {
                if t % 2 != 0 {
                    let a = f64::from(t) * 6.0;
                    let base = ((c.card.x - c.target.x).atan2(-(c.card.z - c.target.z)) / 3.141593 * 180.0) as f32;
                    let rad = |d: f64| d / 180.0 * f64::from(3.141593f32);
                    let b = f64::from(base);
                    let toward = angle_to(c.card, c.target);
                    for i in -5..6 {
                        let bx = c.card.x - rad(a).sin() * rad(b).cos() * 3.0;
                        let bz = c.card.z + rad(a).cos() * rad(b).sin() * 3.0;
                        let sx = -rad(b + 90.0).sin();
                        let sz = rad(b + 90.0).cos();
                        let s = shot_full(form::LIGHT, 1, 0.8, 7.0, 0, 50, 0);
                        let y = c.card.y + f64::from(i);
                        fire.shot(v3(bx + sx * 2.5, y, bz + sz * 2.5), toward, 0.0, 0.9, 0.9, 0.0, V3::ZERO, s);
                        fire.shot(v3(bx - sx * 2.5, y, bz - sz * 2.5), toward, 0.0, 0.9, 0.9, 0.0, V3::ZERO, s);
                    }
                }
            }
            Card::YasakaNoKamikaze => {
                if t >= 5 {
                    let a = if c.target_alive {
                        angle_to(c.target, c.pos_user())
                    } else {
                        vec_from_angle(c.user.yaw, c.user.pitch, -1.0)
                    };
                    let rad = |d: f32| d / 180.0 * std::f32::consts::PI;
                    let distance = (f64::from(rad((t * 2) as f32) - std::f32::consts::PI)).cos() * 2.0 + 3.0;
                    if t % 46 < 23 {
                        let s = shot(form::RICE, 2, 10, 40, special::WIND);
                        let s = shot_full(s.form, s.color, s.size, 18.0, s.delay, s.end, s.special);
                        let span = rad((t * 16) as f32).sin() * 50.0 + 50.0;
                        fire.ring(c.pos_user(), a, 180.0, 1, 0.0, 1.6, 0.1, V3::ZERO, s, 8, distance, span, t as f32 * 10.0);
                    } else {
                        let s = shot(form::RICE, 5, 10, 40, special::WIND);
                        let s = shot_full(s.form, s.color, s.size, 18.0, s.delay, s.end, s.special);
                        let span = rad((-t * 16) as f32).sin() * 50.0 + 50.0;
                        fire.ring(c.pos_user(), a, -180.0, 2, 0.0, 1.6, 0.1, V3::ZERO, s, 8, distance, span, -t as f32 * 10.0);
                    }
                }
            }
            Card::EternalMeek => {
                if t < 40 {
                    let a = if c.target_alive { c.at_target() } else { c.user.look() };
                    let s = shot_full(form::KNIFE, 1, SIZE[28], 3.0, 3, 60, 0);
                    fire.random_ring(c.pos_user_toward(a, 0.7), a, 0.0, 9999, 1.5, 1.5, 0.0, V3::ZERO, s, 3, 0.1, 50.0);
                }
            }
            Card::HoukaKenran => {
                let a = if c.target_alive { c.at_target() } else { c.user.look() };
                if t < 150 && t % 3 == 0 {
                    let wave = (t as f32 / 20.0).sin() * 40.0;
                    let s = shot_full(form::CRYSTAL, 3, SIZE[11], 5.0, 0, 80, 0);
                    fire.ring_simple(c.pos_user(), a, 0.4, s, 6, 0.6, 45.0 + wave, t as f32 * 2.0);
                    fire.ring_simple(c.pos_user(), a, 0.4, s, 6, 0.6, 45.0 - wave, t as f32 * -2.0);
                }
                if t < 150 && t % 20 == 19 {
                    let s = shot_full(form::CRYSTAL, 0, SIZE[11], 5.0, 0, 80, 0);
                    fire.ring_simple(c.pos_user(), a, 0.4, s, 32, 0.6, 45.0, t as f32 * -2.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shot::Shot;
    use crate::vec::Rng;

    fn run(card: Card) -> usize {
        let user = Actor { pos: v3(0.0, 64.0, 0.0), eye_height: 1.62, yaw: 0.0, pitch: 0.0 };
        let mut out: Vec<Shot> = Vec::new();
        let mut rng = Rng::new(3);
        let mut state = [0.0; 2];
        for time in 0..card.info().end {
            let ctx = Ctx {
                time,
                level: 2,
                user,
                target: v3(0.0, 65.0, 16.0),
                target_alive: true,
                card: card_position(&user),
                card_yaw: 0.0,
            };
            card.tick(&ctx, &mut state, &mut Fire { out: &mut out, rng: &mut rng });
        }
        assert!(out.iter().all(|s| s.pos.length().is_finite() && s.motion.length().is_finite()));
        out.len()
    }

    #[test]
    fn every_card_fires_and_fires_finite_shots() {
        for card in CARDS {
            assert!(run(card) > 0, "{card:?} fired nothing");
        }
    }

    #[test]
    fn cards_fire_as_many_as_their_scripts_say() {
        // Fifteen tick each of two circles of fifteen.
        assert_eq!(run(Card::KashoNoEimin), 15 * 2 * 15);
        // Perfect Freeze: 4 + level shots a tick from 5 to 57.
        assert_eq!(run(Card::PerfectFreeze), 53 * 6);
        // Eternal Meek: three knives a tick for forty ticks.
        assert_eq!(run(Card::EternalMeek), 120);
    }

    #[test]
    fn items_find_their_cards() {
        for card in CARDS {
            assert_eq!(Card::by_item(&format!("thkaguyamod:{}", card.info().item)), Some(card));
        }
        assert_eq!(Card::by_item("minecraft:paper"), None);
    }
}
