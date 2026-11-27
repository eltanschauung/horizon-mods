//! Public read interface for accessory effects in other native mods.
//! Item eligibility is declared through the standard `curios:<slot>` item tags.

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, ITEM_STACKS,
    ItemResource, ItemStack, PlayerRef, StackList, Tag,
};

/// Stable storage order. Append new slot types here without renumbering existing entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Slot {
    Curio,
    Head,
    Necklace,
    Back,
    Bracelet,
    Ring,
    Belt,
    Feet,
    Charm,
}

impl Slot {
    pub const ALL: [Self; 9] = [
        Self::Curio,
        Self::Head,
        Self::Necklace,
        Self::Back,
        Self::Bracelet,
        Self::Ring,
        Self::Belt,
        Self::Feet,
        Self::Charm,
    ];

    pub fn name(self) -> &'static str {
        [
            "curio", "head", "necklace", "back", "bracelet", "ring", "belt", "feet", "charm",
        ][self as usize]
    }

    pub fn tag(self) -> &'static str {
        [
            "curios:curio",
            "curios:head",
            "curios:necklace",
            "curios:back",
            "curios:bracelet",
            "curios:ring",
            "curios:belt",
            "curios:feet",
            "curios:charm",
        ][self as usize]
    }

    pub fn enabled(self, mask: u16) -> bool {
        mask & (1 << self as u8) != 0
    }
}

pub(super) const ALL_SLOTS: u16 = (1 << Slot::ALL.len()) - 1;

fn encode_mask(mask: &u16) -> Result<Compound, AttachmentError> {
    Ok([("enabled".into(), Tag::Int(i32::from(*mask)))].into())
}

fn decode_mask(data: &Compound) -> Result<u16, AttachmentError> {
    match data.get(&"enabled".into()) {
        Some(Tag::Int(mask)) if (0..=i32::from(ALL_SLOTS)).contains(mask) => Ok(*mask as u16),
        _ => Err(AttachmentError::Decode),
    }
}

pub(super) static ENABLED: AttachmentType<u16> = AttachmentType {
    namespace: "curios_horizon",
    name: "enabled",
    default: || ALL_SLOTS,
    codec: Some(AttachmentCodec {
        encode: encode_mask,
        decode: decode_mask,
    }),
    copy_on_death: true,
    sync: true,
};

pub(super) static ACCESSORIES: AttachmentType<StackList<ItemResource>> = AttachmentType {
    namespace: "curios_horizon",
    name: "accessories",
    default: Vec::new,
    codec: Some(ITEM_STACKS),
    copy_on_death: true,
    sync: true,
};

/// The effective server slot mask, or None when no Curios state has arrived.
pub fn enabled_slots(host: &mut dyn Host, player: PlayerRef) -> Option<u16> {
    ENABLED
        .get_existing(host, Holder::Player(player.uuid()))
        .ok()
        .flatten()
}

/// An equipped stack for this player. Disabled slots never contribute effects.
pub fn equipped(host: &mut dyn Host, player: PlayerRef, slot: Slot) -> Option<ItemStack> {
    if !slot.enabled(enabled_slots(host, player)?) {
        return None;
    }
    let holder = Holder::Player(player.uuid());
    if let Ok(Some(stack)) = ACCESSORIES.with_decoded(host, holder, |items| {
        items.map(|items| {
            let (item, count) = items.get(slot as usize)?.as_ref()?;
            item.to_stack(*count)
        })
    }) {
        return stack;
    }
    let items = ACCESSORIES.get_existing(host, holder).ok()??;
    let (item, count) = items.get(slot as usize)?.as_ref()?;
    item.to_stack(*count)
}

pub(super) fn accepts(host: &dyn Host, slot: Slot, stack: &ItemStack) -> bool {
    let tags = host.tags();
    let contains = |slot: Slot| {
        let id = mod_api::Identifier::parse(slot.tag()).expect("valid Curios slot tag");
        tags.key(&id)
            .and_then(|key| tags.is_in_tag(stack.item(), key))
            == Some(true)
    };
    if slot == Slot::Curio {
        Slot::ALL.into_iter().any(contains)
    } else {
        contains(slot) || contains(Slot::Curio)
    }
}
