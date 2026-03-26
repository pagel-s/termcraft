<div align="center">
  <h1>termcraft</h1>
  <p><strong>Terminal-olny 2D sandbox survival in Rust, tuned toward a classic early-2012 block-survival loop.</strong></p>
  <p>
    <a href="https://pagel-s.github.io/termcraft/"><img alt="Docs" src="https://img.shields.io/badge/docs-project%20guide-2f6feb?style=flat-square"></a>
    <a href="."><img alt="Version" src="https://img.shields.io/badge/version-0.1.0-3a7a3a?style=flat-square"></a>
    <a href="."><img alt="Status" src="https://img.shields.io/badge/status-early%20alpha-c97a00?style=flat-square"></a>
    <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/rust-stable-93450a?style=flat-square&logo=rust"></a>
  </p>
  <p>
    <a href="https://pagel-s.github.io/termcraft/">Docs</a>
    ·
    <a href="docs/installation.md">Installation</a>
    ·
    <a href="docs/gameplay.md">Gameplay</a>
    ·
    <a href="docs/supported-content.md">Supported Content</a>
    ·
    <a href="docs/media-credits.md">Media Credits</a>
  </p>
</div>

> Unofficial fan project. Not affiliated with Mojang or Microsoft.
>
> Early alpha. The game is playable, but some systems are still rough or buggy.

![termcraft highlight](docs/assets/termcraft.gif)

> Full highlight video: [termcraft.mp4](docs/assets/termcraft.mp4)  
> YouTube: https://youtu.be/kR986Xqzj7E  
> Soundtrack in the full highlight: [Fantasy Orchestral Theme](https://opengameart.org/content/fantasy-orchestral-theme) by Joth and [Boss Battle #2 \[Symphonic Metal\]](https://opengameart.org/content/boss-battle-2-symphonic-metal) by nene via OpenGameArt, both `CC0`. Full details: [Media Credits](docs/media-credits.md)

## Overview

`termcraft` keeps the classic survival progression, dimensions, crafting, and exploration pressure of the early block-survival formula, but adapts the experience to a side-on terminal game.

Current scope:

- procedural Overworld, Nether, and End generation
- mining, placement, inventory, crafting, furnaces, brewing, boats, and chests
- health, hunger, combat, weather, fluids, gravity blocks, crops, and farming
- passive and hostile mobs, villages, dungeons, strongholds, and Nether fortresses
- repo-local save persistence and autosave

## Install And Play

Requirements:

- Rust stable toolchain
- a terminal with raw input support
- mouse support for the best local experience

Install Rust first:

- [rustup.rs](https://rustup.rs/)


Clone the repo and start the game:

```bash
git clone https://github.com/pagel-s/termcraft.git
cd termcraft
cargo run --release
```

If you want the optimized binary directly:

```bash
git clone https://github.com/pagel-s/termcraft.git
cd termcraft
cargo build --release
./target/release/termcraft
```

If you want it installed into Cargo's local bin path after cloning:

```bash
git clone https://github.com/pagel-s/termcraft.git
cd termcraft
cargo install --path .
termcraft
```

Local saves are written into `saves/` inside the repo.

### Distro Packages

On Arch Linux, you can install the [AUR package](https://aur.archlinux.org/packages/termcraft):

```bash
paru -S termcraft
```

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

Developer/test shortcuts currently available:

- `F5`: travel to Overworld
- `F6`: travel to Nether
- `F7`: travel to End
- `F8`: return to spawn
- `F9`: equip diamond combat loadout

## Notes

- The primary supported mode right now is local single-player.
- Client/server code exists, but it is still experimental and is not a featured public mode yet.
- If mouse right-click is unreliable in your terminal, use `F` as the explicit interaction fallback.

## Contact

For feedback, bugs, or release questions, contact: `pagel.sebastian.1@gmail.com`

## Development

Useful checks:

```bash
cargo test --quiet
cargo clippy --all-targets -- -D warnings
./scripts/release_smoke.sh
```

Release process:

- [Release Checklist](RELEASE_CHECKLIST.md)

## Save Data

Local saves are written under `saves/` and are intentionally repo-local, not OS-global.

- chunk files: `saves/<dimension>_chunk_<x>.bin`
- progression file: `saves/player_progression.bin`

See [World Format](docs/world-format.md) for the current save layout.
