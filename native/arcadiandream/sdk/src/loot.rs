// SPDX-License-Identifier: EUPL-1.2-or-later
// Original drop rates: Maxmani and contributors' Arcadian Dream 0.1.0.
use mod_api::serde_json::{Value, json};
pub fn extend(id: &str, table: &mut Value) {
    let mut added = Vec::new();
    let item = |name: &str| json!({"type":"minecraft:item","name":format!("arcadiandream:{name}")});
    let pool = |chance: f64, entry: Value| json!({"rolls":1,"conditions":[{"condition":"minecraft:random_chance","chance":chance}],"entries":[entry]});
    let path = id.strip_prefix("minecraft:chests/").unwrap_or("");
    if [
        "simple_dungeon",
        "end_city_treasure",
        "bastion_treasure",
        "desert_pyramid",
        "woodland_mansion",
        "ancient_city_ice_box",
        "ancient_city",
        "buried_treasure",
        "jungle_temple",
        "abandoned_mineshaft",
        "stronghold_crossing",
        "stronghold_corridor",
    ]
    .contains(&path)
        || id == "arcadiandream:chests/abandoned_shrine_treasure"
    {
        let mut p = pool(0.05, item("time_orb"));
        p["entries"] = json!([item("time_orb"), item("folding_chair")]);
        added.push(p);
    }
    if path == "end_city_treasure" {
        for (name, chance) in [
            ("dragon_gem", 0.25),
            ("hihiirokane_upgrade_smithing_template", 0.1),
        ] {
            let mut p = pool(chance, item(name));
            p["functions"] = json!([{"function":"minecraft:set_count","count":{"type":"minecraft:uniform","min":1.0,"max":2.0}}]);
            added.push(p);
        }
    }
    let pattern = match path {
        "shipwreck_supply" | "shipwreck_treasure" | "shipwreck_map" => {
            Some(("spread_pattern_template", 0.1))
        }
        "pillager_outpost" => Some(("ray_pattern_template", 0.25)),
        "nether_bridge" => Some(("ring_pattern_template", 0.25)),
        "ancient_city" => Some(("arc_pattern_template", 0.25)),
        "abandoned_mineshaft" => Some(("double_pattern_template", 0.25)),
        "stronghold_corridor" | "stronghold_crossing" | "stronghold_library" => {
            Some(("triple_pattern_template", 0.1))
        }
        _ => None,
    };
    if let Some((name, chance)) = pattern {
        added.push(pool(chance, item(name)));
    }
    if ["igloo_chest", "ancient_city_ice_box"].contains(&path) {
        added.push(pool(0.1, item("enchanted_ice")));
    }
    if path.starts_with("shipwreck_") {
        let mut p = pool(0.1, item("enchanted_ice"));
        p["conditions"].as_array_mut().unwrap().push(json!({"condition":"minecraft:any_of","terms":[{"condition":"minecraft:location_check","predicate":{"biomes":"minecraft:frozen_ocean"}},{"condition":"minecraft:location_check","predicate":{"biomes":"minecraft:deep_frozen_ocean"}}]}));
        added.push(p);
    }
    if id == "minecraft:gameplay/fishing/fish" || id == "minecraft:gameplay/fishing/treasure" {
        if let Some(pools) = table["pools"].as_array_mut() {
            for p in pools {
                if let Some(entries) = p["entries"].as_array_mut() {
                    let mut e = item(if id.ends_with("/fish") {
                        "lamprey"
                    } else {
                        "life_fragment"
                    });
                    if id.ends_with("/fish") {
                        e["weight"] = json!(12);
                    }
                    entries.push(e);
                }
            }
        }
    }
    for p in &mut added {
        current(p);
    }
    if !added.is_empty() {
        if !table["pools"].is_array() {
            table["pools"] = json!([]);
        }
        table["pools"].as_array_mut().unwrap().extend(added);
    }
}

fn current(v: &mut Value) {
    match v {
        Value::Array(a) => {
            for v in a {
                current(v)
            }
        }
        Value::Object(o) => {
            for v in o.values_mut() {
                current(v)
            }
            if let Some(v) = o.remove("function") {
                o.insert("type".into(), v);
            }
            if !o.contains_key("type") {
                if let Some(v) = o.remove("condition") {
                    o.insert("type".into(), v);
                }
            }
            if let Some(v) = o.remove("functions") {
                o.insert("modifier".into(), v);
            }
        }
        _ => {}
    }
}

pub(crate) fn modify_drops(
    host: &mut dyn mod_api::ServerHost,
    event: &mut mod_api::LootDropsEvent<'_>,
) {
    use mod_api::{Identifier, ItemStack, RandomSource, RecipeInput};

    let context = event.context;
    let Some(tool) = context.tool else { return };
    let Some(level) = host.level(context.dimension) else {
        return;
    };
    if crate::items::uses_autosmelt(host, tool) {
        let smelting = Identifier::parse("minecraft:smelting").unwrap();
        for stack in event.drops.iter_mut() {
            let Ok(Some(matched)) =
                level.recipe_for(host, &smelting, &RecipeInput::Single(stack.clone()))
            else {
                continue;
            };
            let mut output = matched.output;
            let count = output.count().saturating_mul(stack.count());
            if output.set_count(count).is_ok() {
                *stack = output;
            }
        }
    }
    let peach_floor = crate::config::config()["hisouSwordOptions"]["minHeightForPeaches"]
        .as_f64()
        .unwrap_or(196.0);
    if context.table.ends_with("_leaves")
        && context.origin[1] >= peach_floor
        && tool.item().id(host) == "arcadiandream:hisou_sword"
        && level.with_random(host, |random| random.next_float() < 0.1) == Some(true)
        && let Some(peach) =
            host.item_key(&Identifier::parse("arcadiandream:heavenly_peach").unwrap())
    {
        event.drops.push(ItemStack::of(peach));
    }
}
