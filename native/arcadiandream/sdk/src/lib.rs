// SPDX-License-Identifier: EUPL-1.2-or-later

mod config;
mod danmaku;
mod fairies;
mod items;
mod loot;
mod machines;
mod models;
mod relics;
mod render;
mod table_tuning;
mod trades;
mod weapons;
pub use danmaku::{cancel_bullet, hit_target, number as shot_number};
pub use items::{read_tag as read_shot_data, write_tag as write_shot_data};
pub use relics::{block_ethereal_attack, life_ready, necklace_ready, revive_acting};

use mod_api::{
    BlockPart, BlockState, ClientHost, DataLoadContext, HookId, Hooks, Host, ItemPart, LevelHost,
    LootDropsEvent, LootTableLoadEvent, ModifyDefaultComponentsEvent, Registrar,
    RegistrationContext, ServerHost, StandaloneModelsEvent, StateForPlacementEvent, UseAnimation,
    UseAnimationEvent, UseDurationEvent,
};

#[derive(Default)]
struct ArcadianDream;

#[mod_api::horizon_mod]
impl Hooks for ArcadianDream {
    fn register(&self, registrar: &mut dyn Registrar, context: &RegistrationContext) {
        items::register(registrar);
        relics::register(registrar);
        fairies::register(registrar);
        weapons::register(registrar);
        config::register(registrar);
        let _ = registrar.register_custom_stat("arcadiandream:bullets_cancelled");
        machines::register(registrar, context);
        let key = registrar.item_key("arcadiandream:death_scythe");
        registrar.item_part(key, DeathScythe);
        let seal = registrar.block_key("arcadiandream:mysterious_seal");
        let water = registrar.fluid_key("minecraft:water");
        let flowing_water = registrar.fluid_key("minecraft:flowing_water");
        registrar.block_part(
            seal,
            MysteriousSeal {
                water,
                flowing_water,
            },
        );
    }

    fn setting_changed(
        &self,
        host: &mut dyn ClientHost,
        event: &mut mod_api::SettingChangedEvent<'_>,
    ) {
        config::changed(host, event.key);
    }

    fn register_villager_professions(
        &self,
        _: &RegistrationContext,
        event: &mut mod_api::RegisterVillagerProfessionsEvent<'_>,
    ) {
        trades::register(event);
    }

    fn data_pack_files(&self, _: &mut dyn ServerHost, event: &mut mod_api::DataPackFilesEvent<'_>) {
        trades::files(event);
        if relics::setting("nueTridentOptions", "canHaveImpaling", true) {
            event.files.push((
                "tags/minecraft:item/enchantable/trident".to_owned(),
                br#"{"values":["arcadiandream:nue_trident"]}"#.to_vec(),
            ));
        }
    }

    fn entity_interact(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::EntityInteractEvent<'_>,
    ) -> Option<mod_api::InteractionResult> {
        if let mod_api::HostSide::Server(host) = host {
            trades::interact(host, event);
        }
        None
    }

    fn level_tick_post(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::LevelTickPostEvent,
    ) {
        if let mod_api::HostSide::Server(host) = host {
            fairies::spawn(host, event.level);
            fairies::sync_variants(host, event.level);
        }
    }

    fn entity_tick_listened(
        &self,
        _: &mut dyn ServerHost,
        event: &mut mod_api::EntityTickListenedEvent,
    ) {
        event.listened = true;
    }

    fn entity_tick_post(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::EntityTickPostEvent<'_>,
    ) {
        if fairies::is_fairy(event.kind)
            && let Some(entity) = host.entity(event.entity)
        {
            fairies::tick(host, entity, event.kind);
        }
    }

    fn living_damage_post(
        &self,
        _: &mut dyn mod_api::LevelRandomHost,
        event: &mut mod_api::LivingDamagePostEvent<'_>,
    ) {
        if fairies::is_fairy(event.kind)
            && let Ok(entity) = u32::try_from(event.entity)
        {
            fairies::aggro(entity);
        }
    }

    fn render_living_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::RenderLivingPreEvent<'_>,
    ) -> mod_api::Flow {
        if render::draw_fairy(host, event.entity, event.draw) {
            mod_api::Flow::Cancel
        } else {
            mod_api::Flow::Continue
        }
    }

    fn render_living_post(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::RenderLivingPostEvent<'_>,
    ) {
        render::draw_hat(host, event.entity, event.draw);
    }

    fn living_entity_use_item_finish(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::LivingEntityUseItemFinishEvent<'_>,
    ) {
        relics::finish_use(host, event);
    }

    fn player_tick_post(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::PlayerTickPostEvent,
    ) {
        if let mod_api::HostSide::Server(host) = host {
            relics::tick(host, event.player);
            weapons::carried(host, event.player);
        }
    }

    fn living_death(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::LivingDeathEvent<'_>,
    ) -> mod_api::Flow {
        relics::protect_life(host, event)
    }

    fn mob_effect_applicable(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::MobEffectApplicableEvent,
    ) -> Option<mod_api::EffectApplicable> {
        relics::immunity(host, event)
    }

    fn register_commands(
        &self,
        _: &mut dyn ServerHost,
        event: &mut mod_api::RegisterCommandsEvent<'_>,
    ) {
        event.commands.extend(
            ["life", "necklace", "charm"]
                .map(|slot| mod_api::Command::new(&format!("arcadian unequip {slot}"))),
        );
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::RunModCommandEvent<'_>,
    ) -> mod_api::Flow {
        relics::command(host, event)
    }

    fn tnt_primed(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::TntPrimedEvent,
    ) -> mod_api::Flow {
        machines::tnt_primed(host, event.pos)
    }

    fn attack_entity(
        &self,
        host: &mut dyn ServerHost,
        _: &mut mod_api::AttackEntityEvent<'_>,
    ) -> mod_api::Flow {
        relics::block_ethereal_attack(host)
    }

    fn left_click_block(
        &self,
        host: &mut dyn ServerHost,
        _: &mut mod_api::LeftClickBlockEvent,
    ) -> mod_api::Flow {
        relics::block_ethereal_attack(host)
    }

    fn right_click_block(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::RightClickBlockEvent<'_>,
    ) -> mod_api::Flow {
        machines::interact(host, event)
    }

    fn loot_table_load(&self, _: &dyn DataLoadContext, event: &mut LootTableLoadEvent<'_>) {
        loot::extend(event.id, event.table);
    }

    fn loot_drops(&self, host: &mut dyn ServerHost, event: &mut LootDropsEvent<'_>) {
        loot::modify_drops(host, event);
    }

    fn standalone_models(&self, _: &RegistrationContext, event: &mut StandaloneModelsEvent<'_>) {
        models::register(event.models);
    }

    fn modify_default_components(
        &self,
        _: &dyn DataLoadContext,
        event: &mut ModifyDefaultComponentsEvent<'_>,
    ) {
        if event.item == "arcadiandream:hisou_sword" {
            let disables_shields = config::config()["hisouSwordOptions"]["canDisableShields"]
                .as_bool()
                .unwrap_or(true);
            event.components.insert(
                "minecraft:weapon".into(),
                mod_api::serde_json::json!({
                    "item_damage_per_attack": 1,
                    "disable_blocking_for_seconds": if disables_shields { 5.0 } else { 0.0 },
                }),
            );
        }
    }
}

struct DeathScythe;

impl ItemPart for DeathScythe {
    fn item_subscribed(&self) -> &'static [Option<HookId>] {
        &[
            Some(HookId::UseDuration),
            Some(HookId::UseAnimation),
            Some(HookId::UseItem),
            Some(HookId::ReleaseUsing),
        ]
    }

    fn use_item(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::UseItemEvent<'_>,
    ) -> Option<mod_api::InteractionResult> {
        let mod_api::HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let living = host.living(player.entity())?;
        living
            .start_using_item(host, event.hand)
            .then_some(mod_api::InteractionResult::Consume)
    }

    fn release_using(
        &self,
        host: mod_api::HostSide<'_>,
        event: &mut mod_api::ReleaseUsingEvent<'_>,
    ) {
        if let mod_api::HostSide::Server(host) = host {
            weapons::scythe_release(host, event);
        }
    }

    fn use_duration(&self, _: &dyn Host, event: &mut UseDurationEvent<'_>) {
        event.duration = 72_000;
    }

    fn use_animation(&self, _: &dyn ClientHost, event: &mut UseAnimationEvent<'_>) {
        *event.animation = UseAnimation::Bow;
    }
}

struct MysteriousSeal {
    water: mod_api::FluidKey,
    flowing_water: mod_api::FluidKey,
}

impl BlockPart for MysteriousSeal {
    fn block_subscribed(&self) -> &'static [Option<HookId>] {
        &[Some(HookId::StateForPlacement)]
    }

    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let direction = match event.context.face {
            mod_api::Direction::Up => "up",
            mod_api::Direction::Down => "down",
            mod_api::Direction::North => "north",
            mod_api::Direction::South => "south",
            mod_api::Direction::East => "east",
            mod_api::Direction::West => "west",
        };
        let fluid = event.fluid.fluid;
        let water = !event.fluid.is_empty() && (fluid == self.water || fluid == self.flowing_water);
        let state = level
            .state_with_property(event.state, "facing", direction)
            .and_then(|state| {
                level.state_with_property(
                    state,
                    "waterlogged",
                    if water { "true" } else { "false" },
                )
            });
        Some(state)
    }
}
