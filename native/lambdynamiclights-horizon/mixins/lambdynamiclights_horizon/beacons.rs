// SPDX-License-Identifier: MIT
pub(crate) fn sources(game: &crate::game::Game<'_>) -> Vec<glam::DVec3> {
    let Some(player) = &game.player else {
        return Vec::new();
    };
    let eye = player.position.y + f64::from(player.dimensions.eye_height);
    game.world
        .block_entities
        .iter()
        .filter_map(|(&(x, y, z), entity)| {
            let sections = super::beam(entity)?;
            let height = sections
                .iter()
                .map(|(_, h)| f64::from(*h))
                .sum::<f64>()
                .max(256.0);
            Some(glam::DVec3::new(
                x as f64 + 0.5,
                eye.clamp(y as f64, y as f64 + height),
                z as f64 + 0.5,
            ))
        })
        .collect()
}
