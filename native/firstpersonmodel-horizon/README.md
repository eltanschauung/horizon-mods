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

**Auto-hide body** defaults to On. The first-person body hides while swimming or looking down beyond 80° pitch, then returns automatically. This never changes the camera, ordinary third-person rendering, or the vanilla first-person hands. Toggle it under Options → Mods → FirstPersonModel.

With Auto-hide body enabled, empty first-person arms (including punching fists and their sleeves) are hidden. Item-bearing arms and first-person item animations remain visible. Turning the setting Off restores the empty arms. Other players and third-person views are unaffected.
