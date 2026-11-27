//! Joon's screen on the client: the mod's own screen (as Yukari Gaps' and Block Variety's), drawn
//! in the old GUI's grey. Her owner gets two tabs, Trades (her trades, set from their own items)
//! and Send (who to send her to); the player she visits gets her trade offer. What it shows comes
//! from the server (`net::JoonView`); what is done on it goes back as `net::JoonAct`, sent on the
//! next client tick.

use std::sync::Mutex;

use mod_api::{ClientHost, DrawContext, Text};

use crate::joon::{MAX_TRADES, clock};
use crate::net::{JoonAct, JoonView, Shown, ShownTrade};
use crate::stacks::DollStack;

pub(crate) const MOD_ID: &str = "alice_dolls_native";

type Rect = [f32; 4];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Trades,
    Send,
}

#[derive(Debug, Default)]
struct Ui {
    /// What the server last sent; None when there is no screen.
    view: Option<JoonView>,
    /// The screen is up.
    open: bool,
    /// The server asked for it; it opens on the next tick.
    want_open: bool,
    /// The server closed it (or it was left for her stock).
    want_close: bool,
    tab: Tab,
    /// The trade being edited, and which of its parts (0 and 1 what is paid, 2 what is got).
    chosen: usize,
    part: u8,
    /// First rows shown of each list.
    trades_top: usize,
    players_top: usize,
    offer_top: usize,
    outbox: Vec<JoonAct>,
}

static UI: Mutex<Option<Ui>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let mut g = UI.lock().ok()?;
    Some(f(g.get_or_insert_with(Ui::default)))
}

// ---- What the server says, and the tick ----

pub(crate) fn receive(data: &[u8]) {
    let Ok(v) = crate::net::decode_joon(data) else { return };
    with(|ui| match v {
        Some(v) => {
            if ui.view.is_none() {
                // A fresh screen: from the top.
                *ui = Ui { outbox: std::mem::take(&mut ui.outbox), ..Ui::default() };
                ui.want_open = true;
            }
            ui.view = Some(v);
        }
        None => {
            ui.view = None;
            ui.want_close = ui.open;
        }
    });
}

pub(crate) fn ours(host: &dyn ClientHost, id: &str) -> bool {
    id == "mod" && host.mod_screen() == Some(MOD_ID)
}

/// Sends what was done, and opens and closes the screen as the server says.
pub(crate) fn tick(host: &mut dyn ClientHost) {
    let Some((out, open, close)) = with(|ui| (std::mem::take(&mut ui.outbox), std::mem::take(&mut ui.want_open), std::mem::take(&mut ui.want_close)))
    else {
        return;
    };
    for a in out {
        let _ = host.send_payload_to_server(&mod_api::PayloadId(crate::net::JOON_ACT.into()), mod_api::PayloadBytes(crate::net::encode_joon_act(a)));
    }
    if close {
        host.mod_screen_close(MOD_ID);
        with(|ui| {
            ui.open = false;
            ui.view = None;
        });
        return;
    }
    if open {
        let up = host.mod_screen() == Some(MOD_ID) || host.mod_screen_open(MOD_ID);
        with(|ui| {
            ui.open = up;
            if !up {
                // Another screen is up: she is told it closed.
                ui.view = None;
                ui.outbox.push(JoonAct::Close);
            }
        });
        return;
    }
    let shut = host.mod_screen() != Some(MOD_ID);
    let closed_by_player = with(|ui| {
        let gone = ui.open && shut;
        if gone {
            ui.open = false;
            ui.view = None;
        }
        gone
    });
    if closed_by_player == Some(true) {
        let _ = host.send_payload_to_server(&mod_api::PayloadId(crate::net::JOON_ACT.into()), mod_api::PayloadBytes(crate::net::encode_joon_act(JoonAct::Close)));
    }
}

// ---- Layout ----

const PW: f32 = 256.0;
const PH: f32 = 222.0;

const PANEL: u32 = 0xFFC6_C6C6;
const LABEL: u32 = 0xFF40_4040;
const DIM: u32 = 0xFF70_7070;
const RED: u32 = 0xFFAA_0000;
const GREEN: u32 = 0xFF00_6A00;

/// Rows shown at once.
const TRADE_ROWS: usize = 5;
const PLAYER_ROWS: usize = 7;
const OFFER_ROWS: usize = 6;

/// What a click lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Tab(Tab),
    Row(usize),
    Part(u8),
    Minus,
    Plus,
    Clear,
    Delete,
    Stock,
    Item(u8),
    Player(usize),
    Offer(usize),
    Home,
    TradesList,
    PlayersList,
    OffersList,
}

/// The panel's top left in the window, with room above it for the tabs.
fn origin(window: Rect) -> [f32; 2] {
    let x = (window[0] + (window[2] - PW) / 2.0).floor();
    let y = (window[1] + (window[3] - PH) / 2.0 + 9.0).floor().max(window[1] + 20.0);
    [x, y]
}

fn at(o: [f32; 2], x: f32, y: f32, w: f32, h: f32) -> Rect {
    [o[0] + x, o[1] + y, w, h]
}

fn inside(r: Rect, p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

fn tab_rect(o: [f32; 2], t: Tab) -> Rect {
    let i = if t == Tab::Trades { 0.0 } else { 1.0 };
    at(o, 4.0 + 66.0 * i, -19.0, 64.0, 21.0)
}

fn row_rect(o: [f32; 2], r: usize) -> Rect {
    at(o, 8.0, 30.0 + 18.0 * r as f32, 116.0, 18.0)
}

/// A trade part's slot on the Trades tab.
fn part_rect(o: [f32; 2], part: u8) -> Rect {
    match part {
        0 => at(o, 140.0, 31.0, 18.0, 18.0),
        1 => at(o, 160.0, 31.0, 18.0, 18.0),
        _ => at(o, 210.0, 31.0, 18.0, 18.0),
    }
}

/// The player's slot `i` (`JoonView::items`): their inventory as their own screen lays it out,
/// the hotbar under it and the off hand beside it.
fn item_rect(o: [f32; 2], i: usize) -> Rect {
    match i {
        0..9 => at(o, 8.0 + 18.0 * i as f32, 190.0, 18.0, 18.0),
        9..36 => at(o, 8.0 + 18.0 * ((i - 9) % 9) as f32, 132.0 + 18.0 * ((i - 9) / 9) as f32, 18.0, 18.0),
        _ => at(o, 176.0, 190.0, 18.0, 18.0),
    }
}

fn player_rect(o: [f32; 2], r: usize) -> Rect {
    at(o, 8.0, 30.0 + 24.0 * r as f32, 232.0, 22.0)
}

fn offer_rect(o: [f32; 2], r: usize) -> Rect {
    at(o, 8.0, 30.0 + 26.0 * r as f32, 232.0, 24.0)
}

fn offer_button(row: Rect) -> Rect {
    [row[0] + row[2] - 82.0, row[1] + 4.0, 78.0, 16.0]
}

/// The trades the owner's list shows: hers, and a row for a new one while there is room.
fn trade_rows(v: &JoonView) -> usize {
    (v.trades.len() + 1).min(MAX_TRADES)
}

fn any_set(v: &JoonView) -> bool {
    v.trades.iter().any(|t| t.parts.iter().any(Option::is_some))
}

fn hits(v: &JoonView, ui: &Ui, o: [f32; 2]) -> Vec<(Rect, Hit)> {
    let mut out = Vec::new();
    if v.visitor {
        for r in 0..OFFER_ROWS {
            let i = ui.offer_top + r;
            if i < v.trades.len() {
                out.push((offer_button(offer_rect(o, r)), Hit::Offer(i)));
            }
        }
        out.push((at(o, 8.0, 30.0, 232.0, 26.0 * OFFER_ROWS as f32), Hit::OffersList));
        out.push((at(o, 8.0, 196.0, 232.0, 20.0), Hit::Home));
        return out;
    }
    out.push((tab_rect(o, Tab::Trades), Hit::Tab(Tab::Trades)));
    out.push((tab_rect(o, Tab::Send), Hit::Tab(Tab::Send)));
    match ui.tab {
        Tab::Trades => {
            for r in 0..TRADE_ROWS {
                let i = ui.trades_top + r;
                if i < trade_rows(v) {
                    out.push((row_rect(o, r), Hit::Row(i)));
                }
            }
            out.push((at(o, 8.0, 30.0, 122.0, 18.0 * TRADE_ROWS as f32), Hit::TradesList));
            for p in 0..3 {
                out.push((part_rect(o, p), Hit::Part(p)));
            }
            out.push((at(o, 140.0, 56.0, 16.0, 16.0), Hit::Minus));
            out.push((at(o, 192.0, 56.0, 16.0, 16.0), Hit::Plus));
            out.push((at(o, 212.0, 56.0, 36.0, 16.0), Hit::Clear));
            out.push((at(o, 140.0, 76.0, 108.0, 16.0), Hit::Delete));
            out.push((at(o, 176.0, 132.0, 72.0, 20.0), Hit::Stock));
            for i in 0..v.items.len().min(37) {
                out.push((item_rect(o, i), Hit::Item(i as u8)));
            }
        }
        Tab::Send => {
            for r in 0..PLAYER_ROWS {
                let i = ui.players_top + r;
                if i < v.players.len() {
                    out.push((player_rect(o, r), Hit::Player(i)));
                }
            }
            out.push((at(o, 8.0, 30.0, 240.0, 24.0 * PLAYER_ROWS as f32), Hit::PlayersList));
        }
    }
    out
}

fn hit_at(v: &JoonView, ui: &Ui, o: [f32; 2], mouse: [f32; 2]) -> Option<Hit> {
    hits(v, ui, o).into_iter().find(|(r, _)| inside(*r, mouse)).map(|(_, h)| h)
}

fn part_of(t: Option<&ShownTrade>, part: u8) -> Shown {
    t.and_then(|t| t.parts.get(usize::from(part)).cloned().flatten())
}

// ---- Input ----

pub(crate) fn pressed(window: Rect, mouse: [f32; 2], button: i32, shift: bool) {
    if !matches!(button, 0 | 1) {
        return;
    }
    let right = button == 1;
    with(|ui| {
        let Some(v) = ui.view.clone() else { return };
        let o = origin(window);
        let Some(hit) = hit_at(&v, ui, o, mouse) else { return };
        let t = v.trades.get(ui.chosen);
        let trade = ui.chosen.min(MAX_TRADES - 1) as u8;
        let count = |part: u8| part_of(t, part).map_or(0, |(_, n)| n);
        let act = match hit {
            Hit::Tab(tab) => {
                ui.tab = tab;
                None
            }
            Hit::Row(i) => {
                ui.chosen = i;
                ui.part = 0;
                None
            }
            Hit::Part(p) if right => Some(JoonAct::Count { trade, part: p, count: 0 }).filter(|_| count(p) > 0),
            Hit::Part(p) => {
                ui.part = p;
                None
            }
            Hit::Minus | Hit::Plus if count(ui.part) > 0 => {
                let step = if shift { 8 } else { 1 };
                let n = count(ui.part);
                let n = if hit == Hit::Plus { n.saturating_add(step).min(64) } else { n.saturating_sub(step).max(1) };
                Some(JoonAct::Count { trade, part: ui.part, count: n as u8 })
            }
            Hit::Clear if count(ui.part) > 0 => Some(JoonAct::Count { trade, part: ui.part, count: 0 }),
            Hit::Delete if ui.chosen < v.trades.len() => Some(JoonAct::Delete { trade }),
            Hit::Stock => {
                ui.want_close = true;
                Some(JoonAct::Stock)
            }
            Hit::Item(slot) if v.items.get(usize::from(slot)).is_some_and(Option::is_some) => {
                let act = JoonAct::Set { trade, part: ui.part, slot, one: right };
                // On to the next empty part, as filling a trade in goes.
                if let Some(next) = (0..3u8).find(|&p| p != ui.part && part_of(t, p).is_none()) {
                    ui.part = next;
                }
                Some(act)
            }
            Hit::Player(i) => v
                .players
                .get(i)
                .filter(|p| p.wait == 0 && any_set(&v))
                .map(|p| JoonAct::Send { player: p.uuid }),
            Hit::Offer(i) => v
                .trades
                .get(i)
                .filter(|t| t.affordable && t.stock > 0)
                .map(|_| JoonAct::Trade { trade: i as u8, many: shift }),
            Hit::Home => Some(JoonAct::Home),
            _ => None,
        };
        if let Some(a) = act {
            ui.outbox.push(a);
        }
    });
}

pub(crate) fn scrolled(window: Rect, mouse: [f32; 2], by: f64) {
    if by == 0.0 {
        return;
    }
    let up = by > 0.0;
    with(|ui| {
        let Some(v) = ui.view.clone() else { return };
        let o = origin(window);
        let roll = |top: &mut usize, len: usize, shown: usize| {
            let max = len.saturating_sub(shown);
            *top = if up { top.saturating_sub(1) } else { (*top + 1).min(max) };
        };
        // Over a part's slot the wheel counts it up and down.
        let over_part = (0..3u8).find(|&p| !v.visitor && ui.tab == Tab::Trades && inside(part_rect(o, p), mouse));
        if let Some(p) = over_part {
            let n = part_of(v.trades.get(ui.chosen), p).map_or(0, |(_, n)| n);
            if n > 0 {
                let n = if up { (n + 1).min(64) } else { n.saturating_sub(1).max(1) };
                ui.outbox.push(JoonAct::Count { trade: ui.chosen.min(MAX_TRADES - 1) as u8, part: p, count: n as u8 });
            }
            return;
        }
        if v.visitor {
            roll(&mut ui.offer_top, v.trades.len(), OFFER_ROWS);
        } else if ui.tab == Tab::Trades {
            roll(&mut ui.trades_top, trade_rows(&v), TRADE_ROWS);
        } else {
            roll(&mut ui.players_top, v.players.len(), PLAYER_ROWS);
        }
    });
}

// ---- Drawing ----

/// The old GUI's panel: a black rim, white light on the top left, dark grey shade on the bottom
/// right.
fn panel(d: &mut dyn DrawContext, r: Rect) {
    let [x, y, w, h] = r;
    d.fill([x + 1.0, y, w - 2.0, h], 0xFF00_0000);
    d.fill([x, y + 1.0, w, h - 2.0], 0xFF00_0000);
    d.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], 0xFFFF_FFFF);
    d.fill([x + 3.0, y + 3.0, w - 4.0, h - 4.0], 0xFF55_5555);
    d.fill([x + 3.0, y + 3.0, w - 6.0, h - 6.0], PANEL);
}

/// A slot: sunk in, dark on the top left and light on the bottom right.
fn slot(d: &mut dyn DrawContext, r: Rect) {
    let [x, y, w, h] = r;
    d.fill([x, y, w - 1.0, h - 1.0], 0xFF37_3737);
    d.fill([x + 1.0, y + 1.0, w - 1.0, h - 1.0], 0xFFFF_FFFF);
    d.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], 0xFF8B_8B8B);
}

/// The old stone button: grey, blue under the mouse, dark when it can't be pressed.
fn button(d: &mut dyn DrawContext, r: Rect, label: &str, mouse: [f32; 2], enabled: bool) {
    let hover = enabled && inside(r, mouse);
    let (face, light, shade, text) = match (enabled, hover) {
        (false, _) => (0xFF2C_2C2C, 0xFF3C_3C3C, 0xFF20_2020, 0xFFA0_A0A0),
        (true, true) => (0xFF7E_88BF, 0xFFBC_C4F0, 0xFF4E_568F, 0xFFFF_FFA0),
        (true, false) => (0xFF6F_6F6F, 0xFFAA_AAAA, 0xFF3F_3F3F, 0xFFE0_E0E0),
    };
    let [x, y, w, h] = r;
    d.fill(r, 0xFF00_0000);
    d.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], shade);
    d.fill([x + 1.0, y + 1.0, w - 3.0, h - 3.0], light);
    d.fill([x + 2.0, y + 2.0, w - 4.0, h - 4.0], face);
    let label = fit(d, label, w - 6.0);
    let tw = d.text_width(&label);
    d.text(&label, [(x + (w - tw) / 2.0).floor(), (y + (h - 8.0) / 2.0).floor()], text, true);
}

fn fit(d: &dyn DrawContext, text: &str, width: f32) -> String {
    if d.text_width(text) <= width {
        return text.to_owned();
    }
    let mut cut: String = text.to_owned();
    while !cut.is_empty() && d.text_width(&format!("{cut}...")) > width {
        cut.pop();
    }
    format!("{cut}...")
}

fn label(d: &mut dyn DrawContext, text: &str, at: [f32; 2], argb: u32) {
    d.text(text, at, argb, false);
}

fn right_label(d: &mut dyn DrawContext, text: &str, right: f32, y: f32, argb: u32) {
    let w = d.text_width(text);
    d.text(text, [right - w, y], argb, false);
}

fn scrollbar(d: &mut dyn DrawContext, track: Rect, top: usize, len: usize, shown: usize) {
    if len <= shown {
        return;
    }
    d.fill(track, 0xFF00_0000);
    let h = (track[3] * shown as f32 / len as f32).max(8.0);
    let y = track[1] + (track[3] - h) * top as f32 / (len - shown) as f32;
    d.fill([track[0], y, track[2], h], 0xFF80_8080);
    d.fill([track[0], y, track[2] - 1.0, h - 1.0], 0xFFC0_C0C0);
}

/// An arrow, pointing right, from `x` at mid-height `y`.
fn arrow(d: &mut dyn DrawContext, x: f32, y: f32) {
    d.fill([x, y - 1.0, 8.0, 3.0], LABEL);
    d.quad([[x + 7.0, y - 5.0], [x + 13.0, y + 0.5], [x + 13.0, y + 0.5], [x + 7.0, y + 6.0]], LABEL);
}

/// A villager's red cross, over a 16 by 16 item at `x`, `y`.
fn cross(d: &mut dyn DrawContext, x: f32, y: f32) {
    for (off, argb) in [(1.0, 0xFF55_0000), (0.0, 0xFFFF_3030)] {
        let (a, b) = (x + 2.0 + off, y + 2.0 + off);
        let (c, e) = (x + 14.0 + off, y + 14.0 + off);
        d.quad([[a + 1.0, b - 1.0], [c + 1.0, e - 1.0], [c - 1.0, e + 1.0], [a - 1.0, b + 1.0]], argb);
        d.quad([[c + 1.0, b + 1.0], [a + 1.0, e + 1.0], [a - 1.0, e - 1.0], [c - 1.0, b - 1.0]], argb);
    }
}

fn game_stack(host: &dyn ClientHost, s: &(String, u32)) -> Option<mod_api::ItemStack> {
    DollStack::new(s.0.clone(), s.1.max(1)).to_game(host)
}

/// An item with its count, and, under the mouse, what it is.
fn item(host: &dyn ClientHost, d: &mut dyn DrawContext, s: &Shown, x: f32, y: f32, mouse: [f32; 2], tip: &mut Option<String>) {
    let Some(s) = s else { return };
    let Some(stack) = game_stack(host, s) else { return };
    d.gui_draw_item_with(&stack, [x, y], None);
    if inside([x, y, 16.0, 16.0], mouse) {
        let name = host.hover_name(&stack);
        *tip = Some(if s.1 > 1 { format!("{name} x{}", s.1) } else { name });
    }
}

/// The three parts of trade `t` as a row: what is paid, an arrow, what is got.
fn trade_items(host: &dyn ClientHost, d: &mut dyn DrawContext, t: &ShownTrade, x: f32, y: f32, mouse: [f32; 2], tip: &mut Option<String>) {
    item(host, d, &t.parts[0], x, y, mouse, tip);
    item(host, d, &t.parts[1], x + 18.0, y, mouse, tip);
    arrow(d, x + 37.0, y + 8.0);
    item(host, d, &t.parts[2], x + 54.0, y, mouse, tip);
    if t.parts[2].is_some() && t.stock == 0 {
        cross(d, x + 54.0, y);
    }
}

pub(crate) fn draw(host: &dyn ClientHost, d: &mut dyn DrawContext, window: Rect, mouse: [f32; 2]) {
    let Some(Some((v, tab, chosen, part, tops))) = UI.lock().ok().map(|g| {
        g.as_ref().and_then(|ui| ui.view.clone().map(|v| (v, ui.tab, ui.chosen, ui.part, (ui.trades_top, ui.players_top, ui.offer_top))))
    }) else {
        return;
    };
    let o = origin(window);
    d.fill(window, 0x9010_1010);
    let mut tip = None;
    if v.visitor {
        draw_offer(host, d, &v, o, mouse, tops.2, &mut tip);
    } else {
        // The tab not chosen sits behind the panel, the chosen one is part of it.
        let other = if tab == Tab::Trades { Tab::Send } else { Tab::Trades };
        let tr = tab_rect(o, other);
        panel(d, [tr[0], tr[1] + 2.0, tr[2], tr[3]]);
        d.fill([tr[0] + 3.0, tr[1] + 5.0, tr[2] - 6.0, tr[3] - 6.0], 0xFFA0_A0A0);
        panel(d, [o[0], o[1], PW, PH]);
        let cr = tab_rect(o, tab);
        panel(d, cr);
        d.fill([cr[0] + 3.0, cr[1] + 3.0, cr[2] - 6.0, cr[3]], PANEL);
        for (t, name) in [(Tab::Trades, "Trades"), (Tab::Send, "Send"), ] {
            let r = tab_rect(o, t);
            let w = d.text_width(name);
            let y = if t == tab { r[1] + 7.0 } else { r[1] + 9.0 };
            label(d, name, [(r[0] + (r[2] - w) / 2.0).floor(), y], if t == tab { LABEL } else { DIM });
        }
        match tab {
            Tab::Trades => draw_trades(host, d, &v, o, mouse, chosen, part, tops.0, &mut tip),
            Tab::Send => draw_send(d, &v, o, mouse, tops.1),
        }
    }
    if let Some(tip) = tip {
        let _ = d.gui_tooltip(&[Text::literal(tip)], mouse);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_trades(host: &dyn ClientHost, d: &mut dyn DrawContext, v: &JoonView, o: [f32; 2], mouse: [f32; 2], chosen: usize, part: u8, top: usize, tip: &mut Option<String>) {
    label(d, "Joon's trades", [o[0] + 8.0, o[1] + 8.0], LABEL);
    right_label(d, "Click one to edit it", o[0] + PW - 8.0, o[1] + 8.0, DIM);
    label(d, "Her trades", [o[0] + 8.0, o[1] + 20.0], DIM);
    let rows = trade_rows(v);
    for r in 0..TRADE_ROWS {
        let i = top + r;
        if i >= rows {
            break;
        }
        let rr = row_rect(o, r);
        if i == chosen {
            d.fill(rr, 0xFFFF_FFFF);
            d.fill([rr[0] + 1.0, rr[1] + 1.0, rr[2] - 2.0, rr[3] - 2.0], 0xFF8B_8B8B);
        } else if inside(rr, mouse) {
            d.fill(rr, 0xFFB0_B0B0);
        }
        match v.trades.get(i).filter(|t| t.parts.iter().any(Option::is_some)) {
            Some(t) => {
                trade_items(host, d, t, rr[0] + 2.0, rr[1] + 1.0, mouse, tip);
                let (text, argb) = match t.stock {
                    u32::MAX => ("gift".to_owned(), DIM),
                    0 => ("out".to_owned(), RED),
                    n => (format!("x{}", n.min(999)), if i == chosen { 0xFFFF_FFFF } else { LABEL }),
                };
                right_label(d, &text, rr[0] + rr[2] - 3.0, rr[1] + 5.0, argb);
            }
            None if i == v.trades.len() => label(d, "+ New trade", [rr[0] + 4.0, rr[1] + 5.0], if i == chosen { 0xFFFF_FFFF } else { LABEL }),
            None => label(d, "(empty)", [rr[0] + 4.0, rr[1] + 5.0], DIM),
        }
    }
    scrollbar(d, [o[0] + 126.0, o[1] + 30.0, 4.0, 18.0 * TRADE_ROWS as f32], top, rows, TRADE_ROWS);

    // The trade being edited.
    let t = v.trades.get(chosen);
    label(d, "They pay", [o[0] + 140.0, o[1] + 20.0], DIM);
    label(d, "They get", [o[0] + 206.0, o[1] + 20.0], DIM);
    for p in 0..3u8 {
        let r = part_rect(o, p);
        slot(d, r);
        if inside(r, mouse) {
            d.fill([r[0] + 1.0, r[1] + 1.0, 16.0, 16.0], 0x80FF_FFFF);
        }
        let s = part_of(t, p);
        item(host, d, &s, r[0] + 1.0, r[1] + 1.0, mouse, tip);
        if p == 2 && s.is_some() && t.is_some_and(|t| t.stock == 0) {
            cross(d, r[0] + 1.0, r[1] + 1.0);
        }
        if p == part {
            // The part their items go into: a yellow frame.
            let c = 0xFFFF_DD00;
            d.fill([r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, 1.0], c);
            d.fill([r[0] - 1.0, r[1] + r[3], r[2] + 2.0, 1.0], c);
            d.fill([r[0] - 1.0, r[1] - 1.0, 1.0, r[3] + 2.0], c);
            d.fill([r[0] + r[2], r[1] - 1.0, 1.0, r[3] + 2.0], c);
        }
    }
    arrow(d, o[0] + 186.0, o[1] + 40.0);
    let n = part_of(t, part).map_or(0, |(_, n)| n);
    button(d, at(o, 140.0, 56.0, 16.0, 16.0), "-", mouse, n > 1);
    let field = at(o, 158.0, 56.0, 32.0, 16.0);
    d.fill(field, 0xFFA0_A0A0);
    d.fill([field[0] + 1.0, field[1] + 1.0, field[2] - 2.0, field[3] - 2.0], 0xFF00_0000);
    let count = if n > 0 { n.to_string() } else { "-".to_owned() };
    let w = d.text_width(&count);
    d.text(&count, [(field[0] + (field[2] - w) / 2.0).floor(), field[1] + 4.0], 0xFFE0_E0E0, true);
    button(d, at(o, 192.0, 56.0, 16.0, 16.0), "+", mouse, n > 0 && n < 64);
    button(d, at(o, 212.0, 56.0, 36.0, 16.0), "Clear", mouse, n > 0);
    button(d, at(o, 140.0, 76.0, 108.0, 16.0), "Delete trade", mouse, chosen < v.trades.len());
    let (status, argb) = match t.filter(|t| t.parts.iter().any(Option::is_some)) {
        None => ("Pick a slot, then an item".to_owned(), DIM),
        Some(t) if t.parts[2].is_none() => ("They get nothing: a gift".to_owned(), DIM),
        Some(t) if t.stock == 0 => ("Out of stock".to_owned(), RED),
        Some(t) => (format!("In stock for {} trades", t.stock.min(9999)), GREEN),
    };
    let status = fit(d, &status, 108.0);
    label(d, &status, [o[0] + 140.0, o[1] + 98.0], argb);

    // Their items, to set the parts from.
    label(d, "Your items: click one to put it in", [o[0] + 8.0, o[1] + 120.0], LABEL);
    button(d, at(o, 176.0, 132.0, 72.0, 20.0), "Her stock", mouse, true);
    for i in 0..v.items.len().min(37) {
        let r = item_rect(o, i);
        slot(d, r);
        if inside(r, mouse) {
            d.fill([r[0] + 1.0, r[1] + 1.0, 16.0, 16.0], 0x80FF_FFFF);
        }
        item(host, d, &v.items[i], r[0] + 1.0, r[1] + 1.0, mouse, tip);
    }
    label(d, "Off hand", [o[0] + 196.0, o[1] + 195.0], DIM);
    label(d, "Right-click", [o[0] + 178.0, o[1] + 158.0], DIM);
    label(d, "puts in one", [o[0] + 178.0, o[1] + 168.0], DIM);
}

fn draw_send(d: &mut dyn DrawContext, v: &JoonView, o: [f32; 2], mouse: [f32; 2], top: usize) {
    label(d, "Send Joon to a player", [o[0] + 8.0, o[1] + 8.0], LABEL);
    let ready = any_set(v);
    if !ready {
        label(d, "Set a trade first, in the Trades tab", [o[0] + 8.0, o[1] + 20.0], RED);
    } else {
        label(d, "Players in her dimension", [o[0] + 8.0, o[1] + 20.0], DIM);
    }
    if v.players.is_empty() {
        label(d, "Nobody is here", [o[0] + 12.0, o[1] + 36.0], DIM);
    }
    for r in 0..PLAYER_ROWS {
        let i = top + r;
        let Some(p) = v.players.get(i) else { break };
        let rr = player_rect(o, r);
        let enabled = ready && p.wait == 0;
        button(d, rr, "", mouse, enabled);
        let hover = enabled && inside(rr, mouse);
        let text = if !enabled { 0xFFA0_A0A0 } else if hover { 0xFFFF_FFA0 } else { 0xFFE0_E0E0 };
        let name = if p.you { format!("{} (you)", p.name) } else { p.name.clone() };
        let status = if p.wait > 0 {
            format!("again in {}", clock(p.wait))
        } else if p.you {
            "Try her trades".to_owned()
        } else {
            "Send".to_owned()
        };
        let sw = d.text_width(&status);
        let name = fit(d, &name, rr[2] - sw - 20.0);
        d.text(&name, [rr[0] + 6.0, rr[1] + 7.0], text, true);
        d.text(&status, [rr[0] + rr[2] - 6.0 - sw, rr[1] + 7.0], text, true);
    }
    scrollbar(d, [o[0] + 244.0, o[1] + 30.0, 4.0, 24.0 * PLAYER_ROWS as f32], top, v.players.len(), PLAYER_ROWS);
    label(d, "She stays 200 s, and can't visit them", [o[0] + 8.0, o[1] + 200.0], DIM);
    label(d, "again for 300 s. Nothing can hurt her.", [o[0] + 8.0, o[1] + 210.0], DIM);
}

fn draw_offer(host: &dyn ClientHost, d: &mut dyn DrawContext, v: &JoonView, o: [f32; 2], mouse: [f32; 2], top: usize, tip: &mut Option<String>) {
    panel(d, [o[0], o[1], PW, PH]);
    let title = fit(d, &format!("Trade offer from {}", v.owner), PW - 60.0);
    label(d, &title, [o[0] + 8.0, o[1] + 8.0], LABEL);
    right_label(d, &clock(v.secs), o[0] + PW - 8.0, o[1] + 8.0, LABEL);
    label(d, "Shift-click Trade to trade all you can", [o[0] + 8.0, o[1] + 20.0], DIM);
    if v.trades.is_empty() {
        label(d, "She has nothing to trade", [o[0] + 12.0, o[1] + 36.0], DIM);
    }
    for r in 0..OFFER_ROWS {
        let i = top + r;
        let Some(t) = v.trades.get(i) else { break };
        let rr = offer_rect(o, r);
        d.fill(rr, 0xFFB4_B4B4);
        for k in 0..3 {
            let x = rr[0] + 3.0 + [0.0, 20.0, 60.0][k];
            slot(d, [x, rr[1] + 3.0, 18.0, 18.0]);
        }
        let shown = ShownTrade { parts: t.parts.clone(), ..t.clone() };
        item(host, d, &shown.parts[0], rr[0] + 4.0, rr[1] + 4.0, mouse, tip);
        item(host, d, &shown.parts[1], rr[0] + 24.0, rr[1] + 4.0, mouse, tip);
        arrow(d, rr[0] + 44.0, rr[1] + 12.0);
        item(host, d, &shown.parts[2], rr[0] + 64.0, rr[1] + 4.0, mouse, tip);
        if t.parts[2].is_some() && t.stock == 0 {
            cross(d, rr[0] + 64.0, rr[1] + 4.0);
        }
        if t.parts[0].is_none() && t.parts[1].is_none() {
            label(d, "Free", [rr[0] + 86.0, rr[1] + 8.0], GREEN);
        }
        let (text, enabled) = if t.stock == 0 {
            ("Out of stock", false)
        } else if !t.affordable {
            ("Can't pay", false)
        } else {
            ("Trade", true)
        };
        button(d, offer_button(rr), text, mouse, enabled);
    }
    scrollbar(d, [o[0] + 244.0, o[1] + 30.0, 4.0, 26.0 * OFFER_ROWS as f32], top, v.trades.len(), OFFER_ROWS);
    button(d, at(o, 8.0, 196.0, 240.0, 20.0), "Send her home", mouse, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::ShownPlayer;

    fn owner_view() -> JoonView {
        JoonView {
            trades: vec![ShownTrade { parts: [Some(("minecraft:emerald".into(), 2)), None, Some(("minecraft:bread".into(), 6))], stock: 3, affordable: true }],
            items: vec![Some(("minecraft:dirt".into(), 10)); 37],
            players: vec![ShownPlayer { uuid: 7, name: "Steve".into(), wait: 0, you: false }],
            ..JoonView::default()
        }
    }

    #[test]
    fn everything_clickable_is_on_the_panel_or_its_tabs() {
        let window = [0.0, 0.0, 427.0, 240.0];
        let o = origin(window);
        let panel = [o[0], o[1] - 20.0, PW, PH + 20.0];
        let v = owner_view();
        for tab in [Tab::Trades, Tab::Send] {
            let ui = Ui { tab, ..Ui::default() };
            for (r, h) in hits(&v, &ui, o) {
                assert!(r[0] >= panel[0] && r[1] >= panel[1], "{h:?} {r:?}");
                assert!(r[0] + r[2] <= panel[0] + panel[2] && r[1] + r[3] <= panel[1] + panel[3], "{h:?} {r:?}");
            }
        }
        // The player's slots don't overlap.
        let slots: Vec<Rect> = (0..37).map(|i| item_rect(o, i)).collect();
        for (i, a) in slots.iter().enumerate() {
            for b in &slots[i + 1..] {
                assert!(!(a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]));
            }
        }
    }

    #[test]
    fn clicks_become_what_the_server_is_told() {
        let window = [0.0, 0.0, 427.0, 240.0];
        let o = origin(window);
        let centre = |r: Rect| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
        {
            let mut g = UI.lock().unwrap();
            *g = Some(Ui { view: Some(owner_view()), open: true, ..Ui::default() });
        }
        // An item into the first part: the next empty part (the second) is chosen after.
        pressed(window, centre(item_rect(o, 4)), 0, false);
        pressed(window, centre(tab_rect(o, Tab::Send)), 0, false);
        pressed(window, centre(player_rect(o, 0)), 0, false);
        let ui = UI.lock().unwrap().take().unwrap();
        assert_eq!(ui.part, 1);
        assert_eq!(ui.tab, Tab::Send);
        assert_eq!(ui.outbox, vec![JoonAct::Set { trade: 0, part: 0, slot: 4, one: false }, JoonAct::Send { player: 7 }]);
    }
}
