// SPDX-License-Identifier: MIT
/// User-facing elevation: straight ahead is 90 degrees, up is 180, down is 0.
/// Horizon's camera pitch instead has straight ahead at 0 and down at +90.
pub fn above_hide_angle(camera_pitch: f32) -> bool {
    90.0 - camera_pitch > 80.0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hides_above_eighty_in_the_users_angle_convention() {
        for pitch in [-90.0, -45.0, 0.0, 9.0] { assert!(above_hide_angle(pitch)); }
        for pitch in [10.0, 11.0, 45.0, 80.0, 90.0] { assert!(!above_hide_angle(pitch)); }
    }
}
