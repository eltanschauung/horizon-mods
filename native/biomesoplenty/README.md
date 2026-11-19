# Biomes O’ Plenty for Horizon

Source for the fork’s data assembler, Sengoku field integration and native feature prototype. The shared fork’s gameplay corrections are in `content-fixes/biomesoplenty/`. `source.json` records the original Biomes O’ Plenty JAR revision and asset provenance.

Build with the existing BOP assembly workspace. It contains the locally extracted `upstream/` data, Carry’s `import/carry/` output, `baked/data/` feature variants, and `tools/oracle/export/` registry dumps. For example, from the `horizon-mods` checkout on the project desktop:

```sh
python3 native/biomesoplenty/tools/build.py --workspace ../biomesoplenty-horizon
```

The assembler reads the Sengoku JAR next to that workspace and packages through `cargo-horizon-mod` from the sibling Lunatic checkout. `--sengoku-jar` and `--pack-tool` override those paths. Generated content stays in the selected workspace. The raw JARs, extracted assets and generated data remain local.

`--native-dev` assembles a separate prototype package. Its `mixins/` implements high grass and small fumaroles against the fork’s native feature interface. The playable assembly uses precomputed feature variants. The prototype covers only those two features.
