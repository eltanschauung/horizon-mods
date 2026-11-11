# FirstPersonModel Horizon

A client-side Horizon recreation of [tr7zw’s FirstPersonModel](https://github.com/tr7zw/FirstPersonModel).

Shows your animated character body, armor and held items in first person, with the
head hidden and the floating vanilla hands replaced. Uses Horizon’s existing
player poses and character scaling. Maps and spyglasses retain the normal view.
Toggle it through Horizon’s Mods menu. No server installation or gameplay changes.

Build: `python3 tools/build.py`. Install the generated `.hnmod` in
`~/.local/share/horizon/horizon-native-mods/`, enable it, and build the native client.
MIT; original concept and offset behavior by tr7zw. This is an independent Rust
recreation, not the original Java mod or its Java compatibility API.

Source: https://github.com/eltanschauung/firstpersonmodel-horizon
