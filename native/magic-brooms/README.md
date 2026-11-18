# Magic Brooms for Horizon

The Horizon port uses the supplied Iced Fox Studios Magic Brooms Forge 1.18.2
3.0.0 archive. Its metadata reserves all rights. Keep the archive and generated
assets private.

Run `python3 tools/assemble.py /path/to/magic-brooms-forge-1.18.2-3.0.0.jar`.
Build the generated `content/` folder with `cargo-horizon-mod build`.
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
