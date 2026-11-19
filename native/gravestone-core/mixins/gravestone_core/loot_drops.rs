// Gravestone Core Player Death Loot Drops Hook
fn gravestone_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::gravestone_glue::loot_drops(game, context, drops);
}
