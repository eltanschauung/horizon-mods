// SPDX-License-Identifier: MIT
use mod_api::{Hooks, Registrar, RegistrationContext, SettingSpec};

#[derive(Default)]
struct ChatHeads;

#[mod_api::horizon_mod]
impl Hooks for ChatHeads {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = registrar.register_setting(SettingSpec::toggle(
            "heads",
            "chatheads.setting.heads",
            true,
        ));
    }
}
