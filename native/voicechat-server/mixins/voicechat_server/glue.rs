// Voice chat's server half in the game (`game::voicechat_server`): the relay
// (crate voicechat_server_relay) opens its UDP port when the first voice chat
// client asks for a secret, hears the clients' messages through Horizon's mod
// messages, and learns each tick where every player is. A player without a
// voice chat client is never sent anything.

use super::native_inventories::NativeInventoryKind;
use crate::game::Game;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use voicechat_server_relay::{Config, Player, Server, proto};

/// Messages to one player a tick at most (mod messages allow 32, shared with other mods).
const PER_TICK: usize = 24;
/// The config file in the server's `config/` folder.
const FILE: &str = "voicechat-server.json";
/// After the port failed to open, wait this long before trying again.
const RETRY: Duration = Duration::from_secs(60);

enum Relay {
    Off,
    Running(Server),
    Failed(Instant),
}

static RELAY: Mutex<Relay> = Mutex::new(Relay::Off);

fn relay() -> std::sync::MutexGuard<'static, Relay> {
    RELAY.lock().unwrap_or_else(|e| e.into_inner())
}

fn players(game: &Game<'_>) -> Vec<Player> {
    game.native_server_players()
        .into_iter()
        .map(|p| Player {
            uuid: p.uuid,
            name: p.name,
            dimension: p.dimension,
            position: p.position,
            // Lunatic maintains this effect from the authoritative hotbar.
            broadcast: game.native_effects(p.uuid).is_some_and(|effects| {
                effects.iter().any(|effect| effect.id == "lunatic:all_talk")
            }),
        })
        .collect()
}

/// `config/voicechat-server.json`, written with its defaults the first time
/// and again with the keys a newer version adds (values kept). A file that
/// doesn't read is left alone and the defaults apply.
fn config() -> Config {
    let Ok(dir) = Game::native_config_dir() else {
        return Config::default();
    };
    let path = dir.join(FILE);
    let config = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Config>(&text) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "Voice chat: {} doesn't read ({e}); using the defaults",
                    path.display()
                );
                return Config::default();
            }
        },
        Err(_) => Config::default(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&path, text + "\n");
    }
    config
}

/// A message from a player's client: only `voicechat:` channels are ours.
pub(super) fn message(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if !channel.starts_with("voicechat:") {
        return;
    }
    let mut relay = relay();
    if !matches!(*relay, Relay::Running(_)) {
        if channel != proto::REQUEST_SECRET {
            return;
        }
        if let Relay::Failed(at) = *relay
            && at.elapsed() < RETRY
        {
            return;
        }
        let config = config();
        match Server::start(config.clone()) {
            Ok(server) => {
                let at = server
                    .local_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                log::info!("Voice chat: relaying voices on UDP {at}");
                *relay = Relay::Running(server);
            }
            Err(e) => {
                log::warn!(
                    "Voice chat: not started on UDP {}:{}: {e}",
                    config.bind,
                    config.port
                );
                *relay = Relay::Failed(Instant::now());
                return;
            }
        }
    }
    let Relay::Running(server) = &*relay else {
        return;
    };
    // A player who joined since the last tick (or a relay that just started):
    // learn who is on first. Not for every message: it lists every player.
    if !server.knows(player) {
        server.tick(players(game));
    }
    server.message(player, channel, data);
}

/// Each server tick, once the relay runs.
pub(super) fn tick(game: &mut Game<'_>) {
    let (out, wear) = {
        let relay = relay();
        let Relay::Running(server) = &*relay else {
            return;
        };
        server.tick(players(game));
        (server.outgoing(PER_TICK), server.take_phone_wear())
    };
    for player in wear {
        wear_phone(game, player);
    }
    for (player, channel, data) in out {
        if let Err(e) = game.native_net_send_to_player(player, &channel, &data) {
            log::debug!("Voice chat: {channel} not sent: {e}");
        }
    }
}

fn wear_phone(game: &mut Game<'_>, player: u128) {
    let kind = NativeInventoryKind::Player;
    let Ok(inventory) = game.native_inventory(player, kind) else {
        return;
    };
    if !inventory.online {
        return;
    }
    let Some((slot, phone)) =
        (0..9)
            .chain(std::iter::once(crate::game::shield::OFFHAND_SLOT))
            .filter_map(|slot| inventory.slots.get(slot).map(|stack| (slot, stack)))
            .find_map(|(slot, stack)| {
                stack
                    .as_ref()
                    .filter(|stack| stack.item.as_ref() == "lunatic:koishi_phone")
                    .map(|phone| (slot, phone))
            })
    else {
        return;
    };
    let mut worn = phone.clone();
    worn.damage = worn.damage.saturating_add(1);
    let next = (worn.damage < 250).then_some(worn);
    let _ = game.native_set_inventory_slot(player, kind, slot, Some(phone), next);
}

pub(super) fn left(player: u128) {
    if let Relay::Running(server) = &*relay() {
        server.left(player);
    }
}

/// The world closed: the port closes too (it opens again on the next request).
pub(super) fn stop() {
    *relay() = Relay::Off;
}
