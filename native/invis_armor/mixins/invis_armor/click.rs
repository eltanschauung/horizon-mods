// SPDX-License-Identifier: MIT
fn click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::invis_armor::ID && button == 0 {
        crate::game::invis_armor::open(game);
    }
}
