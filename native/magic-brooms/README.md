# Magic Brooms for Horizon

**Original mod and assets by Iced Fox Studios.** This port uses Magic Brooms
3.0.0 for Minecraft Forge 1.18.2. The converted assets are included with
permission. See [NOTICE](NOTICE) for attribution.

Run `cargo-horizon-mod build` from `content/` to package the checked-in assets.
To regenerate the content from the original JAR, run
`python3 tools/assemble.py /path/to/magic-brooms-forge-1.18.2-3.0.0.jar`.
The resulting package registers as `horizon_magic_brooms` and requires a
Horizon build that includes the Magic Brooms engine integration.

Use a broom item to place it, right-click the placed broom to mount, and
sneak-right-click it to retrieve it. Move with the normal directional
controls, hold jump to climb, and hold sprint to descend. Sneak dismounts.
Use a scroll from the matching family while mounted to cast it.

The assembly bench accepts wood, a hay bale, and then optional infusion
and upgrade materials. The scribing table takes paper and a catalyst.
Use an empty hand to collect the output. The rack stores two brooms and
keeps their item data.
