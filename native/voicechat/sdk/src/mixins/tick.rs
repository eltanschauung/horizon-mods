// Each client tick on a server: where the player and the other players are,
// then the payloads the voice chat has for the server, its settings and its
// chat lines.
fn voicechat_tick(game: &mut crate::game::Game<'_>) {
    use voicechat::engine as vc;
    let Some(r) = game.remote.as_ref() else {
        return;
    };
    if !game.on(vc::ID) || !r.joined || game.integrated.is_some() || r.reconfiguring.is_some() {
        voicechat::set_connected_world(false);
        vc::tick(None, &|_| None);
        return;
    }
    let address = r.session.address.clone();
    let players: Vec<(u128, [f64; 3])> = r
        .level_players
        .iter()
        .filter_map(|(&id, &uuid)| {
            let id = u32::try_from(id).ok()?;
            let mob = game.mobs.iter().find(|mob| mob.id == id)?;
            let eye = mob.eye_height();
            Some((uuid, [mob.position.x, mob.position.y + eye, mob.position.z]))
        })
        .collect();
    let listener = game.native_player().map(|p| vc::mix::Listener {
        eye: [p.x, p.y + vc::EYE_HEIGHT, p.z],
        yaw: p.yaw,
    });
    let world = vc::World {
        address: &address,
        listener,
        players,
        redirect: horizon_protocol::server_redirect,
    };
    #[cfg(feature = "audio")]
    vc::audio::install_backend(crate::voicechat_sdk_audio::run);
    voicechat::set_connected_world(true);
    let done = vc::tick(Some(world), &|key| {
        voicechat::from_setting(key, game.setting(vc::ID, key))
    });
    vc::update_display_names(|uuid| {
        game.pack.character_catalog.get(0)?;
        Some(game.character_for(uuid).map_or_else(
            || format!("Player_{:08x}", (uuid >> 96) as u32),
            |character| character.display_name().plain().to_string(),
        ))
    });
    for (channel, data) in done.payloads {
        game.send(horizon_protocol::packets::play::Serverbound::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload { channel, data },
        ));
    }
    for (key, value) in done.store {
        game.set_setting(vc::ID, &key, voicechat::to_setting(&key, &value));
    }
    for line in done.chat {
        game.native_chat(&line);
    }
}
