// SPDX-License-Identifier: MIT
fn click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::firstpersonmodel::ID && button == 0 { game.firstperson_toggle_hands(); }
}
