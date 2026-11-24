//! The client's biome screen, like the original's: a search field, the biomes in a list that
//! scrolls, sorted by name or by the mod they come from, and a Search button.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use mod_api::{DrawContext, Host};

use crate::{icon, key as item_key, pretty, source};

pub const MOD_ID: &str = "naturescompass_native";
pub const COMPASS: &str = "naturescompass:naturescompass";
/// Server to client: open the screen with these biomes.
pub const OPEN: &str = "naturescompass_native:open";
/// Client to server: search for this biome.
pub const SEARCH: &str = "naturescompass_native:search";
const QUERY_MAX: usize = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    Source,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Sort::Name => "Sort: Name",
            Sort::Source => "Sort: Source",
        }
    }
}

#[derive(Debug, Default)]
pub struct Screen {
    pub biomes: Vec<String>,
    /// What the held compass found last, if anything.
    pub current: Option<String>,
    pub query: String,
    pub sort: Sort,
    /// The selected biome (its id: it stays selected when the list is filtered or sorted).
    pub selected: Option<String>,
    pub first_row: usize,
    pub dragging: bool,
    last_click: Option<(String, Instant)>,
}

impl Screen {
    pub fn new(biomes: Vec<String>, current: Option<String>) -> Self {
        Self {
            selected: current.clone(),
            biomes,
            current,
            ..Self::default()
        }
    }

    /// The biomes shown: those matching the search (by name, id or mod), in the chosen order.
    pub fn shown(&self) -> Vec<&String> {
        let q = self.query.to_lowercase();
        let mut v: Vec<&String> = self
            .biomes
            .iter()
            .filter(|b| {
                q.is_empty()
                    || pretty(b).to_lowercase().contains(&q)
                    || b.to_lowercase().contains(&q)
                    || source(b).to_lowercase().contains(&q)
            })
            .collect();
        match self.sort {
            Sort::Name => v.sort_by_key(|b| pretty(b).to_lowercase()),
            Sort::Source => v.sort_by_key(|b| {
                (
                    source(b) != "Minecraft",
                    source(b).to_lowercase(),
                    pretty(b).to_lowercase(),
                )
            }),
        }
        v
    }
}

#[derive(Debug, Default)]
pub enum Ui {
    #[default]
    Closed,
    Open(Screen),
}

pub static UI: Mutex<Ui> = Mutex::new(Ui::Closed);

/// What the input hooks (which can't send) leave for the client tick: messages, and a close.
#[derive(Debug, Default)]
pub struct Outbox {
    pub messages: Vec<(&'static str, Vec<u8>)>,
    pub close: bool,
}

pub static OUTBOX: Mutex<Outbox> = Mutex::new(Outbox {
    messages: Vec::new(),
    close: false,
});

fn search(biome: &str) {
    if let Ok(mut out) = OUTBOX.lock() {
        out.messages.push((SEARCH, biome.as_bytes().to_vec()));
        out.close = true;
    }
}

fn close() {
    if let Ok(mut out) = OUTBOX.lock() {
        out.close = true;
    }
}

pub fn encode_open(biomes: &[String], current: Option<&str>) -> Vec<u8> {
    let mut out = format!("current\t{}\n", current.unwrap_or(""));
    for b in biomes {
        out += &format!("biome\t{b}\n");
    }
    out.into_bytes()
}

pub fn decode_open(data: &[u8]) -> Option<(Vec<String>, Option<String>)> {
    let text = std::str::from_utf8(data).ok()?;
    let (mut biomes, mut current) = (Vec::new(), None);
    for line in text.lines() {
        match line.split_once('\t') {
            Some(("current", c)) if !c.is_empty() => current = Some(c.to_owned()),
            Some(("biome", b)) if b.contains(':') => biomes.push(b.to_owned()),
            _ => {}
        }
    }
    Some((biomes, current))
}

// ---- Layout ----

type Rect = [f32; 4];

fn inside(r: Rect, p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

const ROW_H: f32 = 18.0;
const PAD: f32 = 8.0;
const BAR_W: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub panel: Rect,
    pub field: Rect,
    pub sort: Rect,
    pub list: Rect,
    pub track: Option<Rect>,
    pub search: Rect,
    pub cancel: Rect,
    pub info_y: f32,
    pub visible: usize,
    pub count: usize,
}

impl Layout {
    pub fn new(w: f32, h: f32, count: usize) -> Self {
        let pw = (w - 16.0).min(340.0).max(200.0);
        let ph = (h - 16.0).min(300.0).max(150.0);
        let panel = [(w - pw) / 2.0, (h - ph) / 2.0, pw, ph];
        let (x, y) = (panel[0], panel[1]);
        let sort_w = 78.0;
        let field = [x + PAD, y + 22.0, pw - 3.0 * PAD - sort_w, 16.0];
        let sort = [field[0] + field[2] + PAD, y + 22.0, sort_w, 16.0];
        let buttons_y = y + ph - PAD - 18.0;
        let info_y = buttons_y - 13.0;
        let list = [
            x + PAD,
            y + 44.0,
            pw - 2.0 * PAD - BAR_W - 2.0,
            (info_y - 4.0 - (y + 44.0)).max(ROW_H),
        ];
        let visible = ((list[3] / ROW_H).floor() as usize).max(1);
        let track = (count > visible).then(|| [list[0] + list[2] + 2.0, list[1], BAR_W, list[3]]);
        let bw = (pw - 3.0 * PAD) / 2.0;
        Self {
            panel,
            field,
            sort,
            list,
            track,
            search: [x + PAD, buttons_y, bw, 18.0],
            cancel: [x + 2.0 * PAD + bw, buttons_y, bw, 18.0],
            info_y,
            visible,
            count,
        }
    }

    pub fn max_first(&self) -> usize {
        self.count.saturating_sub(self.visible)
    }

    /// Where row `index` is drawn, when it is in view.
    pub fn row(&self, index: usize, first: usize) -> Option<Rect> {
        if index < first || index >= first + self.visible {
            return None;
        }
        Some([
            self.list[0],
            self.list[1] + (index - first) as f32 * ROW_H,
            self.list[2],
            ROW_H,
        ])
    }

    pub fn thumb(&self, first: usize) -> Option<Rect> {
        let t = self.track?;
        let h = (t[3] * self.visible as f32 / self.count as f32).max(10.0);
        let y = t[1] + (t[3] - h) * first as f32 / self.max_first().max(1) as f32;
        Some([t[0], y, t[2], h])
    }

    pub fn first_at(&self, y: f32) -> usize {
        let Some(t) = self.track else { return 0 };
        let f = ((y - t[1]) / t[3]).clamp(0.0, 1.0);
        ((f * self.max_first() as f32).round() as usize).min(self.max_first())
    }
}

// ---- Drawing ----

const PANEL: u32 = 0xF0_14_1C_12;
const EDGE: u32 = 0xFF_6E_8B_3D;
const ROW: u32 = 0xFF_20_2B_1C;
const ROW_HOVER: u32 = 0xFF_33_44_2B;
const ROW_SELECTED: u32 = 0xFF_4F_6B_32;
const WHITE: u32 = 0xFF_FF_FF_FF;
const GREY: u32 = 0xFF_A0_A0_A0;
const GREEN: u32 = 0xFF_B5_E0_7A;
const GOLD: u32 = 0xFF_FF_C8_4A;

fn frame(draw: &mut dyn DrawContext, r: Rect, fill: u32, edge: u32) {
    draw.fill([r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], edge);
    draw.fill(r, fill);
}

fn button(draw: &mut dyn DrawContext, r: Rect, label: &str, mouse: [f32; 2], enabled: bool) {
    let fill = if !enabled {
        0xFF_24_24_24
    } else if inside(r, mouse) {
        0xFF_55_72_36
    } else {
        0xFF_34_47_25
    };
    frame(draw, r, fill, if enabled { EDGE } else { 0xFF_44_44_44 });
    let w = draw.text_width(label);
    draw.text(
        label,
        [r[0] + (r[2] - w) / 2.0, r[1] + (r[3] - 8.0) / 2.0],
        if enabled { WHITE } else { 0x70_70_70 },
        true,
    );
}

fn fit(draw: &dyn DrawContext, text: &str, width: f32) -> String {
    if draw.text_width(text) <= width {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while !cut.is_empty() && draw.text_width(&format!("{cut}...")) > width {
        cut.pop();
    }
    format!("{cut}...")
}

pub fn draw(
    host: &dyn Host,
    draw: &mut dyn DrawContext,
    window: Rect,
    mouse: [f32; 2],
    blink: bool,
) {
    let Ok(ui) = UI.lock() else { return };
    let Ui::Open(s) = &*ui else { return };
    let shown = s.shown();
    let l = Layout::new(window[2], window[3], shown.len());
    draw.fill(window, 0x90_00_00_00);
    frame(draw, l.panel, PANEL, EDGE);
    if let Some(key) = item_key(host, COMPASS) {
        draw.item(key, [l.panel[0] + PAD - 2.0, l.panel[1] + 3.0], 16.0);
    }
    draw.text(
        "Nature's Compass",
        [l.panel[0] + PAD + 18.0, l.panel[1] + 7.0],
        GREEN,
        true,
    );
    let count = format!("{} biomes", shown.len());
    let cw = draw.text_width(&count);
    draw.text(
        &count,
        [l.panel[0] + l.panel[2] - PAD - cw, l.panel[1] + 7.0],
        GREY,
        false,
    );
    // The search field.
    frame(draw, l.field, 0xFF_00_00_00, 0xFF_A0_A0_A0);
    let cursor = if blink { "_" } else { "" };
    let at = [l.field[0] + 4.0, l.field[1] + 4.0];
    if s.query.is_empty() {
        draw.text("Search biomes...", at, 0xFF_60_60_60, false);
        draw.text(cursor, at, WHITE, false);
    } else {
        draw.text(
            &format!("{}{cursor}", fit(draw, &s.query, l.field[2] - 12.0)),
            at,
            WHITE,
            false,
        );
    }
    button(draw, l.sort, s.sort.label(), mouse, true);
    // The list.
    draw.fill(l.list, 0xFF_0C_10_0A);
    let mut hovered: Option<&String> = None;
    for (i, b) in shown.iter().enumerate() {
        let Some(r) = l.row(i, s.first_row) else {
            continue;
        };
        let over = inside(r, mouse);
        if over {
            hovered = Some(b);
        }
        let sel = s.selected.as_deref() == Some(b.as_str());
        draw.fill(
            [r[0], r[1], r[2], r[3] - 1.0],
            if sel {
                ROW_SELECTED
            } else if over {
                ROW_HOVER
            } else {
                ROW
            },
        );
        if let Some(key) = item_key(host, icon(b)) {
            draw.item(key, [r[0] + 1.0, r[1] + 1.0], 16.0);
        }
        let src = source(b);
        let sw = draw.text_width(&src);
        draw.text(&src, [r[0] + r[2] - 4.0 - sw, r[1] + 5.0], GREY, false);
        let name = fit(draw, &pretty(b), r[2] - sw - 30.0);
        let colour = if s.current.as_deref() == Some(b.as_str()) {
            GOLD
        } else {
            WHITE
        };
        draw.text(&name, [r[0] + 20.0, r[1] + 5.0], colour, true);
    }
    if shown.is_empty() {
        draw.text(
            "No biome matches",
            [l.list[0] + 6.0, l.list[1] + 6.0],
            GREY,
            false,
        );
    }
    if let (Some(t), Some(th)) = (l.track, l.thumb(s.first_row)) {
        draw.fill(t, 0xFF_08_0A_06);
        draw.fill(
            th,
            if inside(th, mouse) {
                0xFF_B5_E0_7A
            } else {
                EDGE
            },
        );
    }
    // What is selected or pointed at.
    let info = match hovered.or(s.selected.as_ref()) {
        Some(b) => format!("{}  ({})", pretty(b), b),
        None => match &s.current {
            Some(c) => format!("Last found: {}", pretty(c)),
            None => "Pick a biome, then Search (or double-click it)".to_owned(),
        },
    };
    draw.text(
        &fit(draw, &info, l.panel[2] - 2.0 * PAD),
        [l.panel[0] + PAD, l.info_y],
        GREY,
        false,
    );
    button(draw, l.search, "Search", mouse, s.selected.is_some());
    button(draw, l.cancel, "Cancel", mouse, true);
}

// ---- Input ----

fn reselect_into_view(s: &mut Screen, visible: usize) {
    let shown = s.shown();
    let Some(sel) = &s.selected else { return };
    if let Some(i) = shown.iter().position(|b| *b == sel) {
        if i < s.first_row {
            s.first_row = i;
        } else if i >= s.first_row + visible {
            s.first_row = i + 1 - visible;
        }
    }
}

fn clamp_scroll(s: &mut Screen, window: Rect) {
    let n = s.shown().len();
    let max = Layout::new(window[2], window[3], n).max_first();
    s.first_row = s.first_row.min(max);
}

pub fn typed(window: Rect, c: char) {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open(s) = &mut *ui
        && !c.is_control()
        && s.query.chars().count() < QUERY_MAX
    {
        s.query.push(c);
        s.first_row = 0;
        clamp_scroll(s, window);
    }
}

/// A key: whether it is ours (Escape goes on and closes the screen).
pub fn key(window: Rect, name: &str) -> bool {
    let Ok(mut ui) = UI.lock() else { return false };
    let Ui::Open(s) = &mut *ui else { return false };
    if name == "key.keyboard.escape" {
        return false;
    }
    let visible = Layout::new(window[2], window[3], s.shown().len()).visible;
    match name {
        "key.keyboard.backspace" => {
            s.query.pop();
            clamp_scroll(s, window);
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
            let shown = s.shown();
            let pick = s
                .selected
                .clone()
                .filter(|b| shown.contains(&b))
                .or_else(|| (shown.len() == 1).then(|| shown[0].clone()));
            if let Some(b) = pick {
                search(&b);
            }
        }
        "key.keyboard.up" | "key.keyboard.down" => {
            let shown: Vec<String> = s.shown().into_iter().cloned().collect();
            if !shown.is_empty() {
                let at = s
                    .selected
                    .as_ref()
                    .and_then(|b| shown.iter().position(|x| x == b));
                let next = match (at, name) {
                    (None, _) => 0,
                    (Some(i), "key.keyboard.up") => i.saturating_sub(1),
                    (Some(i), _) => (i + 1).min(shown.len() - 1),
                };
                s.selected = Some(shown[next].clone());
                reselect_into_view(s, visible);
            }
        }
        "key.keyboard.page.up" => s.first_row = s.first_row.saturating_sub(visible),
        "key.keyboard.page.down" => {
            s.first_row += visible;
            clamp_scroll(s, window);
        }
        _ => {}
    }
    true
}

pub fn pressed(window: Rect, mouse: [f32; 2], button_: i32) {
    if button_ != 0 {
        return;
    }
    let Ok(mut ui) = UI.lock() else { return };
    let Ui::Open(s) = &mut *ui else { return };
    let shown: Vec<String> = s.shown().into_iter().cloned().collect();
    let l = Layout::new(window[2], window[3], shown.len());
    if inside(l.cancel, mouse) {
        close();
        return;
    }
    if inside(l.search, mouse) {
        if let Some(b) = &s.selected {
            search(b);
        }
        return;
    }
    if inside(l.sort, mouse) {
        s.sort = if s.sort == Sort::Name {
            Sort::Source
        } else {
            Sort::Name
        };
        let visible = l.visible;
        reselect_into_view(s, visible);
        return;
    }
    if let Some(t) = l.track
        && inside([t[0] - 2.0, t[1], t[2] + 4.0, t[3]], mouse)
    {
        s.dragging = true;
        s.first_row = l.first_at(mouse[1]);
        return;
    }
    let clicked = shown.iter().enumerate().find_map(|(i, b)| {
        l.row(i, s.first_row)
            .filter(|r| inside(*r, mouse))
            .map(|_| b.clone())
    });
    if let Some(b) = clicked {
        let now = Instant::now();
        let double = s.last_click.as_ref().is_some_and(|(last, at)| {
            *last == b && now.duration_since(*at) < Duration::from_millis(400)
        });
        s.selected = Some(b.clone());
        s.last_click = Some((b.clone(), now));
        if double {
            search(&b);
        }
    }
}

pub fn released() {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open(s) = &mut *ui
    {
        s.dragging = false;
    }
}

pub fn dragged(window: Rect, mouse: [f32; 2]) {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open(s) = &mut *ui
        && s.dragging
    {
        s.first_row = Layout::new(window[2], window[3], s.shown().len()).first_at(mouse[1]);
    }
}

pub fn scrolled(window: Rect, by: f64) {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open(s) = &mut *ui
    {
        if by > 0.0 {
            s.first_row = s.first_row.saturating_sub(3);
        } else if by < 0.0 {
            s.first_row += 3;
            clamp_scroll(s, window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        Screen::new(
            vec![
                "minecraft:plains".into(),
                "biomesoplenty:lavender_field".into(),
                "minecraft:stony_peaks".into(),
                "biomesoplenty:crag".into(),
            ],
            Some("minecraft:plains".into()),
        )
    }

    #[test]
    fn the_list_filters_and_sorts() {
        let mut s = screen();
        assert_eq!(
            s.shown(),
            [
                "biomesoplenty:crag",
                "biomesoplenty:lavender_field",
                "minecraft:plains",
                "minecraft:stony_peaks"
            ]
        );
        s.sort = Sort::Source;
        assert_eq!(
            s.shown(),
            [
                "minecraft:plains",
                "minecraft:stony_peaks",
                "biomesoplenty:crag",
                "biomesoplenty:lavender_field"
            ]
        );
        s.query = "plenty".into();
        assert_eq!(s.shown().len(), 2);
        s.query = "PEAK".into();
        assert_eq!(s.shown(), ["minecraft:stony_peaks"]);
    }

    #[test]
    fn the_open_message_round_trips() {
        let biomes = vec![
            "minecraft:plains".to_owned(),
            "biomesoplenty:crag".to_owned(),
        ];
        assert_eq!(
            decode_open(&encode_open(&biomes, Some("minecraft:plains"))),
            Some((biomes.clone(), Some("minecraft:plains".into())))
        );
        assert_eq!(
            decode_open(&encode_open(&biomes, None)),
            Some((biomes, None))
        );
    }

    #[test]
    fn rows_scroll_and_stay_in_the_list() {
        let l = Layout::new(427.0, 240.0, 120);
        assert!(l.track.is_some());
        assert!(l.row(0, 0).is_some() && l.row(l.visible, 0).is_none());
        let end = l.max_first();
        assert!(l.row(119, end).is_some());
        for i in end..120 {
            let r = l.row(i, end).unwrap();
            assert!(r[1] + r[3] <= l.list[1] + l.list[3] + 0.01);
        }
        assert!(l.search[1] > l.list[1] + l.list[3]);
        let t = l.track.unwrap();
        assert_eq!(l.first_at(t[1]), 0);
        assert_eq!(l.first_at(t[1] + t[3]), end);
    }
}
