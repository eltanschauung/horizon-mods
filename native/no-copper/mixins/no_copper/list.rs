// SPDX-License-Identifier: MIT
fn no_copper_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use horizon_world::worldgen::overrides::no_copper as policy;
    mods.push(crate::game::native_hooks::NativeMod {id:policy::ID.into(),title:format!("No Copper {}",policy::VERSION),description:"Disables copper ore deposits and large copper veins in newly generated chunks. Required only on the world host.".into(),keys:vec![]});
}
