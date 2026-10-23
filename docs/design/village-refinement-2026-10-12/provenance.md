# Connected village review, 12 October 2026

Development checkpoint after `766cc424e40fbb4092cf80b20127963e2fee40eb` on
`beta/terminal-reborn`. This is a candidate for human playtesting, not a published beta.

## What changed and what the reviewers caught

The previous single hut cramped furniture and players into a narrow room. Fresh
ordinary Overworld generation now selects a whole site, grades its road ends, and
clips the same deterministic settlement into each intersecting chunk. Three distinct
buildings share a covered market gate, square and underground irrigated garden.
A cottage has a left loft, the workshop a stone roof and right loft, and the pale
farmhouse a porch and right loft. Framed windows and two-block doors retain real
collision. Street-level corridors remain clear; normal ladder movement reaches
storage, beds and workshop stations. No serialized fields or protocol changes.

Two read-only reviewers checked art and gameplay. The first capture pass prompted
brighter back walls, mirrored lofts, material variety, taller headroom, a restrained
workshop roof, mature crop variation and a clearer communal landmark. Actual tests
caught dry irrigation (source water is level 8), loft interaction/headroom issues,
and outdated roofless home fixtures. Opposite doors now belong to one home, and
unconnected workshop furniture cannot make the market gate a home. Daytime home
bias points outside the entry rather than toward its covered door.

Final image review caught a cropped workshop eave and dark distant loft stations.
Centering the capture and placing real torches above loft chests resolved both.
The final reviewer found no remaining blocker for this village pass. The market
reads as a gatehouse; desert masonry still has limited biome-specific identity.

## Captures and reproduction

[Gallery](index.html): 22 frames. The old hut is from the preceding `766cc42` review
(binary `77750078e8b96e23b5f8d7d937f0085034001323496683516f32e48c72e8cd15`).
It is a historical layout comparison, not identical before/after geometry.
New village images use ordinary generation with seeds `[1337,7331,9917,4444]`,
center −8224 and ground 31. Player/villager poses and screenshot time are staged;
terrain is generated without a recipe. The 480×32 views are wide virtual overviews.
80×24 uses the existing glyph fallback; 120×40 and 160×48 use pixel rendering.
Cave/Nether/End regression scenes stage their geometry and actors separately.
The blank rasterizer contact sheet is excluded.

```sh
cargo build --release --locked --bins --examples
env -u NO_COLOR target/release/examples/village_review /tmp/village-ansi
python3 scripts/capture_terminal_art.py /tmp/village-ansi /tmp/village-png
python3 scripts/test_village_walk.py --binary target/release/termcraft --fixture /tmp/village-ansi/village.json --report /tmp/village-native.json
env -u NO_COLOR target/release/examples/village_profile
env -u NO_COLOR target/release/examples/render_profile
```

The example prints a separate review save path. Run with `TERMCRAFT_SAVE_DIR` set
to that path to begin immediately outside the village. The focused save disables
spawning/daylight; O settings can enable those rules. Ordinary new saves do not
guarantee a village near spawn. None of the review tools opens normal user saves.

## Gameplay and qualification

The native test copies the generated review save, walks 90.67 blocks, opens all six
building doors with real SGR mouse clicks, and sends zero jump events. Feet remain
at height 31 throughout the level route. It observes live position through a
read-only test-observation connection and verifies terminal restoration on exit.
Separate engine tests walk both directions, climb to lofts, open chest/furnace,
sleep and return to the saved spawn, and exercise villagers entering/leaving homes.
World tests cover negative chunk load order, support, graded road geometry,
100 irrigation updates, emptied chest persistence and saved door edits.

[Exact source, binary and results](../../assets/village-refinement-2026-10-12.json)
record 630 tests on stable/Rust 1.88, strict Clippy, native/co-op input, real TCP,
SSH joining, normal-rule Lantern Run, simulation timing and renderer timing.
These timings exclude terminal emulator/WAN/model latency. The native archive
has its separate hash and qualification report in `dist`.

## Remaining limits

Existing saved chunks retain old buildings; no retrofit or regeneration is done.
Mixed old/new chunk borders can show partial settlements. Recipe worlds retain
their recipe generation. Wider town variety, desert color/detail, bespoke animations,
long cast sunlight shadows, full fluids/boats, human art/feel acceptance, full survival,
real WAN and clean Windows/macOS testing remain open. Protocol 12 and progression
v7 stay unchanged. This pass does not retune player land movement.
