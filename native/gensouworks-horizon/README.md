# Gensouworks Horizon

Horizon content port of ZZY’s Gensou Works 0.1.1, imported from the user-supplied `gensouworks-forge-1.20.1-ykh-0.1.1.jar`.

Includes all eight structures: Eientei, Kirisame House, Koumakan, Kourindou, Larva Cave, Margatroid House, Sanyousei New House and Signal Tower. All 30 templates and 21 jigsaw pools are retained. One random-spread structure set selects each structure with weight 1 (spacing 32 chunks, separation 8). A shared woodland/swamp/mountain biome union removes the original common/rare and per-structure biome bias. Selection is equally weighted, not a guarantee of identical counts in a finite explored area. Existing generated chunks are unaffected.

This is a data-driven Horizon `.hmod`, not a Forge mod or a native-code bundle. No engine rebuild is required. Internal resource IDs retain `gensouworks:`; the Horizon mod ID is `gensouworks_horizon`.

Missing other-mod furniture/plants are replaced with vanilla decorations. Original foreign entities/items are removed and mod-specific chest loot uses `minecraft:chests/simple_dungeon`. Optional mod integration is intentionally omitted. `conversion-report.json` lists every block substitution, template size and the original JAR checksum. Structure layouts and jigsaw connections remain intact.

Build: run `cargo-horizon-mod build` in this directory using the workspace’s cached packager, then check `target/horizon-mod/gensouworks_horizon.hmod`.

Reimport: install `nbtlib` in a development virtual environment and run `tools/import_jar.py ORIGINAL.jar --resources PREPARED_26_3_RESOURCES --output THIS_DIRECTORY` before building. No dependencies are needed at runtime.

## Original rights

The original JAR declares **All Rights Reserved**, author **ZZY**. Original structure assets retain those rights; this port does not relicense them. This directory is for the requested personal deployment. It is not submitted to the public Horizon registry. Public redistribution/registry submission requires the original author’s permission under `MOD-POLICY.md`.
