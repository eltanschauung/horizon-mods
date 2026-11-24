// The settings screen, an options screen in the game's own look (the title, a scrolling list of
// buttons and sliders two to a row under headings, tooltips, Done). The mod's page in
// Options > Mods opens it with its "Settings: Open..." row, over the Options screen; Mod Menu's
// Configure opens the mod's own screen (`mod_screen_open`), and it shows over that. Done and
// Escape go back where it was opened from. Every change is in effect at once and kept, as the
// game's options are.
//
// The screen hooks only read the game: the files are written at the next tick (`store`).
use crate::runtime::{self as dl, Heading, Kind, Opt, Settings};
use crate::store;
use mod_api::*;
use std::sync::Mutex;

/// GLFW's Shift bit.
const SHIFT: u32 = 1;

/// The list's rows, top to bottom.
#[derive(Clone, Copy)]
enum Row {
    Wide(Opt),
    Pair(Opt, Opt),
    Header(Heading),
    Key,
}

const ROWS: [Row; 11] = [
    Row::Wide(Opt::Enabled),
    Row::Header(Heading::Sources),
    Row::Pair(Opt::Kind(Kind::OwnHands), Opt::Kind(Kind::Players)),
    Row::Pair(Opt::Kind(Kind::Mobs), Opt::Kind(Kind::Dropped)),
    Row::Pair(Opt::Kind(Kind::Burning), Opt::Kind(Kind::Creatures)),
    Row::Pair(Opt::Kind(Kind::Projectiles), Opt::Kind(Kind::Explosives)),
    Row::Header(Heading::Lighting),
    Row::Pair(Opt::Water, Opt::Brightness),
    Row::Pair(Opt::MaxSources, Opt::Range),
    Row::Header(Heading::Controls),
    Row::Key,
];

/// The screen's widgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    Opt(Opt),
    Key,
    KeyReset,
    Done,
}

/// A widget where it is drawn this frame.
#[derive(Clone, Copy)]
struct Placed {
    w: W,
    rect: [f32; 4],
    /// In the scrolling list (clipped to it), not the footer.
    list: bool,
}

/// Text in the list: a heading (centred) or the key row's label.
struct Label {
    text: Text,
    x: f32,
    y: f32,
    centred: bool,
}

/// List geometry: as the game's options screens (header and footer 33 high, rows 25 apart,
/// 310 wide).
const TOP: f32 = 33.0;
const ROW: f32 = 25.0;
/// `AbstractSelectionList`'s scrollbar.
const BAR_W: f32 = 6.0;

struct Layout {
    widgets: Vec<Placed>,
    labels: Vec<Label>,
    top: f32,
    bottom: f32,
    content: f32,
}

/// The list's scroll (`AbstractScrollArea`).
#[derive(Clone, Copy, Default)]
struct Scroll {
    amount: f32,
    /// The scroller is held: the pointer's y and the amount when it was pressed.
    held: Option<(f32, f32)>,
}

impl Scroll {
    fn max(top: f32, bottom: f32, content: f32) -> f32 {
        (content - (bottom - top)).max(0.0)
    }
    fn clamp(&mut self, top: f32, bottom: f32, content: f32) {
        self.amount = self.amount.clamp(0.0, Self::max(top, bottom, content));
    }
    /// The scroller's top and height.
    fn scroller(&self, top: f32, bottom: f32, content: f32) -> (f32, f32) {
        let view = bottom - top;
        let h = (view * view / content.max(1.0)).clamp(32.0, (view - 8.0).max(32.0));
        let max = Self::max(top, bottom, content);
        let y = if max > 0.0 { top + (self.amount / max) * (view - h) } else { top };
        (y, h)
    }
}

#[derive(Default)]
struct State {
    /// The screen is up.
    open: bool,
    /// Over the mod's own screen (Mod Menu's Configure opened it), not the Options screen.
    via_mod_screen: bool,
    /// The GUI's size, as the screen was last drawn.
    size: [f32; 2],
    /// Done or Escape: the screen goes at the next tick.
    close: bool,
    /// The focused widget (a click's or the keyboard's).
    focus: Option<W>,
    /// The focus came from the keyboard: its tooltip shows beside it.
    keyboard: bool,
    scroll: Scroll,
    /// The slider held by the mouse.
    dragging: Option<Opt>,
    /// The key button waits for a key or a mouse button.
    binding: bool,
    /// Mouse buttons whose press the screen took, so their release is taken too.
    taken: u8,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(State::default))
}

fn inside([px, py]: [f32; 2], [x, y, w, h]: [f32; 4]) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}

fn russian(host: &dyn ClientHost) -> bool {
    // The game's language from the mod's own texts: `client_language` is None on the title.
    host.translate("dynamiclights.language").as_deref() == Some("ru")
}

/// The screen is up over `screen`: the Options screen, or the mod's own screen.
fn over(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    let (open, via) = with(|s| (s.open, s.via_mod_screen));
    open && match screen.id {
        "options" => !via,
        "mod" => via && host.mod_screen() == Some(dl::ID),
        _ => false,
    }
}

/// The screen is up: the screen under it is not drawn (`screen_render_pre`), as the game's own
/// sub-screens replace their parent.
pub(crate) fn covers(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    over(host, screen)
}

fn click(host: &dyn ClientHost) {
    host.play_local_sound(&SoundPlay::ui("minecraft:ui.button.click"), 0);
}

fn width(host: &dyn ClientHost, text: &Text) -> f32 {
    host.text_width(text).map_or(0.0, |w| w as f32)
}

/// The GUI's size, as the screen was last drawn.
fn gui_size(_: &ScreenView<'_>) -> [f32; 2] {
    with(|s| s.size)
}

/// The page's row was pressed: the screen opens over the Options screen.
pub(crate) fn setting_changed(_: &mut dyn ClientHost, key: &str) {
    if key == store::CONFIGURE {
        with(|s| *s = State { open: true, size: s.size, ..State::default() });
    }
}

/// Each tick: Mod Menu's Configure opened the mod's own screen, so the settings show over it;
/// the screen closes after Done or Escape (the mod's screen with it), or once the screen under
/// it is gone.
pub(crate) fn tick(host: &mut dyn ClientHost) {
    let mine = host.mod_screen() == Some(dl::ID);
    let options = host.observed_screen_id().is_some_and(|id| id.id == "options");
    let close_mod_screen = with(|s| {
        if mine && !s.open {
            *s = State { open: true, via_mod_screen: true, size: s.size, ..State::default() };
        }
        let gone = if s.via_mod_screen { !mine } else { !options };
        if s.open && (s.close || gone) {
            let via = s.via_mod_screen && mine;
            *s = State { size: s.size, ..State::default() };
            return via;
        }
        false
    });
    if close_mod_screen {
        host.mod_screen_close(dl::ID);
    }
}

// ---- layout -------------------------------------------------------------------------------

fn heading(text: &str) -> Text {
    let mut t = Text::literal(text);
    t.style.bold = Some(true);
    t.style.underlined = Some(true);
    t
}

/// Every widget and label where the list, scrolled by `scroll`, puts them.
fn layout([w, h]: [f32; 2], scroll: f32, ru: bool) -> Layout {
    let cx = (w / 2.0).floor();
    let (top, bottom) = (TOP, h - TOP);
    let mut widgets = Vec::new();
    let mut labels = Vec::new();
    let mut y = top + 2.0 - scroll;
    for (n, row) in ROWS.iter().enumerate() {
        match *row {
            Row::Wide(opt) => {
                widgets.push(Placed { w: W::Opt(opt), rect: [cx - 155.0, y + 2.0, 310.0, 20.0], list: true });
                y += ROW;
            }
            Row::Pair(a, b) => {
                widgets.push(Placed { w: W::Opt(a), rect: [cx - 155.0, y + 2.0, 150.0, 20.0], list: true });
                widgets.push(Placed { w: W::Opt(b), rect: [cx + 5.0, y + 2.0, 150.0, 20.0], list: true });
                y += ROW;
            }
            Row::Header(hd) => {
                // A heading row: room above it unless it starts the list.
                let padding = if n == 0 { 0.0 } else { 18.0 };
                labels.push(Label { text: heading(hd.text(ru)), x: cx, y: y + 2.0 + padding + 4.0, centred: true });
                y += padding + 18.0;
            }
            Row::Key => {
                // As a row of the game's Key Binds list: the name, the key, Reset.
                labels.push(Label { text: Text::literal(dl::key_label(ru)), x: cx - 155.0, y: y + 8.0, centred: false });
                widgets.push(Placed { w: W::Key, rect: [cx + 25.0, y + 2.0, 75.0, 20.0], list: true });
                widgets.push(Placed { w: W::KeyReset, rect: [cx + 105.0, y + 2.0, 50.0, 20.0], list: true });
                y += ROW;
            }
        }
    }
    let content = y + scroll - top + 2.0;
    widgets.push(Placed { w: W::Done, rect: [cx - 100.0, h - 27.0, 200.0, 20.0], list: false });
    Layout { widgets, labels, top, bottom, content }
}

/// The layout with the scroll kept inside the list.
fn current(host: &dyn ClientHost, size: [f32; 2]) -> Layout {
    let ru = russian(host);
    let first = layout(size, with(|s| s.scroll.amount), ru);
    let changed = with(|s| {
        let before = s.scroll.amount;
        s.scroll.clamp(first.top, first.bottom, first.content);
        before != s.scroll.amount
    });
    if changed { layout(size, with(|s| s.scroll.amount), ru) } else { first }
}

/// The widget under the pointer: a list widget only inside the list.
fn under(l: &Layout, p: [f32; 2]) -> Option<Placed> {
    l.widgets.iter().copied().find(|pl| inside(p, pl.rect) && (!pl.list || (p[1] >= l.top && p[1] < l.bottom)))
}

/// The scrollbar's x: right of the list's 310 wide rows.
fn bar_x(w: f32) -> f32 {
    (w / 2.0).floor() + 163.0
}

// ---- drawing ------------------------------------------------------------------------------

fn on_off(on: bool) -> Text {
    Text::translate(if on { "options.on" } else { "options.off" }, Vec::new())
}

/// "Caption: value", as the game's options write it (`Options.genericValueLabel`).
fn option_label(caption: &str, value: Text) -> Text {
    Text::translate("options.generic_value", vec![Text::literal(caption), value])
}

/// Options other than the main switch do nothing while it is off.
fn active(w: W, s: &Settings, key: &str) -> bool {
    match w {
        W::Opt(Opt::Enabled) | W::Done | W::Key => true,
        W::Opt(_) => s.enabled,
        W::KeyReset => key != dl::UNBOUND,
    }
}

/// `AbstractSliderButton`: its track (lit while the keyboard focus is on it but not moving it),
/// the handle 8 wide at the value (lit while hovered or moved by the keyboard), the label
/// centred, white, or grey while inactive.
fn slider(host: &dyn ClientHost, draw: &mut dyn DrawContext, rect: [f32; 4], label: &Text, value: f32, hovered: bool, focused: bool, keyboard: bool, on: bool) {
    let [x, y, w, h] = rect;
    let track = if on && focused && keyboard { "gui/sprites/widget/slider_highlighted" } else { "gui/sprites/widget/slider" };
    let handle = if on && hovered { "gui/sprites/widget/slider_handle_highlighted" } else { "gui/sprites/widget/slider_handle" };
    draw.gui_nine_slice(track, rect, 0xFFFF_FFFF);
    draw.gui_nine_slice(handle, [x + (value * (w - 8.0)).round(), y, 8.0, h], 0xFFFF_FFFF);
    let lw = width(host, label);
    let color = if on { 0xFFFF_FFFF } else { 0xFFA0_A0A0 };
    let clipped = lw > w - 4.0 && draw.push_clip([x + 2.0, y, w - 4.0, h]).is_ok();
    let tx = if lw > w - 4.0 { x + 2.0 } else { x + ((w - lw) / 2.0).trunc() };
    let _ = draw.gui_paint(&[GuiDraw::Text { text: label.clone(), x: tx as i32, y: (y + (h - 8.0) / 2.0) as i32, color, shadow: true }]);
    if clipped {
        let _ = draw.pop_clip();
    }
}

/// The key the toggle is bound to, as its button writes it: "> key <" while it waits.
fn key_message(key: &str, binding: bool) -> Text {
    let name = match dl::key_name(key) {
        dl::KeyName::Literal(s) => Text::literal(s),
        dl::KeyName::Translate(k) => Text::translate(k, Vec::new()),
        dl::KeyName::Button(n) => Text::translate("key.mouse", vec![Text::literal(n.to_string())]),
    };
    if !binding {
        return name;
    }
    let mut name = name;
    name.style.color = Some("white".into());
    name.style.underlined = Some(true);
    let mut out = Text::literal("> ");
    out.style.color = Some("yellow".into());
    let mut end = Text::literal(" <");
    end.style.color = Some("yellow".into());
    out.append(name).append(end)
}

/// A tooltip's lines: its text split at 170 wide, as the game's option tooltips are.
fn tooltip_lines(host: &dyn ClientHost, text: &str) -> Vec<Text> {
    text.split('\n')
        .flat_map(|line| {
            let t = Text::literal(line.to_owned());
            host.text_split(&t, 170).unwrap_or_else(|_| vec![t])
        })
        .collect()
}

/// Draws the screen over the Options screen (`screen_render_post`): its background, the title,
/// the list and Done, then a tooltip.
pub(crate) fn render(host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
    let screen = event.screen;
    if !over(host, screen) {
        return;
    }
    let draw = &mut *event.draw;
    let [w, h] = draw.size();
    with(|s| s.size = [w, h]);
    let in_world = host.local_player().is_some();
    let mouse = screen.mouse;
    let ru = russian(host);
    host.apply_screen_blur();
    draw.draw_menu_background([0.0, 0.0, w, h], in_world);
    let l = current(host, [w, h]);
    let s = store::settings(host);
    let key = store::toggle_key(host);
    let (focus, keyboard, dragging, binding, scroll) = with(|st| (st.focus, st.keyboard, st.dragging, st.binding, st.scroll));
    let hovered = under(&l, mouse);

    // The list's background, the title.
    let list_sprite = if in_world { "gui/inworld_menu_list_background" } else { "gui/menu_list_background" };
    draw.tiled_sprite(list_sprite, [0.0, l.top, w, l.bottom - l.top], 32.0, 0xFFFF_FFFF);
    let title = Text::literal(dl::title(ru));
    let tw = width(host, &title);
    let _ = draw.gui_paint(&[GuiDraw::Text { text: title, x: ((w - tw) / 2.0).floor() as i32, y: 12, color: 0xFFFF_FFFF, shadow: true }]);

    // The list: its labels and widgets, cut to it.
    let clipped = draw.push_clip([0.0, l.top, w, l.bottom - l.top]).is_ok();
    let labels: Vec<GuiDraw> = l
        .labels
        .iter()
        .filter(|lb| lb.y + 9.0 > l.top && lb.y < l.bottom)
        .map(|lb| {
            let x = if lb.centred { (lb.x - width(host, &lb.text) / 2.0).floor() } else { lb.x };
            GuiDraw::Text { text: lb.text.clone(), x: x as i32, y: lb.y as i32, color: 0xFFFF_FFFF, shadow: true }
        })
        .collect();
    let _ = draw.gui_paint(&labels);
    for pl in &l.widgets {
        let [_, y, _, wh] = pl.rect;
        if pl.list && (y >= l.bottom || y + wh <= l.top) {
            continue;
        }
        if !pl.list {
            continue;
        }
        let on = active(pl.w, &s, &key);
        let hover = hovered.is_some_and(|hv| hv.w == pl.w) || focus == Some(pl.w);
        match pl.w {
            W::Opt(opt) if opt.slider() => {
                let label = option_label(opt.caption(ru), Text::literal(s.number_text(opt, ru)));
                let held = dragging == Some(opt);
                let kb = keyboard && focus == Some(pl.w);
                slider(host, draw, pl.rect, &label, s.fraction(opt), hover || held, focus == Some(pl.w), kb, on);
            }
            W::Opt(opt) => {
                let label = option_label(opt.caption(ru), on_off(s.switch(opt).unwrap_or(false)));
                let _ = draw.widget_button(pl.rect, &label, on, on && hover);
            }
            W::Key => {
                let _ = draw.widget_button(pl.rect, &key_message(&key, binding), true, hover);
            }
            W::KeyReset => {
                let _ = draw.widget_button(pl.rect, &Text::translate("controls.reset", Vec::new()), on, on && hover);
            }
            W::Done => {}
        }
    }
    if clipped {
        let _ = draw.pop_clip();
    }

    // The separators, the scrollbar when the list is longer than its room, Done.
    let (header, footer) = if in_world {
        ("gui/inworld_header_separator", "gui/inworld_footer_separator")
    } else {
        ("gui/header_separator", "gui/footer_separator")
    };
    // The separators are 32x2 textures repeated along the width.
    let mut x = 0.0;
    while x < w {
        let part = (w - x).min(32.0);
        draw.sprite(header, [0.0, 0.0, part, 2.0], [x, l.top - 2.0, part, 2.0], None);
        draw.sprite(footer, [0.0, 0.0, part, 2.0], [x, l.bottom, part, 2.0], None);
        x += 32.0;
    }
    if l.content > l.bottom - l.top {
        let (sy, sh) = scroll.scroller(l.top, l.bottom, l.content);
        draw.gui_nine_slice("gui/sprites/widget/scroller_background", [bar_x(w), l.top, BAR_W, l.bottom - l.top], 0xFFFF_FFFF);
        draw.gui_nine_slice("gui/sprites/widget/scroller", [bar_x(w), sy, BAR_W, sh], 0xFFFF_FFFF);
    }
    if let Some(done) = l.widgets.iter().find(|pl| pl.w == W::Done) {
        let hover = hovered.is_some_and(|hv| hv.w == W::Done) || focus == Some(W::Done);
        let _ = draw.widget_button(done.rect, &Text::translate("gui.done", Vec::new()), true, hover);
    }

    // The tooltip: the pointer's option, else the keyboard's.
    let tip_of = |w: W| match w {
        W::Opt(opt) => Some(opt.tooltip(ru)),
        _ => None,
    };
    if dragging.is_none() {
        if let Some(pl) = hovered
            && let Some(tip) = tip_of(pl.w)
        {
            let _ = draw.gui_tooltip(&tooltip_lines(host, tip), mouse);
        } else if keyboard
            && let Some(f) = focus
            && let Some(pl) = l.widgets.iter().find(|pl| pl.w == f)
            && let Some(tip) = tip_of(f)
        {
            // Under the focused widget, as a keyboard's tooltip shows (`MenuTooltipPositioner`):
            // the game places a tooltip 12 right of and 12 above the point it is given.
            let [x, y, _, wh] = pl.rect;
            let _ = draw.gui_tooltip(&tooltip_lines(host, tip), [x + 3.0 - 12.0, y + wh + 4.0 + 12.0]);
        }
    }
}

// ---- input --------------------------------------------------------------------------------

/// Widget `w` pressed (a click, Enter or Space).
fn press(host: &dyn ClientHost, w: W) {
    let mut s = store::settings(host);
    if !active(w, &s, &store::toggle_key(host)) {
        return;
    }
    click(host);
    match w {
        W::Opt(opt) if !opt.slider() => {
            s.flip(opt);
            store::set(s);
        }
        W::Opt(_) => {}
        W::Key => with(|st| st.binding = true),
        W::KeyReset => {
            store::set_toggle_key(dl::UNBOUND);
            with(|st| st.binding = false);
        }
        W::Done => with(|st| st.close = true),
    }
}

/// `AbstractSliderButton.setValueFromMouse`: the handle's middle at the pointer.
fn slider_value(rect: [f32; 4], x: f32) -> f32 {
    ((x - (rect[0] + 4.0)) / (rect[2] - 8.0)).clamp(0.0, 1.0)
}

/// A slider follows the pointer's x.
fn slide(host: &dyn ClientHost, opt: Opt, rect: [f32; 4], x: f32) {
    let mut s = store::settings(host);
    if s.set_fraction(opt, slider_value(rect, x)) {
        store::set(s);
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// A mouse button's key name, as the game names it.
fn mouse_key(button: i32) -> String {
    match button {
        0 => "key.mouse.left".to_owned(),
        1 => "key.mouse.right".to_owned(),
        2 => "key.mouse.middle".to_owned(),
        n => format!("key.mouse.{}", n + 1),
    }
}

/// A mouse button pressed over the screen: true when it took it.
pub(crate) fn pressed(host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> bool {
    if !over(host, screen) {
        return false;
    }
    with(|s| s.taken |= bit(button));
    // The key button waits: any mouse button is the new key.
    if with(|s| std::mem::take(&mut s.binding)) {
        store::set_toggle_key(&mouse_key(button));
        return true;
    }
    if button != 0 {
        return true;
    }
    let p = screen.mouse;
    let size = gui_size(screen);
    let l = current(host, size);
    // The scrollbar: its scroller is held, or a press beside it jumps there.
    let bx = bar_x(size[0]);
    if l.content > l.bottom - l.top && p[0] >= bx && p[0] < bx + BAR_W && p[1] >= l.top && p[1] < l.bottom {
        with(|s| {
            let (sy, sh) = s.scroll.scroller(l.top, l.bottom, l.content);
            if p[1] < sy || p[1] >= sy + sh {
                let max = Scroll::max(l.top, l.bottom, l.content);
                let view = l.bottom - l.top;
                s.scroll.amount = ((p[1] - l.top - sh / 2.0) / (view - sh)).clamp(0.0, 1.0) * max;
            }
            s.scroll.held = Some((p[1], s.scroll.amount));
        });
        return true;
    }
    match under(&l, p) {
        Some(pl) => {
            with(|s| {
                s.focus = Some(pl.w);
                s.keyboard = false;
            });
            if let W::Opt(opt) = pl.w
                && opt.slider()
            {
                if active(pl.w, &store::settings(host), "") {
                    with(|s| s.dragging = Some(opt));
                    slide(host, opt, pl.rect, p[0]);
                }
            } else {
                press(host, pl.w);
            }
        }
        None => with(|s| s.focus = None),
    }
    true
}

/// A mouse button let go: taken when its press was. A slider let go clicks.
pub(crate) fn released(host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> bool {
    if screen.id != "options" && screen.id != "mod" {
        return false;
    }
    let (was, slid) = with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        let mut slid = false;
        if button == 0 {
            s.scroll.held = None;
            slid = s.dragging.take().is_some();
        }
        (was, slid)
    });
    if slid {
        click(host);
    }
    was
}

/// The pointer dragged: the held slider or scrollbar follows it.
pub(crate) fn dragged(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    if !over(host, screen) {
        return false;
    }
    let p = screen.mouse;
    let l = current(host, gui_size(screen));
    if let Some((y0, a0)) = with(|s| s.scroll.held) {
        with(|s| {
            let (_, sh) = s.scroll.scroller(l.top, l.bottom, l.content);
            let max = Scroll::max(l.top, l.bottom, l.content);
            let room = (l.bottom - l.top - sh).max(1.0);
            s.scroll.amount = (a0 + (p[1] - y0) * max / room).clamp(0.0, max);
        });
    } else if let Some(opt) = with(|s| s.dragging)
        && let Some(pl) = l.widgets.iter().find(|pl| pl.w == W::Opt(opt))
    {
        slide(host, opt, pl.rect, p[0]);
    }
    true
}

/// The wheel scrolls the list (half a row a step).
pub(crate) fn scrolled(host: &dyn ClientHost, screen: &ScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(host, screen) {
        return false;
    }
    let l = current(host, gui_size(screen));
    with(|s| {
        s.scroll.amount -= scroll[1] as f32 * ROW / 2.0;
        s.scroll.clamp(l.top, l.bottom, l.content);
    });
    true
}

/// Scrolls the list so that the focused widget shows.
fn show_focus(host: &dyn ClientHost, size: [f32; 2]) {
    let l = current(host, size);
    let Some(f) = with(|s| s.focus) else { return };
    let Some(pl) = l.widgets.iter().find(|pl| pl.w == f && pl.list) else { return };
    let [_, y, _, h] = pl.rect;
    with(|s| {
        if y < l.top + 2.0 {
            s.scroll.amount -= l.top + 2.0 - y;
        } else if y + h > l.bottom - 2.0 {
            s.scroll.amount += y + h - (l.bottom - 2.0);
        }
        s.scroll.clamp(l.top, l.bottom, l.content);
    });
}

/// The nearest widget from `from` in direction `(dx, dy)`, as the arrows move the focus.
fn step(rects: &[[f32; 4]], from: usize, dx: f32, dy: f32) -> Option<usize> {
    let c = |r: [f32; 4]| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
    let [fx, fy] = c(rects[from]);
    rects
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != from)
        .filter_map(|(i, r)| {
            let [x, y] = c(*r);
            let (ax, ay) = (x - fx, y - fy);
            let along = ax * dx + ay * dy;
            let across = (ax * dy - ay * dx).abs();
            (along > 0.5).then_some((i, along + across * 2.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// A key: while the key button waits, the new key (Escape: none); else Escape closes, Tab and
/// the arrows move the focus, Enter and Space press, left and right move a focused slider.
/// Every key is the screen's.
pub(crate) fn key_pressed(host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(host, screen) {
        return false;
    }
    if with(|s| std::mem::take(&mut s.binding)) {
        store::set_toggle_key(if key == "key.keyboard.escape" { dl::UNBOUND } else { key });
        return true;
    }
    let shift = modifiers & SHIFT != 0;
    let size = gui_size(screen);
    let l = current(host, size);
    let order: Vec<W> = l.widgets.iter().map(|pl| pl.w).collect();
    let rects: Vec<[f32; 4]> = l.widgets.iter().map(|pl| pl.rect).collect();
    let focus = with(|s| s.focus);
    let index = focus.and_then(|f| order.iter().position(|w| *w == f));
    let set_focus = |i: usize| {
        with(|s| {
            s.focus = Some(order[i]);
            s.keyboard = true;
        });
        show_focus(host, size);
    };
    match key {
        "key.keyboard.escape" => with(|s| s.close = true),
        "key.keyboard.tab" => {
            let n = order.len();
            set_focus(match index {
                None if shift => n - 1,
                None => 0,
                Some(i) if shift => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
            });
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" | "key.keyboard.space" => {
            if let Some(w) = focus {
                press(host, w);
            }
        }
        "key.keyboard.left" | "key.keyboard.right"
            if matches!(focus, Some(W::Opt(o)) if o.slider()) && with(|s| s.keyboard) =>
        {
            if let Some(W::Opt(opt)) = focus {
                let mut s = store::settings(host);
                if s.enabled && s.step(opt, key == "key.keyboard.right") {
                    store::set(s);
                }
            }
        }
        _ => {
            let dir = match key {
                "key.keyboard.left" => Some((-1.0, 0.0)),
                "key.keyboard.right" => Some((1.0, 0.0)),
                "key.keyboard.up" => Some((0.0, -1.0)),
                "key.keyboard.down" => Some((0.0, 1.0)),
                _ => None,
            };
            if let Some((dx, dy)) = dir {
                let next = match index {
                    None => Some(0),
                    Some(i) => step(&rects, i, dx, dy),
                };
                if let Some(next) = next {
                    set_focus(next);
                }
            }
        }
    }
    true
}

/// A key let go, or a character typed: nothing under the screen hears it.
pub(crate) fn swallowed(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    over(host, screen)
}
