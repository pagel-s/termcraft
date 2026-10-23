# Creature physics and room review, 12 October 2026

Unpublished development checkpoint following `cfb2f3e` on `beta/terminal-reborn`.

## Review rounds and fixes

The physics reviewer identified feet-center-only Ghast/Blaze collision, undersized
spawn pockets, NPC fluid control inheriting human jump input, and villager home
steering cancelling recoil. Full-body gravity-free sweeps now stop side/head
impacts at actual terrain shapes, including fast movement and slab/stair air.
The shared tree-platform policy applies to both sweep and overlap recovery.
Natural/spawner clearance rejects pockets that cannot fit the flying body. That
spawn check remains conservative at occupied slab/stair voxels.

Ground NPCs explicitly use their own jump input. Villager home/shelter steering
and voluntary wall jumps wait through hurt recovery; farm-animal wall jumps do
likewise. New tests exercise both flight directions, head-only ceilings, fast
knockback, shape/tree behavior, full AI ticks, rejected spawn pockets, daytime and
nighttime villager recovery, and player-independent NPC fluid movement.

The presentation reviewer identified row-local false cutaway walls behind the
ruins and starter shelter. Room qualification now requires full side boundaries
and a continuous roof. Open architecture retains sky; complete huts retain their
backdrop. Wooden interiors stay warm, masonry uses muted gray, and transparent
prop pixels keep the room wall behind them. End tunnels retain their short
obsidian recess. Tests cover enclosure, glass/open doors/torches, incomplete ruins,
roof gaps, floating props and the End recess.

Compact objectives retain the complete actionable goal; one contextual hint covers
climbing, chest transfer, bridge placement and the signal. Other active status
keeps priority. A renderer regression checks the 80-column goal and ladder hint.

## Evidence and reproduction

The [gallery](index.html) contains fourteen actual renderer/terminal frames.
The enclosed village before/after is expected to retain its appearance; the open
ruin/shelter pairs show removal of false panels. Earlier frames identify source
`cfb2f3e` and executable `84cdff989a0052ad19aa9baebec419a5f0d7d61f1658d0bc8e0311eaea6010e1`.
Focused examples stage positions; no AI-generated artwork or substitute mockup is
used. `flight.json` records the full-body stop positions after one real engine tick.

```sh
cargo build --release --locked --bins --examples
env -u NO_COLOR target/release/examples/lantern_review /tmp/lantern-ansi
env -u NO_COLOR target/release/examples/pixel_review /tmp/worlds-ansi
env -u NO_COLOR target/release/examples/flight_review /tmp/flight-ansi
python3 scripts/capture_terminal_art.py /tmp/lantern-ansi /tmp/lantern-png
python3 scripts/capture_terminal_art.py /tmp/worlds-ansi /tmp/worlds-png
python3 scripts/capture_terminal_art.py /tmp/flight-ansi /tmp/flight-png
python3 scripts/test_lantern_trial.py --binary target/release/termcraft --report /tmp/lantern.json
env -u NO_COLOR target/release/examples/render_profile > /tmp/renderer.json
```

The separate normal-rule CLI test again completes the trail, all three guards,
looting, eight-block bridge, signal and save/reopen. A real native PTY starts
`trial native-review`, checks the 80-column controls, resizes to 160×48, walks and
jumps through ordinary input, saves/exits and verifies the saved movement and
airborne position through the test-observation interface. It also checks terminal restoration.

[Exact source/binary and qualification](../../assets/room-physics-2026-10-12.json)
record 622 tests on stable/Rust 1.88, strict Clippy, TCP, native/co-op input, SSH and
renderer work. The new enclosed-room profile is separate from earlier benchmarks;
CPU/ANSI time excludes emulator presentation, simulation, network and model time.
The final native archive has its separate hash/report in `dist`.

## Limits

No human acceptance, full survival campaign, real WAN or clean Windows/macOS
qualification is claimed. Player land tuning is unchanged by this pass; the
physics changes target creature collision, recoil and input ownership. Sweeps are
axis-separated, like the walking solver. Native files remain individually atomic,
not a whole-world transaction. Protocol 12 and progression v7 remain unchanged.
