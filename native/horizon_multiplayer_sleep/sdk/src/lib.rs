// SPDX-License-Identifier: MIT
use mod_api::Hooks;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}

mod threshold;

pub use threshold::players_needed;
