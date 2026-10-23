# termcraft

Terminal-first 2D sandbox survival in Rust: gather resources, craft equipment,
build a home, explore three dimensions and face the Ender Dragon.

> Unofficial fan project. Not affiliated with Mojang or Microsoft.
>
> The published `0.1.0` is the early alpha. The **Terminal Reborn** development
> branch prepares the next beta; its executables still report `0.1.0`.
> No beta has been published.

## Install the published alpha

Install [Rust 1.88 or newer](https://rustup.rs/), then:

```sh
cargo install termcraft --locked
termcraft
```

The alpha includes procedural Overworld, Nether and End terrain; mining,
placement, crafting, furnaces, brewing, boats, storage, farming and combat.
Expect rough edges and incomplete parity with the survival games that inspired it.

## Try Terminal Reborn

The development branch adds coordinated terminal pixel art, movement and collision
refinements, a recipe book and furnace slots, connected villages, fluid and
survival fixes, and co-op with independent dimension travel and SSH joining.
**Lantern Run** provides a separate combat, traversal and bridge-building route.
Fresh setups start in Detail; saved view choices and terminal resizing preserve
your selected view. Use **-** for more world context.

Download a native development archive or build `beta/terminal-reborn` from source;
these changes are not in crates.io `0.1.0`. See the
[installation guide](https://pagel-s.github.io/termcraft/installation/) and
[source README](https://github.com/pagel-s/termcraft/tree/beta/terminal-reborn).
Use the branch documentation while the hosted site is being updated.

## Development controls

- Hold **A/D** or arrows to walk; tap, release, then quickly hold again to sprint.
- **W/Up/Space** jumps or swims up; **X** toggles sneak and **Shift** holds it.
- **Left click** mines/attacks; **right click** places/interacts; **F** is the use fallback.
- **E** opens inventory/crafting; **1–9** selects the hotbar.
- **V / Shift+V** drops one item / its stack.
- **= or + / -** zooms; **F4** cycles views; **F3** shows diagnostics.
- **O** opens settings; **Q/Esc** closes UI or saves and quits from the world.

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
