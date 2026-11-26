// SPDX-License-Identifier: MIT
use mod_api::{
    ClientHost, ClientTickPostEvent, Flow, Hooks, Registrar, RegistrationContext, RenderHandEvent,
    SettingChangedEvent, SettingSpec,
};
use std::cell::Cell;

pub mod view_policy;

pub const ID: &str = "firstpersonmodel_horizon";

thread_local! {
    static SETTINGS: Cell<(bool, bool, bool)> = const { Cell::new((true, true, true)) };
    static CANCEL_HANDS: Cell<bool> = const { Cell::new(false) };
}

pub fn settings() -> (bool, bool, bool) {
    SETTINGS.with(Cell::get)
}

pub fn cancel_hands(cancel: bool) {
    CANCEL_HANDS.with(|state| state.set(cancel));
}

fn refresh(host: &dyn ClientHost) {
    SETTINGS.with(|state| {
        state.set((
            host.setting_bool("enabled").unwrap_or(true),
            host.setting_bool("auto_hide_body").unwrap_or(true),
            host.setting_bool("vanilla_hands").unwrap_or(true),
        ));
    });
}

#[derive(Default)]
struct FirstPersonModel;

#[mod_api::horizon_mod]
impl Hooks for FirstPersonModel {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = registrar.register_setting(SettingSpec::toggle(
            "enabled",
            "firstpersonmodel_horizon.enabled",
            true,
        ));
        let _ = registrar.register_setting(SettingSpec::toggle(
            "vanilla_hands",
            "firstpersonmodel_horizon.vanilla_hands",
            true,
        ));
        let _ = registrar.register_setting(SettingSpec::toggle(
            "auto_hide_body",
            "firstpersonmodel_horizon.auto_hide_body",
            true,
        ));
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        refresh(host);
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        refresh(host);
    }

    fn render_hand(&self, _: &dyn ClientHost, _: &mut RenderHandEvent<'_, '_>) -> Flow {
        if CANCEL_HANDS.with(Cell::get) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}
