// SPDX-License-Identifier: MIT
fn settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::invis_armor::ID {
        buttons.push("Configure armor visibility...".into());
    }
}
