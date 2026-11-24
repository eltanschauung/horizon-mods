// SPDX-License-Identifier: EUPL-1.2-or-later

use mod_api::serde_json::json;
use mod_api::{Host, ItemKey, ItemStack, Tag};

use crate::shot_number as number;

fn key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&mod_api::Identifier::parse(id).ok()?)
}

fn take(slot: &mut Option<ItemStack>, count: u32) {
    if count == 0 {
        return;
    }
    if let Some(stack) = slot {
        let left = stack.count().saturating_sub(count);
        if left == 0 {
            *slot = None;
        } else {
            let _ = stack.set_count(left);
        }
    }
}

pub fn run(host: &dyn Host, slots: &mut Vec<Option<ItemStack>>) -> bool {
    slots.resize(7, None);
    if slots[2].is_some() {
        return false;
    }
    if slots[0].is_some() && slots[1].is_none() {
        let core = slots[0].as_ref().unwrap();
        let item = core.item().id(host);
        if let Some(family) = item
            .strip_prefix("arcadiandream:")
            .and_then(|s| s.strip_suffix("_bullet_core"))
        {
            if slots[3]
                .as_ref()
                .is_some_and(|s| s.item().id(host).as_str() == "arcadiandream:big_power_item")
            {
                let Some(item) = key(host, &format!("arcadiandream:{family}_shot")) else {
                    return false;
                };
                let result = ItemStack::of(item);
                take(&mut slots[0], 1);
                take(&mut slots[3], 1);
                slots[2] = Some(result);
                return true;
            }
        }
        return false;
    }
    if slots[0].is_some() {
        return false;
    }
    let Some(original) = slots[1]
        .as_ref()
        .filter(|s| s.item().id(host).ends_with("_shot"))
    else {
        return false;
    };
    let mut result = original.clone();
    let id = result.item().id(host);
    let Some(family) = id
        .strip_prefix("arcadiandream:")
        .and_then(|id| id.strip_suffix("_shot"))
    else {
        return false;
    };
    let mut data = crate::items::shot_settings(&result, family);
    if !data.is_object() {
        return false;
    }
    let mut used = [0u32; 7];
    if let Some(modifier) = &slots[3] {
        let field = match modifier.item().id(host).as_str() {
            "arcadiandream:power_item" => "power",
            "arcadiandream:big_power_item" => "density",
            "arcadiandream:point_item" => "speed",
            "arcadiandream:star_item" => "duration",
            _ => "",
        };
        if !field.is_empty() {
            let maxkey = match field {
                "power" => "maxPower",
                "density" => "maxDensity",
                "speed" => "maxSpeed",
                _ => "maxDuration",
            };
            let maximum = number(&data, maxkey, 100.0);
            for _ in 0..modifier.count() {
                let old = number(&data, field, 1.0);
                if old >= maximum {
                    break;
                }
                let next = if field == "density" {
                    old + 1.0
                } else {
                    let n = old * (1.0 + 0.05 * (1.0 - old / (maximum * 2.0)));
                    if field == "duration" { n.ceil() } else { n }
                };
                data[field] = json!(next.min(maximum));
                data["cooldown"] = json!((number(&data, "cooldown", 5.0) + 1.0).min(number(
                    &data,
                    "maxCooldown",
                    100.0
                )));
                for (other, max, minimum) in [
                    ("power", "maxPower", 0.1),
                    ("speed", "maxSpeed", 0.05),
                    ("duration", "maxDuration", 20.0),
                ] {
                    if other != field {
                        let v = number(&data, other, 1.0);
                        let n = (v * (1.0 - 0.02 * v / number(&data, max, 100.0))).max(minimum);
                        data[other] = json!(if other == "duration" { n.floor() } else { n });
                    }
                }
                used[3] += 1;
            }
        } else if modifier.item().id(host).as_str() == "arcadiandream:time_orb"
            && data["timeOrbUsed"] != true
        {
            data["timeOrbUsed"] = json!(true);
            used[3] = 1;
        } else if modifier.item().id(host).as_str() == "arcadiandream:enchanted_ice"
            && data["icy"] != true
        {
            data["icy"] = json!(true);
            data["divergence"] = json!((number(&data, "divergence", 0.0) + 5.0).min(number(
                &data,
                "maxDivergence",
                90.0
            )));
            used[3] = 1;
        }
    }
    if let Some(repair) = slots[4]
        .as_ref()
        .filter(|s| s.item().id(host).as_str() == "arcadiandream:faith_item")
    {
        used[4] = result.damage().div_ceil(50).min(repair.count());
        result.set_damage(result.damage().saturating_sub(used[4] * 50));
    }
    if let Some(p) = &slots[5] {
        let item = p.item().id(host);
        if let Some(pattern) = item
            .strip_prefix("arcadiandream:")
            .and_then(|s| s.strip_suffix("_pattern"))
        {
            if ["spread", "ray", "ring", "arc", "double", "triple"].contains(&pattern) {
                data["pattern"] = json!(pattern);
                used[5] = 1;
            }
        }
    }
    if let Some(dye) = &slots[6] {
        let item = dye.item().id(host);
        if let Some(color) = item
            .strip_prefix("minecraft:")
            .and_then(|s| s.strip_suffix("_dye"))
        {
            if let Some(rgb) = dye_color(color) {
                data["color"] = json!(rgb);
                if result
                    .apply_components(
                        &[("minecraft:dyed_color".into(), Tag::Int(rgb as i32))].into(),
                    )
                    .is_err()
                {
                    return false;
                }
                used[6] = 1;
            }
        }
    }
    if used.iter().all(|n| *n == 0) {
        return false;
    }
    data["modified"] = json!(true);
    if crate::items::set_custom(&mut result, &data).is_none() {
        return false;
    }
    take(&mut slots[1], 1);
    for i in 3..7 {
        take(&mut slots[i], used[i]);
    }
    slots[2] = Some(result);
    true
}

fn dye_color(s: &str) -> Option<u32> {
    Some(match s {
        "white" => 16777215,
        "orange" => 16753920,
        "magenta" => 16711935,
        "light_blue" => 3847130,
        "yellow" => 16770048,
        "lime" => 12582656,
        "pink" => 15961002,
        "gray" => 4210752,
        "light_gray" => 8421504,
        "cyan" => 65535,
        "purple" => 10494192,
        "blue" => 1908223,
        "brown" => 9849600,
        "green" => 65280,
        "red" => 16711680,
        "black" => 657930,
        _ => return None,
    })
}
