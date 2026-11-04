// SPDX-License-Identifier: MIT
fn listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::lambdynamiclights as lights;
    mods.push(crate::game::native_hooks::NativeMod {
        id: lights::ID.into(),
        title: format!("LambDynamicLights Horizon {}", lights::VERSION),
        description: lights::DESCRIPTION.into(),
        keys: vec![],
    });
}
