fn save_death_drops(
    game: &mut Game<'_>,
    call: mod_api::__private::LivingDropsCall<'_>,
    run: impl FnOnce(&mut Game<'_>, mod_api::__private::LivingDropsCall<'_>) -> (),
) {
    let mod_api::__private::LivingDropsCall { entity, kind, player, source, recently_hit, drops, cancel } = call;
    run(game, mod_api::__private::LivingDropsCall { entity, kind, player, source, recently_hit, drops: &mut *drops, cancel: &mut *cancel });
    if *cancel || kind != "minecraft:player" || !game.on("gravestone_core") {
        return;
    }
    let (Some(uuid), Some(first)) = (player, drops.first()) else { return };
    let origin = first.position;
    if let Ok(engine) = crate::game::api_values::item_drops_into_engine(drops, game.items()) {
        let mut stacks = engine.into_iter().map(|drop| drop.stack).collect();
        crate::game::gravestone_glue::loot_drops(game, uuid, origin, &mut stacks);
        if stacks.is_empty() { drops.clear(); }
    }
}
