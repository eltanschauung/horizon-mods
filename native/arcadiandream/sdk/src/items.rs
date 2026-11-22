// SPDX-License-Identifier: EUPL-1.2-or-later
// Native adaptation of Arcadian Dream by Maxmani and contributors.

use mod_api::serde_json::{self, Value};
use mod_api::{
    ClientHost, HookId, Host, HostSide, HostTransaction, InteractionResult, ItemPart, ItemPlace,
    ItemStack, Registrar, ServerHost, Tag, Text, TooltipEvent, UseItemEvent, UseOnEvent,
};
use std::sync::OnceLock;

const CUSTOM_DATA: &str = "minecraft:custom_data";

pub(crate) fn register(registrar: &mut dyn Registrar) {
    for family in crate::danmaku::FAMILIES {
        let item = registrar.item_key(&format!("arcadiandream:{family}_shot"));
        registrar.item_part(item, Tooltip::Shot(family));
    }
    for item in [
        "hihiirokane_axe",
        "hihiirokane_hoe",
        "hihiirokane_pickaxe",
        "hihiirokane_shovel",
        "hihiirokane_sword",
    ] {
        let item = registrar.item_key(&format!("arcadiandream:{item}"));
        registrar.item_part(item, Tooltip::Autosmelt);
    }
    let mallet = registrar.item_key("arcadiandream:mochi_mallet");
    registrar.item_part(mallet, Tooltip::Mallet);
}

pub(crate) fn custom(stack: &ItemStack) -> Option<&mod_api::Compound> {
    let Tag::Compound(root) = stack.components()?.as_nbt().get(&CUSTOM_DATA.into())? else {
        return None;
    };
    let Tag::Compound(arcadian) = root.get(&"arcadiandream".into())? else {
        return None;
    };
    Some(arcadian)
}

fn number(value: &Tag) -> Option<f64> {
    match value {
        Tag::Double(n) => Some(f64::from_bits(*n)),
        Tag::Float(n) => Some(f64::from(f32::from_bits(*n))),
        Tag::Int(n) => Some(f64::from(*n)),
        Tag::Long(n) => Some(*n as f64),
        Tag::Short(n) => Some(f64::from(*n)),
        _ => None,
    }
    .filter(|n| n.is_finite())
}

pub(crate) fn autosmelt(stack: &ItemStack) -> bool {
    match custom(stack).and_then(|data| data.get(&"autosmelt".into())) {
        Some(Tag::Byte(enabled)) => *enabled != 0,
        _ => true,
    }
}

pub(crate) fn shot_settings(stack: &ItemStack, family: &str) -> Value {
    static SHOTS: OnceLock<Value> = OnceLock::new();
    let defaults =
        SHOTS.get_or_init(|| serde_json::from_str(include_str!("data/shots.json")).unwrap());
    let mut settings = defaults[family].clone();
    if let (Some(base), Some(patch)) = (settings.as_object_mut(), custom(stack)) {
        base.extend(
            patch
                .iter()
                .map(|(key, value)| (key.to_string_lossy(), read_tag(value))),
        );
    }
    settings
}

pub fn read_tag(tag: &Tag) -> Value {
    match tag {
        Tag::Byte(n) => Value::from(*n != 0),
        Tag::Short(n) => Value::from(*n),
        Tag::Int(n) => Value::from(*n),
        Tag::Long(n) => Value::from(*n),
        Tag::String(s) => Value::from(s.to_string_lossy()),
        Tag::List(list) => Value::Array(list.iter().map(read_tag).collect()),
        Tag::Compound(entries) => Value::Object(
            entries
                .iter()
                .map(|(k, v)| (k.to_string_lossy(), read_tag(v)))
                .collect(),
        ),
        _ => number(tag).map_or(Value::Null, Value::from),
    }
}

pub fn write_tag(value: &Value) -> Tag {
    match value {
        Value::Bool(value) => Tag::Byte(i8::from(*value)),
        Value::Number(n) => n
            .as_i64()
            .map(Tag::Long)
            .unwrap_or_else(|| Tag::Double(n.as_f64().unwrap_or_default().to_bits())),
        Value::String(value) => Tag::String(value.as_str().into()),
        Value::Array(values) => Tag::List(values.iter().map(write_tag).collect()),
        Value::Object(values) => Tag::Compound(
            values
                .iter()
                .map(|(key, value)| (key.as_str().into(), write_tag(value)))
                .collect(),
        ),
        Value::Null => Tag::Byte(0),
    }
}

pub(crate) fn set_custom(stack: &mut ItemStack, data: &Value) -> Option<()> {
    let mut root = stack
        .components()
        .and_then(|patch| patch.as_nbt().get(&CUSTOM_DATA.into()))
        .and_then(|tag| match tag {
            Tag::Compound(root) => Some(root.clone()),
            _ => None,
        })
        .unwrap_or_default();
    root.insert("arcadiandream".into(), write_tag(data));
    stack
        .apply_components(&[(CUSTOM_DATA.into(), Tag::Compound(root))].into())
        .ok()
}

fn toggle_autosmelt(
    host: &mut dyn ServerHost,
    uuid: u128,
    hand: mod_api::Hand,
) -> Option<InteractionResult> {
    let player = host.player(uuid)?;
    if hand != mod_api::Hand::Main || player.is_secondary_use_active(host) != Some(true) {
        return None;
    }
    let offhand = ItemPlace::PlayerHand {
        player: uuid,
        hand: mod_api::Hand::Off,
    };
    if offhand.stack(host).is_some() {
        return None;
    }
    let place = ItemPlace::PlayerHand { player: uuid, hand };
    host.transaction(|host, tx| {
        let mut updated = false;
        let changed = place.edit(host, tx, |stack| {
            let mut mode = custom(stack)
                .map(|data| read_tag(&Tag::Compound(data.clone())))
                .unwrap_or_else(|| serde_json::json!({}));
            mode["autosmelt"] = Value::from(!autosmelt(stack));
            updated = set_custom(stack, &mode).is_some();
        });
        (changed && updated).then_some(()).ok_or(())
    })
    .ok()?;
    let _ = player.action_bar(host, Text::literal("Hihiirokane tool mode switched."));
    Some(InteractionResult::SuccessServer)
}

enum Tooltip {
    Shot(&'static str),
    Autosmelt,
    Mallet,
}

impl ItemPart for Tooltip {
    fn item_subscribed(&self) -> &'static [Option<HookId>] {
        match self {
            Self::Shot(_) => &[
                Some(HookId::Tooltip),
                Some(HookId::UseItem),
                Some(HookId::UseOn),
                Some(HookId::DispenseItem),
            ],
            Self::Autosmelt => &[
                Some(HookId::Tooltip),
                Some(HookId::UseItem),
                Some(HookId::UseOn),
            ],
            _ => &[Some(HookId::Tooltip)],
        }
    }

    fn use_item(
        &self,
        host: HostSide<'_>,
        event: &mut UseItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        match self {
            Self::Shot(family) => crate::danmaku::fire(host, event, family),
            Self::Autosmelt => toggle_autosmelt(host, event.player, event.hand),
            _ => None,
        }
    }

    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        match self {
            Self::Shot(family) => {
                crate::danmaku::fire_hand(host, event.player, event.hand, event.stack, family)
            }
            Self::Autosmelt => toggle_autosmelt(host, event.player, event.hand),
            _ => None,
        }
    }

    fn dispense_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::DispenseItemEvent<'_>,
    ) -> Option<(Option<ItemStack>, i32)> {
        match self {
            Self::Shot(_) => crate::danmaku::dispense(host, event),
            _ => None,
        }
    }

    fn tooltip(&self, _: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let stack = event.stack;
        let text = match self {
            Self::Shot(family) => {
                let settings = shot_settings(stack, family);
                let stat = |key, fallback| crate::danmaku::number(&settings, key, fallback);
                format!(
                    "Power {:.2}  Speed {:.2}  Duration {:.0}  Density {:.0}  Cooldown {:.0}",
                    stat("power", 2.0),
                    stat("speed", 0.8),
                    stat("duration", 100.0),
                    stat("density", 1.0),
                    stat("cooldown", 5.0),
                )
            }
            Self::Autosmelt => format!(
                "Autosmelt: {} (sneak + use with empty offhand)",
                autosmelt(stack),
            ),
            Self::Mallet => format!(
                "Kills: {:.0}",
                custom(stack)
                    .and_then(|data| data.get(&"kill_count".into()))
                    .and_then(number)
                    .unwrap_or(0.0),
            ),
        };
        event.lines.push((Text::literal(text), 0xffaaaaaa));
    }
}

pub(crate) fn uses_autosmelt(host: &dyn Host, stack: &ItemStack) -> bool {
    stack
        .item()
        .id(host)
        .starts_with("arcadiandream:hihiirokane_")
        && autosmelt(stack)
}
