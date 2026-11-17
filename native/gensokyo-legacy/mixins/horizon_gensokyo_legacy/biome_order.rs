// Migrate old shared-feature ordering from the player's current vanilla resources.
fn biome_order(overrides: &WorldgenOverrides, bundle: &str, base: &mut BTreeMap<String, String>) {
    if bundle != "biomes"
        || !overrides.mod_enabled("horizon_gensokyo_legacy")
        || !base
            .keys()
            .any(|k| k.starts_with("horizon_gensokyo_legacy:"))
    {
        return;
    }
    use crate::worldgen::feature_sort::{FeatureIdentity, build_features_per_step};
    let mut ids = BTreeMap::new();
    let mut inputs = Vec::new();
    for (name, text) in base.iter() {
        if !name.starts_with("minecraft:")
            || matches!(
                name.as_str(),
                "minecraft:nether_wastes"
                    | "minecraft:crimson_forest"
                    | "minecraft:warped_forest"
                    | "minecraft:soul_sand_valley"
                    | "minecraft:basalt_deltas"
                    | "minecraft:the_end"
                    | "minecraft:small_end_islands"
                    | "minecraft:end_midlands"
                    | "minecraft:end_highlands"
                    | "minecraft:end_barrens"
            )
        {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(text).expect("biome JSON");
        let mut steps = Vec::new();
        for row in v["features"].as_array().into_iter().flatten() {
            let mut features = Vec::new();
            for value in row.as_array().into_iter().flatten() {
                if let Some(name) = value.as_str() {
                    let next = FeatureIdentity(ids.len() as u32);
                    features.push(*ids.entry(name.to_owned()).or_insert(next));
                }
            }
            steps.push(features);
        }
        inputs.push(steps);
    }
    let plan = build_features_per_step(&inputs).expect("current vanilla Overworld feature order");
    let ranks: Vec<_> = plan
        .iter()
        .map(|step| {
            step.features()
                .iter()
                .enumerate()
                .map(|(rank, id)| (*id, rank))
                .collect::<BTreeMap<_, _>>()
        })
        .collect();
    for (name, text) in base.iter_mut() {
        if !name.starts_with("horizon_gensokyo_legacy:") {
            continue;
        }
        let mut v: serde_json::Value = serde_json::from_str(text).expect("ported biome JSON");
        for (step, row) in v["features"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(ranks) = ranks.get(step) else {
                continue;
            };
            let row = row.as_array_mut().expect("biome features");
            let rank = |value: &serde_json::Value| {
                value
                    .as_str()
                    .and_then(|name| ids.get(name))
                    .and_then(|id| ranks.get(id))
                    .copied()
            };
            let mut shared: Vec<_> = row
                .iter()
                .filter_map(|v| rank(v).map(|r| (r, v.clone())))
                .collect();
            shared.sort_by_key(|v| v.0);
            let mut shared = shared.into_iter();
            for value in row.iter_mut() {
                if rank(value).is_some() {
                    *value = shared.next().unwrap().1;
                }
            }
        }
        *text = v.to_string();
    }
}
