# Gensokyo Legacy – Horizon

- Follow sibling `horizon/AGENTS.md` for engine changes and deployment.
- This is the 17-biome Overworld port; preserve upstream attribution and LGPL notices.
- `tools/import_jar.py` imports only the biome dependency closure; `inventory.json`
  pins the supplied jar and all imported hashes. Do not substitute a newer jar silently.
- `tools/assemble.py` migrates codecs and namespaces into disposable `content/`.
- Use `tools/build.py` for the combined SDK package, then run Horizon's frozen
  preflight with Biomes O’ Plenty and Tectonic before installation. Verify
  decoration for all 17 biomes in a scratch world.
- Never treat a successfully packaged hmod as proof that world generation works.
