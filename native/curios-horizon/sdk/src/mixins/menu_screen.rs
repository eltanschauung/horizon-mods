fn curios_menu_screen(value: Option<mod_api::Screen>, menu: &str) -> Option<mod_api::Screen> {
    value.or_else(|| mod_curios_horizon::screen_layout(menu))
}
