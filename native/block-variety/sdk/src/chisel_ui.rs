//! The chisel's screen, the mod's own (not a chest): the blocks of the group of what is in your
//! other hand, as icons in a grid that scrolls, with a search field. Click one and the whole
//! stack becomes it; right-click and one block of it does. It stays open, so you can go on.
//!
//! The server says what is held (`OPEN`: "id\ncount", count 0 to close); the client says what to
//! make of it (`CHOOSE`: "target\nwhole" or "target\none").

use std::sync::Mutex;

use mod_api::{DrawContext, Host};

use crate::names::NAMES;

pub const MOD_ID: &str = "block_variety_native";
pub const OPEN: &str = "block_variety_native:chisel_open";
pub const CHOOSE: &str = "block_variety_native:chisel";
const FILTER_MAX: usize = 32;

// ---- Protocol ----

pub fn encode_open(held: &str, count: u32) -> Vec<u8> {
    format!("{held}\n{count}").into_bytes()
}

pub fn decode_open(data: &[u8]) -> Option<(String, u32)> {
    let s = std::str::from_utf8(data).ok()?;
    let (held, count) = s.split_once('\n')?;
    Some((held.to_owned(), count.parse().ok()?))
}

pub fn encode_choose(target: &str, whole: bool) -> Vec<u8> {
    format!("{target}\n{}", if whole { "whole" } else { "one" }).into_bytes()
}

pub fn decode_choose(data: &[u8]) -> Option<(String, bool)> {
    let s = std::str::from_utf8(data).ok()?;
    let (target, how) = s.split_once('\n')?;
    let whole = match how {
        "whole" => true,
        "one" => false,
        _ => return None,
    };
    Some((target.to_owned(), whole))
}

/// What a block is called, as its item is in English (its id when it isn't known).
pub fn name(id: &str) -> &str {
    match NAMES.binary_search_by(|(i, _)| (*i).cmp(id)) {
        Ok(k) => NAMES[k].1,
        Err(_) => id,
    }
}

// ---- State ----

#[derive(Debug, Default)]
pub enum Ui {
    #[default]
    Closed,
    Open {
        /// The block in the other hand, and how many.
        held: String,
        count: u32,
        /// Its group's blocks that have an item.
        entries: Vec<&'static str>,
        /// What is typed: only blocks whose name has it are shown.
        filter: String,
        first_row: usize,
        /// Whether the scroll bar's thumb is held.
        dragging: bool,
    },
}

pub static UI: Mutex<Ui> = Mutex::new(Ui::Closed);

/// What the input hooks (which can't send) leave for the client tick.
pub static OUTBOX: Mutex<Vec<(&'static str, Vec<u8>)>> = Mutex::new(Vec::new());

fn send(channel: &'static str, data: Vec<u8>) {
    if let Ok(mut out) = OUTBOX.lock() {
        out.push((channel, data));
    }
}

/// The server said what is held: the screen shows it, keeping the search and scroll when it was
/// already open on the same group. Whether there is something to show.
pub fn set_held(held: String, count: u32) -> bool {
    let Ok(mut ui) = UI.lock() else { return false };
    let group = crate::group_of(&held);
    let (Some(group), true) = (group, count > 0) else {
        *ui = Ui::Closed;
        return false;
    };
    let entries = crate::entries(group);
    match &mut *ui {
        Ui::Open {
            held: h,
            count: c,
            entries: e,
            ..
        } if *e == entries => {
            *h = held;
            *c = count;
        }
        _ => {
            *ui = Ui::Open {
                held,
                count,
                entries,
                filter: String::new(),
                first_row: 0,
                dragging: false,
            }
        }
    }
    true
}

/// The entries the search lets through.
fn shown(entries: &[&'static str], filter: &str) -> Vec<&'static str> {
    if filter.is_empty() {
        return entries.to_vec();
    }
    let f = filter.to_lowercase();
    entries
        .iter()
        .copied()
        .filter(|b| name(b).to_lowercase().contains(&f) || b.contains(&f))
        .collect()
}

// ---- Layout ----

type Rect = [f32; 4];

fn inside(r: Rect, p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

const TILE: f32 = 24.0;
const GAP: f32 = 2.0;
const PAD: f32 = 10.0;
const HEADER: f32 = 50.0;
const FOOTER: f32 = 20.0;
const BAR_W: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub panel: Rect,
    pub field: Rect,
    pub view: Rect,
    pub cols: usize,
    pub rows: usize,
    pub visible_rows: usize,
    pub track: Option<Rect>,
}

impl Layout {
    /// For a window `w` by `h` GUI pixels and `count` blocks shown.
    pub fn new(w: f32, h: f32, count: usize) -> Self {
        let pw = (w - 24.0).clamp(
            8.0 * (TILE + GAP) + 2.0 * PAD + BAR_W,
            14.0 * (TILE + GAP) + 2.0 * PAD + BAR_W + GAP,
        );
        let ph = (h - 24.0).clamp(HEADER + FOOTER + 2.0 * (TILE + GAP) + PAD, 300.0);
        let panel = [((w - pw) / 2.0).floor(), ((h - ph) / 2.0).floor(), pw, ph];
        let field = [panel[0] + PAD, panel[1] + 28.0, pw - 2.0 * PAD, 14.0];
        let view = [
            panel[0] + PAD,
            panel[1] + HEADER,
            pw - 2.0 * PAD - BAR_W - GAP,
            ph - HEADER - FOOTER,
        ];
        let cols = (((view[2] + GAP) / (TILE + GAP)).floor() as usize).max(1);
        let rows = count.div_ceil(cols);
        let visible_rows = (((view[3] + GAP) / (TILE + GAP)).floor() as usize).max(1);
        let track = (rows > visible_rows).then(|| {
            [
                view[0] + view[2] + GAP,
                view[1],
                BAR_W,
                visible_rows as f32 * (TILE + GAP) - GAP,
            ]
        });
        Self {
            panel,
            field,
            view,
            cols,
            rows,
            visible_rows,
            track,
        }
    }

    pub fn max_first_row(&self) -> usize {
        self.rows.saturating_sub(self.visible_rows)
    }

    /// Where tile `index` is, when its row is in view. The grid is centred in the view.
    pub fn tile(&self, index: usize, first_row: usize) -> Option<Rect> {
        let (row, col) = (index / self.cols, index % self.cols);
        if row < first_row || row >= first_row + self.visible_rows {
            return None;
        }
        let used = self.cols as f32 * (TILE + GAP) - GAP;
        let x0 = self.view[0] + ((self.view[2] - used) / 2.0).floor();
        Some([
            x0 + col as f32 * (TILE + GAP),
            self.view[1] + (row - first_row) as f32 * (TILE + GAP),
            TILE,
            TILE,
        ])
    }

    pub fn thumb(&self, first_row: usize) -> Option<Rect> {
        let track = self.track?;
        let h = (track[3] * self.visible_rows as f32 / self.rows as f32).max(10.0);
        let y = track[1] + (track[3] - h) * first_row as f32 / self.max_first_row().max(1) as f32;
        Some([track[0], y, track[2], h])
    }

    pub fn row_at(&self, y: f32) -> usize {
        let Some(track) = self.track else { return 0 };
        let t = ((y - track[1]) / track[3]).clamp(0.0, 1.0);
        ((t * self.max_first_row() as f32).round() as usize).min(self.max_first_row())
    }
}

// ---- Drawing ----

const SHADE: u32 = 0x90_00_00_00;
const PANEL: u32 = 0xF0_14_16_1A;
const EDGE: u32 = 0xFF_7A_86_99;
const TILE_FILL: u32 = 0xFF_26_2A_31;
const TILE_HOVER: u32 = 0xFF_3B_42_4D;
const TILE_EDGE: u32 = 0xFF_3A_40_4A;
const GOLD_EDGE: u32 = 0xFF_E0_B0_4A;
const WHITE: u32 = 0xFF_FF_FF_FF;
const GREY: u32 = 0xFF_A0_A8_B4;
const DIM: u32 = 0xFF_60_66_70;
const GOLD: u32 = 0xFF_E0_B0_4A;

fn frame(draw: &mut dyn DrawContext, r: Rect, fill: u32, edge: u32) {
    draw.fill([r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], edge);
    draw.fill(r, fill);
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
    let Ui::Open {
        held,
        count,
        entries,
        filter,
        first_row,
        ..
    } = &*ui
    else {
        return;
    };
    let list = shown(entries, filter);
    let l = Layout::new(window[2], window[3], list.len());
    let [x, y, w, h] = l.panel;
    draw.fill(window, SHADE);
    frame(draw, l.panel, PANEL, EDGE);

    // What is held, and how many there are to pick from.
    if let Some(key) = super::item_key(host, held) {
        draw.item(key, [x + PAD, y + 6.0], 16.0);
    }
    let total = if filter.is_empty() {
        format!("{} blocks", entries.len())
    } else {
        format!("{} of {}", list.len(), entries.len())
    };
    let tw = draw.text_width(&total);
    draw.text(&total, [x + w - PAD - tw, y + 11.0], GREY, false);
    let title = fit(
        draw,
        &format!("{}  x{count}", name(held)),
        w - 2.0 * PAD - 26.0 - tw - 8.0,
    );
    draw.text(&title, [x + PAD + 22.0, y + 11.0], WHITE, true);

    // The search field.
    frame(
        draw,
        l.field,
        0xFF_0A_0B_0D,
        if filter.is_empty() { TILE_EDGE } else { EDGE },
    );
    let at = [l.field[0] + 4.0, l.field[1] + 3.0];
    let cursor = if blink { "_" } else { "" };
    if filter.is_empty() {
        draw.text("Type to search", at, DIM, false);
    } else {
        draw.text(
            &format!("{}{cursor}", fit(draw, filter, l.field[2] - 14.0)),
            at,
            WHITE,
            false,
        );
    }

    let mut hovered = None;
    for (i, b) in list.iter().enumerate() {
        let Some(r) = l.tile(i, *first_row) else {
            continue;
        };
        let over = inside(r, mouse);
        if over {
            hovered = Some(*b);
        }
        let edge = if *b == held.as_str() {
            GOLD_EDGE
        } else if over {
            EDGE
        } else {
            TILE_EDGE
        };
        frame(draw, r, if over { TILE_HOVER } else { TILE_FILL }, edge);
        if let Some(key) = super::item_key(host, b) {
            draw.item(key, [r[0] + 4.0, r[1] + 4.0], 16.0);
        }
    }
    if list.is_empty() {
        draw.text(
            "No block of this group has that in its name",
            [l.view[0], l.view[1] + 4.0],
            GREY,
            false,
        );
    }
    if let (Some(track), Some(thumb)) = (l.track, l.thumb(*first_row)) {
        draw.fill(track, 0xFF_0A_0B_0D);
        draw.fill(
            thumb,
            if inside(thumb, mouse) {
                0xFF_B0_BA_C8
            } else {
                EDGE
            },
        );
    }

    let fy = y + h - FOOTER + 6.0;
    match hovered {
        Some(b) if b == held.as_str() => {
            draw.text(
                &fit(draw, &format!("{} (held)", name(b)), w - 2.0 * PAD),
                [x + PAD, fy],
                GOLD,
                false,
            );
        }
        Some(b) => draw.text(
            &fit(draw, name(b), w - 2.0 * PAD),
            [x + PAD, fy],
            WHITE,
            false,
        ),
        None => {
            let hint = "Click: the whole stack   Right-click: one";
            draw.text(&fit(draw, hint, w - 2.0 * PAD), [x + PAD, fy], GREY, false);
        }
    }
}

// ---- Input ----

pub fn typed(c: char) {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open {
            filter, first_row, ..
        } = &mut *ui
        && !c.is_control()
        && filter.chars().count() < FILTER_MAX
    {
        filter.push(c);
        *first_row = 0;
    }
}

/// A key (Escape is not passed here: it closes the screen as on any).
pub fn key(name: &str) {
    let Ok(mut ui) = UI.lock() else { return };
    let Ui::Open {
        held,
        entries,
        filter,
        first_row,
        ..
    } = &mut *ui
    else {
        return;
    };
    let list = shown(entries, filter);
    let max = Layout::new(1.0e4, 1.0e4, list.len()).max_first_row();
    match name {
        "key.keyboard.backspace" => {
            filter.pop();
            *first_row = 0;
        }
        "key.keyboard.up" | "key.keyboard.page.up" => *first_row = first_row.saturating_sub(1),
        "key.keyboard.down" | "key.keyboard.page.down" => *first_row = (*first_row + 1).min(max),
        // Enter: the first block the search shows, when it isn't what is held.
        "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
            if let Some(b) = list.iter().find(|b| **b != held.as_str()) {
                send(CHOOSE, encode_choose(b, true));
            }
        }
        _ => {}
    }
}

/// A click: left for the whole stack, right for one.
pub fn pressed(window: Rect, mouse: [f32; 2], button: i32) {
    if button != 0 && button != 1 {
        return;
    }
    let Ok(mut ui) = UI.lock() else { return };
    let Ui::Open {
        held,
        entries,
        filter,
        first_row,
        dragging,
        ..
    } = &mut *ui
    else {
        return;
    };
    let list = shown(entries, filter);
    let l = Layout::new(window[2], window[3], list.len());
    if button == 0
        && let Some(track) = l.track
        && inside([track[0] - 2.0, track[1], track[2] + 4.0, track[3]], mouse)
    {
        *dragging = true;
        *first_row = l.row_at(mouse[1]);
        return;
    }
    let chosen = list.iter().enumerate().find_map(|(i, b)| {
        l.tile(i, *first_row)
            .filter(|r| inside(*r, mouse))
            .map(|_| *b)
    });
    if let Some(b) = chosen
        && b != held.as_str()
    {
        send(CHOOSE, encode_choose(b, button == 0));
    }
}

pub fn released() {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open { dragging, .. } = &mut *ui
    {
        *dragging = false;
    }
}

pub fn dragged(window: Rect, mouse: [f32; 2]) {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open {
            entries,
            filter,
            first_row,
            dragging: true,
            ..
        } = &mut *ui
    {
        *first_row =
            Layout::new(window[2], window[3], shown(entries, filter).len()).row_at(mouse[1]);
    }
}

pub fn scrolled(window: Rect, by: f64) {
    if let Ok(mut ui) = UI.lock()
        && let Ui::Open {
            entries,
            filter,
            first_row,
            ..
        } = &mut *ui
    {
        let max = Layout::new(window[2], window[3], shown(entries, filter).len()).max_first_row();
        if by > 0.0 {
            *first_row = first_row.saturating_sub(1);
        } else if by < 0.0 {
            *first_row = (*first_row + 1).min(max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sorted_and_found() {
        assert!(
            NAMES.windows(2).all(|w| w[0].0 < w[1].0),
            "NAMES must be sorted by id for the search"
        );
        assert_eq!(name("minecraft:crying_obsidian"), "Crying Obsidian");
        assert_ne!(name("block_variety:obsidian_1"), "block_variety:obsidian_1");
        assert_eq!(name("nope:nothing"), "nope:nothing");
    }

    #[test]
    fn messages_go_both_ways() {
        assert_eq!(
            decode_open(&encode_open("minecraft:stone", 64)),
            Some(("minecraft:stone".into(), 64))
        );
        assert_eq!(
            decode_choose(&encode_choose("block_variety:stone_3", false)),
            Some(("block_variety:stone_3".into(), false))
        );
        assert_eq!(decode_choose(b"x\nall"), None);
        assert_eq!(decode_open(b"x"), None);
    }

    #[test]
    fn the_grid_scrolls_and_stays_in_the_panel() {
        let few = Layout::new(427.0, 240.0, 5);
        assert!(few.track.is_none());
        let many = Layout::new(427.0, 240.0, 400);
        assert!(many.track.is_some() && many.rows > many.visible_rows);
        let end = many.max_first_row();
        assert!(many.tile(399, end).is_some());
        assert!(many.tile(0, end).is_none());
        for i in 0..400 {
            if let Some(r) = many.tile(i, 0) {
                assert!(r[0] >= many.view[0] && r[0] + r[2] <= many.view[0] + many.view[2] + 0.01);
                assert!(r[1] + r[3] <= many.view[1] + many.view[3] + 0.01);
            }
        }
        let track = many.track.unwrap();
        assert_eq!(many.row_at(track[1]), 0);
        assert_eq!(many.row_at(track[1] + track[3]), end);
        // A small window still fits the panel.
        let small = Layout::new(320.0, 180.0, 40);
        assert!(small.panel[0] >= 0.0 && small.panel[1] >= 0.0);
        assert!(small.panel[1] + small.panel[3] <= 180.0);
    }

    #[test]
    fn searching_and_clicking_sends_what_to_make() {
        assert!(set_held("minecraft:stone".into(), 32));
        typed('s');
        typed('m');
        let (entries, filter) = match &*UI.lock().unwrap() {
            Ui::Open {
                entries, filter, ..
            } => (entries.clone(), filter.clone()),
            Ui::Closed => panic!("open"),
        };
        let list = shown(&entries, &filter);
        assert!(!list.is_empty() && list.len() < entries.len());
        assert!(
            list.iter()
                .all(|b| name(b).to_lowercase().contains("sm") || b.contains("sm"))
        );
        OUTBOX.lock().unwrap().clear();
        let (w, h) = (427.0, 240.0);
        let l = Layout::new(w, h, list.len());
        let k = list.iter().position(|b| *b != "minecraft:stone").unwrap();
        let r = l.tile(k, 0).unwrap();
        pressed([0.0, 0.0, w, h], [r[0] + 2.0, r[1] + 2.0], 1);
        let out = std::mem::take(&mut *OUTBOX.lock().unwrap());
        assert_eq!(out.len(), 1);
        assert_eq!(decode_choose(&out[0].1), Some((list[k].to_owned(), false)));
        // The server's answer keeps the search.
        assert!(set_held(list[k].to_owned(), 31));
        assert!(
            matches!(&*UI.lock().unwrap(), Ui::Open { filter, count: 31, .. } if filter == "sm")
        );
        // Nothing left: closed.
        assert!(!set_held(list[k].to_owned(), 0));
        assert!(matches!(&*UI.lock().unwrap(), Ui::Closed));
    }
}
