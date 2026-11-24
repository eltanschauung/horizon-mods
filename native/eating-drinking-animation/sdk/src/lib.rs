// SPDX-License-Identifier: MIT
use mod_api::Hooks;

pub mod state;

#[derive(Default)]
struct EatingDrinkingAnimation;

#[mod_api::horizon_mod]
impl Hooks for EatingDrinkingAnimation {}
