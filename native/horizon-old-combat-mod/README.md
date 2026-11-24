# Horizon Old Combat Mod

A native Rust recreation of [TheHangel’s Old Combat Mod](https://github.com/TheHangel/old-combat-mod), based on upstream revision `bf99cc2cb731a31a58f4518a9408bfc6aaa20173`.

Matches its behavior: attack-speed base 1024, the resulting old hand/equip animation, shields retained, and no sprint cancellation from attack knockback. `/gamerule horizon_old_combat_mod:old_combat true|false` switches it per world (default true). The SDK port uses normal attribute synchronization and writes only when a player’s attack speed needs to change. The fork imports the old world rule into the namespaced rule. It does not replace damage formulas, critical-hit conditions, or hit invulnerability rules. The base fork's former 50% recovery multiplier is removed; disabling this mod restores vanilla recovery.

Original implementation in Rust; no upstream Java code or assets are included.

The native SDK project is in `sdk/`. Run `cargo horizon-mod build` there with the pinned SDK.
