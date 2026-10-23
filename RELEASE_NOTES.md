# Terminal Reborn — proposed v0.2.0-beta.1

**Draft release announcement, 23 October 2026. Not published.**
The latest public release remains `v0.1.0`; development binaries still report
`0.1.0`. Version/tag/package alignment and final qualification precede publication.

Termcraft's next release brings a new terminal pixel-art presentation, a broader
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
  Join with `termcraft join user@host --player alice` through an existing SSH account.
- **Better everyday interactions.** A recipe book and batch crafting, furnace
  input/fuel/output slots, stack handling, dropping, food feedback, working
  doors/portals, redstone-ore glow and projectile returns.
- **Longer journeys over land.** Fewer river crossings, broader dry stretches,
  gentler hills and graded banks. Nighttime surface encounters no longer get
  crowded out by cave populations or start entirely beyond normal awareness.
- **Richer places to explore.** Connected village buildings and irrigated gardens,
  refined Nether materials and landmarks, balanced ore pockets, and a front-facing
  dragon with slim pillars and a colorful fourteen-second defeat sequence.
- **Water and survival fixes.** Connected basins fill toward source height,
  supported underwater building works, and drowning, lava/fire and loose-item
  destruction follow the same world rules.
- **Lantern Run.** A separate combat, traversal and bridge/signal route gives new
  players a focused place to start.
- **Safer saves and native distribution.** Persistent entities and encounter state,
  recoverable interrupted checkpoints, emergency recovery copies, and
  Linux/Windows/macOS archives with launchers and checksums.

## Try the candidate

Get a successful native artifact for `beta/terminal-reborn` from
[Native development artifacts](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml),
or build the branch with Rust 1.88+. Archive launchers start Lantern Run; run the
executable directly for classic survival. Rust is not needed to run an archive.

Hold A/D or arrows to walk; double-tap and hold to sprint. **Detail is the default**
for fresh setups; saved view choices are respected. **-** zooms out, **=** zooms
in, and **F4** cycles views. **V / Shift+V** drops one
item / its stack. See [installation](docs/installation.md) and
[full release notes](docs/beta-release-notes.md).

## Before upgrading

Back up the whole save directory, including hidden checkpoint data. Native saves
use version **11**, server checkpoints **5**, and co-op protocol **18**; all peers
need matching builds. Older executables cannot read the new checkpoints.
Saved terrain is preserved, so new villages, ore layouts and terrain details
require fresh chunks. Furnaces currently use each player's session rather than
independent shared storage in every placed furnace.

Development builds remain subject to terminal/input, long-session co-op and full
survival playtesting. Native archives are unsigned. No public hosting service is
provided; remote SSH requires an existing account and forwarding permission.

Unofficial fan project. Not affiliated with Mojang or Microsoft.
