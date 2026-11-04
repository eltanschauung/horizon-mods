# No Copper

Native Horizon mod (`no_copper`, 0.1.0) that disables copper ore generation on the world host.
It removes regular and deepslate copper targets from ordinary/scattered ore features and disables entire copper ore-vein material rules,
including their raw-copper blocks. Iron veins and other ores remain unchanged. It runs after worldgen overrides, including Tectonic and data packs.

Install and build on the dedicated server, or on a client hosting a singleplayer/LAN world. Joining clients do not need it.
No blocks/items are registered and no content pack or additional network traffic is required.
Generation uses the world’s saved `no_copper` selection (enabled by default unless explicitly false).
Existing chunks change only when the separate offline converter is run.

## Package

```sh
cargo test --locked --manifest-path native/no-copper/Cargo.toml
python3 native/tools/native.py package no-copper
```

`native/target/no_copper-0.1.0.hnmod` is the source bundle. `compatibility.json` pins the tested Horizon fork and mixins revisions.

## Offline copper-to-coal conversion

`tools/convert.rs` uses Horizon’s own Anvil region/NBT codecs. Regular copper ore becomes coal ore;
deepslate copper ore becomes deepslate coal ore. It supports compact string palettes and compound palettes.
Inventories, entities, other blocks, palette indices, light/heightmaps and chunk timestamps are preserved.
Untouched chunks retain their original compressed bytes. It accepts modern padded block storage (Minecraft 1.16+ / Horizon 26.3).

With a sibling `horizon` checkout, build and test the tool:

```sh
cargo test --locked --profile fast --manifest-path native/no-copper/tools/Cargo.toml
cargo build --locked --profile fast --manifest-path native/no-copper/tools/Cargo.toml
python3 native/no-copper/tools/test_converter.py /path/to/copper-to-coal
```

Stop the world’s server and make a full backup before conversion:

```sh
copper-to-coal scan /path/to/world
copper-to-coal stage /path/to/world /new/staging/directory
copper-to-coal scan /new/staging/directory
```

The tool never writes to its input world. `stage` exports only affected terrain region files into a new directory, preserving relative paths.
It re-reads every changed chunk before export to verify NBT round trips and complete ore replacement.
After verifying the staged result, keep the server stopped, back up the original affected region files, and atomically replace them with their staged counterparts.
Audit the full world again before starting the server. Keep the backup and migration report.
The converter does not change raw copper blocks or copper items already saved in the world.

License: MIT.
