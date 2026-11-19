// Gravestone Core Mod List Entry
fn gravestone_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::gravestone_glue::listed(mods);
}
