//! Joon at work on the server: what her screen shows (`net::JoonView`, drawn by `joon_ui`), what
//! is done on it (`net::JoonAct`), the trades themselves, and her visits.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use mod_api::{Holder, ServerHost, Text};

use crate::joon::{self, Trade, Visitee};
use crate::net::{self, JoonAct, JoonView, ShownPlayer, ShownTrade};
use crate::players::{DollPlayerAccess, max_stack_size};
use crate::stacks::DollStack as ItemStack;
use crate::{DOLL, DollData, Mod, goals};

/// Whose screen a player asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum View {
    /// Her owner's: her trades, and who to send her to.
    Edit,
    /// The visited player's: her trade offer.
    Trade,
}

/// A player's open Joon screen, and what it was last sent.
#[derive(Clone)]
struct Panel {
    doll: u32,
    view: View,
    shown: Option<JoonView>,
}

/// A visit: whose she is, who she is with, how long she has been there, which doll she is (her
/// saved id, for the cooldown) and how many trades she has made.
#[derive(Clone)]
struct Visit {
    owner: u128,
    target: u128,
    ticks: u32,
    tag: u64,
    traded: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    TimeUp,
    SentHome,
    Gone,
}

/// Joon screens asked for, opened on the next server tick: player, doll, which.
pub(crate) static PENDING_JOON: Mutex<Vec<(u128, u32, View)>> = Mutex::new(Vec::new());
/// What players did on their Joon screens, done on the next server tick.
pub(crate) static ACTS: Mutex<Vec<(u128, JoonAct)>> = Mutex::new(Vec::new());
static OPEN: LazyLock<Mutex<HashMap<u128, Panel>>> = LazyLock::new(Default::default);
static VISITS: LazyLock<Mutex<HashMap<u32, Visit>>> = LazyLock::new(Default::default);
/// When each Joon (by her saved id) last left each player.
static COOLDOWN: LazyLock<Mutex<HashMap<(u64, u128), Instant>>> = LazyLock::new(Default::default);
static TICKS: AtomicU32 = AtomicU32::new(0);

/// Who doll `id` is visiting, while she is.
pub(crate) fn visiting(id: u32) -> Option<u128> {
    VISITS.lock().ok()?.get(&id).map(|v| v.target)
}

pub(crate) fn is_visiting(id: u32) -> bool {
    visiting(id).is_some()
}

/// Seconds before Joon `tag` may go to `player` again. Never for her own owner: she can always be
/// sent to them to try her trades.
fn cooldown_left(tag: u64, player: u128, owner: u128) -> u32 {
    if player == owner {
        return 0;
    }
    let Some(at) = COOLDOWN.lock().ok().and_then(|c| c.get(&(tag, player)).copied()) else { return 0 };
    joon::COOLDOWN_SECS.saturating_sub(at.elapsed().as_secs()) as u32
}

fn bar(host: &mut dyn ServerHost, player: u128, text: impl Into<String>) {
    host.action_bar(player, Text::literal(text.into()));
}

fn send_view(host: &mut dyn ServerHost, player: u128, v: Option<&JoonView>) {
    crate::send_doll_packet(host, player, net::JOON, &net::encode_joon(v));
}

fn trim(trades: &mut Vec<Trade>) {
    while trades.last().is_some_and(|t| !t.is_set()) {
        trades.pop();
    }
}

fn shown(s: &Option<ItemStack>) -> net::Shown {
    s.as_ref().map(|s| (s.item.to_string(), s.count))
}

/// Part `part` of trade `t` (0 and 1 what is paid, 2 what is got).
fn part_mut(t: &mut Trade, part: u8) -> &mut Option<ItemStack> {
    match part {
        0 => &mut t.cost[0],
        1 => &mut t.cost[1],
        _ => &mut t.result,
    }
}

/// The trades a visitor is offered: those that are set.
fn offered(data: &DollData) -> Vec<Trade> {
    data.trades.iter().filter(|t| t.is_set()).cloned().collect()
}

/// Whether `player` may have Joon `doll`'s screen `view` open: her owner's while she is home, the
/// visited player's while she is with them.
fn allowed(data: &DollData, doll: u32, player: u128, view: View) -> bool {
    match view {
        View::Trade => visiting(doll) == Some(player),
        View::Edit => data.owner == player && !is_visiting(doll),
    }
}

impl Mod {
    /// Joon `doll`'s screen `view` for `player`, if it is theirs to see.
    pub(crate) fn open_joon(&self, host: &mut dyn ServerHost, player: u128, doll: u32, view: View) {
        let holder = Holder::Entity(doll);
        if host.entity(doll).is_none() || !DOLL.has(host, holder).unwrap_or(false) {
            return;
        }
        if goals::chest_open() == Some(doll) {
            bar(host, player, format!("{} : my inventory is open", goals::JOON));
            return;
        }
        let data = DOLL.get(host, holder).unwrap_or_default();
        if !allowed(&data, doll, player, view) {
            return;
        }
        OPEN.lock().ok().map(|mut o| o.insert(player, Panel { doll, view, shown: None }));
        self.refresh_joon(host, player);
    }

    /// The players Joon can be sent to: everyone in her dimension (but spectators), her owner first.
    fn visitees(&self, host: &mut dyn ServerHost, owner: u128, doll: u32, tag: u64) -> Vec<Visitee> {
        let dimension = host.entity(doll).and_then(|e| e.level(host)).map(|l| l.dimension_id().to_owned());
        // Slot 36 of the carried slots is the off hand.
        let axe = host.player_stacks(owner).get(36).cloned().flatten().is_some_and(|s| s.item == "minecraft:wooden_axe" && s.count > 0);
        let mut players: Vec<Visitee> = host
            .server_players()
            .into_iter()
            .filter(|p| p.game_mode != 3 && Some(&p.dimension) == dimension.as_ref())
            .map(|p| Visitee { uuid: p.uuid, wait: if axe { 0 } else { cooldown_left(tag, p.uuid, owner) }, you: p.uuid == owner, name: p.name })
            .collect();
        players.sort_by(|a, b| b.you.cmp(&a.you).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        players
    }

    /// What `player`'s screen of `panel` shows now.
    fn joon_view(&self, host: &mut dyn ServerHost, player: u128, panel: &Panel, data: &DollData) -> JoonView {
        let stock = goals::inventory(panel.doll).unwrap_or_else(|| data.items.clone());
        let have = host.player_stacks(player);
        let show = |t: &Trade| ShownTrade {
            parts: [shown(&t.cost[0]), shown(&t.cost[1]), shown(&t.result)],
            stock: joon::stock_for(&stock, t).unwrap_or(u32::MAX),
            affordable: joon::can_pay(&have, t),
        };
        match panel.view {
            View::Trade => {
                let owner = host
                    .server_players()
                    .into_iter()
                    .find(|p| p.uuid == data.owner)
                    .map_or_else(|| "her owner".to_owned(), |p| p.name);
                let secs = VISITS
                    .lock()
                    .ok()
                    .and_then(|v| v.get(&panel.doll).map(|v| joon::VISIT_TICKS.saturating_sub(v.ticks).div_ceil(20)))
                    .unwrap_or(0);
                JoonView { visitor: true, owner, secs, trades: offered(data).iter().map(show).collect(), ..JoonView::default() }
            }
            View::Edit => JoonView {
                visitor: false,
                owner: String::new(),
                secs: 0,
                trades: data.trades.iter().map(show).collect(),
                items: have.iter().map(shown).collect(),
                players: self
                    .visitees(host, data.owner, panel.doll, data.tag)
                    .into_iter()
                    .map(|p| ShownPlayer { uuid: p.uuid, name: p.name, wait: p.wait, you: p.you })
                    .collect(),
            },
        }
    }

    /// Sends `player` their Joon screen as it is now, if it changed; closes it when it may no
    /// longer be open.
    fn refresh_joon(&self, host: &mut dyn ServerHost, player: u128) {
        let Some(panel) = OPEN.lock().ok().and_then(|o| o.get(&player).cloned()) else { return };
        let holder = Holder::Entity(panel.doll);
        if host.entity(panel.doll).is_none() || !DOLL.has(host, holder).unwrap_or(false) {
            self.close_joon(host, player);
            return;
        }
        let data = DOLL.get(host, holder).unwrap_or_default();
        if !allowed(&data, panel.doll, player, panel.view) {
            self.close_joon(host, player);
            return;
        }
        let view = self.joon_view(host, player, &panel, &data);
        if panel.shown.as_ref() == Some(&view) {
            return;
        }
        send_view(host, player, Some(&view));
        if let Ok(mut o) = OPEN.lock()
            && let Some(p) = o.get_mut(&player)
        {
            p.shown = Some(view);
        }
    }

    /// `player`'s Joon screen, closed by the server.
    fn close_joon(&self, host: &mut dyn ServerHost, player: u128) {
        if OPEN.lock().ok().and_then(|mut o| o.remove(&player)).is_some() {
            send_view(host, player, None);
        }
    }

    /// Each server tick: what was done on Joon screens, and, twice a second, the screens brought
    /// up to date (her stock and their inventories change under them).
    pub(crate) fn joon_tick(&self, host: &mut dyn ServerHost) {
        let acts: Vec<(u128, JoonAct)> = ACTS.lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default();
        for (player, act) in acts {
            self.joon_act(host, player, act);
        }
        if TICKS.fetch_add(1, Ordering::Relaxed) % 10 == 0 {
            let players: Vec<u128> = OPEN.lock().map(|o| o.keys().copied().collect()).unwrap_or_default();
            for player in players {
                self.refresh_joon(host, player);
            }
        }
    }

    fn joon_act(&self, host: &mut dyn ServerHost, player: u128, act: JoonAct) {
        if act == JoonAct::Close {
            OPEN.lock().ok().map(|mut o| o.remove(&player));
            return;
        }
        let Some(panel) = OPEN.lock().ok().and_then(|o| o.get(&player).cloned()) else { return };
        let doll = panel.doll;
        let holder = Holder::Entity(doll);
        if host.entity(doll).is_none() || !DOLL.has(host, holder).unwrap_or(false) {
            self.close_joon(host, player);
            return;
        }
        let mut data = DOLL.get(host, holder).unwrap_or_default();
        if !allowed(&data, doll, player, panel.view) {
            self.close_joon(host, player);
            return;
        }
        let owner = panel.view == View::Edit;
        let mut changed = false;
        match act {
            JoonAct::Set { trade, part, slot, one } if owner && usize::from(trade) < joon::MAX_TRADES && part < 3 => {
                let Some(mut s) = host.player_stacks(player).get(usize::from(slot)).cloned().flatten() else { return };
                let max = max_stack_size(host, &s.item).unwrap_or(64).clamp(1, 64);
                s.count = if one { 1 } else { s.count.clamp(1, max) };
                let i = usize::from(trade);
                if data.trades.len() <= i {
                    data.trades.resize(i + 1, Trade::default());
                }
                *part_mut(&mut data.trades[i], part) = Some(s);
                changed = true;
            }
            JoonAct::SetId { trade, part, ref id } if owner && usize::from(trade) < joon::MAX_TRADES && part < 3 => {
                // Any item the game has (not air), one of it.
                if id == "minecraft:air" || max_stack_size(host, id).is_none() {
                    bar(host, player, format!("{} : I don't know the item {id}", goals::JOON));
                    return;
                }
                let i = usize::from(trade);
                if data.trades.len() <= i {
                    data.trades.resize(i + 1, Trade::default());
                }
                *part_mut(&mut data.trades[i], part) = Some(ItemStack::new(id.clone(), 1));
                changed = true;
            }
            JoonAct::Count { trade, part, count } if owner && part < 3 => {
                let Some(t) = data.trades.get_mut(usize::from(trade)) else { return };
                let p = part_mut(t, part);
                let Some(s) = p.as_mut() else { return };
                if count == 0 {
                    *p = None;
                } else {
                    let max = max_stack_size(host, &s.item).unwrap_or(64).clamp(1, 64);
                    s.count = u32::from(count).min(max);
                }
                changed = true;
            }
            JoonAct::Delete { trade } if owner && usize::from(trade) < data.trades.len() => {
                data.trades.remove(usize::from(trade));
                changed = true;
            }
            JoonAct::Stock if owner => {
                OPEN.lock().ok().map(|mut o| o.remove(&player));
                crate::PENDING.lock().ok().map(|mut p| p.push((player, doll)));
                return;
            }
            JoonAct::Send { player: to } if owner => {
                if !data.trades.iter().any(Trade::is_set) {
                    bar(host, player, format!("{} : set a trade first", goals::JOON));
                    return;
                }
                let Some(to) = self.visitees(host, data.owner, doll, data.tag).into_iter().find(|p| p.uuid == to) else {
                    bar(host, player, format!("{} : they aren't here any more", goals::JOON));
                    return;
                };
                if to.wait > 0 {
                    bar(host, player, format!("{} : I was with {} lately; I can go again in {}", goals::JOON, to.name, joon::clock(to.wait)));
                    return;
                }
                self.close_joon(host, player);
                self.start_visit(host, doll, &data, to.uuid);
                return;
            }
            JoonAct::Trade { trade, many } if !owner => {
                let Some(t) = offered(&data).get(usize::from(trade)).cloned() else { return };
                self.joon_trade(host, player, doll, &mut data, &t, many);
            }
            JoonAct::Home if !owner => {
                self.end_visit(host, doll, End::SentHome);
                return;
            }
            _ => return,
        }
        if changed {
            trim(&mut data.trades);
            if let Some(d) = crate::doll_of(host, doll) {
                crate::remember(doll, d, &data);
            }
            let _ = DOLL.set(host, holder, data);
            if let Some(p) = host.server_players().into_iter().find(|p| p.uuid == player) {
                crate::player_sound(host, &p.dimension, "minecraft:ui.button.click", p.position, 0.3, 1.2);
            }
        }
        self.refresh_joon(host, player);
    }

    /// `player` takes trade `t` from Joon `doll`: once, or with `many` as many times as they can pay
    /// and she can pay out (a free gift only once a click).
    fn joon_trade(&self, host: &mut dyn ServerHost, player: u128, doll: u32, data: &mut DollData, t: &Trade, many: bool) {
        if goals::chest_open() == Some(doll) {
            bar(host, player, format!("{} : one moment, my inventory is open", goals::JOON));
            return;
        }
        let Some(me) = host.server_players().into_iter().find(|p| p.uuid == player) else { return };
        let costs: Vec<ItemStack> = t.cost.iter().flatten().cloned().collect();
        // As many as can be: at most a full inventory's worth of rounds.
        let rounds = if many && !costs.is_empty() { 36 * 64 } else { 1 };
        let maxes: HashMap<String, u32> =
            costs.iter().map(|c| (c.item.to_string(), max_stack_size(host, &c.item).unwrap_or(64))).collect();
        let mut stock = goals::inventory(doll).unwrap_or_else(|| data.items.clone());
        let (mut done, mut why) = (0, None);
        for _ in 0..rounds {
            if !joon::can_pay(&host.player_stacks(player), t) {
                why = Some("You haven't got what she asks for");
                break;
            }
            let mut after = stock.clone();
            if let Err(e) = joon::joon_side(&mut after, t, |id| maxes.get(id).copied().unwrap_or(64)) {
                why = Some(if e == "full" { "Joon has no room for that" } else { "Joon is out of stock" });
                break;
            }
            if !costs.is_empty() && !host.take_stacks(player, &costs) {
                why = Some("You haven't got what she asks for");
                break;
            }
            stock = after;
            if let Some(r) = &t.result {
                crate::give_or_drop(host, &me, vec![r.clone()]);
            }
            done += 1;
        }
        let at = host.entity(doll).and_then(|e| e.position(host)).unwrap_or(me.position);
        if done > 0 {
            goals::put_inventory(doll, stock.clone());
            data.items = stock;
            if let Some(d) = crate::doll_of(host, doll) {
                crate::remember(doll, d, data);
            }
            let _ = DOLL.set(host, Holder::Entity(doll), data.clone());
            if let Ok(mut v) = VISITS.lock()
                && let Some(v) = v.get_mut(&doll)
            {
                v.traded += done;
            }
            crate::doll_sound(host, doll, "minecraft:entity.wandering_trader.yes", at, 0.8, 1.2);
            if done > 1 {
                bar(host, player, format!("Traded {done} times"));
            }
        } else if let Some(why) = why {
            bar(host, player, why);
            crate::doll_sound(host, doll, "minecraft:entity.wandering_trader.no", at, 0.8, 1.2);
        }
    }

    /// Joon `doll` goes to `target`, with her trades, for `VISIT_TICKS`.
    fn start_visit(&self, host: &mut dyn ServerHost, doll: u32, data: &DollData, target: u128) {
        let Some(dimension) = host.entity(doll).and_then(|e| e.level(host)).map(|l| l.dimension_id().to_owned()) else { return };
        let players = host.server_players();
        let Some(t) = players.iter().find(|p| p.uuid == target && p.dimension == dimension) else {
            bar(host, data.owner, format!("{} : they aren't here any more", goals::JOON));
            return;
        };
        if !data.trades.iter().any(Trade::is_set) || is_visiting(doll) {
            return;
        }
        let (to, name) = ([t.position[0] + 1.0, t.position[1], t.position[2] + 1.0], t.name.clone());
        let owner_name = players.iter().find(|p| p.uuid == data.owner).map_or_else(|| "Someone".to_owned(), |p| p.name.clone());
        VISITS.lock().ok().map(|mut v| v.insert(doll, Visit { owner: data.owner, target, ticks: 0, tag: data.tag, traded: 0 }));
        goals::set_state(doll, goals::FOLLOW, true, target);
        crate::teleport_mob(host, &dimension, doll, to);
        crate::doll_sound(host, doll, "minecraft:entity.enderman.teleport", to, 0.8, 1.2);
        let secs = joon::VISIT_TICKS / 20;
        if target == data.owner {
            host.tell(target, Text::literal(format!("Joon is here with her trade offer: right-click her to trade (she stays {secs} seconds)")));
        } else {
            host.tell(target, Text::literal(format!("{owner_name} sent you Joon with a trade offer: right-click her to trade (she stays {secs} seconds)")));
            bar(host, data.owner, format!("{} : off to see {name}!", goals::JOON));
        }
    }

    /// In Joon `doll`'s tick, while she is visiting: home when her time is up or they have gone.
    pub(crate) fn visit_tick(&self, host: &mut dyn ServerHost, doll: u32) {
        let Some(mut v) = VISITS.lock().ok().and_then(|v| v.get(&doll).cloned()) else { return };
        v.ticks += 1;
        let dimension = host.entity(doll).and_then(|e| e.level(host)).map(|l| l.dimension_id().to_owned());
        let here = host.server_players().into_iter().any(|p| p.uuid == v.target && Some(&p.dimension) == dimension.as_ref());
        VISITS.lock().ok().map(|mut m| m.insert(doll, v.clone()));
        if v.ticks >= joon::VISIT_TICKS {
            self.end_visit(host, doll, End::TimeUp);
            return;
        }
        if !here {
            self.end_visit(host, doll, End::Gone);
            return;
        }
        if v.ticks % 20 == 0 {
            goals::set_state(doll, goals::FOLLOW, true, v.target);
        }
    }

    fn end_visit(&self, host: &mut dyn ServerHost, doll: u32, why: End) {
        let Some(v) = VISITS.lock().ok().and_then(|mut m| m.remove(&doll)) else { return };
        if v.target != v.owner {
            COOLDOWN.lock().ok().map(|mut c| {
                c.retain(|_, at| at.elapsed().as_secs() < joon::COOLDOWN_SECS);
                c.insert((v.tag, v.target), Instant::now())
            });
        }
        if OPEN.lock().ok().and_then(|o| o.get(&v.target).cloned()).is_some_and(|p| p.doll == doll && p.view == View::Trade) {
            self.close_joon(host, v.target);
        }
        let data = DOLL.get(host, Holder::Entity(doll)).unwrap_or_default();
        crate::tell_state(doll, &data);
        let players = host.server_players();
        let dimension = host.entity(doll).and_then(|e| e.level(host)).map(|l| l.dimension_id().to_owned());
        if let Some(dimension) = dimension
            && let Some(o) = players.iter().find(|p| p.uuid == v.owner && p.dimension == dimension)
        {
            let home = [o.position[0] + 1.0, o.position[1], o.position[2] + 1.0];
            crate::teleport_mob(host, &dimension, doll, home);
            crate::doll_sound(host, doll, "minecraft:entity.enderman.teleport", home, 0.8, 1.2);
        }
        let name = players.iter().find(|p| p.uuid == v.target).map_or_else(|| "them".to_owned(), |p| p.name.clone());
        if v.target != v.owner {
            let text = match why {
                End::SentHome => "Joon went home",
                _ => "Joon's visit is over: she went home",
            };
            host.tell(v.target, Text::literal(text));
        }
        let text = match why {
            End::Gone => format!("{} : I couldn't stay with {name}, so I came back ({} trades)", goals::JOON, v.traded),
            End::SentHome if v.target != v.owner => format!("{} : {name} sent me home ({} trades)", goals::JOON, v.traded),
            _ => format!("{} : back from {name} ({} trades)", goals::JOON, v.traded),
        };
        host.tell(v.owner, Text::literal(text));
    }
}
