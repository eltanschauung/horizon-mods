// SPDX-License-Identifier: MIT
fn click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::lambdynamiclights as lights;
    if mod_id != lights::ID {
        return;
    }
    let mut s = lights::Settings::read(|k| game.native_setting(lights::ID, k));
    s.click(button);
    for (key, value) in s.stored() {
        game.native_set_setting(lights::ID, key, &value);
    }
    crate::game::lambdynamiclights_bridge::invalidate();
}
