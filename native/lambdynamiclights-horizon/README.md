# LambDynamicLights Horizon

`lambdynamiclights-horizon` is an unofficial native Rust port of
[LambdAurora’s LambDynamicLights](https://github.com/LambdAurora/LambDynamicLights)
for Horizon 26.3. Its registry ID is `lambdynamiclights_horizon` (Horizon requires underscores in IDs). It builds into the client with horizon-mixins.

## Features

- Held, offhand, equipped and dropped luminous items, including light-emitting block items from content mods.
- Water-sensitive torches, campfires, lava buckets and fire charges; the brighter equipped light wins.
- Burning players, mobs, dropped items and projectiles; other players work on remote servers and hosted LAN games.
- Blazes, allays, magma cubes (squish-dependent), glow squids (hurt/dark timer), Endermen’s carried blocks,
  creeper charging, fireballs, wither skulls, spectral arrows and luminous thrown items.
- Armor stands, item frames, glow frames, item displays and block displays.
- Beacon and guardian beam lights; firefly and sonic-boom particle lights.
- Options → Mods → LambDynamicLights Horizon → Settings: Off / Fastest (500 ms) / Fast (250 ms) /
  Fancy (every rendered frame), entity and own lights, water checks, creeper/TNT modes, frames/displays, particles and beams.
  Creepers default to Simple, TNT to Off, and lighting to Fancy, following the MIT original.
- `/ldlights status` shows active renderer lights; `/ldlights reload` reloads light rules and settings locally.

Lighting stays client-side: no packets, chunk rebuilds, world changes, mob-spawn changes or dedicated-server install.
It uses Horizon’s existing GPU terrain/entity lighting and retains independent native colored lights, including Lunatic’s.
Turning this mod off returns to the original Horizon Extras dynamic-light behavior without changing that setting.

## Install and build

```sh
cargo test --locked --manifest-path native/lambdynamiclights-horizon/Cargo.toml
python3 native/tools/native.py package lambdynamiclights-horizon
```

Install `native/target/lambdynamiclights_horizon-0.1.0.hnmod` into the instance’s `horizon-native-mods`, then build its
selected native mods in the launcher. This port uses renderer internals as well as stable native hooks;
`compatibility.json` records the fork source and mixins revision actually tested. Rebuilds on a different source require validation.

Settings persist under `horizon-mods/.horizon-mod-data/lambdynamiclights_horizon/` using Horizon’s normal settings API.
Rules/settings are loaded once, with no per-frame filesystem reads. The two light consumers share one collection per frame.
Changing packs replaces the cached registry; changing a setting, disabling the mod or `/ldlights reload` invalidates it.
World/dimension changes discard cached source positions. Fast modes deliberately hold source positions for their update interval.

## Resource-pack rules

Read from the active, overlaid resource root at `assets/<namespace>/dynamiclights/item/*.json` and
`assets/<namespace>/dynamiclights/entity/*.json`. Packs replace matching files through Horizon’s normal pack layering;
files are applied in sorted namespace/path order after the built-in defaults. Invalid files are ignored without replacing a valid earlier rule.
Each JSON file is limited to 64 KiB. Reload packs normally, or use `/ldlights reload` after editing a prepared development root.

MIT item schema:

```json
{"item":"example:lantern","luminance":12,"water_sensitive":false}
```

`luminance` also accepts `"block"` (the item’s block) or a block ID, such as `"minecraft:lava"`.
The simple newer schema is supported too:

```json
{"match":{"items":["example:wand","example:lamp"]},"luminance":10}
```

Newer block forms `{"type":"block_self"}` and `{"type":"block","block":"minecraft:lava"}` also work.
Constant entity overrides use `{"match":{"type":"minecraft:blaze"},"luminance":5}`; selectors accept an ID or array.
Levels are integers 0–15. No rules execute code or fetch network resources.

## Differences from the Java mod

This is a port of the MIT-era behavior onto Horizon’s renderer, not a Fabric/NeoForge compatibility layer.
Horizon currently selects the nearest 16 white light sources and uses Minecraft-style falloff of one light level per block;
Java LambDynamicLights’ chunk-rebuild scheduler and 7.75-block attenuation are not used.
The newer Java API, arbitrary predicate/tag/component selectors, custom luminance codecs, beam geometry outside Horizon’s loaded entities,
and end-gateway beams are not implemented. Simple item/entity JSON overrides and Horizon’s native visual API provide extension points.

## Attribution and provenance

Original project and author: **LambDynamicLights — LambdAurora**, copyright © 2020 LambdAurora.
The implementation reference and bundled item definitions are the final MIT-licensed parent of the license change:
[`aecdeccc27aaaac38995c577b2605476b41d8ba4`](https://github.com/LambdAurora/LambDynamicLights/tree/aecdeccc27aaaac38995c577b2605476b41d8ba4).
The original MIT notice is preserved in `LICENSE` and in the native source bundle.
Later Lambda-licensed source/assets are not redistributed here. Rust rules, settings persistence, native integration,
GPU source selection, display/beam/particle integration, pack overrides, commands and tests were written for this Horizon port.
