// Gravestone Core Destroy Block Hook
fn gravestone_destroy_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    if !*canceled {
        crate::game::gravestone_glue::destroy_block(game, pos, canceled);
    }
}
