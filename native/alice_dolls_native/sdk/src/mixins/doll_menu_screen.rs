fn doll_menu_screen(value: Option<mod_api::Screen>, menu: &str) -> Option<mod_api::Screen> {
    value.or_else(|| mod_alice_dolls_native::inventory_screen(menu))
}
