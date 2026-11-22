//! The vector maths of Touhou Items Mod's `THShotLib`, kept to its exact formulas: patterns depend on
//! its signs and conventions (yaw 0 faces +z, pitch up is negative), not on tidier ones.

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub const ZERO: V3 = v3(0.0, 0.0, 0.0);

    /// Its length as Minecraft's `MathHelper.sqrt_double` takes it, in single precision.
    pub fn length(self) -> f64 {
        f64::from(((self.x * self.x + self.y * self.y + self.z * self.z) as f32).sqrt())
    }

    pub fn normalize(self) -> V3 {
        let inverse = 1.0 / self.length();
        self * inverse
    }

    pub fn array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    pub fn from_array(a: [f64; 3]) -> V3 {
        v3(a[0], a[1], a[2])
    }

    pub fn lerp(self, to: V3, t: f64) -> V3 {
        self + (to - self) * t
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, k: f64) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

/// `getVecFromAngle`: the direction a yaw and pitch (degrees) look along, times `force`.
pub fn vec_from_angle(yaw: f32, pitch: f32, force: f64) -> V3 {
    let yaw = f64::from(yaw) / 180.0 * std::f64::consts::PI;
    let pitch = f64::from(pitch) / 180.0 * std::f64::consts::PI;
    v3(-yaw.sin() * pitch.cos() * force, -pitch.sin() * force, yaw.cos() * pitch.cos() * force)
}

/// `angle(yaw, pitch)`: a unit direction.
pub fn angle(yaw: f32, pitch: f32) -> V3 {
    vec_from_angle(yaw, pitch, 1.0)
}

/// `getYawFromVector`.
pub fn yaw_of(x: f64, z: f64) -> f32 {
    (x.atan2(z) * 180.0 / f64::from(std::f32::consts::PI)) as f32
}

/// `getPitchFromVector`.
pub fn pitch_of(x: f64, y: f64, z: f64) -> f32 {
    let flat = f64::from(((x * x + z * z) as f32).sqrt());
    (y.atan2(flat) * 180.0 / f64::from(std::f32::consts::PI)) as f32
}

/// `getVectorFromRotation`: `v` turned `degrees` about the axis `axis` (Rodrigues, as written there).
pub fn rotate(axis: V3, v: V3, degrees: f32) -> V3 {
    let a = f64::from(degrees) / 180.0 * std::f64::consts::PI;
    let (s, c) = a.sin_cos();
    let (rx, ry, rz) = (axis.x, axis.y, axis.z);
    v3(
        (rx * rx * (1.0 - c) + c) * v.x + (rx * ry * (1.0 - c) - rz * s) * v.y + (rz * rx * (1.0 - c) + ry * s) * v.z,
        (rx * ry * (1.0 - c) + rz * s) * v.x + (ry * ry * (1.0 - c) + c) * v.y + (ry * rz * (1.0 - c) - rx * s) * v.z,
        (rz * rx * (1.0 - c) - ry * s) * v.x + (ry * rz * (1.0 - c) + rx * s) * v.y + (rz * rz * (1.0 - c) + c) * v.z,
    )
}

/// `angle_ToPos`: the unit direction from `a` to `b`.
pub fn angle_to(a: V3, b: V3) -> V3 {
    (b - a).normalize()
}

/// A small, seedable generator standing in for `java.util.Random`: patterns only need its spread.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// `nextFloat`: in [0, 1).
    pub fn float(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// `nextDouble`: in [0, 1).
    pub fn double(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `nextInt(n)`: in [0, n).
    pub fn int(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n.max(1))) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: V3, b: V3) -> bool {
        (a - b).length() < 1e-6
    }

    #[test]
    fn yaw_zero_looks_south_and_pitch_up_is_negative() {
        assert!(near(angle(0.0, 0.0), v3(0.0, 0.0, 1.0)));
        assert!(near(angle(90.0, 0.0), v3(-1.0, 0.0, 0.0)));
        assert!(near(angle(0.0, -90.0), v3(0.0, 1.0, 0.0)));
    }

    #[test]
    fn yaw_and_pitch_come_back_from_a_direction() {
        let d = angle(30.0, 20.0);
        // getYawFromVector's convention is mirrored in x against getVecFromAngle's, as in the original.
        assert!((yaw_of(-d.x, d.z) - 30.0).abs() < 1e-3);
        assert!((pitch_of(d.x, -d.y, d.z) - 20.0).abs() < 1e-3);
    }

    #[test]
    fn a_quarter_turn_about_up_turns_south_to_east() {
        let turned = rotate(v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0), 90.0);
        assert!(near(turned, v3(1.0, 0.0, 0.0)), "{turned:?}");
    }

    #[test]
    fn the_generator_stays_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let f = r.float();
            assert!((0.0..1.0).contains(&f));
            assert!(r.int(8) < 8);
        }
    }
}
