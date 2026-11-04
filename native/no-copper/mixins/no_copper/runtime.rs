// SPDX-License-Identifier: MIT
use serde_json::Value;
use std::collections::BTreeMap;
pub const ID: &str = "no_copper";
pub const VERSION: &str = "0.1.0";
fn copper(v: &Value) -> bool {
    let name = v.as_str().or_else(|| v.get("Name").and_then(Value::as_str));
    matches!(
        name,
        Some(
            "minecraft:copper_ore"
                | "minecraft:deepslate_copper_ore"
                | "copper_ore"
                | "deepslate_copper_ore"
        )
    )
}
fn visit(v: &mut Value) -> bool {
    let mut changed = false;
    match v {
        Value::Array(a) => {
            for v in a {
                changed |= visit(v);
            }
        }
        Value::Object(o) => {
            let kind = o
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim_start_matches("minecraft:")
                .to_owned();
            if kind == "ore_vein" && o.get("ore_block").is_some_and(copper) {
                *v = serde_json::json!({"type":"minecraft:sequence","sequence":[]});
                return true;
            }
            if kind == "ore" || kind == "scattered_ore" {
                let config = if o.get("config").is_some_and(Value::is_object) {
                    o.get_mut("config").unwrap().as_object_mut().unwrap()
                } else {
                    &mut *o
                };
                if let Some(Value::Array(targets)) = config.get_mut("targets") {
                    let old = targets.len();
                    targets.retain(|t| !t.get("state").is_some_and(copper));
                    let empty = targets.is_empty();
                    changed |= old != targets.len();
                    if empty && old > 0 {
                        config.insert("size".into(), Value::from(0));
                    }
                }
            }
            for v in o.values_mut() {
                changed |= visit(v);
            }
        }
        _ => {}
    }
    changed
}
/// Runs after all generation overrides, including terrain mods and data packs.
pub fn rewrite(bundle: &str, base: &mut BTreeMap<String, String>) -> usize {
    if bundle != "features" && bundle != "rules" {
        return 0;
    }
    let mut count = 0;
    for json in base.values_mut() {
        if !json.contains("copper_ore") {
            continue;
        }
        if let Ok(mut value) = serde_json::from_str::<Value>(json)
            && visit(&mut value)
        {
            *json = value.to_string();
            count += 1;
        }
    }
    count
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_normal_deepslate_and_mixed_targets_only() {
        let mut features=BTreeMap::from([("ore".into(),r#"{"type":"minecraft:ore","size":20,"targets":[{"state":"minecraft:copper_ore"},{"state":{"Name":"minecraft:deepslate_copper_ore"}},{"state":"minecraft:iron_ore"}]}"#.into())]);
        assert_eq!(rewrite("features", &mut features), 1);
        let v: Value = serde_json::from_str(&features["ore"]).unwrap();
        assert_eq!(v["targets"].as_array().unwrap().len(), 1);
        assert_eq!(v["size"], 20);
        let mut copper = BTreeMap::from([(
            "ore".into(),
            r#"{"type":"minecraft:ore","size":20,"targets":[{"state":"minecraft:copper_ore"}]}"#
                .into(),
        )]);
        rewrite("features", &mut copper);
        let v: Value = serde_json::from_str(&copper["ore"]).unwrap();
        assert_eq!(v["size"], 0);
        assert!(v["targets"].as_array().unwrap().is_empty());
    }
    #[test]
    fn disables_entire_copper_vein_but_preserves_iron_and_other_bundles() {
        let mut rules=BTreeMap::from([("copper".into(),r#"{"type":"minecraft:ore_vein","ore_block":"minecraft:copper_ore","raw_ore_block":"minecraft:raw_copper_block"}"#.into()),("iron".into(),r#"{"type":"minecraft:ore_vein","ore_block":"minecraft:deepslate_iron_ore"}"#.into())]);
        let before = rules.clone();
        assert_eq!(rewrite("rules", &mut rules), 1);
        assert_eq!(rules["iron"], before["iron"]);
        assert_eq!(
            serde_json::from_str::<Value>(&rules["copper"]).unwrap()["sequence"],
            serde_json::json!([])
        );
        let mut unrelated = before.clone();
        assert_eq!(rewrite("biomes", &mut unrelated), 0);
        assert_eq!(unrelated, before);
    }
}
