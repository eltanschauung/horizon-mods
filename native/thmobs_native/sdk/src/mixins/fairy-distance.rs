fn thmobs_despawn_on_departure(game: &mut crate::game::Game<'_>, from: crate::game::Dim, to: crate::game::Dim) {
    if from == to || !game.on("thmobs_native") {
        return;
    }
    let departing = horizon_client::lan::playerdata::ints_uuid(game.acting_uuid());
    let back = game.enter_command_level(from);
    if game.dim == from && let Some(level) = game.level(from.id()) {
        mod_thmobs_native::despawn_wild_mobs(game, level, Some(departing));
    }
    if let Some(back) = back {
        game.enter_command_level(back);
    }
}
