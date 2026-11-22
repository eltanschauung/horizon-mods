fn protect_doll(value: bool, item: &str) -> bool {
    value || mod_alice_dolls_native::goals::is_doll_item(item)
}
