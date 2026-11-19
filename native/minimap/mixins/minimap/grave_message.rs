// SPDX-License-Identifier: GPL-3.0-or-later
fn receive(game: &crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::game_grave_waypoints::receive(game, channel, data);
}
