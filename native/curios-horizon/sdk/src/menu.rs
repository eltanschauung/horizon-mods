use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mod_api::*;

use crate::api::{ACCESSORIES, Slot, accepts};
use crate::server_mask;

pub const PREFIX: &str = "curios_horizon:accessories_";
pub const STAND_TAG: &str = "curios_horizon_menu";

/// The SDK binds menu slots through an entity's item provider. This marker
/// lends the equipment handler while the menu is open. It stores no items.
#[derive(Clone, Copy)]
struct Session {
    marker: EntityRef,
    window: i32,
    mask: u16,
}

static MENUS: LazyLock<Mutex<HashMap<u128, Session>>> = LazyLock::new(Default::default);

pub fn slots(mask: u16) -> impl Iterator<Item = Slot> {
    Slot::ALL.into_iter().filter(move |s| s.enabled(mask))
}

pub fn session_mask(uuid: u128) -> Option<u16> {
    MENUS.lock().ok()?.get(&uuid).map(|session| session.mask)
}

pub fn register(reg: &mut dyn Registrar) {
    reg.entity_capability_before_game(&ENTITY_ITEMS, "minecraft:armor_stand", |_, marker, ()| {
        MENUS.lock().ok()?.iter().find_map(|(&owner, session)| {
            (session.marker == marker).then(|| {
                let inner = AttachedItems::new(
                    &ACCESSORIES,
                    Holder::Player(owner),
                    Slot::ALL.len(),
                    1,
                    |_| true,
                )
                .expect("nine attachment slots");
                Box::new(Inventory {
                    inner,
                    mask: session.mask,
                }) as Box<dyn ResourceHandler<ItemResource>>
            })
        })
    });
}

pub fn open(host: &mut dyn ServerHost, player: PlayerRef) {
    let uuid = player.uuid();
    if matches!(player.game_mode(host), Some(GameMode::Spectator) | None) {
        return;
    }
    if let Some(session) = MENUS.lock().ok().and_then(|m| m.get(&uuid).copied())
        && host.menu(uuid, session.window).is_some()
    {
        return;
    }
    close(host, uuid, None);
    let mask = server_mask();
    if mask == 0 {
        let _ = player.tell(host, Text::literal("All accessory slots are disabled."));
        return;
    }
    let name = format!("{PREFIX}{}", mask.count_ones());
    let Some(menu) = host
        .menu_keys()
        .into_iter()
        .find(|key| key.id(host) == name)
    else {
        let _ = player.tell(host, Text::literal("Accessory menu type is unavailable."));
        return;
    };
    let (Some(level), Some(at)) = (player.level(host), player.entity().position(host)) else {
        return;
    };
    let nbt: Compound = [
        ("Invisible".into(), Tag::Byte(1)),
        ("Marker".into(), Tag::Byte(1)),
        ("NoGravity".into(), Tag::Byte(1)),
        ("Invulnerable".into(), Tag::Byte(1)),
        ("Silent".into(), Tag::Byte(1)),
        (
            "Tags".into(),
            Tag::List(vec![Tag::String(STAND_TAG.into())]),
        ),
    ]
    .into();
    let Some(id) = host.summon(level.dimension_id(), "minecraft:armor_stand", at, nbt) else {
        return;
    };
    let Some(marker) = host.entity(id) else {
        return;
    };
    let Ok(Some(window)) = player.open_mod_menu(
        host,
        menu,
        Text::literal("Accessories"),
        mask.to_le_bytes().to_vec(),
        None,
        MenuReplace::CloseFirst,
    ) else {
        marker.remove(host, RemovalReason::Discarded);
        let _ = player.tell(host, Text::literal("Could not open the accessory menu."));
        return;
    };
    MENUS.lock().expect("accessory menus").insert(
        uuid,
        Session {
            marker,
            window,
            mask,
        },
    );
    let places = slots(mask)
        .enumerate()
        .map(|(slot, _)| MenuSlotPlace::EntityItems {
            entity: marker,
            slot,
        })
        .collect();
    if player.back_mod_menu_slots(host, window, places).is_err() {
        host.close_menu(uuid);
        close(host, uuid, Some(window));
        let _ = player.tell(host, Text::literal("Could not bind the accessory slots."));
    }
}

pub fn close(host: &mut dyn ServerHost, uuid: u128, window: Option<i32>) {
    let marker = {
        let mut menus = MENUS.lock().expect("accessory menus");
        if menus
            .get(&uuid)
            .is_none_or(|s| window.is_some_and(|w| s.window != w))
        {
            return;
        }
        menus.remove(&uuid).map(|s| s.marker)
    };
    if let Some(marker) = marker {
        marker.remove(host, RemovalReason::Discarded);
    }
}

pub fn close_all(host: &mut dyn ServerHost) {
    let players = MENUS
        .lock()
        .expect("accessory menus")
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for uuid in players {
        host.close_menu(uuid);
        close(host, uuid, None);
    }
}

struct Inventory {
    inner: AttachedItems,
    mask: u16,
}

impl Inventory {
    fn index(&self, index: usize) -> Option<Slot> {
        let slot = slots(self.mask).nth(index)?;
        slot.enabled(server_mask()).then_some(slot)
    }
}

impl ResourceHandler<ItemResource> for Inventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.mask.count_ones() as usize
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        self.inner.resource(host, self.index(index)? as usize)
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.index(index)
            .map_or(0, |slot| self.inner.amount(host, slot as usize))
    }
    fn capacity(&self, _: &mut dyn ServerHost, index: usize, _: &ItemResource) -> u32 {
        u32::from(self.index(index).is_some())
    }
    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> bool {
        self.index(index)
            .is_some_and(|slot| accepts(host, slot, resource.as_stack()))
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        item: &ItemResource,
        count: u32,
    ) -> u32 {
        let Some(slot) = self
            .index(index)
            .filter(|&slot| accepts(host, slot, item.as_stack()))
        else {
            return 0;
        };
        self.inner.insert(host, tx, slot as usize, item, count)
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        item: &ItemResource,
        count: u32,
    ) -> u32 {
        let Some(slot) = self.index(index) else {
            return 0;
        };
        self.inner.extract(host, tx, slot as usize, item, count)
    }
}
