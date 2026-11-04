// SPDX-License-Identifier: MIT
fn settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::lambdynamiclights as lights;
    if mod_id == lights::ID {
        buttons.extend(lights::Settings::read(|k| game.native_setting(lights::ID, k)).labels());
    }
}
