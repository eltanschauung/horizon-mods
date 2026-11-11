// SPDX-License-Identifier: MIT
fn settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::firstpersonmodel::ID {
        buttons.push(format!("Hands: {}", if game.firstperson_vanilla_hands() { "Vanilla" } else { "Body" }));
    }
}
