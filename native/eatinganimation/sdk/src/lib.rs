use mod_api::Hooks;

#[derive(Default)]
struct EatingAnimation;

#[mod_api::horizon_mod]
impl Hooks for EatingAnimation {}
