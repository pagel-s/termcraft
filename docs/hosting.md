# Safe hosting on Linux

Use this guide when you want a dedicated server with limited access to your
computer. Players on Linux, Windows and macOS can join the same Linux server.
For two terminals or a trusted home-network session, start with the
[multiplayer setup guide](multiplayer.md).

The bundled `hosting/server.py` launcher uses **rootless Podman**. It publishes
one game port on an explicitly chosen private IPv4 address, mounts one dedicated
save directory, and runs without root privileges. It refuses wildcard/public
bindings and a rootful runtime. It builds the image locally from your Linux
executable; there is no separate published Termcraft container image to trust.

## What the sandbox protects

- The application filesystem is read-only; the server's persistent host mount
  is its dedicated save folder. Temporary container files are disposable.
- Your home folder, SSH keys, project files and container-management socket are
  not mounted. The image builder receives only the executable, license and
  Containerfile, rather than your whole checkout.
- The server has no Linux capabilities and cannot gain new privileges. Defaults
  limit it to 1 GiB memory, two CPUs and 128 processes/threads.
- Rootless user mapping makes save files belong to your normal host user. The
  save directory is private to that user.

These are containment measures, not a security audit. The container shares the
host kernel and its private network can make outbound connections. It does not
add game accounts, encrypted game transport, bans or protection against players
destroying a world. Use trusted players, backups and the network restrictions
below. For a public service with strangers, use a separate VM and further
security qualification.

See the upstream [Podman runtime options](https://docs.podman.io/en/latest/markdown/podman-run.1.html)
for the isolation and resource controls used here. CPU/memory enforcement also
depends on the host's cgroup configuration.

## Install the host prerequisites

1. Download and extract the current **Linux x86-64 archive** from the same build
   your players use. The archive includes `termcraft`, `LICENSE`, `hosting/` and
   this guide. Open a terminal in its extracted folder. From source, build the
   native executable first and pass `--binary ./target/release/termcraft` to
   `build` or `start`.
2. On Ubuntu, install the runtime tools:

   ```sh
   sudo apt update
   sudo apt install podman uidmap slirp4netns fuse-overlayfs dbus-user-session python3
   ```

3. As your **normal user**, check the runtime:

   ```sh
   podman info --format json
   python3 hosting/server.py --help
   ```

   `host.security.rootless` must be `true`. Do not run the launcher with `sudo`.
   If subordinate UID/GID mappings or cgroup delegation are missing, fix the
   rootless runtime first; the launcher does not fall back to privileged hosting.

   Older systemd installations may delegate only memory and process limits.
   If the launcher reports a missing CPU controller, an administrator can add a
   per-user override before hosting:

   ```sh
   sudo mkdir -p "/etc/systemd/system/user@$(id -u).service.d"
   printf '[Service]\nDelegate=cpu cpuset io memory pids\n' | sudo tee "/etc/systemd/system/user@$(id -u).service.d/termcraft.conf"
   sudo systemctl daemon-reload
   ```

   Reboot or fully restart your user session before retrying. Do this while no
   game server is running. The launcher requires cgroup v2 and CPU, memory and
   process controllers; it does not silently omit limits. See the upstream
   [resource-delegation troubleshooting](https://github.com/podman-container-tools/podman/blob/main/troubleshooting.md#26-running-containers-with-resource-limits-fails-with-a-permissions-error).

The first start downloads the pinned Debian runtime base and builds a local
image. Subsequent starts reuse the image when the executable and recipe match.
The bundled Linux archive is the supported input; an executable built against a
newer host libc might not run in the pinned runtime. This hosting path is Linux-only.

## Start a private server

1. Start with access from this computer only:

   ```sh
   python3 hosting/server.py start
   ```

   Wait for **Server ready**. This starts a detached dedicated server; closing
   the launcher terminal does not stop it. The computer must stay on and awake.

2. Join in another terminal:

   ```sh
   ./termcraft client 127.0.0.1:25565 --player alice
   ```

3. Join another terminal as `bob`, then check chat with **T**:

   ```sh
   ./termcraft client 127.0.0.1:25565 --player bob
   ```

4. Check the service and its logs:

   ```sh
   python3 hosting/server.py status
   python3 hosting/server.py logs
   ```

The default data folder is `~/.local/share/termcraft-server`; the world is under
`multiplayer/default/` inside it. This is separate from your native solo and
native multiplayer saves. Keep the first client's profile stable; it controls
shared settings in **O**. Other clients use their own local reconnect profiles.

To change the bind address, executable, port, data folder or limits, stop the
server first. For example, to serve a trusted LAN interface:

```sh
python3 hosting/server.py stop &&
python3 hosting/server.py start --bind 192.168.0.147
```

Replace that example with an IPv4 address actually assigned to your host.
Friends connect to that address on port 25565. The launcher accepts loopback,
RFC1918 LAN addresses and Tailscale's `100.64.0.0/10` range. It refuses `0.0.0.0`
and public addresses. Firewall policy still applies.

For a different dedicated data folder or resource limit:

```sh
python3 hosting/server.py start --data "$HOME/termcraft-world-two" --port 25566 --memory 2g --cpus 2
```

Use `--name world-two` on **every** management command if you also run another
container. The default name is `termcraft-server`. A nonempty directory must
already contain this launcher's `.termcraft-server-data` marker; this prevents
accidentally mounting your home folder. `init --data PATH` prepares an empty
dedicated folder before importing a stopped server's complete save tree.

## Restrict network access

For friends on separate networks, use a private VPN such as Tailscale. Run
Tailscale on the host computer and players' computers, outside the game
container. Friends use their own accounts. The game needs no SSH login.

1. Install Tailscale using its [official installation guide](https://tailscale.com/docs/install/linux)
   and sign in. On the host, obtain its private address:

   ```sh
   tailscale ip -4
   ```

2. **Restrict shared users to the game port before distributing invitations.**
   Machine sharing alone grants network reachability to the shared computer;
   it is not automatically limited to Termcraft. In the Tailscale access-control
   editor, use the following as an example for a new personal game network:

   ```json
   {
     "hosts": {"termcraft-host": "100.101.102.103"},
     "acls": [],
     "grants": [
       {"src": ["owner@example.com"], "dst": ["*"], "ip": ["*"]},
       {"src": ["autogroup:shared"], "dst": ["termcraft-host"], "ip": ["tcp:25565"]}
     ]
   }
   ```

   Replace the host IP and owner identity. Preserve needed administrative access
   when adapting an existing network. **Remove or narrow every other rule that
   gives guests broader access**, including any existing allow-all ACL. Grants
   and ACLs are additive: this narrow grant cannot override another broad allow.
   See [Tailscale grants](https://tailscale.com/docs/reference/syntax/grants).

3. In **Machines → your host → … → Share**, create individual invitations for
   friends. They install Tailscale, sign in and accept. Share this one machine;
   avoid inviting game guests as general members of your whole network.
   [Tailscale sharing documentation](https://tailscale.com/docs/features/sharing).

4. Add policy tests for each actual friend's identity. For example, add this
   `tests` field beside `grants`, replacing the email:

   ```json
   "tests": [
     {
       "src": "friend@example.com",
       "proto": "tcp",
       "accept": ["termcraft-host:25565"],
       "deny": ["termcraft-host:22", "termcraft-host:80", "termcraft-host:443", "termcraft-host:25566"]
     }
   ]
   ```

   The editor must accept these tests. They check policy permissions even when
   no service is listening on those ports. We cannot verify your personal
   network's identities or rules from the game. See [policy tests](https://tailscale.com/docs/reference/syntax/policy-file#tests).

5. Stop a loopback server if it is already running, then bind only the actual
   Tailscale address. Example:

   ```sh
   python3 hosting/server.py stop &&
   python3 hosting/server.py start --bind 100.101.102.103
   ```

6. Host and friends join with matching game builds:

   ```sh
   ./termcraft client 100.101.102.103:25565 --player alice
   ```

   Check chat and reconnect once. Keep Tailscale connected. This setup needs no
   router forwarding of game port 25565. If the host firewall requires a rule,
   permit only TCP 25565 on its Tailscale interface, rather than opening the
   port to every interface. Revoke machine sharing when a friend should lose access.

## Stop and restart

```sh
python3 hosting/server.py stop &&
python3 hosting/server.py start
```

Pass the same `--bind`, `--port`, `--data` and `--name` options you originally
chose when starting again. **Start options are not remembered**; omitting `--bind`
returns to loopback. `stop` needs only the matching `--name`.

Stopping sends SIGTERM and allows 60 seconds for a save. Wait for the clean-stop
message. Data and the stopped container's logs remain. A clean subsequent start
replaces the stopped container using your chosen options. Dying or closing a
client does not stop this dedicated server. There is no automatic boot service
or crash restart configured by this launcher.

## Back up and restore

1. Stop the server cleanly and check `status`. Keep the complete data folder,
   including hidden files and the directory marker. For the default folder:

   ```sh
   python3 hosting/server.py stop &&
   umask 077 &&
   mkdir -p "$HOME/termcraft-backups" &&
   tar -czf "$HOME/termcraft-backups/world-backup.tar.gz" -C "$HOME/.local/share/termcraft-server" .
   ```

   Choose a new archive filename each time to retain older backups. Each
   client's local `clients/` credential folder also needs a private backup;
   it is outside the container and identifies that player's saved inventory.

2. Restore your own trusted backup into a **new, empty folder**, keeping the
   original world intact:

   ```sh
   mkdir -m 700 "$HOME/termcraft-restored" &&
   tar -xzf "$HOME/termcraft-backups/world-backup.tar.gz" -C "$HOME/termcraft-restored"
   ```

3. Start with that folder, your original private bind address and original port:

   ```sh
   python3 hosting/server.py start --data "$HOME/termcraft-restored"
   ```

   Add your original `--bind`/`--port` options if they differed from loopback
   port 25565. Reconnect with the same client address/profile and check the
   world and inventory before removing any old copy.

Never copy only `server-state.json`: chunks, hidden checkpoints and player
records belong together. More recovery details are in [Saving and Recovery](world-format.md).

## Update the server

1. Stop it cleanly and make a backup.
2. Extract the replacement Linux package into a new application folder. Keep
   your data folder outside it. Update players to that same build.
3. Run the new folder's launcher with the original data, bind, port and name.
   It builds a new local image when the executable or container recipe changes.
4. Reconnect and check world/inventory. Keep the old package and backup until
   satisfied. To roll back, use the older package with a **copy of its matching
   pre-upgrade backup**; older builds may not understand newer saves.

## After a crash

The launcher retains an abnormally exited container and refuses to replace it
automatically. Read `status` and `logs`, and back up the complete data folder.
Check the exit code: 137 can indicate a forced stop or an out-of-memory kill.

Only after confirming that **no container or native server owns that data
folder**, remove a stale `multiplayer/default/.session.lock` if one remains,
then remove the stopped container with `podman rm termcraft-server`. Use the
correct name for a custom server. Do not remove hidden checkpoint/journal files.
Interrupted checkpoints are resolved by the save loader. For damaged saves,
restore a backup into a new folder as above instead of editing the original.

The lock's recorded PID belongs to the container's PID namespace. Do not treat
it as a host PID or kill a host process with the same number.

An emergency recovery path printed in the logs belongs to the container, not
your host's `/tmp`. If the container is still running, copy an available recovery
directory out with `podman cp termcraft-server:/tmp/PATH ./recovered-copy` before
stopping it. Replace `PATH` with the exact reported name. Container tmpfs files
are lost when the container stops; persistent checkpoints and your offline
backups remain the recovery path afterward.

## Troubleshooting

| Problem | Action |
| --- | --- |
| Root/rootful runtime refused | Run without `sudo`; repair rootless Podman before continuing. |
| UID mapping or cgroup/limit error | Check `podman info`; ask the host administrator to configure subordinate IDs and delegated cgroup v2 controllers. Keep the isolation and resource limits enabled. |
| Nonempty/unmarked data folder | Choose a dedicated empty folder. To import an existing stopped save, `init` the destination first, then copy the complete save tree into it. |
| Image build fails | Check access to the pinned base image registry. No project files are uploaded by the builder. |
| Container exits before ready | Read `logs`; confirm the executable is the Linux build for this host and compatible with the pinned runtime. |
| Address/port unavailable | Use an address assigned to the host, stop any other listener or choose `--port 25566`. |
| World already open | Check all owners before removing a stale lock; follow [crash recovery](#after-a-crash). |
| Guest cannot connect | Check actual private IP, port, game build, VPN sharing/policy and host firewall. |

The native CI qualification exercises the packaged launcher with rootless Podman,
three terminal players, private port binding, filesystem/capability/resource
controls, graceful shutdown, backup restore and a replacement executable. Live
Tailscale rules, WAN latency, physical power loss and resistance to hostile
container escapes remain separate checks.
