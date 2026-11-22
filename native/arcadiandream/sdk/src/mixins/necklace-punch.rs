fn necklace_punch(game: &mut crate::game::Game<'_>, index: usize) -> core::ops::ControlFlow<bool> {
    let Some(shot) = game.creatures.projectiles.get(index) else {
        return core::ops::ControlFlow::Continue(());
    };
    if !shot.kind.id().starts_with("arcadiandream:") || !shot.kind.id().ends_with("_bullet") {
        return core::ops::ControlFlow::Continue(());
    }
    let id = shot.id;
    let cancelled = mod_arcadiandream::cancel_bullet(game, id);
    if cancelled {
        game.reset_attack_strength();
        game.swing_arm();
    }
    core::ops::ControlFlow::Break(cancelled)
}
