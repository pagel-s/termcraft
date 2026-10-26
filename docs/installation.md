# Installation

This guide covers **`0.2.0`**. Download the game from
[Releases](https://github.com/pagel-s/termcraft/releases/tag/v0.2.0).
Source builds require Rust 1.88 or newer.

## Play a native beta build

Download the `termcraft-0.2.0-TARGET` archive for your computer from
[Releases](https://github.com/pagel-s/termcraft/releases/tag/v0.2.0) and extract all its files.
Development builds are available from successful
[Native artifacts runs](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml).
Those require GitHub sign-in: extract the artifact ZIP, then its contained game archive.

| Computer | Download `0.2.0` | Start Lantern Run from the extracted folder |
| --- | --- | --- |
| Linux, Intel/AMD 64-bit | [Linux archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-x86_64-unknown-linux-gnu.tar.gz) | `./play.sh` |
| Windows, Intel/AMD 64-bit | [Windows archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-x86_64-pc-windows-msvc.zip) | `.\PLAY.cmd` in Windows Terminal |
| Mac, Apple silicon | [Apple silicon archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-aarch64-apple-darwin.tar.gz) | `./PLAY.command` in Terminal |
| Mac, Intel | [Intel Mac archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-x86_64-apple-darwin.tar.gz) | `./PLAY.command` in Terminal |

Rust and Cargo are unnecessary for these archives. Launchers start the separate
**Lantern Run** combat/building route by default, and forward any arguments:
`./play.sh --help`, for example. Start classic survival with `./termcraft` on
Linux/Mac or `.\termcraft.exe` on Windows. Mac archives are unsigned;
use your system's normal controls for a downloaded executable you trust. These
are beta builds; broader human playtesting is still needed.

Use a terminal with raw input, a monospace font and preferably true color,
keyboard release events and mouse support. Start at **120×40** or larger for more
world context. **Detail is the default** unless you have saved another view.
**-** zooms out, **=** zooms in, and **F4** cycles views. Resizing keeps your
selection; Detail also works at 80×24. **F3** shows timing/input diagnostics.

## Install with a package manager

On macOS, run `brew install pagel-s/termcraft/termcraft`. With Rust 1.88 or newer,
run `cargo install termcraft --locked`; the current release is `0.2.0`.
Debian/Ubuntu users can follow the [signed APT setup](distribution.md#apt-setup).
WinGet is pending Microsoft's review; Windows users can download the archive above.
See [package-manager channels](distribution.md#package-manager-channels).

## Build from source

Rust **1.88 or newer** is required only to build. Install it from
[rustup.rs](https://rustup.rs/), then:

```sh
git clone https://github.com/pagel-s/termcraft.git
cd termcraft
cargo build --release --locked
./target/release/termcraft trial
```

On Windows use `.\target\release\termcraft.exe trial`. To install this checkout
into your Cargo bin directory, run `cargo install --path . --locked`.

## First play

- **A/D** or arrow keys: move; **W/Up/Space**: jump or climb.
- Hold a direction to walk; tap, release, then quickly hold it again to sprint.
- **E**: inventory; **Left Click**: mine/attack; **Right Click**: place/interact.
- **F**: interact with the hovered block if right-click is unreliable.
- **V / Shift+V**: drop one selected item / its stack. **Q**: save and quit.

Lantern Run provides an encounter, supplies and a bridge/signal goal. Classic
survival is a separate save. Back up existing saves before upgrading; new
checkpoint formats cannot be opened by older builds.

## Save location

- Linux: `$XDG_DATA_HOME/termcraft/saves`, or `~/.local/share/termcraft/saves`.
- macOS: `~/Library/Application Support/termcraft/saves`.
- Windows: `%APPDATA%\termcraft\saves`.

`TERMCRAFT_SAVE_DIR` overrides the location. Existing repo-local `./saves` files
are copied into the default user data directory on first run. See
[save formats and recovery](world-format.md).

## Play with friends

Use matching builds. Host and play in the first terminal:

```sh
./termcraft host 127.0.0.1:25565
```

Join another terminal with `./termcraft client 127.0.0.1:25565 --player bob`.
Follow [multiplayer setup](multiplayer.md) for all three network arrangements and
[safe hosting](hosting.md) for the optional rootless Linux server launcher bundled
in native archives. For an administrator-provisioned SSH host, see
[SSH joining](distribution.md#join-through-ssh).
