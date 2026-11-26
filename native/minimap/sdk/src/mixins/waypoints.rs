use super::{Game, Screen};
use horizon_client::{hud::HudBatch, text::Text, ui};
use std::{
    cell::RefCell,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Waypoint {
    name: String,
    position: [f64; 3],
    dimension: String,
    color: usize,
    enabled: bool,
}
const COLORS: [u32; 6] = [
    0xff5555ff, 0xff55ff55, 0xffffaa00, 0xffff55ff, 0xff55ffff, 0xffffffff,
];
#[derive(Default)]
struct State {
    scope: String,
    points: Vec<Waypoint>,
    writable: bool,
    open: bool,
    selected: Option<usize>,
    form: [String; 4],
    field: usize,
    replace_on_type: bool,
    color: usize,
    page: usize,
    rects: Vec<[f32; 4]>,
    opacity_slider: [f32; 4],
    dragging_opacity: Option<f32>,
    error: String,
}
thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }
pub(super) fn scope(game: &Game<'_>) -> String {
    let source = if game.integrated.is_none() {
        game.server_address().map(str::to_owned)
    } else {
        None
    }
    .unwrap_or_else(|| game.save.root.to_string_lossy().into_owned());
    let hash = source
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        });
    format!("waypoints-{hash:016x}.json")
}
fn waypoint_path(game: &Game<'_>, key: &str) -> PathBuf {
    game.native_data_dir(minimap::runtime::ID).join(key)
}

fn read_waypoints(path: &Path) -> io::Result<Vec<Waypoint>> {
    const MAX_BYTES: u64 = 1024 * 1024;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Waypoint file is too large",
        ));
    }
    let points: Vec<Waypoint> = serde_json::from_slice(&bytes)?;
    if points.len() > 512
        || points.iter().any(|p| {
            p.name.chars().count() > 64
                || p.position
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 30_000_000.0)
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid waypoint data",
        ));
    }
    Ok(points)
}

fn load(game: &Game<'_>, s: &mut State) {
    let key = scope(game);
    if s.scope == key {
        return;
    }
    let result = read_waypoints(&waypoint_path(game, &key));
    *s = match result {
        Ok(points) => State {
            scope: key,
            points,
            writable: true,
            ..State::default()
        },
        Err(_) => State {
            scope: key,
            error: "Cannot read waypoint file; original preserved".into(),
            ..State::default()
        },
    };
}
fn save(game: &Game<'_>, s: &mut State) {
    if !s.writable {
        return;
    }
    let path = waypoint_path(game, &s.scope);
    let result = serde_json::to_vec(&s.points)
        .ok()
        .and_then(|bytes| horizon_client::save::write_durable(&path, &bytes).ok());
    if result.is_none() {
        s.error = "Could not save waypoints; previous file preserved".into();
    }
}

fn new_point(game: &Game<'_>, s: &mut State) {
    s.selected = None;
    s.field = 0;
    s.replace_on_type = true;
    s.form = ["Waypoint".into(), "0".into(), "64".into(), "0".into()];
    if let Some(p) = game.native_player() {
        s.form[1] = format!("{:.0}", p.x);
        s.form[2] = format!("{:.0}", p.y);
        s.form[3] = format!("{:.0}", p.z);
    }
}
pub(super) fn open_key(game: &mut Game<'_>) {
    if !game.on(minimap::runtime::ID)
        || game.screen != Screen::Playing
        || !game.consume_click(minimap::runtime::ID, "waypoints_key")
    {
        return;
    }
    STATE.with_borrow_mut(|s| {
        load(game, s);
        s.open = true;
        s.dragging_opacity = None;
        new_point(game, s);
    });
    if !game.mod_screen_open(minimap::runtime::ID) {
        STATE.with_borrow_mut(|s| s.open = false);
    }
}
fn active(s: &State, screen: &mod_api::ScreenView<'_>) -> bool {
    s.open && screen.id == "mod"
}
const OPACITY: &str = "waypoint_opacity";

thread_local! {
    static WAYPOINT_OPACITY: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

fn opacity(game: &Game<'_>) -> f32 {
    if let Some(value) = WAYPOINT_OPACITY.get() {
        return value;
    }
    let value = game
        .setting(minimap::runtime::ID, OPACITY)
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .map_or(1.0, |value| value.clamp(0.0, 1.0));
    WAYPOINT_OPACITY.set(Some(value));
    value
}

fn alpha(color: u32, opacity: f32) -> u32 {
    (color & 0xffffff) | ((((color >> 24) as f32 * opacity).round() as u32) << 24)
}

fn slider_value(rect: [f32; 4], x: f32) -> f32 {
    ((x - rect[0] - 4.0) / (rect[2] - 8.0)).clamp(0.0, 1.0)
}

pub(super) fn drag(
    game: &Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    STATE.with_borrow_mut(|s| {
        if game.mod_screen() == Some(minimap::runtime::ID)
            && active(s, screen)
            && button == 0
            && s.dragging_opacity.is_some()
        {
            s.dragging_opacity = Some(slider_value(s.opacity_slider, screen.mouse[0]));
            *canceled = true;
        }
    });
}

pub(super) fn release(
    game: &Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    STATE.with_borrow_mut(|s| {
        if game.mod_screen() == Some(minimap::runtime::ID) && active(s, screen) {
            *canceled = true;
            if button == 0 && s.dragging_opacity.take().is_some() {
                let value = slider_value(s.opacity_slider, screen.mouse[0]);
                let value = (value * 100.0).round() / 100.0;
                game.set_setting(minimap::runtime::ID, OPACITY, &format!("{value:.2}"));
                WAYPOINT_OPACITY.set(Some(value));
            }
        }
    });
}

pub(super) fn draw(game: &Game<'_>, screen: &mod_api::ScreenView<'_>, batch: &mut HudBatch) {
    STATE.with_borrow_mut(|s| {
        if game.mod_screen() != Some(minimap::runtime::ID) || !active(s, screen) {
            return;
        }
        let a = &game.gui.atlas;
        batch.fill(a, 0.0, 0.0, batch.width, batch.height, 0xff161616);
        let w = (batch.width - 16.0).clamp(100.0, 340.0);
        let x = (batch.width - w) / 2.0;
        let y = 18.0;
        batch.text(a, "Waypoints", x, y - 12.0, u32::MAX, true);
        s.rects.clear();
        let rows = 5;
        let from = s.page * rows;
        for row in 0..rows {
            let i = from + row;
            let label = s
                .points
                .get(i)
                .map(|p| {
                    format!(
                        "{} {} [{}]",
                        if p.enabled { "●" } else { "○" },
                        p.name.chars().take(14).collect::<String>(),
                        p.dimension
                            .trim_start_matches("minecraft:")
                            .chars()
                            .take(6)
                            .collect::<String>()
                    )
                })
                .unwrap_or_else(|| "-".into());
            let rect = [x, y + row as f32 * 21.0, w * 0.54, 20.0];
            s.rects.push(rect);
            ui::button(
                batch,
                a,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                &Text::literal(label),
                i < s.points.len(),
                s.selected == Some(i),
            );
        }
        let fy = y;
        let fx = x + w * 0.54 + 4.0;
        for i in 0..4 {
            let rect = [fx, fy + i as f32 * 21.0, w * 0.46 - 4.0, 20.0];
            s.rects.push(rect);
            ui::button(
                batch,
                a,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                &Text::literal(format!(
                    "{}: {}{}",
                    ["Name", "X", "Y", "Z"][i],
                    s.form[i]
                        .chars()
                        .rev()
                        .take(13)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>(),
                    if s.field == i { "_" } else { "" }
                )),
                true,
                s.field == i,
            );
        }
        let labels = ["New", "Save", "Delete", "Toggle", "Color", "<", ">"];
        for (i, label) in labels.iter().enumerate() {
            let bw = w / 7.0;
            let rect = [x + i as f32 * bw, fy + 110.0, bw - 1.0, 20.0];
            s.rects.push(rect);
            ui::button(
                batch,
                a,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                &Text::literal(*label),
                s.writable,
                false,
            );
        }
        s.opacity_slider = [x, fy + 133.0, w, 20.0];
        let value = s.dragging_opacity.unwrap_or_else(|| opacity(game));
        let label = Text::literal(format!("Waypoint Opacity: {:.0}%", value * 100.0));
        ui::slider(
            batch,
            a,
            x,
            s.opacity_slider[1],
            w,
            &label,
            value,
            hit(s.opacity_slider, screen.mouse),
            true,
            s.dragging_opacity.is_some(),
        );
        if !s.error.is_empty() {
            batch.text(a, &s.error, x, fy + 157.0, 0xff5555ff, true);
        }
    });
}
fn hit(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}
pub(super) fn press(
    game: &Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    STATE.with_borrow_mut(|s| {
        if game.mod_screen() != Some(minimap::runtime::ID) || !active(s, screen) {
            return;
        }
        *canceled = true;
        if button != 0 {
            return;
        }
        if hit(s.opacity_slider, screen.mouse) {
            s.dragging_opacity = Some(slider_value(s.opacity_slider, screen.mouse[0]));
            return;
        }
        let Some(i) = s.rects.iter().position(|r| hit(*r, screen.mouse)) else {
            return;
        };
        if i < 5 {
            let idx = s.page * 5 + i;
            if let Some(p) = s.points.get(idx) {
                s.selected = Some(idx);
                s.form = [
                    p.name.clone(),
                    p.position[0].to_string(),
                    p.position[1].to_string(),
                    p.position[2].to_string(),
                ];
                s.color = p.color % 6;
                s.replace_on_type = true;
            }
            return;
        }
        if i < 9 {
            s.field = i - 5;
            s.replace_on_type = true;
            return;
        }
        match i - 9 {
            0 => new_point(game, s),
            1 => {
                let coords = s.form[1..]
                    .iter()
                    .map(|v| v.parse::<f64>())
                    .collect::<Result<Vec<_>, _>>();
                if let Ok(c) = coords
                    && c.iter().all(|v| v.is_finite() && v.abs() <= 30_000_000.0)
                    && !s.form[0].trim().is_empty()
                    && s.writable
                {
                    let p = Waypoint {
                        name: s.form[0].trim().into(),
                        position: [c[0], c[1], c[2]],
                        dimension: game.dim.id().into(),
                        color: s.color,
                        enabled: true,
                    };
                    if let Some(i) = s.selected {
                        s.points[i] = p;
                    } else if s.points.len() < 512 {
                        s.points.push(p);
                        s.selected = Some(s.points.len() - 1);
                    }
                    s.error.clear();
                    save(game, s);
                } else {
                    s.error = "Enter a name and valid coordinates".into();
                }
            }
            2 => {
                if let Some(i) = s.selected.take() {
                    if s.writable {
                        s.points.remove(i);
                        save(game, s);
                    }
                }
            }
            3 => {
                if let Some(i) = s.selected {
                    if s.writable {
                        s.points[i].enabled = !s.points[i].enabled;
                        save(game, s);
                    }
                }
            }
            4 => s.color = (s.color + 1) % 6,
            5 => s.page = s.page.saturating_sub(1),
            6 => s.page = (s.page + 1).min(s.points.len().saturating_sub(1) / 5),
            _ => {}
        }
    });
}
pub(super) fn input(
    game: &Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    STATE.with_borrow_mut(|s| {
        if game.mod_screen() != Some(minimap::runtime::ID) || !active(s, screen) {
            return;
        }
        if key.ends_with("escape") {
            s.open = false;
            return;
        }
        *canceled = true;
        if key.ends_with("a") && modifiers & 2 != 0 {
            s.replace_on_type = true;
        }
        if key.ends_with("tab") {
            s.field = (s.field + 1) % 4;
            s.replace_on_type = true;
        }
        if key.ends_with("backspace") {
            let f = s.field;
            if s.replace_on_type {
                s.form[f].clear();
                s.replace_on_type = false;
            } else {
                s.form[f].pop();
            }
        }
    });
}
pub(super) fn typed(
    game: &Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    character: char,
    canceled: &mut bool,
) {
    STATE.with_borrow_mut(|s| {
        if game.mod_screen() == Some(minimap::runtime::ID) && active(s, screen) {
            *canceled = true;
            let f = s.field;
            if !character.is_control() {
                if s.replace_on_type {
                    s.form[f].clear();
                    s.replace_on_type = false;
                }
                if s.form[f].chars().count() < 64 {
                    s.form[f].push(character);
                }
            }
        }
    });
}
pub(super) fn swallow(game: &Game<'_>, screen: &mod_api::ScreenView<'_>, canceled: &mut bool) {
    STATE.with_borrow(|s| {
        if game.mod_screen() == Some(minimap::runtime::ID) && active(s, screen) {
            *canceled = true;
        }
    });
}
fn skin_rects(game: &Game<'_>, skin: &str) -> Option<([f32; 4], [f32; 4])> {
    let r = *game.gui.atlas.sprites.get(skin)?;
    let sub = |x: f32, y: f32| {
        [
            r.u0 + (r.u1 - r.u0) * x / 64.0,
            r.v0 + (r.v1 - r.v0) * y / 64.0,
            r.u0 + (r.u1 - r.u0) * (x + 8.0) / 64.0,
            r.v0 + (r.v1 - r.v0) * (y + 8.0) / 64.0,
        ]
    };
    Some((sub(8.0, 8.0), sub(40.0, 8.0)))
}
pub(super) fn markers(game: &Game<'_>, settings: minimap::runtime::Settings) {
    use minimap::state::{MARKERS, Marker, OWN_HEAD};
    let Some(p) = game.native_player() else {
        return;
    };
    let (size, _) = settings.view();
    let Some(view) = game.composed_minimap_view() else {
        return;
    };
    let project = |x, z| view.project(x, z, size);
    let partial = game.clock.camera_fraction();
    let radius = size / 2.0;
    let norm = |x: f32, y: f32| {
        if settings.round {
            x.hypot(y)
        } else {
            x.abs().max(y.abs())
        }
    };
    let mut markers = Vec::new();
    for m in game
        .mobs
        .iter()
        .filter(|m| m.kind.id.as_str() == "minecraft:player" && !m.is_invisible())
    {
        let (x, y) = project(
            m.previous.x + (m.position.x - m.previous.x) * partial,
            m.previous.z + (m.position.z - m.previous.z) * partial,
        );
        if norm(x, y) <= radius - 7.0
            && let Some((face, hat)) = m.skin.as_deref().and_then(|skin| skin_rects(game, skin))
        {
            let opacity = game
                .player_presentation_opacity(m, glam::DVec3::new(p.x, p.y, p.z))
                .clamp(0.0, 1.0);
            let color = (((opacity * 255.0).round() as u32) << 24) | 0x00ffffff;
            markers.push(Marker {
                x,
                y,
                face: Some(face),
                hat: Some(hat),
                color,
                label: String::new(),
                size: 0.0,
            });
        }
    }
    if settings.player_head
        && let Some((face, hat)) = game
            .player_model
            .as_ref()
            .and_then(|m| m.skin.as_deref())
            .and_then(|skin| skin_rects(game, skin))
    {
        markers.push(Marker {
            x: 0.0,
            y: 0.0,
            face: Some(face),
            hat: Some(hat),
            color: u32::MAX,
            label: String::new(),
            size: 0.0,
        });
        OWN_HEAD.with(|v| v.set(true));
    }
    let opacity = opacity(game);
    if opacity > 0.0 {
        for_each_waypoint(game, |point, _, glyph, color| {
            let (mut x, mut y) = project(point[0], point[2]);
            let distance = norm(x, y);
            let size = 10.0 * (1.0 - 0.65 * (distance / radius).clamp(0.0, 1.0));
            // The entire square badge stays inside the map, even on a round map.
            let half = size / 2.0;
            let inset = if settings.round {
                std::f32::consts::SQRT_2 * half
            } else {
                half
            };
            let reach = radius - inset;
            if distance > reach {
                x *= reach / distance;
                y *= reach / distance;
            }
            markers.push(Marker {
                x,
                y,
                face: None,
                hat: None,
                color: alpha(color, opacity),
                label: glyph.to_string(),
                size,
            });
        });
    }
    MARKERS.with_borrow_mut(|v| *v = markers);
}

fn for_each_waypoint(game: &Game<'_>, mut visit: impl FnMut([f64; 3], &str, char, u32)) {
    STATE.with_borrow_mut(|s| {
        load(game, s);
        for w in s
            .points
            .iter()
            .filter(|w| w.enabled && w.dimension == game.dim.id())
        {
            let glyph = w
                .name
                .chars()
                .find(|c| !c.is_whitespace() && !c.is_control())
                .and_then(|c| c.to_uppercase().next())
                .unwrap_or('?');
            visit(w.position, &w.name, glyph, COLORS[w.color % COLORS.len()]);
        }
    });
    for point in super::game_grave_waypoints::points(game) {
        visit(point, "Gravestone", '×', 0xffff5555);
    }
}

pub(super) fn draw_world(game: &Game<'_>, batch: &mut HudBatch) {
    use glam::{DVec3, Mat4, Vec4};

    if game.screen != Screen::Playing || !game.on(minimap::runtime::ID) {
        return;
    }
    let opacity = opacity(game);
    if opacity == 0.0 {
        return;
    }
    let [width, height] = [batch.width, batch.height];
    let (eye, matrix) = if let Some(camera) = game.camera() {
        (
            camera.position,
            Mat4::from_cols_array(&camera.projection) * Mat4::from_cols_array(&camera.view),
        )
    } else if let Some((eye, yaw, pitch, fov)) = game.locator_camera {
        // A mixin-only build can use the last rendered camera when the native API has no listeners.
        (
            eye,
            Mat4::perspective_infinite_rh(fov.to_radians(), width / height.max(1.0), 0.05)
                * horizon_client::camera::view_matrix(yaw, pitch),
        )
    } else {
        return;
    };

    let mut labels = Vec::new();
    let origin = game.native_player().map_or(eye, |p| [p.x, p.y, p.z]);
    for_each_waypoint(game, |point, title, glyph, color| {
        let distance = (DVec3::from_array(point) - DVec3::from_array(origin)).length();
        let offset = DVec3::from_array(point) - DVec3::from_array(eye) + DVec3::Y;
        let clip = matrix * Vec4::from((offset.as_vec3(), 1.0));
        if clip.w <= 0.05 || !clip.is_finite() {
            return;
        }
        let x = (0.5 + clip.x / (2.0 * clip.w)) * width;
        let y = (0.5 - clip.y / (2.0 * clip.w)) * height;
        if (7.0..=width - 7.0).contains(&x) && (7.0..=height - 19.0).contains(&y) {
            labels.push((distance, x, y, Text::literal(title), glyph, color));
        }
    });
    labels.sort_by(|a, b| b.0.total_cmp(&a.0));

    let atlas = &game.gui.atlas;
    let zoom = game.native_zoom_now().map_or(1.0, |zoom| zoom.factor);
    for (distance, x, y, title, glyph, color) in labels {
        let distance_scale = (1.0 - 0.65 * (distance / 1000.0).clamp(0.0, 1.0)) as f32 * zoom;
        let mut letter = [0; 4];
        draw_badge(
            batch,
            atlas,
            [x, y],
            glyph.encode_utf8(&mut letter),
            alpha(color, opacity),
            12.0 * distance_scale,
        );
        let text_width = atlas.font.width_text(&title).max(1.0);
        let scale = ((width - 8.0) / text_width).min(distance_scale);
        let title_width = text_width * scale + 4.0 * distance_scale;
        let tx = (x - title_width / 2.0)
            .clamp(2.0, (width - title_width - 2.0).max(2.0))
            .round();
        let ty = y + 7.0 * distance_scale;
        batch.fill(
            atlas,
            tx,
            ty,
            title_width,
            3.0 * distance_scale + 8.0 * scale,
            alpha(0xb0000000, opacity),
        );
        let start = batch.vertices.len();
        batch.text_styled(
            atlas,
            &title,
            tx + 2.0 * distance_scale,
            ty + distance_scale,
            alpha(u32::MAX, opacity),
            true,
        );
        if scale != 1.0 {
            batch.scale_since(
                start,
                tx + 2.0 * distance_scale,
                ty + distance_scale,
                scale,
                scale,
            );
        }
    }
}

/// A colored waypoint badge shared by the radar and the floating label.
pub fn draw_badge(
    batch: &mut horizon_client::hud::HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    [x, y]: [f32; 2],
    label: &str,
    color: u32,
    size: f32,
) {
    use horizon_client::hud::HudBatch;
    let opacity = color >> 24;
    if opacity == 0 {
        return;
    }
    let start = batch.vertices.len();
    // Draw at a fixed logical size, then scale the entire badge around its centre.
    let left = x - 6.0;
    let top = y - 6.0;
    let draw_size = 12.0;
    batch.fill(
        atlas,
        left,
        top,
        draw_size,
        draw_size,
        (opacity * 0xd0 / 255) << 24,
    );
    batch.fill(
        atlas,
        left + 1.0,
        top + 1.0,
        draw_size - 2.0,
        draw_size - 2.0,
        color,
    );

    let scale = (draw_size - 3.0) / 9.0;
    let width = HudBatch::text_width(atlas, label);
    let tx = left + (draw_size - width * scale) / 2.0;
    let ty = top + (draw_size - 8.0 * scale) / 2.0;
    batch.text(atlas, label, tx, ty, (opacity << 24) | 0xffffff, true);
    batch.scale_since(start, x, y, size / draw_size, size / draw_size);
}
