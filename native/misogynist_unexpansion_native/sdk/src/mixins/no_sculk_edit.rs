// Misogynist Unexpansion: no sculk, leaf litter, wildflowers or pink petals in new terrain. A
// feature tree, fully expanded, with every sculk patch given no rounds of spreading or growth and
// every sculk-vein multiface growth allowed on no face (neither then places anything), every
// leaf litter, wildflower or pink petal block it would place made air, and the trees' leaf-litter
// carpets (`place_on_ground` decorators) taken off. None when the tree has none of them.

/// The blocks no longer on the ground.
const GONE: [&str; 3] = ["minecraft:leaf_litter", "minecraft:wildflowers", "minecraft:pink_petals"];

pub fn edit(expanded: &str) -> Option<String> {
    let quoted = |b: &&str| expanded.contains(&format!("\"{b}\""));
    if !expanded.contains("sculk") && !GONE.iter().any(quoted) {
        return None;
    }
    let mut value: serde_json::Value = serde_json::from_str(expanded).ok()?;
    walk(&mut value).then(|| value.to_string())
}

fn short(s: &str) -> &str {
    s.trim_start_matches("minecraft:")
}

/// Whether `v` is a block state of one of the blocks that are gone: a name, or `{"id"}`/`{"Name"}`.
fn gone_state(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::String(s) => GONE.contains(&s.as_str()),
        serde_json::Value::Object(m) => ["id", "Name"]
            .iter()
            .any(|k| m.get(*k).and_then(|n| n.as_str()).is_some_and(|n| GONE.contains(&n))),
        _ => false,
    }
}

/// Air, written the way `v` was.
fn air_like(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) if m.contains_key("Name") => serde_json::json!({ "Name": "minecraft:air" }),
        serde_json::Value::Object(_) => serde_json::json!({ "id": "minecraft:air" }),
        _ => serde_json::Value::String("minecraft:air".to_owned()),
    }
}

/// The changes made under `v`: whether there were any.
fn walk(v: &mut serde_json::Value) -> bool {
    let mut changed = false;
    match v {
        serde_json::Value::Object(map) => {
            let kind = map.get("type").and_then(|t| t.as_str()).map(|t| short(t).to_owned());
            match kind.as_deref() {
                Some("sculk_patch") => {
                    changed |= zero(map);
                    if let Some(serde_json::Value::Object(config)) = map.get_mut("config") {
                        changed |= zero(config);
                    }
                }
                Some("multiface_growth") => {
                    changed |= no_faces(map);
                    if let Some(serde_json::Value::Object(config)) = map.get_mut("config") {
                        changed |= no_faces(config);
                    }
                }
                _ => {}
            }
            // A weighted entry's or a simple provider's block state.
            for key in ["data", "state"] {
                if let Some(s) = map.get_mut(key)
                    && gone_state(s)
                {
                    *s = air_like(s);
                    changed = true;
                }
            }
            // A tree's carpet of leaf litter around its trunk.
            if let Some(serde_json::Value::Array(list)) = map.get_mut("decorators") {
                let before = list.len();
                list.retain(|d| {
                    let carpet = d.get("type").and_then(|t| t.as_str()).is_some_and(|t| short(t) == "place_on_ground");
                    let text = d.to_string();
                    !(carpet && GONE.iter().any(|b| text.contains(&format!("\"{b}\""))))
                });
                changed |= list.len() != before;
            }
            for child in map.values_mut() {
                changed |= walk(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                changed |= walk(child);
            }
        }
        _ => {}
    }
    changed
}

fn zero(map: &mut serde_json::Map<String, serde_json::Value>) -> bool {
    let mut changed = false;
    for key in ["spread_rounds", "growth_rounds"] {
        if map.get(key).and_then(|n| n.as_i64()).is_some_and(|n| n != 0) {
            map.insert(key.to_owned(), serde_json::Value::from(0));
            changed = true;
        }
    }
    changed
}

fn no_faces(map: &mut serde_json::Map<String, serde_json::Value>) -> bool {
    if map.get("block").and_then(|b| b.as_str()).is_none_or(|b| short(b) != "sculk_vein") {
        return false;
    }
    let mut changed = false;
    for key in ["can_place_on_ceiling", "can_place_on_floor", "can_place_on_wall"] {
        if map.get(key).and_then(|f| f.as_bool()) == Some(true) {
            map.insert(key.to_owned(), serde_json::Value::Bool(false));
            changed = true;
        }
    }
    changed
}
