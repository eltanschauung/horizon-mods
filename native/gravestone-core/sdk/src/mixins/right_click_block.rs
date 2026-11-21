fn click_grave(game: &mut crate::game::Game<'_>, click: &mod_api::BlockClick<'_>, result: &mut mod_api::InteractionResult, canceled: &mut bool) {
    if !game.is_logical_server() || *canceled || !game.on("gravestone_core") {
        return;
    }
    crate::game::gravestone_glue::right_click_block(game, click.pos, canceled);
    if *canceled {
        *result = mod_api::InteractionResult::Consume;
    }
}
