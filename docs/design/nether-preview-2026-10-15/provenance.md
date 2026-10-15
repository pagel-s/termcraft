# Current Nether preview, 15 October 2026

[Open the preview](index.html). Captured after `a8fdd64688b98b6f59d412ff2425be3d21a3304a` using the existing native renderer. This checkpoint adds a disposable capture example and documentation; it does not change gameplay, terrain generation, lighting, creature art or the installed executable.

`examples/nether_review.rs` opens temporary seeded worlds, loads the real Nether and stages only the player/camera. The generated cavern is at x=0.5, y=60; the fortress gate is at x=528.5, y=50. Both use seeds `[1337,7331,9917,4444]` and untouched generated blocks. Captures cover normal overview at 160×48 and 200×60.

`examples/entity_review.rs` supplies a separate staged creature lineup with seeds `[42,73,99,44]`: netherrack floor, a small lava patch, pigman, player, blaze and ghast. This lineup demonstrates current creature art and scale; it is not a generated gameplay encounter.

```sh
cargo build --release --locked --example nether_review --example entity_review
env -u NO_COLOR target/release/examples/nether_review /tmp/nether-generated
env -u NO_COLOR target/release/examples/entity_review /tmp/nether-entities
python3 scripts/capture_terminal_art.py /tmp/nether-generated /tmp/nether-images
python3 scripts/capture_terminal_art.py /tmp/nether-entities /tmp/nether-entity-images
```

PNGs are software rasterizations of actual ANSI output with square half-cell pixels, rather than terminal-emulator screenshots. They do not establish input latency, live frame rate, combat quality or art acceptance. The current dark terrain, flat backdrop and small creature silhouettes are preserved for an honest review.

The earlier front-dragon preview uses a one-block-wide, four-block-high pillar placeholder (`examples/dragon_review.rs`). Generated End columns (`src/world/mod.rs`, `build_end_chunk_with_noise`) are five blocks wide with seven-block crowns and tops 11–16 blocks above the base, plus buried foundations and a walking cutaway. The earlier art comparison therefore does not demonstrate final arena proportions.

[Scene positions](scenes.json). [Capture hashes and validation](evidence.json).
