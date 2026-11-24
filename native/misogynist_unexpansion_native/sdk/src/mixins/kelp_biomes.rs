// Kelp only in warm, lukewarm and frozen oceans (and their deep kinds): none in the ordinary and
// cold ones, which vanilla gives it to, and some in the warm and frozen ones, which it doesn't.
// The biome's feature list as the world generator reads it, with the kelp moved. A biome is known
// by its data: the ordinary and cold oceans are the ones with `kelp_cold` (the lukewarm ones have
// `kelp_warm` and keep it), the warm ocean the one with its coral, the frozen oceans the frozen
// ones. Each edited text is kept once, a handful per world.
fn kelp_biomes(value: &str) -> &str {
    use serde_json::Value;
    use std::{collections::HashMap, sync::Mutex};
    let has = |feature: &str| value.contains(&format!("\"minecraft:{feature}\""));
    let kelp = if has("kelp_cold") {
        None
    } else if has("warm_ocean_vegetation") && !has("kelp_warm") {
        Some("minecraft:kelp_warm")
    } else if value.contains("\"frozen\"") && value.contains("temperature_modifier") && !has("kelp_cold") {
        Some("minecraft:kelp_cold")
    } else {
        return value;
    };
    static KEPT: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
    let Ok(mut kept) = KEPT.lock() else { return value };
    let kept = kept.get_or_insert_with(HashMap::new);
    if let Some(done) = kept.get(value) {
        return done;
    }
    let Ok(mut json) = serde_json::from_str::<Value>(value) else { return value };
    let Some(steps) = json.get_mut("features").and_then(Value::as_array_mut) else { return value };
    let is_kelp = |v: &Value| v.as_str().is_some_and(|s| s.starts_with("minecraft:kelp"));
    for step in steps.iter_mut() {
        if let Some(list) = step.as_array_mut() {
            list.retain(|v| !is_kelp(v));
        }
    }
    // Vegetal decoration, where vanilla's kelp grows: just after the seagrass, as in the lukewarm
    // ocean, or after the last vanilla feature where there is none. Every biome's features must
    // keep one order, so kelp at the end, after features a mod adds behind it elsewhere (Wild
    // Caves' cobwebs), stops a world being made or opened ("feature order cycle found").
    if let Some(kelp) = kelp
        && let Some(list) = steps.get_mut(9).and_then(Value::as_array_mut)
    {
        let named = |v: &Value, prefix: &str| v.as_str().is_some_and(|s| s.starts_with(prefix));
        let at = list
            .iter()
            .rposition(|v| named(v, "minecraft:seagrass"))
            .or_else(|| list.iter().rposition(|v| named(v, "minecraft:")))
            .map_or(list.len(), |i| i + 1);
        list.insert(at, Value::from(kelp));
    }
    let Ok(text) = serde_json::to_string(&json) else { return value };
    let text: &'static str = Box::leak(text.into_boxed_str());
    kept.insert(value.to_owned(), text);
    text
}
