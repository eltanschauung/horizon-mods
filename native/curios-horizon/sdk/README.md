# Curios Horizon

Curios Horizon adds an accessory inventory to the Horizon 26.3 SDK. Open it from the Curios button in the survival inventory or run `/curios`. The dedicated server owns the slots and uses Horizon's ordinary inventory click and sync path.

## Server settings

The server creates `config/curios_horizon/slots.toml`. All nine slots are enabled by default. Set a slot to `false` to remove it from the menu and suspend its effects. Changes apply on reload. The player's item stays in its saved slot until the slot is enabled again. Enabled accessories join the normal death-drop list when Keep Inventory is off. Disabled accessories stay in the save.

## Mod API

1. Add your item to the appropriate item tag. For instance, to register a ring, ship `data/curios/tags/item/ring.json` containing `{"values":["example:my_ring"]}`. The slot names are `curio`, `head`, `necklace`, `back`, `bracelet`, `ring`, `belt`, `feet`, and `charm`. The generic Curio slot accepts items in any of those tags. The `curios:curio` tag makes an item eligible for every slot.
2. Use the public SDK interface in [src/api.rs](src/api.rs). For independently built native packages, include the small API module in your crate. It uses stable, typed Horizon attachment keys. Check that Curios Horizon is loaded if you support it as an optional dependency.
3. In your own server-side gameplay hooks, call `curios::equipped(host, player, curios::Slot::Ring)` and inspect the returned stack. The same read works against the client copy after synchronization. Curios Horizon holds the equipment. The item-owning mod implements and removes its own effects.

## Attribution

The slot and screen sprites come from [Curios API, branch 26.x](https://github.com/TheIllusiveC4/Curios/tree/26.x) at `103982397af5f622790f3799b72724085d6dbcd0`. Curios is copyright C4 and contributors. The original notices and LGPL terms are in [LICENSE](LICENSE), [COPYING](COPYING), and [COPYING.LESSER](COPYING.LESSER).
