fn necklace_pick(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<Option<(usize, f64)>> {
    use horizon_client::{
        game::{
            deflect::{attackable, pick_box},
            ray_box,
        },
        lan::playerdata::{ints_uuid, uuid_ints},
    };
    let own = game
        .lan_acting_uuid
        .map(uuid_ints)
        .or(game.level.player_uuid);
    let necklace = mod_arcadiandream::necklace_ready(
        own.map(ints_uuid).unwrap_or_default(),
        game.remote.is_some(),
    ) && !game
        .cooldowns
        .is_on_cooldown("arcadiandream:magatama_necklace");
    let Some((eye, direction)) = game.aim() else {
        return core::ops::ControlFlow::Break(None);
    };
    let remote_own = own.map(|uuid| format!("{:032x}", ints_uuid(uuid)));
    let is_own = |projectile: &horizon_client::projectiles::Projectile| {
        if game.remote.is_none() {
            return own.is_some() && projectile.owner_uuid == own;
        }
        remote_own.as_ref().is_some_and(|own| {
            projectile.item.as_ref()
                .and_then(|item| item.data()?.other("minecraft:custom_data"))
                .and_then(|data| data.get("arcadiandream")?.get("owner"))
                .is_some_and(|owner| matches!(owner, horizon_core::nbt::Tag::String(value) if value.to_string_lossy() == *own))
        })
    };
    let selected = game
        .creatures
        .projectiles
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            attackable(p)
                && (!p.kind.id().starts_with("arcadiandream:") || (necklace && !is_own(p)))
        })
        .filter_map(|(index, p)| {
            ray_box(eye, direction, game.entity_reach, pick_box(p)).map(|t| (index, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    core::ops::ControlFlow::Break(selected)
}
