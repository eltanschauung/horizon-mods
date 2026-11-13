// SPDX-License-Identifier: MIT
fn list_mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod { id: "horizon_old_combat_mod".into(), title: "Horizon Old Combat Mod 0.1.0".into(), description: "TheHangel's Old Combat Mod recreated for Horizon: instant attack recovery, old hand animation, sprint retention. /gamerule old_combat false restores vanilla combat.".into(), keys: vec![] });
}
