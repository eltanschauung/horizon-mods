fn save_shot(shot: &crate::projectiles::Projectile, tag: &mut horizon_core::nbt::Tag) {
    crate::arcadian_motion::save(shot, tag);
}
