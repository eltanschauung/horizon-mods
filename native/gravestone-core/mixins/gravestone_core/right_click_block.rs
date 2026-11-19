// Gravestone Core Right-Click Block Hook
fn gravestone_right_click_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    if !*canceled {
        crate::game::gravestone_glue::right_click_block(game, pos, canceled);
    }
}
