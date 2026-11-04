fn list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "sleep_threshold".into(), title: "Sleep Threshold 0.1.0".into(),
        description: "Choose how many eligible players must sleep to skip the night. The server or world host controls this setting.".into(), keys: Vec::new(),
    });
}
