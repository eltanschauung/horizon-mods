//! Named stacks in doll inventories and attachments. Moscow's slots can hold up to 297
//! items, so they are packed into bounded stacks before entering a vanilla container.
use mod_api::{Compound, Host, Identifier, ItemStack, Tag};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DollStack {
    pub item: String,
    pub count: u32,
    pub damage: u32,
    pub extra: Option<Box<StackData>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackData {
    pub custom_name: Option<Tag>,
    pub container: Vec<(u8, DollStack)>,
    pub enchantments: Vec<(String, u16)>,
    pub(crate) opaque_components: Compound,
}

impl DollStack {
    pub fn new(item: impl Into<String>, count: u32) -> Self {
        Self {
            item: item.into(),
            count,
            damage: 0,
            extra: None,
        }
    }

    pub fn container(&self) -> &[(u8, DollStack)] {
        self.extra.as_ref().map_or(&[], |extra| &extra.container)
    }

    pub fn enchantment_level(&self, name: &str) -> u32 {
        let Some(extra) = &self.extra else { return 0 };
        if let Some((_, level)) = extra.enchantments.iter().find(|(kind, _)| kind == name) {
            return u32::from(*level);
        }
        let Some(enchantments) = extra.opaque_components.get(&"minecraft:enchantments".into()) else {
            return 0;
        };
        enchantments.get("levels").unwrap_or(enchantments).get(name).and_then(unsigned).unwrap_or(0)
    }

    pub fn from_game(host: &dyn Host, stack: &ItemStack) -> Option<Self> {
        Self::decode(&Tag::Compound(stack.to_nbt(host)?))
    }

    pub fn to_game(&self, host: &dyn Host) -> Option<ItemStack> {
        ItemStack::from_nbt(host, &self.encode())
    }

    fn decode(tag: &Tag) -> Option<Self> {
        let Tag::String(id) = tag.get("id")? else {
            return None;
        };
        let id = Identifier::parse(&id.to_string_lossy()).ok()?.to_string();
        if id == "minecraft:air" {
            return None;
        }
        let count = tag.get("count").and_then(unsigned).unwrap_or(1).max(1);
        let mut stack = Self::new(id, count);
        if let Some(Tag::Compound(patch)) = tag.get("components") {
            let mut other = patch.clone();
            let damage = other.get(&"minecraft:damage".into()).and_then(unsigned);
            if let Some(damage) = damage {
                stack.damage = damage;
                // Keep an explicit zero because it overrides the item's own default.
                if damage != 0 {
                    other.remove(&"minecraft:damage".into());
                }
            }
            let name = other.remove(&"minecraft:custom_name".into());
            let mut container = Vec::new();
            if let Some(Tag::List(entries)) = other.get(&"minecraft:container".into()) {
                let decoded: Option<Vec<_>> = entries
                    .iter()
                    .map(|entry| {
                        let slot = u8::try_from(unsigned(entry.get("slot")?)?).ok()?;
                        Some((slot, Self::decode(entry.get("item")?)?))
                    })
                    .collect();
                if let Some(decoded) = decoded {
                    container = decoded;
                    other.remove(&"minecraft:container".into());
                }
            }
            // Arbitrary enchantment patches are kept byte-for-byte. Doll-generated panel icons
            // use `enchantments` to add their single visual enchantment on encoding.
            let extra = StackData {
                custom_name: name,
                container,
                enchantments: Vec::new(),
                opaque_components: other,
            };
            if extra != StackData::default() {
                stack.extra = Some(Box::new(extra));
            }
        }
        Some(stack)
    }

    pub fn encode(&self) -> Compound {
        let mut tag: Compound = [
            ("id".into(), Tag::String(self.item.as_str().into())),
            (
                "count".into(),
                Tag::Int(self.count.min(i32::MAX as u32) as i32),
            ),
        ]
        .into();
        let mut components = Compound::new();
        if let Some(extra) = &self.extra {
            components = extra.opaque_components.clone();
            if let Some(name) = &extra.custom_name {
                components.insert("minecraft:custom_name".into(), name.clone());
            }
            if !extra.container.is_empty() {
                let entries = extra
                    .container
                    .iter()
                    .map(|(slot, stack)| {
                        Tag::compound([
                            ("slot", Tag::Int(i32::from(*slot))),
                            ("item", Tag::Compound(stack.encode())),
                        ])
                    })
                    .collect();
                components.insert("minecraft:container".into(), Tag::List(entries));
            }
            if !extra.enchantments.is_empty() {
                let entries = extra
                    .enchantments
                    .iter()
                    .map(|(kind, level)| (kind.as_str().into(), Tag::Int(i32::from(*level))))
                    .collect();
                components.insert("minecraft:enchantments".into(), Tag::Compound(entries));
            }
        }
        if self.damage != 0 {
            components.insert(
                "minecraft:damage".into(),
                Tag::Int(self.damage.min(i32::MAX as u32) as i32),
            );
        }
        if !components.is_empty() {
            tag.insert("components".into(), Tag::Compound(components));
        }
        tag
    }
}

fn unsigned(tag: &Tag) -> Option<u32> {
    u32::try_from(match tag {
        Tag::Byte(n) => i64::from(*n),
        Tag::Short(n) => i64::from(*n),
        Tag::Int(n) => i64::from(*n),
        Tag::Long(n) => *n,
        _ => return None,
    })
    .ok()
}

pub fn encode_items(items: &[Option<DollStack>]) -> Tag {
    Tag::List(
        items
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| {
                let slot = match u8::try_from(slot) {
                    Ok(slot) => Tag::Byte(slot as i8),
                    Err(_) => Tag::Int(i32::try_from(slot).ok()?),
                };
                let stack = stack.as_ref().filter(|stack| stack.count > 0)?;
                let mut tag = stack.encode();
                tag.insert("Slot".into(), slot);
                Some(Tag::Compound(tag))
            })
            .collect(),
    )
}

pub fn decode_items(tag: Option<&Tag>, slots: usize) -> Vec<Option<DollStack>> {
    let mut items = vec![None; slots];
    if let Some(Tag::List(entries)) = tag {
        for entry in entries {
            let slot = match entry.get("Slot") {
                Some(Tag::Byte(slot)) => usize::from(*slot as u8),
                Some(tag) => match unsigned(tag).and_then(|slot| usize::try_from(slot).ok()) {
                    Some(slot) => slot,
                    None => continue,
                },
                None => continue,
            };
            if let Some(item) = items.get_mut(slot) {
                *item = DollStack::decode(entry);
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserve_last_slot_and_unknown_item_components() {
        let name = Tag::String("older world".into());
        let data: Compound = [("example:unknown_component".into(), Tag::Int(42))].into();
        let entry = Tag::compound([
            ("id", Tag::String("example:unloaded_item".into())),
            ("count", Tag::Int(297)),
            ("Slot", Tag::Byte(250_u8 as i8)),
            ("components", Tag::Compound(data.clone())),
        ]);
        let mut items = decode_items(Some(&Tag::List(vec![entry])), 256);
        let stack = items[250].as_mut().unwrap();
        assert_eq!(stack.count, 297);
        assert_eq!(stack.extra.as_ref().unwrap().opaque_components, data);
        stack.extra.as_mut().unwrap().custom_name = Some(name);
        items.resize(300, None);
        items[299] = items[250].clone();
        let saved = encode_items(&items);
        assert_eq!(decode_items(Some(&saved), 300), items);
    }

    #[test]
    fn read_enchantments_without_reencoding_the_patch() {
        for patch in [
            Tag::compound([("minecraft:lure", Tag::Int(3))]),
            Tag::compound([("levels", Tag::compound([("minecraft:lure", Tag::Int(3))]))]),
        ] {
            let components = Tag::compound([("minecraft:enchantments", patch)]);
            let stack = DollStack::decode(&Tag::compound([
                ("id", Tag::String("minecraft:fishing_rod".into())),
                ("components", components.clone()),
            ])).unwrap();
            assert_eq!(stack.enchantment_level("minecraft:lure"), 3);
            assert_eq!(stack.enchantment_level("minecraft:unbreaking"), 0);
            assert_eq!(stack.encode().get(&"components".into()), Some(&components));
        }
    }
}
