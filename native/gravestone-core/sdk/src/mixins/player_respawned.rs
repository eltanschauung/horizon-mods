// Gravestone Core Player Respawn Hook
fn gravestone_player_respawned(
    game: &mut crate::game::Game<'_>,
    player: u128,
    name: &str,
) {
    crate::game::gravestone_glue::player_respawned(game, player, name);
}
