fn fpm_in_view(value: bool, game: &crate::game::Game<'_>) -> bool {
    value || crate::game::perspective::fork_first_person_body::body_in_view(game)
}
