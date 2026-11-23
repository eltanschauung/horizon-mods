// SPDX-License-Identifier: EUPL-1.2-or-later

fn despawn_on_dimension_departure(
    game: &mut crate::game::Game<'_>,
    from: crate::game::Dim,
    to: crate::game::Dim,
) {
    if from == to || !game.on("arcadiandream") {
        return;
    }
    // The player's new dimension is already recorded. Visit the departure
    // level before its chunks can unload, using the normal command level swap.
    // The guest turn still presents the traveler as the local player.
    let departing = horizon_client::lan::playerdata::ints_uuid(game.acting_uuid());
    let back = game.enter_command_level(from);
    if game.dim == from
        && let Some(level) = game.level(from.id())
    {
        mod_arcadiandream::despawn_level(game, level, Some(departing));
    }
    if let Some(back) = back {
        game.enter_command_level(back);
    }
}
