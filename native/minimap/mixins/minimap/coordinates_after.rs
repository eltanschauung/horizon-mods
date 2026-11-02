fn coordinates_after(result: (), corners: &mut Corners, look: &MinimapLook, batch: &mut HudBatch, atlas: &HudAtlas) {
    minimap_coordinates::COORDINATES.with(|c| {
        let text = c.borrow();
        let Some(text) = text.as_deref() else { return; };
        let width = HudBatch::text_width(atlas, text);
        let (x, y) = if look.corner.top() {
            corners.place(look.corner, look.size, 12.0)
        } else {
            let i = Corner::ALL.iter().position(|c| *c == look.corner).unwrap_or(0);
            let y = corners.height - corners.used[i] + look.size + 2.0;
            let x = if look.corner.left() { 2.0 } else { corners.width - 2.0 - look.size };
            (x, y)
        };
        batch.text(atlas, text, (x + (look.size - width) / 2.0).round(), y + 2.0, rgba(255,255,255,255), true);
    });
}
