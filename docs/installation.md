# Installation

This branch builds the **`0.2.0-beta.1`** prerelease candidate. Publication is
pending; crates.io `0.1.0` is the older alpha and does not contain these changes.
Use `beta/terminal-reborn` or a matching native artifact to try the updated game.

## Play a native beta build

Download the archive for your computer from the latest successful
[Native artifacts run](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml)
on `beta/terminal-reborn`. Open the run, then its `termcraft-0.2.0-beta.1-TARGET` artifact.
GitHub may require sign-in to download artifacts. Extract the downloaded artifact,
then extract the contained game archive with all its files.

| Computer | Target | Start Lantern Run from the extracted folder |
| --- | --- | --- |
| Linux, Intel/AMD 64-bit | `x86_64-unknown-linux-gnu` | `./play.sh` |
| Windows, Intel/AMD 64-bit | `x86_64-pc-windows-msvc` | `.\PLAY.cmd` in Windows Terminal |
| Mac, Apple silicon | `aarch64-apple-darwin` | `./PLAY.command` in Terminal |
| Mac, Intel | `x86_64-apple-darwin` | `./PLAY.command` in Terminal |

Rust and Cargo are unnecessary for these archives. Launchers start the separate
**Lantern Run** combat/building route by default, and forward any arguments:
`./play.sh --help`, for example. Start classic survival with `./termcraft` on
Linux/Mac or `.\termcraft.exe` on Windows. Mac archives are unsigned;
use your system's normal controls for a downloaded executable you trust. These
are beta candidates; broader human playtesting is still needed.

Use a terminal with raw input, a monospace font and preferably true color,
keyboard release events and mouse support. Start at **120×40** or larger for more
world context. **Detail is the default** unless you have saved another view.
**-** zooms out, **=** zooms in, and **F4** cycles views. Resizing keeps your
selection; Detail also works at 80×24. **F3** shows timing/input diagnostics.

## Build from source

Rust **1.88 or newer** is required only to build. Install it from
[rustup.rs](https://rustup.rs/), then:

```sh
git clone --branch beta/terminal-reborn https://github.com/pagel-s/termcraft.git
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
