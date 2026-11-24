fn fpm_frame_pre(game: &mut crate::game::Game<'_>) {
    crate::game::perspective::fork_first_person_body::frame_pre(game);
}
