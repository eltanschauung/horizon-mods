// SPDX-License-Identifier: MIT
use mod_api::Hooks;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}

mod worldgen;

pub use worldgen::rewrite;
