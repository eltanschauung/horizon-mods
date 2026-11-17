fn noises(overrides: &WorldgenOverrides, bundle: &str, base: &mut BTreeMap<String, String>) {
    if bundle != "noises" || !overrides.mod_enabled("horizon_gensokyo_legacy") {
        return;
    }
    for (name, text) in [
        (
            "summit_surface",
            r#"{"firstOctave":-5,"amplitudes":[1.0,1.0]}"#,
        ),
        (
            "magical_forest_type",
            r#"{"firstOctave":-7,"amplitudes":[1.0,0.5]}"#,
        ),
        (
            "magical_forest_clearing",
            r#"{"firstOctave":-5,"amplitudes":[1.0]}"#,
        ),
    ] {
        base.entry(format!("horizon_gensokyo_legacy:{name}"))
            .or_insert_with(|| text.into());
    }
}
