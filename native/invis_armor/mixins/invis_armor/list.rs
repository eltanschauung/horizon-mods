// SPDX-License-Identifier: MIT
fn list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "invis_armor".into(),
        title: "Invisible Armor 0.1.0".into(),
        description: "Client-only armor and elytra visibility. Configurable opacity and Hats Only."
            .into(),
        keys: vec![],
    });
}
