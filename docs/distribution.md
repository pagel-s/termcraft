# Native archives and SSH

Terminal Reborn **`0.2.0-beta.1`** is prepared for prerelease publication. Its
native CI archives run without Cargo or a Rust installation. See
[installation](installation.md) for downloads and terminal requirements.

## Native archives

Extract all files and open a terminal in the extracted folder:

| Platform | Start Lantern Run | Classic survival |
| --- | --- | --- |
| Linux | `./play.sh` | `./termcraft` |
| macOS | `./PLAY.command` | `./termcraft` |
| Windows | `.\PLAY.cmd` | `.\termcraft.exe` |

The launcher starts the separate Lantern Run route when called without arguments.
Arguments are forwarded, so `./play.sh --help` shows available commands. Classic
survival uses the executable directly. The archive includes player documentation,
a license, release notes, launch instructions, a file-hash manifest and an archive
SHA-256 checksum. It excludes development reports, test scripts and review media.

Archives are named `termcraft-0.2.0-beta.1-TARGET.tar.gz` on Linux/macOS or
`termcraft-0.2.0-beta.1-TARGET.zip` on Windows. `MANIFEST.json` records the version,
target, prerelease status and each bundled file's SHA-256 hash. The executable's
`--version` output matches the archive version.

Copying the executable does not copy your saves. See [saving and recovery](world-format.md).
macOS builds are unsigned. Check the exact build when downloading from
[Native artifacts](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml),
and use matching builds for co-op.

## Join through SSH

On a server you control, install Termcraft and run:

```sh
./termcraft server 127.0.0.1:25565
# Or serve a named save, after closing its local owner:
./termcraft world serve WORLD_ID 127.0.0.1:25565
```

On each player's computer, install the same build and an OpenSSH client, then run:

```sh
./termcraft join user@host --player alice
# Nondefault ports:
./termcraft join user@host --player alice --ssh-port 2222 --game-port 25566
```

Complete normal SSH host verification and authentication before the game opens.
The command starts an SSH loopback forward, renders in your local terminal and
closes the tunnel when the game exits. Keep the server game port bound to
loopback; SSH provides the protected transport. The SSH server and account must
already permit TCP forwarding. Termcraft does not create accounts or hosts.
Windows requires OpenSSH in PATH.

An ordinary SSH account may also allow shell, command and file access. The
client's `-N` option does not restrict what its credentials can do elsewhere.
Do not give game guests your personal login or private key. Have the server
administrator provide a dedicated forwarding-only account: deny shell/subsystem
sessions (`MaxSessions 0`), allow only local TCP forwarding, restrict `PermitOpen`
to `127.0.0.1:25565`, and disable remote/Unix-socket forwarding, agent forwarding
and X11. Validate the effective SSH configuration and test that shell/SFTP access
is refused before sharing access. These settings are server-administrator
requirements; the game does not apply them. See the [OpenSSH manual](https://man.openbsd.org/sshd_config).
For a setup without SSH accounts, use [safe hosting](hosting.md).

Use the same SSH destination, ports and player profile to reconnect. Changing a
host alias or connection details creates a different local identity key. Each
player needs a distinct profile when sharing a computer. Authentication/startup
has a two-minute deadline; manage credentials through OpenSSH's configuration.

For direct local connections, use `termcraft client --player alice`. Direct co-op
uses plain TCP: use a trusted local interface, or the SSH wrapper for remote
access. See [co-op](multiplayer.md) for saves and gameplay details.
