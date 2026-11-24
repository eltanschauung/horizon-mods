// SPDX-License-Identifier: MIT
/// User-facing elevation: straight ahead is 90 degrees, up is 180, down is 0.
/// Horizon's camera pitch instead has straight ahead at 0 and down at +90.
pub fn above_hide_angle(camera_pitch: f32) -> bool {
    90.0 - camera_pitch > 80.0
}
/// Keep the world body out of horizontal swimming/gliding views.
pub fn hide_body(camera_pitch: f32, swimming: bool, gliding: bool) -> bool {
    above_hide_angle(camera_pitch) || swimming || gliding
}
/// Above the threshold the body is absent, so the vanilla fist takes over.
pub fn hide_empty_hand(camera_pitch: f32) -> bool {
    !above_hide_angle(camera_pitch)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swaps_fist_and_body_and_hides_horizontal_poses() {
        assert!(hide_body(0.0, false, false));
        assert!(!hide_empty_hand(0.0));
        assert!(!hide_body(10.0, false, false));
        assert!(hide_empty_hand(10.0));
        assert!(hide_body(60.0, true, false));
        assert!(hide_body(60.0, false, true));
        assert!(!hide_body(60.0, false, false));
    }
    #[test]
    fn hides_above_eighty_in_the_users_angle_convention() {
        for pitch in [-90.0, -45.0, 0.0, 9.99] {
            assert!(above_hide_angle(pitch));
        }
        for pitch in [10.0, 10.01, 30.0, 45.0, 80.0, 90.0] {
            assert!(!above_hide_angle(pitch));
        }
    }
}
