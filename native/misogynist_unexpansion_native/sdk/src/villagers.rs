//! Villagers picked up and carried, as Easy Villagers does it (by henkelmax): sneak and right-click
//! a villager to pick it up as an item, a stack of one named "Villager" ("Baby Villager" for a baby,
//! its own name if it has one) that tells its profession and level, and use the item on a block to
//! set it down again, just as it was: trades, experience, gossip, its job and home.
//!
//! The item is a villager spawn egg holding the villager's saved data in its custom data. This mod
//! answers every use of it, so the egg's own behaviour (a new villager, a baby from a villager, a
//! spawner turned into a villager spawner) never runs.

use mod_api::{
    BlockShapeKind, Compound, EntityInteractEvent, Host, HostSide, HostTransaction, InteractionResult,
    ItemPart, ItemPlace, ItemResource, ItemStack, ServerHost, Tag, Text, UseItemEvent, UseOnEvent,
};

pub const KIND: &str = "minecraft:villager";
pub const ITEM: &str = "minecraft:villager_spawn_egg";
const DATA: &str = "minecraft:custom_data";
/// Where in the custom data the villager is.
const KEY: &str = "misogynist_unexpansion:villager";

/// What says where the villager was and who it was in the world, not carried with it: it is set
/// down somewhere else, and comes back as a new entity (the game gives it a new id).
const LEFT_BEHIND: [&str; 11] = [
    "Pos",
    "Motion",
    "UUID",
    "OnGround",
    "FallDistance",
    "Fire",
    "PortalCooldown",
    "SleepingX",
    "SleepingY",
    "SleepingZ",
    "leash",
];

fn get<'a>(c: &'a Compound, key: &str) -> Option<&'a Tag> {
    c.iter().find(|(k, _)| k.to_string_lossy() == key).map(|(_, v)| v)
}

fn string(tag: Option<&Tag>) -> Option<String> {
    match tag? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn int(tag: Option<&Tag>) -> Option<i32> {
    match tag? {
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Int(v) => Some(*v),
        _ => None,
    }
}

/// The villager's data as it is carried: all of it but where and who it was in the world.
pub fn carried(nbt: &Compound) -> Compound {
    let mut c = nbt.clone();
    c.retain(|k, _| !LEFT_BEHIND.contains(&k.to_string_lossy().as_str()));
    c
}

pub fn is_baby(nbt: &Compound) -> bool {
    int(get(nbt, "Age")).is_some_and(|age| age < 0)
}

/// A villager can't be picked up asleep in its bed or dead, as in Easy Villagers.
pub fn can_pick_up(nbt: &Compound) -> bool {
    let asleep = get(nbt, "SleepingX").is_some();
    let dead = match get(nbt, "Health") {
        Some(Tag::Float(bits)) => f32::from_bits(*bits) <= 0.0,
        _ => false,
    };
    !asleep && !dead
}

/// `minecraft:master_mason`'s "Master Mason".
fn title(id: &str) -> String {
    let bare = id.rsplit(':').next().unwrap_or(id);
    bare.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const LEVELS: [&str; 5] = ["Novice", "Apprentice", "Journeyman", "Expert", "Master"];

/// "Farmer (Apprentice)": the villager's profession and level, for the item's tooltip. None for a
/// villager without a profession.
pub fn profession_line(nbt: &Compound) -> Option<String> {
    let Some(Tag::Compound(data)) = get(nbt, "VillagerData") else { return None };
    let profession = string(get(data, "profession"))?;
    if profession.ends_with(":none") || profession == "none" {
        return None;
    }
    let level = int(get(data, "level")).unwrap_or(1).clamp(1, 5) as usize;
    Some(format!("{} ({})", title(&profession), LEVELS[level - 1]))
}

/// The item a picked-up villager becomes.
pub fn item_for(host: &dyn Host, nbt: &Compound) -> Option<ItemStack> {
    let mut stack = ItemStack::of(host.item_key(&mod_api::Identifier::parse(ITEM).ok()?)?);
    let mut inner = Compound::new();
    inner.insert(KEY.into(), Tag::Compound(carried(nbt)));
    let name = if is_baby(nbt) { "Baby Villager" } else { "Villager" };
    let mut patch: Compound = [
        (DATA.into(), Tag::Compound(inner)),
        ("minecraft:item_name".into(), Tag::String(name.into())),
        ("minecraft:max_stack_size".into(), Tag::Int(1)),
    ].into();
    if let Some(line) = profession_line(nbt) {
        patch.insert("minecraft:lore".into(), Tag::List(vec![Tag::String(line.as_str().into())]));
    }
    if let Some(custom) = get(nbt, "CustomName") {
        patch.insert("minecraft:custom_name".into(), custom.clone());
    }
    stack.set_components(patch).ok()?;
    Some(stack)
}

/// The villager an item carries, if it is one.
pub fn villager_in(host: &dyn Host, stack: &ItemStack) -> Option<Compound> {
    if stack.item().id(host) != ITEM {
        return None;
    }
    let Tag::Compound(data) = get(stack.components()?.as_nbt(), DATA)? else { return None };
    match get(data, KEY)? {
        Tag::Compound(v) => Some(v.clone()),
        _ => None,
    }
}

/// Where a villager set down on the block at `pos`, clicked on `face`, stands: on the clicked spot
/// when it has nothing to stand against (grass, flowers, snow), else in front of the face.
pub fn set_down_at(pos: [i32; 3], face: [i32; 3], passable: bool) -> [f64; 3] {
    let b = if passable { pos } else { [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]] };
    [f64::from(b[0]) + 0.5, f64::from(b[1]), f64::from(b[2]) + 0.5]
}

/// Sneak, right-click a villager: it is picked up. Any right-click on a mob with a carried villager
/// does nothing (a spawn egg's would make a baby).
pub fn interact_entity(host: &mut dyn ServerHost, event: &EntityInteractEvent<'_>) -> Option<InteractionResult> {
    if event.stack.is_some_and(|stack| villager_in(host, stack).is_some()) {
        return Some(InteractionResult::Consume);
    }
    let player = host.players().acting_player()?;
    if event.kind != KIND || player.is_secondary_use_active(host) != Some(true)
        || player.game_mode(host) == Some(mod_api::GameMode::Spectator)
    {
        return None;
    }
    let entity = host.entity(u32::try_from(event.entity).ok()?)?;
    let nbt = entity.nbt(host)?;
    if !can_pick_up(&nbt) {
        return None;
    }
    let item = item_for(host, &nbt)?;
    let uuid = player.uuid();
    let in_hand = ItemPlace::PlayerHand { player: uuid, hand: event.hand };
    let destination = std::iter::once(in_hand)
        .chain((0..36).map(|slot| ItemPlace::PlayerSlot { player: uuid, slot }))
        .find(|place| place.stack(host).is_none());
    let Some(place) = destination else {
        let _ = player.action_bar(host, Text::literal("No room in your inventory for the villager"));
        return Some(InteractionResult::Consume);
    };
    let _ = host.transaction(|host, tx| {
        if place.insert(host, tx, &ItemResource::of(&item), 1) != 1 || !entity.discard(host) {
            return Err(());
        }
        Ok(())
    });
    Some(InteractionResult::SuccessServer)
}

pub struct CarriedVillager;

impl ItemPart for CarriedVillager {
    fn item_subscribed(&self) -> &'static [Option<mod_api::HookId>] {
        &[Some(mod_api::HookId::UseOn), Some(mod_api::HookId::UseItem)]
    }
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let read: &dyn Host = match &host {
            HostSide::Client(host) => &**host,
            HostSide::Server(host) => &**host,
        };
        let nbt = villager_in(read, event.stack)?;
        let HostSide::Server(host) = host else { return Some(InteractionResult::Consume) };
        let player = host.player(event.player)?;
        let level = player.level(host)?;
        let state = level.block_state(host, event.pos)?;
        let passable = level.block_shape(host, state, event.pos, BlockShapeKind::Collision)
            .is_some_and(|shape| shape.is_empty());
        let at = set_down_at(event.pos, event.face, passable);
        let survival = player.has_infinite_materials(host) != Some(true);
        let place = ItemPlace::PlayerHand { player: event.player, hand: event.hand };
        let resource = ItemResource::of(event.stack);
        if survival && !host.transaction(|host, tx| {
            Ok::<_, ()>(place.extract(host, tx, &resource, 1) == 1)
        }).unwrap_or(false) {
            return Some(InteractionResult::Consume);
        }
        if host.summon(level.dimension_id(), KIND, at, nbt).is_none() {
            if survival {
                let _ = host.transaction(|host, tx| {
                    Ok::<_, ()>(place.insert(host, tx, &resource, 1))
                });
            }
            let _ = player.action_bar(host, Text::literal("There is no room for the villager here"));
            return Some(InteractionResult::Consume);
        }
        Some(InteractionResult::SuccessServer)
    }

    fn use_item(&self, host: HostSide<'_>, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let host: &dyn Host = match host {
            HostSide::Client(host) => host,
            HostSide::Server(host) => host,
        };
        villager_in(host, event.stack).map(|_| InteractionResult::Consume)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    fn test_host() -> FakeHost {
        let mut host = FakeHost::new();
        host.registries.resolve_items([ITEM, "minecraft:stone"].map(|id| mod_api::Identifier::parse(id).unwrap())).unwrap();
        host
    }

    fn villager(age: i32, profession: &str, level: i32) -> Compound {
        let mut data = Compound::new();
        data.insert("profession".into(), Tag::String(profession.into()));
        data.insert("level".into(), Tag::Int(level));
        data.insert("type".into(), Tag::String("minecraft:plains".into()));
        let mut c = Compound::new();
        c.insert("Age".into(), Tag::Int(age));
        c.insert("VillagerData".into(), Tag::Compound(data));
        c.insert("Pos".into(), Tag::List(vec![Tag::Double(1f64.to_bits()); 3]));
        c.insert("UUID".into(), Tag::IntArray(vec![1, 2, 3, 4]));
        c.insert("Xp".into(), Tag::Int(42));
        c.insert("Health".into(), Tag::Float(20f32.to_bits()));
        c
    }

    #[test]
    fn the_item_carries_the_villager_and_comes_back_as_it_was() {
        let v = villager(0, "minecraft:farmer", 2);
        let host = test_host();
        let item = item_for(&host, &v).unwrap();
        assert_eq!(item.item().id(&host), ITEM);
        assert_eq!(item.count(), 1);
        let back = villager_in(&host, &item).expect("a villager");
        assert_eq!(get(&back, "Xp"), Some(&Tag::Int(42)));
        assert!(get(&back, "VillagerData").is_some());
        // Not where or who it was.
        assert!(get(&back, "Pos").is_none() && get(&back, "UUID").is_none());
    }

    #[test]
    fn named_by_age_with_its_profession_and_level() {
        let host = test_host();
        let d = |item: &ItemStack, k: &str| get(item.components()?.as_nbt(), k).cloned();
        let grown = item_for(&host, &villager(0, "minecraft:master_mason", 5)).unwrap();
        assert_eq!(d(&grown, "minecraft:item_name"), Some(Tag::String("Villager".into())));
        assert_eq!(profession_line(&villager(0, "minecraft:farmer", 2)).as_deref(), Some("Farmer (Apprentice)"));
        assert_eq!(profession_line(&villager(0, "minecraft:master_mason", 9)).as_deref(), Some("Master Mason (Master)"));
        assert_eq!(profession_line(&villager(0, "minecraft:none", 1)), None);
        let baby = item_for(&host, &villager(-24000, "minecraft:none", 1)).unwrap();
        assert_eq!(d(&baby, "minecraft:item_name"), Some(Tag::String("Baby Villager".into())));
        assert_eq!(d(&baby, "minecraft:lore"), None);
        assert_eq!(d(&grown, "minecraft:max_stack_size"), Some(Tag::Int(1)));
    }

    #[test]
    fn not_asleep_or_dead() {
        let mut v = villager(0, "minecraft:none", 1);
        assert!(can_pick_up(&v));
        v.insert("SleepingX".into(), Tag::Int(3));
        assert!(!can_pick_up(&v));
        let mut dead = villager(0, "minecraft:none", 1);
        dead.insert("Health".into(), Tag::Float(0f32.to_bits()));
        assert!(!can_pick_up(&dead));
    }

    #[test]
    fn plain_eggs_and_other_items_are_not_villagers() {
        let host = test_host();
        for id in [ITEM, "minecraft:stone"] {
            let stack = ItemStack::of((&host as &dyn Host).item_key(&mod_api::Identifier::parse(id).unwrap()).unwrap());
            assert!(villager_in(&host, &stack).is_none());
        }
    }

    #[test]
    fn set_down_in_front_of_a_solid_face_or_on_grass() {
        assert_eq!(set_down_at([0, 64, 0], [0, 1, 0], false), [0.5, 65.0, 0.5]);
        assert_eq!(set_down_at([3, 64, -2], [1, 0, 0], false), [4.5, 64.0, -1.5]);
        assert_eq!(set_down_at([0, 65, 0], [0, 1, 0], true), [0.5, 65.0, 0.5]);
    }
}
