//! Better Dynamic Lights for Horizon, a native mod on Horizon's mod API: torches, lanterns and
//! other glowing things light up the world as they move.
//!
//! - `runtime`: the settings, texts and light levels, with no game types (`cargo test` tests
//!   them on their own).
//! - `store`: the settings as the game keeps them, one file per setting.
//! - `lights`: the light sources near the camera, supplied to the block-light renderer each frame.
//! - `net`: what only the server knows of the entities (what they hold, fire, dropped items,
//!   lit minecarts), sent to the client as the mod's own payloads.
//! - `screen`: the settings screen over Options > Mods > Better Dynamic Lights.
pub mod runtime;

#[cfg(not(test))]
mod lights;
#[cfg(not(test))]
mod net;
#[cfg(not(test))]
mod screen;
#[cfg(not(test))]
mod store;

// The launcher's entry calls `crate::hooks()`, which `#[horizon_mod]` writes beside the impl.
#[cfg(not(test))]
pub use entry::hooks;
#[cfg(not(test))]
pub use lights::with_frame_sources;

#[cfg(not(test))]
mod entry {
    use super::{lights, net, screen, store};
    use mod_api::*;

    #[derive(Default)]
    struct DynamicLights;

    #[mod_api::horizon_mod]
    impl Hooks for DynamicLights {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            store::register(r);
            net::register(r);
        }

        fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
            net::client_tick(host);
            lights::tick(host);
            screen::tick(host);
            store::flush(host);
        }
        fn gui_layer_post(&self, host: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
            if event.layer == "minecraft:overlay_message" {
                lights::draw_line(host, &mut *event.draw);
            }
        }
        fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
            lights::key(host, event);
        }
        fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut SettingChangedEvent<'_>) {
            store::setting_changed(host, event.key);
            screen::setting_changed(host, event.key);
        }

        fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
            if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
                net::server_tick(host);
            }
        }
        fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
            net::player_left(event.player);
        }

        fn screen_render_pre(&self, host: &dyn ClientHost, event: &mut ScreenRenderPreEvent<'_>) -> Flow {
            flow(screen::covers(host, event.screen))
        }
        fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
            screen::render(host, event);
        }
        fn screen_mouse_button_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
            flow(screen::pressed(host, event.screen, event.button))
        }
        fn screen_mouse_button_released_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseButtonReleasedPreEvent<'_>) -> Flow {
            flow(screen::released(host, event.screen, event.button))
        }
        fn screen_mouse_dragged_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseDraggedPreEvent<'_>) -> Flow {
            flow(screen::dragged(host, event.screen))
        }
        fn screen_mouse_scrolled_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseScrolledPreEvent<'_>) -> Flow {
            flow(screen::scrolled(host, event.screen, event.scroll))
        }
        fn screen_key_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
            flow(screen::key_pressed(host, event.screen, event.key, event.modifiers))
        }
        fn screen_key_released_pre(&self, host: &dyn ClientHost, event: &mut ScreenKeyReleasedPreEvent<'_>) -> Flow {
            flow(screen::swallowed(host, event.screen))
        }
        fn screen_char_typed_pre(&self, host: &dyn ClientHost, event: &mut ScreenCharTypedPreEvent<'_>) -> Flow {
            flow(screen::swallowed(host, event.screen))
        }
    }

    fn flow(cancel: bool) -> Flow {
        if cancel { Flow::Cancel } else { Flow::Continue }
    }
}
