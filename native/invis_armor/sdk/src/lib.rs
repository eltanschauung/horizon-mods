// SPDX-License-Identifier: MIT
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

use mod_api::{ClientHost, ClientTickPostEvent, Hooks, Registrar, RegistrationContext, SettingSpec};

const ID: &str = "invis_armor";
// Keep one coherent snapshot for the render hook (percentage in the low 7 bits).
const HATS_ONLY: u8 = 128;
const DISABLED: u8 = 127;
static SETTINGS: AtomicU8 = AtomicU8::new(DISABLED);

#[derive(Default)]
struct InvisibleArmor;

#[mod_api::horizon_mod]
impl Hooks for InvisibleArmor {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = registrar.register_setting(SettingSpec::range(
            "opacity",
            "invis_armor.opacity",
            0.0,
            100.0,
            1.0,
            100.0,
        ));
        let _ = registrar.register_setting(SettingSpec::toggle(
            "hats_only",
            "invis_armor.hats_only",
            true,
        ));
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let settings = if host.on(ID) {
            let opacity = host.setting_f64("opacity").unwrap_or(100.0).clamp(0.0, 100.0) as u8;
            let hats = if host.setting_bool("hats_only").unwrap_or(true) {
                HATS_ONLY
            } else {
                0
            };
            opacity | hats
        } else {
            DISABLED
        };
        SETTINGS.store(settings, Relaxed);
    }
}

pub fn opacity(original: f32, slot: &str) -> f32 {
    let settings = SETTINGS.load(Relaxed);
    if settings == DISABLED {
        return original;
    }
    if settings & HATS_ONLY != 0 && slot != "helmet" {
        return 0.0;
    }
    f32::from(settings & !HATS_ONLY) / 100.0
}
