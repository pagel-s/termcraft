# Termcraft beta: implementation and qualification

Updated 12 October 2026. Proposed release: `v0.2.0-beta.1`.

**The foundation is implemented, but the gameplay and visual quality target has
been reopened after playtest feedback. Beta qualification and publication are
not complete.** The checkout and local executable contain the work below.
Development checkpoints are tracked on `beta/terminal-reborn`, with Sebastian
Pagel as the sole commit author. No beta release has been published.

## Quality pass: responsive, recognizable, worth sharing

The user's 11 October feedback is that figures/gameplay still look much like the
old game and movement feels laggy both locally and over co-op. Renderer work-time
benchmarks do not resolve that feedback. The earlier foundation work is not an
acceptance gate for the art or game feel.

1. **Refine the selected terminal pixel art.** The initial comparison at
   `docs/design/visual-directions/index.html` shows the previous renderer, an
   atmospheric ASCII concept, and a playable terminal pixel-art prototype in the
   same fixture. The user selected terminal pixel art. A shared material/creature
   atlas, detailed view, room cutaways and fresh hut roofs are implemented locally.
   Review consistency across characters, terrain, structures and HUD in real play;
   larger villages, bespoke animations and stronger combat feedback remain open.
2. **Make control response consistent.** This pass corrects enhanced held-key
   expiry, enables focus-loss events, wakes for input, renders up to 60 fps, fixes
   initial co-op arrow presses and displays the player at the actual jump height.
   Co-op now predicts with the authoritative movement/collision rules and waits
   for sequence acknowledgements before discarding inputs. Terminals without
   release events still use repeat-based fallback. Qualify those separately;
   increasing a timeout would trade pauses for unwanted movement after a tap.
3. **Build a compelling five-minute gameplay slice.** Start with a clear goal,
   readable enemy attacks, impactful hits and useful rewards, then a building or
   traversal payoff. Show danger before damage; make movement, jump timing,
   knockback and recovery decisions legible. Connect director-created challenges
   to this loop without requiring an always-on LLM player. Lantern Run now
   implements an encounter, supplies and bridge/signal payoff; human acceptance
   remains open.
4. **Judge physics through an obstacle course.** Include reversal, jump buffering,
   ledges, low ceilings, stairs, ladders, water entry/exit and combat knockback.
   Test local and delayed co-op with the same route. Check collision alignment in
   both views and all creature sizes; compare human observations with traces.
5. **Earn the release clip.** A friend joins through SSH, a player requests a
   recognizable challenge, the world changes, and the group survives/builds
   something worth showing. Test the complete loop with people before writing a
   viral-release claim or calling the beta plan finished.

Current fixes and reproducible checks are documented in `docs/feel-and-art.md`.
The art prototype is a review candidate, not a claim that the final visual or
gameplay quality gate has been met.

The headline is **“Survive and build with friends over SSH.”** The distinguishing
feature is a player-directed AI game master: ask in the running game for a battle,
mobs, an environment or a structure, then continue playing while your chosen LLM
plans a bounded change. Existing Codex/Claude/MCP clients can serve as the game
master without a second API key. Optional LLM play is easy to connect; an active
LLM player or bundled model service is not required. Performance, readability,
physics and easy installation support this experience. Sharing/virality is an
outcome to test with people, not something the implementation can guarantee.

## Assessment baseline

Source baseline: main commit
[`1df974f`](https://github.com/pagel-s/termcraft/commit/1df974fccb9d5bc56d1c507ef6e8373e165eb81b),
also the published crates.io 0.1.0 source. The older GitHub v0.1.0 tag points to
`1a3dcc6`; packaging work after that tag means release notes must distinguish the
two baselines. Original local checks passed 507 tests, formatting and a release
build. Existing main CI passed Rust 1.88; new results below are local checks,
not newly published GitHub CI results.

The baseline already included Overworld/Nether/End survival, crafting, mining,
building, farming, workstations, boats, weather, redstone/TNT and dragon progression.
Its networking lacked full per-player inventory, entity and interaction parity:
see the original
[`ServerSnapshot`](https://github.com/pagel-s/termcraft/blob/1df974fccb9d5bc56d1c507ef6e8373e165eb81b/src/engine/net.rs#L147)
and community [issue #2](https://github.com/pagel-s/termcraft/issues/2).
The assessment found no open issues and an open
[Debian packaging PR #4](https://github.com/pagel-s/termcraft/pull/4); that is a dated
snapshot, not a live GitHub status claim. No GitHub items were modified.

The workspace's `.git` directory is empty/protected. The baseline verification
checkout is `/tmp/termcraft-latest-20261010`; working source is
`/home/sebastian/projects/termcraft`.

## Completed local implementation

| Area | Result | Main evidence |
| --- | --- | --- |
| Performance | Brightness-priority lighting with material attenuation cached once; reusable frame encoder, one output write per frame; safe exploration cache eviction around all viewers | `src/renderer/lighting.rs`, `src/renderer/terminal.rs`, `examples/render_profile.rs`, `examples/coop_profile.rs`, `docs/performance.md` |
| Visuals | Readable solid HUD, separate sprint/target rows, terrain-preserving target tint, mining progress/reach feedback, remote player ID labels, quieter cloud/End animation; F3 diagnostics | `src/renderer/terminal.rs`, rendered PTY/ANSI captures |
| Physics | Shared release braking without cancelling later knockback; continuous ladder climb/hold; residual-momentum sneak ledge protection; correct final landing-distance damage; swept wall/ceiling collisions | `src/engine/state.rs`, `src/engine/physics_tests.rs`, existing collision regressions |
| Saves/recovery | Reject unreadable chunks/seeds/progression; keep failed writes dirty; cancel unsafe dimension changes; v7 full native player checkpoint preserving crafting grid/enchantments/effects; upgrade v1–6; partial emergency native recovery copy | `src/world/chunk.rs`, `src/world/mod.rs`, `src/engine/state.rs`, `docs/world-format.md` |
| In-game director | T chat, asynchronous typed waves/weather/time/ambience/structures, whole-plan validation/preflight, stale-area checks, cancellation, preferences, conditional structure undo | `src/engine/director.rs`, `src/engine/game_loop.rs`, `src/world/environment.rs` |
| Existing LLM as director | `/connect` opt-in authenticated loopback bridge; six live MCP tools for context, bounded wait, requests, validated submission/results and cancellation; sole simulation writer, no second key | `src/engine/director_bridge.rs`, `src/engine/agent.rs`, `tests/multiplayer_tcp.rs`, `docs/agents.md` |
| Personalized events | `/auto exploration|building|combat`, healthy-player/travel/modal/encounter gates, 120-second cooldown, three attempts/session; `/pause` and reset controls | `src/engine/director.rs`, cooldown regressions and prior 128-second PTY run |
| Shared human game | Independent full player sessions, normal inventory/crafting/storage/workstations/combat, world/entities tick once, distant terrain physics, separate death/respawn, reconnect and periodic/exit saves | `src/engine/actors.rs`, `src/engine/net.rs`, `src/engine/server_save.rs`, `src/engine/cooperative_tests.rs`, `tests/multiplayer_tcp.rs` |
| Shared director | Host-only requests and `/connect`, shared messages, no pause while typing; cancel/revoke/pause on host departure before promotion | `src/engine/network_client.rs`, `src/engine/net.rs`, real TCP/MCP and two-rendered-client checks |
| Optional LLM play | 16 stepped world/player tools plus six live director tools; absolute-path JSON/TOML client config; normal game rules, bounded observations/actions, no developer loadout/teleport | `src/engine/agent.rs`, `src/engine/mcp.rs`, `docs/agents.md` |
| New worlds | Typed deterministic recipes, validation/repair, retained prompt, named isolated saves and writer leases, native entry; optional trusted provider executable | `src/world/recipe.rs`, `src/worlds.rs`, `src/generator.rs`, `scripts/generate_world.py` |
| Join/distribute | `termcraft join user@host --player NAME` manages a local SSH tunnel and stable reconnect identity; native archive packager with license/docs/manifest/checksum; manual four-platform artifact workflow | `src/engine/ssh.rs`, `scripts/package_release.py`, `.github/workflows/native-artifacts.yml`, `docs/distribution.md` |

The local executable is `/home/sebastian/projects/termcraft/termcraft`.
The package version remains `0.1.0`; generated archives are explicitly development
builds, not mislabeled beta artifacts. Network protocol is **12**, requiring
matching builds. New native saves use progression version **7**; back up older
saves before testing because older executables cannot read the new format.

## Measured results and validation

### Lantern Run and combat review, 12 October

The next candidate implements a complete opt-in route with camp, terrain steps,
three guards, supplies, an eight-block bridge and a signal objective. Normal CLI
MCP actions complete the route and preserve completion across save/reopen.
Attack windups, committed facing, skeleton aim, swept arrows, body targeting,
knockback recovery, airborne/swing/hurt poses and quieter idle HUD connect the
visual changes to actual gameplay decisions. Chest-only edits now mark chunks
dirty; agent jump requests queue actual takeoff and open-container tools use the
normal UI. Fresh co-op identities receive one trial kit, without reconnect refills.

Read-only physics/gameplay and visual review rounds caught and fixed inconsistent
impulses, attack steering, invisible compact warning/leg poses, and an ocelot jump
indexing regression. The new regression runs one ocelot with zero wolves through
an obstacle. Wolf/ocelot movement and jump probes use consistent body dimensions.
Stable and Rust 1.88 pass **611 unit tests** and strict all-target Clippy. Protocol
**12** requires restarting matching clients/servers; progression remains v7.

[Review and provenance](docs/design/lantern-run-2026-10-12/provenance.md) and
[exact qualification identity](docs/assets/lantern-run-2026-10-12.json) distinguish
staged pose captures, ordinary-rule CLI completion, terminal input and transport
checks. The route is reproducible with `./termcraft trial`; this is not acceptance
of overall human enjoyment, all-terminal/WAN smoothness or the full survival arc.
Larger village design, full fluids/boats, human co-op and clean Windows/macOS
qualification remain open. Previous reports retain their original build identities.

### 2D translation and complete item review, 12 October

The current candidate replaces the End's thin arch stems with five-block-wide
upper columns, seven-block crowns, 11–16-block heights and continuous three-block
base tunnels. A wider boss view keeps the arena readable. Fresh generation changes;
existing saved terrain is preserved. Slabs and stairs now share explicit half-block
geometry between physics, material art, occlusion and contact shadows. Walking
steps up/down half blocks; full-block climbs still require jumping. Four farm-animal
solvers now use the shared swept rules with sprite-sized heights. Sub-tick releases
and small corrections settle without abruptly resetting the camera.

All 135 item types have shared pixel icons in inventory, hotbar, crafting output
and world drops. Material items use the placed-block palette. The reproducible
catalog covers 97 block/state samples, all items and both pixel-view resolutions;
`examples/material_review.rs` and `examples/translation_review.rs` isolate saves.
Inactive redstone torches, buttons, piston directions and filled End frames now
have distinct visual states. Local stable and Rust 1.88 checks pass **599 unit tests**, strict
all-target Clippy and three real TCP regressions. Protocol **11** requires matching
client/server builds. The contested-pickup TCP fixture now separates players before
mining and waits for pickup rather than the initial spawn coordinate.

Four review rounds also correct crop growth/hydration, spawner identities, false
background columns below floating props, ridge support above half-height eaves,
dropped-item landing surfaces and exact terrain intersections for fast projectiles.
The dragon has head/neck/torso hit regions matching its figure, and the End receives
a readable ambient fill. [Complete searchable review](docs/design/2d-refinement-2026-10-12/index.html)
contains 86 full frames, all catalog samples and a physics-driven traversal clip.

This remains a review candidate. Human control/art acceptance, real WAN latency,
full fluid/boat qualification, larger village design and bespoke combat animations
remain open. Exact qualifying identities and scope are in
`docs/assets/2d-refinement-2026-10-12.json`; earlier reports retain their original
binary identities. The first checkpoint `27d7e54` passed GitHub CI.


### Ender Dragon candidate, 12 October

The dragon now uses a 64×40 authored source canvas with ribbed near/far wings,
curved tail, horned jaw, readable eyes and seven wing poses. The body stays fixed
through a twelve-step wing cycle; swooping changes its rate. A dedicated boss
health/phase banner preserves armor information. Targeting moves above the hotbar
during the fight, clearing the boss's face near the right screen edge.

`docs/design/dragon-refinement-2026-10-12/provenance.md` contains unchanged-fixture
before/after, a wing loop and 15 focused pose captures at three sizes. All **587
unit tests** and strict Clippy pass on stable Rust and Rust 1.88. Regressions cover
fixed body/eye anchors and both facing directions near the HUD, at each pixel-view
size. `examples/render_profile.rs` adds a separate animated `end_dragon` scene.
Exact build identity and costs: `docs/assets/dragon-refinement-2026-10-12.json`.

Development checkpoints are pushed to `beta/terminal-reborn`, authored only by
Sebastian Pagel. CI now runs on `beta/**` pushes. Existing boss AI/combat/physics
are unchanged; banking/dive/attack poses, tail articulation, encounter redesign
and human acceptance remain open.

### Entity, proportion and shadow candidate, 11 October

The second art audit replaces generic/stretch-scaled creatures with individually
authored silhouettes and palettes, explicit visual states, equipment/skin separation
and preserved small-view eyes. All 26 entity atlas kinds are covered. Continuous
shared lighting, restrained local color, clipped ground-contact shadows, terrain
occlusion and quieter material textures improve scene consistency. Direct pixel
composition removes the glyph fragments left in transparent objects.

`docs/design/art-audit-2026-10-11/provenance.md` retains before/after comparisons,
27 full-size entity captures and four world captures. All **585 unit tests** and
strict all-target Clippy pass on stable Rust and Rust 1.88; optimized examples,
native/co-op enhanced-input traces and offline crate verification pass. Renderer
CPU/ANSI p95 is **0.452–2.577 ms** across twelve cases, excluding emulator, kernel
I/O, simulation and network time. Richer per-pixel illumination costs more than
the previous candidate; this is not a speedup claim. Exact identities and scope:
`docs/assets/art-audit-2026-10-11.json`. Movement/protocol 10 are unchanged.

Long cast sunlight shadows, bespoke attack/flight/equipment animations, larger
villages, finer collision geometry and human art/feel acceptance remain open.
Earlier TCP/SSH reports below identify their original binaries; they were not
rerun for this renderer-only revision.

### Earlier selected pixel-art candidate, 11 October

The shared block/creature atlas, larger pixel view, cutaway back walls, fresh hut
roofs and cross-world atmosphere are implemented locally. Movement uses finer
land speed, sharper reversal, variable held jumps and a collision-resolved local
presentation preview. The camera uses time-based following with a jump dead zone.
Network protocol **10** prevents prediction with the previous movement tuning.

`docs/design/pixel-refinement/` contains renderer captures and a physics-driven
movement clip; `examples/pixel_review.rs` recreates their disposable fixtures.
Validation and exact build identity: `docs/assets/pixel-refinement-2026-10-11.json`.
All **577 unit tests** pass on stable Rust and Rust 1.88, along with formatting,
strict all-target Clippy, optimized builds, three TCP regressions, native/co-op
enhanced-input traces and the isolated OpenSSH fixture. Renderer CPU/ANSI work
p95 is 0.307–0.870 ms across twelve scene/viewport cases, excluding terminal
presentation, networking and simulation. Native development archive extraction
and runtime are checked separately against the same binary hash.
These are a substantial candidate pass, not human acceptance of final art, perfect
physics or a guarantee of jitter-free behavior on all terminals and networks.


### Earlier movement and art candidate, 11 October

- **569 unit tests** pass on stable Rust and Rust 1.88; formatting, strict
  all-target Clippy on both toolchains and the optimized build pass.
- Native and co-op enhanced-input PTY checks pass continuous hold without repeat
  events, release braking, initial arrow presses, focus loss, F3/F4, inventory and
  terminal restoration. This is automated input evidence, not human approval of
  the feel. Repeat-only terminal input retains its known initial-repeat gap.
- The three real TCP regressions and isolated SSH fixture pass with protocol 9.
  Local physics prediction now covers jumps/collision and retains unacknowledged
  commands; real WAN correction quality remains open.
- The new close-view renderer writes once per frame. Twelve scene/viewport cases
  have p95 CPU/encoding work of 0.129–0.493 ms. Its closer field of view renders
  less terrain, so these are not an equal-coverage speed comparison with the
  foundation renderer or an end-to-end input latency measurement.
- The portable three-direction image page was prepared for review. Art choice
  was still open at that point; terminal pixel art has since been selected.
  Encounter/reward design and human gameplay acceptance remain open.

Exact candidate identity, input traces and test scope:
`docs/assets/feel-2026-10-11.json`. Logs use
`/tmp/termcraft-feel-*-20261011.log`.

### Earlier foundation checks

The following measurements describe the earlier foundation candidate. They are
retained as historical evidence; the checks above cover the movement/art pass.

On Linux x86_64 / Intel Core i9-13900K:

- **563 unit tests** pass on stable Rust 1.98.1 and minimum Rust 1.88.0. Formatting,
  strict all-target Clippy on both toolchains, optimized builds and offline Cargo
  package verification pass. Three offline Python provider-contract tests and
  strict MkDocs build pass.
- **Three real TCP/process regressions pass**: contested mine/collect/craft/build/
  reconnect/restart; and a live host request through an actual MCP subprocess,
  including invalid, duplicate, cancelled and revoked results, bounded waiting,
  guest restrictions and host departure. The third exercises eight actors, a delayed
  link and seven stalled readers, then save/restart. These require loopback socket permission
  and are deliberately ignored in the ordinary unit-test command.
- Native and shared rendered PTY checks cover T chat, typing without movement
  leakage, responsive play during provider delay, modal deferral, keyboard cancel,
  shared results, read-only guests, resize at 120×40/200×60 and terminal restoration.
  Native exit-on-save-error also produces a separate recovery copy and releases
  the world lease. Fixtures supply plans; these are not live model-quality tests.
- Real stdio JSON-RPC smoke checks expose **21 tools**, repair an invalid world
  design, act/save/reopen, and enter that world through the native terminal.
- Three alternating **color-enabled** renderer comparisons retain identical
  visuals/output bytes while replacing the original lighting/encoder algorithms.
  Torch-cave p95 falls from **1.811–1.940 ms to 0.672–0.706 ms at 200×60**. All twelve
  scene/viewport cases improve. Output writes fall to one per frame. This excludes
  kernel I/O/emulator presentation; it is not a visible-FPS claim. Earlier
  `NO_COLOR` measurements are retained and labeled. Raw data and methodology:
  `docs/assets/performance-2026-10-11.json`, `docs/performance.md`.
- Eight actors over 72,000 accelerated ticks (60 **simulated** minutes) retain at
  most 22 chunks; p99 shared tick/cache/snapshot-encoding work is 0.334 ms. This is
  not a 60-minute wall-clock soak, network-latency test or survival campaign.
- Real OpenSSH-to-Paramiko encrypted transport passes with temporary keys: seven
  authenticated sessions, host-key/auth/forward rejection, missing game server,
  reconnect/restart, dropped-link terminal restoration and reaped SSH processes.
  The repeatable fixture changes no host daemon/account/config. Remote accounts
  and WAN behavior still need qualification. See `scripts/test_ssh_join.py`.
- A **120-second wall-clock** eight-player TCP soak passes with 80–120 ms ordered
  delay per direction on one link. Direct/delayed snapshot-gap p95 is 55.9/76.0 ms;
  each actor receives about 2,400 snapshots. Seven stalled readers are removed in
  11.4 seconds; the worst healthy-client gap during that phase is 164 ms. Normal
  inventory, flat terrain and disabled natural spawning are fixture limits. This
  is not a 60-minute human session or packet-loss/terminal test.
- Four plans authored interactively by this Codex task through real stdio MCP
  apply successfully: snow/dawn, cows/pigs, an accessible stone watchtower and a
  finite zombie/skeleton battle. Observed request-to-application times are
  6.0–13.6 seconds with zero repairs; these include tool orchestration and capture,
  not isolated model inference. A prior request correctly expired during geometry
  inspection. The captured development demo is `dist/director-demo-20261011.gif`
  with four PNGs and a JSON transcript. It is automated single-player capture,
  not human co-op or a client/model comparison. Raw qualification evidence:
  `docs/assets/qualification-2026-10-11.json`.
- Live testing found and fixed a surface search that could skip ladder-blocked
  ground and choose a cave floor. Its regression also checks that a rejected
  spawn leaves the rest of the plan unapplied.
- The Linux standalone archive is extracted, file/manifest checksums verified and
  launched with no Cargo/Rust in PATH. Windows/macOS workflow definitions exist;
  those artifacts have not been built or run here.

Local logs use `/tmp/termcraft-final-*-20261011.log`. Reproduce the core checks:

```sh
cargo test --locked
cargo test --locked --test multiplayer_tcp -- --ignored
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo package --allow-dirty --offline
python3 scripts/test_generate_world.py
mkdocs build --strict
cargo run --release --locked --example render_profile
cargo run --release --locked --example coop_profile -- 72000
```

The release checklist is now a concrete handoff at `RELEASE_CHECKLIST.md`, with
repeatable network/SSH commands, human/platform scripts and migration/recovery
instructions. Draft player-facing release notes are ready locally.

## Boundaries and remaining release gates

**Content/gameplay:** Director actions use existing mobs and blocks, at most four
bounded actions/request. Local biome requests change ambience and biome-based
spawning, not terrain shape. Structures require flat empty natural ground and fit
16×12; existing builds/storage/players are protected. Mob additions are finite
waves, not timed spawn-rate modifiers. No generated quests/new mechanics/bosses.
Generated world recipes do not automatically supply classic progression
structures/resources. Prompt validity does not establish accessibility or fun.

**LLM integration:** Keep the external game-master task actively waiting; connecting
does not wake an idle coding-client chat. Requests expire after 90 seconds. Client
approvals/model latency count toward this deadline. Existing-client authoring is
implemented, but real Codex/Claude/model task fidelity and latency still need user
qualification beyond the single interactive Codex run above. Optional LLM play owns a stepped single-player world, so simultaneous
human/LLM player entry is deferred. Ordinary gameplay remains model-independent.

**Shared play:** Eight connections/64 saved identities, Overworld only, trusted
local TCP or an existing SSH tunnel. Player IDs, not custom display names, label
avatars. Prediction remains approximate and server-authoritative; qualify jitter,
slow clients, bandwidth and input feel under latency. No public host/accounts or
internet-grade abuse controls have been provisioned. Cross-dimension multiplayer
is outside the first shared release slice.

**Persistence:** Writes are atomic per file, not across a whole world/plan. Mobs,
loose items and encounters are session-only. Recovery copies capture loaded chunks
of the current dimension and player state; they are partial snapshots, not full
backups. Disk failure can also prevent recovery. Stale world locks require checking
that no owner remains before manual removal. Interrupted writes, slow/full disks,
whole-world crash consistency and recovery instructions still need broader trials.

**Human/platform qualification before beta:**

1. Complete classic fresh-world survival through dragon/credits, including storage,
   workstations, death/respawn, save upgrades and dimension travel. Assess movement
   feel, knockback, swimming, boats, passive mobs, falling blocks and fluids.
2. Complete a real 60-minute co-op session with multiple humans: contested edits,
   transfers, combat, reconnect/restart, separation across chunks and director use.
3. Test actual remote SSH accounts/WAN links and reconnect input feel. Encrypted
   local SSH rejection/forwarding/reconnect and delayed TCP buffers are now tested.
4. Build/extract/run matching Linux/Windows/macOS archives on clean machines; test
   mouse, keyboard, glyphs, colors, resize, shutdown and disconnect in real terminals.
5. Record live-model request latency, repair counts, output size, prompt fidelity
   and useful results for battle/mobs/environment/structure requests. The first
   user's commands worked but model performance was disappointing; no timings/model
   were recorded, so model choice alone is not an established explanation.
6. Record a human shared-play demo, try uncoached onboarding,
   and fix any blocking crashes, save loss, progression or usability defects found.

After those gates pass, align crate/tag/archive versions, review Debian PR #4 and
AUR guidance, finalize `docs/beta-release-notes.md` and obtain separate publication/hosting
authorization. No beta tag, public SSH endpoint, announcement or release has been
created. These external qualification gates remain open; the implementation work
above must not be described as a fully qualified beta.
