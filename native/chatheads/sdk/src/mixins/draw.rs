// SPDX-License-Identifier: MIT
// Succeeds the ChatHeads layout and skin helpers by Domanffe.
fn draw_heads(game: &crate::game::Game<'_>, draws: &mut Vec<horizon_client::hud_layout::GuiDraw>) {
    use horizon_client::hud_layout::{Blend, GuiDraw};

    if !game.on("chatheads")
        || game
            .setting("chatheads", "heads")
            .is_some_and(|value| value.trim() == "false")
    {
        return;
    }
    let atlas = &game.gui.atlas;
    let rows = std::mem::take(draws);
    draws.reserve(rows.len());
    for mut row in rows {
        if let GuiDraw::Text {
            text, x, y, color, ..
        } = &mut row
            && *x == 0
        {
            let component = text.component();
            let character_skin = component
                .get("lunatic_skin")
                .and_then(|value| value.as_str());
            let speaker = component
                .get("horizon_chat_author")
                .and_then(|value| value.as_str());
            let skin = character_skin
                .filter(|path| atlas.sprites.contains_key(*path))
                .or_else(|| {
                    let name = speaker?;
                    if let Some(character) = game.character_for_name(name) {
                        return Some(character.skin(atlas));
                    }
                    let is_account = name.len() <= 16
                        && name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
                    if !is_account {
                        return None;
                    }
                    let key = format!("name:{}", name.to_ascii_lowercase());
                    atlas
                        .object_skins
                        .get(&key)
                        .map(String::as_str)
                        .or_else(|| Some(horizon_client::tab_list::default_skin(0)))
                });
            if let Some(skin) = skin {
                draws.push(GuiDraw::Blit {
                    texture: skin.to_owned().into(),
                    blend: Blend::Textured,
                    x: *x,
                    y: *y,
                    w: 8,
                    h: 8,
                    uv: [8.0, 8.0],
                    src: [8, 8],
                    tex: [64, 64],
                    color: *color,
                });
            }
            *x += 10;
        }
        draws.push(row);
    }
}
