# Sleep Threshold

Native multiplayer sleep policy. One setting in **Options → Mods → Sleep Threshold → Settings** cycles through **One Player**, **>=10%**, **>=25%**, **>=33%** (default), **>=50%**, **>=75%**, **Vanilla**.

Percentages round up to a whole player: 50% of five eligible players requires three. Vanilla delegates to the world's `players_sleeping_percentage` gamerule (normally 100). Disabling the mod restores vanilla behavior. Spectators/ignored-player handling, deep sleep's 100-tick delay, dimension restrictions, time/weather rules and other mods' time-skip hooks are unchanged. A gamerule above 100 still disables night skipping.

The authoritative server/world host must build with this mod. A client's setting cannot override a remote server. Dedicated servers store the same single setting in `horizon-mods/.horizon-mod-data/sleep_threshold/threshold`, containing one of the labels above. Changes are noticed within one second; the integrated server also observes menu changes. No new chunks/world reset is needed.

Package: `python3 native/tools/native.py package sleep-threshold`. Install the resulting `.hnmod` and rebuild the game/server. This mod targets the pinned sleep-count method as well as native menu hooks; new Horizon revisions should be checked for compatibility.

Validation: two policy/settings tests pass, mixin application succeeds and the patched game passes `cargo check` on the pinned upstream Horizon revision. No live dedicated server was changed.
