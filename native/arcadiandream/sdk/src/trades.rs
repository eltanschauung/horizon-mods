// SPDX-License-Identifier: EUPL-1.2-or-later
// Trade prices, experience and shot ranges from Arcadian Dream.

use mod_api::serde_json::{Value, json};
use mod_api::{
    AttachmentType, Compound, DataPackFilesEvent, EntityInteractEvent, Holder, Identifier,
    ItemStack, MerchantOffer, PoiKey, ProfessionDefinition, ProfessionKey, ProfessionWorkstation,
    RegisterVillagerProfessionsEvent, ServerHost, Tag, TradeSetKey,
};

const ANTIQUARIAN: &str = "arcadiandream:antiquarian";

pub fn register(event: &mut RegisterVillagerProfessionsEvent<'_>) {
    let id = |name| Identifier::parse(name).unwrap();
    let poi = PoiKey(id("arcadiandream:danmaku_crafting_table"));
    event.professions.register(ProfessionDefinition {
        key: ProfessionKey(id(ANTIQUARIAN)),
        workstations: vec![ProfessionWorkstation {
            poi: poi.clone(),
            block: id("arcadiandream:danmaku_crafting_table"),
            properties: Vec::new(),
        }],
        held_poi: vec![poi.clone()],
        acquirable_poi: vec![poi],
        requested_items: Vec::new(),
        secondary_blocks: Vec::new(),
        work_sound: None,
        trades: (1..=5)
            .map(|level| {
                let name = format!("arcadiandream:antiquarian/level_{level}");
                (level, TradeSetKey(Identifier::parse(&name).unwrap()))
            })
            .collect(),
    });
}

pub fn files(event: &mut DataPackFilesEvent<'_>) {
    let mut file = |path: &str, value: Value| {
        event.files.push((
            path.to_owned(),
            mod_api::serde_json::to_vec(&value).unwrap(),
        ));
    };
    let offer = |wants: Value, gives: Value, max_uses, xp, price_multiplier| {
        json!({
            "wants": wants, "gives": gives, "max_uses": max_uses, "xp": xp, "price_multiplier": price_multiplier,
        })
    };
    let stack = |id, count| json!({"id": id, "count": count});
    for (name, count, uses) in [
        ("star_item", 16, 12),
        ("power_item", 8, 16),
        ("point_item", 8, 16),
    ] {
        let value = offer(
            json!({"id":format!("arcadiandream:{name}"),"count":count}),
            stack("minecraft:emerald", 1),
            uses,
            1,
            0.05,
        );
        file(
            &format!("villager_trade/arcadiandream:antiquarian/{name}"),
            value,
        );
    }
    for (name, price, uses, xp, multiplier) in [
        ("faith_item", 12, 12, 10, 0.05),
        ("healing_charm", 16, 12, 20, 0.2),
    ] {
        let value = offer(
            stack("minecraft:emerald", price),
            json!({"id":format!("arcadiandream:{name}"),"count":1}),
            uses,
            xp,
            multiplier,
        );
        file(
            &format!("villager_trade/arcadiandream:antiquarian/{name}"),
            value,
        );
    }
    // Trade-set selection has the same prices and settings range as the
    // previous generated offer. All families and colors appear in the pool.
    let mut shots = Vec::new();
    let colors = [
        16711680, 16753920, 16770048, 12582656, 65280, 65535, 3847130, 1908223, 10494192, 16711935,
        15961002,
    ];
    for (i, family) in crate::danmaku::FAMILIES.into_iter().enumerate() {
        for (j, color) in colors.into_iter().enumerate() {
            let n = i * colors.len() + j;
            let name = format!("arcadiandream:antiquarian/{family}_{j}");
            let shot = json!({
                "id": format!("arcadiandream:{family}_shot"), "count": 1,
                "components": {
                    "minecraft:dyed_color": color,
                    "minecraft:custom_data": {"arcadiandream": {
                        "power": 6 + n * 7 % 5,
                        "speed": 0.6 + f64::from((n * 313 % 60) as u32) / 100.0,
                        "duration": 40 + n * 11 % 41,
                        "cooldown": 30 + n * 17 % 31,
                        "density": 3 + n * 13 % 6,
                        "color": color,
                    }},
                },
            });
            file(
                &format!("villager_trade/{name}"),
                offer(stack("minecraft:emerald", 32), shot, 3, 30, 0.2),
            );
            shots.push(name);
        }
    }
    let id = |name| format!("arcadiandream:antiquarian/{name}");
    for (level, amount, offers) in [
        (
            1,
            2,
            vec![id("star_item"), id("power_item"), id("point_item")],
        ),
        (2, 1, vec![id("faith_item")]),
        (3, 1, vec![id("healing_charm")]),
        (4, 1, shots.clone()),
        (5, 1, shots.clone()),
    ] {
        let sequence = format!("arcadiandream:antiquarian/level_{level}");
        file(
            &format!("trade_set/{sequence}"),
            json!({
                "amount": amount, "trades": offers, "random_sequence": sequence,
            }),
        );
    }
}

static WANDERING: AttachmentType<bool> = AttachmentType {
    namespace: "arcadiandream",
    name: "wandering_offers",
    default: || false,
    copy_on_death: false,
    sync: false,
    codec: Some(mod_api::AttachmentCodec {
        encode: |value| {
            Ok(Compound::from([(
                "rolled".into(),
                Tag::Byte(i8::from(*value)),
            )]))
        },
        decode: |data| Ok(matches!(data.get(&"rolled".into()), Some(Tag::Byte(1)))),
    }),
};

pub fn interact(host: &mut dyn ServerHost, event: &EntityInteractEvent<'_>) -> Option<()> {
    if !matches!(
        event.kind,
        "minecraft:villager" | "minecraft:wandering_trader"
    ) {
        return None;
    }
    let entity = host.entity(u32::try_from(event.entity).ok()?)?;
    let merchant = host.merchant(entity)?;
    if event.kind == "minecraft:villager" {
        let saved = merchant.offers(host)?;
        if merchant.profession(host).as_deref() != Some(ANTIQUARIAN)
            && saved.iter().any(|offer| {
                offer
                    .base_cost_a
                    .stack
                    .item()
                    .id(host)
                    .starts_with("arcadiandream:")
            })
            && merchant.set_profession(host, ANTIQUARIAN)
        {
            // An older villager's offers can include paid-for trades and
            // configured danmaku. Keep them on the same persistent villager.
            for (index, offer) in saved.into_iter().enumerate() {
                if index < merchant.offers(host)?.len() {
                    let _ = merchant.set_offer(host, index as u32, offer);
                } else {
                    let _ = merchant.add_offer(host, offer);
                }
            }
        }
        if merchant.profession(host).as_deref() == Some(ANTIQUARIAN)
            && merchant.level(host).unwrap_or(0) >= 5
            && !merchant
                .offers(host)?
                .iter()
                .any(|offer| offer.result.item().id(host) == "arcadiandream:bomb_item")
        {
            let cost = stack(host, "minecraft:emerald", 42)?;
            let bomb = stack(host, "arcadiandream:bomb_item", 1)?;
            let offer = MerchantOffer::new(mod_api::ItemCost::new(cost), None, bomb, 3, 30, 0.2);
            let _ = merchant.add_offer(host, offer);
        }
    } else {
        let holder = Holder::Entity(entity.id());
        if WANDERING.get(host, holder).unwrap_or(false) {
            return None;
        }
        let tick = entity.level(host)?.game_time(host).unwrap_or_default();
        let roll = (tick as u64 ^ u64::from(entity.id())).wrapping_mul(0x9e3779b97f4a7c15) >> 32;
        let already = merchant.offers(host)?;
        for (name, price, denom) in [("ghastly_lantern", 12, 10), ("life_fragment", 8, 6)] {
            let name = format!("arcadiandream:{name}");
            if roll % denom != 0
                || already
                    .iter()
                    .any(|offer| offer.result.item().id(host) == name)
            {
                continue;
            }
            let result = stack(host, &name, 1)?;
            let cost = stack(host, "minecraft:emerald", price)?;
            let offer = MerchantOffer::new(mod_api::ItemCost::new(cost), None, result, 3, 1, 0.05);
            let _ = merchant.add_offer(host, offer);
        }
        let _ = WANDERING.set(host, holder, true);
    }
    Some(())
}

fn stack(host: &dyn ServerHost, name: &str, count: u32) -> Option<ItemStack> {
    ItemStack::new(host.item_key(&Identifier::parse(name).ok()?)?, count)
}
