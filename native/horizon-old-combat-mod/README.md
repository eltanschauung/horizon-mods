# Horizon Old Combat Mod

A native Rust recreation of [TheHangel’s Old Combat Mod](https://github.com/TheHangel/old-combat-mod), based on upstream revision `bf99cc2cb731a31a58f4518a9408bfc6aaa20173`.

Matches its behavior: attack-speed base 1024, the resulting old hand/equip animation, shields retained, and no sprint cancellation from attack knockback. `/gamerule old_combat true|false` switches it per world (default true). The mod uses normal attribute synchronization and only updates players on join/rule changes. It does not replace damage formulas, critical-hit conditions, or hit invulnerability rules. The base fork's former 50% recovery multiplier is removed; disabling this mod restores vanilla recovery.

Original implementation in Rust; no upstream Java code or assets are included.
