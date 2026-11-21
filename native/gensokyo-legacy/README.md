# Gensokyo Legacy – Horizon

An LGPL-2.1 port of the 17 requested Gensokyo Legacy surface biomes, with their
custom natural blocks and authored vegetation templates, into the normal Overworld.
The mod ID and content namespace are `horizon_gensokyo_legacy`.

Build input: the jar pinned in `inventory.json`. Import it with
`python tools/import_jar.py /home/xxlib/Desktop/horizon-lunatic/gensokyolegacy-1.0.7.1-1.20.1.jar`, then run
`python tools/assemble.py`. The SDK project in `sdk/` contains the feature
and placement algorithms. `python tools/build.py` bundles them with the
maintained content snapshot as a single `target/horizon-mod/horizon_gensokyo_legacy.hmod`.
The packaging tool uses the Horizon SDK revision pinned in `sdk/horizon-mod.toml`.

Overworld placement adds its own regional pool (weight 4, the shared region grid
and a 1536-block minimum tier footprint) alongside BOP. Temperature, humidity, continentalness and erosion
separate fields, forests, lake/shores and mountain biomes. Terrain remains owned
by Horizon/Tectonic; this does not replace the noise router or add a dimension.
Only newly generated chunks gain these biomes.

See THIRD_PARTY_NOTICES.md for provenance and scope.

The original dimension's terrain is adapted to the host Overworld rather than
copied. Out-of-climate regions fall back to vanilla, so the new pool does not
replace deserts or cold biomes with temperate forests. Original plant-patch
holders reuse the player's current vanilla patch definitions at runtime.

Use the combined package in Horizon’s frozen-manifest workflow. Native changes
require a game rebuild. Verify all 17 biomes with Biomes O’ Plenty and Tectonic
in the frozen selection before deploying.
