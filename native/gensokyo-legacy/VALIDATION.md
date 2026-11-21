# Validation

## SDK migration

The combined SDK package passed schema validation. Its three replacement hooks
passed the SDK planner, and the generated patch type-checked on the synchronized
fork.

The direct terrain probe on engine revision `d41d7ce` installed the three
SDK replacements and used a scratch resource snapshot with BOP and Tectonic
content. All 17 forced biomes generated and decorated with zero failed or
unsupported feature roots. All 17 appeared in 196,608 natural climate samples.
A frozen distribution still needs the combined runtime check.

## Previous engine baseline

- Input jar and reference source are pinned in `inventory.json` and `upstream.json`.
- All 17 biomes: direct terrain generation and decoration, first in isolation and
  then with BOP and Tectonic; zero unsupported or failed feature roots.
- Combined natural sampling: all 17 biomes reached across 196,608 climate samples.
- Full frozen package set: all 17 biomes generate with zero failed/unsupported
  roots; all 17 remain naturally reachable. Client chunks save/reopen correctly
  for all 17 forced biomes.
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
