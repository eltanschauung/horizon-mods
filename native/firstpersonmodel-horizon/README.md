# FirstPersonModel Horizon

A client-side Horizon recreation of [tr7zw’s FirstPersonModel](https://github.com/tr7zw/FirstPersonModel).

Shows your animated character body and armor in first person with its head hidden.
**Mods → FirstPersonModel Horizon → Settings → Hands** chooses:

- **Vanilla** (default): unchanged first-person hands/items, with body arms, sleeves,
  arm armor and duplicate world-held items hidden only in first person.
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
