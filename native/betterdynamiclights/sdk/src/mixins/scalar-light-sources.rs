fn scalar_light_sources(
    game: &mut crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
) -> core::ops::ControlFlow<()> {
    use horizon_client::dynamic_light::{Source, select_iter};

    if !game.on("dynamiclights") {
        return core::ops::ControlFlow::Continue(());
    }
    let mut out = std::mem::take(&mut game.dynamic_lights);
    out.clear();
    dynamiclights_horizon::with_frame_sources(game, camera.to_array(), alpha, |sources, settings| {
        select_iter(
            sources.map(|(at, level)| Source { at: at.into(), level }),
            camera,
            &mut out,
        );
        out.truncate(usize::from(settings.max_sources));
        for light in &mut out {
            light[3] = settings.shown(light[3]);
        }
    });
    game.dynamic_lights = out;
    core::ops::ControlFlow::Break(())
}
