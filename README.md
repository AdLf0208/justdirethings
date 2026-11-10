# justdirethings (Horizon port)

A Horizon port of [justdirethings](Direwolf20-MC/JustDireThings) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- The Simple Coal Generator (`justdirethings:generatort1`) and the four coals it
- burns (`coal_t1..t4`).** It is the smallest machine family in the jar that the pin
- expresses end to end: one block, one insert-only FE buffer, one fuel slot, one
- tick, no neighbour reads, no GUI needed to prove it, no fluid and no entity. It
- is also the pack's first power source, so later slices (the Energy Transmitter,
- the Fuel Generator, the pocket generator, item automation) build on it. The jar
- inventory behind the choice, and the other families it ships, are in
- `FINDINGS.md`.

## Not yet ported

- Push-out to neighbours** (`providePowerAdjacent`, `fePerTick × 10` per side).
- API 142 gives `native_move_energy`; the sink is the Energy Transmitter.
- No GUI/menu.** Upstream opens `GeneratorT1Screen`; this slice has no menu
- type, so right-click does nothing and the fields are read by command.
- The Fuel Generator** (`generatorfluidt1`): 5,000,000 FE buffer, 5,000 FE/t,
- tiered fuel fluids (`fuel_tier2/3/4_fe_per_mb` 450/1300/4000). Needs
- `block_entity_fluids` tanks and a fluid capability.
- The Energy Transmitter** (`energytransmitter`): the network, per-side

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

Original mod: Direwolf20-MC/JustDireThings. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
