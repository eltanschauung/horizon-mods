fn coordinates_after(result: (), corners: &mut Corners, look: &MinimapLook, batch: &mut HudBatch, atlas: &HudAtlas) {
    minimap_coordinates::COORDINATES.with(|c| {
        let text = c.borrow();
        let Some(text) = text.as_deref() else { return; };
        let width = HudBatch::text_width(atlas, text) + 4.0;
        let (x, y) = if look.corner.top() {
            corners.place(look.corner, width, 12.0)
        } else {
            let i = Corner::ALL.iter().position(|c| *c == look.corner).unwrap_or(0);
            let y = corners.height - corners.used[i] + look.size + 2.0;
            let x = if look.corner.left() { 2.0 } else { corners.width - 2.0 - width };
            (x, y)
        };
        batch.fill(atlas, x, y, width, 12.0, 0x78000000);
        batch.text(atlas, text, x + 2.0, y + 2.0, rgba(255,255,255,255), true);
    });
}
