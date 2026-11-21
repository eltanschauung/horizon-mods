use mod_api::Hooks;

#[derive(Default)]
struct GensokyoLegacy;

#[mod_api::horizon_mod]
impl Hooks for GensokyoLegacy {}
