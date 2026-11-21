fn fpm_held() -> core::ops::ControlFlow<()> {
    crate::game::perspective::fork_first_person_body::hide_held_items()
}
