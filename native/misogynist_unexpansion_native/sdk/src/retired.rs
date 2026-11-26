//! What the game had no business adding.
//!
//! - No Dried Ghast: its recipe is gone and a broken one drops nothing (with the happy ghast a
//!   removed mob, it would hatch nothing anyway). Nether fossils still place one now and then: the
//!   prebuilt game lets no mod replace that piece of world generation.
//! - No Pigstep: a Pigstep disc in any loot table is one of C418's own discs instead, at random.

use mod_api::serde_json::{Value, json};

/// C418's discs, the original ones.
pub const C418_DISCS: [&str; 12] = ["13", "cat", "blocks", "chirp", "far", "mall", "mellohi", "stal", "strad", "ward", "11", "wait"];

const PIGSTEP: &str = "minecraft:music_disc_pigstep";

/// Data pack files: the Dried Ghast's recipe taken away, and its block's drop.
pub fn files() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "recipe/minecraft:dried_ghast".into(),
            br#"{"neoforge:conditions":[{"type":"neoforge:false"}]}"#.to_vec(),
        ),
        ("loot_table/minecraft:blocks/dried_ghast".into(), br#"{"type":"minecraft:block","pools":[]}"#.to_vec()),
    ]
}

/// Every Pigstep entry in `table` made a pick of one of C418's discs; how many there were.
pub fn replace_pigstep(table: &mut Value) -> usize {
    match table {
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some("minecraft:item") && o.get("name").and_then(Value::as_str) == Some(PIGSTEP) {
                o.remove("name");
                o.insert("type".into(), json!("minecraft:loot_table"));
                let discs: Vec<Value> = C418_DISCS
                    .iter()
                    .map(|d| json!({"type": "minecraft:item", "name": format!("minecraft:music_disc_{d}")}))
                    .collect();
                o.insert("value".into(), json!({"pools": [{"rolls": 1, "entries": discs}]}));
                return 1;
            }
            o.values_mut().map(replace_pigstep).sum()
        }
        Value::Array(a) => a.iter_mut().map(replace_pigstep).sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pigstep_becomes_any_c418_disc_with_its_weight() {
        let mut t: Value = mod_api::serde_json::from_str(
            r#"{"pools":[{"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:gold_ingot","weight":5},{"type":"minecraft:item","name":"minecraft:music_disc_pigstep","weight":2}]}]}"#,
        )
        .unwrap();
        assert_eq!(replace_pigstep(&mut t), 1);
        let e = &t["pools"][0]["entries"][1];
        assert_eq!(e["type"], "minecraft:loot_table");
        assert_eq!(e["weight"], 2);
        assert!(e.get("name").is_none());
        let discs = e["value"]["pools"][0]["entries"].as_array().unwrap();
        assert_eq!(discs.len(), 12);
        assert_eq!(discs[0]["name"], "minecraft:music_disc_13");
        assert_eq!(t["pools"][0]["entries"][0]["name"], "minecraft:gold_ingot");
        assert!(!t.to_string().contains("pigstep"));
    }

    #[test]
    fn the_dried_ghast_recipe_is_switched_off() {
        let f = files();
        assert!(f.iter().any(|(k, v)| k == "recipe/minecraft:dried_ghast" && String::from_utf8_lossy(v).contains("neoforge:false")));
    }
}
