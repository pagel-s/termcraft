# Nether refinement, 15 October 2026

[Open the before/after review](index.html). Implemented after `74c77fc` on `beta/terminal-reborn`.

The Nether now has dimension-specific ambient terrain fill and stronger actor fill. The player keeps the existing silhouette and equipment palette. Local flame tint is restrained on actors so a ghast stays ivory and the player stays distinguishable from the lava. Blazes cast local, occluded presentation light. Simulation light, spawning, saves and protocol are unchanged.

Fortress bricks and cutaway interiors use red/plum masonry. Netherrack has a quieter matte texture with coordinate-stable mirroring; soul sand has a separate pitted treatment, and glowstone has amber edges with bright cores. Glowstone uses this same material art in all dimensions. Pigmen have exposed bone, mossy skin and a gold weapon; ghasts have a shaded face, an attack mouth and curling tentacles; blazes have a brighter core and two rod poses. Source sampling, hit feedback, normal zoom defaults and existing combat/collision dimensions are retained. Orbiting rods and the held weapon can extend beyond a body.

The background uses recessed rock silhouettes at the same world scale and coordinates as terrain, without parallax. Sparse ambient embers rise in world space. The visual background has no collision and is distinct from the foreground. Full-height Overworld precipitation behavior is unchanged.

Review rounds corrected an initially repetitive rock pattern, grey corridor interiors, sharp background transitions at the origin and over-warm creature tint. Captures at overview and detailed scale were inspected directly, including smaller terminals. This is an implementation review; human visual/gameplay acceptance remains open.

Before images are copied from `../nether-preview-2026-10-15/`. After images use the same viewport, scene seeds, positions and normal overview. Generated cavern and fortress blocks are untouched. The separate creature lineup uses the same staged floor/positions as before.

```sh
cargo build --release --locked --example nether_review --example entity_review --example render_profile
env -u NO_COLOR target/release/examples/nether_review /tmp/nether-generated --creatures --animate
env -u NO_COLOR target/release/examples/entity_review /tmp/nether-entities
python3 scripts/capture_terminal_art.py /tmp/nether-generated /tmp/nether-images
python3 scripts/capture_terminal_art.py /tmp/nether-entities /tmp/nether-entity-images
env -u NO_COLOR target/release/examples/render_profile
```

The encounter stages creatures in collision-free air above actual generated terrain. The animation stages ages and attack-ready flags with fixed positions/camera; it is not simulated combat or human input. Its 72 incremental ANSI frames must be decoded in sequence with `Screen.feed` from `scripts/capture_terminal_art.py`, then rasterized using `raster`. GIF frame durations repeat 20/10/20 ms. No new artwork was generated outside the game renderer.

[Scene positions](scenes.json). [Exact source/binary/capture hashes, checks and profile results](evidence.json).

The counting-writer renderer profile excludes terminal presentation, kernel output, gameplay simulation, networking and model time. No general gameplay, physics, clean Windows/macOS or beta qualification is implied. New world generation is not required for these visual changes; they apply to existing Nether worlds.
