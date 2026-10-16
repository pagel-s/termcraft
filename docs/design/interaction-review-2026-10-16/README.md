# Interaction and consistency corrections — 16 October 2026

The feedback exposed separate simulation and presentation faults. This pass keeps the
chosen pixel artwork and corrects the actual interaction paths.

- **Actions:** misses now start a swing. Held tools use their own pixel silhouettes,
  with an articulated near arm. Bow drawing shows the curved bow, pulled string,
  aimed/nocked arrow and release. Eating/drinking and successful block uses have
  distinct poses; consuming the last item retains it during the animation. These
  transient poses replicate to cooperative clients (network protocol **14**).
- **Self-use:** food and potions do not depend on reaching the pointed terrain.
  Full hunger does not waste food; raw/cooked mutton joins the supported foods.
- **Zoom and facing:** F4's middle view previously selected the legacy glyph art.
  All three colour views now sample the same pixel atlas. Small terminals retain
  the reduced pixel view; `NO_COLOR` retains its intentional glyph fallback.
  Downsampling is mirrored after sampling, so turning no longer changes the body
  silhouette. F5/F6/F7 use the regular world and renderer paths, not alternate art.
- **Cave spawning:** new Overworld hostiles require no artificial light at their
  body cells. Daytime surface spawning is rejected, while dark caves can spawn
  during the day. Floors are selected near the current cave level. Natural spawns
  stay at least 24 blocks from players. Dungeon and silverfish spawners activate
  within 20 blocks, respect light, wait 4.5–7.5 seconds between attempts, keep four
  nearby mobs and avoid the closest four blocks. Existing mobs can walk into light.
- **Portals:** complete, active Nether portal uprights sit behind the walking plane
  in the 2D projection. Floor/cap blocks and inactive/broken frames remain solid.
  Four ticks of membrane contact trigger travel, from either side, in solo and
  cooperative play. Arrival clears movement/momentum and prevents automatic
  bounce-back. Explicit use remains available after the two-second cooldown.
- **Water/farming:** incoming flow no longer downgrades an existing source. Source
  regeneration requires solid ground or source water beneath. Buckets can pour
  at the player's feet and pick the source back up. Clicking farmland with seeds
  plants above that soil, regardless of approach direction. Unsupported mature
  wheat drops wheat and seed. Crop growth requires daylight access or artificial
  light level 9; hydrated/dry growth averages about 140/560 seconds to maturity.
  Soil dries more slowly. The existing four-block, shallow-root irrigation rule
  remains compatible with village beds and their subsurface channels.
- **Ores:** smaller bounded 2D pockets preserve depth progression. Across the same
  three seed sets, largest coal veins fell from 41–42 to 28 cells, iron 18–19 to
  16, gold 18–19 to 11–12, diamond 10–14 to 8–10. Coal/iron totals stay within
  roughly +2–10% of the preceding census; rare ore totals are lower. See the
  before/after census files. These are fresh chunks; existing saved deposits and
  structures are not regenerated.

Verification: 696 Rust tests; four real localhost TCP/MCP integration tests;
Linux native/cooperative PTY movement, jump, focus and all-view door checks.
`native-input.json` records the movement/door checks before the final held-gear
lighting correction. `native-actions.json` checks the final game build in both
solo and co-op PTYs: all three zooms draw the bow, shots consume exactly one arrow,
and aiming at distant terrain still allows eating. Terminal restoration passes. Native save v8 and
server checkpoint v2 remain compatible; transient presentation state is not saved.
New source pushes run the four-platform native CI separately.

`examples/interaction_review.rs` uses actual commands in disposable saves to capture
combat misses, mining, bow draw/release, eating, bucket use, planting and quick
travel at all three zooms. The refreshed world gallery includes these action scenes
and motion sequences. Captures are software rasterizations of the real ANSI output;
they do not measure terminal latency or establish human art/gameplay acceptance.
