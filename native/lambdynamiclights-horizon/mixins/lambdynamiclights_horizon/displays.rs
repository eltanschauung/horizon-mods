// SPDX-License-Identifier: MIT
pub(crate) fn sources(
    game: &crate::game::Game<'_>,
    alpha: f64,
    item: impl Fn(&str) -> u8,
) -> Vec<(glam::DVec3, u8)> {
    game.text_displays
        .values()
        .filter_map(|d| {
            let level = match d.shows {
                super::Shows::Block => d
                    .block
                    .and_then(|id| game.res.facts().states.get(id.0 as usize))
                    .map_or(0, |f| f.emission),
                super::Shows::Item => d.item.as_ref().map_or(0, |s| item(&s.item)),
                super::Shows::Text => 0,
            };
            (level > 0).then(|| (d.previous.lerp(d.position, alpha), level))
        })
        .collect()
}
