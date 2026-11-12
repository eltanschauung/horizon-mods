# FirstPersonModel Horizon

A client-side Horizon recreation of [tr7zw’s FirstPersonModel](https://github.com/tr7zw/FirstPersonModel).

Shows your animated character body and armor in first person with its head hidden.
**Mods → FirstPersonModel Horizon → Settings → Hands** chooses:

- **Vanilla** (default): unchanged first-person hands/items, plus visible body arms, sleeves
  and arm armor. Body-held items are hidden to avoid rendering items twice.
- **Body**: the original full body arms and held-item rendering, replacing floating hands.

Uses Horizon’s existing poses and character scaling. Third-person views and other
players are unchanged. The choice persists and takes effect immediately, with no
per-frame disk reads. Body mode retains vanilla fallback for maps and spyglasses.
No server installation or gameplay changes.

Build: `python3 tools/build.py`. Install the generated `.hnmod` in
`~/.local/share/horizon/horizon-native-mods/`, enable it, and build the native client.
MIT; original concept and offset behavior by tr7zw. This is an independent Rust
recreation, not the original Java mod or its Java compatibility API.

Source: https://github.com/eltanschauung/firstpersonmodel-horizon

**Auto-hide body** defaults to On. It hides the body while swimming or looking above 60° in a view-angle convention where straight ahead is 90°, straight up 180°, and straight down 0°. Thus the body returns when looking at least 30° downward (Horizon camera pitch ≥30°). This never changes the camera or ordinary third-person rendering. Toggle it under Options → Mods → FirstPersonModel.

The same setting hides only the vanilla first-person empty-hand/fist overlay. The world-model arms and their sleeves remain visible whenever the first-person body is shown, including when unarmed and punching. Held-item animations are preserved. Turning the setting Off restores the vanilla fist overlay as well.
