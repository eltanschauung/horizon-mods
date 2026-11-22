fn suppress_vanilla_behavior(value: bool, m: &crate::mobs::Mob) -> bool {
    value || crate::alice_doll_goals::is_doll_mob(m)
}
