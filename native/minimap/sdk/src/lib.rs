pub mod runtime;
pub mod state;

use mod_api::*;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        registrar.register_key_mapping(
            KeyRegistration::new("waypoints_key", "key.keyboard.u").label("minimap.waypoints"),
        );
        for (key, label, choices, default) in [
            (
                "corner_choice",
                "minimap.corner",
                &[
                    "minimap.top_left",
                    "minimap.top_right",
                    "minimap.bottom_left",
                    "minimap.bottom_right",
                ][..],
                1,
            ),
            (
                "size_choice",
                "minimap.size",
                &["minimap.small", "minimap.medium", "minimap.large"][..],
                0,
            ),
            (
                "shape_choice",
                "minimap.shape",
                &["minimap.square", "minimap.round"][..],
                0,
            ),
            (
                "zoom_choice",
                "minimap.zoom",
                &[
                    "minimap.zoom_half",
                    "minimap.zoom_one",
                    "minimap.zoom_two",
                    "minimap.zoom_four",
                ][..],
                1,
            ),
            (
                "outline_choice",
                "minimap.outline",
                &["minimap.black", "minimap.stone", "minimap.bamboo"][..],
                1,
            ),
        ] {
            let _ = registrar.register_setting(SettingSpec::choice(
                key,
                label,
                choices.iter().copied(),
                default,
            ));
        }
        for (key, label, default) in [
            ("rotate", "minimap.rotate", false),
            ("show_coordinates", "minimap.coordinates", true),
            ("player_head", "minimap.player_head", false),
        ] {
            let _ = registrar.register_setting(SettingSpec::toggle(key, label, default));
        }
    }

    fn setting_changed(&self, _: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        runtime::clear_settings();
    }

    fn client_setup(&self, host: &mut dyn ClientHost, _: &mut ClientSetupEvent) {
        let get = |key: &str| host.setting(runtime::ID, key);
        let settings = runtime::Settings::read(get);
        for (key, new_key, value) in [
            ("corner", "corner_choice", settings.corner),
            ("size", "size_choice", settings.size),
            ("shape", "shape_choice", usize::from(settings.round)),
            ("zoom", "zoom_choice", settings.zoom),
            ("outline", "outline_choice", settings.outline),
        ] {
            if get(key).is_some() && get(new_key).is_none() {
                host.set_setting(runtime::ID, new_key, &value.to_string());
            }
        }
        runtime::clear_settings();
    }
}
