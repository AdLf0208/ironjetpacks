# ironjetpacks (Horizon port)

A Horizon port of [ironjetpacks](BlakeBr0/IronJetpacks) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- Item** `ironjetpacks:jetpack`, the 26.1 single jetpack item fixed to the
- iron tier. It is a chest item (right-click equips it), unbreakable, stacks
- to 1, armor 3, and uses the upstream item model/texture and worn equipment
- Flight**: while the jetpack is in the chest slot and the jump key is
- held, the player rises. The vertical velocity is
- `min(motionY + accel, speedVert) * throttle * sprint` with the upstream
- acceleration/cap/water/sprint rules. The thrust ticks also zero the fall
- distance.

## Not yet ported

- One item, one tier.** 26.1 uses one `jetpack` item with a `jetpack_id`
- component and a synced registry of 14 tiers; the pin's content format
- cannot register that component, so the port ships the iron tier only and
- the item id itself is the tier. The creative tab lists only the iron-tier
- components (one advanced coil, one cell/thruster/capacitor).
- No HUD / item bar for the item-backed tank.** Slice 2 stores FE in the
- stack's own `minecraft:custom_data` (`horizon_energy`) and the commands
- report/set it, so it survives with the item; the pin's `item_bar` (API 307)

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: BlakeBr0/IronJetpacks. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
