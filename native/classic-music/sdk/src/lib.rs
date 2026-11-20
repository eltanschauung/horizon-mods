// SPDX-License-Identifier: MIT
use mod_api::{self, ClientHost, Hooks, MusicEvent};

#[derive(Default)]
struct ClassicMusic;

#[mod_api::horizon_mod]
impl Hooks for ClassicMusic {
    fn music(&self, _host: &dyn ClientHost, event: &mut MusicEvent<'_>) {
        if let Some(music) = event.music.as_mut()
            && let Some(playlist) = classic_playlist(&music.sound)
        {
            music.sound = playlist.into();
        }
    }
}

/// Select the playlist while keeping vanilla's chosen delay range and volume.
fn classic_playlist(event: &str) -> Option<&'static str> {
    let event = event.strip_prefix("minecraft:music.")?;
    Some(match event {
        "menu" => "classic_music:music.menu",
        "end" => "classic_music:music.end",
        "dragon" => "classic_music:music.dragon",
        "credits" => "classic_music:music.credits",
        "nether" => "classic_music:music.nether",
        event if event.starts_with("nether.") => "classic_music:music.nether",
        _ => "classic_music:music.overworld",
    })
}
