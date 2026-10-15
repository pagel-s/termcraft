# Movement and art review

The 11 October quality pass follows feedback that both native and co-op movement
felt laggy and the figures had not visibly improved. It reopens the art/gameplay
quality gate; prior renderer benchmarks measure CPU work, not enjoyment or input
latency.

## Try the candidate

Run the local development executable. Restart the co-op server and every client
together: the updated movement rules use **network protocol 12**.

- Hold A/D or an arrow for one second, then release. In a terminal with key-release
  support, movement should stay continuous through the initial keyboard-repeat
  delay, then brake on release. F3 reports `keys:release` or `keys:repeat`.
- Jump beside a wall and under a ceiling. The player now moves vertically relative
  to the easing camera; co-op predicts jump, landing and collision locally.
- Switch focus while holding movement. Focus loss clears held controls.
- The default is a pixel overview at 100×32 and above. F4 cycles overview →
  wide glyphs → detailed pixels. Detail uses 8×8 block pixels at 140×44 and above;
  the default overview shows twice as much world horizontally. Smaller terminals
  use wide glyphs. HUD and inventory retain their normal resolution.
- Mine/place around negative world coordinates, then repeat after F4 and a resize.
  Targeting should match the visible tile.

Terminals that do not send key-up events retain the repeat-based fallback. They
cannot distinguish a short tap from a held key waiting for its first repeat.
This pass does not claim that limitation is solved. Simulation stays at 20 ticks
per second; the 60 fps render budget does not change survival/gameplay timers.

## Terminal feedback correction, 12 October

The village checkpoint passed narrow fixtures but missed real input compatibility
and moving terminal output. A capability-positive terminal sending one plain `d`
kept walking indefinitely. The game now starts conservatively, promotes held-key
support only after receiving an actual movement/jump release, and uses a two-second
watchdog even after confirmation. Native and co-op share the policy. Repeats renew
holds; focus loss clears them. The first long hold can pause before OS repeat until
key-up is observed. Repeat-only input retains its inherent tap/hold ambiguity.

Right-click dragging no longer repeatedly toggles a world door; inventory dragging
is retained. Target feedback says `R-click / F: use` for reachable fixtures and
`Blocked: move around` when terrain obstructs the interaction. A tighter test uses
chests, furnace and beds from the ladder, without jumping across furniture tops.

Frames now carry synchronized-update boundaries and avoid redundant cursor moves
and invisible foreground changes. Frame pacing waits after the terminal flush,
preventing slow output from immediately queuing another frame. Synchronization
requires emulator support; CPU timings alone do not prove smooth presentation.
The default pixel overview shows 40 blocks across at 160×48 instead of 20. F4 gives
wide glyphs, then optional detail. The sky uses restrained screen-space clouds,
a stepped sun/moon and stable stars in both color overview/detail and wide views.
The noisy cloud field, foreground-derived distant ridge and competing sky motion
are removed. Swing poses use a short shaded arm and compact held tool.

[Actual captures, reproduction and limits](design/terminal-feedback-2026-10-12/provenance.md)
include a moving-output profile. The retained native/co-op PTY test advertises
keyboard support but first sends plain taps; it also drops a release deliberately
and opens/drags/walks through doors in all three views. These checks do not establish
your emulator's latency, real WAN feel or overall human visual/gameplay acceptance.

## Torch attachment correction, 15 October

Ordinary torches now share support rules between the world and renderer. Their
shafts meet full blocks, slabs and stair treads; side mounts lean into a visible
bracket, and enclosed cutaway rooms show a back-wall mounting plate. The color
wide view preserves the flame and shaft instead of losing the stem when shrinking
the art. Monochrome retains its glyph fallback.

Unsupported placement keeps the inventory item. Removing or replacing support
drops one torch. Fresh villages and strongholds place lights at actual surfaces;
supported lights in existing rooms remain mounted. Unsupported old torches become
items after a one-time loaded-chunk check with neighboring data present. No save
format or protocol change is required. Redstone torches retain their existing rules.

[Captures, reproduction and limits](design/torch-attachments-2026-10-15/provenance.md)
show staged attachment fixtures and fresh generated village interiors. Check the
floor, both wall directions and slabs after restarting the executable, then remove
their support and confirm that each drops once. The inferred cutaway back wall is
bounded to small enclosed rooms; this is not a general saved background-wall system.

## Connected villages

Fresh ordinary Overworld chunks now contain three distinct buildings around a
covered market gate and irrigated garden. Furniture sits in reachable lofts above
clear walking corridors. Cottage and farmhouse beds, storage, a workshop furnace,
crafting station, two-block doors and framed windows use normal game interactions.
Torches light both the walking lane and loft storage. Detailed-view framing leaves
more space above the player for roofs; enclosed-room backdrops continue behind
partial lofts. Villagers recognize three homes rather than six doors or the market,
seek shelter at night, and move toward outdoor entry space by day.

[Compare the old hut and new settlement](design/village-refinement-2026-10-12/provenance.md).
The overview is a deliberately wide virtual viewport; ordinary-size captures include
80×24 glyph fallback, 120×40 and 160×48 pixel views, plus nighttime and desert masonry.
The covered market reads as a gatehouse, and desert architecture still has limited
biome-specific color and detail. This is a reviewed candidate for playtesting.

Existing saved chunks keep old villages. For immediate inspection, generate the
separate seeded review save below; its printed path starts outside a village.
Spawning and daylight are disabled for the focused route. None of your usual saves
are opened. This requires a development checkout and Rust; the local executable
can then play that prepared save without Cargo.

```sh
cargo build --release --locked --bins --examples
env -u NO_COLOR target/release/examples/village_review /tmp/village-captures
TERMCRAFT_SAVE_DIR=<printed-review-save-path> target/release/termcraft
```

Walk through all buildings in both directions, right-clicking the doors. Climb each
ladder with Space, hold Shift near the top of the ladder, and open the chest and furnace. Compare faces, windows, floors and roofs at 120×40 and 160×48, then F4 and
80×24. Enable `doDaylightCycle` in O settings to reach night, then use a bed and check
the saved respawn. Enable `doMobSpawning` to observe villagers. Inspect crop hydration and
return after saving/quitting to confirm edits and emptied chests persist. Villager
AI is covered separately by full engine tests; the focused native route disables
spawning. Ordinary worlds do not guarantee a village close to spawn.

## Lantern Run: play the complete loop

```sh
./termcraft trial
```

This creates or resumes the isolated `lantern-run` world. Use a different ID,
for example `./termcraft trial second-run`, for a fresh attempt. Your usual
survival save is separate. No model or provider is required.

Start at the camp with tools, bread, six planks and four torches. Follow the trail
right, cross the steps and broken ground, then defeat the three ruin guards.
Read their amber attack warnings: move behind a committed melee attack, jump
cover, or interrupt a skeleton's drawn bow. Open the gate, right-click the supply
chest and transfer its supplies. Build across the final eight-block gap and place
a torch on top of the stone signal. The objective row records shared progress.
A/D moves, Space jumps, left-click attacks/mines, right-click uses/places, E opens
inventory, and Q saves/exits. Quick-move chest stacks with Shift-click.

The same world can be hosted with `./termcraft world serve lantern-run` and joined
with the usual client/SSH commands. New co-op identities receive one starter kit;
reconnecting retains their saved inventory. An interrupted encounter restarts its
wave on reopen, since mobs are session-only. Loot, bridge blocks and completed
progress persist. The supply chest is ordinary world storage and can be reached
before the guards are defeated; the objective still requires all three kills.

This pass adds phase-linked weapon swings, compact jump poses, player hit flashes,
actual enemy windup markers, body-based melee selection and swept arrows that hit
the first actor before terrain. Hurt recovery preserves knockback instead of
letting AI steering erase it on the next tick. Idle target panels clear the scene;
reachable creatures show identity and health.

[Combat and traversal captures](design/lantern-run-2026-10-12/index.html) distinguish
staged renderer poses from the full CLI playthrough. Reproduce that playthrough:

```sh
python3 scripts/test_lantern_trial.py --binary ./termcraft --report /tmp/lantern.json
```

The scripted run uses ordinary inventory, attacks, mining and placement, then
saves/reopens completion. It knows the map and advances simulated time between
actions. Human enjoyment, uncoached onboarding and real network feel still need
playtesting; this is a focused candidate, not final acceptance of the whole game.

## Five-minute feel check

Start at 160×48 or larger in a color terminal, with `NO_COLOR` unset. Back up a
save before upgrading it. Walk on flat ground, release, then reverse immediately;
check for unwanted sliding or a camera snap. Compare a quick jump release with a
held jump, then jump against a wall and under a low ceiling. Repeat beside a
ledge while sneaking. Mine/place the tile under the pointer before and after F4
and resizing. Visit a hut and compare interior objects, door and collision edges.
Existing huts keep their old roofs; the new roof geometry appears in fresh chunks.

Repeat the same route in co-op after restarting the server and clients with this
build. Record the terminal/size, F3 `keys` mode, native or co-op, and the exact
action/location of any pause, correction, sliding or collision mismatch. A screen
recording of that short route is more useful for the next feel pass than an FPS
number alone. Boats, water/lava, ladders, knockback and dimension changes need a
separate longer check.

## Further physics and room refinement

Ghasts and Blazes now move their full bodies against terrain instead of checking
only the feet-center block. Gravity-free sweeps stop heads at ceilings and sides
at walls, including fast recoil, slab/stair shapes and the shared tree-platform
policy. Spawn checks reject pockets that do not fit their heads and sides.
Villagers keep knockback while home/shelter steering waits for recovery; farm
animals no longer voluntarily jump during recoil. NPC fluid motion uses NPC input
instead of inheriting the currently selected player's held jump.

Cutaway walls now require a continuous architectural roof and complete side walls.
Open ruins and shelters retain a consistent sky background; nearby trees and short
cover columns no longer create disconnected brown panels. Enclosed wooden huts
keep warm back walls, stone rooms use muted gray, and transparent props retain
the wall behind their empty pixels. The End tunnel recess stays separate.

At small terminal sizes, Lantern Run displays a complete compact objective and
one contextual controls hint for climbing, chest transfer, bridging or lighting
the signal. The game-master status keeps priority over that hint.

[Review captures and reproduction](design/room-physics-2026-10-12/index.html)
include the same seeded village before/after, focused flight impacts, and updated
trial poses at compact and large sizes. These are staged software renderer
fixtures; ordinary-rule CLI completion and terminal-input checks are separate.

## 12 October: shape, catalog and world review

[Open the complete review gallery](design/2d-refinement-2026-10-12/index.html),
including 135 item types, 97 block/state samples, three-size entity lineups and
actual seeded End geometry. [Review rounds and reproduction](design/2d-refinement-2026-10-12/provenance.md)
record the findings and limits.

Slabs and stairs use their visible half-block geometry for movement, occlusion,
contact shadows, ray checks and dropped-item landing. Walk up/down half-block
steps; jump to climb a full block. Small presentation corrections settle across
ticks and releasing between ticks no longer rewinds the displayed position.
Farm animals share the swept solver with body sizes matching their figures.

Fresh End towers have five-block columns, seven-block crowns, 11–16-block heights
and uninterrupted three-block walking tunnels. The boss view shows more of the
arena. Head/neck/torso hits follow the dragon figure; decorative wings/tail remain
outside attack geometry. New village ridge pieces have full support beneath them,
while eaves retain half-height slabs. Existing saved structures are preserved.

Crop stages, hydrated soil, spawner species and redstone states are distinguished.
Inventory, crafting, hotbar and every world drop share one item atlas. The End has
cool ambient fill; Overworld cave darkness and gameplay light rules are unchanged.
For testing, restart native/co-op sessions with this executable, use a fresh world
for generated architecture, and repeat the five-minute route above.

## Earlier input and prediction pass

`src/engine/game_loop.rs` and `src/engine/network_client.rs` now wake through event
polling rather than sleeping past arriving input. Enhanced input is held until a
release/focus/modal event. Initial co-op arrow presses now reach the handler.

`src/engine/net.rs` predicts the local actor through the movement/collision code
in `GameState`, using replicated terrain. Inputs remain pending until explicitly
acknowledged by sequence; a newer server tick alone is insufficient. Prediction
is bounded to ten ticks beyond the last snapshot. World edits, combat, inventory
transactions and survival remain authoritative. Prediction does not remove WAN
latency, and correction behavior under real networks still needs human testing.

`src/renderer/sprites.rs` introduces colored half-block figures for the player and
common creatures, walking/facing/crouch poses, clothing/armor colors, hit flashes
and local tool swings. The close view renders the world at a larger scale while
keeping UI resolution. Pixel offsets retain one-cell camera motion; picking uses
the rendered camera. The selected refinement below extends scenery and creature
coverage; encounter design and combat feedback still need further work.

## Review and evidence

- [Initial visual comparison](design/visual-directions/index.html): baseline,
  atmospheric concept and the earlier pixel prototype, with full-size inspection.
- `examples/visual_showcase.rs`: common deterministic geometry and actor fixture.
- `docs/assets/feel-2026-10-11.json`: historical protocol-9 test scope, build hash
  and input trace results, preceding the selected refinement below.
- Unit regressions cover long enhanced holds, tile picking/vertical jump display,
  delayed unacknowledged commands, predicted jumping/landing and wall collision.

## Entity, proportion and lighting audit

The follow-up audit replaces stretched/generic figures with separately authored
silhouettes and palettes for all 26 entity kinds. Standard humanoids are about
1.75 blocks tall; Endermen, animals and small creatures retain distinct scales.
Helmets preserve faces, wool/sheared and charged variants remain visible, and
damage/alert states use explicit flags. Small-view sampling preserves eye pixels.

Characters and terrain now share continuous, locally colored illumination. Ground
contact shadows fade with height and clip to exposed solid support; opaque terrain
occludes creature pixels. Quieter textures, corner/room shading and detailed
furniture help separate objects from scenery. Direct pixel composition removes
the old glyph fragments around transparent torches and windows.

[Before/after and all full-size captures](design/art-audit-2026-10-11/provenance.md)
cover nine staged entity groups at three sizes, plus the generated hut, cave,
Nether and End. `examples/entity_review.rs` and `scripts/capture_terminal_art.py`
reproduce the entity review. Exact checks, renderer cost and identities are in
`docs/assets/art-audit-2026-10-11.json`. These are software captures of actual ANSI
output; human art acceptance remains open. This pass adds bounded contact shadows,
not general projected sunlight. Bespoke creature animations, larger villages,
HUD refinement and finer furniture/stair/slab collision still need work.

## Ender Dragon follow-up

The dragon has a larger authored canvas with separate wing ribs/membranes,
curved tail, horned jaw, visible eyes and seven wing poses. Its body remains fixed
while wings flap; swooping changes the flap rate. A separate boss health/phase
banner preserves armor information, and targeting moves above the hotbar during
the fight, clearing the previous overlap with its face near the right edge.
[Before/after, wing loop and all pose sizes](design/dragon-refinement-2026-10-12/provenance.md)
come from the real renderer. That earlier pass retained AI, combat geometry and
physics; the 12 October shape pass above updates torso/head hit regions.
Banking/dive/attack poses, tail articulation and human boss-fight acceptance remain
open. Exact checks and build identity: `docs/assets/dragon-refinement-2026-10-12.json`.

## Earlier selected pixel-art refinement

Terminal pixel art is the selected direction. The new shared material atlas covers
all block variants and applies to procedural worlds, designed worlds and builds.
The larger view adds terrain edges and world-anchored texture variation; ores,
doors, glass, furniture, crops, redstone and portals have pixel patterns. A bounded
visual back wall gives enclosed rooms a cutaway interior without adding collision
or saved blocks. Fresh village huts gain stepped roofs, eaves and chimneys. Existing
saved structures are preserved. That earlier pass improved huts; the connected village redesign above supersedes
its generation. It did not add new villager schedules.

The creature atlas now also covers Nether/End mobs, spiders, silverfish, slimes,
squids, boats, projectiles and drops. Humanoids have more detailed faces/clothes,
distance-driven walking poses, crouch/airborne states, held-item colors and pixel
swing marks. This remains a small terminal pixel canvas, not a finished bespoke
animation set for every creature, attack and equipment piece.

The controlled player's sub-tick presentation resolves the next movement sample
in an isolated, revision-cached terrain replica through the same collision rules.
It interpolates toward that resolved result instead of extrapolating velocity
through walls, without imposing an extra full tick of display delay on local
controls. Other actors interpolate completed samples. Camera following is time
based, with a dead zone for ordinary jumps. Half-cell vertical pixel composition
preserves the other half of the underlying tile. These changes reduce specific
sources of jitter; they do not promise zero latency, zero correction or motion
finer than the terminal's pixel grid.

Land walking is tuned to 6.4 blocks/second in Classic mode (previously 12), with
sharper acceleration/reversal, preserved release braking and fluid escape control.
Early release of an enhanced held jump shortens its arc. Buffered discrete jumps
and the repeat-only fallback remain available. Simulation/gameplay timers stay at
20 Hz, presentation is budgeted at 60 fps. `NO_COLOR` uses the wide glyph view.

[Earlier cross-world captures and movement clip](design/pixel-refinement/provenance.md)
come from the real renderer. `examples/pixel_review.rs` finds an actual generated
village and runs a scripted physics course in disposable saves. Renderer p95 work
and validation are retained in `docs/assets/pixel-refinement-2026-10-11.json`.

The remaining quality plan is in `BETA_RELEASE_PLAN.md`. A coherent encounter /
reward / building slice and human approval of visual consistency and game feel
remain open. Actual remote SSH/WAN correction feel, clean Windows/macOS terminals,
boats, fluids and full survival progression still need qualification.
The earlier material pass retained full-tile stair/slab collision. The 12 October
shape pass above replaces it; new fluid/boat physics still need separate work.
