thread_local! {
 pub static COORDINATES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
thread_local! {
 pub static OUTLINE: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, false)) };
}

#[derive(Clone)]
pub struct Marker {
    pub x: f32,
    pub y: f32,
    pub face: Option<crate::hud::Rect>,
    pub hat: Option<crate::hud::Rect>,
    pub color: u32,
    pub label: String,
    pub size: f32,
}
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
    let opacity = color >> 24;
    if opacity == 0 { return; }
    let start = batch.vertices.len();
    // Draw at a fixed logical size, then scale the entire badge around its centre.
    let left = x - 6.0;
    let top = y - 6.0;
    let draw_size = 12.0;
    batch.fill(atlas, left, top, draw_size, draw_size, (opacity * 0xd0 / 255) << 24);
    batch.fill(atlas, left + 1.0, top + 1.0, draw_size - 2.0, draw_size - 2.0, color);

    let scale = (draw_size - 3.0) / 9.0;
    let width = HudBatch::text_width(atlas, label);
    let tx = left + (draw_size - width * scale) / 2.0;
    let ty = top + (draw_size - 8.0 * scale) / 2.0;
    batch.text(atlas, label, tx, ty, (opacity << 24) | 0xffffff, true);
    batch.scale_since(start, x, y, size / draw_size, size / draw_size);
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
                draw_badge(batch, atlas, [px, py], &m.label, m.color, m.size);
            }
        }
    });
}

thread_local! {
    pub static COMPASS_NORTH: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}

pub fn draw_compass(
    batch: &mut crate::hud::HudBatch,
    atlas: &crate::hud::HudAtlas,
    x: f32,
    y: f32,
    size: f32,
) {
    use crate::hud::HudBatch;
    let Some((nx, ny)) = COMPASS_NORTH.with(|direction| direction.get()) else {
        return;
    };
    let round = OUTLINE.with(|outline| outline.get().1);
    let radius = size / 2.0;
    let reach = radius - 6.0;
    let scale = 0.65;
    for (letter, dx, dy) in [
        ("N", nx, ny),
        ("E", -ny, nx),
        ("S", -nx, -ny),
        ("W", ny, -nx),
    ] {
        let rim = if round {
            reach
        } else {
            reach / dx.abs().max(dy.abs())
        };
        let width = HudBatch::text_width(atlas, letter) * scale;
        let tx = (x + radius + dx * rim - width / 2.0).round();
        let ty = (y + radius + dy * rim - 4.0 * scale).round();
        let start = batch.vertices.len();
        batch.text(atlas, letter, tx, ty, 0xf0ffffff, true);
        batch.scale_since(start, tx, ty, scale, scale);
    }
}
