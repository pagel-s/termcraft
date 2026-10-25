# Terminal Reborn — v0.2.0-beta.1

**Beta prerelease prepared 23 October 2026. Publication pending.**
The source, executable and native archives use `0.2.0-beta.1`. Until publication,
the latest public release remains `v0.1.0`; use the beta branch or CI artifacts below.

Terminal Reborn brings a new terminal pixel-art presentation, a broader
2D survival experience and shared adventures across the Overworld, Nether and End.
Build a home, explore with friends over SSH and take on the dragon together.

## Highlights

- **A consistent pixel-art world.** Reworked characters, creatures, blocks and item
  icons; supported lights, room cutaways, shadows and clipped rain/snow. Overview,
  Wide and Detail retain their artwork across terminal sizes.
- **Refined movement and physics.** Walking/sprinting, variable-height jumps,
  ladders, ledges, swept collisions and swimming; animals avoid shore hazards,
  paddle toward air and climb reachable banks.
- **Build and survive together.** Up to eight players with independent inventories,
  crafting, health and dimension travel; shared terrain, combat and reconnect saves.
  Press **T** for player chat across all dimensions on your server.
  `termcraft host` starts a server and joins automatically in one terminal.
  PvP combat starts enabled, with a host setting to disable it. Personal
  travel/loadout shortcuts start disabled and can be toggled privately.
  Join with `termcraft join user@host --player alice` through an existing SSH account.
- **Better everyday interactions.** A recipe book and batch crafting, furnace
  input/fuel/output slots, stack handling, dropping, food feedback, working
  doors/portals, redstone-ore glow and projectile returns.
  Nether portals allow passing traffic and require a two-second stay for automatic
  travel; direct blasts respect obsidian's resistance. Multiplayer join failures
  explain profile conflicts, build mismatches and lost host connections.
- **Longer journeys over land.** Fewer river crossings, broader dry stretches,
  varied rolling terrain, gentler hills and graded banks. Nighttime surface encounters no longer get
  crowded out by cave populations or start entirely beyond normal awareness.
  Ground vegetation and a cave-dwelling first co-op player no longer suppress
  nighttime surface spawning.
- **Richer places to explore.** Connected village buildings and irrigated gardens,
  refined Nether materials and landmarks, balanced ore pockets, and a front-facing
  dragon with slim pillars and a colorful fourteen-second defeat sequence.
- **Water and survival fixes.** Connected basins fill toward source height,
  supported underwater building works, and drowning, lava/fire and loose-item
  destruction follow the same world rules.
  Underwater walking stays slower than dry land while retaining responsive steering;
  land animals cannot add an airborne step to jump over a two-block barrier.
- **Co-op playtest refinements.** Dead players disappear from other players' views
  until respawning. Villagers share a doorway hold timer and wait for players to
  clear the opening. Feet-level attacks keep their visibility checks above the
  floor, and distant surface enemies no longer suppress local night refills.
  Full underground populations can give up inactive, unseen slots for night
  encounters, while preserving nearby enemies and the shared population limit.
  Shallow hollows keep their sky background; deep narrow shafts retain an
  underground backdrop below their open-air mouths while admitting skylight.
- **Lantern Run.** A separate combat, traversal and bridge/signal route gives new
  players a focused place to start.
- **Safer saves and native distribution.** Persistent entities and encounter state,
  recoverable interrupted checkpoints, emergency recovery copies, and
  Linux/Windows/macOS archives with launchers and checksums.
- **Guided private hosting.** Step-by-step local, LAN and remote setup, plus a
  bundled rootless Linux server launcher with private address binding, a dedicated
  save mount, reduced privileges and resource limits. Guides cover game-only VPN
  access, reconnect profiles, graceful shutdown, backup restore and updates.

## Try the candidate

Get a successful native artifact for `beta/terminal-reborn` from
[Native artifacts](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml),
or build the branch with Rust 1.88+. Archive launchers start Lantern Run; run the
executable directly for classic survival. Rust is not needed to run an archive.

Hold A/D or arrows to walk; double-tap and hold to sprint. **Detail is the default**
for fresh setups; saved view choices are respected. **-** zooms out, **=** zooms
in, and **F4** cycles views. **V / Shift+V** drops one
item / its stack. See [installation](docs/installation.md) and
[gameplay guide](docs/gameplay.md).

## Before upgrading

Back up the whole save directory, including hidden checkpoint data. Native saves
use version **11**, server checkpoints **5**, and co-op protocol **21**; all peers
need matching builds. Older executables cannot read the new checkpoints.
Saved terrain is preserved, so new villages, ore layouts and terrain details
require fresh chunks. Furnaces currently use each player's session rather than
independent shared storage in every placed furnace.

This beta remains subject to terminal/input, long-session co-op and full
survival playtesting. Native archives are unsigned. No public hosting service is
provided; remote SSH requires an existing account and forwarding permission.

Unofficial fan project. Not affiliated with Mojang or Microsoft.
