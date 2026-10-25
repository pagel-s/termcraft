# Multiplayer setup

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
quality still need broader playtesting. The `0.2.0-beta.1` candidate is not yet published.

## Before you start

1. Every player needs the **same build** for their own operating system. Get the
   matching [native archives](installation.md); Rust is not needed to run them.
2. Extract all files and open a terminal in the folder containing the executable.
   Open each additional terminal in that folder too.
3. Commands below use Linux/macOS `./termcraft`. On Windows, use
   `.\termcraft.exe` in Windows Terminal. If you installed through Cargo, use
   `termcraft`. From source, build with `cargo build --release --locked` and use
   `./target/release/termcraft` in place of `./termcraft`.

Choose the setup that matches where your players are:

| Players | Address friends use | Start here |
| --- | --- | --- |
| One computer, several terminals | `127.0.0.1:25565` | [Same computer](#same-computer) |
| Different computers on one trusted network | Host's LAN address | [Same network](#same-network) |
| Different networks | Host's private VPN address | [Separate networks](#separate-networks) |

Hosting opens a separate multiplayer world. It does not import your solo
inventory. Players start with empty inventories and can mine, build and fight in
the shared world. Back up worlds you care about before inviting others.

## Same computer

1. In terminal 1, start hosting **and playing**:

   ```sh
   ./termcraft host 127.0.0.1:25565
   ```

   This address accepts players on this computer only. Keep this terminal open.

2. Open terminal 2 and join as a different saved player:

   ```sh
   ./termcraft client 127.0.0.1:25565 --player bob
   ```

3. For a third player, open terminal 3:

   ```sh
   ./termcraft client 127.0.0.1:25565 --player charlie
   ```

4. Press **T**, type a message and press **Enter**. Confirm the other terminals
   receive it. Players in the same dimension should see each other's characters.

Every terminal on one computer needs a **different `--player` profile**. Reusing
one while it is connected gives an "already connected" rejection. A profile is
a local reconnect identity, not a public display name. Names accept 1–32 letters,
digits, underscores or hyphens. Use the same profile and server address later to
restore that player; changing between `127.0.0.1` and a LAN address creates a
different local identity.

## Same network

1. On the host computer, start:

   ```sh
   ./termcraft host
   ```

   The default listens on local interfaces on TCP port 25565. Use this setup on
   a trusted home network. For an isolated dedicated server, use
   [safe hosting](hosting.md) instead.

2. In the host game, press **T** and read the **Join:** command. For example,
   it might show `192.168.0.147:25565`; use your actual displayed address.

3. On each friend's computer, enter that command with their profile. Example:

   ```sh
   ./termcraft client 192.168.0.147:25565 --player bob
   ```

   Replace `NAME` in the displayed command with a name such as `bob`. Another
   local terminal uses a different profile, such as `charlie`.

4. Send a message with **T** and confirm everyone receives it. If a connection
   fails, allow the game through the host's firewall on the private network and
   check that guest Wi-Fi is not isolating the computers.

**Do not give another computer `127.0.0.1` or `0.0.0.0` as its join address.**
`127.0.0.1` means the computer typing it; `0.0.0.0` is a server listening address.
If the host's displayed address is loopback, check the LAN address in the host's
network settings. Prefer a wired connection when diagnosing network pauses.

Brief network pauses are buffered without blocking the server's simulation or
the client's rendering. Connections that remain blocked are still disconnected
with bounded queues. If a player is repeatedly kicked while the host stays open,
check `multiplayer/default/host.log` inside the host's save directory: it records
joins, socket errors and output-queue limits. Keep the full client error too.

To use a separate multiplayer world, prefix **one** host command with a save root:

```sh
TERMCRAFT_SAVE_DIR="$HOME/.local/share/termcraft/saves/coop-fresh-1" ./termcraft host
```

Use the same command to resume that world. Do not paste a second `./termcraft host`
after it: the shell will run that second command when you quit the first session,
opening the default world.

## Separate networks

The recommended Linux host setup is the [rootless container and restricted
Tailscale guide](hosting.md). It limits the server's host-file access and gives
friends access to the game port through a private network.

1. The host follows [safe hosting](hosting.md), including the game-only network
   rules, then starts the sandbox on its actual Tailscale IPv4 address:

   ```sh
   python3 hosting/server.py start --bind 100.101.102.103
   ```

   `100.101.102.103` is an example; replace it with the host's `tailscale ip -4`
   result. The host must have the Linux archive; players can use any supported OS.

2. Each friend installs Tailscale, signs in with their own account and accepts
   the host's machine-sharing invitation. Keep Tailscale connected while playing.

3. Players join using the host's actual private address:

   ```sh
   ./termcraft client 100.101.102.103:25565 --player bob
   ```

4. Check player chat. The host also runs a client to play; the sandbox is a
   dedicated server and has no game window. The first connected player controls
   shared settings, so the host should join first.

If you already have administrator-provisioned SSH access, use
[SSH joining](distribution.md#join-through-ssh). Ordinary SSH credentials can
grant shell/file access; use a forwarding-only account for game guests. Neither
Termcraft nor the sandbox configures SSH accounts, VPN policies or routers.
Direct game TCP has no TLS or account login. Keep port 25565 off the public
internet; a container does not add those protocol protections.

## Leaving, reconnecting and stopping

- **Dying:** the server keeps running. Press **R** or **Enter** to respawn.
- **Guest quits:** other players continue; that player's records are saved.
- **`termcraft host` player quits:** the managed server saves and stops for
  everyone. Restart the same command, then reconnect with the same profiles.
- **Dedicated server player's window closes:** the server remains running. A
  remaining player becomes the settings host if the first player leaves.
- **Sandbox stop:** use `python3 hosting/server.py stop`; wait for the saved/stopped
  message before backing up, replacing the binary or shutting down the machine.

For a native dedicated server on Linux/macOS, keep one terminal running:

```sh
./termcraft server 127.0.0.1:25565
```

Join from separate terminals with the `client` commands above. Stop this native
server with **Ctrl+C** and wait for exit. The sandbox is Linux-only; Windows
players can use native `host` for managed startup/shutdown or join a Linux server.

To serve an existing named world, close its local owner first and run:

```sh
./termcraft world serve WORLD_ID 127.0.0.1:25565
```

The server uses that world's blocks and chests, with separate multiplayer player
records. A writer lease prevents simultaneous owners. New players' inventories
remain separate from the solo player's inventory.

## Troubleshooting

| Message or symptom | Check |
| --- | --- |
| Player already connected | Use a different `--player` in each local terminal. |
| Connection refused | Host is still running; address/port match; firewall allows the intended private connection. |
| Build mismatch or connection closes while joining | Update everyone from the same build and restart the host process. Replacing its file alone does not update a running server. |
| Unrecognized credential or server identity changed | This address now serves a different world. Keep the old credentials; use a new profile to join fresh. |
| World already open / `.session.lock` | Close the original owner. After a crash, verify no owner is running before following [recovery](hosting.md#after-a-crash); never remove a live world's lock. |
| Lost connection when host quit | Expected for managed `host`; use a dedicated server if friends should keep playing. |
| Sandbox cannot start or cannot save | Read `python3 hosting/server.py logs` and the [sandbox troubleshooting](hosting.md#troubleshooting) section. |

Back up the server's complete data folder **and each client's local `clients/`
credential folder**. Save paths and current storage limits are below; the
[safe hosting guide](hosting.md#back-up-and-restore) gives sandbox backup commands.

## Controls

Use A/D or arrow keys to move, W/Space to jump, mouse left hold to mine/fight,
mouse right click or F to interact/place, E for inventory, and 1–9 for the hotbar.
Inventory supports left-click swap, right-click split/place one and Shift-click
quick transfer. Enter crafts the grid output; Shift+Enter crafts as many as
possible; Delete clears the grid. Furnace/enchanting/anvil/brewing actions use
the displayed slots and mouse options. Drag distribution and batch crafting use
the same inventory rules in native and shared play. The host can change shared
difficulty and rules in **O: settings**; guests cannot change them. Select a row
with the arrow keys and press **Enter** to change its value. Personal travel and
loadout shortcuts require each player to opt in, as described below.

## Portals and shared progression

Build and ignite a normal Nether portal, then right-click or press F on the
opening/frame to travel. Only the interacting player travels. Friends can follow
through the same linked frame, remain behind, or explore another dimension.
Standing in a Nether opening for two seconds also travels; walking or running
past it does not.
The End frame uses twelve Eyes of Ender. Players in the End share one dragon;
one player viewing credits does not pause or teleport everyone else. Death
returns that player to their Overworld spawn. Travel/loadout shortcuts remain
disabled until that player enables them with `/gamemode`.

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
Cancelling startup also releases the world lock after loading finishes.
Chunk and player files are written individually inside a recoverable world
checkpoint. Interrupted writes roll back to the last completed checkpoint; a
forced kill can lose work since that boundary. Keep the hidden checkpoint data
with backups. Physical power-loss behavior still needs qualification. Save
failures are reported and retained data is retried. Corrupt records are rejected without replacement.
Runtime mobs, boats, loose items/XP, projectiles and active encounters persist
with each dimension's clocks, health and timers. Restarting resumes surviving wave
members and End boss/crystal damage. Nothing advances while offline. Normal live
spawn/despawn rules still apply. Old server records have no entities to restore.

## Player chat

Press **T** while playing co-op, type a message, then press **Enter** to send.
**Esc** closes chat and keeps the draft; **Ctrl+C** discards it. **Page Up / Page
Down** scroll through the recent history. Movement and held attacks stop when
you open chat; the server keeps running, so find a safe place before typing.

Messages reach everyone on that server, including players in other dimensions.
Your server-assigned label (such as **P1** or **P2**) appears in the chat title and beside your
messages, matching the player labels in the world. `--player` selects a local
reconnect profile, not a public display name.

Messages are limited to 280 characters, with a short burst allowance and one new
message per second thereafter. Recent messages appear briefly in the HUD. The
last 32 messages remain available in chat until the server stops; chat is not
saved with the world. Solo **T** opens a command console.

### Personal shortcuts

F5–F9 travel and loadout shortcuts start disabled. Type `/gamemode` in **T** to
toggle them for your player, or use `/gamemode on` / `/gamemode off`. The command
is handled privately and never broadcast as a chat message. It unlocks a
**Travel/loadout** setting under **O** for that session. This enables shortcuts;
it does not grant flight, invulnerability or unlimited blocks. Reconnecting
starts with shortcuts disabled again. Other players' settings are unaffected.

## Compatibility and transport

PvP starts enabled. Swords and bows can damage other players, with armor,
knockback, line-of-sight checks and a damage cooldown. The host can disable it
in **O: settings**; guests cannot change shared rules. Equipped armor and each
player's assigned colors appear consistently to other players. There is no
appearance selector yet. Arrows restored after a server restart keep PvE damage
and do not retain a previous connection's PvP ownership.

Protocol **22** requires matching builds. Terrain and nearby entities stream
around each player. Disconnected and stale actors release held actions. Live
clients send a periodic heartbeat, so holding the mouse can continue mining
without pointer movement.

Transport is plain TCP. Reconnect tokens are bearer credentials, without TLS,
account authentication or internet-grade abuse protection. Keep this slice on a
trusted local interface, or use [SSH joining](distribution.md#join-through-ssh).
The SSH wrapper does not provision accounts or hosts. Terminal input and network
conditions affect responsiveness.
