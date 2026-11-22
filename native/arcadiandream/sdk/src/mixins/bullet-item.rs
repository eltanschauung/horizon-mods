fn draw_shot(value: Option<ItemStack>, shot: &crate::projectiles::Projectile) -> Option<ItemStack> {
    crate::arcadian_motion::drawn(shot).or(value)
}
