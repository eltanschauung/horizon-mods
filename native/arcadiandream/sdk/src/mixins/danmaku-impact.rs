fn danmaku_impact(
    value: bool,
    host: &mut dyn mod_api::ServerHost,
    shot: &horizon_client::projectiles::Projectile,
    hit: horizon_client::projectiles::Hit,
) -> bool {
    use horizon_client::{mobs::Who, projectiles::Hit};
    let kind = shot.kind.id();
    if value || !kind.starts_with("arcadiandream:") || !kind.ends_with("_bullet") {
        return value;
    }
    let Hit::Entity { who, .. } = hit else {
        return true;
    };
    let victim = match who {
        Who::Mob(id) => host.entity(id),
        Who::Player => host.players().acting_player().map(|player| player.entity()),
    };
    let owner_uuid = shot
        .owner_uuid
        .map(horizon_client::lan::playerdata::ints_uuid)
        .and_then(|uuid| host.player(uuid))
        .map(|player| player.entity());
    let owner = owner_uuid.or_else(|| match shot.owner {
        Some(Who::Mob(id)) => host.entity(id),
        Some(Who::Player) => host.players().acting_player().map(|player| player.entity()),
        None => None,
    });
    let data = shot
        .item
        .as_ref()
        .and_then(|item| item.data()?.other("minecraft:custom_data"))
        .and_then(|tag| tag.get("arcadiandream"))
        .map(mod_arcadiandream::read_shot_data)
        .unwrap_or_default();
    if let Some(victim) = victim {
        let direct = host.entity(shot.id);
        mod_arcadiandream::hit_target(host, victim, owner, direct, kind, &data);
    }
    true
}
