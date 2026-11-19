// Gravestone Core Player Inventory Changed Hook
fn gravestone_player_inventory_changed(
    player: u128,
    stacks: &[Option<horizon_client::items::ItemStack>],
) {
    crate::game::gravestone_glue::player_inventory_changed(player, stacks);
}
