# termcraft

Terminal-first 2D sandbox survival in Rust: gather resources, craft equipment,
build a home, explore three dimensions and face the Ender Dragon.

> Unofficial fan project. Not affiliated with Mojang or Microsoft.
>
> **Termcraft `0.2.0-beta.1`** is a beta prerelease candidate.
> Publication is pending; crates.io `0.1.0` is the older alpha.

## Try Termcraft

Download a matching native archive from a successful
[Native artifacts run](https://github.com/pagel-s/termcraft/actions/workflows/native-artifacts.yml)
for this version. No Rust installation is needed for an archive.
The included launcher starts **Lantern Run**, a separate combat, traversal and
bridge-building route. Run the executable directly for classic survival.

To install from source, use [Rust 1.88 or newer](https://rustup.rs/), then:

```sh
cargo install --git https://github.com/pagel-s/termcraft --locked
termcraft
```

The beta adds coordinated terminal pixel art, movement and collision
refinements, a recipe book and furnace slots, connected villages, fluid and
survival fixes, and co-op with independent dimension travel and SSH joining.
Fresh setups start in Detail; saved view choices and terminal resizing preserve
your selected view. Use **-** for more world context.

Until beta publication, `cargo install termcraft --locked` installs the older
alpha. See the
[installation guide](https://pagel-s.github.io/termcraft/installation/) and
[source README](https://github.com/pagel-s/termcraft).

## Controls

- Hold **A/D** or arrows to walk; tap, release, then quickly hold again to sprint.
- **W/Up/Space** jumps or swims up; **X** toggles sneak and **Shift** holds it.
- **Left click** mines/attacks; **right click** places/interacts; **F** is the use fallback.
- **E** opens inventory/crafting; **1–9** selects the hotbar.
- **V / Shift+V** drops one item / its stack.
- **= or + / -** zooms; **F4** cycles views; **F3** shows diagnostics.
- **O** opens settings; **Q/Esc** closes UI or saves and quits from the world.
- **T** opens player chat in co-op or commands in solo; **Enter** sends and **Esc** closes.

Run `termcraft host` to host and play in one terminal. Friends on the same
network join with `termcraft client ADDRESS --player bob`; press **T** for the
address. PvP starts enabled; the host can turn it off in **O: settings**.

See the repository's [multiplayer setup guide](https://github.com/pagel-s/termcraft/blob/main/docs/multiplayer.md)
for local and remote play, and [safe hosting](https://github.com/pagel-s/termcraft/blob/main/docs/hosting.md)
for the rootless Linux server. Native archives include the optional hosting
launcher and container recipe; `cargo install` installs the game executable.

## Saves

- Linux: `$XDG_DATA_HOME/termcraft/saves`, or `~/.local/share/termcraft/saves`.
- macOS: `~/Library/Application Support/termcraft/saves`.
- Windows: `%APPDATA%\termcraft\saves`.

Set `TERMCRAFT_SAVE_DIR` to override the location. Back up the full directory,
including hidden files, before upgrading. Older builds cannot open newer saves.

The executable is the supported package interface; the Rust modules are not a
stable public library API.

[Docs](https://pagel-s.github.io/termcraft/) ·
[Source](https://github.com/pagel-s/termcraft) ·
[Issues](https://github.com/pagel-s/termcraft/issues) ·
[Media credits](https://pagel-s.github.io/termcraft/media-credits/)
