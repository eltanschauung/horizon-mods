fn use_obituary(game: &mut crate::game::Game<'_>, click: &mod_api::ItemClick<'_>, result: &mut mod_api::InteractionResult, canceled: &mut bool) {
    use crate::game::api_values::TryIntoEngine as _;
    if game.is_logical_server() || *canceled || !game.on("gravestone_core") {
        return;
    }
    let Ok(stack) = click.stack.try_into_engine(game.items()) else { return };
    let mut claimed = false;
    crate::game::gravestone_glue::use_item(game, &stack, &mut claimed);
    if claimed {
        *canceled = true;
        *result = mod_api::InteractionResult::Consume;
    }
}
