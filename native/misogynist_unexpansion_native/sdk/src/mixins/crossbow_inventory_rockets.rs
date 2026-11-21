// `Player.getProjectile` for a crossbow with nothing to shoot in either hand: the inventory's first
// arrows or firework rockets.
fn crossbow_inventory_rockets(
    value: Option<usize>,
    weapon: &str,
    hands: &[(usize, Option<&horizon_client::items::ItemStack>); 2],
    inventory: &[Option<horizon_client::items::ItemStack>],
) -> Option<usize> {
    if weapon != "minecraft:crossbow" || value.is_some_and(|v| hands.iter().any(|(slot, _)| *slot == v)) {
        return value;
    }
    inventory
        .iter()
        .position(|s| {
            s.as_ref().is_some_and(|s| {
                matches!(
                    &*s.item,
                    "minecraft:arrow" | "minecraft:spectral_arrow" | "minecraft:tipped_arrow" | "minecraft:firework_rocket"
                )
            })
        })
        .or(value)
}
