//! Doll menus borrow the entity's journaled attachment as their backing inventory.
//!
//! A doll is a data-defined mob (`alice_dolls:doll`), and the engine finds an entity's
//! capabilities by its type among the built-in ones only: a doll's own `ENTITY_ITEMS` is never
//! found, and a menu backed by it closed at once. Each open doll menu is backed instead by a
//! stand-in, an invisible marker armour stand at the doll, whose items are the doll's.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mod_api::{Compound, Tag};
use mod_api::{
    EntityRef, Holder, Host, ItemResource, MenuReplace, MenuSlotPlace, Registrar, ResourceHandler,
    ServerHost, Text, Transaction,
};

use crate::{DOLL, DollData, KIND, bauble_block, doll_of, goals, same_kind, stack_limit, switch_row};
use crate::stacks::DollStack;

const BAUBLES: usize = 512;
const FILLER: usize = BAUBLES + goals::BAUBLE_SLOTS;
const SWITCH: usize = FILLER + 2;

/// The stand-ins: stand -> doll, and player -> (dimension, stand) for the menu they have open.
static STAND_FOR: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(Default::default);
static STAND_OF: LazyLock<Mutex<HashMap<u128, (String, u32)>>> = LazyLock::new(Default::default);
const STAND_TAG: &str = "alice_dolls_menu";

fn stand_nbt() -> Compound {
    [
        ("Invisible".into(), Tag::Byte(1)),
        ("Marker".into(), Tag::Byte(1)),
        ("NoGravity".into(), Tag::Byte(1)),
        ("Invulnerable".into(), Tag::Byte(1)),
        ("Silent".into(), Tag::Byte(1)),
        ("Tags".into(), Tag::List(vec![Tag::String(STAND_TAG.into())])),
    ]
    .into()
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    registrar.entity_capability(&mod_api::ENTITY_ITEMS, KIND, |host, entity, ()| {
        DOLL.has(host, Holder::Entity(entity.id())).ok().filter(|exists| *exists)?;
        Some(Box::new(DollInventory(entity)))
    });
    // Ahead of the game's own, and only for a stand standing in for a doll: any other stand is
    // the game's.
    registrar.entity_capability_before_game(&mod_api::ENTITY_ITEMS, "minecraft:armor_stand", |host, stand, ()| {
        let doll = STAND_FOR.lock().ok()?.get(&stand.id()).copied()?;
        DOLL.has(host, Holder::Entity(doll)).ok().filter(|exists| *exists)?;
        Some(Box::new(DollInventory(host.entity(doll)?)))
    });
}

/// The stand-in for `player`'s doll menu, gone: the menu closed or another replaces it.
pub(crate) fn release(host: &mut dyn ServerHost, player: u128) {
    let Some((_, stand)) = STAND_OF.lock().ok().and_then(|mut s| s.remove(&player)) else { return };
    STAND_FOR.lock().ok().map(|mut s| s.remove(&stand));
    if let Some(entity) = host.entity(stand) {
        entity.remove(host, mod_api::RemovalReason::Discarded);
    }
}

/// A new stand-in for doll `doll`, at it, for `player`'s menu.
fn stand_in(host: &mut dyn ServerHost, player: u128, doll: EntityRef) -> Option<EntityRef> {
    release(host, player);
    let dimension = doll.level(host)?.dimension_id().to_owned();
    let at = doll.position(host)?;
    let stand = host.summon(&dimension, "minecraft:armor_stand", at, stand_nbt())?;
    STAND_FOR.lock().ok()?.insert(stand, doll.id());
    STAND_OF.lock().ok()?.insert(player, (dimension, stand));
    host.entity(stand)
}

pub(crate) fn open(
    host: &mut dyn ServerHost,
    uuid: u128,
    id: u32,
    title: Text,
    layout: goals::MenuLayout,
    has_switch: bool,
    replace: MenuReplace,
) -> Option<i32> {
    let player = host.player(uuid)?;
    let doll = host.entity(id)?;
    let rows = (layout.slots + goals::BAUBLE_CELLS).div_ceil(9);
    let total_rows = rows + usize::from(has_switch);
    let name = format!("alice_dolls_native:inventory_{total_rows}");
    let kind = host.menu_keys().into_iter().find(|kind| kind.id(host) == name)?;
    let window = player.open_mod_menu(host, kind, title, Vec::new(), None, replace).ok()??;
    let Some(stand) = stand_in(host, uuid, doll) else {
        crate::debug(host, &format!("inventory {id}: no stand-in for its menu"));
        host.close_menu(uuid);
        release(host, uuid);
        return None;
    };
    if !bind(host, uuid, stand, window, layout, has_switch) {
        crate::debug(host, &format!("inventory {id}: its menu's slots would not bind"));
        host.close_menu(uuid);
        release(host, uuid);
        return None;
    }
    Some(window)
}

pub(crate) fn refresh(host: &mut dyn ServerHost, uuid: u128, id: u32, window: i32, layout: goals::MenuLayout) {
    let stand = STAND_OF.lock().ok().and_then(|s| s.get(&uuid).map(|(_, stand)| *stand));
    let backs_it = stand.is_some_and(|s| STAND_FOR.lock().is_ok_and(|f| f.get(&s) == Some(&id)));
    if let Some(entity) = stand.filter(|_| backs_it).and_then(|s| host.entity(s)) {
        let _ = bind(host, uuid, entity, window, layout, true);
    }
}

fn bind(
    host: &mut dyn ServerHost,
    uuid: u128,
    entity: EntityRef,
    window: i32,
    layout: goals::MenuLayout,
    has_switch: bool,
) -> bool {
    let Some(player) = host.player(uuid) else { return false };
    let slots = (0..layout.slots).map(|slot| {
        let slot = layout.offset + slot;
        if slot < layout.total { slot } else { FILLER + 1 }
    });
    let baubles = (0..goals::BAUBLE_CELLS)
        .map(|cell| goals::bauble_cell(cell, 0).flatten().map_or(FILLER, |slot| BAUBLES + slot));
    let switch = (0..usize::from(has_switch) * 9).map(|column| SWITCH + column);
    let places = slots.chain(baubles).chain(switch)
        .map(|slot| MenuSlotPlace::EntityItems { entity, slot })
        .collect();
    player.back_mod_menu_slots(host, window, places).is_ok()
}

struct DollInventory(EntityRef);

impl DollInventory {
    fn holder(&self) -> Holder {
        Holder::Entity(self.0.id())
    }

    fn item(data: &DollData, index: usize) -> Option<&DollStack> {
        if index >= BAUBLES {
            data.baubles.get(index - BAUBLES)?.as_ref()
        } else {
            data.items.get(index)?.as_ref()
        }
    }

    fn slot(data: &mut DollData, index: usize) -> Option<&mut Option<DollStack>> {
        if index >= BAUBLES {
            data.baubles.get_mut(index - BAUBLES)
        } else {
            data.items.get_mut(index)
        }
    }

    fn capacity_for(&self, host: &dyn Host, data: &DollData, index: usize, resource: &ItemResource) -> u32 {
        let item = resource.item().id(host);
        if index >= BAUBLES {
            return u32::from(index < FILLER && goals::is_bauble(&item));
        }
        let total = data.items.len();
        if index >= total {
            return 0;
        }
        if total > goals::ARMOUR_SLOTS {
            if let Some(part) = (0..goals::ARMOUR_SLOTS).find(|&part| index == goals::armour_slot(total, part)) {
                return u32::from(goals::armour_part(&item) == Some(part));
            }
            if index == goals::shield_slot(total) {
                return u32::from(item == "minecraft:shield");
            }
        }
        let limit = doll_of(host, self.0.id()).and_then(|doll| stack_limit(doll.name, data));
        limit.or_else(|| host.max_stack_size(resource.item())).unwrap_or(64)
    }

    fn shown_item(data: &DollData, index: usize) -> Option<DollStack> {
        match index {
            FILLER => bauble_block(&[]).into_iter().next().flatten(),
            SWITCH.. => switch_row(data.keep_rock).get(index - SWITCH)?.clone(),
            _ => Self::item(data, index).cloned(),
        }
    }
}

impl ResourceHandler<ItemResource> for DollInventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        SWITCH + 9
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        let data = DOLL.get(host, self.holder()).ok()?;
        let mut item = Self::shown_item(&data, index)?;
        item.count = 1;
        Some(ItemResource::of(&item.to_game(host)?))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        DOLL.get(host, self.holder()).ok()
            .and_then(|data| Self::shown_item(&data, index))
            .map_or(0, |item| item.count)
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> u32 {
        let Ok(data) = DOLL.get(host, self.holder()) else { return 0 };
        self.capacity_for(host, &data, index, resource)
    }

    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        if host.transaction_depth(tx).is_none() { return 0; }
        let Ok(mut data) = DOLL.get(host, self.holder()) else { return 0 };
        let limit = self.capacity_for(host, &data, index, resource);
        let Some(mut candidate) = DollStack::from_game(host, resource.as_stack()) else { return 0 };
        let Some(slot) = Self::slot(&mut data, index) else { return 0 };
        if slot.as_ref().is_some_and(|stack| !same_kind(stack, &candidate)) {
            return 0;
        }
        let count = slot.as_ref().map_or(0, |stack| stack.count);
        let inserted = amount.min(limit.saturating_sub(count));
        if inserted == 0 { return 0; }
        candidate.count = count + inserted;
        *slot = Some(candidate);
        if DOLL.set(host, self.holder(), data).is_ok() { inserted } else { 0 }
    }

    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        if host.transaction_depth(tx).is_none() { return 0; }
        let Ok(mut data) = DOLL.get(host, self.holder()) else { return 0 };
        let Some(candidate) = DollStack::from_game(host, resource.as_stack()) else { return 0 };
        let Some(slot) = Self::slot(&mut data, index) else { return 0 };
        let Some(stack) = slot.as_mut().filter(|stack| same_kind(stack, &candidate)) else { return 0 };
        let extracted = amount.min(stack.count);
        if extracted == 0 { return 0; }
        stack.count -= extracted;
        if stack.count == 0 {
            *slot = None;
        }
        if DOLL.set(host, self.holder(), data).is_ok() { extracted } else { 0 }
    }
}

pub fn inventory_screen(menu: &str) -> Option<mod_api::Screen> {
    let rows = menu.strip_prefix("alice_dolls_native:inventory_")?.parse::<u8>().ok()?;
    if !(3..=6).contains(&rows) {
        return None;
    }
    let slots = (0..rows * 9)
        .map(|slot| Some([8.0 + 18.0 * f32::from(slot % 9), 18.0 + 18.0 * f32::from(slot / 9)]))
        .collect();
    Some(mod_api::Screen {
        width: 176.0,
        height: 114.0 + f32::from(rows) * 18.0,
        slots,
        inventory_y: 31.0 + f32::from(rows) * 18.0,
        inventory_x: 8.0,
        ..mod_api::Screen::default()
    })
}
