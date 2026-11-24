// SPDX-License-Identifier: EUPL-1.2-or-later

use crate::items::ItemStack;
use crate::projectiles::{Hit, Projectile, ProjectileKind, ProjectileWorld};
use horizon_core::{geometry::Vec3, nbt::Tag};
use serde_json::{Value, json};

pub fn data(shot: &Projectile) -> Value {
    shot.item
        .as_ref()
        .and_then(|item| item.data()?.other("minecraft:custom_data"))
        .and_then(|tag| tag.get("arcadiandream"))
        .map(mod_arcadiandream::read_shot_data)
        .unwrap_or_else(|| json!({}))
}

fn store(shot: &mut Projectile, data: &Value) {
    let ProjectileKind::Thrown(kind) = shot.kind else {
        return;
    };
    let item = shot
        .item
        .get_or_insert_with(|| ItemStack::new(kind.item, 1));
    let mut patch = match item
        .data()
        .and_then(|data| data.other("minecraft:custom_data"))
    {
        Some(Tag::Compound(patch)) => patch.clone(),
        _ => Default::default(),
    };
    patch.insert(
        "arcadiandream".into(),
        mod_arcadiandream::write_shot_data(data),
    );
    item.data_mut()
        .set_loot_component("minecraft:custom_data", &Some(Tag::Compound(patch)));
}

fn is_danmaku(shot: &Projectile) -> bool {
    matches!(shot.kind, ProjectileKind::Thrown(kind) if kind.id.starts_with("arcadiandream:") && kind.id.ends_with("_bullet"))
}

pub fn tick(
    shot: &mut Projectile,
    world: &dyn ProjectileWorld,
    impact: bool,
) -> Option<(bool, Hit)> {
    if !is_danmaku(shot) {
        return None;
    }
    let mut data = data(shot);
    let duration =
        mod_arcadiandream::shot_number(&data, "duration", 100.0).clamp(1.0, 1200.0) as u32;
    if shot.age >= duration {
        return Some((false, Hit::None));
    }
    shot.previous = shot.position;
    shot.previous_yaw = shot.yaw;
    shot.previous_pitch = shot.pitch;
    shot.age += 1;
    shot.velocity.y -= mod_arcadiandream::shot_number(&data, "gravity", 0.0);
    let from = shot.position;
    let to = Vec3::new(
        from.x + shot.velocity.x,
        from.y + shot.velocity.y,
        from.z + shot.velocity.z,
    );
    let block = world.clip(from, to);
    let end = block.map_or(to, |hit| hit.0);
    let scale = mod_arcadiandream::shot_number(&data, "scale", 1.0).clamp(0.1, 4.0);
    let radius = scale
        * match shot.kind.id() {
            "arcadiandream:bubble_bullet" => 0.375,
            "arcadiandream:star_bullet" => 0.25,
            "arcadiandream:pellet_bullet" => 0.075,
            _ => 0.125,
        };
    let hit = if impact {
        let target = world
            .targets()
            .iter()
            .filter(|(owner, _)| {
                Some(*owner) != shot.owner
                    && !data["hit_entities"].as_array().is_some_and(|entities| {
                        let owner = format!("{owner:?}");
                        entities
                            .iter()
                            .any(|id| id.as_str() == Some(owner.as_str()))
                    })
            })
            .filter_map(|(owner, bounds)| {
                let at = bounds
                    .inflate(Vec3::new(radius, radius, radius))
                    .clip(from, end)?;
                Some((*owner, at))
            })
            .min_by(|(_, a), (_, b)| {
                a.subtract(from)
                    .length_squared()
                    .total_cmp(&b.subtract(from).length_squared())
            });
        if let Some((who, at)) = target {
            Hit::Entity { who, at }
        } else if let Some((at, pos, face)) = block {
            Hit::Block { pos, at, face }
        } else {
            Hit::None
        }
    } else {
        Hit::None
    };
    shot.position = match hit {
        Hit::Entity { at, .. } | Hit::Block { at, .. } => at,
        Hit::None => to,
    };
    shot.left_owner = true;
    let velocity = shot.velocity;
    shot.yaw = velocity.x.atan2(velocity.z).to_degrees() as f32;
    shot.pitch = velocity.y.atan2(velocity.x.hypot(velocity.z)).to_degrees() as f32;
    let mut piercing = false;
    if shot.kind.id() == "arcadiandream:kunai_bullet"
        && let Hit::Entity { who, .. } = hit
    {
        piercing = true;
        if !data["hit_entities"].is_array() {
            data["hit_entities"] = json!([]);
        }
        if let Some(seen) = data["hit_entities"].as_array_mut()
            && seen.len() < 256
        {
            seen.push(json!(format!("{who:?}")));
        }
        store(shot, &data);
    }
    Some((matches!(hit, Hit::None) || piercing, hit))
}

pub fn drawn(shot: &Projectile) -> Option<ItemStack> {
    let ProjectileKind::Thrown(kind) = shot.kind else {
        return None;
    };
    if !is_danmaku(shot) {
        return None;
    }
    let mut item = ItemStack::new(kind.item, 1);
    let data = data(shot);
    item.data_mut().dyed_color = Some(data["color"].as_i64().unwrap_or(0xff0000) as i32);
    let scale = ((mod_arcadiandream::shot_number(&data, "scale", 1.0) - 1.0) / 0.75)
        .round()
        .clamp(0.0, 4.0) as u8;
    let name = kind
        .id
        .strip_prefix("arcadiandream:")?
        .strip_suffix("_bullet")?;
    let phase = if name == "star" { shot.age % 16 } else { 0 };
    let model = format!("arcadiandream:arcadian_projectile/{name}_{scale}_{phase}");
    item.data_mut().set_loot_component(
        "minecraft:item_model",
        &Some(Tag::String(model.as_str().into())),
    );
    Some(item)
}

pub fn save(shot: &Projectile, tag: &mut Tag) {
    if is_danmaku(shot)
        && let Tag::Compound(tag) = tag
    {
        tag.insert("arcadiandream:age".into(), Tag::Int(shot.age as i32));
    }
}

pub fn load(shot: &mut Projectile, tag: &Tag) {
    if !is_danmaku(shot) {
        return;
    }
    if let Some(Tag::Int(age)) = tag.get("arcadiandream:age") {
        shot.age = (*age).max(0) as u32;
    }
    // Older worlds saved the same settings separately from the thrown item's own components.
    if let Some(Tag::String(saved)) = tag.get("arcadiandream:shot")
        && let Ok(data) = serde_json::from_str::<Value>(&saved.to_string_lossy())
    {
        store(shot, &data);
    }
}
