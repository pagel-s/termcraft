# Terminal Reborn: next beta release notes

**Draft for proposed `v0.2.0-beta.1`, updated 23 October 2026.**
No beta is published. The latest public release is
[`v0.1.0`](https://github.com/pagel-s/termcraft/releases/tag/v0.1.0), the older alpha.
This page describes `beta/terminal-reborn`; development binaries still report `0.1.0`.

## Survive and build with friends over SSH

Up to eight clients share terrain, mobs, pickups, chests and combat. Players have
independent inventories, armor, crafting, health, hunger and respawn. Portals move
the interacting player: friends can follow, stay behind or explore another
dimension. Dragon completion and portal links are shared. Reconnect profiles
restore each player's inventory and dimension after restarting.

Use matching builds. Start a server on a host you control, then join through an
existing SSH account with `termcraft join user@host --player alice`. The launcher
manages the loopback tunnel and restores the terminal on exit. Slow readers have
bounded queues so their connection cannot block the simulation.

[Co-op setup](multiplayer.md) · [SSH requirements](distribution.md#join-through-ssh)

## A consistent terminal pixel-art world

Blocks, items, creatures and held tools share coordinated pixel art across the
Overworld, Nether and End. Species have distinct silhouettes and scales. Ground
shadows, shared lighting, opaque terrain and room cutaways anchor figures in the
scene. Torches attach to real supports; glowstone illuminates nearby terrain.

Overview, Wide and Detail keep their artwork when the terminal window or font
changes. Press **=** from Overview for the detailed figure; **F4** cycles views,
and the selection survives quitting. Rain and snow fall downward and stop at roofs
and cave ceilings. The HUD labels the view and separates health, food, sprint,
targeting and mining progress.

[World gallery](design/world-atlas-2026-10-15/index.html) · [Art review](feel-and-art.md)

## Movement, collision and combat

Walk by holding a direction; tap, release and quickly hold it again to sprint.
Variable-height jumps, held ladder climbing, ledge sneaking, sharper reversal and
collision-aware presentation refine the 2D loop. Swept wall/ceiling checks prevent
fast actors from tunnelling through shaped terrain. Co-op uses the same movement
rules with local prediction and acknowledged inputs.

Enemies show attack preparation and commit their facing. Hits interrupt attacks
and preserve knockback. Hostile targeting respects walls; mining and interaction
rays reject blocked corners. Swings and misses, bow draw/release, eating and tool
use have distinct poses. Ghast and blaze fireballs can be returned with a melee
hit. Hurt feedback covers environmental damage, and cacti cause contact damage.

Terminal behavior varies: confirmed key-release input supports held chords and
double-tap sprint; repeat-only terminals use a bounded fallback. **F3** shows
input mode and rolling timings. Renderer measurements cover controlled scenes,
not a universal FPS or terminal-latency guarantee.

[Controls](gameplay.md) · [Performance evidence](performance.md)

## Inventory, crafting and workstations

The bag separates storage from the hotbar, identifies selected stacks and keeps
partial merge remainders selected. A recipe book supports one-click and batch
crafting, scrolling and compact-screen navigation. Manual grids support stack
placement, one-item placement, dragging and quick transfer. Crafting preserves
ingredients if the whole output will not fit.

Furnaces expose **INPUT**, **FUEL** and **OUTPUT**, with cooking progress and heat.
Input, fuel, output and the active job survive closing and saving. This currently
uses each player's furnace session; individual placed furnaces do not yet have
independent shared storage.

Press **V** to drop one selected item, **Shift+V** for its stack. Tools retain wear
and enchantments. A pickup delay allows throwing items to friends. Full-bag drops
settle on the ground until space opens; fire and lava can destroy them. Eating
and hunger icons handle fractional values consistently. Redstone ore glows when
hit, used or stepped on.

[Inventory, crafting and furnaces](gameplay.md#inventory-and-crafting)

## Water, farming and survival

Waterfalls descend before spreading; supported runoff follows nearby drops.
Contained basins fill toward source height, including connected vessels, spillways
and drains. Removing or sealing the supply drains supplied water. Water/lava
contact produces stone, cobblestone or obsidian according to the flow. This remains
a source-based model, without conserved fluid volume or physical currents.

Solid blocks replace water beside reachable supports. Aiming into deeper water
can build against a bank or floor within normal reach. Land animals avoid walking
into water/lava, paddle toward air and climb reachable banks, including one-block
raised banks. They can still drown when no air is reachable.

Farmland hydrates from nearby water; new village gardens expose irrigation at
soil level. Buckets use the aimed face instead of falling back under a field.
Breathing players and creatures have finite air, lava/fire cause damage and
afterburn, and water extinguishes flames. Air bubbles and burning effects show
hazard state. Fire resistance and creature immunities are respected.

[Fluid rules](gameplay.md#water-and-lava-flow) ·
[Water behavior captures](design/water-behavior-2026-10-19/README.md)

## Villages, the Nether and the End

Fresh villages have connected cottages, workshops, farmhouses, loft storage,
roads and irrigated gardens. Villagers use doors and seek shelter. Small, spaced
ore pockets replace broad clumps; coal and iron taper near bedrock while deep
layers offer a varied mix of rarer ores.

Nether materials, backdrop, fortresses and route landmarks are refined for the
side view. Obsidian portal frames ignite and their active openings can be walked
through or used. Linked arrivals preserve nearby buildings and storage.

The dragon faces the player, uses a leaner silhouette and navigates slim pillars
with improved crown/overhang clearance. Its fourteen-second defeat sequence has
authored death poses, colored fractures, wing fragments and energy flowing into
the return portal. The large victory title is removed. Close views briefly pan
to fit the raised dragon, then return to the normal camera.

[Village review](design/village-refinement-2026-10-12/provenance.md) ·
[Dragon finale](design/dragon-finale-2026-10-19/README.md)

## Start with Lantern Run

`termcraft trial` creates or resumes a separate world with a camp, traversal
course, three ruin guards, supply chest and a bridge/signal goal. Supplies and
defeated guards stay saved. It also works with named-world co-op hosting.
The route is an authored gameplay slice, not a replacement for classic survival.

## Safer saves and native distribution

Entities, loose items, boats, projectiles, dimension clocks, encounter state and
portal links persist alongside player sessions. Unloaded terrain suspends loose
entity physics. Interrupted checkpoints roll back together on checked startup;
save failures are reported, and native fatal errors attempt a separate emergency
recovery copy. Backups must include the whole world directory and hidden files.

Linux x86_64, Windows x86_64 and macOS Apple-silicon/Intel archives include a
launcher, documentation, license, manifest and checksums. Rust is unnecessary at
runtime. Launchers start Lantern Run unless arguments are supplied; the executable
without arguments starts classic survival.

[Installation](installation.md) · [Recovery guide](world-format.md#recovering-into-a-copy)

## Compatibility and qualification

- Native progression **v11** reads versions 1–10; server checkpoint **v5** reads
  versions 1–4. Co-op protocol **18** requires matching clients and server.
- Older executables cannot read newer checkpoints. Back up before upgrading.
- Existing terrain is preserved. Fresh village layouts, ore balance and terrain
  details need newly generated chunks. Runtime artwork applies after restarting
  the updated executable. Recognized untouched older End towers upgrade in place;
  player-modified footprints remain intact.
- The preceding full local suite passed **822** tests on stable Rust and Rust
  1.88, strict Clippy and seven real TCP regressions. The title-removal follow-up
  passed six targeted finale checks and solo/co-op terminal qualification.
- [`efb36f4` CI](https://github.com/pagel-s/termcraft/actions/runs/39374722180)
  and the [four-platform native pipeline](https://github.com/pagel-s/termcraft/actions/runs/39374722036)
  passed. These identify the tested implementation, not a published beta.
- Manual terminal input/presentation, complete survival progression, sustained
  human co-op, remote SSH/WAN behavior and clean-machine acceptance remain open.
  Development archives are unsigned; no accounts or public hosting are provided.

The version bump, tag, signed-off release checklist and publication remain separate
steps. This note does not announce a released beta.
