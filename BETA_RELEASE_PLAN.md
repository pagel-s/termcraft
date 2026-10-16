# Termcraft beta: implementation and qualification

Updated 16 October 2026. Proposed release: `v0.2.0-beta.1`.

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
   atlas, detailed view, room cutaways and multi-building settlements are implemented locally.
   Review consistency across characters, terrain, structures and HUD in real play;
   village variety, bespoke animations and human combat/feel acceptance remain open.
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

The 15 October torch correction shares attachment rules between placement,
support removal and rendering. Floor/slab/stair lights meet their actual surface;
side and enclosed-room lights have visible mounts in every color view. Fresh
village/stronghold generation uses supported positions. Loaded unsupported legacy
torches become dropped items after neighboring chunk data is available. See
`docs/design/torch-attachments-2026-10-15/provenance.md` and the matching evidence
report. This addresses the floating-torch feedback; overall human art/feel
acceptance remains open.

The run/jump and weather follow-up on the same date corrects direction expiry
when Space takes keyboard repeat, upward precipitation, cave/roof leakage and
missing wide-color precipitation. Confirmed key-up streams now retain held
controls until release/reset; conservative repeat-only input gets a bounded
takeoff grace and keeps its fundamental multi-key limitation. Snow has distinct
motion/art and a correct local label. Character art is unchanged. See
`docs/design/run-jump-weather-2026-10-15/provenance.md` for evidence and tradeoffs.

The headline is **“Survive and build with friends over SSH.”** The distinguishing
feature is a player-directed AI game master: ask in the running game for a battle,
mobs, an environment or a structure, then continue playing while your chosen LLM
plans a bounded change. Existing Codex/Claude/MCP clients can serve as the game
master without a second API key. Optional LLM play is easy to connect; an active
LLM player or bundled model service is not required. Performance, readability,
physics and easy installation support this experience. Sharing/virality is an
outcome to test with people, not something the implementation can guarantee.

## Co-op dimension follow-up, 15 October

The shared engine now owns separate terrain/entity/weather/clock state for each
dimension. Players travel independently through normal portals; only actors in
the same dimension participate in collision, damage, pickups and targeting.
Shared dragon completion and portal links persist alongside per-identity dimensions
in server checkpoint v2 (v1 remains readable). Native saves remain v8, and the
server no longer overwrites native player checkpoints when authoring shared terrain.
Network protocol 13 stamps actions with their source dimension and resets terrain
revision tracking on transitions. Reconnect can begin directly in the Nether/End.

Regressions cover friends remaining behind, shared arrivals, per-dimension ticks,
explosions, stale actions, reconnect/restart, shared End progression, independent
credits and native-checkpoint preservation. A real two-socket test ignites and uses
a portal, verifies dimension-specific terrain streaming, reconnects in the Nether,
returns one player, and restarts the server with the players in different dimensions.
The test uses staged frames/starting tools; it is not a complete survival campaign.
End return now finds supported clear space when a generated recipe lacks the usual
stronghold landing. Human feel/art, long sessions and clean-platform checks remain open.

The follow-up removes repeated session/entity snapshot captures, adds game-master
`/status` (client contact, request phase and deadline), and supplies no-Rust
launchers that default to Lantern Run. Archive checks exercise launcher argument
forwarding from a path with spaces and another working directory. The first real
four-platform workflow passed Linux but exposed inherited nonblocking sockets on
Mac and a Windows-only strict-lint failure; both have source fixes awaiting fresh
platform results. Accepted game/director sockets now explicitly use blocking
reads with their existing deadlines. See `docs/performance.md`,
`docs/installation.md` and the native workflow results for the exact qualification.

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
| Saves/recovery | Reject unreadable chunks/seeds/progression; keep failed writes dirty; cancel unsafe dimension changes; v9 native / v3 server checkpoints preserving full player state, entities, encounters and portal links; upgrade native v1–8 / server v1–2; partial emergency native recovery copy | `src/world/chunk.rs`, `src/world/mod.rs`, `src/engine/state.rs`, `docs/world-format.md` |
| In-game director | T chat, asynchronous typed waves/weather/time/ambience/structures, whole-plan validation/preflight, stale-area checks, cancellation, preferences, conditional structure undo | `src/engine/director.rs`, `src/engine/game_loop.rs`, `src/world/environment.rs` |
| Existing LLM as director | `/connect` opt-in authenticated loopback bridge; six live MCP tools for context, bounded wait, requests, validated submission/results and cancellation; sole simulation writer, no second key | `src/engine/director_bridge.rs`, `src/engine/agent.rs`, `tests/multiplayer_tcp.rs`, `docs/agents.md` |
| Personalized events | `/auto exploration|building|combat`, healthy-player/travel/modal/encounter gates, 120-second cooldown, three attempts/session; `/pause` and reset controls | `src/engine/director.rs`, cooldown regressions and prior 128-second PTY run |
| Shared human game | Independent full player sessions, normal inventory/crafting/storage/workstations/combat, world/entities tick once, distant terrain physics, separate death/respawn, reconnect and periodic/exit saves | `src/engine/actors.rs`, `src/engine/net.rs`, `src/engine/server_save.rs`, `src/engine/cooperative_tests.rs`, `tests/multiplayer_tcp.rs` |
| Shared director | Host-only requests and `/connect`, shared messages, no pause while typing; cancel/revoke/pause on host departure before promotion | `src/engine/network_client.rs`, `src/engine/net.rs`, real TCP/MCP and two-rendered-client checks |
| Optional LLM play | 16 stepped world/player tools plus six live director tools; absolute-path JSON/TOML client config; normal game rules, bounded observations/actions, no developer loadout/teleport | `src/engine/agent.rs`, `src/engine/mcp.rs`, `docs/agents.md` |
| New worlds | Typed deterministic recipes, validation/repair, retained prompt, named isolated saves and writer leases, native entry; optional trusted provider executable | `src/world/recipe.rs`, `src/worlds.rs`, `src/generator.rs`, `scripts/generate_world.py` |
| Join/distribute | `termcraft join user@host --player NAME` manages a local SSH tunnel and stable reconnect identity; native archive packager with license/docs/manifest/checksum; four-platform development artifact workflow | `src/engine/ssh.rs`, `scripts/package_release.py`, `.github/workflows/native-artifacts.yml`, `docs/distribution.md` |

The local executable is `/home/sebastian/projects/termcraft/termcraft`.
The package version remains `0.1.0`; generated archives are explicitly development
builds, not mislabeled beta artifacts. Network protocol is **15**, requiring
matching builds. New native saves use progression version **9**; back up older
saves before testing because older executables cannot read the new format.

## Measured results and validation


### Native terminal feedback correction, 12 October

Human playtesting exposed indefinite movement, repeatedly toggled doors, poor sky,
oversized swings, apparent frame layers and excessive default zoom. Earlier scripted
checks trusted ideal key events and mostly stationary renderer measurements.
That checkpoint bounded input even when key-up was missing, confirmed actual
control releases, removes world right-drag use repeats, brackets whole frames with
synchronized updates and paces output after flush. Default pixel overview doubles
world context; close detail is explicit. Clouds/sun/stars share one stable sky space,
and swings use proportionate shaded arms. Reach/obstruction feedback explains fixture
use; ladder-level storage/bed tests avoid furniture-top acrobatics.

[Review and reproduction](docs/design/terminal-feedback-2026-10-12/provenance.md)
and [exact build evidence](docs/assets/terminal-feedback-2026-10-12.json) cover a
capability-positive plain-key PTY, native/co-op lost releases, all-view door clicks
and moving ANSI load. The first unconfirmed hold retains repeat-based limitations;
emulator-specific latency and human visual/feel acceptance remain open. These
corrections are necessary progress, not completion of the overall quality target.

### Connected village redesign, 12 October

Fresh ordinary Overworld chunks now generate connected settlements with a cottage,
stone-roofed workshop, pale farmhouse, covered market gate, shared square and
irrigated garden. Clear street-level corridors replace furniture obstacles; reachable
lofts hold storage, beds and workstations. Real two-block side doors, framed windows,
restrained roofs and graded half-block road ends use the existing collision rules.
Saved chunks retain their existing buildings. Generation is deterministic across
chunk boundaries and negative coordinates; it does not retrofit existing saves or
recipe worlds. Mixed old/new generation boundaries can contain partial settlements.

Room backdrops now handle partial lofts, and detailed viewport framing gives roofs
more vertical space. Real torches illuminate the walking lane and loft storage.
Home recognition pairs opposite doors and excludes the market; daytime villagers
bias toward the outside of their entry instead of oscillating around a covered door.

Two read-only review rounds covered layout, actual day/night captures and gameplay.
[Village review and reproduction](docs/design/village-refinement-2026-10-12/provenance.md)
record generated geometry, normal-input walking/climbing/storage/bed checks,
hydration and save persistence. The current [qualification](docs/assets/village-refinement-2026-10-12.json)
keeps build identity separate from prior reports. This completes the first connected
settlement pass. Greater architectural/biome variety, human art/feel acceptance,
full survival, fluids/boats and real WAN qualification remain open.

### Further creature physics and room review, 12 October

A second independent review found center-point flying collision, undersized spawn
clearance, villager home steering cancelling recoil, and NPC fluid movement using
the selected player's jump hold. Full-body gravity-free flight sweeps, full-footprint
spawn rejection, hurt steering/jump gates and explicit NPC input address these.
Regressions cover both flight directions, fast side/head impacts, slab air, tree
platforms, whole AI ticks, low-ceiling spawn pockets and daytime/nighttime recoil.

A full architectural envelope replaces row-by-row cutaway guesses. Open ruins and
shelters retain sky; complete huts retain warm wooden or muted masonry back walls.
Transparent props preserve the backdrop. Compact trial goals and contextual controls
remain readable at 80 columns. Updated captures compare the same seeded village
and trial terrain, with focused flying-body impact fixtures.

[Review](docs/design/room-physics-2026-10-12/provenance.md) and
[qualification identity](docs/assets/room-physics-2026-10-12.json) retain the final
source/binary, **622 unit tests**, normal-rule CLI completion, transport/input checks
and a new enclosed-room renderer profile. Network fields and progression formats
are unchanged at protocol 12 / progression v7. Human enjoyment, real WAN feel,
clean Windows/macOS and full survival/fluid/boat qualification remain open.

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

**Shared play:** Eight connections/64 saved identities across all three dimensions, trusted
local TCP or an existing SSH tunnel. Player IDs, not custom display names, label
avatars. Prediction remains approximate and server-authoritative; qualify jitter,
slow clients, bandwidth and input feel under latency. No public host/accounts or
internet-grade abuse controls have been provisioned. Independent co-op Nether/End portal travel is implemented in the follow-up pass;
real human cross-dimension progression remains a qualification gate.

**Persistence:** Native v9 and server v3 checkpoints retain entities and encounters
alongside their associated player inventories. Dimensions and item ages pause
while offline, and solo travel retains each world's entities. Older checkpoints
remain readable but contain no historical entities to recover. Writes remain
atomic per file, not across a whole world/plan. Recovery copies capture loaded chunks
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


## Final automated follow-up, 15 October

`d48fe93` adds native/shared tick-by-tick obstacle routes and corrects a demonstrated
partial-submersion swimming stall. Glowstone becomes a full crystal block; Nether
and End portals have distinct world-anchored membranes. All 95 world-gallery
scenes/213 views were recaptured, including eight client-replica co-op scenes,
and all 97 block/state comparisons refreshed. The local optimized executable is
updated; start with `./termcraft trial`.

Both Rust versions pass 684 unit checks and strict Clippy. Actual native/co-op
terminal run+jump/stop/focus-loss checks pass. Lantern Run completes through normal
CLI/MCP rules and survives reopen; the generated village route walks through six
doors without jumping or floor jitter. The Linux archive launches from a path
with spaces without Rust in PATH. See `docs/assets/cooperative-qualification-2026-10-15.json`.

Human art/feel acceptance, a sustained human co-op session, classic survival through
the dragon, real WAN links, interactive Windows/Mac terminals and live model quality
remain release gates. Entity/loose-item restart persistence was subsequently
implemented in the 16 October follow-up below. Larger authoring/terrain changes
and architectural variety remain unimplemented follow-ups; these checks
do not silently expand the game master's bounded Overworld contract. No release
version bump, tag, crate publication or merge has been performed.


## Native qualification collected 16 October

[Run 38797431069](https://github.com/pagel-s/termcraft/actions/runs/38797431069)
passes all four targets: Linux x86_64, Windows x86_64, macOS ARM64 and macOS Intel.
Each target passes Rust 1.88 tests, strict Clippy, native build, archive hashes,
launcher argument forwarding and no-Rust-in-PATH MCP/save-reopen checks. Unix
checks native/default-launcher PTYs and terminal restoration; Linux also checks
real OpenSSH joining. This qualifies automated native packages, not interactive
human OS/terminal behavior. The macOS terminal observer keeps the controlling
session alive until restoration is checked, and rejects a raw-mode negative control.

The final eight-player delayed-link soak passes for ten wall-clock minutes, with
roughly 12,000 snapshots per player, seven stalled readers removed and saved
reconnect after restart. Door clicking/right-drag passes in overview, wide and
close views. Evidence is retained in `docs/assets/cooperative-qualification-2026-10-15.json`.

## Entity and encounter persistence, 16 October

Native v9 and server v3 checkpoints now retain every visited dimension's complete
entity state with the corresponding player inventories. Solo portals/quick travel
and respawn preserve world entities, finite game-master waves resume surviving
members, and Lantern Run retains credited kills and remaining guards. End health
and destroyed crystals no longer reset on reopening. Item/XP ages and dimension
clocks pause offline. Native v1–8 and server v1/v2 remain readable; older executables
cannot reopen the new formats. Network protocol stays 14.

A runtime revision selects the newest entity checkpoint when switching between
native and hosted play in the same save root. Separate native/shared inventories
retain their existing identities. Invalid records are rejected, failed checkpoints
keep the previous inventory/entity pair, and failed travel no longer consumes its
cooldown. This completes the entity/loose-item/encounter persistence follow-up;
whole-world crash consistency and physical-storage qualification remain open.

[Implementation, scope and reproduction](docs/design/entity-persistence-2026-10-16/README.md)
record the regression checks, real-server restart/pickup test and final build identity.

## Slow-client isolation and checkpoint recovery, 16 October

The latest native workflow exposed a real Intel Mac stalled-reader freeze after
the earlier four-platform success. Source `52861f6` moves all server socket writes
to bounded per-player writer queues, preserves packet order and rejection delivery,
and batches disconnect saves. The original one-second healthy-client assertion is
unchanged. All five real TCP/MCP tests pass locally; a two-minute eight-player run
has a worst healthy update gap of 56 ms during seven stalled-reader disconnections.
This measures application update arrivals in the fixture, not input latency or WAN
quality.

Linux fault injection now exercises checkpoint ENOSPC, partial/slow writes, fsync
failure and process exit at either side of rename using normal CLI crafting and
reopen. A real native failed-save exit restores the terminal and produces a private
emergency copy; overlay into a second backup copy retains unsaved movement through
two reopenings. The fault library is test-only, scoped to a disposable checkpoint.
These checks join CI and the local smoke gate; they do not prove whole-world
transactions or physical-disk durability.

The updated local executable passes 716 stable/MSRV unit checks, strict Clippy,
native/shared input and action PTYs, real SSH joining and no-Rust Linux archive
checks. [Exact evidence and current platform results](docs/design/reliability-2026-10-16/README.md)
are retained separately from earlier candidates. Human feel/art acceptance, a
full survival campaign, sustained human co-op, WANs and live model quality remain
the main release gates.

General CI passes on the networking source, and Intel Mac now passes its real
co-op/MCP regression. Windows and ARM Mac complete native qualification. The
same matrix catches a random Linux door fixture: a generated neighboring village
reassigns the test villager's home. `b35fe15` isolates the day/night door and step
routes without relaxing their assertions; all 60 repeated checks and both full
unit suites pass. Fresh native CI is running for that correction; its pending
stages remain unqualified. No production villager behavior changed.

## Fluid flow correction, 16 October

The source-based fluid solver now computes a deterministic next frame before
applying edits. Falling columns remain vertical, supported runoff seeks the
nearest drop, and unsupported branches decay after removal or blocking of supply.
Water/lava contact no longer depends on which fluid happened to update first.
Source regeneration and existing fluid ranges/cadence remain explicit game rules.

Visible fluid depth now agrees with player immersion, breathing and boat height
in all color views; thin puddles retain grounded jumping. Sky above shallow flow
stays sky. Legacy zero-level flows drain without underflow, and new Lantern Run
pools use persistent sources. Existing save and protocol formats are unchanged.
The [flow review](docs/design/fluid-flow-2026-10-16/README.md) records test results,
actual bucket/save-reopen checks, screenshots and limits. Finite-volume pressure,
waterlogging and human swimming/boat acceptance remain separate follow-ups.

## Source-height basin follow-up, 16 October

Water now fills contained basins toward the source surface, including rising
through connected low passages. Local spillways constrain each terrace; overflow
feeds downstream pools. Opened drains lower the pool and closing them lets it
refill. Waterfalls and uncontained runoff retain gravity-first routing.

Full supplied water is separate from source cells and survives saving/co-op
replication. Removing or isolating the sole source makes its supplied pool recede;
this is a source-fed hydraulic model, not conserved finite water volume. Loaded
geometry bounds the calculation, and settled pools leave the active frontier.
Matching protocol **15** builds are required. See the
[basin review](docs/design/water-basins-2026-10-16/README.md) for qualification.

## View, movement and pressure follow-up, 16 October

Full Detail now stays at the authored sprite resolution on compact terminals and
in the End. `=`/`+` zooms in and `-` zooms out; the header labels the selected view.
Changing terminal font/window size changes context, without silently changing the
selected artwork. F4 retains its existing cycle. Legacy input timeout resets the
pending sprint tap, preventing a delayed OS repeat from starting sprint. Actual
directional releases retain intentional double-tap-and-hold sprinting.

Rendering/terminal flushing consumes the frame budget; it no longer adds a whole
sleep after each rendered frame. Co-op diagnostics now measure client prediction
and render work. They do not measure the server or the terminal emulator's GPU.

The basin follow-up also found that pressure could cross dry air from an unrelated
high source. Heads now propagate through actual wet connections, with one dry
growth neighbour. The solo/co-op obstacle fixture is enclosed against unrelated
generated terrain; the earlier Windows/ARM Mac failure requires fresh CI results.
See the [view/input review](docs/design/view-controls-2026-10-16/README.md).
