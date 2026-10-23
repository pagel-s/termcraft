# Development distribution and SSH

This checkout is an unreleased development build. The proposed beta has not been
qualified or published. The native archives run without a Rust installation.

## Native archives

A development archive contains one optimized executable, a native launcher,
license, documentation, launch instructions, a file-hash manifest and an archive SHA-256 checksum. It runs
without Cargo or a Rust installation. Build and package the current machine's
binary with Python 3.11 or newer:

```sh
cargo build --release --locked
python3 scripts/package_release.py --binary target/release/termcraft \
  --target x86_64-unknown-linux-gnu --output /tmp/termcraft-dist
```

Use the actual binary/target for your platform; on Windows the binary ends in
`.exe`. The packager checks the binary's version against Cargo.toml, labels output
as a development build and refuses to replace an existing archive. It does not
publish or upload anything. Extract the complete archive, open a terminal there,
and start `./play.sh` on Linux, `./PLAY.command` on Mac or `.\PLAY.cmd` on Windows.
The launcher starts Lantern Run by default; supplied arguments are forwarded.
Run `./termcraft` or `.\termcraft.exe` for classic survival. Use `--help` for
commands. The repeatable local archive check is:

```sh
python3 scripts/test_native_archive.py --directory /tmp/termcraft-dist \
  --report /tmp/termcraft-native.json
```

Use a directory containing exactly one archive and its checksum. Saves live in the per-user data directory; copying the
executable does not copy saves. See [Installation](installation.md).

`.github/workflows/native-artifacts.yml` runs on beta source pushes and can also
be manually dispatched. It provides a build/test/
Clippy/package matrix for Linux x86_64, Windows x86_64 and macOS ARM64/x86_64. It
uploads development workflow artifacts only. Every target also extracts its
archive, verifies all hashes and runs the executable without Rust in PATH; Unix
checks PTY launches, the launcher's default route and terminal restoration.
Launchers are exercised from paths with spaces and another working directory.
Linux additionally runs the real OpenSSH fixture. Consult the
[workflow results](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml)
for the exact commit being downloaded. Passing automation and native human input/runtime checks must
pass before distributing a beta. A Linux binary built on this workstation is
not proof of compatibility with older Linux/glibc systems.

The [latest verified implementation run](https://github.com/pagel-s/termcraft/actions/runs/39374722036)
passes on all four targets at `efb36f4`, including archive extraction, launchers
and no-Rust runtime checks. Its matching
[CI run](https://github.com/pagel-s/termcraft/actions/runs/39374722180) also passes.
These are development artifacts, not a released beta. Native interactive terminal
behavior and clean-machine acceptance remain separate checks.

## Join through SSH

On a server you already control, install the matching Termcraft build and run:

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

Complete normal SSH host verification/authentication before the game opens. The
command starts an SSH loopback forward, renders in your local terminal and closes
the tunnel when the game exits. It does not run a remote shell command. Keep the
server game port bound to loopback; SSH provides the protected transport. The SSH
server/account must already permit TCP forwarding. Termcraft creates no accounts,
SSH daemon, public host or access policy. Windows requires OpenSSH in PATH; this
platform's input and server shutdown still need qualification.

Use the same SSH destination, ports and player profile to reconnect. Changing a
host alias or connection details creates a different local identity key. Each
player needs a distinct profile when sharing one local machine. SSH failures
before game entry leave the terminal in its normal state. Authentication/startup
has a two-minute deadline; keep credentials and SSH configuration in OpenSSH's
usual management rather than game configuration.

The wrapper's arguments, tunnel lifecycle and reconnect identity are checked
locally. Real encrypted OpenSSH key/host verification, authentication/forwarding
rejection, reconnect/restart and dropped-link terminal cleanup pass against a
disposable loopback Paramiko endpoint; `scripts/test_ssh_join.py` reproduces the
check with temporary keys/configuration and no host service changes. See the
[qualification report](assets/qualification-2026-10-11.json). Actual remote
accounts, WAN links and human input feel remain release gates. Co-op itself uses plain TCP, so do
not expose the game port directly to the internet. See [Co-op](multiplayer.md)
for identity, save, bandwidth and gameplay limits.

## Beta qualification still needed

- Launch extracted native archives without a compiler on each supported OS.
- Exercise Windows Terminal, Linux terminals and macOS input, glyphs, resize,
  mouse, disconnect and shutdown behavior at 120×40 and 200×60.
- Test real SSH access, denied forwarding, authentication failure, slow links and
  reconnect/restart with multiple humans.
- Complete sustained co-op and classic survival/progression playthroughs.

Release version/tag/crate/archive alignment and publication are separate from
creating these local development artifacts.
