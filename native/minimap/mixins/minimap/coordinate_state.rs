thread_local! {
 pub static COORDINATES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
thread_local! {
 pub static OUTLINE: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, false)) };
}

#[derive(Clone)]
pub struct Marker { pub x:f32, pub y:f32, pub face:Option<crate::hud::Rect>, pub hat:Option<crate::hud::Rect>, pub color:u32, pub label:String }
thread_local! {
 pub static MARKERS: std::cell::RefCell<Vec<Marker>> = const { std::cell::RefCell::new(Vec::new()) };
 pub static OWN_HEAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// A colored waypoint badge shared by the radar and the floating label.
pub fn draw_badge(
    batch: &mut crate::hud::HudBatch,
    atlas: &crate::hud::HudAtlas,
    [x, y]: [f32; 2],
    label: &str,
    color: u32,
    size: f32,
) {
    use crate::hud::HudBatch;
    let left = (x - size / 2.0).round();
    let top = (y - size / 2.0).round();
    batch.fill(atlas, left, top, size, size, 0xd0000000);
    batch.fill(atlas, left + 1.0, top + 1.0, size - 2.0, size - 2.0, color);

    let scale = (size - 3.0) / 9.0;
    let width = HudBatch::text_width(atlas, label);
    let tx = (left + (size - width * scale) / 2.0).round();
    let ty = (top + (size - 8.0 * scale) / 2.0).round();
    let start = batch.vertices.len();
    batch.text(atlas, label, tx, ty, u32::MAX, true);
    batch.scale_since(start, tx, ty, scale, scale);
}

pub fn draw_markers(
    batch: &mut crate::hud::HudBatch,
    atlas: &crate::hud::HudAtlas,
    x: f32,
    y: f32,
    size: f32,
) {
    MARKERS.with_borrow(|markers| {
        for m in markers {
            let (px, py) = (x + size / 2.0 + m.x, y + size / 2.0 + m.y);
            if let Some(face) = m.face {
                batch.rect_uv(px - 4.0, py - 4.0, 8.0, 8.0, face, m.color);
                if let Some(hat) = m.hat {
                    batch.rect_uv(px - 4.5, py - 4.5, 9.0, 9.0, hat, m.color);
                }
            } else {
                draw_badge(batch, atlas, [px, py], &m.label, m.color, 10.0);
            }
        }
    });
}
