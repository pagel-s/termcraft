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

Current network protocol version in code: `11`. `src/engine/actors.rs` separates
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
in a separate per-world sidecar. Shared director requests are host-controlled in the Overworld; guests observe
results while normal play continues. The network protocol carries shared chat and
director state.
`src/engine/metrics.rs` provides bounded timing windows for F3 diagnostics.

Public positioning recommendation:

- do not market the current server/client path as a flagship feature
- describe it as experimental until local-play parity and networking parity are materially closer

## Combat and authored trial

`src/engine/combat.rs` centralizes body targeting, terrain line of sight, swept
projectile ordering, damage dispatch and melee windup timing. Entity AI preserves
hurt impulses and pauses steering during committed attacks. `src/engine/flight.rs`
sweeps full flying bodies against true terrain boxes without gravity. Ground NPC
movement explicitly owns its jump input; player input stays in the player path. Protocol 12 carries
the new warning states and shared trial progress; matching builds are required.

`src/engine/trial.rs` creates the opt-in Lantern Run recipe, tracks credited guard
kills and detects the bridge/signal finish. `lantern-trial.json` is a separate
versioned atomic sidecar; progression remains v7. Session-only guards restart an
unfinished encounter after reopening. Supplies are ordinary preseeded chest
contents, never refilled on resume. Trial creation does not alter existing worlds.

`examples/lantern_review.rs` stages renderer poses on disposable trial terrain.
`scripts/test_lantern_trial.py --binary ./termcraft` instead completes the route
through real stdio tools without teleport, loadout or direct state edits, and
checks save/reopen. Its accelerated, map-aware actions are not a human playtest.

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
- `scripts/test_storage_recovery.py`: Linux checkpoint ENOSPC/partial-write,
  sync-error, slow-write and interrupted-rename checks through real CLI/MCP;
  optional native PTY recovery into a backup copy
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

On Linux, checkpoint fault checks require a C compiler and use only disposable
worlds. The test compiles `scripts/storage_faults.c` into a temporary interposer;
it is opt-in for its child processes and matches only their checkpoint path.
It fills no disk and changes no mount or running game. The native recovery check
also needs Pillow and PTY/loopback permission:

```sh
python3 scripts/test_storage_recovery.py --binary target/release/termcraft
python3 scripts/test_storage_recovery.py --binary target/release/termcraft --native-recovery --report /tmp/storage-qualification.json
```

These tests verify checkpoint publication/retry and the documented backup-copy
overlay workflow. They do not establish physical-disk power-loss durability or
atomicity across chunks, metadata and checkpoints.

## Generated village review

`examples/village_review.rs` uses fixed seed files and ordinary Overworld generation
in a disposable save, captures common sizes/night/desert, then prints a native review
save path. It stages player/villager poses for screenshots; it does not change the
terrain fixture. `examples/pixel_review.rs` uses the same pure site diagnostics
rather than assuming chest/door heights. `World::village_sites_near` describes fresh
generation only, not the contents of already saved chunks or recipe worlds.

```sh
cargo build --release --locked --bins --examples
env -u NO_COLOR target/release/examples/village_review /tmp/village-ansi
python3 scripts/capture_terminal_art.py /tmp/village-ansi /tmp/village-png
python3 scripts/test_village_walk.py --binary target/release/termcraft --fixture /tmp/village-ansi/village.json --report /tmp/village-native.json
env -u NO_COLOR target/release/examples/village_profile
env -u NO_COLOR target/release/examples/render_profile
```

The native test copies that disposable review save, presses a held movement key,
opens all six building doors through mouse events, and observes live position over
a read-only director connection. It never edits player/world state. Pillow is needed
for software captures. The village profile stages six villagers and measures 1,000
real engine ticks after warmup. Renderer profiles include a generated village scene;
these CPU timings exclude terminal emulator and WAN latency.

## Terminal compatibility and moving output

For ordinary torch attachment review, build the examples, then run:

```sh
env -u NO_COLOR target/release/examples/torch_review /tmp/torch-ansi
python3 scripts/capture_terminal_art.py /tmp/torch-ansi /tmp/torch-png
```

The disposable fixture covers floor, lower slab, both side walls and an inferred
back wall at all three color views, day and night. World/renderer regressions
also cover stairs, support removal/replacement, chunk seams, deferred legacy
checks and generated village/stronghold lights. The attachment code is runtime
only; `BlockType` serialization and network protocol 12 are unchanged.

```sh
cargo build --release --locked --bins --examples
python3 scripts/test_terminal_input.py --binary target/release/termcraft --village-example target/release/examples/village_review --report /tmp/terminal-input.json --keep-artifacts
env -u NO_COLOR target/release/examples/sky_review /tmp/sky-ansi
python3 scripts/capture_terminal_art.py /tmp/sky-ansi /tmp/sky-png
env -u NO_COLOR target/release/examples/motion_profile > /tmp/motion-profile.json
```

The terminal test deliberately answers the enhancement query while sending plain
ASCII taps with no key-up. It then checks unrelated releases, bounded legacy jump
takeoff, confirmed run+jump chords, Space repeats without direction repeats,
continued movement after Space-up, a quiet leftward hold beyond two seconds,
actual direction release, focus loss and restoration in native/co-op modes.
A generated copied village checks door click+drag+walk in every view. The sky
example stages fixed poses/time in disposable geometry. The motion profile compares
stationary and synthetic 6.4-block/s camera pans with color enabled. Counting-writer
bytes and CPU time exclude emulator presentation; synchronized output is only a
request to supporting terminals, not a latency measurement.

`examples/weather_review.rs` stages rain/snow outside and around a covered room,
plus 48 incremental ANSI frames at 20 Hz for each weather type. Run it with an
output directory, then use `scripts/capture_terminal_art.py` for the static frames.
Renderer regressions compare wet/clear output with identical lighting intensities,
test downward particle displacement and require zero precipitation below roofs
above the viewport at scales 1, 2 and 4. The candidate's confirmed control mode
trusts real release events; it no longer expires a physically held key merely
because OS repeat moved to another key. See `feel-and-art.md` for the lost-key-up
and repeat-only limitations.
