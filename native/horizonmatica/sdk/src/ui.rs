//! Immediate-mode widgets for the mod's own screen (the menu, `menu.rs`), drawn with the mod
//! API's `DrawContext`: each frame the screen is drawn top to bottom, and each widget answers the
//! input that came in since the last frame (the screen hooks queue it with `on_*`).
//!
//! Buttons are the game's (`widget_button`), edit boxes the API's `TextField`s (with the game's
//! editing keys and clipboard), sliders the game's slider sprites.

use mod_api::client::widgets::{Outcome, TextField, WidgetId, Widgets};
use mod_api::{ClientHost, DrawContext, Text};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// A rectangle in GUI pixels: x, y, width, height.
pub type Rect = [f32; 4];

/// GLFW's modifier bits.
const SHIFT: u32 = 1;

fn inside(p: [f32; 2], r: Rect) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

/// What the widgets keep between frames, and the input queued for the next one.
#[derive(Default)]
struct Kept {
    /// Left presses since the last frame.
    presses: Vec<[f32; 2]>,
    mouse_down: bool,
    released: bool,
    wheel: f32,
    /// Keys pressed that no edit box took (`key.keyboard.1`...).
    keys: Vec<String>,
    modifiers: u32,
    /// The slider being dragged.
    held: Option<String>,
    scroll: HashMap<String, f32>,
    /// The edit boxes, by id.
    fields: Widgets,
    ids: HashMap<String, WidgetId>,
}

thread_local! {
    static KEPT: RefCell<Kept> = RefCell::new(Kept::default());
}

fn kept<R>(f: impl FnOnce(&mut Kept) -> R) -> R {
    KEPT.with_borrow_mut(f)
}

// ---------------------------------------------------------------- input (from the screen hooks)

/// A mouse button went down at `at`.
pub fn on_press(host: &dyn ClientHost, at: [f32; 2], button: i32, modifiers: u32) {
    kept(|k| {
        k.modifiers = modifiers;
        // The edit boxes take a press on them (focus, cursor); one beside them leaves them.
        let taken = k.fields.mouse_pressed(host, at, button, modifiers);
        if button == 0 {
            k.mouse_down = true;
            if !matches!(taken, Outcome::Handled) {
                k.presses.push(at);
            }
        }
    });
}

pub fn on_release(button: i32) {
    kept(|k| {
        k.fields.mouse_released(button);
        if button == 0 {
            k.mouse_down = false;
            k.released = true;
        }
    });
}

pub fn on_drag(host: &dyn ClientHost, at: [f32; 2], button: i32) {
    kept(|k| {
        k.fields.mouse_dragged(host, at, button);
    });
}

pub fn on_scroll(dy: f64) {
    kept(|k| k.wheel += dy as f32);
}

/// A key went down: the focused edit box takes it, else the screen's shortcuts get it.
/// While an edit box has the focus, only Enter reaches the screen: printable keys arrive as
/// "ignored" here (their text comes through `on_char`), so they must not fire shortcuts too.
pub fn on_key(host: &dyn ClientHost, key: &str, modifiers: u32) {
    kept(|k| {
        k.modifiers = modifiers;
        if k.fields.focused().is_some() {
            if let Outcome::Submitted(_) = k.fields.key_pressed(host, key, modifiers) {
                k.keys.push("key.keyboard.enter".into());
            }
        } else {
            k.keys.push(key.to_string());
        }
    });
}

pub fn on_char(host: &dyn ClientHost, c: char) {
    kept(|k| {
        k.fields.char_typed(host, c);
    });
}

/// The screen closed: input in flight is dropped, the edit boxes leave the focus.
pub fn on_close(host: &dyn ClientHost) {
    kept(|k| {
        k.presses.clear();
        k.keys.clear();
        k.mouse_down = false;
        k.held = None;
        k.fields.set_focus(host, None);
    });
}

// ---------------------------------------------------------------- one frame

/// One frame of the screen: draw widgets top to bottom; each answers this frame's input.
pub struct Ui<'a> {
    pub draw: &'a mut dyn DrawContext,
    pub host: &'a dyn ClientHost,
    pub width: f32,
    pub height: f32,
    pub cursor: [f32; 2],
    /// Shift held (buttons may do more, sliders and choices step back).
    pub shift: bool,
    presses: Vec<[f32; 2]>,
    mouse_down: bool,
    released: bool,
    wheel: f32,
    keys: Vec<String>,
    clip: Option<Rect>,
    tooltip: Vec<String>,
    close: bool,
    /// Edit boxes drawn this frame.
    used: HashSet<String>,
}

impl<'a> Ui<'a> {
    pub fn begin(draw: &'a mut dyn DrawContext, host: &'a dyn ClientHost, cursor: [f32; 2]) -> Self {
        let [width, height] = draw.size();
        let ui = kept(|k| Ui {
            draw,
            host,
            width,
            height,
            cursor,
            shift: k.modifiers & SHIFT != 0,
            presses: std::mem::take(&mut k.presses),
            mouse_down: k.mouse_down,
            released: std::mem::take(&mut k.released),
            wheel: std::mem::take(&mut k.wheel),
            keys: std::mem::take(&mut k.keys),
            clip: None,
            tooltip: Vec::new(),
            close: false,
            used: HashSet::new(),
        });
        ui.draw.draw_menu_background([0.0, 0.0, ui.width, ui.height], true);
        ui
    }

    /// Ends the frame: the edit boxes and the tooltip go on top. Whether the screen asked to
    /// close.
    pub fn end(mut self) -> bool {
        let used = std::mem::take(&mut self.used);
        let mouse = self.cursor;
        kept(|k| {
            // Edit boxes not drawn this frame (another tab) are moved away and take nothing.
            for (id, wid) in &k.ids {
                if !used.contains(id)
                    && let Some(f) = k.fields.text_field_mut(*wid)
                {
                    f.rect = [-10_000.0, -10_000.0, 1.0, 1.0];
                    f.active = false;
                }
            }
            let _ = k.fields.draw(self.draw, self.host, mouse);
            if self.released || !self.mouse_down {
                k.held = None;
            }
        });
        if !self.tooltip.is_empty() {
            let lines: Vec<Text> = self.tooltip.iter().map(|l| Text::literal(l.as_str())).collect();
            let _ = self.draw.gui_tooltip(&lines, self.cursor);
        }
        self.close
    }

    /// The pointer is over `r` (and inside the scroll area being drawn).
    pub fn hovered(&self, r: Rect) -> bool {
        inside(self.cursor, r) && self.clip.is_none_or(|c| inside(self.cursor, c))
    }

    /// A press this frame inside `r` (taken: one widget gets it).
    fn take_press(&mut self, r: Rect) -> bool {
        let clip = self.clip;
        if let Some(i) = self.presses.iter().position(|&p| inside(p, r) && clip.is_none_or(|c| inside(p, c))) {
            self.presses.remove(i);
            return true;
        }
        false
    }

    /// A press inside `r` this frame (for rows and areas that aren't buttons).
    pub fn clicked(&mut self, r: Rect) -> bool {
        self.take_press(r)
    }

    /// A key pressed this frame that no edit box took (`key.keyboard.m`).
    pub fn key(&mut self, key: &str) -> bool {
        if let Some(i) = self.keys.iter().position(|k| k == key) {
            self.keys.remove(i);
            return true;
        }
        false
    }

    /// Closes the screen after this frame.
    pub fn close(&mut self) {
        self.close = true;
    }

    /// Lines shown beside the pointer at the end of the frame.
    pub fn tooltip(&mut self, lines: &[&str]) {
        self.tooltip = lines.iter().map(|l| l.to_string()).collect();
    }

    pub fn fill(&mut self, r: Rect, argb: u32) {
        self.draw.fill(r, argb);
    }

    /// An outline one pixel wide.
    pub fn frame(&mut self, r: Rect, argb: u32) {
        let [x, y, w, h] = r;
        self.fill([x, y, w, 1.0], argb);
        self.fill([x, y + h - 1.0, w, 1.0], argb);
        self.fill([x, y, 1.0, h], argb);
        self.fill([x + w - 1.0, y, 1.0, h], argb);
    }

    pub fn text_width(&self, text: &str) -> f32 {
        self.draw.text_width(text)
    }

    /// Text with a shadow; `rgb` like 0xFFFFFF.
    pub fn text(&mut self, x: f32, y: f32, text: &str, rgb: u32) {
        self.draw.text(text, [x, y], 0xFF00_0000 | rgb, true);
    }

    /// Text cut to `width` with "...".
    pub fn text_fit(&mut self, x: f32, y: f32, width: f32, text: &str, rgb: u32) {
        let shown = self.fit(text, width);
        self.text(x, y, &shown, rgb);
    }

    pub fn fit(&self, text: &str, width: f32) -> String {
        if self.text_width(text) <= width {
            return text.to_string();
        }
        let mut out = String::new();
        for c in text.chars() {
            out.push(c);
            if self.text_width(&format!("{out}...")) > width {
                out.pop();
                break;
            }
        }
        format!("{out}...")
    }

    pub fn text_right(&mut self, right: f32, y: f32, text: &str, rgb: u32) {
        let w = self.text_width(text);
        self.text(right - w, y, text, rgb);
    }

    pub fn text_centered(&mut self, cx: f32, y: f32, text: &str, rgb: u32) {
        let w = self.text_width(text);
        self.text((cx - w / 2.0).floor(), y, text, rgb);
    }

    /// Text broken into lines of at most `width`; returns the height used (10 per line).
    pub fn text_wrapped(&mut self, x: f32, y: f32, width: f32, text: &str, rgb: u32) -> f32 {
        let mut line = String::new();
        let mut dy = 0.0;
        for word in text.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && self.text_width(&candidate) > width {
                self.text(x, y + dy, &line, rgb);
                dy += 10.0;
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            self.text(x, y + dy, &line, rgb);
            dy += 10.0;
        }
        dy
    }

    /// The game's button; true when clicked.
    pub fn button(&mut self, r: Rect, label: &str, active: bool) -> bool {
        let hover = self.hovered(r);
        let _ = self.draw.widget_button(r, &Text::literal(label), active, active && hover);
        let clicked = active && self.take_press(r);
        if clicked {
            let _ = self.host.play_local_sound(&mod_api::SoundPlay::ui("minecraft:ui.button.click"), 0);
        }
        clicked
    }

    /// A button with a tooltip.
    pub fn button_tip(&mut self, r: Rect, label: &str, active: bool, tip: &[&str]) -> bool {
        if self.hovered(r) {
            self.tooltip(tip);
        }
        self.button(r, label, active)
    }

    /// The game's slider at `value` (0 to 1), 20 high. Some(new value) while dragged.
    pub fn slider(&mut self, id: &str, r: Rect, label: &str, value: f32) -> Option<f32> {
        let hover = self.hovered(r);
        if self.take_press(r) {
            kept(|k| k.held = Some(id.to_string()));
        }
        let dragging = kept(|k| k.held.as_deref() == Some(id));
        let new = (dragging && (self.mouse_down || self.released)).then(|| ((self.cursor[0] - (r[0] + 4.0)) / (r[2] - 8.0)).clamp(0.0, 1.0));
        let shown = new.unwrap_or(value);
        self.draw.gui_nine_slice("gui/sprites/widget/slider", r, 0xFFFF_FFFF);
        let handle = if hover || dragging { "gui/sprites/widget/slider_handle_highlighted" } else { "gui/sprites/widget/slider_handle" };
        let hx = (r[0] + shown * (r[2] - 8.0)).floor();
        self.draw.gui_nine_slice(handle, [hx, r[1], 8.0, r[3]], 0xFFFF_FFFF);
        let w = self.text_width(label);
        let tx = (r[0] + (r[2] - w) / 2.0).floor();
        self.draw.text(label, [tx, r[1] + (r[3] - 8.0) / 2.0], 0xFFFF_FFFF, true);
        new.filter(|v| (v - value).abs() > f32::EPSILON)
    }

    /// The edit box `id`; its text. `hint` shows while it is empty and not focused.
    pub fn edit(&mut self, id: &str, r: Rect, hint: &str, max_length: usize) -> String {
        self.used.insert(id.to_string());
        // Out of the scroll area being drawn: not shown (it would draw over what is around).
        let visible = self.clip.is_none_or(|c| r[1] >= c[1] && r[1] + r[3] <= c[1] + c[3]);
        kept(|k| {
            let wid = match k.ids.get(id) {
                Some(w) => *w,
                None => {
                    let wid = k.fields.add_text_field(TextField::new(r, max_length));
                    k.ids.insert(id.to_string(), wid);
                    wid
                }
            };
            let focused = k.fields.focused() == Some(wid);
            let Some(f) = k.fields.text_field_mut(wid) else { return String::new() };
            f.rect = if visible { r } else { [-10_000.0, -10_000.0, 1.0, 1.0] };
            f.active = visible;
            f.suggestion = (f.value().is_empty() && !focused).then(|| hint.to_string());
            f.value().to_string()
        })
    }

    /// Replaces an edit box's text (when the mod changes what it shows).
    pub fn set_edit(&mut self, id: &str, value: &str) {
        kept(|k| {
            let wid = match k.ids.get(id) {
                Some(w) => *w,
                None => {
                    let wid = k.fields.add_text_field(TextField::new([-10_000.0, -10_000.0, 1.0, 1.0], 256));
                    k.ids.insert(id.to_string(), wid);
                    wid
                }
            };
            if let Some(f) = k.fields.text_field_mut(wid) {
                f.set_value(value);
            }
        });
    }

    pub fn focused(&self, id: &str) -> bool {
        kept(|k| k.ids.get(id).is_some_and(|w| k.fields.focused() == Some(*w)))
    }

    /// An item's icon, `size` pixels (16 is the inventory's).
    pub fn item(&mut self, x: f32, y: f32, item: &str, size: f32) -> bool {
        let Some(key) = mod_api::Identifier::parse(item).ok().and_then(|id| self.host.item_key(&id)) else { return false };
        self.draw.item(key, [x, y], size)
    }

    /// Tabs across `r`: the current one is shown pressed. The tab clicked, if any.
    pub fn tabs(&mut self, r: Rect, labels: &[&str], current: usize) -> Option<usize> {
        let n = labels.len().max(1) as f32;
        let w = ((r[2] - (n - 1.0) * 2.0) / n).floor();
        let mut clicked = None;
        for (i, label) in labels.iter().enumerate() {
            let x = r[0] + i as f32 * (w + 2.0);
            if self.button([x, r[1], w, r[3]], label, i != current) {
                clicked = Some(i);
            }
        }
        clicked
    }

    /// Starts a scrolled area `id` over `r` with `content` pixels of rows: draw them from the
    /// returned top (y of the first row). The wheel scrolls it while the pointer is over it.
    pub fn scroll_begin(&mut self, id: &str, r: Rect, content: f32) -> f32 {
        let max = (content - r[3]).max(0.0);
        let mut offset = kept(|k| k.scroll.get(id).copied().unwrap_or(0.0));
        if self.wheel != 0.0 && inside(self.cursor, r) {
            offset -= self.wheel * 12.0;
            self.wheel = 0.0;
        }
        // A press on the scroll bar's column jumps there.
        let bar = [r[0] + r[2] - 6.0, r[1], 6.0, r[3]];
        if max > 0.0 && self.take_press(bar) {
            offset = (self.cursor[1] - r[1]) / r[3] * content - r[3] / 2.0;
        }
        offset = offset.clamp(0.0, max);
        kept(|k| k.scroll.insert(id.to_string(), offset));
        self.clip = Some(r);
        let _ = self.draw.push_clip(r);
        r[1] - offset
    }

    /// Ends the area: what was drawn is cut to it, and its scroll bar drawn.
    pub fn scroll_end(&mut self, id: &str, r: Rect, content: f32) {
        let _ = self.draw.pop_clip();
        self.clip = None;
        let offset = kept(|k| k.scroll.get(id).copied().unwrap_or(0.0));
        if content > r[3] {
            let x = r[0] + r[2] - 6.0;
            self.fill([x, r[1], 6.0, r[3]], 0xFF00_0000);
            let h = (r[3] * r[3] / content).max(32.0).min(r[3]);
            let y = r[1] + (r[3] - h) * (offset / (content - r[3]));
            self.fill([x, y, 6.0, h], 0xFF80_8080);
            self.fill([x, y, 5.0, h - 1.0], 0xFFC0_C0C0);
        }
    }
}
