// SPDX-License-Identifier: MIT
fn click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id != crate::game::firstpersonmodel::ID { return; }
    match button {
        0 => game.firstperson_toggle_hands(),
        1 => game.firstperson_toggle_auto_hide(),
        _ => {},
    }
}
