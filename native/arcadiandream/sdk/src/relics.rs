// SPDX-License-Identifier: EUPL-1.2-or-later
// Native adaptation of Arcadian Dream by Maxmani and contributors.

use std::collections::BTreeMap;

use mod_api::serde_json::{self, Value, json};
use mod_api::{
    ActiveEffect, AttachmentCodec, AttachmentError, AttachmentType, AttributeModifier,
    AttributeOperation, ClientHost, CommandSource, Compound, EffectApplicable, EffectCategory,
    Flow, GameMode, Holder, HookId, HostSide, HostTransaction, Identifier, InteractionResult,
    ItemPart, ItemPlace, ItemResource, ItemStack, LivingDeathEvent, LivingEntityUseItemFinishEvent,
    MobEffectApplicableEvent, ModifierLifetime, PlayerRef, Registrar, RunModCommandEvent,
    ServerHost, Tag, Text, TooltipEvent, UseItemEvent, UseOnEvent,
};

use crate::{config, items};

const ACCESSORIES: [(&str, &str); 3] = [
    ("arcadiandream:extend_item", "life"),
    ("arcadiandream:magatama_necklace", "necklace"),
    ("arcadiandream:fairy_charm", "charm"),
];

#[derive(Clone, Default)]
struct State {
    elixir: u8,
    fatigue: u32,
    fatigue_level: u8,
    tiny: bool,
    revive: u8,
    accessories: BTreeMap<String, Compound>,
}

static PLAYER: AttachmentType<State> = AttachmentType {
    namespace: "arcadiandream",
    name: "player",
    default: State::default,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: true,
    sync: false,
};

fn encode(player: &State) -> Result<Compound, AttachmentError> {
    let accessories = player
        .accessories
        .iter()
        .map(|(slot, stack)| (slot.as_str().into(), Tag::Compound(stack.clone())))
        .collect();
    Ok([
        ("elixir".into(), Tag::Byte(player.elixir as i8)),
        ("fatigue".into(), Tag::Int(player.fatigue as i32)),
        (
            "fatigue_level".into(),
            Tag::Byte(player.fatigue_level as i8),
        ),
        ("tiny".into(), Tag::Byte(i8::from(player.tiny))),
        ("revive".into(), Tag::Byte(player.revive as i8)),
        ("accessories".into(), Tag::Compound(accessories)),
    ]
    .into())
}

fn decode(value: &Compound) -> Result<State, AttachmentError> {
    let int = |key: &str| match value.get(&key.into()) {
        Some(Tag::Byte(value)) => i32::from(*value).max(0) as u32,
        Some(Tag::Int(value)) => (*value).max(0) as u32,
        _ => 0,
    };
    let accessories = match value.get(&"accessories".into()) {
        Some(Tag::Compound(slots)) => slots
            .iter()
            .filter_map(|(slot, value)| {
                let Tag::Compound(value) = value else {
                    return None;
                };
                Some((slot.to_string_lossy(), value.clone()))
            })
            .collect(),
        _ => Default::default(),
    };
    Ok(State {
        elixir: int("elixir").min(3) as u8,
        fatigue: int("fatigue"),
        fatigue_level: int("fatigue_level").min(4) as u8,
        tiny: int("tiny") != 0,
        revive: int("revive").min(2) as u8,
        accessories,
    })
}

fn legacy_uuid(uuid: u128) -> String {
    let hex = format!("{uuid:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn legacy(host: &dyn ServerHost, uuid: u128) -> Option<(State, i32)> {
    let bytes = std::fs::read(host.world_dir()?.join("arcadiandream/players.json")).ok()?;
    let file: Value = serde_json::from_slice(&bytes).ok()?;
    let player = file.get(legacy_uuid(uuid))?;
    let mut accessories = BTreeMap::new();
    for (_, slot) in ACCESSORIES {
        if let Some(array) = player["accessories"][slot].as_array()
            && array.len() <= 1024 * 1024
            && let Some(bytes) = array
                .iter()
                .map(|byte| u8::try_from(byte.as_u64()?).ok())
                .collect::<Option<Vec<_>>>()
            && let Some(stack) = mod_api::decode_network_compound(&bytes)
        {
            accessories.insert(slot.to_owned(), stack);
        }
    }
    let state = State {
        elixir: player["elixir"].as_u64().unwrap_or_default().min(3) as u8,
        fatigue: player["fatigue"].as_u64().unwrap_or_default().min(6000) as u32,
        fatigue_level: player["fatigue_level"].as_u64().unwrap_or_default().min(4) as u8,
        tiny: player["tiny"] == true,
        revive: 0,
        accessories,
    };
    let ethereal = player["ethereal"]
        .as_i64()
        .unwrap_or_default()
        .clamp(0, 300) as i32;
    Some((state, ethereal))
}

fn state(host: &mut dyn ServerHost, player: PlayerRef) -> State {
    let holder = Holder::Player(player.uuid());
    if PLAYER.has(host, holder) == Ok(true) {
        return PLAYER.get(host, holder).unwrap_or_default();
    }
    let (state, ethereal) = legacy(host, player.uuid()).unwrap_or_default();
    if PLAYER.set(host, holder, state.clone()).is_ok() && ethereal > 0 {
        add_effect(host, player, "arcadiandream:ethereal", ethereal, 0);
    }
    state
}

const NECKLACE_STATE: &str = "arcadiandream:necklace_state";
static NECKLACE_SERVER: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u128>>> =
    std::sync::LazyLock::new(std::sync::Mutex::default);
static NECKLACE_CLIENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct NecklacePacket;

impl mod_api::IPayloadHandler for NecklacePacket {
    fn handle(
        &self,
        bytes: mod_api::PayloadBytes,
        context: &mut dyn mod_api::IPayloadContext,
    ) -> Result<(), mod_api::PayloadHandleError> {
        if let [value] = bytes.0.as_slice()
            && context.client_host().is_ok()
        {
            NECKLACE_CLIENT.store(*value == 1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }
}

pub fn necklace_ready(player: u128, remote: bool) -> bool {
    if remote {
        NECKLACE_CLIENT.load(std::sync::atomic::Ordering::Relaxed)
    } else {
        NECKLACE_SERVER
            .lock()
            .is_ok_and(|players| players.contains(&player))
    }
}

pub fn setting(section: &str, key: &str, default: bool) -> bool {
    config::config()[section][key].as_bool().unwrap_or(default)
}

fn sync_necklace(host: &mut dyn ServerHost, player: PlayerRef, state: &State) {
    let equipped = state.accessories.contains_key("necklace");
    let changed = NECKLACE_SERVER.lock().is_ok_and(|mut players| {
        if equipped {
            players.insert(player.uuid())
        } else {
            players.remove(&player.uuid())
        }
    });
    if changed
        || player
            .level(host)
            .and_then(|level| level.game_time(host))
            .unwrap_or(0)
            .rem_euclid(20)
            == 0
    {
        let _ = player.send_payload(
            host,
            &mod_api::PayloadId(NECKLACE_STATE.into()),
            mod_api::PayloadBytes(vec![u8::from(equipped)]),
        );
    }
}

pub fn register(registrar: &mut dyn Registrar) {
    registrar.payload_handlers().registrar("1").play_to_client(
        mod_api::PayloadId(NECKLACE_STATE.into()),
        Some(std::sync::Arc::new(NecklacePacket)),
    );
    for (id, slot) in ACCESSORIES {
        let key = registrar.item_key(id);
        registrar.item_part(key, Relic::Accessory(slot));
    }
    for (id, relic) in [
        ("arcadiandream:ghastly_lantern", Relic::Lantern),
        ("arcadiandream:hourai_elixir", Relic::Elixir),
        ("arcadiandream:ibuki_gourd", Relic::Gourd),
        ("arcadiandream:miracle_mallet", Relic::Mallet),
        ("arcadiandream:bomb_item", Relic::Bomb),
    ] {
        let key = registrar.item_key(id);
        registrar.item_part(key, relic);
    }
}

enum Relic {
    Accessory(&'static str),
    Lantern,
    Elixir,
    Gourd,
    Mallet,
    Bomb,
}

impl Relic {
    fn use_hand(
        &self,
        host: &mut dyn ServerHost,
        uuid: u128,
        hand: mod_api::Hand,
        stack: &ItemStack,
    ) -> Option<InteractionResult> {
        let player = host.player(uuid)?;
        let holder = Holder::Player(uuid);
        let place = ItemPlace::PlayerHand { player: uuid, hand };
        match self {
            Self::Accessory(slot) => {
                if *slot == "charm" && player.is_secondary_use_active(host) == Some(true) {
                    let mode = (custom(stack)["mode"].as_i64().unwrap_or(0) + 1).rem_euclid(3);
                    edit_custom(host, &place, |data| data["mode"] = json!(mode))?;
                    player.set_item_cooldown(host, "arcadiandream:fairy_charm", 20);
                    let _ = player.action_bar(
                        host,
                        Text::literal(
                            [
                                "Fairy Charm: pacify",
                                "Fairy Charm: prevent spawning",
                                "Fairy Charm: pacify and prevent spawning",
                            ][mode as usize],
                        ),
                    );
                } else {
                    let mut state = state(host, player);
                    let previous = state
                        .accessories
                        .get(*slot)
                        .and_then(|tag| ItemStack::from_nbt(host, tag));
                    let mut equipped = stack.clone();
                    equipped.set_count(1).ok()?;
                    let saved = equipped.to_nbt(host)?;
                    let creative = player.game_mode(host) == Some(GameMode::Creative);
                    let committed = host
                        .transaction(|host, tx| {
                            if !creative
                                && place.extract(host, tx, &ItemResource::of(stack), 1) != 1
                            {
                                return Err(());
                            }
                            if let Some(previous) = previous
                                && !give_without_overflow(host, tx, uuid, &previous)
                            {
                                return Err(());
                            }
                            state.accessories.insert((*slot).to_owned(), saved);
                            PLAYER.set(host, holder, state).map_err(|_| ())
                        })
                        .is_ok();
                    let message = if committed {
                        format!("Equipped {slot}. /arcadian unequip {slot} returns it.")
                    } else {
                        "Make room in your inventory first.".to_owned()
                    };
                    let _ = player.action_bar(host, Text::literal(message));
                }
            }
            Self::Lantern => {
                if player.game_mode(host) != Some(GameMode::Creative) {
                    host.transaction(|host, tx| {
                        (place.extract(host, tx, &ItemResource::of(stack), 1) == 1)
                            .then_some(())
                            .ok_or(())
                    })
                    .ok()?;
                }
                add_effect(host, player, "arcadiandream:ethereal", 300, 0);
                player.set_item_cooldown(host, "arcadiandream:ghastly_lantern", 300);
            }
            Self::Elixir => {
                if config::config()["houraiElixirOptions"]["canDrink"] == true {
                    return None;
                }
                let _ = player.action_bar(
                    host,
                    Text::literal("Drinking the Hourai Elixir is disabled in the server config."),
                );
            }
            Self::Gourd => {
                if custom(stack)["filled"] == true {
                    return None;
                }
            }
            Self::Bomb => {
                let count = crate::danmaku::cancel_near_player(host, player, false);
                if count == 0 {
                    return Some(InteractionResult::Pass);
                }
                if player.game_mode(host) != Some(GameMode::Creative) {
                    host.transaction(|host, tx| {
                        (place.extract(host, tx, &ItemResource::of(stack), 1) == 1)
                            .then_some(())
                            .ok_or(())
                    })
                    .ok()?;
                }
                if let Some(star) =
                    host.item_key(&Identifier::parse("arcadiandream:star_item").unwrap())
                {
                    for chunk in (0..count).step_by(64) {
                        let stack = ItemStack::new(star, (count - chunk).min(64) as u32)?;
                        let _ = player.give_stack(host, stack);
                    }
                }
                player.set_item_cooldown(host, "arcadiandream:bomb_item", 100);
            }
            Self::Mallet => {
                if player.is_secondary_use_active(host) != Some(true) {
                    return crate::danmaku::enlarge_aimed(host, player, place, stack);
                }
                let mut state = state(host, player);
                state.tiny = !state.tiny;
                let damage = stack.damage().saturating_add(1);
                let creative = player.game_mode(host) == Some(GameMode::Creative);
                host.transaction(|host, tx| {
                    if !creative && !place.edit(host, tx, |item| item.set_damage(damage)) {
                        return Err(());
                    }
                    PLAYER.set(host, holder, state).map_err(|_| ())
                })
                .ok()?;
                player.set_item_cooldown(host, "arcadiandream:miracle_mallet", 60);
            }
        }
        Some(InteractionResult::SuccessServer)
    }
}

impl ItemPart for Relic {
    fn item_subscribed(&self) -> &'static [Option<HookId>] {
        &[
            Some(HookId::Tooltip),
            Some(HookId::UseItem),
            Some(HookId::UseOn),
        ]
    }

    fn tooltip(&self, _: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        if let Self::Accessory(slot) = self {
            event.lines.push((
                Text::literal(format!("Use to equip; /arcadian unequip {slot} to remove")),
                0xffaaaaaa,
            ));
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
        self.use_hand(host, event.player, event.hand, event.stack)
    }

    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if matches!(self, Self::Gourd)
            && let Some(result) = crate::weapons::use_on(host, event)
        {
            return Some(result);
        }
        self.use_hand(host, event.player, event.hand, event.stack)
    }
}

pub fn custom(stack: &ItemStack) -> Value {
    items::custom(stack)
        .map(|data| items::read_tag(&Tag::Compound(data.clone())))
        .unwrap_or_else(|| json!({}))
}

pub fn edit_custom(
    host: &mut dyn ServerHost,
    place: &ItemPlace,
    edit: impl FnOnce(&mut Value),
) -> Option<()> {
    host.transaction(|host, tx| {
        let mut encoded = false;
        let changed = place.edit(host, tx, |stack| {
            let mut data = custom(stack);
            edit(&mut data);
            encoded = items::set_custom(stack, &data).is_some();
        });
        (changed && encoded).then_some(()).ok_or(())
    })
    .ok()
}

fn give_without_overflow(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    uuid: u128,
    item: &ItemStack,
) -> bool {
    let resource = ItemResource::of(item);
    let mut left = item.count();
    for slot in 0..36 {
        left -= ItemPlace::PlayerSlot { player: uuid, slot }.insert(host, tx, &resource, left);
        if left == 0 {
            return true;
        }
    }
    false
}

pub fn command(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) -> Flow {
    let [root, action, slot] = event.command.literals.as_slice() else {
        return Flow::Continue;
    };
    if root != "arcadian" || action != "unequip" {
        return Flow::Continue;
    }
    let CommandSource::Player { player, .. } = event.source else {
        return Flow::Continue;
    };
    let player = *player;
    let uuid = player.uuid();
    let mut state = state(host, player);
    let Some(tag) = state.accessories.get(slot) else {
        let _ = player.tell(host, Text::literal("That accessory slot is empty."));
        return Flow::Cancel;
    };
    let Some(item) = ItemStack::from_nbt(host, tag) else {
        return Flow::Cancel;
    };
    let removed = host
        .transaction(|host, tx| {
            if !give_without_overflow(host, tx, uuid, &item) {
                return Err(());
            }
            state.accessories.remove(slot);
            PLAYER
                .set(host, Holder::Player(uuid), state)
                .map_err(|_| ())
        })
        .is_ok();
    let message = if removed {
        format!("Unequipped {slot}.")
    } else {
        "Make room in your inventory first.".to_owned()
    };
    let _ = player.tell(host, Text::literal(message));
    Flow::Cancel
}

fn add_effect(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    id: &str,
    duration: i32,
    amplifier: i32,
) {
    if let (Some(living), Some(effect)) = (host.living(player.entity()), host.effect_key(id)) {
        living.add_effect(
            host,
            &ActiveEffect::new(effect, duration, amplifier, false, true, true),
        );
    }
}

pub fn finish_use(host: HostSide<'_>, event: &mut LivingEntityUseItemFinishEvent<'_>) {
    let HostSide::Server(host) = host else { return };
    let Some(player) = event
        .entity
        .and_then(|entity| player_for(host, entity as u32))
        .or_else(|| host.players().acting_player())
    else {
        return;
    };
    match event.stack.item().id(host).as_str() {
        "arcadiandream:healing_charm" => add_effect(host, player, "minecraft:regeneration", 100, 1),
        "arcadiandream:ibuki_gourd" => {
            if custom(event.stack)["filled"] == true {
                add_effect(host, player, "minecraft:strength", 400, 0);
                add_effect(host, player, "minecraft:nausea", 600, 0);
            }
            let mut out = event.stack.clone();
            let mut data = custom(&out);
            data["filled"] = json!(false);
            if items::set_custom(&mut out, &data).is_some() {
                *event.result = Some(out);
            }
        }
        "arcadiandream:hourai_elixir"
            if config::config()["houraiElixirOptions"]["canDrink"] == true =>
        {
            let mut state = state(host, player);
            state.elixir = (state.elixir + 1).min(3);
            let count = state.elixir;
            if PLAYER
                .set(host, Holder::Player(player.uuid()), state)
                .is_err()
            {
                return;
            }
            for id in [
                "minecraft:blindness",
                "minecraft:weakness",
                "minecraft:poison",
            ] {
                add_effect(host, player, id, 100, 0);
            }
            let mut out = event.stack.clone();
            if player.game_mode(host) != Some(GameMode::Creative) {
                out.set_damage(out.damage() + 1);
            }
            *event.result = (out.damage() < 3).then_some(out);
            let _ = player.action_bar(
                host,
                Text::literal(format!("Hourai Elixir: {count}/3 doses")),
            );
        }
        _ => {}
    }
}

fn player_for(host: &dyn ServerHost, entity: u32) -> Option<PlayerRef> {
    host.players()
        .server_players()
        .into_iter()
        .find(|player| player.entity().id() == entity)
}

pub fn tick(host: &mut dyn ServerHost, player: PlayerRef) {
    use AttributeOperation::{AddMultipliedTotal, AddValue};
    let state = state(host, player);
    sync_necklace(host, player, &state);
    let Some(living) = host.living(player.entity()) else {
        return;
    };
    let multiplier = f64::from(state.fatigue_level) + 1.0;
    let modifier = |host: &mut dyn ServerHost, name, id, amount: Option<f64>, op| {
        let id = Identifier::parse(id).unwrap();
        let existing = living
            .attribute_modifiers(host, name)
            .unwrap_or_default()
            .into_iter()
            .find(|old| old.id == id);
        if let Some(amount) = amount {
            if !existing.is_some_and(|old| old.amount == amount) {
                living.replace_attribute_modifier(
                    host,
                    name,
                    AttributeModifier::new(id, amount, op),
                    ModifierLifetime::Transient,
                );
            }
        } else if existing.is_some() {
            living.remove_attribute_modifier(host, name, &id);
        }
    };
    let fatigued = state.fatigue > 0;
    modifier(
        host,
        "minecraft:attack_damage",
        "arcadiandream:elixir_fatigue",
        fatigued.then_some(-0.2 * multiplier),
        AddMultipliedTotal,
    );
    modifier(
        host,
        "minecraft:movement_speed",
        "arcadiandream:elixir_fatigue",
        fatigued.then_some(-0.1 * multiplier),
        AddMultipliedTotal,
    );
    modifier(
        host,
        "minecraft:max_health",
        "arcadiandream:elixir_fatigue",
        fatigued.then_some(-2.0 * multiplier),
        AddValue,
    );
    modifier(
        host,
        "minecraft:scale",
        "arcadiandream:miracle_mallet",
        state.tiny.then_some(-0.75),
        AddMultipliedTotal,
    );
    if fatigued || state.revive != 0 {
        let mut next = state.clone();
        next.fatigue = next.fatigue.saturating_sub(1);
        if state.revive != 0 {
            if let Some(maximum) = living.max_health(host) {
                living.set_health(host, maximum);
            }
            if state.revive == 1 {
                living.remove_all_effects(host);
                add_effect(host, player, "minecraft:resistance", 60, 4);
                crate::danmaku::cancel_near_player(host, player, true);
            } else if next.fatigue > 0 {
                add_effect(
                    host,
                    player,
                    "arcadiandream:elixir_fatigue",
                    next.fatigue as i32,
                    i32::from(next.fatigue_level),
                );
            }
            next.revive = 0;
        }
        let _ = PLAYER.set(host, Holder::Player(player.uuid()), next);
    }
}

pub fn immunity(
    host: &mut dyn ServerHost,
    event: &mut MobEffectApplicableEvent,
) -> Option<EffectApplicable> {
    if event.effect.id(host) == "arcadiandream:elixir_fatigue"
        || event.effect.category(host) != Some(EffectCategory::Harmful)
    {
        return None;
    }
    let player = player_for(host, event.entity.entity().id())?;
    (state(host, player).elixir >= 2).then_some(EffectApplicable::Deny)
}

fn can_revive(state: &State) -> bool {
    if state.accessories.contains_key("life") {
        return true;
    }
    let settings = &config::config()["houraiElixirOptions"];
    let threshold = settings["elixirFatigueThreshold"].as_u64().unwrap_or(1);
    state.elixir >= 3
        && (settings["allowDying"] != true
            || state.fatigue == 0
            || u64::from(state.fatigue_level) + 1 < threshold)
}

pub fn life_ready(host: &mut dyn ServerHost) -> bool {
    let Some(player) = host.players().acting_player() else {
        return false;
    };
    can_revive(&state(host, player))
}

pub fn revive_acting(host: &mut dyn ServerHost) -> bool {
    let Some(player) = host.players().acting_player() else {
        return false;
    };
    revive(host, player)
}

fn revive(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    let mut state = state(host, player);
    if !can_revive(&state) {
        return false;
    }
    if state.accessories.remove("life").is_some() {
        state.revive = 1;
    } else {
        if config::config()["houraiElixirOptions"]["nerfElixir"]
            .as_bool()
            .unwrap_or(true)
        {
            state.fatigue_level = if state.fatigue > 0 {
                (state.fatigue_level + 1).min(4)
            } else {
                0
            };
            state.fatigue = 6000;
        }
        state.revive = 2;
    }
    if PLAYER
        .set(host, Holder::Player(player.uuid()), state)
        .is_err()
    {
        return false;
    }
    tick(host, player);
    true
}

pub fn protect_life(host: &mut dyn ServerHost, event: &LivingDeathEvent<'_>) -> Flow {
    if event.kind != "minecraft:player"
        || matches!(
            event.source.damage_type,
            "minecraft:out_of_world" | "minecraft:generic_kill"
        )
    {
        return Flow::Continue;
    }
    if let Some(player) = player_for(host, event.entity)
        && revive(host, player)
    {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

pub fn worn(host: &mut dyn ServerHost, player: PlayerRef, slot: &str) -> Option<ItemStack> {
    let state = state(host, player);
    ItemStack::from_nbt(host, state.accessories.get(slot)?)
}

pub fn block_ethereal_attack(host: &dyn ServerHost) -> Flow {
    let Some(player) = host.players().acting_player() else {
        return Flow::Continue;
    };
    if host
        .living(player.entity())
        .zip(host.effect_key("arcadiandream:ethereal"))
        .is_some_and(|(living, effect)| living.potion_affected(host, effect))
    {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}
