<p align="center"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/hero-phone.svg"><img src="../.github/releases/dynamiclights/hero.svg" alt="Better Dynamic Lights 1.1.0" width="640"></picture></p>

<p align="center"><a href="https://github.com/DeportedToHell/MCRE-Mods/releases/download/dynamiclights-1.1.0/dynamiclights-1.1.0.hmod"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/download-phone.svg"><img src="../.github/releases/dynamiclights/download.svg" alt="Download dynamiclights-1.1.0.hmod" width="500"></picture></a><br>
<a href="https://playhorizon.dev/mods/dynamiclights"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/catalog-phone.svg"><img src="../.github/releases/dynamiclights/catalog.svg" alt="Horizon catalog" width="500"></picture></a><br>
<a href="https://github.com/DeportedToHell/MCRE-Mods/releases/tag/dynamiclights-1.1.0"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/github-phone.svg"><img src="../.github/releases/dynamiclights/github.svg" alt="Add from GitHub" width="500"></picture></a></p>

<p align="center"><img src="../.github/releases/common/title-features.svg" alt="Features" width="640"></p>

<p align="center">
<img src="../.github/releases/dynamiclights/feat1.svg" alt="Torch in hand lights up" width="315">
<img src="../.github/releases/dynamiclights/feat2.svg" alt="Dropped lights keep shining" width="315">
<img src="../.github/releases/dynamiclights/feat3.svg" alt="Burning and glowing mobs" width="315">
<img src="../.github/releases/dynamiclights/feat4.svg" alt="Choose what glows" width="315">
</p>

<p align="center"><img src="../.github/releases/common/title-install.svg" alt="How to install" width="640"></p>

<p align="center"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/steps-phone.svg"><img src="../.github/releases/common/steps.svg" alt="Install from the file: download, put it in horizon-native-mods, Native build > Build" width="640"></picture></p>

<details>
<summary><b>All features in detail · Все возможности подробно</b></summary>

An original native mod (`.hmod`) on Horizon's mod API for [Horizon](https://github.com/horizon-rust/horizon), the Rust
rewrite of Minecraft: Java Edition 26.3: torches, lanterns and other glowing things light up the world as they move.

### Better than the built-in lights

Horizon has dynamic lights of its own (Options > Horizon Extras > Dynamic lights, off by default): items in your hands,
dropped items and burning mobs light up, with one on/off switch. Better Dynamic Lights does much more:

| | Horizon Extras | Better Dynamic Lights |
|---|---|---|
| Held items (yours, other players', mobs') and dropped items | yes | yes, also armor stands |
| Burning players, mobs and items | yes | yes |
| Glowing creatures: blazes, magma cubes, glow squids, allays, vexes | — | yes |
| Projectiles: fireballs, wither skulls, shulker bullets, spectral arrows, firework rockets | — | yes |
| Primed TNT, TNT and furnace minecarts | — | yes |
| Water puts out fire (lanterns keep shining) | — | yes |
| A switch for every kind of light | — | yes |
| Brightness, range and number of lights | — | yes |
| A key to turn the lights on and off | — | yes |
| Its own settings screen, our Mod Menu, English and Russian | — | yes |
| On by default | off | on |

### Features

- **Held items**: a glowing item in your main hand or off hand lights the blocks and mobs around you. So do
  glowing items held by other players, mobs (a zombie with a torch) and armor stands.
- **Dropped items**: a torch or a lantern lying on the ground keeps shining.
- **Burning entities**: players, mobs and items on fire shine as brightly as fire.
- **Glowing creatures**: blazes, magma cubes (brighter the bigger they are), glow squids (dark for a while after
  being hurt, as in vanilla), allays and vexes.
- **Projectiles**: fireballs, dragon fireballs, wither skulls, shulker bullets, spectral arrows and firework rockets.
- **TNT and minecarts**: primed TNT, a lit minecart with TNT and a running minecart with furnace.
- **Water puts out fire** (option): torches, campfires, lava buckets, fireballs and blazes give no light
  underwater; lanterns keep their flame behind glass.

An item glows as brightly as its block does when placed (torch 14, lantern and glowstone 15, soul torch 10…),
glow berries as cave vines with berries (14). The light is warm, like a torch's, and fades over one block per
level; where two lights meet, the place looks as bright as the brighter one, as block light does.

#### Settings

Options > Mods > Better Dynamic Lights > Config > **Settings: Open...** (or Configure in our Mod Menu) opens the
settings screen, an options screen in the game's own look, in English or Russian with the game's language:

| Option | Default | |
|---|---|---|
| Dynamic Lights | ON | the main switch (also on the mod's page) |
| Your Hands, Other Players, Mobs, Dropped Items, Burning Entities, Glowing Creatures, Projectiles, TNT and Minecarts | ON | each kind of light source |
| Water Puts Out Fire | ON | |
| Brightness | 100% | 25–100% of the full light level |
| Max Light Sources | 8 | how many of the nearest sources shine at once |
| Range | 32 blocks | 16–32 blocks from the camera |
| Toggle Dynamic Lights | not bound | a key that turns the lights on and off while playing |

Every change is kept at once (the main switch by the game, the rest one file per setting in
`horizon-mod-data/dynamiclights/`).

#### How it works, and its limits

Horizon's new mod API has no hook into its dynamic lights yet, so for now the lights are the API's frame lights
(`visuals`): the mod picks the sources each frame and the game's terrain and entity shaders light the world around
them (client side only; the light engine, chunks and saves never see them). Compared with 1.0.4 this means:

- **at most 8 lights, within 32 blocks** of the player (1.0.4: 16 and 96), so Max Light Sources and Range stop there;
- **coloured light rather than block light**: it is added on top of the world's light, so it shows a little in
  daylight too (where lights meet, the mod keeps the place as bright as the brighter one).

A fuller version comes when Horizon adds a dynamic-light hook for mods.

A client sees other entities' kind and place but not what they hold, whether they burn, what a dropped item is or
whether a minecart runs. The mod's server part sends those to the client as its own payloads: single player always
has it (your own game runs it), and so does a server with the mod installed. On a server without the mod, your own
hands, glowing creatures, projectiles and primed TNT still light up.

Horizon Extras' own Dynamic lights are separate from the mod: leave them off, as they are at first.

</details>

<details>
<summary><b>Install by hand and source code · Установка вручную и исходный код</b></summary>

### Install

From the launcher: Mods, find **Better Dynamic Lights**, Install, then Build. The launcher compiles only the mod,
against the prebuilt game it pins; the game itself is not rebuilt.

By hand: put `dynamiclights-<version>.hmod` in `<game folder>/horizon-native-mods/` and choose Build on the
launcher's Mods page.

Native mods run with your rights and are not sandboxed: build only code you trust. The same file goes on a server
(the facts the client can't see are its part).

### Source layout

```
horizon-mod.toml               the manifest: id, version, links, our Mod Menu's lines; [native] pins the Horizon build
Cargo.toml                     one crate on the pinned SDK's horizon-mod-api (.horizon/sdk/, fetched by cargo horizon-mod)
icon.png                       the icon (drawn by tools/icon.mjs)
assets/dynamiclights/lang/     the mod page's rows (Settings: Open..., the main switch), English and Russian
src/
  lib.rs                       the hooks: frame lights, ticks, keys, settings, screens, payloads
  runtime.rs                   settings, texts, light levels, the facts and their wire format: no game types (unit tests)
  lights.rs                    the light sources near the camera, the toggle key and its line over the hotbar
  net.rs                       what only the server knows, sent to the client as the mod's payloads
  store.rs                     the settings as the game keeps them
  screen.rs                    the settings screen
tools/                         icon.mjs
```

Build: `cargo horizon-mod build` writes `target/horizon-mod/dynamiclights.hmod`; `cargo horizon-mod check <file>`
checks it. Tests: `cargo test` (Rust 1.99.1, edition 2024).

Built for Horizon Nightly 2026-11-18 (`0b14f8acfd97fa2c750e42795249dcc981178ca4`), on its signed SDK.

</details>

<p align="center"><a href="https://github.com/DeportedToHell/MCRE-Mods/tree/main/betterdynamiclights"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/code-phone.svg"><img src="../.github/releases/common/code.svg" alt="Source code" width="245"></picture></a> <a href="https://github.com/DeportedToHell/MCRE-Mods/issues/new?template=bug-better-dynamic-lights.yml"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/bug-phone.svg"><img src="../.github/releases/common/bug.svg" alt="Report a bug" width="245"></picture></a></p>

## Licence

MIT License, Copyright (c) 2026 DeportedToHell ([LICENSE](LICENSE)). Written from scratch for Horizon. Not
affiliated with Mojang or Microsoft.

---

**По-русски.** Better Dynamic Lights — улучшенный динамический свет для Horizon. В самой игре есть встроенный
динамический свет (Настройки > Дополнения Horizon, по умолчанию выключен): светятся предметы в руке и на земле и горящие
мобы. Мод умеет больше: светятся всполохи, магмовые кубы, светящиеся спруты, тихони и вредины, файерболы, черепа визера,
светящиеся стрелы и фейерверки, подожжённый динамит и вагонетки с динамитом и печкой; под водой огонь гаснет, а фонари
продолжают светить; у каждого вида света свой переключатель, можно настроить яркость, дальность и число источников, есть
клавиша включения и свой экран настроек. Пока в новом API модов нет хука в динамический свет, поэтому сейчас: не больше 8 огней в 32 блоках
от игрока, и это цветной свет поверх мира, а не свет блоков (немного виден и днём). Полная версия выйдет, когда
Horizon добавит такой хук. Что держат другие, горение и предметы на земле знает только сервер: в одиночной игре и на сервере с модом светится всё, на сервере без
мода — ваши руки, светящиеся существа, снаряды и динамит. Настройки: Настройки > Моды > Better Dynamic Lights > Config >
«Настройки: Открыть...» (или «Настроить» в нашем Mod Menu).
