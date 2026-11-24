//! Netherite without a smithing template: a diamond tool, weapon or piece of armour and a netherite
//! ingot in an anvil make its netherite kind, keeping its enchantments, name and wear, as the
//! smithing table would. The anvil works it out on both sides (the server's menu and the window's
//! prediction) from what it is told, so a server and its players agree.

use mod_api::ItemStack;

/// The diamond items that have a netherite kind.
const UPGRADES: [&str; 10] =
    ["sword", "pickaxe", "axe", "shovel", "hoe", "spear", "helmet", "chestplate", "leggings", "boots"];

/// Levels the upgrade costs: the least a take can cost (a free result can't be taken).
pub const XP_COST: i32 = 1;

/// The netherite kind of `item`, when it is one of the diamond items that have one.
pub fn netherite_of(item: &str) -> Option<String> {
    let piece = item.strip_prefix("minecraft:diamond_")?;
    UPGRADES.contains(&piece).then(|| format!("minecraft:netherite_{piece}"))
}

/// What the anvil makes of `left` and `right`: `left` as its netherite kind, when `right` is a
/// netherite ingot. `known` says whether the game has that item.
pub fn upgrade(host: &dyn mod_api::Host, left: &ItemStack, right: Option<&ItemStack>) -> Option<ItemStack> {
    let right = right?;
    if right.item().id(host) != "minecraft:netherite_ingot" || left.count() != 1 {
        return None;
    }
    let to = mod_api::Identifier::parse(&netherite_of(&left.item().id(host))?).ok()?;
    let mut out = ItemStack::of(host.item_key(&to)?);
    if let Some(components) = left.components() {
        out.set_components(components.as_nbt().clone()).ok()?;
    }
    if let Some(damage) = left.damage_component() {
        out.set_damage(damage);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Host, Identifier, testing::FakeHost};

    fn stack(host: &dyn Host, id: &str) -> ItemStack {
        ItemStack::of(host.item_key(&Identifier::parse(id).unwrap()).unwrap())
    }

    #[test]
    fn diamond_gear_becomes_netherite_with_an_ingot() {
        let mut host = FakeHost::new();
        host.registries.resolve_items([
            "minecraft:diamond_sword",
            "minecraft:netherite_ingot",
            "minecraft:netherite_sword",
        ].map(|id| Identifier::parse(id).unwrap())).unwrap();
        let mut sword = stack(&host, "minecraft:diamond_sword");
        sword.set_damage(40);
        let ingot = stack(&host, "minecraft:netherite_ingot");
        let out = upgrade(&host, &sword, Some(&ingot)).unwrap();
        assert_eq!(out.item().id(&host), "minecraft:netherite_sword");
        assert_eq!((out.count(), out.damage_component()), (1, Some(40)));
        assert_eq!(netherite_of("minecraft:diamond_boots").as_deref(), Some("minecraft:netherite_boots"));
    }

    #[test]
    fn nothing_else_upgrades() {
        let mut host = FakeHost::new();
        host.registries.resolve_items([
            "minecraft:netherite_ingot",
            "minecraft:diamond",
            "minecraft:iron_sword",
            "minecraft:diamond_sword",
            "minecraft:diamond_spear",
        ].map(|id| Identifier::parse(id).unwrap())).unwrap();
        let ingot = stack(&host, "minecraft:netherite_ingot");
        let diamond = stack(&host, "minecraft:diamond");
        let iron = stack(&host, "minecraft:iron_sword");
        let sword = stack(&host, "minecraft:diamond_sword");
        assert!(upgrade(&host, &diamond, Some(&ingot)).is_none());
        assert!(upgrade(&host, &iron, Some(&ingot)).is_none());
        assert!(upgrade(&host, &sword, Some(&diamond)).is_none());
        assert!(upgrade(&host, &sword, None).is_none());
        let spear = stack(&host, "minecraft:diamond_spear");
        assert!(upgrade(&host, &spear, Some(&ingot)).is_none());
    }
}
