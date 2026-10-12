# termcraft

Terminal based 2D sandbox survival in Rust, tuned toward a classic early-2012 block-survival loop.

> Unofficial fan project. Not affiliated with Mojang or Microsoft.
>
> Early alpha. The game is playable, but some systems are still rough or buggy.

Demo video: https://youtu.be/kR986Xqzj7E

Full project docs: https://pagel-s.github.io/termcraft/

## Overview

`termcraft` keeps the classic survival progression, dimensions, crafting, and exploration pressure of the early block-survival formula, but adapts the experience to a side-on terminal game.

Current scope:

- procedural Overworld, Nether, and End generation
- mining, placement, inventory, crafting, furnaces, brewing, boats, and chests
- health, hunger, combat, weather, fluids, gravity blocks, crops, and farming
- passive and hostile mobs, villages, dungeons, strongholds, and Nether fortresses
- per-user save persistence and autosave

## Install And Play

Requirements:

- Rust 1.88 or newer
- a terminal with raw input support
- mouse support for the best local experience

Install from crates.io:

```bash
cargo install termcraft
termcraft
```

Local saves are written to the per-user data directory by default:

- Linux: `$XDG_DATA_HOME/termcraft/saves`, or `~/.local/share/termcraft/saves` when `XDG_DATA_HOME` is unset
- macOS: `~/Library/Application Support/termcraft/saves`
- Windows: `%APPDATA%\termcraft\saves`

Set `TERMCRAFT_SAVE_DIR` to override this location.

## Controls Snapshot

- `A` / `D` or arrow keys: move
- `W` / `Up` / `Space`: jump or swim up
- `X`: toggle sneak
- `E`: inventory
- `1`-`9`: hotbar selection
- `Left Click`: mine / attack
- `Right Click`: place / interact
- `F`: explicit hovered-block use fallback if right click is unreliable in the current terminal
- `O`: settings menu
- `Q` / `Esc`: close UI or quit from world view

## Notes

- The development checkout includes experimental in-game AI game master chat, an MCP interface and generated worlds. See `docs/agents.md` in the source package for configuration and limits. Shared host-controlled authoring works with your existing LLM through `/connect`; active LLM players are optional. These additions are not in the published 0.1.0 release.

- The primary supported mode right now is local single-player.
- The development checkout includes experimental Overworld co-op, reconnect saves and an SSH join command. Native archive packaging and platform qualification are described in `docs/distribution.md`.
- The crates.io package is intended as an installable game binary. The Rust module layout is not a stable public library API yet.

## Links

- Docs: https://pagel-s.github.io/termcraft/
- Source: https://github.com/pagel-s/termcraft
- Media credits: https://pagel-s.github.io/termcraft/media-credits/
