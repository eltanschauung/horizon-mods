fn load_shot(shot: &mut crate::projectiles::Projectile, tag: &horizon_core::nbt::Tag) {
    crate::arcadian_motion::load(shot, tag);
}
