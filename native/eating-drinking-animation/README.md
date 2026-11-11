# Eating & Drinking Animation

Original native Horizon mod (`eating_drinking_animation`). Raises the active item arm with a subtle eating/drinking bob and smoothly lowers it over six ticks when use ends. Works for remote players and your third-person model, both hands and left-handed players; sleeves and held items follow the same animated bones. First-person item animations are unchanged. Existing EatingAnimation item-frame textures remain compatible.

Uses effective consumable components and native animation overrides, including modded drinks. Eating crumbs use the animated held-item origin. No additional packets, server gameplay changes or dependencies. Disable from the mod menu. MIT.

Package: `python3 tools/build.py`. Test timing: `cargo test --offline` (use Horizon’s managed compiler/cache). Install the resulting `.hnmod`, freeze a manifest with `horizon/tools/fork.py`, preflight, build and deploy through that plan.
