fn receive(game: &crate::game::Game<'_>, packet: &horizon_protocol::packets::play::Clientbound) {
    if let horizon_protocol::packets::play::Clientbound::CustomPayload(payload) = packet {
        crate::game::game_grave_waypoints::receive(game, &payload.channel, &payload.data);
    }
}
