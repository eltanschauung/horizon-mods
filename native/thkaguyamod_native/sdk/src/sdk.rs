//! Read snapshots for the original spell scripts and typed writes for their effects.

use mod_api::{
    ActiveEffect, EntityRef, GameMode, Host, HostTransaction, ItemPlace, ItemResource, LevelRef,
    PlayerRef, ServerHost, Text,
};

pub(crate) struct Caster {
    pub uuid: u128,
    pub entity_id: i32,
    pub dimension: String,
    pub position: [f64; 3],
    pub eye_height: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub game_mode: i32,
    pub health: f32,
    pub sneaking: bool,
    pub xp_level: i32,
    pub xp_progress: f32,
}

fn caster(host: &dyn ServerHost, player: PlayerRef) -> Option<Caster> {
    let entity = player.entity();
    let [yaw, pitch] = entity.rotation(host)?;
    let xp = player.experience(host)?;
    let game_mode = match player.game_mode(host)? {
        GameMode::Creative => 1,
        GameMode::Spectator => 3,
        GameMode::Adventure => 2,
        _ => 0,
    };
    Some(Caster {
        uuid: player.uuid(),
        entity_id: entity.id() as i32,
        dimension: player.level(host)?.dimension_id().to_owned(),
        position: entity.position(host)?,
        eye_height: entity.eye_height(host)? as f32,
        yaw,
        pitch,
        game_mode,
        health: host.living(entity)?.health(host)?,
        sneaking: player.is_secondary_use_active(host) == Some(true),
        xp_level: xp.level,
        xp_progress: xp.progress,
    })
}

pub(crate) struct EffectSpec<'a> {
    pub id: &'a str,
    pub amplifier: u8,
    pub duration: i32,
    pub ambient: bool,
    pub particles: bool,
}

pub(crate) fn block_id(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|key| key.id(host))
}

pub(crate) fn entities(host: &dyn Host, dimension: &str) -> Vec<(u32, String, [f64; 3])> {
    host.level(dimension)
        .and_then(|level| level.all_entities(host))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entity| {
            Some((
                entity.id(),
                entity.kind(host)?.id(host),
                entity.position(host)?,
            ))
        })
        .collect()
}

pub(crate) trait SpellHost {
    fn acting_caster(&self) -> Option<Caster>;
    fn casters(&self) -> Vec<Caster>;
    fn action_bar(&mut self, player: u128, message: Text);
    fn apply_effect(&mut self, player: u128, spec: &EffectSpec<'_>);
    fn hurt_in(
        &mut self,
        dimension: &str,
        victim: u32,
        damage: f32,
        kind: &str,
        source: Option<u32>,
    ) -> bool;
    fn push_mob(&mut self, dimension: &str, id: u32, delta: [f64; 3]);
    fn set_mob_health(&mut self, dimension: &str, id: u32, value: f32);
    fn spend_item(&mut self, player: u128, item: &str, amount: u32) -> bool;
}

impl SpellHost for dyn ServerHost + '_ {
    fn acting_caster(&self) -> Option<Caster> {
        caster(self, self.players().acting_player()?)
    }

    fn casters(&self) -> Vec<Caster> {
        self.players()
            .server_players()
            .into_iter()
            .filter_map(|player| caster(self, player))
            .collect()
    }

    fn action_bar(&mut self, player: u128, message: Text) {
        if let Some(player) = self.player(player) {
            let _ = player.action_bar(self, message);
        }
    }

    fn apply_effect(&mut self, player: u128, spec: &EffectSpec<'_>) {
        let (Some(player), Some(key)) = (self.player(player), self.effect_key(spec.id)) else {
            return;
        };
        if let Some(living) = self.living(player.entity()) {
            living.add_effect(
                self,
                &ActiveEffect::new(
                    key,
                    spec.duration,
                    i32::from(spec.amplifier),
                    spec.ambient,
                    spec.particles,
                    true,
                ),
            );
        }
    }

    fn hurt_in(
        &mut self,
        dimension: &str,
        victim: u32,
        damage: f32,
        kind: &str,
        source: Option<u32>,
    ) -> bool {
        let Some(victim) = resident(self, dimension, victim) else {
            return false;
        };
        let source = source.and_then(|id| self.entity(id));
        victim.hurt(self, kind, damage, None, source) == Some(true)
    }

    fn push_mob(&mut self, dimension: &str, id: u32, delta: [f64; 3]) {
        if let Some(entity) = resident(self, dimension, id) {
            entity.push(self, delta);
        }
    }

    fn set_mob_health(&mut self, dimension: &str, id: u32, value: f32) {
        if let Some(living) = resident(self, dimension, id).and_then(|entity| self.living(entity)) {
            living.set_health(self, value);
        }
    }

    fn spend_item(&mut self, player: u128, item: &str, amount: u32) -> bool {
        let Some(place) = [mod_api::Hand::Main, mod_api::Hand::Off]
            .into_iter()
            .find_map(|hand| {
                let place = ItemPlace::PlayerHand { player, hand };
                let stack = place.stack(self)?;
                (stack.item().id(self) == item).then_some((place, stack))
            })
        else {
            return false;
        };
        let resource = ItemResource::of(&place.1);
        self.transaction(|host, tx| {
            (place.0.extract(host, tx, &resource, amount) == amount)
                .then_some(())
                .ok_or(())
        })
        .is_ok()
    }
}

fn resident(host: &dyn ServerHost, dimension: &str, id: u32) -> Option<EntityRef> {
    let entity = host.entity(id)?;
    (entity.level(host)?.dimension_id() == dimension).then_some(entity)
}

pub(crate) const ITEMS: &[&str] = &[
    super::torch::GIGATORCH,
    "thkaguyamod:aja_red_stone",
    "thkaguyamod:amenonuhoko",
    "thkaguyamod:aqua_pearl",
    "thkaguyamod:arrow_shot",
    "thkaguyamod:big_light_shot",
    "thkaguyamod:big_power_item",
    "thkaguyamod:big_shot",
    "thkaguyamod:bloodthirsty_yin_yang_orb",
    "thkaguyamod:blue_pearl",
    "thkaguyamod:bomb_item",
    "thkaguyamod:buddha_stone_bowl",
    "thkaguyamod:butterfly_shot",
    "thkaguyamod:celestium_dust",
    "thkaguyamod:celestium_ingot",
    "thkaguyamod:cherry_item",
    "thkaguyamod:circle_shot",
    "thkaguyamod:closed_third_eye",
    "thkaguyamod:crest_of_yggdrasill",
    "thkaguyamod:crystal_shot",
    "thkaguyamod:cursed_decoy_doll",
    "thkaguyamod:death_scythe",
    "thkaguyamod:diffusion_amulet",
    "thkaguyamod:dragon_neck_jewel",
    "thkaguyamod:elly_scythe",
    "thkaguyamod:extend_item",
    "thkaguyamod:faith_item",
    "thkaguyamod:fire_rat_robe",
    "thkaguyamod:floating_hina_doll",
    "thkaguyamod:fox_mask",
    "thkaguyamod:fuku_no_kami_mask",
    "thkaguyamod:full_power_item",
    "thkaguyamod:gap",
    "thkaguyamod:gap_folding_umbrella",
    "thkaguyamod:gensokium_dust",
    "thkaguyamod:gensokium_ingot",
    "thkaguyamod:gensokyo_notes_arcane",
    "thkaguyamod:gensokyo_notes_dusty",
    "thkaguyamod:gensokyo_notes_imbued",
    "thkaguyamod:gensokyo_notes_normal",
    "thkaguyamod:gensokyo_notes_patched",
    "thkaguyamod:gensokyo_notes_ruined",
    "thkaguyamod:goa_enchanted_book",
    "thkaguyamod:goa_laevatein",
    "thkaguyamod:goa_miko_stick",
    "thkaguyamod:goa_third_eye",
    "thkaguyamod:green_pearl",
    "thkaguyamod:grilled_lamprey",
    "thkaguyamod:grimoire_book",
    "thkaguyamod:hakurei_gohei",
    "thkaguyamod:hakurouken",
    "thkaguyamod:hannya_mask",
    "thkaguyamod:heart_shot",
    "thkaguyamod:heavenly_peach",
    "thkaguyamod:hihiirokane",
    "thkaguyamod:hisou_sword",
    "thkaguyamod:homing_amulet",
    "thkaguyamod:hoshiguma_dish",
    "thkaguyamod:hoshiguma_dish_fire_resistance",
    "thkaguyamod:hoshiguma_dish_fire_resistance_long",
    "thkaguyamod:hoshiguma_dish_healing",
    "thkaguyamod:hoshiguma_dish_healing_2",
    "thkaguyamod:hoshiguma_dish_healing_3",
    "thkaguyamod:hoshiguma_dish_invisibility",
    "thkaguyamod:hoshiguma_dish_invisibility_long",
    "thkaguyamod:hoshiguma_dish_night_vision",
    "thkaguyamod:hoshiguma_dish_night_vision_long",
    "thkaguyamod:hoshiguma_dish_regeneration",
    "thkaguyamod:hoshiguma_dish_regeneration_2",
    "thkaguyamod:hoshiguma_dish_regeneration_2_long",
    "thkaguyamod:hoshiguma_dish_regeneration_3",
    "thkaguyamod:hoshiguma_dish_speed",
    "thkaguyamod:hoshiguma_dish_speed_2",
    "thkaguyamod:hoshiguma_dish_speed_2_long",
    "thkaguyamod:hoshiguma_dish_speed_3",
    "thkaguyamod:hoshiguma_dish_strength",
    "thkaguyamod:hoshiguma_dish_strength_2",
    "thkaguyamod:hoshiguma_dish_strength_2_long",
    "thkaguyamod:hoshiguma_dish_strength_3",
    "thkaguyamod:hoshiguma_dish_water_breathing",
    "thkaguyamod:hoshiguma_dish_water_breathing_long",
    "thkaguyamod:hourai_jeweled_branch",
    "thkaguyamod:hyottoko_mask",
    "thkaguyamod:ibaraki_box",
    "thkaguyamod:ibaraki_box_empty",
    "thkaguyamod:ibuki_gourd",
    "thkaguyamod:icicle_sword",
    "thkaguyamod:illness_charm",
    "thkaguyamod:impure_rock",
    "thkaguyamod:instant_hina_doll",
    "thkaguyamod:kaigo_stick",
    "thkaguyamod:kappa_cap",
    "thkaguyamod:kappa_water_pistol",
    "thkaguyamod:kappas_nostrum",
    "thkaguyamod:kinkakuji",
    "thkaguyamod:knife_shot",
    "thkaguyamod:kokoros_masks",
    "thkaguyamod:koomote_mask",
    "thkaguyamod:kunai_shot",
    "thkaguyamod:laevateinn",
    "thkaguyamod:light_shot",
    "thkaguyamod:long_laser",
    "thkaguyamod:magic_broom",
    "thkaguyamod:makaiium_dust",
    "thkaguyamod:makaiium_ingot",
    "thkaguyamod:maple_leaf_shield",
    "thkaguyamod:marisa_hat",
    "thkaguyamod:mask",
    "thkaguyamod:mask_of_hope",
    "thkaguyamod:max_doll_point",
    "thkaguyamod:medium_shot",
    "thkaguyamod:middle_laser",
    "thkaguyamod:miko_stick",
    "thkaguyamod:mini_hakkero",
    "thkaguyamod:miracle_mallet",
    "thkaguyamod:mochi_hammer",
    "thkaguyamod:momiji_scimitar",
    "thkaguyamod:monkey_mask",
    "thkaguyamod:nazrin_pendulum",
    "thkaguyamod:nazrin_stick",
    "thkaguyamod:nuclear_control_rod",
    "thkaguyamod:nue_trident",
    "thkaguyamod:old_celestial_spellcard",
    "thkaguyamod:old_demon_spellcard",
    "thkaguyamod:old_gensokyo_spellcard",
    "thkaguyamod:onbashira",
    "thkaguyamod:orange_pearl",
    "thkaguyamod:oval_shot",
    "thkaguyamod:point_item",
    "thkaguyamod:primordial_shield",
    "thkaguyamod:purple_pearl",
    "thkaguyamod:raiden_mask",
    "thkaguyamod:red_pearl",
    "thkaguyamod:rice_shot",
    "thkaguyamod:roukanken",
    "thkaguyamod:sakuya_stopwatch",
    "thkaguyamod:sakuya_watch",
    "thkaguyamod:scale_shot",
    "thkaguyamod:shimenawa_rope",
    "thkaguyamod:ship_ghost_dipper",
    "thkaguyamod:ship_ghost_dipper_empty",
    "thkaguyamod:short_laser",
    "thkaguyamod:shot_material",
    "thkaguyamod:shroom_slice",
    "thkaguyamod:silver_knife_blue",
    "thkaguyamod:silver_knife_green",
    "thkaguyamod:silver_knife_red",
    "thkaguyamod:silver_knife_white",
    "thkaguyamod:small_power_item",
    "thkaguyamod:small_shot",
    "thkaguyamod:small_star_shot",
    "thkaguyamod:soldified_paper",
    "thkaguyamod:sorcerer_sutra_scroll",
    "thkaguyamod:soul_torch",
    "thkaguyamod:spear_the_gungnir",
    "thkaguyamod:spell_card_adamant_helix",
    "thkaguyamod:spell_card_agunisyain",
    "thkaguyamod:spell_card_agunisyain_limited",
    "thkaguyamod:spell_card_annihilating_roar",
    "thkaguyamod:spell_card_anomalies_strength_disorder_and_spirits",
    "thkaguyamod:spell_card_bad_fortune",
    "thkaguyamod:spell_card_biorhythmofthe_misfortune_god",
    "thkaguyamod:spell_card_broken_amulet",
    "thkaguyamod:spell_card_broken_charmof_protection",
    "thkaguyamod:spell_card_catadioptric",
    "thkaguyamod:spell_card_curse_of_dreams_and_reality",
    "thkaguyamod:spell_card_cursed_hina_dolls",
    "thkaguyamod:spell_card_dark_sideofthe_moon",
    "thkaguyamod:spell_card_demarcation",
    "thkaguyamod:spell_card_diamond_blizzard",
    "thkaguyamod:spell_card_double_spark",
    "thkaguyamod:spell_card_dreadful_raging_waves",
    "thkaguyamod:spell_card_eternal_meek",
    "thkaguyamod:spell_card_exiled_doll",
    "thkaguyamod:spell_card_fafurotskies_no_kiseki",
    "thkaguyamod:spell_card_feng_wing_ascension",
    "thkaguyamod:spell_card_fenghuang_egg",
    "thkaguyamod:spell_card_final_spark",
    "thkaguyamod:spell_card_frost_columns",
    "thkaguyamod:spell_card_frost_king",
    "thkaguyamod:spell_card_fujiyama_volcano",
    "thkaguyamod:spell_card_gagouji_cyclone",
    "thkaguyamod:spell_card_great_raging_wavesofthe_reversed_scale",
    "thkaguyamod:spell_card_hailstorm",
    "thkaguyamod:spell_card_hells_artificial_sun",
    "thkaguyamod:spell_card_hells_wheelof_pain",
    "thkaguyamod:spell_card_hikouchuu_nest",
    "thkaguyamod:spell_card_houka_kenran",
    "thkaguyamod:spell_card_icicle_fall",
    "thkaguyamod:spell_card_kachoufuugetu",
    "thkaguyamod:spell_card_kachoufuugetu_limited",
    "thkaguyamod:spell_card_kappa_pororoca",
    "thkaguyamod:spell_card_kasho_no_eimin",
    "thkaguyamod:spell_card_kerochan_fuuu_ni_makezu",
    "thkaguyamod:spell_card_kitsune_tanuki_youkai_laser",
    "thkaguyamod:spell_card_knock_out_in_three_steps",
    "thkaguyamod:spell_card_kujizasi",
    "thkaguyamod:spell_card_lingering_cold",
    "thkaguyamod:spell_card_little_bug_storm",
    "thkaguyamod:spell_card_lunatic_red_slap",
    "thkaguyamod:spell_card_master_spark",
    "thkaguyamod:spell_card_master_spark_limited",
    "thkaguyamod:spell_card_mesh_light_darkness",
    "thkaguyamod:spell_card_mesh_light_darkness_limited",
    "thkaguyamod:spell_card_meteonic_shower",
    "thkaguyamod:spell_card_meteor_on_earth",
    "thkaguyamod:spell_card_midnight_bird",
    "thkaguyamod:spell_card_minus_k",
    "thkaguyamod:spell_card_miracle_fruit",
    "thkaguyamod:spell_card_misfortunes_wheel",
    "thkaguyamod:spell_card_mishagujisama",
    "thkaguyamod:spell_card_moonlight_ray",
    "thkaguyamod:spell_card_moses_miracle",
    "thkaguyamod:spell_card_musou_fuuin",
    "thkaguyamod:spell_card_musou_fuuin_limited",
    "thkaguyamod:spell_card_nami_to_tubu_no_kyoukai",
    "thkaguyamod:spell_card_night_bird",
    "thkaguyamod:spell_card_nijuu_kokushichou",
    "thkaguyamod:spell_card_non_directional_laser",
    "thkaguyamod:spell_card_old_lady_ohganes_fire",
    "thkaguyamod:spell_card_pain_flow",
    "thkaguyamod:spell_card_pendulum_guard",
    "thkaguyamod:spell_card_perfect_freeze",
    "thkaguyamod:spell_card_purification_river_deposit",
    "thkaguyamod:spell_card_red_magic",
    "thkaguyamod:spell_card_saikou_ranbu",
    "thkaguyamod:spell_card_satuzin_doll",
    "thkaguyamod:spell_card_scale_wave",
    "thkaguyamod:spell_card_scarlet_shoot",
    "thkaguyamod:spell_card_schoolof_fish",
    "thkaguyamod:spell_card_seijyahitumetu",
    "thkaguyamod:spell_card_starbow_break",
    "thkaguyamod:spell_card_stardust_reverie",
    "thkaguyamod:spell_card_stardust_reverie_limited",
    "thkaguyamod:spell_card_stormon_mt_ooe",
    "thkaguyamod:spell_card_tail_fin_slap",
    "thkaguyamod:spell_card_tail_fin_slap_limited",
    "thkaguyamod:spell_card_tidal_waveofthe_lake",
    "thkaguyamod:spell_card_yasaka_no_kamikaze",
    "thkaguyamod:spell_card_youryoku_spoiler",
    "thkaguyamod:spiritual_strike_talisman",
    "thkaguyamod:star_item",
    "thkaguyamod:star_shot",
    "thkaguyamod:stockpiling_misfortune",
    "thkaguyamod:sukuna_needle",
    "thkaguyamod:suwako_hat",
    "thkaguyamod:swallow_cowrie_shell",
    "thkaguyamod:sword_of_kusanagi",
    "thkaguyamod:talisman_shot",
    "thkaguyamod:tamahagane",
    "thkaguyamod:tengu_fan",
    "thkaguyamod:third_eye",
    "thkaguyamod:time_orb",
    "thkaguyamod:tiny_shot",
    "thkaguyamod:toyosatomimi_sword",
    "thkaguyamod:uba_mask",
    "thkaguyamod:ufo_blue",
    "thkaguyamod:ufo_green",
    "thkaguyamod:ufo_red",
    "thkaguyamod:volatile_string",
    "thkaguyamod:wall_passing_chisel",
    "thkaguyamod:white_pearl",
    "thkaguyamod:yellow_pearl",
    "thkaguyamod:yin_yang_orb",
    "thkaguyamod:yin_yang_orb_yukari",
    "thkaguyamod:youkai_book",
    "thkaguyamod:yuuka_parasol",
    "thkaguyamod:yuyuko_fan",
];
