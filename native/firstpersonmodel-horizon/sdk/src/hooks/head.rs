fn fpm_head() -> core::ops::ControlFlow<()> {
    crate::game::perspective::fork_first_person_body::hide_head_items()
}
