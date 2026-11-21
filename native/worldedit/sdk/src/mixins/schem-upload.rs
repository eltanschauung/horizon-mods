fn apply_upload(game: &mut crate::game::Game<'_>, phase: mod_api::ServerTickPhase) {
    if matches!(phase, mod_api::ServerTickPhase::ServerTickEnd { .. }) && game.on("worldedit") {
        crate::game::worldedit::schematic_from_client(game);
    }
}
