# Co-op in the development build

Up to eight terminal clients can play across the Overworld, Nether and End.
Each dimension has its own authoritative terrain, entities and simulation clock.
Players can travel independently while friends remain in another dimension. Players have
independent inventories, armor, health, hunger, crafting state and movement.
Mining, building, chest transfers, workstations, mobs, projectiles, pickups and
terrain changes run on the server. The world advances once per tick, including
terrain physics around distant players. Nearby entities and complete player
inventories are replicated to clients.

Use trusted local connections or the SSH join command for a host you control.
Native archives build and pass automated checks on Linux, Windows and both Mac
architectures. Public hosting, sustained human sessions and remote-link input
quality remain unfinished qualification work; no beta is published.

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

A writer lease prevents simultaneous local/server owners of the named world.
The server uses that world's blocks and chests, with separate multiplayer player
records. New players start with empty inventories. Existing single-player
inventories are not imported into multiplayer records.

## Controls

Use A/D or arrow keys to move, W/Space to jump, mouse left hold to mine/fight,
mouse right click or F to interact/place, E for inventory, and 1–9 for the hotbar.
Inventory supports left-click swap, right-click split/place one and Shift-click
quick transfer. Enter crafts the grid output; Shift+Enter crafts as many as
possible; Delete clears the grid. Furnace/enchanting/anvil/brewing actions use
the displayed slots and mouse options. Drag distribution and batch crafting use
the same inventory rules in native and shared play. Global difficulty/rule/debug
travel shortcuts are ignored by the server.

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

## Saves and limits

The default server stores data under `$TERMCRAFT_SAVE_DIR/multiplayer/default`
(or the platform's normal save directory when the variable is unset). Named
servers store it in their world directory. `server-state.json` holds a versioned
server ID, world clocks/rules and player records. Server checkpoint version 5
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
Chunk and player files are written individually inside a recoverable world
checkpoint. Interrupted writes roll back to the last completed checkpoint; a
forced kill can lose work since that boundary. Keep the hidden checkpoint data
with backups. Physical power-loss behavior still needs qualification. Save
failures are reported and retained data is retried. Corrupt records are rejected without replacement.
Runtime mobs, boats, loose items/XP, projectiles and active encounters persist
with each dimension's clocks, health and timers. Restarting resumes surviving wave
members and End boss/crystal damage. Nothing advances while offline. Normal live
spawn/despawn rules still apply. Old server records have no entities to restore.

Protocol **18** requires matching builds. Terrain and nearby entities stream
around each player. Disconnected and stale actors release held actions. Live
clients send a periodic heartbeat, so holding the mouse can continue mining
without pointer movement.

Transport is plain TCP. Reconnect tokens are bearer credentials, without TLS,
account authentication or internet-grade abuse protection. Keep this slice on a
trusted local interface, or use [SSH joining](distribution.md#join-through-ssh).
The SSH wrapper does not provision accounts or hosts. Terminal input and network
conditions affect responsiveness.
