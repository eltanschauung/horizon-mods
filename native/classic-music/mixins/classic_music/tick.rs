// SPDX-License-Identifier: MIT
fn classic_music_tick(game: &mut crate::game::Game<'_>) {
    use horizon_client_render::sound_events::{
        classic_music_policy as policy, classic_music_registry as registry,
    };
    if game.options.split_guest {
        return;
    }
    let enabled = game.native_on(policy::ID);
    if !registry::sync(&mut game.audio.engine.registry, enabled) || !enabled {
        return;
    }
    let replace = game
        .music
        .current_instance()
        .and_then(|id| game.audio.engine.instance(id))
        .and_then(|i| {
            let group = policy::group(&i.identifier)?;
            let sound = i.sound.as_ref()?;
            (!group.allows(&sound.location)).then(|| i.identifier.clone())
        });
    if let Some(event) = replace
        && let Some(track) = game.audio.engine.registry.tracks(&event).first().cloned()
    {
        game.music.play_track(&event, &track, &mut game.audio);
    }
}
