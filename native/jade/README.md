# Jade for Horizon

An unofficial port of [Jade](https://github.com/Snownee/Jade) 26.3.5 by Snownee to
[Horizon](https://github.com/horizon-rust/horizon): the look-at overlay for blocks and entities, its providers, its
settings under Options > Mods, and its keys.

## What differs from Jade

Rewritten in Rust for Horizon's hooks, so no original code runs. Its overlay, providers, themes, config screens, drag-to-place
screen, profiles, `/jade` and `/jadec` commands and the `jade:max_position_deviation` game rule are ported.

### Missing or not the same

- **Jade's Java API.** Java plugins for Jade can't run. Other mods register with this port's own provider functions.
- **Energy icon.** Jade's energy sprite isn't in Horizon's atlas, so the icon style of the energy line shows redstone dust.
- **The game rule** works through mixins on tables compiled into the game, so it is the part most likely to break on a
  Horizon update.

### Not tested

Jade's mouse binds, such as a mouse button for Show Overlay.

## Install

`jade-26.3.7.hnmod` in this repository is the packaged native mod, checked with `cargo horizon-mod check`. It's built
for the Horizon commit named below and needs no other mod.

## Build

This is a [Horizon](https://github.com/horizon-rust/horizon) native mod bundle: Rust mixins compiled into the game.
It was last tested on Horizon commit `6560641` (Minecraft 26.3, native hooks API 411), with AE2, JEI, Elevator, Iron Furnaces and Zoom.
It also builds on Horizon commit `fdc87ae` (Nightly 2026-11-08), together with AE2, JEI, Elevator, Iron Furnaces, Hostile Neural
Networks, Tinkers' Construct, Ender IO, Powah and Apothic Spawners (`native-fdc87ae-83d10460ef68`, all ten with none left out).
Native mods are unsandboxed code that runs with your rights, so read what you build.

Put each bundle's `mixins/<id>` folder in one folder, then:

```bash
horizon-launcher native-build "path/to/that/folder" --trust-native-code
```

## Status

An unofficial port, not made or endorsed by the original authors. Please report problems here, not to them.
It was tested on Windows, in single player and on LAN with one guest; this version not on a dedicated server. Other platforms were not tested.
A Horizon update can break it until it is rebuilt: it is built for the one Horizon commit named under Build.

## Credits and licence

Jade, by Snownee. This port is an adapted version, shared under the same licence,
[CC BY-NC-SA 4.0](LICENSE): noncommercial use only, and adaptations must keep this licence.

## Horizon fork additions

This copy is maintained in our horizon-mods repository from
[Joeypc6/Jade-Horizon](https://github.com/Joeypc6/Jade-Horizon), commit
`4ee222801ca156b508344ef3b2d739a0b0f2de6c`. Original Jade is by Snownee;
Joeypc6 maintains the upstream Horizon adaptation. License and review grant
are preserved unchanged. Fork changes retain CC BY-NC-SA 4.0.

**F3 Only: On/Off** defaults to **On** in both Jade settings pages. When On,
Jade suspends its overlay, target sampling and server-info requests unless the
actual F3 debug screen is open. Configuration remains accessible from Options
or `/jadec config`; disabling F3 Only restores normal Jade behavior. Existing
overlay visibility, GUI and player-list preferences still apply.

Package with `python3 native/tools/native.py package jade` from horizon-mods,
then install `native/target/jade-26.3.8+fork.1.hnmod` and use the Horizon frozen
client preflight/build/deploy workflow.

For local installation use `python3 native/jade/tools/package.py --install`.
This backs up the previous package and records this fork as the GitHub update
source, so upstream updates cannot silently discard F3 Only. Commit first.
