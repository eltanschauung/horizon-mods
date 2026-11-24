//! `THShotLib`'s ways of firing: one shot, a fan, a circle, a ring, a random ring and a sphere, with
//! the same arguments and the same arithmetic, so spell cards port line for line.

use crate::shot::{RAINBOW, RANDOM, Shot, ShotData};
use crate::vec::{Rng, V3, pitch_of, rotate, v3, vec_from_angle, yaw_of};

const RAINBOW_ORDER: [u8; 7] = [0, 6, 3, 2, 5, 1, 4];

/// Where fired shots go, and the dice their random colours and spreads come from.
pub struct Fire<'a> {
    pub out: &'a mut Vec<Shot>,
    pub rng: &'a mut Rng,
}

pub fn rotate_default() -> V3 {
    v3(0.0, 1.0, 0.0)
}

pub fn gravity(y: f64) -> V3 {
    v3(0.0, y, 0.0)
}

/// `LaserData`: a laser's look (`color`, its form times eight plus its colour, 8 and 9 picking at
/// random), width, length, damage, delay, end and special.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaserData {
    pub color: i32,
    pub width: f32,
    pub length: f32,
    pub damage: f32,
    pub delay: i32,
    pub end: i32,
    pub special: i32,
}

/// `LaserData.laser(color, width, length, damage, delay, end, special)`.
pub fn laser(color: i32, width: f32, length: f32, damage: f32, delay: i32, end: i32, special: i32) -> LaserData {
    LaserData { color, width, length, damage, delay, end, special }
}

impl LaserData {
    fn shot(self, rng: &mut Rng) -> ShotData {
        let color = match self.color {
            8 => rng.int(8) as i32,
            9 => rng.int(7) as i32,
            c => c,
        };
        ShotData {
            form: (color / 8) as u8,
            color: (color % 8) as u8,
            size: self.width,
            damage: self.damage,
            delay: self.delay,
            end: self.end,
            special: self.special,
        }
    }
}

impl Fire<'_> {
    /// `createShot`, all arguments: a random colour is picked for this shot alone.
    #[allow(clippy::too_many_arguments)]
    pub fn shot_turning(
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
        mut data: ShotData,
    ) {
        if data.color == RAINBOW {
            data.color = self.rng.int(7) as u8;
        } else if data.color == RANDOM {
            data.color = self.rng.int(8) as u8;
        }
        self.out.push(Shot::new(pos, angle, slope, rotate, rot_speed, rot_end, first, limit, accel, gravity, data));
    }

    /// `createShot(user, source, pos, angle, slope, first, limit, accel, gravity, shot)`: it turns
    /// about the axis square to its course, at no speed.
    #[allow(clippy::too_many_arguments)]
    pub fn shot(&mut self, pos: V3, angle: V3, slope: f32, first: f64, limit: f64, accel: f64, gravity: V3, data: ShotData) {
        let yaw = yaw_of(angle.x, angle.z);
        let pitch = pitch_of(angle.x, angle.y, angle.z);
        let axis = vec_from_angle(yaw, pitch + 90.0, 1.0);
        self.shot_turning(pos, angle, slope, axis, 0.0, 9999, first, limit, accel, gravity, data);
    }

    /// `createLaserA`: a laser fired like a shot, growing from nothing to its length as it flies.
    #[allow(clippy::too_many_arguments)]
    pub fn laser_a(&mut self, pos: V3, angle: V3, first: f64, limit: f64, accel: f64, gravity: V3, l: LaserData) {
        let data = l.shot(self.rng);
        let mut s = Shot::new(pos, angle, 0.0, rotate_default(), 0.0, 9999, first, limit, accel, gravity, data);
        s.laser_max = l.length;
        self.out.push(s);
    }

    /// `createWideLaserA`: `way` lasers across `wide` degrees, `distance` out.
    #[allow(clippy::too_many_arguments)]
    pub fn wide_laser_a(&mut self, pos: V3, angle: V3, first: f64, limit: f64, accel: f64, gravity: V3, l: LaserData, way: i32, wide: f32, distance: f64) {
        let yaw = yaw_of(angle.x, angle.z);
        let pitch = pitch_of(angle.x, angle.y, angle.z);
        let axis = vec_from_angle(-yaw, -pitch + 90.0, 1.0);
        let mut a = -wide / 2.0;
        let span = wide / (way - 1).max(1) as f32;
        let mut color = self.rng.int(7) as usize;
        for _ in 0..way {
            let mut each = l;
            if l.color == 9 {
                each.color = i32::from(RAINBOW_ORDER[color % 7]);
                color += 1;
            }
            let v = rotate(axis, angle, a);
            self.laser_a(pos + v * distance, v, first, limit, accel, gravity, each);
            a += span;
        }
    }

    /// `createLaserB`: a laser born whole, held `anchor` = (out along its way, up) from its owner's
    /// feet if it has one, else standing where it was set; turning `rot_speed` degrees a tick about
    /// `rotate` until `rot_end`.
    #[allow(clippy::too_many_arguments)]
    pub fn laser_b(&mut self, pos: V3, angle: V3, rotate: V3, rot_speed: f32, rot_end: i32, l: LaserData, anchor: Option<(f32, f32)>) {
        let data = l.shot(self.rng);
        let mut s = Shot::new(pos, angle, 0.0, rotate, rot_speed, rot_end, 0.0, 0.0, 0.0, V3::ZERO, data);
        s.laser_max = l.length;
        s.laser = l.length;
        s.anchor = anchor;
        self.out.push(s);
    }

    /// `createWideShot`: `way` shots across `wide` degrees.
    #[allow(clippy::too_many_arguments)]
    pub fn wide(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        mut data: ShotData,
        way: i32,
        wide: f32,
        distance: f64,
        base_angle: f32,
    ) {
        let yaw = yaw_of(angle.x, angle.z);
        let pitch = pitch_of(angle.x, angle.y, angle.z);
        let axis = vec_from_angle(-yaw, -pitch + 90.0, 1.0);
        let mut a = -wide / 2.0 + base_angle;
        let span = wide / (way - 1) as f32;
        let mut color = self.rng.int(7) as usize;
        let rainbow = data.color == RAINBOW;
        if data.color == RANDOM {
            data.color = self.rng.int(8) as u8;
        }
        for _ in 0..way {
            if rainbow {
                data.color = RAINBOW_ORDER[color % 7];
                color += 1;
            }
            let v = rotate(axis, angle, a);
            let slope = pitch * (a / 180.0 * 3.141593).sin();
            self.shot_turning(pos + v * distance, v, slope, axis, rot_speed, rot_end, first, limit, accel, gravity, data);
            a += span;
        }
    }

    /// `createCircleShot`: `way` shots all the way round, `distance` out from `pos`.
    #[allow(clippy::too_many_arguments)]
    pub fn circle(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        data: ShotData,
        way: i32,
        distance: f64,
        mut base_angle: f32,
    ) {
        if way % 2 == 0 {
            base_angle += 360.0 / (way as f32 * 2.0);
        }
        let wide = 360.0 - 360.0 / way as f32;
        self.wide(pos, angle, rot_speed, rot_end, first, limit, accel, gravity, data, way, wide, distance, base_angle);
    }

    /// `createCircleShot(user, pos, angle, speed, shot, way)`.
    pub fn circle_simple(&mut self, pos: V3, angle: V3, speed: f64, data: ShotData, way: i32) {
        self.circle(pos, angle, 0.0, 9999, speed, speed, 0.0, V3::ZERO, data, way, f64::from(data.size), 0.0);
    }

    /// `createRingShot`: `way` shots on a cone `span` degrees wide about `angle`.
    #[allow(clippy::too_many_arguments)]
    pub fn ring(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        mut data: ShotData,
        way: i32,
        distance: f64,
        span: f32,
        base_angle: f32,
    ) {
        let half = span / 2.0;
        let yaw = yaw_of(-angle.x, angle.z);
        let pitch = pitch_of(-angle.x, -angle.y, angle.z);
        let base = vec_from_angle(yaw, pitch + half, 1.0);
        let base_over = vec_from_angle(yaw, pitch + half + 90.0, 1.0);
        let over = vec_from_angle(yaw, pitch + 90.0, 1.0);
        let mut a = base_angle;
        let step = 360.0 / way as f32;
        let mut color = self.rng.int(7) as usize;
        let rainbow = data.color == RAINBOW;
        if data.color == RANDOM {
            data.color = self.rng.int(8) as u8;
        }
        for _ in 0..way {
            if rainbow {
                data.color = RAINBOW_ORDER[color % 7];
                color += 1;
            }
            let moving = rotate(angle, base, a);
            let extend = rotate(angle, over, a);
            let axis = rotate(angle, base_over, a);
            self.shot_turning(pos + extend * distance, moving, a, axis, rot_speed, rot_end, first, limit, accel, gravity, data);
            a += step;
        }
    }

    /// `createRingShot(user, pos, angle, speed, shot, way, distance, span, base)`.
    #[allow(clippy::too_many_arguments)]
    pub fn ring_simple(&mut self, pos: V3, angle: V3, speed: f64, data: ShotData, way: i32, distance: f64, span: f32, base: f32) {
        self.ring(pos, angle, 0.0, 9999, speed, speed, 0.0, V3::ZERO, data, way, distance, span, base);
    }

    /// `createRandomRingShot`: `num` shots at random within a cone `span` degrees wide.
    #[allow(clippy::too_many_arguments)]
    pub fn random_ring(
        &mut self,
        pos: V3,
        angle: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        mut data: ShotData,
        num: i32,
        distance: f64,
        span: f32,
    ) {
        let max = span / 2.0;
        let yaw = yaw_of(-angle.x, angle.z);
        let pitch = pitch_of(angle.x, angle.y, angle.z);
        let over = vec_from_angle(-yaw, pitch + 90.0, 1.0);
        let mut color = self.rng.int(7) as usize;
        let rainbow = data.color == RAINBOW;
        if data.color == RANDOM {
            data.color = self.rng.int(8) as u8;
        }
        let axis = v3(-angle.x, -angle.y, angle.z);
        for _ in 0..num {
            if rainbow {
                data.color = RAINBOW_ORDER[color % 7];
                color += 1;
            }
            let a = self.rng.float() * 360.0;
            let base = vec_from_angle(-yaw, pitch + self.rng.float() * max, 1.0);
            let moving = rotate(axis, base, a);
            let extend = rotate(axis, over, a);
            let at = v3(pos.x - extend.x * distance, pos.y - extend.y * distance, pos.z + extend.z * distance);
            self.shot_turning(at, v3(-moving.x, -moving.y, moving.z), a, angle, rot_speed, rot_end, first, limit, accel, gravity, data);
        }
    }

    /// `createSphereShot`: rings stacked into a ball, `way` picking how many and how full.
    #[allow(clippy::too_many_arguments)]
    pub fn sphere(
        &mut self,
        pos: V3,
        angle: V3,
        slope: f32,
        axis: V3,
        rot_speed: f32,
        rot_end: i32,
        first: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        mut data: ShotData,
        way: usize,
        distance: f64,
        base_angle: f32,
    ) {
        let Some(rows) = WAYS.get(way) else { return };
        let front = pos + angle * distance;
        let back = pos - angle * distance;
        let span = if rows.len() >= 2 { 360.0 / (rows.len() - 1) as f32 } else { 0.0 };
        let mut ring_base = 0.0;
        let mut front_done = false;
        let mut tilt = 0.0;
        if data.color == RANDOM {
            data.color = self.rng.int(8) as u8;
        }
        for &n in *rows {
            if n == 1 {
                if !front_done {
                    self.shot_turning(front, angle, slope, axis, rot_speed, rot_end, first, limit, accel, gravity, data);
                    front_done = true;
                } else {
                    self.shot_turning(back, -angle, slope, axis, rot_speed, rot_end, first, limit, accel, gravity, data);
                }
            } else {
                self.ring(pos, angle, rot_speed, rot_end, first, limit, accel, gravity, data, n, distance, ring_base, base_angle + tilt);
                tilt += 180.0 / rows.len() as f32;
            }
            ring_base += span;
        }
    }
}

/// `THShotLib.ways`: the rings of each sphere, from one pole to the other.
const WAYS: [&[i32]; 45] = [
    &[0], &[1], &[1, 1], &[1, 0, 2, 0], &[1, 0, 3, 0], &[1, 3, 1], &[1, 4, 1], &[1, 5, 1], &[1, 3, 3, 1],
    &[1, 5, 3, 0], &[1, 4, 4, 1], &[1, 6, 4, 0], &[1, 5, 5, 1], &[1, 3, 5, 3, 1], &[1, 3, 6, 3, 1],
    &[1, 4, 5, 4, 1], &[1, 4, 6, 4, 1], &[1, 4, 7, 4, 1], &[1, 3, 5, 5, 3, 1], &[1, 3, 6, 6, 3],
    &[1, 3, 6, 6, 3, 1], &[1, 5, 9, 5, 1], &[1, 3, 7, 7, 3, 1], &[1, 6, 9, 6, 1], &[1, 4, 7, 7, 4, 1],
    &[1, 3, 5, 7, 5, 3, 1], &[1, 3, 5, 8, 5, 3, 1], &[1, 3, 5, 9, 5, 3, 1], &[1, 3, 6, 8, 6, 3, 1],
    &[1, 3, 6, 9, 6, 3, 1], &[1, 4, 6, 8, 6, 4, 1], &[1, 3, 6, 11, 6, 3, 1], &[1, 3, 6, 12, 6, 3, 1], &[0],
    &[0], &[0], &[0], &[0], &[0], &[0], &[0], &[0], &[0], &[0], &[1, 3, 6, 12, 12, 6, 3, 1],
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shot::{form, shot};
    use crate::vec::angle;

    fn fire<T>(f: impl FnOnce(&mut Fire) -> T) -> Vec<Shot> {
        let mut out = Vec::new();
        let mut rng = Rng::new(1);
        f(&mut Fire { out: &mut out, rng: &mut rng });
        out
    }

    #[test]
    fn a_circle_spreads_its_shots_evenly_round() {
        let shots = fire(|f| f.circle_simple(V3::ZERO, angle(0.0, 0.0), 0.3, shot(form::SMALL, 0, 0, 80, 0), 8));
        assert_eq!(shots.len(), 8);
        for s in &shots {
            assert!((s.speed() - 0.3).abs() < 1e-6);
            assert!(s.motion.y.abs() < 1e-9, "a level circle stays level");
        }
        let sum = shots.iter().fold(V3::ZERO, |a, s| a + s.motion);
        assert!(sum.length() < 1e-6, "evenly round sums to nothing: {sum:?}");
    }

    #[test]
    fn a_ring_is_a_cone_about_its_direction() {
        let dir = angle(40.0, -10.0);
        let shots = fire(|f| f.ring_simple(V3::ZERO, dir, 0.4, shot(form::CRYSTAL, 3, 0, 80, 0), 6, 0.6, 45.0, 0.0));
        assert_eq!(shots.len(), 6);
        let cos = (22.5f64).to_radians().cos();
        for s in &shots {
            let d = s.angle;
            let along = d.x * dir.x + d.y * dir.y + d.z * dir.z;
            assert!((along - cos).abs() < 1e-3, "{along} vs {cos}");
        }
    }

    #[test]
    fn rainbow_and_random_colours_are_real_colours() {
        let shots = fire(|f| {
            f.ring_simple(V3::ZERO, angle(0.0, 0.0), 0.4, shot(form::SMALL, RAINBOW, 0, 80, 0), 14, 0.0, 30.0, 0.0);
            f.random_ring(V3::ZERO, angle(0.0, 0.0), 0.0, 9999, 1.0, 1.0, 0.0, V3::ZERO, shot(0, RANDOM, 0, 60, 0), 5, 0.1, 50.0);
        });
        assert!(shots.iter().all(|s| s.color < 8));
    }

    #[test]
    fn wide_lasers_fan_out_and_grow() {
        let shots = fire(|f| f.wide_laser_a(V3::ZERO, angle(0.0, 0.0), 0.5, 0.5, 0.0, V3::ZERO, laser(7, 0.1, 4.0, 4.0, 0, 80, 0), 3, 30.0, 0.0));
        assert_eq!(shots.len(), 3);
        assert!(shots.iter().all(|s| s.is_laser() && s.laser == 0.0 && s.laser_max == 4.0 && s.color == 7));
        let held = fire(|f| f.laser_b(V3::ZERO, angle(0.0, 0.0), rotate_default(), -2.0, 9999, laser(24, 4.2, 60.0, 8.0, 30, 120, 0), Some((1.0, 0.9))));
        assert_eq!((held[0].form, held[0].color, held[0].laser), (3, 0, 60.0));
    }

    #[test]
    fn a_sphere_of_twelve_is_poles_and_rings() {
        let shots = fire(|f| {
            f.sphere(V3::ZERO, angle(0.0, 0.0), 0.0, rotate_default(), 0.0, 9999, 0.2, 0.6, 0.05, V3::ZERO, shot(form::LIGHT, 4, 0, 40, 0), 12, 2.0, 0.0)
        });
        assert_eq!(shots.len(), 1 + 5 + 5 + 1);
    }
}
