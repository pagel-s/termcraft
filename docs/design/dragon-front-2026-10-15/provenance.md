# Front-facing Ender Dragon comparison, 15 October 2026

[Open the comparison](index.html). This is a selectable art preview after
`991be267768c3c915259e837854716826672644c`, requested before replacing the
side-facing design. The playable default and installed executable remain unchanged.

Both candidates use the real renderer in the same staged End scene: identical
seed files, terrain, player/dragon/crystal positions, camera, viewport, pose and
clock. PNGs are software rasterized from ANSI with square half-cell pixels. These
are renderer captures, not generated concept images or an emulator recording.

The front candidate has a horned wedge skull, two violet eyes, a projecting
muzzle, open fanged jaw, breast plates, tucked forward talons and ribbed purple
wings. Seven authored wing poses form the same twelve-step cycle as the side
view; head and torso stay fixed. Art is rasterized at compile time on the same
64×40 canvas. A first visual review flagged an overly flat snout and long hanging
foreclaws; the next pass adds a lit muzzle plane and shorter, raised forelegs.

The gallery distinguishes **enlarged review** from **current gameplay scale**.
The normal boss pixel view samples the source at half resolution. The explicit
review mode also permits detailed source-size art at 140×44 and above. Both designs
are shown at matching scale; the smaller 120×40 captures use the normal overview.
The wing clips contain 66 fixed-position frames sampled at 60 Hz, encoded using
a 20/10/20 ms pattern. They illustrate authored motion, not measured game FPS.

`Renderer::set_dragon_front_preview(true)` selects the candidate; `false` selects
the side design for the same comparison. Ordinary rendering has no preview mode
selected and retains the original side view. AI, movement, combat hit regions,
attacks, progression, saves, protocol and other creature art are unchanged.

```sh
cargo build --release --locked --example dragon_review
env -u NO_COLOR target/release/examples/dragon_review /tmp/dragon-side --detail
env -u NO_COLOR target/release/examples/dragon_review /tmp/dragon-front --front --detail
env -u NO_COLOR target/release/examples/dragon_review /tmp/dragon-front-overview --front
python3 scripts/capture_terminal_art.py /tmp/dragon-front /tmp/dragon-front-png
cargo test --locked dragon
```

[Exact source, example and capture hashes](../../assets/dragon-front-2026-10-15.json)
record validation. The existing renderer regression now checks both designs in
overview/detailed review, both travel directions, several wing phases and positions
at 120×40, 160×48 and 200×60, preserving visible eyes and unobstructed boss/armor UI.

This comparison does not qualify frontal combat. Before adopting it, align the
attackable head/torso with the new projection and review flight/attack direction,
crystal beams and full boss-fight readability. The current side-oriented combat
regions intentionally remain in place during this art-only selection. Glyph boss
fallback is unchanged. The front silhouette is deliberately symmetrical and may
need further depth/pose refinement after the user's visual preference is known.
