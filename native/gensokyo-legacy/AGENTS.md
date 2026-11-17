# Gensokyo Legacy – Horizon

- Follow sibling `horizon/AGENTS.md` for engine changes and deployment.
- This is the 17-biome Overworld port; preserve upstream attribution and LGPL notices.
- `tools/import_jar.py` imports only the biome dependency closure; `inventory.json`
  pins the supplied jar and all imported hashes. Do not substitute a newer jar silently.
- `tools/assemble.py` migrates codecs and namespaces into disposable `content/`.
- Run `tools/check_native.py` and `tools/run_smoke.py` before installation; both use
  private scratch directories and the managed native compiler/cache.
- Never treat a successfully packaged hmod as proof that world generation works.
