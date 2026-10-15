# Swamp, framing, dragon, ores and block audit — 15 October 2026

[Open all 97 labelled block/state comparisons](index.html) ·
[Updated world gallery](../world-atlas-2026-10-15/index.html).

Material/wetland/frontal implementation: [`1324916`](https://github.com/pagel-s/termcraft/commit/13249167957875298fcb8d18922e63343a94086e).
Baseline: `40f3e11` (same game renderer/generation as `3181b09`).
Nether/gallery delivery follow-up: `846ea2e`. Overworld ore generation and this
block artwork were unchanged by that follow-up. The current Now/Detail block
captures were refreshed from `d48fe93`: glowstone now has a full crystalline
silhouette, Nether portals continuous violet ribbons and End portals a dark teal
stellar surface. The original Before samples and ore census are retained. [HTTP image check](../world-atlas-2026-10-15/image-loads.json).

## Findings and changes

- **Swamp:** every native swamp column used to be submerged. Tree and reed
  placement correctly refused water, leaving a nearly empty biome. Signed height
  noise now produces shallow pools and dry hummocks; broad, low crowns and reeds
  grow on the dry ground/banks. Adjacent land fills only beside genuinely wet
  columns, avoiding isolated pools beside dry swamp ground.
  [Generator and regression](https://github.com/pagel-s/termcraft/blob/1324916/src/world/mod.rs#L2681).
- **Framing:** the gallery's generic position finder chose the safe floor nearest
  Y=36, which could be a cave inside a hill. Biome portraits now find the first
  actual terrain surface and clear headroom. The live camera already favored the
  area above the player. Its jump dead zone and time-based follow are preserved;
  surface/ground visibility is checked at four terminal sizes and three heights.
  [Capture placement](https://github.com/pagel-s/termcraft/blob/1324916/examples/world_review.rs#L112),
  [camera regression](https://github.com/pagel-s/termcraft/blob/1324916/src/renderer/terminal.rs#L5679).
- **Dragon:** the selected frontal preview is now the playable default. Head,
  armored torso and haunch ellipses match that projection independently of flight
  direction. Melee picking, arrow intersection and contact damage share these
  regions. Wing/tail air is excluded. Existing arena dimensions are preserved.
  [Combat regions](https://github.com/pagel-s/termcraft/blob/1324916/src/entities/ender_dragon.rs#L27),
  [default-art regression](https://github.com/pagel-s/termcraft/blob/1324916/src/renderer/sprites.rs#L1609).
- **Blocks:** inspected all 97 block/state samples in overview and detail. Refined
  bedrock, gravel, sand, birch leaves, snow, ice, wool, saplings, sugar cane, ore
  inclusions and obsidian. Collision/interaction silhouettes are preserved.
  The audit also exposed invisible overview redstone torches: blindly sampling
  larger detail textures could miss every pixel of a thin stem. Overview now uses
  the authored 4×4 atlas, detail the 8×8 atlas; torches, ladder rails and latches
  remain visible. [Visibility regression](https://github.com/pagel-s/termcraft/blob/1324916/src/renderer/materials.rs#L1419).

Existing saved chunks are **not regenerated**. Wetland and ore generation changes
apply when new chunks are created. The dragon and block rendering changes apply
in existing saves too.

## Ore census

`examples/generation_audit.rs` generates three disposable worlds, each spanning
x=−2048…2047 and the full 128-row column. The census examines **12,288 columns /
1,572,864 cells**, including deposits crossing chunk seams. Connected veins use
four-neighbor adjacency. Full seed data, depth bands, cave exposure, minima,
median/p95/max vein sizes and host-rock counts are in
[before](ores-before.json) and [after](ores-after.json).

| Ore | Total before → after | Largest connected deposit before → after (per-world range) |
| --- | ---: | ---: |
| Coal | 22,953 → 15,747 | 160–221 → 41–42 |
| Iron | 14,591 → 9,594 | 62–89 → 18–19 |
| Redstone | 1,083 → 714 | 24–42 → 16–18 |
| Gold | 812 → 646 | 54–98 → 18–19 |
| Diamond | 255 → 246 | 23–42 → 10–14 |

The old noise fields made occasional excessive connected deposits. Rounded,
globally anchored pockets break those up; slightly adjusted rare-ore thresholds
preserve the approximate diamond supply. Coal and iron remain substantially more
common than precious ores. Iron starts at world Y=38, redstone at 68, gold at 83,
diamond at 105; **world Y increases downward**. Additional generation tests sample
three terrain seeds, verify depth/rarity and bound connected deposits across
chunk boundaries. The census establishes sampled balance, not every possible
seed or the time required for a complete survival progression.

## Visual review

Two review rounds checked actual ANSI rasterizations, the complete labelled block
catalog, native swamp/hills surfaces, cave ores and the frontal dragon at overview
and detail. The first round found the thin-fixture sampling bug; the second
confirmed its correction. A larger detail viewport is an inspection aid; normal
boss play retains the overview so the arena remains visible.

| Scene | Baseline | Current |
| --- | --- | --- |
| Swamp | [Before](biome-swamp-before.png) | [Now](biome-swamp-now.png) |
| Hills | [Before](biome-extremehills-before.png) | [Surface view](biome-extremehills-now.png) |
| Dragon | [Side view](end-dragon-before.png) | [Adopted front](end-dragon-now.png) |

Block samples are crops of the same staged game geometry, enlarged with nearest
neighbor pixels. The before fixture predates the new detailed captures; Before and
Now use the same overview, while Detail shows the current larger authored atlas.
Textures, lighting, sky and silhouettes all come from the renderer. The samples
are not externally painted assets.

## Verification and reproduction

- Stable Rust and MSRV 1.88: **662 tests passed** each. Three opt-in integration
  tests remain ignored by their existing configuration.
- `cargo fmt --all --check` and strict Clippy for all targets passed on both.
- Release binary built; disposable world creation and native Unix PTY
  render/quit/terminal restoration passed. [Runtime record](runtime-smoke.json).
- Historical material-pass (`1324916`) counting-writer profile: [full measurements](render-profile.json).
  At 200×60, p95 was about 2.13 ms for daylight, 1.16 ms for Nether and 0.86 ms
  for the dragon fixture. This excludes emulator presentation, simulation,
  networking and input latency; it is not a human play-feel claim.
- Local executable `./termcraft` updated to the reviewed release build.
- No human full boss-fight playthrough, native Windows/macOS session or interactive
  browser test was performed in this environment. Gallery HTML, linked resources,
  image dimensions, catalog coverage and JavaScript syntax were checked offline.

```sh
cargo build --release --locked --example material_review --example generation_audit
env -u NO_COLOR target/release/examples/material_review /tmp/materials-after
target/release/examples/generation_audit > /tmp/ores-after.json
python3 scripts/build_material_gallery.py /tmp/materials-before /tmp/materials-after /tmp/material-gallery
```

Baseline overview captures were produced before editing this checkpoint, at
`40f3e11`. Per-block crop bounds come from the public screen-to-world mapping of
the identical current fixture. Exact input/source/output hashes and check results
are recorded in [evidence.json](evidence.json).
