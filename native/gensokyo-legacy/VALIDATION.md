# Validation

- Input jar and reference source are pinned in `inventory.json` and `upstream.json`.
- All 17 biomes: direct terrain generation and decoration, first in isolation and
  then with BOP and Tectonic; zero unsupported or failed feature roots.
- Combined natural sampling: all 17 biomes reached across 196,608 climate samples.
- All imported models load; original grass, foliage, water, sky and fog tints match.
- 15 focused engine tests passed, including native write/error bounds, seeded
  placement queries, live patch suffixes, guarded climates and stable tier sizes.

Generation checks call the core generator directly and reject failures; they do
not accept the client's empty-chunk fallback. Commands: `python tools/prepare.py`,
`python tools/check_native.py --world-library`, `python tools/run_core_smoke.py`.
Use `--tectonic` on the compile check and `--bop` on preparation for the combined
case, then `python tools/run_core_smoke.py --combined [--natural]`.

Pond shapes and climate placement are adapted to Horizon's Overworld. This is not
a block-for-block Java dimension parity claim. Characters, mobs, quests and house
structures are outside this biome/content port.
