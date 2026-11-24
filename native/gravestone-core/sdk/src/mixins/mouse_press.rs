fn gravestone_mouse_press(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::gravestone_glue::screen_mouse_button_pressed_pre(
        game, screen, button, modifiers, canceled,
    );
}
