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
    EntityRef, Holder, Host, HostTransaction, ItemPlace, ItemResource, MenuReplace, MenuSlotPlace, Registrar,
    ResourceHandler, ServerHost, Text, Transaction,
};

use crate::players::max_stack_size;

use crate::{DOLL, DollData, KIND, bauble_block, doll_of, goals, same_kind, stack_limit, switch_row};
use crate::stacks::DollStack;

const BAUBLES: usize = 512;
const FILLER: usize = BAUBLES + goals::BAUBLE_SLOTS;
const SWITCH: usize = FILLER + 2;
/// Moscow's ender chest tab, in the first cell of her bauble grid.
const ENDER_TAB: usize = SWITCH + 9;
/// The first bauble grid cell: where Moscow's ender chest tab sits.
pub(crate) const ENDER_CELL: usize = 0;
const ENDER_SLOTS: usize = 27;
/// The title of the ender chest screen Moscow opens.
pub(crate) const ENDER_TITLE: &str = "Ender Chest";

/// The stand-ins: stand -> doll, and player -> (dimension, stand) for the menu they have open.
static STAND_FOR: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(Default::default);
static STAND_OF: LazyLock<Mutex<HashMap<u128, (String, u32, i32)>>> = LazyLock::new(Default::default);
/// Stand-ins holding a player's ender chest (stand -> player): its 27 slots are staged in the
/// stand's own doll data, journaled as a doll's are, and written back to the ender chest each
/// tick and when the screen closes.
static STAND_ENDER: LazyLock<Mutex<HashMap<u32, u128>>> = LazyLock::new(Default::default);
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
        Some(Box::new(DollInventory(entity, false)))
    });
    // Ahead of the game's own, and only for a stand standing in for a doll: any other stand is
    // the game's.
    registrar.entity_capability_before_game(&mod_api::ENTITY_ITEMS, "minecraft:armor_stand", |host, stand, ()| {
        let doll = STAND_FOR.lock().ok()?.get(&stand.id()).copied()?;
        DOLL.has(host, Holder::Entity(doll)).ok().filter(|exists| *exists)?;
        if STAND_ENDER.lock().ok()?.contains_key(&stand.id()) {
            DOLL.has(host, Holder::Entity(stand.id())).ok().filter(|exists| *exists)?;
            return Some(Box::new(DollInventory(stand, true)));
        }
        Some(Box::new(DollInventory(host.entity(doll)?, false)))
    });
}

/// The ender chest staged on stand-in `stand` written back to `player`'s, slot by changed slot.
fn flush_one(host: &mut dyn ServerHost, stand: u32, player: u128) {
    let Ok(data) = DOLL.get(host, Holder::Entity(stand)) else { return };
    let Some(who) = host.player(player) else { return };
    let current: HashMap<u32, mod_api::ItemStack> = who.ender_chest(host).unwrap_or_default().into_iter().collect();
    for (i, want) in data.items.iter().enumerate().take(ENDER_SLOTS) {
        let slot = i as u32;
        let have = current.get(&slot).and_then(|s| DollStack::from_game(host, s));
        if have != *want {
            let stack = want.as_ref().and_then(|w| w.to_game(host));
            let _ = who.set_ender_chest_slot(host, slot, stack);
        }
    }
}

/// In entity `id`'s tick: an ender chest staged on it (or on the stand-in of the doll it is)
/// written back.
pub(crate) fn tick_ender(host: &mut dyn ServerHost, id: u32) {
    let open: Vec<(u32, u128)> = match STAND_ENDER.lock() {
        Ok(e) if !e.is_empty() => e.iter().map(|(s, p)| (*s, *p)).collect(),
        _ => return,
    };
    for (stand, player) in open {
        let of_doll = STAND_FOR.lock().is_ok_and(|f| f.get(&stand) == Some(&id));
        if stand == id || of_doll {
            flush_one(host, stand, player);
        }
    }
}

/// Moscow's ender chest tab: `player`'s own ender chest on a screen of its own, by doll `id`.
pub(crate) fn open_ender(host: &mut dyn ServerHost, uuid: u128, id: u32) -> Option<i32> {
    let player = host.player(uuid)?;
    let doll = host.entity(id)?;
    let kind = host.menu_keys().into_iter().find(|kind| kind.id(host) == "alice_dolls_native:inventory_3")?;
    let window = player.open_mod_menu(host, kind, Text::literal(ENDER_TITLE), Vec::new(), None, MenuReplace::CloseFirst).ok()??;
    let Some(stand) = stand_in(host, uuid, doll, window) else {
        host.close_menu(uuid);
        return None;
    };
    let mut items: Vec<Option<DollStack>> = vec![None; ENDER_SLOTS];
    for (slot, stack) in player.ender_chest(host).unwrap_or_default() {
        if let Some(cell) = items.get_mut(slot as usize) {
            *cell = DollStack::from_game(host, &stack);
        }
    }
    let staged = DOLL.set(host, Holder::Entity(stand.id()), DollData { items, ..DollData::default() }).is_ok();
    STAND_ENDER.lock().ok()?.insert(stand.id(), uuid);
    let places = (0..ENDER_SLOTS).map(|slot| MenuSlotPlace::EntityItems { entity: stand, slot }).collect();
    if !staged || player.back_mod_menu_slots(host, window, places).is_err() {
        // Nothing was taken from the ender chest: the stand-in goes without writing back.
        STAND_ENDER.lock().ok().map(|mut e| e.remove(&stand.id()));
        host.close_menu(uuid);
        release(host, uuid, None);
        return None;
    }
    Some(window)
}

/// The stand-in for `player`'s doll menu, gone: the menu `window` closed (None: whichever it
/// backs, as another menu replaces it). An ender chest staged on it is written back first.
/// Whether it held an ender chest.
pub(crate) fn release(host: &mut dyn ServerHost, player: u128, window: Option<i32>) -> bool {
    let Some((_, stand, backs)) = STAND_OF.lock().ok().and_then(|s| s.get(&player).cloned()) else { return false };
    if window.is_some_and(|w| w != backs) {
        return false;
    }
    STAND_OF.lock().ok().map(|mut s| s.remove(&player));
    let ender = STAND_ENDER.lock().ok().and_then(|mut e| e.remove(&stand)).is_some();
    if ender {
        flush_one(host, stand, player);
    }
    STAND_FOR.lock().ok().map(|mut s| s.remove(&stand));
    if let Some(entity) = host.entity(stand) {
        entity.remove(host, mod_api::RemovalReason::Discarded);
    }
    ender
}

/// A new stand-in for doll `doll`, at it, for `player`'s menu.
fn stand_in(host: &mut dyn ServerHost, player: u128, doll: EntityRef, window: i32) -> Option<EntityRef> {
    release(host, player, None);
    let dimension = doll.level(host)?.dimension_id().to_owned();
    let at = doll.position(host)?;
    let stand = host.summon(&dimension, "minecraft:armor_stand", at, stand_nbt())?;
    STAND_FOR.lock().ok()?.insert(stand, doll.id());
    STAND_OF.lock().ok()?.insert(player, (dimension, stand, window));
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
    let Some(stand) = stand_in(host, uuid, doll, window) else {
        crate::debug(host, &format!("inventory {id}: no stand-in for its menu"));
        host.close_menu(uuid);
        release(host, uuid, None);
        return None;
    };
    let tab = layout.slots > 0 && is_moscow(host, id);
    if !bind(host, uuid, stand, window, layout, has_switch, tab) {
        crate::debug(host, &format!("inventory {id}: its menu's slots would not bind"));
        host.close_menu(uuid);
        release(host, uuid, None);
        return None;
    }
    Some(window)
}

pub(crate) fn refresh(host: &mut dyn ServerHost, uuid: u128, id: u32, window: i32, layout: goals::MenuLayout, has_switch: bool) -> bool {
    let stand = STAND_OF.lock().ok().and_then(|s| s.get(&uuid).map(|(_, stand, _)| *stand));
    let backs_it = stand.is_some_and(|s| STAND_FOR.lock().is_ok_and(|f| f.get(&s) == Some(&id)));
    let tab = layout.slots > 0 && is_moscow(host, id);
    match stand.filter(|_| backs_it).and_then(|s| host.entity(s)) {
        Some(entity) => bind(host, uuid, entity, window, layout, has_switch, tab),
        None => false,
    }
}

fn bind(
    host: &mut dyn ServerHost,
    uuid: u128,
    entity: EntityRef,
    window: i32,
    layout: goals::MenuLayout,
    has_switch: bool,
    ender_tab: bool,
) -> bool {
    let Some(player) = host.player(uuid) else { return false };
    let slots = (0..layout.slots).map(|slot| {
        let slot = layout.offset + slot;
        if slot < layout.total { slot } else { FILLER + 1 }
    });
    let baubles = (0..goals::BAUBLE_CELLS)
        .map(|cell| match goals::bauble_cell(cell, 0).flatten() {
            Some(slot) => BAUBLES + slot,
            None if ender_tab && cell == ENDER_CELL => ENDER_TAB,
            None => FILLER,
        });
    let switch = (0..usize::from(has_switch) * 9).map(|column| SWITCH + column);
    let places = slots.chain(baubles).chain(switch)
        .map(|slot| MenuSlotPlace::EntityItems { entity, slot })
        .collect();
    player.back_mod_menu_slots(host, window, places).is_ok()
}

fn is_moscow(host: &dyn Host, id: u32) -> bool {
    doll_of(host, id).is_some_and(|doll| doll.name == goals::MOSCOW)
}

/// A doll's inventory, or (`.1`) an ender chest staged on a stand-in: plain slots, no armour
/// or shield places and no doll's stack limits.
struct DollInventory(EntityRef, bool);

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
        if self.1 {
            if index >= data.items.len().min(ENDER_SLOTS) {
                return 0;
            }
            return host.max_stack_size(resource.item()).unwrap_or(64);
        }
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
            ENDER_TAB => Some(DollStack::new("minecraft:ender_chest", 1)),
            SWITCH.. => switch_row(data.keep_rock).get(index - SWITCH)?.clone(),
            _ => Self::item(data, index).cloned(),
        }
    }
}

impl ResourceHandler<ItemResource> for DollInventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        ENDER_TAB + 1
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        let data = DOLL.get(host, self.holder()).ok()?;
        let mut item = Self::shown_item(&data, index)?;
        item.count = 1;
        Some(ItemResource::of(&item.to_game(host)?))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        let Some(item) = DOLL.get(host, self.holder()).ok().and_then(|data| Self::shown_item(&data, index)) else { return 0 };
        // Past an item's stack (Moscow's slots), the game's slot shows and hands out a stack's
        // worth and the rest stays; the client draws the real count, and `big_click` goes past it.
        item.count.min(max_stack_size(host, &item.item).unwrap_or(64).max(1))
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

/// Whether doll slot `d` of `total` is a plain storage slot: not past her slots, not an armour
/// or shield place.
pub(crate) fn plain_slot(total: usize, d: usize) -> bool {
    if d >= total {
        return false;
    }
    total <= goals::ARMOUR_SLOTS
        || ((0..goals::ARMOUR_SLOTS).all(|part| d != goals::armour_slot(total, part)) && d != goals::shield_slot(total))
}

/// One of `stack`, as the game's resource.
fn resource_of(host: &dyn Host, stack: &DollStack) -> Option<ItemResource> {
    let mut one = stack.clone();
    one.count = 1;
    Some(ItemResource::of(&one.to_game(host)?))
}

/// A click on Moscow's screen done here, past the game's slots (as Sophisticated Backpacks' stack
/// upgrades do): a menu slot stops at an item's stack, and hers hold more. `slot` is the screen's
/// slot: one of hers shown from `offset` (`view` of them), taken from or put into with the left
/// or (`right`) the right button; or with `shift`, one of the player's inventory, moved into her
/// onto stacks like it first and then into empty slots. Unstackable items stay one to a slot.
/// What the click changed; nothing is lost when it can't be done.
pub(crate) fn big_click(
    host: &mut dyn ServerHost,
    uuid: u128,
    doll: u32,
    offset: usize,
    view: usize,
    slot: usize,
    right: bool,
    shift: bool,
) -> bool {
    let Some(player) = host.player(uuid) else { return false };
    let Some(menu) = player.container_menu(host) else { return false };
    let holder = Holder::Entity(doll);
    let Ok(mut data) = DOLL.get(host, holder) else { return false };
    let Some(limit) = doll_of(host, doll).and_then(|d| stack_limit(d.name, &data)) else { return false };
    let limit_of = |host: &dyn Host, item: &str| match max_stack_size(host, item).unwrap_or(64) {
        1 => 1,
        _ => limit,
    };
    let total = data.items.len();
    let cursor = ItemPlace::PlayerCursor { menu };
    if shift {
        let from = ItemPlace::MenuSlot { menu, slot };
        let Some(res) = from.resource(host) else { return false };
        let have = from.amount(host);
        let Some(moving) = DollStack::from_game(host, res.as_stack()) else { return false };
        let limit = limit_of(host, &moving.item);
        let mut left = have;
        let mut plan: Vec<(usize, u32)> = Vec::new();
        for merge in [true, false] {
            for d in (0..total).filter(|&d| plain_slot(total, d)) {
                let count = match &data.items[d] {
                    Some(here) if merge && same_kind(here, &moving) => here.count,
                    None if !merge => 0,
                    _ => continue,
                };
                let put = left.min(limit.saturating_sub(count));
                if put > 0 {
                    plan.push((d, put));
                    left -= put;
                }
            }
        }
        let moved = have - left;
        if moved == 0 || host.transaction(|host, tx| if from.extract(host, tx, &res, moved) == moved { Ok(()) } else { Err(()) }).is_err() {
            return false;
        }
        for (d, put) in plan {
            match &mut data.items[d] {
                Some(here) => here.count += put,
                empty => *empty = Some(DollStack { count: put, ..moving.clone() }),
            }
        }
        return DOLL.set(host, holder, data).is_ok();
    }
    let d = offset + slot;
    if slot >= view || !plain_slot(total, d) {
        return false;
    }
    let here = data.items[d].clone();
    match (cursor.resource(host), here) {
        (None, None) => return false,
        // An empty cursor takes a stack's worth (half of what is here with the right button, no
        // more than a stack).
        (None, Some(stack)) => {
            let max = max_stack_size(host, &stack.item).unwrap_or(64).max(1);
            let take = if right { stack.count.div_ceil(2) } else { stack.count }.min(max);
            let Some(res) = resource_of(host, &stack) else { return false };
            if host.transaction(|host, tx| if cursor.insert(host, tx, &res, take) == take { Ok(()) } else { Err(()) }).is_err() {
                return false;
            }
            data.items[d] = (stack.count > take).then(|| DollStack { count: stack.count - take, ..stack });
        }
        (Some(res), here) => {
            let carried = cursor.amount(host);
            let Some(held) = DollStack::from_game(host, res.as_stack()) else { return false };
            match here {
                // Another item: swapped, when what is here fits on the cursor.
                Some(stack) if !same_kind(&stack, &held) => {
                    let max = max_stack_size(host, &stack.item).unwrap_or(64);
                    if stack.count > max || carried > limit_of(host, &held.item) {
                        return false;
                    }
                    let Some(back) = resource_of(host, &stack) else { return false };
                    let swapped = host.transaction(|host, tx| {
                        let ok = cursor.extract(host, tx, &res, carried) == carried
                            && cursor.insert(host, tx, &back, stack.count) == stack.count;
                        if ok { Ok(()) } else { Err(()) }
                    });
                    if swapped.is_err() {
                        return false;
                    }
                    data.items[d] = Some(DollStack { count: carried, ..held });
                }
                // The same item or an empty slot: all of the cursor (one with the right button), up
                // to her stack.
                here => {
                    let count = here.as_ref().map_or(0, |s| s.count);
                    let put = (if right { 1 } else { carried }).min(limit_of(host, &held.item).saturating_sub(count));
                    if put == 0 || host.transaction(|host, tx| if cursor.extract(host, tx, &res, put) == put { Ok(()) } else { Err(()) }).is_err() {
                        return false;
                    }
                    data.items[d] = Some(DollStack { count: count + put, ..here.unwrap_or(held) });
                }
            }
        }
    }
    DOLL.set(host, holder, data).is_ok()
}
