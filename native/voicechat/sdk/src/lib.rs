// SPDX-License-Identifier: MIT
pub mod engine;

use engine as voice;
use mod_api::*;
use std::sync::atomic::{AtomicBool, Ordering};

static CONNECTED_WORLD: AtomicBool = AtomicBool::new(false);

pub fn set_connected_world(connected: bool) {
    CONNECTED_WORLD.store(connected, Ordering::Relaxed);
}

fn connected_world() -> bool {
    CONNECTED_WORLD.load(Ordering::Relaxed)
}

/// SDK choices are stored as indices. Read the old strings as well so an
/// existing preference still works before the player opens the new controls.
pub fn from_setting(key: &str, raw: Option<String>) -> Option<String> {
    raw.map(|raw| match (key, raw.as_str()) {
        ("mode", "0") => "ptt".into(),
        ("mode", "1") => "voice".into(),
        ("protocol", "0") => "auto".into(),
        ("protocol", "1") => "2.6".into(),
        ("protocol", "2") => "2.7".into(),
        _ => raw,
    })
}

pub fn to_setting<'a>(key: &str, raw: &'a str) -> &'a str {
    match (key, raw) {
        ("mode", "ptt") | ("protocol", "auto") => "0",
        ("mode", "voice") | ("protocol", "2.6") => "1",
        ("protocol", "2.7") => "2",
        _ => raw,
    }
}

fn read_settings(host: &dyn ClientHost) {
    voice::load(&|key| from_setting(key, host.setting(voice::ID, key)));
}

fn save(host: &dyn ClientHost) {
    for (key, value) in voice::take_store() {
        host.set_setting(voice::ID, &key, to_setting(&key, &value));
    }
}

fn binding(host: &dyn ClientHost, name: &str, default: &str) -> String {
    host.mod_key(voice::ID, name)
        .map_or_else(|| default.to_owned(), |(key, _)| key)
}

#[derive(Default)]
struct VoiceChat;

#[mod_api::horizon_mod]
impl Hooks for VoiceChat {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        for (key, _, default) in voice::HOTKEYS {
            registrar.register_key_mapping(
                KeyRegistration::new(key, default).label(&format!("voicechat.{key}")),
            );
        }
        for (key, default) in [
            ("muted", false),
            ("disabled", false),
            ("hud", true),
            ("group_labels", true),
            ("hide_tooltip", true),
        ] {
            let _ = registrar.register_setting(SettingSpec::toggle(
                key,
                format!("voicechat.{key}"),
                default,
            ));
        }
        let _ = registrar.register_setting(SettingSpec::choice(
            "mode",
            "voicechat.mode",
            ["voicechat.ptt", "voicechat.voice"],
            0,
        ));
        let _ = registrar.register_setting(SettingSpec::choice(
            "protocol",
            "voicechat.protocol",
            ["voicechat.auto", "voicechat.v26", "voicechat.v27"],
            0,
        ));
        for (key, min, max, default) in [
            ("volume", 0.0, 200.0, 100.0),
            ("mic_gain", 10.0, 500.0, 100.0),
            ("threshold", -127.0, 0.0, -50.0),
        ] {
            let _ = registrar.register_setting(SettingSpec::range(
                key,
                format!("voicechat.{key}"),
                min,
                max,
                1.0,
                default,
            ));
        }
    }

    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        let corner = Corner::TopLeft;
        if !connected_world() || !host.on(voice::ID) || !host.native_corner_shown(corner) {
            return;
        }
        let ptt = binding(host, voice::KEY_PTT, voice::DEFAULT_PTT);
        for line in voice::hud(&ptt) {
            let width = event.draw.text_width(&line.text) + 4.0;
            let (x, y) = event.corners.place(corner, width, 12.0);
            event.draw.fill([x, y, width, 12.0], 0x9000_0000);
            event.draw.text(&line.text, [x + 2.0, y + 2.0], line.rgb, true);
        }
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        voice::reload(&|key| from_setting(key, host.setting(voice::ID, key)));
    }

    fn client_packet(&self, host: &mut dyn ClientHost, event: &mut ClientPacketEvent<'_>) {
        if let Some(payload) = event.packet.custom_payload.as_ref()
            && payload.channel.starts_with("voicechat:")
            && connected_world()
            && host.on(voice::ID)
        {
            voice::payload(&payload.channel, &payload.data);
        }
    }

    fn client_disconnecting(&self, _: &dyn ClientHost, _: &mut ClientDisconnectingEvent) {
        set_connected_world(false);
        voice::tick(None, &|_| None);
        voice::menu_sync(false);
    }

    fn render_frame_pre(&self, host: &mut dyn ClientHost, _: &mut RenderFramePreEvent) {
        if !connected_world() || !host.on(voice::ID) {
            voice::tick(None, &|_| None);
            host.mod_screen_close(voice::ID);
            voice::menu_sync(false);
            return;
        }
        if voice::menu_take_close() {
            host.mod_screen_close(voice::ID);
        }
        voice::menu_sync(host.mod_screen() == Some(voice::ID));
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        if !connected_world() || !host.on(voice::ID) {
            return;
        }
        if host.consume_click(voice::ID, voice::KEY_MENU) && host.playing() {
            let key = binding(host, voice::KEY_MENU, voice::DEFAULT_MENU);
            voice::menu_open(voice::key_char(&key));
            if !host.mod_screen_open(voice::ID) {
                voice::menu_sync(false);
            }
        }
        let _ = voice::key(
            voice::Key::PushToTalk,
            host.is_down(voice::ID, voice::KEY_PTT),
        );
        let _ = voice::key(
            voice::Key::Whisper,
            host.is_down(voice::ID, voice::KEY_WHISPER),
        );
        for (action, id) in [
            (voice::Key::Mute, voice::KEY_MUTE),
            (voice::Key::Disable, voice::KEY_DISABLE),
        ] {
            if host.consume_click(voice::ID, id)
                && let Some(line) = voice::key(action, true)
            {
                let _ = host.chat_line(Text::literal(line));
            }
        }
        save(host);
    }

    fn register_client_commands(
        &self,
        _: &mut dyn ClientHost,
        event: &mut RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.push(Command::new(voice::COMMAND));
    }

    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        if !connected_world() || !host.on(voice::ID) {
            return Flow::Continue;
        }
        read_settings(host);
        let Some(lines) = voice::command(event.command) else {
            return Flow::Continue;
        };
        for line in lines {
            let _ = host.chat_line(Text::literal(line));
        }
        save(host);
        Flow::Cancel
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if host.mod_screen() != Some(voice::ID) {
            return;
        }
        let keys = voice::HOTKEYS
            .iter()
            .map(|(key, label, fallback)| {
                (
                    label.to_string(),
                    voice::key_label(&binding(host, key, fallback)),
                )
            })
            .collect();
        let draw = &mut *event.draw;
        let commands = voice::menu_draw(
            draw.size(),
            event.screen.mouse,
            &|text| draw.text_width(text),
            keys,
        );
        for command in commands {
            match command {
                voice::ui::Draw::Fill(rect, color) => draw.fill(rect, color),
                voice::ui::Draw::Text(text, at, color, shadow) => {
                    draw.text(&text, at, color | 0xff00_0000, shadow);
                }
            }
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(voice::ID) {
            return Flow::Continue;
        }
        if event.button == 0 {
            voice::menu_click(event.screen.mouse);
            save(host);
        }
        Flow::Cancel
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(voice::ID) {
            return Flow::Continue;
        }
        voice::menu_scroll(event.scroll[1]);
        Flow::Cancel
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(voice::ID) {
            return Flow::Continue;
        }
        let key = binding(host, voice::KEY_MENU, voice::DEFAULT_MENU);
        voice::menu_key(event.key, event.modifiers & 1 != 0, &key);
        save(host);
        Flow::Cancel
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(voice::ID) {
            return Flow::Continue;
        }
        voice::menu_char(event.c);
        Flow::Cancel
    }
}
