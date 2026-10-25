# Native archives and SSH

Termcraft **`0.2.0-beta.1`** is prepared for prerelease publication. Its
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
target, source commit, prerelease status and each bundled file's SHA-256 hash. The executable's
`--version` output matches the archive version.

Copying the executable does not copy your saves. See [saving and recovery](world-format.md).
macOS builds are unsigned. Check the exact build when downloading from
[Native artifacts](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml),
and use matching builds for co-op.

Several development builds can share the same beta version number. Download every
player's package from the same native CI run; `MANIFEST.json` identifies its
`source_commit`. Restart the host/server after replacing its executable: an active
process keeps running the old code. Keep the same world directory, server address,
and player profile to resume existing player records.

## Package-manager channels

The beta is not published yet. These channels are prepared for the approved release;
use the native CI archives until a version is available. They are our own channels,
not inclusion in Homebrew core or Debian's official repositories.

| Platform | Channel | Command after publication |
| --- | --- | --- |
| macOS, Apple Silicon or Intel | Homebrew tap | `brew install pagel-s/termcraft/termcraft` |
| Windows x64 | WinGet community repository, pending Microsoft review | `winget install --id PagelS.Termcraft --exact` |
| Ubuntu 22.04+ / Debian 12+, x64 | Signed Termcraft APT feed | Configure the source below, then `sudo apt install termcraft` |
| Rust users, any supported native target | crates.io, Rust 1.88+ | `cargo install termcraft --version 0.2.0-beta.1 --locked` |

Homebrew and APT automatically follow supported published releases. The APT
`beta` suite includes prereleases. WinGet becomes available after the community
submission is accepted. Native archives remain available for manual installation.

### APT setup after publication

Download the repository's public key and check its fingerprint against
[`signing-fingerprint.txt`](https://github.com/pagel-s/termcraft-apt/blob/main/signing-fingerprint.txt)
before installing the key:

```sh
curl -fsSLo /tmp/termcraft-archive-keyring.gpg https://pagel-s.github.io/termcraft-apt/apt/termcraft-archive-keyring.gpg
gpg --show-keys --with-fingerprint /tmp/termcraft-archive-keyring.gpg
sudo install -m 0644 /tmp/termcraft-archive-keyring.gpg /usr/share/keyrings/termcraft-archive-keyring.gpg
sudo tee /etc/apt/sources.list.d/termcraft.sources >/dev/null <<'EOF'
Types: deb
URIs: https://pagel-s.github.io/termcraft-apt/apt
Suites: beta
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/termcraft-archive-keyring.gpg
EOF
sudo apt update
sudo apt install termcraft
```

The feed retains three supported releases; metadata is signed and refreshed weekly.
The native Linux binary requires glibc 2.35 or later. APT installs the executable
under `/usr/bin`, guides under `/usr/share/doc/termcraft`, and hosting resources
under `/usr/share/termcraft/hosting`. Save locations stay the same.

### Preparing a release

The approved workflows must be on GitHub's default branch before their manual
release controls and publication event handlers become available. Merge the beta
branch only after its changes are approved; setting up the channels does not merge it.

1. Finish playtesting and choose the version in `Cargo.toml` and release notes.
   Create its matching tag only when the source is approved.
2. Run **Prepare verified draft release** with that existing tag. It builds all
   four native targets, exercises the terminal/server, checks native archives,
   installs the candidate Homebrew formula on both Macs, and checks the signed APT
   repository and Windows portable package. Every package must identify the same
   source commit. The workflow creates a draft and refuses to overwrite a published version.
3. Review the draft's assets, checksums and notes, then publish it. Publishing
   triggers Cargo and the WinGet submission workflow. Homebrew and APT check hourly;
   their **Update** workflows can also be run manually for an immediate refresh.

The tap and APT repositories use their own Actions tokens and pinned shared tooling.
APT's private signing key belongs only in its `APT_SIGNING_PRIVATE_KEY` Actions
secret; the public key and fingerprint are committed in the APT repository.
Keep an independent secure private-key backup for rotation and recovery.

### One-time Cargo account setup

Sign in to [Termcraft's crates.io settings](https://crates.io/crates/termcraft/settings)
as `pagel-s`, the existing crate owner. Under Trusted Publishing, add a GitHub
publisher with these exact values:

| Field | Value |
| --- | --- |
| Repository owner | `pagel-s` |
| Repository name | `termcraft` |
| Workflow filename | `publish.yml` |
| Environment | `release` |

Use the filename alone, without `.github/workflows/`. Save the publisher and check
that it appears in the crate settings. No long-lived Cargo token is needed: the
workflow exchanges its GitHub identity for short-lived publishing credentials.
The GitHub `release` environment already permits only `v*` tags. Manual runs of
**Publish Cargo release** are package dry runs and never publish.
See [crates.io trusted publishing](https://crates.io/docs/trusted-publishing).

### One-time WinGet account setup

1. Open [GitHub's classic-token creation page](https://github.com/settings/tokens/new).
   Complete any sign-in or two-factor authentication yourself.
2. Name the token `Termcraft WinGet publishing`, choose a 90-day expiration, and
   select only `public_repo`. Leave the broader `repo`, `workflow` and
   `delete_repo` scopes unchecked. WinGetCreate currently requires a classic token;
   fine-grained tokens are not supported. The scope allows changes to public
   repositories that your account can access, so use a dedicated token.
3. Generate the token and copy it directly into a new
   [Termcraft Actions repository secret](https://github.com/pagel-s/termcraft/settings/secrets/actions/new)
   named `WINGET_SUBMISSION_TOKEN`. Do not paste it into chat, commit it, or pass
   it as a command-line argument.
4. Save the secret and check that its name appears under Actions repository
   secrets. Replace the stored token before it expires; the secret does not renew it.

The workflow passes the secret to WinGetCreate through its environment, following
[Microsoft's token guidance](https://github.com/microsoft/winget-create/blob/main/doc/token.md).
Without the secret, it preserves checked manifests for manual submission. A failed
submission or missing credential does not prevent downloading native release archives.

### Final publication checklist

1. Finish the two account setups above. Keep the approved version and release notes
   together with the source changes.
2. Open a pull request into `main`, review it, and merge
   once its required checks pass. This activates the new release workflows.
3. Tag the approved merged source as `v0.2.0-beta.1` and push that tag. Do not move
   an existing release tag or reuse an already published version.
4. In Termcraft's Actions tab, run **Prepare verified draft release** on `main`
   with tag `v0.2.0-beta.1`. Wait for all native and package-manager checks to pass.
5. Open the resulting draft under Releases. Check the release notes, prerelease
   marker, four native archives, Debian package, checksums and distribution metadata.
6. Publish the reviewed draft when the release is approved. Cargo publication and
   WinGet submission start automatically. Homebrew and APT pick up the release
   within their hourly checks, or their **Update** workflows can be run immediately.
7. Check all publishing runs and test the installation commands in the channel
   table. WinGet installation becomes available only after Microsoft accepts the
   submission; preparing or submitting manifests does not make the package live.

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
