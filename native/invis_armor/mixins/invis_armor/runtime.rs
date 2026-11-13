// SPDX-License-Identifier: MIT
use super::Game;
use horizon_client::{hud::HudBatch, text::Text, ui};
use std::cell::RefCell;
pub(super) const ID: &str = "invis_armor";
#[derive(Clone, Copy)]
struct Config {
    opacity: u8,
    hats: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            opacity: 100,
            hats: true,
        }
    }
}
#[derive(Default)]
struct Page {
    open: bool,
    drag: bool,
    focus: usize,
    rects: [[f32; 4]; 3],
}
thread_local! {
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
    static PAGE: RefCell<Page> = RefCell::new(Page::default());
}
fn config(game: &Game<'_>) -> Config {
    CONFIG.with_borrow_mut(|c| {
        *c.get_or_insert_with(|| Config {
            opacity: game
                .native_setting(ID, "opacity")
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|v| *v <= 100)
                .unwrap_or(100),
            hats: game.native_setting(ID, "hats_only").as_deref() != Some("false"),
        })
    })
}
fn save(game: &Game<'_>) {
    let c = config(game);
    game.native_set_setting(ID, "opacity", &c.opacity.to_string());
    game.native_set_setting(ID, "hats_only", if c.hats { "true" } else { "false" });
}
pub(super) fn opacity(game: &Game<'_>, slot: &str) -> f32 {
    if !game.native_on(ID) {
        return 1.0;
    }
    let c = config(game);
    if c.hats && slot != "helmet" {
        0.0
    } else {
        f32::from(c.opacity) / 100.0
    }
}
pub(super) fn open(game: &Game<'_>) {
    config(game);
    PAGE.with_borrow_mut(|p| {
        *p = Page {
            open: true,
            ..Page::default()
        }
    });
}
fn hit(r: [f32; 4], m: [f32; 2]) -> bool {
    m[0] >= r[0] && m[0] < r[0] + r[2] && m[1] >= r[1] && m[1] < r[1] + r[3]
}
fn slide(r: [f32; 4], mouse: f32) {
    let value = (((mouse - r[0] - 4.0) / (r[2] - 8.0)).clamp(0.0, 1.0) * 100.0).round() as u8;
    CONFIG.with_borrow_mut(|c| {
        if let Some(c) = c {
            c.opacity = value;
        }
    });
}
fn activate(game: &Game<'_>, p: &mut Page, i: usize) {
    match i {
        1 => {
            CONFIG.with_borrow_mut(|c| {
                if let Some(c) = c {
                    c.hats = !c.hats;
                }
            });
            save(game);
        }
        2 => {
            save(game);
            p.open = false;
            p.drag = false;
        }
        _ => {}
    }
}
pub(super) fn draw(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &super::native_hooks::NativeScreenView<'_>,
) {
    PAGE.with_borrow_mut(|p| {
        if !p.open || screen.id != "options" {
            return;
        }
        let w = batch.width.min(240.0).max(40.0) - 20.0;
        let x = (batch.width - w) / 2.0;
        let y = (batch.height / 2.0 - 35.0).max(30.0);
        p.rects = [
            [x, y, w, 20.0],
            [x, y + 26.0, w, 20.0],
            [x, y + 64.0, w, 20.0],
        ];
        if p.drag {
            slide(p.rects[0], screen.mouse[0]);
        }
        let c = config(game);
        let atlas = &game.gui.atlas;
        batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, 0xff101010);
        let label = Text::literal("Invisible Armor");
        batch.text_styled(
            atlas,
            &label,
            (batch.width - game.native_text_width("Invisible Armor")) / 2.0,
            y - 22.0,
            u32::MAX,
            true,
        );
        let r = p.rects[0];
        ui::slider_ext(
            batch,
            atlas,
            r[0],
            r[1],
            r[2],
            &Text::literal(format!("Opacity: {}%", c.opacity)),
            f32::from(c.opacity) / 100.0,
            hit(r, screen.mouse),
            p.focus == 0,
            p.focus == 0,
            true,
            p.drag,
        );
        for (i, label) in [
            (
                1,
                format!("Hats Only: {}", if c.hats { "On" } else { "Off" }),
            ),
            (2, "Done".into()),
        ] {
            let r = p.rects[i];
            ui::button(
                batch,
                atlas,
                r[0],
                r[1],
                r[2],
                r[3],
                &Text::literal(label),
                true,
                hit(r, screen.mouse) || p.focus == i,
            );
        }
    });
}
pub(super) fn pressed(
    game: &Game<'_>,
    screen: &super::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    PAGE.with_borrow_mut(|p| {
        if !p.open || screen.id != "options" {
            return;
        }
        *canceled = true;
        if button != 0 {
            return;
        }
        if let Some(i) = p.rects.iter().position(|r| hit(*r, screen.mouse)) {
            p.focus = i;
            if i == 0 {
                p.drag = true;
                slide(p.rects[0], screen.mouse[0]);
            } else {
                activate(game, p, i);
            }
        }
    });
}
pub(super) fn released(
    game: &Game<'_>,
    screen: &super::native_hooks::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    PAGE.with_borrow_mut(|p| {
        if !p.open || screen.id != "options" {
            return;
        }
        *canceled = true;
        if p.drag {
            slide(p.rects[0], screen.mouse[0]);
            p.drag = false;
            save(game);
        }
    });
}
pub(super) fn key(
    game: &Game<'_>,
    screen: &super::native_hooks::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    PAGE.with_borrow_mut(|p| {
        if !p.open || screen.id != "options" {
            return;
        }
        *canceled = true;
        match key {
            "key.keyboard.escape" | "escape" => activate(game, p, 2),
            "key.keyboard.tab" | "tab" => {
                p.focus = (p.focus + if modifiers & 1 != 0 { 2 } else { 1 }) % 3;
            }
            "key.keyboard.enter" | "key.keyboard.space" | "enter" | "space" => {
                activate(game, p, p.focus)
            }
            "key.keyboard.left" | "key.keyboard.right" | "left" | "right" if p.focus == 0 => {
                let step = if key.ends_with("left") { -1 } else { 1 };
                CONFIG.with_borrow_mut(|c| {
                    if let Some(c) = c {
                        c.opacity = (i32::from(c.opacity) + step).clamp(0, 100) as u8;
                    }
                });
                save(game);
            }
            "key.keyboard.home" | "key.keyboard.end" if p.focus == 0 => {
                CONFIG.with_borrow_mut(|c| {
                    if let Some(c) = c {
                        c.opacity = if key.ends_with("home") { 0 } else { 100 };
                    }
                });
                save(game);
            }
            _ => {}
        }
    });
}
pub(super) fn frame(game: &Game<'_>) {
    PAGE.with_borrow_mut(|p| {
        if p.open && game.screen != super::Screen::Options {
            save(game);
            *p = Page::default();
        }
    });
}

pub(super) fn block_input(screen: &super::native_hooks::NativeScreenView<'_>, canceled: &mut bool) {
    if screen.id == "options" && PAGE.with_borrow(|p| p.open) {
        *canceled = true;
    }
}
