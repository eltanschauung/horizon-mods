// SPDX-License-Identifier: EUPL-1.2-or-later
// Native adaptation of Arcadian Dream by Maxmani and contributors.

use std::{
    collections::HashSet,
    sync::{Arc, LazyLock, Mutex, OnceLock},
};

use mod_api::serde_json::{self, Value, json};
use mod_api::{
    BlockEntityPart, BlockEntitySubmitEvent, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockEntityUpdateTagEvent, BlockUpdateFlags, ClientHost, Compound,
    CriterionContext, CriterionTest, CriterionTestError, Direction, Flow, GameMode, HookId, Host,
    HostSide, HostTransaction, Identifier, Ingredient, InteractionResult, ItemDisplayContext,
    ItemPlace, ItemResource, ItemStack, LevelRef, MenuContainer, MenuPart, MenuSlotRulesEvent,
    ModContainer, Registrar, RegistrationContext, RightClickBlockEvent, Screen, ScreenEvent,
    ServerHost, Tag,
};

const PILLAR: &str = "arcadiandream:onbashira";
const SHRINE: &str = "arcadiandream:ritual_shrine";
const TABLE: &str = "arcadiandream:danmaku_crafting_table";
const SEAL: &str = "arcadiandream:mysterious_seal";
const DISPLAY: &str = "ArcadianDisplayItem";

const PEDESTALS: [[i32; 3]; 16] = [
    [0, 0, -4],
    [3, 0, -3],
    [4, 0, 0],
    [3, 0, 3],
    [0, 0, 4],
    [-3, 0, 3],
    [-4, 0, 0],
    [-3, 0, -3],
    [-2, 1, -5],
    [2, 1, -5],
    [5, 1, -2],
    [5, 1, 2],
    [2, 1, 5],
    [-2, 1, 5],
    [-5, 1, 2],
    [-5, 1, -2],
];

pub fn register(registrar: &mut dyn Registrar, context: &RegistrationContext) {
    for id in [PILLAR, SHRINE, TABLE, SEAL] {
        let name = id.strip_prefix("arcadiandream:").unwrap();
        let kind = registrar.block_entity_type(name).blocks(&[id]).build();
        if let Err(error) = registrar.block_entity_part(&kind, Machine(id)) {
            context.refuse(&format!(
                "cannot attach Arcadian Dream machine {id}: {error:?}"
            ));
            return;
        }
        let block = registrar.block_key(id);
        if id == TABLE {
            registrar.block_container(
                &block,
                ModContainer {
                    slots: 7,
                    faces: std::array::from_fn(|_| Vec::new()),
                    inputs: vec![0, 1, 3, 4, 5, 6],
                    outputs: vec![2],
                    menu: Some(TABLE.to_owned()),
                },
            );
        } else if id != SEAL {
            registrar.block_container(
                &block,
                ModContainer {
                    slots: 1,
                    faces: std::array::from_fn(|_| vec![0]),
                    inputs: vec![0],
                    outputs: vec![0],
                    menu: None,
                },
            );
        }
    }
    registrar.menu_part(TABLE, TableSlots);
}

struct TableSlots;
impl MenuPart for TableSlots {
    fn menu_subscribed(&self) -> &'static [Option<HookId>] {
        &[Some(HookId::MenuSlotRules)]
    }

    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.slot >= 7 || event.container == MenuContainer::Player {
            return;
        }
        let item = event.stack.item().id(host);
        let ours = item.strip_prefix("arcadiandream:");
        let accepted = match event.slot {
            0 => ours.is_some_and(|item| item.ends_with("_bullet_core")),
            1 => ours.is_some_and(|item| item.ends_with("_shot")),
            3 => matches!(
                ours,
                Some(
                    "power_item"
                        | "big_power_item"
                        | "point_item"
                        | "star_item"
                        | "time_orb"
                        | "enchanted_ice"
                )
            ),
            4 => ours == Some("faith_item"),
            5 => ours.is_some_and(|item| item.ends_with("_pattern")),
            6 => item.starts_with("minecraft:") && item.ends_with("_dye"),
            _ => false,
        };
        event.may_place &= accepted;
        if event.slot == 1 {
            event.max_stack = event.max_stack.min(1);
        }
    }
}

struct Machine(&'static str);
impl mod_api::BlockPart for Machine {
    fn block_subscribed(&self) -> &'static [Option<HookId>] {
        &[]
    }
}
impl BlockEntityPart for Machine {
    fn block_entity_subscribed(&self) -> &'static [Option<HookId>] {
        &[
            Some(HookId::BlockEntityOnLoad),
            Some(HookId::OnChunkUnloaded),
            Some(HookId::BlockEntityTicks),
            Some(HookId::BlockEntityTick),
            Some(HookId::BlockEntityUpdateTag),
            Some(HookId::BlockEntitySubmit),
            Some(HookId::Screen),
        ]
    }

    fn block_entity_on_load(
        &self,
        host: HostSide<'_>,
        event: &mut mod_api::BlockEntityOnLoadEvent,
    ) {
        if self.0 == SEAL
            && matches!(host, HostSide::Server(_))
            && let Ok(mut seals) = SEALS.lock()
        {
            seals.insert(event.entity);
        }
    }

    fn on_chunk_unloaded(&self, host: HostSide<'_>, event: &mut mod_api::OnChunkUnloadedEvent<'_>) {
        if self.0 == SEAL
            && matches!(host, HostSide::Server(_))
            && let Ok(mut seals) = SEALS.lock()
        {
            seals.remove(&event.block_entity);
        }
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        *event.reply = BlockEntityTickReply::Unchanged;
        match self.0 {
            TABLE => {
                let mut slots = event.entity.items.clone();
                if crate::table_tuning::run(host, &mut slots) {
                    event.entity.items = slots;
                    *event.reply = BlockEntityTickReply::Changed;
                }
            }
            SEAL => tick_seal(host, event.pos),
            _ => {
                let displayed = event
                    .entity
                    .items
                    .first()
                    .and_then(Option::as_ref)
                    .and_then(|stack| stack.to_nbt(host))
                    .unwrap_or_default();
                let displayed = Tag::Compound(displayed);
                if event.entity.nbt.get(&DISPLAY.into()) != Some(&displayed) {
                    event.entity.nbt.insert(DISPLAY.into(), displayed);
                    *event.reply = BlockEntityTickReply::Changed;
                }
            }
        }
    }

    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        if self.0 != PILLAR && self.0 != SHRINE {
            return None;
        }
        let item = event
            .entity
            .nbt
            .get(&DISPLAY.into())
            .cloned()
            .unwrap_or_else(|| Tag::Compound(Compound::new()));
        Some([(DISPLAY.into(), item)].into())
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        if self.0 != PILLAR && self.0 != SHRINE {
            return;
        }
        let display = event
            .entity
            .nbt
            .get(&DISPLAY.into())
            .and_then(|data| match data {
                Tag::Compound(tag) => ItemStack::from_nbt(host, tag),
                _ => None,
            })
            .or_else(|| event.entity.items.first().cloned().flatten());
        if let Some(display) = display {
            let draw = &mut *event.draw;
            let time = draw.game_time() as f32 + draw.partial();
            draw.push();
            draw.translate([0.5, 1.1 + (time * 0.1).sin() * 0.04, 0.5]);
            draw.rotate_about([0.0, 1.0, 0.0], time * 2.0, [0.0; 3]);
            let _ = draw.item(&display, ItemDisplayContext::Ground, None, 0);
            draw.pop();
        }
    }

    fn screen(&self, _: &dyn Host, _: &mut ScreenEvent<'_>) -> Option<Screen> {
        (self.0 == TABLE).then(|| Screen {
            width: 176.0,
            height: 166.0,
            slots: vec![
                Some([8.0, 35.0]),
                Some([35.0, 35.0]),
                Some([149.0, 35.0]),
                Some([62.0, 17.0]),
                Some([62.0, 53.0]),
                Some([89.0, 17.0]),
                Some([116.0, 17.0]),
            ],
            inventory_x: 8.0,
            inventory_y: 84.0,
            ..Screen::default()
        })
    }
}

static SEALS: LazyLock<Mutex<HashSet<mod_api::BlockEntityRef>>> = LazyLock::new(Mutex::default);

pub fn tnt_primed(host: &mut dyn ServerHost, pos: [i32; 3]) -> Flow {
    let Some(level) = host.level(host.dimension()) else {
        return Flow::Continue;
    };
    let Ok(mut seals) = SEALS.lock() else {
        return Flow::Continue;
    };
    seals.retain(|seal| seal.position(host).is_some());
    if seals.iter().any(|seal| {
        seal.level(host) == Some(level)
            && seal
                .position(host)
                .is_some_and(|at| (0..3).all(|i| at[i].abs_diff(pos[i]) <= 16))
    }) {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

fn tick_seal(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let Some(ticks) = level.game_time(host) else {
        return;
    };
    if ticks.rem_euclid(40) != 0 {
        return;
    }
    if let Some(effect) = host.effect_key("minecraft:mining_fatigue") {
        for player in level.online_players(host).unwrap_or_default() {
            let entity = player.entity();
            if matches!(
                player.game_mode(host),
                Some(GameMode::Creative | GameMode::Spectator)
            ) || !entity
                .position(host)
                .is_some_and(|at| (0..3).all(|i| (at[i] - f64::from(pos[i])).abs() <= 16.0))
            {
                continue;
            }
            if let Some(living) = host.living(entity) {
                living.add_effect(
                    host,
                    &mod_api::ActiveEffect::new(effect, 200, 2, false, false, false),
                );
            }
        }
    }
    let Some(air) = host
        .block_key("minecraft:air")
        .and_then(|key| host.block_state_default(key))
    else {
        return;
    };
    for x in pos[0] - 16..=pos[0] + 16 {
        for z in pos[2] - 16..=pos[2] + 16 {
            for y in pos[1] - 16..=pos[1] + 16 {
                let at = [x, y, z];
                if level.block(host, at).is_some_and(|block| {
                    matches!(
                        block.id(host).as_str(),
                        "minecraft:fire" | "minecraft:soul_fire"
                    )
                }) {
                    level.set_block(
                        host,
                        at,
                        air,
                        BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                    );
                }
            }
        }
    }
}

fn pedestal(level: LevelRef, pos: [i32; 3]) -> ItemPlace {
    ItemPlace::BlockEntity {
        level,
        pos,
        side: Some(Direction::Up),
        slot: 0,
    }
}

pub fn interact(host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
    let HostSide::Server(host) = host else {
        return Flow::Continue;
    };
    let pos = event.click.pos;
    let uuid = event.click.player;
    let Some(player) = host.player(uuid) else {
        return Flow::Continue;
    };
    let Some(level) = player.level(host) else {
        return Flow::Continue;
    };
    let Some(block) = level.block(host, pos).map(|block| block.id(host)) else {
        return Flow::Continue;
    };
    if block != PILLAR && block != SHRINE {
        return Flow::Continue;
    }
    let position = pedestal(level, pos);
    if let Some(item) = position.stack(host) {
        let res = ItemResource::of(&item);
        let _ = host.transaction(|host, tx| {
            let mut accepted = 0;
            for slot in 0..36 {
                let target = ItemPlace::PlayerSlot { player: uuid, slot };
                accepted += target.insert(host, tx, &res, item.count() - accepted);
                if accepted == item.count() {
                    break;
                }
            }
            (position.extract(host, tx, &res, accepted) == accepted)
                .then_some(())
                .ok_or(())
        });
    } else if block == SHRINE {
        craft_ritual(host, level, pos, player);
    } else {
        let hand = ItemPlace::PlayerHand {
            player: uuid,
            hand: event.click.hand,
        };
        if let Some(item) = hand.stack(host) {
            let res = ItemResource::of(&item);
            let creative = player.game_mode(host) == Some(GameMode::Creative);
            let _ = host.transaction(|host, tx| {
                if position.insert(host, tx, &res, 1) != 1 {
                    return Err(());
                }
                (creative || hand.extract(host, tx, &res, 1) == 1)
                    .then_some(())
                    .ok_or(())
            });
        }
    }
    *event.cancellation_result = InteractionResult::SuccessServer;
    Flow::Cancel
}

fn craft_ritual(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    player: mod_api::PlayerRef,
) {
    static RITUALS: OnceLock<Value> = OnceLock::new();
    let recipes =
        RITUALS.get_or_init(|| serde_json::from_str(include_str!("../assets/arcadiandream/embedded/rituals.json")).unwrap());
    let Some(recipes) = recipes.as_object() else {
        return;
    };
    let mut pedestals = Vec::new();
    for offset in PEDESTALS {
        let position = [0, 1, 2].map(|axis| pos[axis] + offset[axis]);
        if level
            .block(host, position)
            .is_some_and(|key| key.id(host) == PILLAR)
        {
            let at = pedestal(level, position);
            if let Some(stack) = at.stack(host) {
                pedestals.push((at, stack));
            }
        }
    }
    if pedestals.is_empty() {
        return;
    }
    let dimension = level.dimension_id();
    for recipe in recipes.values() {
        if recipe["dimension"].as_str().is_some_and(|required| {
            required != "all"
                && dimension.strip_prefix("minecraft:").unwrap_or(dimension) != required
        }) {
            continue;
        }
        let Some(ingredients) = recipe["ingredients"].as_array() else {
            continue;
        };
        if ingredients.len() != pedestals.len() {
            continue;
        }
        let Some(ingredients) = ingredients
            .iter()
            .map(|spec| {
                let value = if let Some(item) = spec["item"].as_str() {
                    json!(item)
                } else if let Some(tag) = spec["tag"].as_str() {
                    json!(format!("#{tag}"))
                } else {
                    spec.clone()
                };
                host.recipe_ingredient(&value.to_string()).ok().flatten()
            })
            .collect::<Option<Vec<Ingredient>>>()
        else {
            continue;
        };
        let compatible = ingredients
            .iter()
            .map(|ingredient| {
                pedestals
                    .iter()
                    .map(|(_, stack)| ingredient.test(host, stack))
                    .collect()
            })
            .collect::<Vec<Vec<bool>>>();
        if !matches_ritual(&compatible) {
            continue;
        }
        let mut result = recipe["result"].clone();
        if result["id"].is_null() {
            result["id"] = result["item"].clone();
        }
        let Ok(Some(result)) = host.recipe_result(&result.to_string()) else {
            continue;
        };
        let crafted = host
            .transaction(|host, tx| {
                let inserted = pedestal(level, pos).insert(
                    host,
                    tx,
                    &ItemResource::of(&result),
                    result.count(),
                );
                if inserted != result.count() {
                    return Err(());
                }
                for (at, stack) in &pedestals {
                    if at.extract(host, tx, &ItemResource::of(stack), stack.count())
                        != stack.count()
                    {
                        return Err(());
                    }
                }
                Ok(())
            })
            .is_ok();
        if crafted {
            let _ = player.trigger_criterion(
                host,
                &Identifier::parse("arcadiandream:ritual_crafting").unwrap(),
                Arc::new(RitualCompleted),
            );
            return;
        }
    }
}

struct RitualCompleted;
impl CriterionTest for RitualCompleted {
    fn test(&self, _: &dyn CriterionContext) -> Result<bool, CriterionTestError> {
        Ok(true)
    }
}

fn matches_ritual(compatible: &[Vec<bool>]) -> bool {
    fn place(
        ingredient: usize,
        compatible: &[Vec<bool>],
        used: &mut [bool],
        assignment: &mut [Option<usize>],
    ) -> bool {
        for (slot, accepted) in compatible[ingredient].iter().enumerate() {
            if !*accepted || used[slot] {
                continue;
            }
            used[slot] = true;
            if assignment[slot].is_none_or(|old| place(old, compatible, used, assignment)) {
                assignment[slot] = Some(ingredient);
                return true;
            }
        }
        false
    }

    let mut assignment = vec![None; compatible.len()];
    (0..compatible.len()).all(|ingredient| {
        let mut used = vec![false; compatible.len()];
        place(ingredient, compatible, &mut used, &mut assignment)
    })
}
