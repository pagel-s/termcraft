# Experimental local co-op

Up to eight terminal clients can play across the Overworld, Nether and End.
Each dimension has its own authoritative terrain, entities and simulation clock.
Players can travel independently while friends remain in another dimension. Players have
independent inventories, armor, health, hunger, crafting state and movement.
Mining, building, chest transfers, workstations, mobs, projectiles, pickups and
terrain changes run on the server. The world advances once per tick, including
terrain physics around distant players. Nearby entities and complete player
inventories are replicated to clients.

This is a development slice for trusted local connections. Shared host-controlled
game-master chat, live MCP authoring and an SSH joining command are implemented.
Public hosting and platform/latency qualification remain unfinished. This slice does not qualify the release as beta.

## Try two players

Build once, then use three terminals in the repository directory:

```bash
cargo build --release --locked
```

Terminal 1 starts an independent multiplayer save:

```bash
./target/release/termcraft server
```

Terminals 2 and 3 join with different local profiles:

```bash
./target/release/termcraft client --player alice
./target/release/termcraft client --player bob
```

For another machine, use [SSH joining](distribution.md#join-through-ssh).

Use the same profile and address to reconnect as the same player. A profile is
local to the computer and server address; it is not a display name. The default
profile is `player`, so use `--player` when running multiple clients on one machine.
Profiles accept 1–32 letters, digits, underscores or hyphens. The default address
is `127.0.0.1:25565`; both commands also accept an explicit address.

To share an existing named/generated world, close its local owner first:

```bash
./target/release/termcraft world serve WORLD_ID 127.0.0.1:25565
```

A writer lease prevents simultaneous native/MCP/server owners of the named world.
The server uses that world's blocks and chests, with separate multiplayer player
records. New players start with empty inventories. Existing single-player
inventories are not imported into multiplayer records.

## Controls and checks

Use A/D or arrow keys to move, W/Space to jump, mouse left hold to mine/fight,
mouse right click or F to interact/place, E for inventory, and 1–9 for the hotbar.
Inventory supports left-click swap, right-click split/place one and Shift-click
quick transfer. Enter crafts the grid output; Shift+Enter crafts as many as
possible; Delete clears the grid. Furnace/enchanting/anvil/brewing actions use
the displayed mouse options. Drag-distribution gestures have not been ported.
Global difficulty/rule/debug travel shortcuts are ignored by the server. Press
**T** for shared game-master chat. Only the first connected player (host) may
submit requests or `/connect`; guests have a read-only view. Chat releases held
movement/attack inputs without pausing the server. See [LLM setup](agents.md#use-your-client-as-the-live-game-master).
Host departure cancels requests and revokes the bridge before host promotion.

Useful manual checks:

1. Have both players mine the same block and collect the drop. Only one copy
   should enter the combined inventories. Craft planks, place them and check
   that both clients see the same result.
2. Put items in a chest and race to Shift-click its stack from both clients.
   The total item count should stay constant.
3. Leave one player in inventory while the other moves or fights. Their health,
   inventory focus and actions should stay separate; the world should continue.
4. Quit a client with Q, then reconnect with its profile. Stop the server with
   Ctrl+C, restart it and reconnect again. Check inventories, durability, terrain
   and chest contents. Keep the same address/profile and world.
5. Resize either client between 120×40 and 200×60. Check the other player,
   inventory focus, weather and F3 readability. Test ordinary survival and death
   separately from the automated regressions.

## Portals and shared progression

Build and ignite a normal Nether portal, then right-click or press F on the
opening/frame to travel. Only the interacting player travels. Friends can follow
through the same linked frame, remain behind, or explore another dimension.
The End frame uses twelve Eyes of Ender. Players in the End share one dragon;
one player viewing credits does not pause or teleport everyone else. Death
returns that player to their Overworld spawn. Debug travel/loadout keys remain
disabled on the server.

Terrain, mobs, boats, pickups, collision and explosions are dimension-local.
Clients receive avatars and terrain from their own dimension. Inputs carry the
source dimension, so delayed clicks cannot edit a different world after travel;
key releases remain accepted. Travel clears held actions and retargets the cursor.
The game-master currently authors the Overworld; the host returns there to request
changes or undo its structures.

## Saves, limits and remaining work

The default server stores data under `$TERMCRAFT_SAVE_DIR/multiplayer/default`
(or the platform's normal save directory when the variable is unset). Named
servers store it in their world directory. `server-state.json` holds a versioned
server ID, world clocks/rules and player records. Server checkpoint version 3
also saves each identity's dimension, portal links and shared dragon completion.
Version 1 records upgrade with players in the Overworld; version 2 adds dimensions
and shared progression. Version 3 adds entities/encounters to the same atomic
record as inventories, and reads both older versions. Reconnect and restart
restore each saved dimension; native single-player inventory checkpoints stay separate. Reconnect credentials live under
`clients/` in the local save directory; protect them like save files. On Unix,
credential and server-record files are written with mode `0600`.

The server supports at most eight concurrent connections and 64 saved player
identities. It saves before issuing a reconnect credential, on disconnect, every
45 seconds and on Unix SIGINT/SIGTERM. Stop it with Ctrl+C and wait for exit.
Chunk and player files are atomic individually, not one whole-world transaction;
a forced kill or storage failure can lose recent work. Save failures are reported
and retained data is retried. Corrupt records are rejected without replacement.
Runtime mobs, boats, loose items/XP, projectiles and active encounters persist
with each dimension's clocks, health and timers. Restarting resumes surviving wave
members and End boss/crystal damage. Nothing advances while offline. Normal live
spawn/despawn rules still apply. Old server records have no entities to restore.

Protocol **15** requires matching builds. Frames are capped at 256 KiB and nearby
entity snapshots at 512 entities within 128 columns. Terrain streaming covers
seven chunk columns around each player. Snapshots are sent at 20 Hz. A two-minute
eight-player delayed-link/stalled-reader fixture now passes; WAN bandwidth and
long human sessions still need measurement. Disconnected and
stale actors release held actions. Live clients send a periodic heartbeat, so
holding the mouse can continue mining without pointer movement.

Transport is plain TCP. Reconnect tokens are bearer credentials, without TLS,
account authentication or internet-grade abuse protection. Keep this slice on a
trusted local interface. Local encrypted OpenSSH authentication/forwarding tests
pass. Actual remote accounts/WAN links, Windows graceful server shutdown, platform
input and a sustained human session remain release gates. The SSH wrapper does
not provision accounts or hosts. Joining as an LLM player is optional.

## Automated evidence

`src/engine/cooperative_tests.rs`, actor tests and client tests cover contested
mining/chests, separate inventories, shared ticks, distant terrain physics,
nearest-player combat, death/respawn, disconnect cleanup, reconnect/restart,
complete actor serialization, corrupt saves and failed writes. The Unix
`tests/multiplayer_tcp.rs` regression runs a real server process with two socket
clients through mining, crafting, building, disconnect and restart.

```bash
cargo test --locked
# Unix; requires permission to bind loopback TCP and signal the child server:
cargo test --locked --test multiplayer_tcp -- --ignored
cargo clippy --all-targets --locked -- -D warnings
```

The third process regression runs eight players, one delayed/jittered link and
seven authenticated stalled readers; it checks saved shutdown and reconnect too.
Set `TERMCRAFT_NETWORK_SOAK_SECONDS=120` (6–3600 supported) and optionally
`TERMCRAFT_NETWORK_REPORT=/tmp/network.json` to retain measurements. The default
is 12 seconds. See [performance](performance.md#wall-clock-network-and-ssh-checks-11-october)
for measured results and fixture limits.

These are correctness checks. They do not establish fun, internet robustness,
full progression parity or the performance targets in the beta plan.

Local automated evidence is tracked in `BETA_RELEASE_PLAN.md` in the source
checkout and [Performance](performance.md). The real TCP suite covers both a
mine/craft/build/reconnect/restart session and authenticated live game-master
requests through an actual MCP subprocess, including cancellation, invalid and
stale plans, guest restrictions and host-departure revocation. Rendered PTY checks
cover two clients, inventory, chat, resizing and terminal restoration. These use
offline fixtures, not live model calls. Other platforms remain unqualified.
