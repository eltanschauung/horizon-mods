// SPDX-License-Identifier: MIT
fn run_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    let mut args = command.split_whitespace();
    if args.next() != Some("ldlights") || *ran {
        return;
    }
    *ran = true;
    match args.next() {
        Some("reload") => {
            crate::game::lambdynamiclights_bridge::invalidate();
            game.native_chat("LambDynamicLights: light rules reloaded.");
        }
        Some("status") => {
            let s = crate::game::lambdynamiclights::Settings::read(|k| {
                game.native_setting(crate::game::lambdynamiclights::ID, k)
            });
            let text = serde_json::json!({"mode":s.mode.name(),"enabled":game.native_on(crate::game::lambdynamiclights::ID),"lights":game.dynamic_lights});
            game.native_chat(&format!("LambDynamicLights: {text}"));
        }
        _ => game.native_chat("LambDynamicLights: /ldlights status or /ldlights reload"),
    }
}
