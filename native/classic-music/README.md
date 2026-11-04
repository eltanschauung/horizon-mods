# Classic Music

A client-only native Horizon mod (`classic_music`, 0.1.0) that replaces Minecraft background-music pools before playback.

## Playlists

Every Overworld background pool—including biome-specific, creative and underwater music—uses exactly these 21 tracks:

Clark; Danny; Dry Hands; Haggstrom; Key; Living Mice; Mice on Venus; Minecraft; Oxygene; Subwoofer Lullaby;
Sweden; Wet Hands; Aria Math; Biome Fest; Blind Spots; Dreiton; Haunt Muskie; Taswell; Axolotl; Dragon Fish; Shuniji.

The main menu uses only Beginning 2, Floating Trees, Moog City 2 and Mutation.

Nether background pools use only Concrete Halls, Dead Voxel, Ballad of the Cats and Warmth—the four original Nether tracks.
The End, dragon fight and credits retain The End, Boss and Alpha, respectively.

**Music discs are untouched**, including newer discs. Other mods’ own music events are untouched.

## Behavior

- Whole pools are replaced, so a biome that originally offered only newer music gets classic music rather than an empty pool.
- Replacement occurs before sound resolution/channel playback. A previously selected disallowed background track is reselected from its context’s approved pool.
- Normal music timing, biome volume, fading, loading-screen behavior and the player’s volume/frequency settings remain in use.
- Turning Classic Music off in Options → Mods restores the exact original sound-event definitions. Resource reloads rebuild the filter from the newly loaded registry.
- Cached playlists have no per-frame file reads and require no network messages, server mod or content-registry changes.

It uses music already present in the client’s prepared Minecraft assets. No Minecraft audio is redistributed.
The filter operates on track IDs; a resource pack can replace the audio behind an allowed ID. Missing client assets cannot be synthesized.

## Build and validate

```sh
cargo test --locked --manifest-path native/classic-music/Cargo.toml
python3 native/classic-music/test.py --horizon /path/to/horizon --resources /path/to/prepared/26.3
python3 native/tools/native.py package classic-music
```

Install `native/target/classic_music-0.1.0.hnmod` into the client instance’s `horizon-native-mods`, then build its selected native mods in the launcher.
`compatibility.json` records the fork and mixins revisions tested together; this mod uses sound-registry internals as well as native hooks.

`test.py` tests the real Horizon SoundRegistry with the mod’s modules: exact pool sizes, randomized resolution from real assets,
newer-only biome replacement, dimension matching, unchanged disc events, reload/clone behavior and exact restoration when disabled.
It writes only ignored test artifacts and does not open a world or contact a server.

License: MIT.
