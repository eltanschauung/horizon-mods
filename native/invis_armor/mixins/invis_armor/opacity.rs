// SPDX-License-Identifier: MIT
fn opacity(_result: f32, game: &crate::game::Game<'_>, slot: &str) -> f32 {
    crate::game::invis_armor::opacity(game, slot)
}
