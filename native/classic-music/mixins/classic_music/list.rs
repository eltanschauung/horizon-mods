// SPDX-License-Identifier: MIT
fn classic_music_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use horizon_client_render::sound_events::classic_music_policy as policy;
    mods.push(crate::game::native_hooks::NativeMod { id:policy::ID.into(),title:format!("Classic Music {}",policy::VERSION),description:"Classic background music: 21 Overworld songs, four menu songs and the four original Nether tracks. Music discs are unchanged. Client-side.".into(),keys:vec![] });
}
