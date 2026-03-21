# Developer Guide

> Version: `0.1.0`
>
> Status: early alpha. Internal systems are still being reshaped, so public-facing promises should stay conservative.

## Architecture Overview

The project is split into four main module areas:

- `src/engine`: gameplay state, input, main loop, networking
- `src/world`: world generation, chunks, blocks, items, save persistence
- `src/entities`: player and mob implementations
- `src/renderer`: terminal rendering

Top-level entry points:

- `src/main.rs`: CLI mode selection and process startup
- `src/lib.rs`: module exports

## Runtime Shape

Single-player local mode:

1. `main` creates the terminal renderer
2. `GameLoop` processes crossterm input
3. input is normalized into `ClientCommand`
4. `GameState` applies the command and advances simulation
5. `Renderer` draws the visible world and UI

Experimental multiplayer path:

- the server runs authoritative simulation ticks
- clients send `ClientInputFrame`
- the server streams `ServerSnapshot` and chunk deltas
- the protocol is version-tagged in `src/engine/net.rs`

Current network protocol version in code: `5`

Public positioning recommendation:

- do not market the current server/client path as a flagship feature
- describe it as experimental until local-play parity and networking parity are materially closer

## Input Boundary

`ClientCommand` in `src/engine/command.rs` is the transport-neutral input boundary.

That is the right layer to change if you want to:

- add a new keybind
- add a network-safe player action
- keep local and remote input behavior aligned

## Rendering Approach

Rendering is terminal-native rather than tilemap-based.

Important traits of the renderer:

- dimension-aware sky/background treatment
- scene-aware entity contrast adjustments
- terminal-friendly glyph palettes rather than pixel sprites
- view-relative world draw with UI overlays rendered afterward

The main renderer lives in `src/renderer/terminal.rs`.

## Save And Persistence Model

- chunk saves live in `src/world/chunk.rs`
- progression saves live in `src/engine/state.rs`
- both use atomic temp-file writes
- chunk saves use a compressed `MCCF` envelope
- progression saves are versioned bincode payloads

See [World Format](world-format.md) for the current file layout.

## Contribution Workflow

Recommended local checks:

```bash
cargo fmt --all
cargo test --quiet
cargo clippy --all-targets -- -D warnings
./scripts/release_smoke.sh
```

Useful operational files:

- `README.md`: public-facing overview and quick start
- `docs/`: detailed player and developer documentation
- `scripts/release_smoke.sh`: local release smoke gate
- `RELEASE_CHECKLIST.md`: release/tag checklist in the repo root

## Documentation And Media

Docs publishing is built from:

- `mkdocs.yml`
- `docs/`
- `.github/workflows/docs.yml`

Current checked-in media used by the public docs:

- `docs/assets/termcraft.gif`: README teaser
- `docs/assets/termcraft.mp4`: docs landing-page highlight
- `docs/assets/termcraft-teaser.mp4`: source teaser clip used to generate the GIF

See [Media Credits](media-credits.md) for soundtrack and asset attribution.

## Contact

For contributor questions, release coordination, or bug reports, contact: `pagel.sebastian.1@gmail.com`
