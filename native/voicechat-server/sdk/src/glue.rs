// The edges between the game and the relay: the config, starting the relay,
// the clients' messages, the players each tick. A player without a voice chat
// client is never sent anything.

use crate::relay::{Config, Player, Server, proto};
use mod_api::{
    ConfigPath, HandlerThread, IPayloadContext, IPayloadHandler, PayloadBytes, PayloadHandleError,
    PayloadId, PlayerRef, Registrar, ServerHost,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Messages to one player a tick at most (payloads allow plenty; this keeps one player's voice chat traffic small).
const PER_TICK: usize = 24;
/// The config file in the server's `config/` folder (a mod keeps files named `<mod id>.<ext>` there).
const FILE: &str = "voicechat_server.json";
const LEGACY_FILE: &str = "voicechat-server.json";
const ID: &str = "voicechat_server";
/// After the port failed to open, wait this long before trying again.
const RETRY: Duration = Duration::from_secs(60);

/// What the client sends the server.
const FROM_CLIENT: [&str; 5] = [
    proto::REQUEST_SECRET,
    proto::UPDATE_STATE,
    proto::CREATE_GROUP,
    proto::SET_GROUP,
    proto::LEAVE_GROUP,
];
/// What the server sends a client.
const TO_CLIENT: [&str; 7] = [
    proto::SECRET,
    proto::STATE,
    proto::STATES,
    proto::REMOVE_STATE,
    proto::JOINED_GROUP,
    proto::ADD_GROUP,
    proto::REMOVE_GROUP,
];

enum Relay {
    Off,
    Running(Server),
    Failed(Instant),
}

static RELAY: Mutex<Relay> = Mutex::new(Relay::Off);

fn relay() -> std::sync::MutexGuard<'static, Relay> {
    RELAY.lock().unwrap_or_else(|e| e.into_inner())
}

/// A voice chat client's message on one `voicechat:` channel.
struct FromClient(&'static str);

impl IPayloadHandler for FromClient {
    fn handle(
        &self,
        bytes: PayloadBytes,
        cx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let Ok(player) = cx.player() else {
            return Ok(());
        };
        if let Ok(host) = cx.server_host()
            && host.on(ID)
        {
            message(host, player, self.0, &bytes.0);
        }
        Ok(())
    }
}

/// Declares the channels, raw, as a Paper plugin channel is: the voice chat
/// client mod needs nothing negotiated, and a vanilla client is never asked.
pub(super) fn register(r: &mut dyn Registrar) {
    let mut payloads = r.payload_handlers();
    let mut view = payloads.registrar("1");
    view.executes_on(HandlerThread::Main);
    for channel in FROM_CLIENT {
        view.play_raw(
            PayloadId(channel.into()),
            Some(Arc::new(FromClient(channel))),
        );
    }
    for channel in TO_CLIENT {
        view.play_raw(PayloadId(channel.into()), None);
    }
}

fn players(host: &dyn ServerHost) -> Vec<(PlayerRef, Player)> {
    host.players()
        .server_players()
        .into_iter()
        .filter_map(|p| {
            let level = p.entity().level(host)?;
            Some((
                p,
                Player {
                    uuid: p.uuid(),
                    name: p.name(host)?,
                    dimension: level.dimension().id(host)?,
                    position: p.entity().position(host)?,
                    broadcast: host.effect_key("lunatic:all_talk").is_some_and(|key| {
                        host.living(p.entity())
                            .is_some_and(|living| living.effect(host, key).is_some())
                    }),
                },
            ))
        })
        .collect()
}

/// `config/voicechat-server.json`, written with its defaults the first time
/// and again with the keys a newer version adds (values kept). A file that
/// doesn't read is left alone and the defaults apply.
fn config(host: &dyn ServerHost) -> Config {
    let files = host.config_files();
    let Ok(path) = ConfigPath::new(FILE) else {
        return Config::default();
    };
    let config_data = if matches!(files.stat(&path), Ok(None)) {
        // This is this mod's pre-SDK config, whose hyphenated name predates ownership checks.
        use std::io::Read;
        let old = host.instance_dir().join("config").join(LEGACY_FILE);
        match std::fs::File::open(old) {
            Ok(file) => {
                let mut bytes = Vec::new();
                if file.take(1_048_577).read_to_end(&mut bytes).is_err() || bytes.len() > 1_048_576
                {
                    return Config::default();
                }
                Some(bytes)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Config::default(),
        }
    } else {
        match files.read(&path) {
            Ok(bytes) => Some(bytes),
            Err(_) => return Config::default(),
        }
    };
    let config = match config_data {
        Some(bytes) => match serde_json::from_slice::<Config>(&bytes) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Voice chat: {FILE} doesn't read ({e}); using the defaults");
                return Config::default();
            }
        },
        None => Config::default(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&config) {
        let _ = files.write(&path, (text + "\n").as_bytes());
    }
    config
}

/// A message from a player's client.
fn message(host: &dyn ServerHost, player: PlayerRef, channel: &str, data: &[u8]) {
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
        let config = config(host);
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
    if !server.knows(player.uuid()) {
        server.tick(players(host).into_iter().map(|(_, p)| p).collect());
    }
    server.message(player.uuid(), channel, data);
}

/// Each server tick, once the relay runs.
pub(super) fn tick(host: &mut dyn ServerHost) {
    if !host.on(ID) {
        stop();
        return;
    }
    let (online, out, wear) = {
        let relay = relay();
        let Relay::Running(server) = &*relay else {
            return;
        };
        let online = players(host);
        server.tick(online.iter().map(|(_, p)| p.clone()).collect());
        let out = server.outgoing(PER_TICK);
        (online, out, server.take_phone_wear())
    };
    for uuid in wear {
        if let Some((player, _)) = online.iter().find(|(player, _)| player.uuid() == uuid) {
            wear_phone(host, *player);
        }
    }
    for (uuid, channel, data) in out {
        let Some((player, _)) = online.iter().find(|(p, _)| p.uuid() == uuid) else {
            continue;
        };
        if let Err(e) = player.send_payload(host, &PayloadId(channel.clone()), PayloadBytes(data)) {
            log::debug!("Voice chat: {channel} not sent: {e:?}");
        }
    }
}

fn wear_phone(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(phone) = host.item_key(
        &mod_api::Identifier::parse("lunatic:koishi_phone").expect("literal item identifier"),
    ) else {
        return;
    };
    let Some(slots) = player.inventory_slots(host) else {
        return;
    };
    let Some((slot, mut stack)) = slots
        .into_iter()
        .filter(|(slot, stack)| (*slot < 9 || *slot == 40) && stack.item() == phone)
        .min_by_key(|(slot, _)| *slot)
    else {
        return;
    };
    let damage = stack.damage().saturating_add(1);
    stack.set_damage(damage);
    let _ = player.set_inventory_slot(host, slot, (damage < 250).then_some(stack));
}

pub(super) fn left(player: u128) {
    if let Relay::Running(server) = &*relay() {
        server.left(player);
    }
}

/// The server stopped: the port closes too (it opens again on the next request).
pub(super) fn stop() {
    *relay() = Relay::Off;
}
