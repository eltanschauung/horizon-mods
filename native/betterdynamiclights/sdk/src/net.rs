// What only the server knows: a client sees the other entities' kind and place, but not what
// they hold, whether they burn, what a dropped item is or whether a minecart is lit (the API's
// client entity reads cover the local player only). So the mod's server part, when the server
// has the mod (single player always does: its own server runs it), sends each client that said
// hello the facts of the entities near it once a tick, as the mod's own optional payloads. On a
// server without the mod, the client lights what it can see by itself (`lights`).
use crate::runtime::{self as dl, Fact, Glow, Kind, Who};
use mod_api::*;
use std::sync::{Mutex, OnceLock};

const CH_HELLO: &str = "dynamiclights:hello";
const CH_FACTS: &str = "dynamiclights:facts";
const PROTOCOL: u8 = 1;

struct Tokens {
    hello: ToServer<u8>,
    facts: ToClient<Vec<u8>>,
}

static TOKENS: OnceLock<Tokens> = OnceLock::new();

/// The facts the client last got, and how many ticks ago.
#[derive(Default)]
struct Client {
    facts: Vec<(u32, [f64; 3], Fact)>,
    age: u32,
    /// Ticks until the next hello.
    hello_in: u32,
}

static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

fn with_client<R>(f: impl FnOnce(&mut Client) -> R) -> R {
    let mut c = CLIENT.lock().unwrap_or_else(|e| e.into_inner());
    f(c.get_or_insert_with(Client::default))
}

/// The players that said hello.
static PLAYERS: Mutex<Vec<u128>> = Mutex::new(Vec::new());

fn players<R>(f: impl FnOnce(&mut Vec<u128>) -> R) -> R {
    f(&mut PLAYERS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// The payloads, both optional: a server or client without the mod simply has none.
pub(crate) fn register(r: &mut dyn Registrar) {
    let mut handlers = r.payload_handlers();
    let registrar = handlers.registrar("1");
    let mut view = registrar.optional();
    let hello = view.play_to_server_typed::<u8>(PayloadId(CH_HELLO.into()), |version, cx| {
        if version == PROTOCOL
            && let Ok(player) = cx.player()
        {
            let uuid = player.uuid();
            players(|p| {
                if !p.contains(&uuid) {
                    p.push(uuid);
                }
            });
        }
        Ok(())
    });
    let facts = view.play_to_client_typed::<Vec<u8>>(PayloadId(CH_FACTS.into()), |bytes, _| {
        let facts = dl::decode_facts(&bytes);
        with_client(|c| {
            c.facts = facts;
            c.age = 0;
        });
        Ok(())
    });
    let _ = TOKENS.set(Tokens { hello, facts });
}

// ---- the client ---------------------------------------------------------------------------

/// Each client tick: hello to the server until it answers (again after it falls silent, as on
/// joining another server).
pub(crate) fn client_tick(host: &mut dyn ClientHost) {
    let Some(tokens) = TOKENS.get() else { return };
    let say = with_client(|c| {
        c.age = c.age.saturating_add(1);
        if c.age > 40 {
            c.facts.clear();
        }
        if c.age <= 40 {
            return false;
        }
        c.hello_in = c.hello_in.saturating_sub(1);
        if c.hello_in == 0 {
            c.hello_in = 100;
            true
        } else {
            false
        }
    });
    if say && host.local_player().is_some() {
        let _ = tokens.hello.send(host, &PROTOCOL);
    }
}

/// The server's facts, while it sends them.
pub(crate) fn server_facts() -> Option<Vec<(u32, [f64; 3], Fact)>> {
    with_client(|c| (c.age <= 40).then(|| c.facts.clone()))
}

// ---- the server ---------------------------------------------------------------------------

/// What an item gives off held or dropped: its light and whether it is a flame.
fn item(host: &dyn Host, stack: Option<ItemStack>) -> (u8, bool) {
    stack.map_or((0, false), |s| {
        let id = s.item().id(host);
        (dl::item_light(&id), dl::fire_item(&id))
    })
}

/// A glow squid's dark ticks after a hit (`GlowSquid.getDarkTicksRemaining`).
fn dark_ticks(host: &mut dyn ServerHost, e: EntityRef) -> i32 {
    let Some(nbt) = e.nbt(host) else { return 0 };
    nbt.iter()
        .find(|(k, _)| k.to_string_lossy() == "DarkTicksRemaining")
        .and_then(|(_, v)| match v {
            Tag::Int(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0)
}

/// The facts of the entities near `player`.
fn facts_near(host: &mut dyn ServerHost, player: PlayerRef) -> Vec<(u32, [f64; 3], Fact)> {
    let me = player.entity();
    let (Some(level), Some(center)) = (me.level(host), me.position(host)) else { return Vec::new() };
    let r = f64::from(dl::MAX_RANGE) + 8.0;
    let min = [center[0] - r, center[1] - r, center[2] - r];
    let max = [center[0] + r, center[1] + r, center[2] + r];
    let Some(entities) = level.entities_in(host, min, max) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entities {
        let (Some(feet), Some(kind)) = (e.position(host), e.kind(host).map(|k| k.id(host))) else { continue };
        let burning = e.fire_ticks(host).is_some_and(|t| t > 0);
        let size = e.bounding_box(host).map_or([0.6, 0.6], |b| [b[1][0] - b[0][0], b[1][1] - b[0][1]]);
        let (mut fact, lift) = if let Some(living) = host.living(e) {
            let who = if e == me {
                Who::Me
            } else if kind == "minecraft:player" {
                if e.spectator(host) == Some(true) {
                    continue;
                }
                Who::Player
            } else {
                Who::Mob
            };
            let hands = [Hand::Main, Hand::Off].map(|h| item(host, living.hand_item(host, h)));
            let mut f = Fact::new(who).hands(hands);
            let dark = if kind == "minecraft:glow_squid" { dark_ticks(host, e) } else { 0 };
            // A magma cube's size from its width (0.52 a size).
            let own = dl::creature_light(&kind, (size[0] / 0.52).round() as i32, dark);
            if own > 0 {
                f.glow = dl::flame(own, dl::fire_creature(&kind));
                f.glow_kind = Kind::Creatures;
            }
            let lift = if kind == "minecraft:armor_stand" { size[1] * 0.6 } else if who == Who::Me { 1.2 } else { size[1] * 0.6 };
            (f, lift)
        } else if let Some(dropped) = host.item_entity(e) {
            (Fact::new(Who::Dropped).hands([item(host, dropped.stack(host))]), 0.25)
        } else {
            let lit_cart = host.minecart(e).and_then(|cart| match kind.as_str() {
                "minecraft:tnt_minecart" if cart.fuse(host).is_some_and(|f| f >= 0) => Some(Glow::TntMinecart),
                "minecraft:furnace_minecart" if cart.fuel(host).is_some_and(|f| f > 0) => Some(Glow::FurnaceMinecart),
                _ => None,
            });
            let mut f = Fact::new(Who::Other);
            let glow = lit_cart.or_else(|| Glow::of(&kind));
            if let Some(g) = glow {
                f.glow = dl::flame(g.level(), g.fire());
                f.glow_kind = g.kind();
            }
            (f, glow.map_or(0.25, Glow::lift))
        };
        fact.burning = burning;
        if fact.held > 0 || fact.burning || fact.glow > 0 {
            out.push((e.id(), [feet[0], feet[1] + lift, feet[2]], fact));
        }
    }
    out
}

/// Each server tick: the facts to every player that said hello.
pub(crate) fn server_tick(host: &mut dyn ServerHost) {
    let Some(tokens) = TOKENS.get() else { return };
    for uuid in players(|p| p.clone()) {
        let Some(player) = host.player(uuid) else { continue };
        let facts = facts_near(host, player);
        let _ = tokens.facts.send(host, player, &dl::encode_facts(&facts));
    }
}

/// A player left: no more facts for them.
pub(crate) fn player_left(uuid: u128) {
    players(|p| p.retain(|u| *u != uuid));
}
