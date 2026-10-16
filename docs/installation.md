# Installation

This branch is an unreleased development candidate for the next beta. Its binary
still reports `0.1.0`; crates.io `0.1.0` is the older alpha and does not contain
these changes. Ordinary play needs no LLM.

## Play a native development build

Download the archive for your computer from the latest successful
[Native development artifacts run](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml)
on `beta/terminal-reborn`. Open the run, then its `termcraft-dev-TARGET` artifact.
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
Linux/Mac or `.\termcraft.exe` on Windows. Mac development builds are unsigned;
use your system's normal controls for a downloaded executable you trust. These
are development artifacts, with native human qualification still open.

Use a terminal with raw input, a monospace font and preferably true color,
keyboard release events and mouse support. Start at **120×40** or larger for more
world context. **=** selects Detail from the default Overview; **-** zooms out and
**F4** cycles views. Detail also works at 80×24. **F3** shows timing/input diagnostics.

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
- **T**: game-master chat; `/help` lists commands. **Q**: save and quit.

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

Use matching builds. Start a local server and two clients in separate terminals:

```sh
./termcraft server
./termcraft client --player alice
./termcraft client --player bob
```

For a remote machine with an existing SSH account:
`./termcraft join user@host --player alice`. The server supports independent
Overworld/Nether/End travel. See [co-op](multiplayer.md) and
[SSH and native archives](distribution.md) for named worlds, saved identities,
requirements and current qualification limits.
