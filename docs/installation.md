# Installation

> Version: `0.1.0`
>
> Status: early alpha. The recommended way to play right now is local single-player.

## Requirements

You need:

- Rust stable toolchain
- a terminal with raw input support
- mouse support for the best local experience

Install Rust via:

- [rustup.rs](https://rustup.rs/)

Confirm your toolchain:

```bash
rustc --version
cargo --version
```

## Install From Crates.io

```bash
cargo install termcraft
termcraft
```

Cargo installs the binary into your local Cargo bin directory, typically `~/.cargo/bin`.

## Clone The Repository

```bash
git clone https://github.com/pagel-s/termcraft.git
cd termcraft
```

## Run From Source

From the repo root:

```bash
cargo run --release
```

This is the simplest way to test local source changes.

## Build A Reusable Binary

If you want the optimized binary directly:

```bash
cargo build --release
./target/release/termcraft
```

## Install From A Local Checkout

If you want to run `termcraft` directly from your shell after cloning locally:

```bash
cargo install --path .
termcraft
```

Cargo installs that binary into your local Cargo bin directory, typically `~/.cargo/bin`.

## First Launch

- start the game
- press any key at the splash screen
- use `A` / `D` or arrow keys to move
- use `W`, `Up`, or `Space` to jump
- use `E` to open inventory
- use `Left Click` to mine or attack
- use `Right Click` to place or interact

If right-click is unreliable in your terminal, use `F` as the explicit hovered-block interaction fallback.

## Save Location

Saves are written to the per-user data directory:

- Linux: `$XDG_DATA_HOME/termcraft/saves`, or `~/.local/share/termcraft/saves` when `XDG_DATA_HOME` is unset
- macOS: `~/Library/Application Support/termcraft/saves`
- Windows: `%APPDATA%\termcraft\saves`

Set `TERMCRAFT_SAVE_DIR` to override this location. Existing repo-local `./saves` files are copied into the default user data directory on first run.

## Experimental Networking

The repo contains a server/client path, but it is still experimental and should not be treated as a polished public feature yet.

Examples:

```bash
cargo run --release -- server 0.0.0.0:25565
cargo run --release -- client 127.0.0.1:25565
```

Use local single-player as the primary supported mode.
