fn fpm_body(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client_render::render_geometry::Vertex>,
) -> core::ops::ControlFlow<()> {
    crate::game::perspective::fork_first_person_body::draw_body(game, camera, alpha, out)
}
