fn jade_requests(game: &mut crate::game::Game<'_>, phase: mod_api::ServerTickPhase) {
    if matches!(phase, mod_api::ServerTickPhase::ServerTickEnd { .. }) && game.on("jade") {
        for (owner, data) in ::jade::take_requests() {
            if game.player(owner.uuid()) == Some(owner) {
                crate::game::jade::from_client(game, owner.uuid(), ::jade::REQUEST, &data);
            }
        }
    }
}
