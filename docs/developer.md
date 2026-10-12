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

Current network protocol version in code: `10`. `src/engine/actors.rs` separates
full player sessions from the shared world. Each actor uses normal gameplay
commands and physics; mobs and terrain advance once per server tick. Snapshots
include full inventories, current chest contents, nearby runtime entities and
biomes. Clients use an in-memory terrain replica instead of generating or loading
local chunks. `src/engine/server_save.rs` persists multiplayer identities and
player state; `src/engine/client_identity.rs` stores local reconnect profiles.
See [Experimental Local Co-op](multiplayer.md) for setup, save semantics and limits.

Experimental agent mode uses `src/engine/agent.rs` for game tools and
`src/engine/mcp.rs` for stdio MCP. Declarative world recipes live in
`src/world/recipe.rs`; `src/worlds.rs` owns named saves and writer leases.
`src/generator.rs` runs bounded designer processes and feeds back validation
errors. See [Agents and Generated Worlds](agents.md) for setup and limits.

The native game master lives in `src/engine/director.rs`. Its worker runs the
trusted provider executable off-thread. Only a typed, bounded action plan crosses
back into the game loop; placement is preflighted against the current world before
application. Pending work carries its world/dimension/origin, supports cancellation,
and waits while chat or another modal UI is open. `ChatView` is presentation/input
state; it does not call providers from rendering or simulation ticks.

`src/world/environment.rs` persists biome ambience overrides without rewriting
blocks. Director preferences, recent context and conditional structure undo live
in a separate per-world sidecar. Native chat is currently Overworld/local only;
the experimental network protocol does not carry these authoring requests.
`src/engine/metrics.rs` provides bounded timing windows for F3 diagnostics.

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
- shared world-anchored material and creature pixel atlases in the close view,
  with a wide glyph fallback for small or `NO_COLOR` terminals
- view-relative world draw with UI overlays rendered afterward

`src/renderer/materials.rs` defines the exhaustive block atlas and sky/effects;
`src/renderer/sprites.rs` composes proportioned creature silhouettes, ground-contact
shadows and terrain occlusion. `src/renderer/shading.rs` provides continuous shared
presentation lighting with warm/ember/arcane local tints; gameplay lighting rules
remain separate. The pixel view paints its background/material masks directly,
so transparent pixels never inherit the old glyph-upscale layer.
`src/renderer/dragon.rs` compile-time rasterizes the dragon's seven wing poses;
the body/eye anchor remains fixed and the runtime allocates no animation frames.
`examples/dragon_review.rs` stages three viewport sizes and both facing directions.
`src/renderer/motion.rs`
interpolates resolved actor positions and previews local motion through isolated
collision prediction. UI presentation must never mutate the authoritative game.

The main renderer lives in `src/renderer/terminal.rs`.

## Save And Persistence Model

- chunk saves live in `src/world/chunk.rs`
- progression saves live in `src/engine/state.rs`
- both use atomic temp-file writes
- production builds resolve saves through `src/save_path.rs`
- chunk saves use a compressed `MCCF` envelope
- progression saves are versioned bincode payloads

See [World Format](world-format.md) for the current file layout.

Use `GameState::try_new` or `open_in_directory` for checked world ownership.
Chunk workers return missing/loaded/error outcomes; decode failures must never
fall through to terrain generation. Native autosave uses the checked incremental
save path. Infallible compatibility helpers remain for existing library callers.

Player and mob movement now sweep collision probes along the movement path so
fast motion cannot skip a one-block wall or ceiling. Existing step, fluid,
ladder and ground-snap rules still apply. See [Performance and Foundation
Checks](performance.md) for the cache policy, profiling example and limitations.

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
- `scripts/release_smoke.sh`: strict Unix smoke gate, including disposable TCP/MCP
  regressions; socket permission failures are reported as failures, not passes
- `RELEASE_CHECKLIST.md`: repeatable qualification and publication handoff
- `scripts/test_ssh_join.py`: optional real OpenSSH/Paramiko loopback regression;
  temporary keys/config/saves, no host daemon or account changes
- `scripts/test_native_archive.py`: standalone extraction/hash/runtime/PTY check
  shared by the four-platform artifact workflow
- `docs/beta-release-notes.md`: unpublished player-facing development draft
- `.github/workflows/ci.yml`: formatting, checks, tests, and package dry-run
- `.github/workflows/publish.yml`: tag-triggered crates.io publish workflow

## Documentation And Media

Docs publishing is built from:

- `mkdocs.yml`
- `docs/`
- `.github/workflows/docs.yml`

The GitHub README keeps the rich local media preview. The crates.io package uses `README-crates.md`, which avoids packaged media and points to hosted docs/video instead.

Current checked-in media used by the public docs and excluded from the crates.io package:

- `docs/assets/termcraft.gif`: README teaser
- `docs/assets/termcraft.mp4`: docs landing-page highlight
- `docs/assets/termcraft-teaser.mp4`: source teaser clip used to generate the GIF

See [Media Credits](media-credits.md) for soundtrack and asset attribution.

## Contact

For contributor questions, release coordination, or bug reports, contact: `pagel.sebastian.1@gmail.com`

Unit tests use per-process random temporary roots and per-test subdirectories;
they ignore the user's `TERMCRAFT_SAVE_DIR`. The random process component avoids
PID reuse across sandbox invocations. Process/TCP integration tests explicitly
pass their own disposable save directories to production binaries. Never run a
smoke server against real player saves.

The ignored TCP suite includes an eight-player delayed-link/stalled-reader test.
Use `TERMCRAFT_NETWORK_SOAK_SECONDS=120` and `TERMCRAFT_NETWORK_REPORT=/tmp/network.json`
for a longer measured run; default duration is 12 seconds. A human hour and clean
native-platform qualification remain separate. See the release checklist before
changing version labels or publishing artifacts.
