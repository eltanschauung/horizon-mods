pub mod api;
mod menu;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering::Relaxed};
use std::sync::{LazyLock, Mutex};

use mod_api::*;

use api::{ACCESSORIES, ALL_SLOTS, ENABLED, Slot};

const ID: &str = "curios_horizon";
const CONFIG: &str = include_str!("slots.toml");
static SERVER_MASK: AtomicU16 = AtomicU16::new(ALL_SLOTS);
static CONFIG_CHANGED: AtomicBool = AtomicBool::new(false);
static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);
static CLIENT_MASK: AtomicU16 = AtomicU16::new(0);
static OPEN_ON_TICK: LazyLock<Mutex<HashSet<u128>>> = LazyLock::new(Default::default);

fn server_mask() -> u16 {
    SERVER_MASK.load(Relaxed)
}

fn update_config(file: &ConfigFile) {
    if file.mod_id != ID || file.kind != ConfigKind::Server {
        return;
    }
    if let Some(mask) = parse_slots(&file.text) {
        if SERVER_MASK.swap(mask, Relaxed) != mask {
            CONFIG_CHANGED.store(true, Relaxed);
        }
    } else {
        eprintln!("Curios Horizon: invalid slots.toml; retaining the previous slot configuration");
    }
}

fn parse_slots(text: &str) -> Option<u16> {
    let mut mask = ALL_SLOTS;
    let mut seen = 0;
    let mut section = false;
    for line in text.lines() {
        let line = line.split('#').next()?.trim();
        if line.is_empty() {
            continue;
        }
        if line == "[slots]" && !section {
            section = true;
            continue;
        }
        let (key, value) = line.split_once('=')?;
        let slot = Slot::ALL
            .into_iter()
            .find(|slot| slot.name() == key.trim())?;
        let bit = 1 << slot as u8;
        if !section || seen & bit != 0 {
            return None;
        }
        seen |= bit;
        match value.trim() {
            "true" => {}
            "false" => mask &= !bit,
            _ => return None,
        }
    }
    Some(mask)
}

#[derive(Default)]
struct Curios;

#[mod_api::horizon_mod]
impl Hooks for Curios {
    fn register(&self, reg: &mut dyn Registrar, _: &RegistrationContext) {
        menu::register(reg);
        for n in 1..=Slot::ALL.len() {
            reg.menu_part(&format!("{}{n}", menu::PREFIX), SlotRules);
        }
    }

    fn register_configs(&self, _: HostSide<'_>, event: &mut RegisterConfigsEvent<'_>) {
        event
            .configs
            .push(Config::new(ID, ConfigKind::Server, CONFIG).file("curios_horizon/slots.toml"));
    }

    fn config_loading(&self, _: HostSide<'_>, event: &mut ConfigLoadingEvent<'_>) {
        update_config(event.config);
    }

    fn config_reloading(&self, _: HostSide<'_>, event: &mut ConfigReloadingEvent<'_>) {
        update_config(event.config);
    }

    fn level_tick_post(&self, host: HostSide<'_>, _: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else { return };
        if CONFIG_CHANGED.swap(false, Relaxed) {
            menu::close_all(host);
            for player in host.players().server_players() {
                let _ = ENABLED.set(host, Holder::Player(player.uuid()), server_mask());
            }
        }
        // Command callbacks may have the guest's state swapped into the engine.
        // Open against its normal network record after the command has unwound.
        let requests = std::mem::take(&mut *OPEN_ON_TICK.lock().expect("accessory requests"));
        for uuid in requests {
            if let Some(player) = host.player(uuid) {
                if player
                    .level(host)
                    .is_some_and(|level| level.dimension_id() == host.dimension())
                {
                    menu::open(host, player);
                } else {
                    OPEN_ON_TICK
                        .lock()
                        .expect("accessory requests")
                        .insert(uuid);
                }
            }
        }
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let _ = ENABLED.set(host, Holder::Player(event.player), server_mask());
    }

    fn player_logged_out(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        menu::close(host, event.player, None);
    }

    fn server_stopping(&self, host: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        menu::close_all(host);
        SERVER_MASK.store(ALL_SLOTS, Relaxed);
        CONFIG_CHANGED.store(false, Relaxed);
        OPEN_ON_TICK.lock().expect("accessory requests").clear();
    }

    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.loaded_from_disk
            && event.kind == "minecraft:armor_stand"
            && host
                .entity(event.entity)
                .and_then(|e| e.tags(host))
                .is_some_and(|tags| tags.iter().any(|tag| tag == menu::STAND_TAG))
        {
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("curios"));
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.as_slice() != ["curios"] {
            return Flow::Continue;
        }
        if let CommandSource::Player { player, .. } = event.source {
            OPEN_ON_TICK
                .lock()
                .expect("accessory requests")
                .insert(player.uuid());
        }
        Flow::Cancel
    }

    fn menu_closed(&self, host: &mut dyn ServerHost, event: &mut MenuClosedEvent) {
        menu::close(host, event.uuid, Some(event.window));
    }

    fn living_drops(&self, host: &mut dyn ServerHost, event: &mut LivingDropsEvent<'_>) -> Flow {
        let Some(uuid) = event.player else {
            return Flow::Continue;
        };
        if host.game_rule("minecraft:keep_inventory") != Ok(GameRuleValue::Bool(false)) {
            return Flow::Continue;
        }
        let Some(at) = host.player(uuid).and_then(|p| p.entity().position(host)) else {
            return Flow::Continue;
        };
        let holder = Holder::Player(uuid);
        let Ok(Some(mut saved)) = ACCESSORIES.get_existing(host, holder) else {
            return Flow::Continue;
        };
        let mut drops = Vec::new();
        for slot in menu::slots(server_mask()) {
            if let Some(cell) = saved.get_mut(slot as usize)
                && let Some(stack) = cell
                    .as_ref()
                    .and_then(|(item, count)| item.to_stack(*count))
            {
                *cell = None;
                drops.push(ItemDrop::new(stack, at, [0.0, 0.18, 0.0], 40).with_thrower(Some(uuid)));
            }
        }
        if !drops.is_empty() && ACCESSORIES.set(host, holder, saved).is_ok() {
            event.drops.extend(drops);
        }
        Flow::Continue
    }

    fn living_death_post(&self, host: &mut dyn ServerHost, event: &mut LivingDeathPostEvent<'_>) {
        if host.game_rule("minecraft:keep_inventory") != Ok(GameRuleValue::Bool(false)) {
            return;
        }
        let Some(player) = host
            .players()
            .server_players()
            .into_iter()
            .find(|player| player.entity().id() == event.entity)
        else {
            return;
        };
        let (Some(level), Some([x, y, z])) = (player.level(host), player.entity().position(host))
        else {
            return;
        };
        if player.game_mode(host) == Some(GameMode::Spectator) {
            return;
        }
        let holder = Holder::Player(player.uuid());
        // Some SDK player-death paths don't emit LivingDrops. Values already
        // transferred to that list are empty here. Journal the fallback item
        // spawns with the attachment write, so refusal cannot consume gear.
        let _ = host.transaction(|host, _| {
            let Some(mut saved) = ACCESSORIES.get_existing(host, holder).map_err(|_| ())? else {
                return Ok(());
            };
            let mut changed = false;
            for slot in menu::slots(server_mask()) {
                let Some(cell) = saved.get_mut(slot as usize) else {
                    continue;
                };
                let Some(stack) = cell
                    .as_ref()
                    .and_then(|(item, count)| item.to_stack(*count))
                else {
                    continue;
                };
                if level
                    .spawn_item_with_motion(host, x, y + 0.25, z, stack, [0.0, 0.18, 0.0], 40)
                    .map_err(|_| ())?
                    .is_none()
                {
                    return Err(());
                }
                *cell = None;
                changed = true;
            }
            if changed {
                ACCESSORIES.set(host, holder, saved).map_err(|_| ())?;
            }
            Ok::<(), ()>(())
        });
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let mask = host
            .local_player()
            .and_then(|p| api::enabled_slots(host, p))
            .unwrap_or(0);
        CLIENT_MASK.store(mask, Relaxed);
        if OPEN_REQUESTED.swap(false, Relaxed) && mask != 0 {
            let _ = host.send_chat("/curios");
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if let Some(bounds) = button(event.screen) {
            let texture = if contains(bounds, event.screen.mouse) {
                "button_highlighted"
            } else {
                "button"
            };
            event.draw.sprite(
                &format!("curios_horizon:gui/sprites/curios/{texture}"),
                [0.0, 0.0, 10.0, 10.0],
                bounds,
                None,
            );
            if contains(bounds, event.screen.mouse) {
                let _ = event
                    .draw
                    .gui_tooltip(&[Text::literal("Accessories")], event.screen.mouse);
            }
            return;
        }
        if !event
            .screen
            .menu
            .is_some_and(|id| id.starts_with(menu::PREFIX))
        {
            return;
        }
        let Some(data) = host
            .screen_extra_data()
            .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        else {
            return;
        };
        let mask = u16::from_le_bytes(data);
        let Some(menu) = host.local_player().and_then(|p| p.container_menu(host)) else {
            return;
        };
        let [x, y, _, _] = event.screen.rect;
        for (i, slot) in menu::slots(mask).enumerate() {
            if !matches!(menu.slot(host, i), Some(None)) {
                continue;
            }
            let bounds = [x + 8.0 + 18.0 * i as f32, y + 18.0, 16.0, 16.0];
            event.draw.sprite(
                "curios_horizon:gui/sprites/curios/inventory",
                [7.0, 7.0, 16.0, 16.0],
                bounds,
                None,
            );
            event.draw.sprite(
                &format!("curios_horizon:gui/sprites/curios/{}", slot.name()),
                [0.0, 0.0, 16.0, 16.0],
                bounds,
                None,
            );
            if contains(bounds, event.screen.mouse) {
                let title = format!(
                    "{}{}",
                    slot.name()[..1].to_ascii_uppercase(),
                    &slot.name()[1..]
                );
                let _ = event
                    .draw
                    .gui_tooltip(&[Text::literal(title)], event.screen.mouse);
            }
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if event.button == 0
            && button(event.screen).is_some_and(|bounds| contains(bounds, event.screen.mouse))
        {
            OPEN_REQUESTED.store(true, Relaxed);
            return Flow::Cancel;
        }
        Flow::Continue
    }
}

fn button(screen: &ScreenView<'_>) -> Option<[f32; 4]> {
    (CLIENT_MASK.load(Relaxed) != 0
        && screen.id == "inventory"
        && screen.menu.is_none()
        && screen.title.is_none())
    .then(|| [screen.rect[0] + 77.0, screen.rect[1] + 8.0, 10.0, 10.0])
}

fn contains([x, y, w, h]: [f32; 4], [mx, my]: [f32; 2]) -> bool {
    (x..x + w).contains(&mx) && (y..y + h).contains(&my)
}

struct SlotRules;

impl MenuPart for SlotRules {
    fn menu_subscribed(&self) -> &'static [Option<HookId>] {
        &[mod_api::hook_ids::menu_slot_rules]
    }

    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        event.max_stack = 1;
        let mask = event
            .context
            .player
            .and_then(menu::session_mask)
            .unwrap_or_else(|| CLIENT_MASK.load(Relaxed));
        if let Some(slot) = menu::slots(mask).nth(event.slot) {
            // The server's attachment-backed handler enforces the same rule for every write.
            event.may_place &= api::accepts(host, slot, event.stack);
        }
    }
}

pub fn screen_layout(menu_id: &str) -> Option<Screen> {
    let count = menu_id.strip_prefix(menu::PREFIX)?.parse::<usize>().ok()?;
    if !(1..=Slot::ALL.len()).contains(&count) {
        return None;
    }
    Some(Screen {
        width: 176.0,
        height: 132.0,
        inventory_x: 8.0,
        inventory_y: 50.0,
        slots: (0..count)
            .map(|i| Some([8.0 + i as f32 * 18.0, 18.0]))
            .collect(),
        ..Screen::default()
    })
}
