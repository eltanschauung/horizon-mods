//! The Horizonmatica menu (M): the mod's own screen (`ClientHost::mod_screen_open`), drawn with
//! the widgets of `ui.rs` in `screen_render_post`. The game keeps running behind it.
//!
//! Tabs: Load, Placement, Save, Tool, Printer, Display, Materials, and the credits. The keys are
//! the game's key mappings now (Options > Controls), so there is no Keys tab any more.

use crate::runtime::{self, display_name, short};
use crate::settings::{self, Kind};
use crate::store;
use crate::ui::{Rect, Ui};
use crate::world::World;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// What the Load tab shows of a file (read when it is picked).
#[derive(Clone, Debug)]
struct FileInfo {
    name: String,
    format: String,
    size: [i32; 3],
    blocks: u64,
    regions: usize,
    author: String,
}

/// What the menu keeps between frames.
#[derive(Default)]
struct Screen {
    /// The menu is to open (or close) at the next tick (that needs the game's mutable host).
    open: bool,
    close: bool,
    tab: usize,
    /// The schematics folder's files with their sizes, and when they were listed.
    files: Vec<(String, u64)>,
    files_at: Option<Instant>,
    /// The picked file, and what is known of files (None while a thread reads one).
    file: Option<String>,
    info: HashMap<String, Option<Result<FileInfo, String>>>,
    info_chan: Option<(std::sync::mpsc::Sender<(String, Result<FileInfo, String>)>, std::sync::mpsc::Receiver<(String, Result<FileInfo, String>)>)>,
    /// What the origin and corner fields were last set to (they follow the placement unless
    /// being typed in).
    origin: Option<[i32; 3]>,
    corners: [Option<[i32; 3]>; 2],
    /// The file a save would overwrite, once asked.
    overwrite: Option<String>,
}

thread_local! {
    static SCREEN: RefCell<Screen> = RefCell::new(Screen::default());
}

fn sc<R>(f: impl FnOnce(&mut Screen) -> R) -> R {
    SCREEN.with_borrow_mut(f)
}

/// Opens the menu (at the next tick).
pub fn open(_w: &World<'_>) {
    sc(|s| s.open = true);
}

/// Whether the menu is to open now (taken).
pub fn take_open() -> bool {
    sc(|s| std::mem::take(&mut s.open))
}

/// Whether the menu is to close now (taken).
pub fn take_close() -> bool {
    sc(|s| std::mem::take(&mut s.close))
}

/// Runs a command for the screen; its first line (or the error) shows at the bottom.
fn act(w: &World<'_>, args: &[&str]) {
    let msg = match runtime::run(w, args) {
        Ok(lines) => lines.into_iter().next().unwrap_or_default(),
        Err(e) => format!("Error: {e}"),
    };
    runtime::with(|s| s.message = msg);
}

/// A key's or a menu row's action; what it says goes to the chat.
fn action(w: &World<'_>, id: &str) {
    runtime::action(w, id);
}

fn file_info(dir: &std::path::Path, name: &str) -> Result<FileInfo, String> {
    let bytes = std::fs::read(dir.join(name)).map_err(|e| e.to_string())?;
    let s = horizonmatica_core::formats::summary(&bytes)?;
    Ok(FileInfo {
        name: if s.name.is_empty() { name.to_string() } else { s.name },
        format: s.format.name(),
        size: s.size,
        blocks: s.blocks,
        regions: s.regions,
        author: s.author,
    })
}

/// What is known of a file; asked for the first time, a thread starts reading it (`None` until
/// it is in).
fn file_info_cached(dir: &std::path::Path, name: &str, key: &str) -> Option<Result<FileInfo, String>> {
    let known = sc(|s| {
        if let Some((_, rx)) = &s.info_chan {
            while let Ok((k, info)) = rx.try_recv() {
                s.info.insert(k, Some(info));
            }
        }
        s.info.get(key).cloned()
    });
    if let Some(info) = known {
        return info;
    }
    let tx = sc(|s| {
        s.info.insert(key.to_string(), None);
        s.info_chan.get_or_insert_with(std::sync::mpsc::channel).0.clone()
    });
    let (name, key, dir) = (name.to_string(), key.to_string(), dir.to_path_buf());
    std::thread::spawn(move || {
        let _ = tx.send((key, file_info(&dir, &name)));
    });
    None
}

// ---------------------------------------------------------------- the settings grid

/// The settings of each part of the menu (registered settings, `settings::SPECS`).
const DISPLAY: &[&str] = &[
    "render", "through", "mode", "alpha", "radius_view", "hide_fluids", "placement_boxes", "show_extra", "show_wrong",
    "highlight_missing", "highlight_wrong", "strict", "power",
];
const HUD: &[&str] = &["hud", "materials_hud"];
const PRINTER: &[&str] = &["printer_interval", "printer_reach", "air_place"];
const EASY_PLACE: &[&str] = &["easy_place", "easy_interval", "easy_air"];
const SELECTION: &[&str] = &["tool", "tool_mode", "tool_range", "tool_target", "tool_fluids", "tool_hud"];

fn number_text(v: f64, step: f64) -> String {
    if step >= 1.0 { format!("{v:.0}") } else if step >= 0.1 { format!("{v:.1}") } else { format!("{v:.2}") }
}

/// The settings `keys` in `columns` columns of the game's widgets (toggles and choices as
/// buttons, numbers as sliders) from the top of `r`. Returns the height used.
fn settings_grid(ui: &mut Ui<'_>, w: &World<'_>, keys: &[&str], r: Rect, columns: usize) -> f32 {
    let columns = columns.max(1);
    let gap = 4.0;
    let cw = ((r[2] - gap * (columns as f32 - 1.0)) / columns as f32).floor();
    let mut changed = false;
    let mut y = r[1];
    for (i, key) in keys.iter().enumerate() {
        let Some(&(_, label, kind)) = settings::SPECS.iter().find(|(k, ..)| k == key) else { continue };
        let col = i % columns;
        if i > 0 && col == 0 {
            y += 24.0;
        }
        let x = r[0] + col as f32 * (cw + gap);
        let b = [x, y, cw, 20.0];
        match kind {
            Kind::Toggle(_) => {
                let on = settings::get_bool(w, key);
                if ui.button(b, &format!("{label}: {}", if on { "ON" } else { "OFF" }), true) {
                    settings::set(w, key, if on { "false" } else { "true" });
                    changed = true;
                }
            }
            Kind::Choice(options, _) => {
                let now = settings::get_choice(w, key);
                if ui.button(b, &format!("{label}: {now}"), true) {
                    let i = options.iter().position(|o| *o == now).unwrap_or(0);
                    let n = options.len();
                    let next = if ui.shift { (i + n - 1) % n } else { (i + 1) % n };
                    settings::set(w, key, options[next]);
                    changed = true;
                }
            }
            Kind::Range(min, max, step, _) => {
                let v = settings::get_number(w, key);
                let fraction = ((v - min) / (max - min)) as f32;
                if let Some(f) = ui.slider(key, b, &format!("{label}: {}", number_text(v, step)), fraction) {
                    let raw = min + f64::from(f) * (max - min);
                    let snapped = ((raw - min) / step).round() * step + min;
                    let snapped = snapped.clamp(min, max);
                    if (snapped - v).abs() > 1e-9 {
                        settings::set(w, key, &snapped.to_string());
                        changed = true;
                    }
                }
            }
        }
    }
    if changed {
        runtime::settings_changed();
        runtime::ensure_settings(w);
    }
    if keys.is_empty() { 0.0 } else { y + 24.0 - r[1] }
}

// ---------------------------------------------------------------- the screen

const TABS: [&str; 7] = ["Load", "Placement", "Save", "Tool", "Printer", "Display", "Materials"];
const CREDITS_TAB: usize = TABS.len();

/// Draws the menu (the `screen_render_post` hook, while it is open). Whether it asked to close.
pub fn draw(w: &World<'_>, mut ui: Ui<'_>) {
    runtime::ensure_settings(w);
    let width = (ui.width - 20.0).min(500.0).floor();
    let x0 = ((ui.width - width) / 2.0).floor();
    let loaded = runtime::with(|s| {
        s.placement().map(|p| if s.slots.len() > 1 { format!("{} ({}/{})", display_name(p), s.selected + 1, s.slots.len()) } else { display_name(p) })
    });
    let cx = ui.width / 2.0;
    ui.text_centered(cx, 7.0, "Horizonmatica", 0xFFFFFF);
    if let Some(name) = &loaded {
        let shown = ui.fit(name, width / 2.0 - 50.0);
        ui.text_right(x0 + width, 7.0, &shown, 0x55FFFF);
    }
    let tab = sc(|s| s.tab).min(CREDITS_TAB);
    let mut next = ui.tabs([x0, 20.0, width, 20.0], &TABS, tab);
    // 1-7 switch tabs, M closes (while no edit box has the keys).
    for i in 0..TABS.len() {
        if ui.key(&format!("key.keyboard.{}", i + 1)) {
            next = Some(i);
        }
    }
    if ui.key("key.keyboard.m") {
        ui.close();
    }
    if let Some(t) = next {
        sc(|s| s.tab = t);
    }
    let body = [x0, 46.0, width, ui.height - 46.0 - 30.0];
    match tab {
        0 => tab_load(w, &mut ui, body),
        1 => tab_placement(w, &mut ui, body),
        2 => tab_save(w, &mut ui, body),
        3 => tab_tool(w, &mut ui, body),
        4 => tab_printer(w, &mut ui, body),
        5 => tab_display(w, &mut ui, body),
        6 => tab_materials(w, &mut ui, body),
        _ => tab_credits(w, &mut ui, body),
    }
    let fy = ui.height - 25.0;
    let msg = runtime::with(|s| s.message.clone());
    if !msg.is_empty() {
        ui.text_fit(x0, fy + 6.0, width - 110.0, &msg, 0xFFFF55);
    }
    if ui.button([x0, 2.0, 50.0, 16.0], "Credits", tab != CREDITS_TAB) {
        sc(|s| s.tab = CREDITS_TAB);
    }
    if ui.button([x0 + width - 100.0, fy, 100.0, 20.0], "Done", true) {
        ui.close();
    }
    if ui.end() {
        sc(|s| s.close = true);
    }
}

fn tab_load(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let dir = store::schematics_dir(w);
    // The folder is listed again every few seconds while the tab is open.
    sc(|s| {
        if s.files_at.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
            s.files_at = Some(Instant::now());
            s.files = dir
                .as_ref()
                .map(|dir| {
                    horizonmatica_core::list_schematics(dir)
                        .into_iter()
                        .map(|n| {
                            let size = std::fs::metadata(dir.join(&n)).map_or(0, |m| m.len());
                            (n, size)
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
    });
    let lw = (r[2] * 0.55).floor();
    let query = ui.edit("search", [r[0] + 1.0, r[1] + 1.0, lw - 2.0, 18.0], "Search...", 64).to_lowercase();
    let files: Vec<(String, u64)> = sc(|s| s.files.iter().filter(|(n, _)| n.to_lowercase().contains(&query)).cloned().collect());
    let total = sc(|s| s.files.len());
    let list = [r[0], r[1] + 24.0, lw, r[3] - 24.0];
    ui.fill(list, 0x9000_0000);
    ui.frame(list, 0xFF50_5050);
    let row_h = 14.0;
    let content = files.len() as f32 * row_h + 4.0;
    let y0 = ui.scroll_begin("files", list, content);
    let selected = sc(|s| s.file.clone());
    let mut picked = None;
    for (i, (name, size)) in files.iter().enumerate() {
        let y = y0 + 2.0 + i as f32 * row_h;
        if y + row_h < list[1] || y > list[1] + list[3] {
            continue;
        }
        let row = [list[0] + 2.0, y, list[2] - 10.0, row_h];
        let is_selected = selected.as_deref() == Some(name.as_str());
        if is_selected {
            ui.fill(row, 0x50FF_FFFF);
        } else if ui.hovered(row) {
            ui.fill(row, 0x25FF_FFFF);
        }
        let size = runtime::human_size(*size);
        let sw = ui.text_width(&size);
        ui.text_fit(row[0] + 3.0, y + 3.0, row[2] - sw - 12.0, name, if is_selected { 0xFFFFFF } else { 0xD0D0D0 });
        ui.text_right(row[0] + row[2] - 3.0, y + 3.0, &size, 0x909090);
        if ui.clicked(row) {
            picked = Some(name.clone());
        }
    }
    ui.scroll_end("files", list, content);
    if files.is_empty() {
        let text = if total == 0 { "No schematic files yet" } else { "Nothing matches" };
        ui.text(list[0] + 5.0, list[1] + 5.0, text, 0x909090);
    }
    if let Some(name) = picked {
        // A second click on the picked file loads it.
        if selected.as_deref() == Some(name.as_str()) {
            act(w, &["load", &name]);
            sc(|s| s.tab = 1);
            return;
        }
        sc(|s| s.file = Some(name));
    }

    // The picked file: what it is, and loading it.
    let rx = r[0] + lw + 8.0;
    let rw = r[2] - lw - 8.0;
    let selected = sc(|s| s.file.clone().filter(|n| s.files.iter().any(|(f, _)| f == n)));
    let mut y = r[1] + 2.0;
    match (&selected, &dir) {
        (Some(name), Ok(dir)) => {
            let size = sc(|s| s.files.iter().find(|(f, _)| f == name).map_or(0, |(_, b)| *b));
            let key = format!("{name}|{size}");
            match file_info_cached(dir, name, &key) {
                None => {
                    ui.text_wrapped(rx, y, rw, "Reading the file...", 0xA0A0A0);
                }
                Some(Ok(i)) => {
                    y += ui.text_wrapped(rx, y, rw, &i.name, 0xFFFFFF) + 2.0;
                    let lines = [
                        (format!("{} x {} x {}", i.size[0], i.size[1], i.size[2]), 0xE0E0E0),
                        (format!("{} blocks", i.blocks), 0xE0E0E0),
                        (format!("{} region{}", i.regions, if i.regions == 1 { "" } else { "s" }), 0xA0A0A0),
                        (i.format.clone(), 0xA0A0A0),
                        (if i.author.is_empty() { String::new() } else { format!("by {}", i.author) }, 0xA0A0A0),
                    ];
                    for (text, rgb) in lines {
                        if !text.is_empty() {
                            ui.text_fit(rx, y, rw, &text, rgb);
                            y += 11.0;
                        }
                    }
                }
                Some(Err(e)) => {
                    ui.text_wrapped(rx, y, rw, &format!("Can't read it: {e}"), 0xFF5555);
                }
            }
            let by = r[1] + r[3] - 68.0;
            if ui.button_tip([rx, by, rw, 20.0], "Load here", true, &["Places it at your feet (or click the file twice)"]) {
                act(w, &["load", name]);
                sc(|s| s.tab = 1);
            }
        }
        _ => {
            ui.text_wrapped(rx, y, rw, "Pick a file on the left. Put .litematic, .schem, .schematic and .nbt files into the schematics folder.", 0xA0A0A0);
        }
    }
    let by = r[1] + r[3] - 44.0;
    let half = ((rw - 4.0) / 2.0).floor();
    if ui.button_tip([rx, by, half, 20.0], "Reload last", true, &["Loads the last schematic again where it was"]) {
        act(w, &["reload"]);
    }
    if ui.button([rx + half + 4.0, by, rw - half - 4.0, 20.0], "Open folder", true) {
        action(w, "folder");
    }
    if ui.button([rx, by + 24.0, rw, 20.0], "Refresh", true) {
        sc(|s| {
            s.files_at = None;
            s.info.clear();
        });
    }
}

fn tab_placement(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    // Every placement: name, origin, hidden or locked.
    let rows: Vec<(String, [i32; 3], bool, bool)> =
        runtime::with(|s| s.slots.iter().map(|x| (display_name(&x.placement), x.placement.origin, x.shown, x.locked)).collect());
    if rows.is_empty() {
        ui.text(r[0], r[1] + 4.0, "No schematic loaded.", 0xA0A0A0);
        if ui.button([r[0], r[1] + 18.0, 150.0, 20.0], "Load one", true) {
            sc(|s| s.tab = 0);
        }
        return;
    }
    let selected = runtime::with(|s| s.selected.min(s.slots.len() - 1));

    // Left: the list; a click selects.
    let lw = (r[2] * 0.38).floor();
    let list = [r[0], r[1], lw, r[3] - 96.0];
    ui.fill(list, 0x9000_0000);
    ui.frame(list, 0xFF50_5050);
    let row_h = 26.0;
    let content = rows.len() as f32 * row_h + 4.0;
    let y0 = ui.scroll_begin("placements", list, content);
    let mut picked = None;
    for (i, (name, origin, shown, locked)) in rows.iter().enumerate() {
        let y = y0 + 2.0 + i as f32 * row_h;
        if y + row_h < list[1] || y > list[1] + list[3] {
            continue;
        }
        let row = [list[0] + 2.0, y, list[2] - 10.0, row_h - 2.0];
        let is_selected = i == selected;
        if is_selected {
            ui.fill(row, 0x50FF_FFFF);
            ui.fill([row[0], row[1], 2.0, row[3]], if *locked { 0xFFFF_AA00 } else { 0xFF55_FFFF });
        } else if ui.hovered(row) {
            ui.fill(row, 0x25FF_FFFF);
        }
        let rgb = match (is_selected, *shown) {
            (true, _) => 0xFFFFFF,
            (false, true) => 0xD0D0D0,
            (false, false) => 0x808080,
        };
        ui.text_fit(row[0] + 6.0, y + 3.0, row[2] - 10.0, &format!("{}. {name}", i + 1), rgb);
        let mut detail = format!("{} {} {}", origin[0], origin[1], origin[2]);
        if !shown {
            detail += " · hidden";
        }
        if *locked {
            detail += " · locked";
        }
        ui.text_fit(row[0] + 6.0, y + 13.0, row[2] - 10.0, &detail, if *locked { 0xFFAA00 } else { 0x909090 });
        if ui.clicked(row) {
            picked = Some(i);
        }
    }
    ui.scroll_end("placements", list, content);
    if let Some(i) = picked
        && i != selected
    {
        act(w, &["select", &(i + 1).to_string()]);
    }
    let by = r[1] + r[3] - 20.0;
    settings_grid(ui, w, EASY_PLACE, [r[0], by - 72.0, lw, 0.0], 1);
    let half = ((lw - 4.0) / 2.0).floor();
    if ui.button_tip([r[0], by, half, 20.0], "Load more", true, &["Another schematic, as one more placement"]) {
        sc(|s| s.tab = 0);
    }
    if ui.button_tip([r[0] + half + 4.0, by, lw - half - 4.0, 20.0], "Unload all", true, &["Removes every placement"]) {
        act(w, &["unload", "all"]);
    }

    // Right: the selected placement.
    let info = runtime::with(|s| {
        let slot = s.sel()?;
        let p = &slot.placement;
        let (lo, hi) = p.bounds();
        Some((
            display_name(p),
            p.origin,
            [0, 1, 2].map(|i| hi[i] - lo[i] + 1),
            p.rotation.name(),
            p.mirror.name(),
            s.layer.describe(),
            s.moving,
            slot.shown,
            slot.locked,
            s.tally().map(|t| (t.percent(), t.missing, t.wrong_block + t.wrong_state, t.extra)),
            s.settings.tool_placement,
        ))
    });
    let Some((name, origin, size, rotation, mirror, layers, moving, shown, locked, tally, tool_placement)) = info else { return };
    let x = r[0] + lw + 8.0;
    // Scrolled where the window is too low for it (at 720p).
    let area = [x, r[1], r[2] - lw - 8.0, r[3]];
    let content = 37.0 + 8.0 * 24.0 - 4.0;
    let wd = if content > area[3] { area[2] - 10.0 } else { area[2] };
    let free = !locked;
    let half = ((wd - 4.0) / 2.0).floor();
    let third = ((wd - 8.0) / 3.0).floor();
    let mut y = ui.scroll_begin("placement_info", area, content);
    ui.text_fit(x, y + 1.0, wd, &format!("{}. {name}", selected + 1), 0xFFFFFF);
    y += 12.0;
    ui.text_fit(x, y, wd, &format!("{} x {} x {} · rotation {rotation} · mirror {mirror}", size[0], size[1], size[2]), 0xC0C0C0);
    y += 11.0;
    match tally {
        Some((pct, missing, wrong, extra)) => ui.text_fit(x, y, wd, &format!("{pct:.1}% done · {missing} missing · {wrong} wrong · {extra} extra"), 0x55FF55),
        None => ui.text_fit(x, y, wd, "Checking the world...", 0xA0A0A0),
    }
    y += 14.0;

    // Shown, locked, gone.
    if ui.button_tip([x, y, third, 20.0], if shown { "Hide" } else { "Show" }, true, &["Hidden placements stay loaded but aren't drawn or printed"]) {
        act(w, &["show", "toggle"]);
    }
    let tip = ["Locked: it can't be moved, turned or mirrored", "(neither here, nor by commands or the tool)"];
    if ui.button_tip([x + third + 4.0, y, third, 20.0], if locked { "Unlock" } else { "Lock" }, true, &tip) {
        act(w, &["lock", "toggle"]);
    }
    if ui.button_tip([x + 2.0 * (third + 4.0), y, wd - 2.0 * (third + 4.0), 20.0], "Unload", true, &["Removes this placement (the world stays as it is)"]) {
        act(w, &["unload"]);
    }
    y += 24.0;

    // Where it is.
    ui.text(x, y + 6.0, "Origin", if free { 0xFFFFFF } else { 0x808080 });
    // The fields follow the origin unless one is being typed in.
    let typing = ui.focused("ox") || ui.focused("oy") || ui.focused("oz");
    if !typing && sc(|s| s.origin != Some(origin)) {
        ui.set_edit("ox", &origin[0].to_string());
        ui.set_edit("oy", &origin[1].to_string());
        ui.set_edit("oz", &origin[2].to_string());
        sc(|s| s.origin = Some(origin));
    }
    let fx = x + 36.0;
    let fw = ((wd - 36.0 - 40.0 - 6.0) / 3.0).floor();
    let ox = ui.edit("ox", [fx, y + 1.0, fw - 2.0, 18.0], "x", 9);
    let oy = ui.edit("oy", [fx + fw, y + 1.0, fw - 2.0, 18.0], "y", 9);
    let oz = ui.edit("oz", [fx + 2.0 * fw, y + 1.0, fw - 2.0, 18.0], "z", 9);
    let set = ui.button([fx + 3.0 * fw + 2.0, y, 40.0, 20.0], "Set", free) || (typing && ui.key("key.keyboard.enter"));
    if set {
        match (ox.trim().parse::<i32>(), oy.trim().parse::<i32>(), oz.trim().parse::<i32>()) {
            (Ok(a), Ok(b), Ok(c)) => act(w, &["origin", &a.to_string(), &b.to_string(), &c.to_string()]),
            _ => runtime::with(|s| s.message = "Origin: three whole numbers".into()),
        }
        sc(|s| s.origin = None);
    }
    y += 24.0;
    // Nudge buttons: 1 block, 10 with Shift.
    let step = if ui.shift { 10 } else { 1 };
    let bw = ((wd - 10.0) / 6.0).floor();
    let nudges: [(&str, [i32; 3]); 6] = [("-X", [-1, 0, 0]), ("+X", [1, 0, 0]), ("-Y", [0, -1, 0]), ("+Y", [0, 1, 0]), ("-Z", [0, 0, -1]), ("+Z", [0, 0, 1])];
    for (i, (label, d)) in nudges.into_iter().enumerate() {
        let tip = format!("Move {step} block{} ({label}); Shift: 10", if step == 1 { "" } else { "s" });
        if ui.button_tip([x + i as f32 * (bw + 2.0), y, bw, 20.0], label, free, &[&tip]) {
            let d = d.map(|v| (v * step).to_string());
            act(w, &["move", &d[0], &d[1], &d[2]]);
        }
    }
    y += 24.0;
    if ui.button([x, y, half, 20.0], "Move to me", free) {
        act(w, &["origin", "here"]);
    }
    let label = format!("Move mode: {}", if moving { "ON" } else { "OFF" });
    if ui.button_tip([x + half + 4.0, y, wd - half - 4.0, 20.0], &label, free, &["Wheel, arrows and PgUp/PgDn move it in the world (N)"]) {
        runtime::toggle_move_mode(w);
        if runtime::with(|s| s.moving) {
            ui.close();
        }
    }
    y += 24.0;

    // How it is turned.
    if ui.button([x, y, third, 20.0], "Rotate left", free) {
        act(w, &["rotate", "ccw"]);
    }
    if ui.button([x + third + 4.0, y, third, 20.0], "Rotate right", free) {
        act(w, &["rotate", "cw"]);
    }
    let tip = ["none, x (front-back), z (left-right)"];
    if ui.button_tip([x + 2.0 * (third + 4.0), y, wd - 2.0 * (third + 4.0), 20.0], &format!("Mirror: {mirror}"), free, &tip) {
        action(w, "mirror_next");
    }
    y += 24.0;

    // Layers (of every placement).
    if ui.button([x, y, wd, 20.0], &format!("Layers: {layers}"), true) {
        let next = runtime::next_layer_mode();
        act(w, &["layer", next]);
    }
    y += 24.0;
    if ui.button([x, y, third, 20.0], "Layer down", true) {
        act(w, &["layer", "down"]);
    }
    if ui.button([x + third + 4.0, y, third, 20.0], "Layer up", true) {
        act(w, &["layer", "up"]);
    }
    if ui.button([x + 2.0 * (third + 4.0), y, wd - 2.0 * (third + 4.0), 20.0], "All layers", true) {
        act(w, &["layer", "all"]);
    }
    y += 24.0;

    // The tool moves placements in its placement mode.
    let label = format!("Selection tool: {} mode", if tool_placement { "placement" } else { "area" });
    let tip = [
        "Placement mode: left click selects the placement looked at,",
        "right click moves the selected one there, Alt+wheel moves it,",
        "Ctrl+wheel turns it. Ctrl+middle click switches the mode.",
    ];
    if ui.button_tip([x, y, wd, 20.0], &label, true, &tip) {
        act(w, &["tool", "mode", "toggle"]);
    }
    ui.scroll_end("placement_info", area, content);
}

fn tab_save(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let area = runtime::with(|s| s.area);
    let mut y = r[1];
    ui.text(r[0], y + 2.0, "Area selection: two opposite corners of the box to save", 0xFFFFFF);
    y += 16.0;
    let label_w = 52.0;
    let bw = 48.0;
    let fw = ((r[2] - label_w - 40.0 - 2.0 * bw - 3.0 * 4.0) / 3.0).floor();
    for (i, corner) in area.into_iter().enumerate() {
        let ids = [0, 1, 2].map(|k| format!("c{}{}", i + 1, ["x", "y", "z"][k]));
        // The fields follow the corner unless one is being typed in.
        let typing = ids.iter().any(|id| ui.focused(id));
        if !typing && sc(|s| s.corners[i] != corner) {
            for (k, id) in ids.iter().enumerate() {
                ui.set_edit(id, &corner.map_or(String::new(), |c| c[k].to_string()));
            }
            sc(|s| s.corners[i] = corner);
        }
        let rgb = if i == 0 { 0xFF7070 } else { 0x7090FF };
        ui.text(r[0], y + 6.0, &format!("Corner {}", i + 1), rgb);
        let fx = r[0] + label_w;
        let v = [0, 1, 2].map(|k| ui.edit(&ids[k], [fx + k as f32 * fw, y + 1.0, fw - 2.0, 18.0], ["x", "y", "z"][k], 9));
        let mut x = fx + 3.0 * fw + 2.0;
        let cmd = if i == 0 { "pos1" } else { "pos2" };
        if ui.button([x, y, 40.0, 20.0], "Set", true) {
            match v.iter().map(|s| s.trim().parse::<i32>()).collect::<Result<Vec<_>, _>>() {
                Ok(c) => act(w, &[cmd, &c[0].to_string(), &c[1].to_string(), &c[2].to_string()]),
                Err(_) => runtime::with(|s| s.message = format!("Corner {}: three whole numbers", i + 1)),
            }
            sc(|s| s.corners[i] = None);
        }
        x += 44.0;
        if ui.button_tip([x, y, bw, 20.0], "Look", true, &["The block you look at (/hm pos1, /hm pos2)"]) {
            act(w, &[cmd]);
        }
        x += bw + 4.0;
        if ui.button_tip([x, y, bw, 20.0], "Here", true, &["The block you stand in"]) {
            act(w, &[cmd, "here"]);
        }
        y += 24.0;
    }
    let size = runtime::with(|s| s.area_size());
    let text = match size {
        Some(d) => format!("{} x {} x {} = {} blocks", d[0], d[1], d[2], runtime::volume_text(d)),
        None => "Set both corners.".to_string(),
    };
    ui.text(r[0], y + 2.0, &text, if size.is_some() { 0x55FF55 } else { 0xA0A0A0 });
    y += 16.0;
    ui.text(r[0], y + 6.0, "Name", 0xFFFFFF);
    let name = ui.edit("save_name", [r[0] + label_w, y + 1.0, r[2] - label_w - 104.0, 18.0], "my_build", 80);
    let stem = runtime::file_stem(&name);
    let exists = stem.as_ref().is_ok_and(|s| store::schematics_dir(w).is_ok_and(|d| d.join(format!("{s}.litematic")).exists()));
    let confirm = sc(|s| s.overwrite.clone()).is_some_and(|n| stem.as_ref().is_ok_and(|s| *s == n));
    let label = if exists && confirm { "Overwrite" } else { "Save" };
    let tip = ["Saves the area as a .litematic into the schematics folder", "(chunk by chunk, as they are loaded)"];
    let progress = runtime::with(|s| runtime::save_lines(w, s));
    if let Some((lines, done)) = &progress {
        // While it runs: Cancel in Save's place, the progress above the note at the bottom.
        if ui.button_tip([r[0] + r[2] - 100.0, y, 100.0, 20.0], "Cancel", runtime::with(|s| s.save.is_some()), &["Stops the save; nothing is written"]) {
            act(w, &["save", "cancel"]);
        }
        let bar = [r[0], r[1] + r[3] - 80.0, r[2], 20.0];
        ui.fill(bar, 0xC000_0000);
        ui.fill([bar[0], bar[1], (bar[2] * done).floor(), bar[3]], 0xC0FF_8800);
        ui.frame(bar, 0xFF80_8080);
        if let Some((text, _)) = lines.first() {
            ui.text_fit(bar[0] + 4.0, bar[1] + 6.0, bar[2] - 8.0, text, 0xFFFFFF);
        }
        if let Some((text, rgb)) = lines.get(1) {
            ui.text_fit(r[0], bar[1] + 24.0, r[2], text, *rgb);
        }
    } else if ui.button_tip([r[0] + r[2] - 100.0, y, 100.0, 20.0], label, size.is_some() && stem.is_ok(), &tip) {
        match &stem {
            Ok(s) if exists && !confirm => {
                let s = s.clone();
                runtime::with(|st| st.message = format!("{s}.litematic exists: click Overwrite to replace it"));
                sc(|st| st.overwrite = Some(s));
            }
            _ => {
                let msg = match runtime::start_save(w, &name, exists) {
                    Ok(m) => m,
                    Err(e) => format!("Error: {e}"),
                };
                runtime::with(|s| s.message = msg);
                sc(|s| s.overwrite = None);
            }
        }
    }
    y += 26.0;
    let half = ((r[2] - 4.0) / 2.0).floor();
    if ui.button([r[0], y, half, 20.0], "Clear selection", area.iter().any(Option::is_some)) {
        act(w, &["area", "clear"]);
    }
    if ui.button([r[0] + half + 4.0, y, r[2] - half - 4.0, 20.0], "Open folder", true) {
        action(w, "folder");
    }
    y += 26.0;
    let (tool, item) = runtime::with(|s| (s.settings.tool, s.settings.tool_item.clone()));
    let label = format!("Selection tool: {} ({}) · settings", if tool { "ON" } else { "OFF" }, short(&item));
    if ui.button_tip([r[0], y, r[2], 20.0], &label, true, &["Opens the Tool tab: which item, its reach, its controls"]) {
        sc(|s| s.tab = 3);
    }
    if tool {
        ui.text_wrapped(r[0], y + 26.0, r[2], &format!("With the {} in hand: left click corner 1, right click corner 2.", short(&item).replace('_', " ")), 0xA0A0A0);
    }
    ui.text_wrapped(
        r[0],
        r[1] + r[3] - 30.0,
        r[2],
        "Saves the blocks with their states, as Litematica reads them. Not saved: what is inside chests, sign texts and entities.",
        0x808080,
    );
}

/// Items the Tool tab offers at a click.
const TOOL_ITEMS: [&str; 8] = [
    "minecraft:stick",
    "minecraft:blaze_rod",
    "minecraft:breeze_rod",
    "minecraft:bone",
    "minecraft:feather",
    "minecraft:wooden_axe",
    "minecraft:golden_hoe",
    "minecraft:arrow",
];

/// The selection tool: which item it is, how it aims, and its controls.
fn tab_tool(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let (on, item) = runtime::with(|s| (s.settings.tool, s.settings.tool_item.clone()));
    let mut y = r[1];
    ui.fill([r[0], y, 40.0, 40.0], 0x9000_0000);
    ui.frame([r[0], y, 40.0, 40.0], if on { 0xFF55_FF55 } else { 0xFF80_8080 });
    ui.item(r[0] + 4.0, y + 4.0, &item, 32.0);
    let held = w.held();
    ui.text(r[0] + 48.0, y + 6.0, &format!("Selection tool: {}", short(&item)), 0xFFFFFF);
    let (state, rgb) = match (on, held.as_deref() == Some(item.as_str())) {
        (false, _) => ("Off: clicks with it break and use as usual".to_string(), 0xFF7070),
        (true, true) => ("On, and in your hand: click blocks to set the corners".to_string(), 0x55FF55),
        (true, false) => ("On: take it in your main hand to select".to_string(), 0xFFFF55),
    };
    ui.text(r[0] + 48.0, y + 22.0, &state, rgb);
    y += 48.0;
    ui.text(r[0], y + 2.0, "Item: pick one, type its id, or take the one in your hand", 0xFFFFFF);
    y += 14.0;
    let size = 22.0;
    for (i, it) in TOOL_ITEMS.iter().enumerate() {
        let x = r[0] + i as f32 * (size + 3.0);
        let current = *it == item;
        if ui.button_tip([x, y, size, size], "", !current, &[short(it)]) {
            act(w, &["tool", it]);
        }
        ui.item(x + 3.0, y + 3.0, it, 16.0);
        if current {
            ui.frame([x - 1.0, y - 1.0, size + 2.0, size + 2.0], 0xFFFF_FF55);
        }
    }
    let ex = r[0] + TOOL_ITEMS.len() as f32 * (size + 3.0) + 4.0;
    let (set_w, held_w) = (36.0, 72.0);
    let ew = r[0] + r[2] - ex - set_w - held_w - 8.0;
    let typed = ui.edit("tool_item", [ex, y + 2.0, ew, 18.0], "item id", 64);
    if ui.button([ex + ew + 4.0, y + 1.0, set_w, 20.0], "Set", !typed.trim().is_empty()) {
        act(w, &["tool", typed.trim()]);
        ui.set_edit("tool_item", "");
    }
    if ui.button_tip([r[0] + r[2] - held_w, y + 1.0, held_w, 20.0], "Held item", held.is_some(), &["The item in your main hand becomes the tool"])
        && let Some(h) = &held
    {
        act(w, &["tool", h.as_str()]);
    }
    y += size + 10.0;
    y += settings_grid(ui, w, SELECTION, [r[0], y, r[2], 0.0], 2) + 4.0;
    for line in [
        "Left click: corner 1. Right click: corner 2.",
        "Middle click: pick a corner. Alt+wheel: move it.",
        "Ctrl+wheel: grow or shrink where you look.",
        "/hm save <name> saves the area.",
    ] {
        ui.text(r[0], y, line, 0xA0A0A0);
        y += 11.0;
    }
}

fn tab_printer(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let (on, placed, note, loaded) = runtime::with(|s| (s.settings.printer_on, s.placed, s.printer_note.clone(), !s.slots.is_empty()));
    let label = if on { "Printer: ON (building)" } else { "Printer: OFF" };
    if ui.button_tip([r[0], r[1], r[2], 20.0], label, loaded, &["Places missing blocks within reach (K)"]) {
        act(w, &["printer"]);
    }
    let mut y = r[1] + 26.0;
    if !loaded {
        ui.text(r[0], y, "Load a schematic first.", 0xA0A0A0);
        y += 12.0;
    }
    ui.text(r[0], y, &format!("Placed this session: {placed}"), 0xE0E0E0);
    y += 11.0;
    if !note.is_empty() {
        ui.text_fit(r[0], y, r[2], &note, 0xA0A0A0);
    }
    y += 16.0;
    y += settings_grid(ui, w, PRINTER, [r[0], y, r[2], 0.0], 2) + 4.0;
    ui.text(r[0], y + 2.0, "Easy place", 0xFFFFFF);
    y += 13.0;
    settings_grid(ui, w, EASY_PLACE, [r[0], y, r[2], 0.0], 2);
    ui.text_wrapped(
        r[0],
        r[1] + r[3] - 20.0,
        r[2],
        "Survival: the blocks must be in your hotbar. On servers, automatic placing may break rules or trip anti-cheat.",
        0x808080,
    );
}

fn tab_display(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let (faces, sections, todo) = crate::overlay::stats();
    let (radius, limit) = runtime::with(|s| (runtime::render_radius(w, &s.settings), s.settings.max_faces));
    let full = limit != 0 && faces >= limit;
    let mib = (faces * crate::overlay::QUAD_BYTES) as f64 / (1024.0 * 1024.0);
    let line = format!(
        "Overlay: {faces} faces in {sections} sections, ~{mib:.1} MiB video memory · radius {radius} blocks{}",
        if full { " · FULL".to_string() } else if todo > 0 { format!(" · {todo} to build") } else { String::new() }
    );
    ui.text_fit(r[0], r[1] + r[3] - 22.0, r[2], &line, if full { 0xFFFF55 } else { 0xA0A0A0 });
    ui.text_fit(r[0], r[1] + r[3] - 11.0, r[2], "Beyond the menu: /hm radius <blocks>, /hm faces <n>|none", 0x808080);
    let mut y = r[1];
    for (title, keys) in [("World overlay", DISPLAY), ("HUD", HUD)] {
        ui.text(r[0], y + 2.0, title, 0xFFFFFF);
        y += 13.0;
        y += settings_grid(ui, w, keys, [r[0], y, r[2], 0.0], 2) + 4.0;
    }
}

fn tab_materials(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let list = runtime::material_list(w);
    let hud = runtime::with(|s| s.settings.materials_hud);
    let bw = 150.0;
    if ui.button_tip([r[0] + r[2] - bw, r[1], bw, 20.0], &format!("HUD list: {}", if hud { "ON" } else { "OFF" }), true, &["The list in the corner while playing (J)"]) {
        runtime::with(|s| s.settings.materials_hud = !s.settings.materials_hud);
        runtime::save_settings(w);
    }
    let tip = ["Copies the list as a table: paste it into a spreadsheet"];
    if ui.button_tip([r[0] + r[2] - bw - 74.0, r[1], 70.0, 20.0], "Copy", !list.is_empty(), &tip) {
        act(w, &["materials", "copy"]);
    }
    let fetch: u64 = list.iter().map(|m| m.to_fetch()).sum();
    let summary = if list.is_empty() { "No schematic loaded, or it needs no items.".to_string() } else { format!("{} kinds, {} items still to fetch", list.len(), fetch) };
    ui.text(r[0], r[1] + 6.0, &summary, 0xE0E0E0);
    // The highlights: what they mark, and a row of the list below picks one item.
    let (missing, wrong, only) = runtime::with(|s| (s.settings.highlight_missing, s.settings.highlight_wrong, s.highlight_item.clone()));
    let on_off = |on: bool| if on { "ON" } else { "OFF" };
    ui.text(r[0], r[1] + 30.0, "Highlight", 0xE0E0E0);
    let tip = ["Marks them through everything, however far"];
    if ui.button_tip([r[0] + 52.0, r[1] + 24.0, 86.0, 20.0], &format!("Missing: {}", on_off(missing)), true, &tip) {
        runtime::with(|s| s.settings.highlight_missing = !s.settings.highlight_missing);
        runtime::save_settings(w);
    }
    if ui.button_tip([r[0] + 142.0, r[1] + 24.0, 80.0, 20.0], &format!("Wrong: {}", on_off(wrong)), true, &tip) {
        runtime::with(|s| s.settings.highlight_wrong = !s.settings.highlight_wrong);
        runtime::save_settings(w);
    }
    let shown = only.as_deref().map_or_else(|| "All items".to_string(), runtime::pretty_item);
    if ui.button_tip([r[0] + 226.0, r[1] + 24.0, r[2] - 226.0, 20.0], &shown, only.is_some(), &["Click a row below to mark only that item, again to mark all"]) {
        runtime::with(|s| s.highlight_item = None);
    }
    let table = [r[0], r[1] + 48.0, r[2], r[3] - 48.0];
    let cols = [table[0] + table[2] - 150.0, table[0] + table[2] - 90.0, table[0] + table[2] - 12.0];
    ui.text(table[0] + 24.0, table[1], "Item", 0xFFFFFF);
    ui.text_right(cols[0], table[1], "Missing", 0xFFFFFF);
    ui.text_right(cols[1], table[1], "Total", 0xFFFFFF);
    ui.text_right(cols[2], table[1], "Carried", 0xFFFFFF);
    let area = [table[0], table[1] + 11.0, table[2], table[3] - 11.0];
    ui.fill(area, 0x9000_0000);
    let row_h = 18.0;
    let content = list.len() as f32 * row_h + 2.0;
    let y0 = ui.scroll_begin("materials", area, content);
    let mut save = false;
    for (i, m) in list.iter().enumerate() {
        let y = y0 + 1.0 + i as f32 * row_h;
        if y + row_h < area[1] || y > area[1] + area[3] {
            continue;
        }
        let row = [area[0] + 1.0, y, area[2] - 8.0, row_h];
        if i % 2 == 1 {
            ui.fill(row, 0x18FF_FFFF);
        }
        let name = runtime::pretty_item(&m.item);
        if only.as_deref() == Some(m.item.as_str()) {
            ui.fill(row, 0x5055_FFFF);
        }
        if ui.hovered(row) {
            ui.fill(row, 0x25FF_FFFF);
            ui.tooltip(&[&name, &format!("{} still to fetch", m.to_fetch()), "Click: highlight only this", &m.item]);
        }
        if ui.clicked(row) {
            runtime::with(|s| {
                let again = s.highlight_item.as_deref() == Some(m.item.as_str());
                s.highlight_item = (!again).then(|| m.item.clone());
                if !again && !s.settings.highlight_wrong {
                    s.settings.highlight_missing = true;
                }
            });
            save = true;
        }
        ui.item(row[0] + 2.0, y + 1.0, &m.item, 16.0);
        ui.text_fit(row[0] + 23.0, y + 5.0, cols[0] - row[0] - 80.0, &name, 0xE0E0E0);
        let done = m.to_fetch() == 0;
        ui.text_right(cols[0], y + 5.0, &m.missing.to_string(), if m.missing == 0 { 0x55FF55 } else { 0xFF7070 });
        ui.text_right(cols[1], y + 5.0, &m.total.to_string(), 0xA0A0A0);
        ui.text_right(cols[2], y + 5.0, &m.available.to_string(), if done { 0x55FF55 } else { 0xFFAA00 });
    }
    ui.scroll_end("materials", area, content);
    if save {
        runtime::save_settings(w);
    }
}

fn tab_credits(w: &World<'_>, ui: &mut Ui<'_>, r: Rect) {
    let cx = r[0] + r[2] / 2.0;
    let mut y = r[1] + 4.0;
    ui.text_centered(cx, y, "Horizonmatica re-creates Litematica for Horizon.", 0xFFFFFF);
    y += 12.0;
    ui.text_centered(cx, y, "All credit for the idea and design goes to:", 0xA0A0A0);
    y += 20.0;
    let people: [(&str, &str, &str); 4] = [
        ("Litematica", "by masa (maruohon), maintained by Sakura-Ryoko", "github.com/maruohon/litematica"),
        ("Litematica Printer", "by aleksilassila, maintained by Sakura-Ryoko", "github.com/aleksilassila/litematica-printer"),
        ("Horizon", "by the horizon-rust team", "github.com/horizon-rust/horizon"),
        ("File formats", "Litematica, Sponge/WorldEdit, MCEdit, Mojang", ".litematic .schem .schematic .nbt"),
    ];
    for (what, who, link) in people {
        ui.text_centered(cx, y, what, 0x55FFFF);
        y += 11.0;
        ui.text_centered(cx, y, who, 0xE0E0E0);
        y += 11.0;
        ui.text_centered(cx, y, link, 0x808080);
        y += 17.0;
    }
    ui.text_centered(cx, y, "Not made or endorsed by them. No code copied: format and behaviour re-created.", 0xA0A0A0);
    y += 16.0;
    ui.text_centered(cx, y, "The menu's keys are in Options > Controls (Horizonmatica).", 0x808080);
    if ui.button([cx - 100.0, r[1] + r[3] - 20.0, 200.0, 20.0], "Show the credits in chat", true) {
        action(w, "credits_chat");
    }
}
