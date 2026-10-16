# Beta qualification and release handoff

Updated 16 October 2026. Candidate: `0.2.0-beta.1`; current development
manifest/executable: `0.1.0`. Nothing here authorizes publication or public hosting.
See [BETA_RELEASE_PLAN.md](BETA_RELEASE_PLAN.md) for results and limitations and
[development release notes](docs/beta-release-notes.md) for player-facing changes.

## Repeatable local gates

- [x] Stable and minimum Rust 1.88 unit tests, formatting, strict all-target
  Clippy, release executable, offline crate verification and strict MkDocs build.
- [x] Real TCP contested resources, crafting/building, reconnect/restart and
  live MCP director host/guest/cancel/revoke regressions.
- [x] Native/shared rendered terminal checks: chat, delayed provider, modal
  deferral, resize, input isolation, exit and save-error recovery.
- [x] Color-enabled renderer comparison with identical output; Linux archive
  extraction, checksums and native/MCP launch without Cargo/Rust in PATH; repeat with
  `python3 scripts/test_native_archive.py --directory dist`.
- [x] Real OpenSSH key authentication, host verification, forwarding failures,
  reconnect/restart and dropped-link cleanup; retain the fixture JSON report.
- [x] Eight-player wall-clock network soak, delayed/jittered buffers and stalled
  readers; retain the JSON report. This is separate from human play.
- [x] Linux process-scoped checkpoint ENOSPC/partial/slow/sync-error and
  interrupted-rename checks; native failed-save terminal restoration and recovery
  into a second backup copy, followed by two normal reopenings.

Run core gates with `./scripts/release_smoke.sh`. Socket tests need loopback
permission and create disposable worlds. For a longer network run:

```sh
TERMCRAFT_NETWORK_SOAK_SECONDS=3600 \
TERMCRAFT_NETWORK_REPORT=/tmp/termcraft-network.json \
cargo test --release --locked --test multiplayer_tcp eight_players -- --ignored --nocapture
```

The default test uses 12 seconds, eight normal-inventory actors on flat terrain
with spawning disabled, one ordered relay with 80–120 ms delay per direction,
then seven authenticated stalled readers. Duration accepts 6–3600 seconds. It
asserts movement, live snapshots, slow-reader removal, saved shutdown and restart.
It does not render terminals, simulate packet loss or assess survival/movement feel.

For SSH on Unix, use Python 3.11+, OpenSSH and the test-only Paramiko dependency:

```sh
python3 -m venv /tmp/termcraft-ssh-tests
/tmp/termcraft-ssh-tests/bin/pip install paramiko==5.0.0
/tmp/termcraft-ssh-tests/bin/python scripts/test_ssh_join.py \
  --binary target/release/termcraft --report /tmp/termcraft-ssh.json
```

This uses the real OpenSSH client, temporary keys/known-hosts/config, an encrypted
loopback Paramiko endpoint and isolated saves. It changes no host account/service
and exposes no public listener. It does not qualify remote account administration.

## Earlier automated follow-up

The independent-dimension and portal-material candidate is `d48fe93`, protocol 13,
native checkpoint v8 and server checkpoint v2. The local executable is updated.
684 unit checks pass on stable/Rust 1.88; strict Clippy passes on both. The
eight-course native/shared movement comparison, actual terminal run+jump/stop
streams, normal-rule Lantern Run completion, generated-village door route and
Linux no-Rust launcher/archive checks pass. Source identities and limits are in
[the follow-up evidence](docs/assets/cooperative-qualification-2026-10-15.json).

Native CI now builds/tests/packages Linux, Windows and both Mac architectures on
beta source pushes. Download and judge only the exact successful run you test;
[the candidate's run](https://github.com/pagel-s/termcraft/actions/runs/38797431069)
passes all four targets, including archive/launcher and no-Rust MCP checks. This
is separate from human terminal qualification. No tag, crate or release was published.

## Human, model and platform gates

Record OS, terminal, executable hash, world/backup, participants, duration and
failures. Test copies of saves: older builds cannot open progression v9.

- [x] **Art direction:** terminal pixel art selected from the initial comparison.
- [ ] **Art readability and consistency:** assess the coordinated characters,
  creatures, terrain, structures and HUD in real play. Review the
  [entity/proportion/shadow audit](docs/design/art-audit-2026-10-11/provenance.md),
  including small-view identities and night/cave lighting, plus the
  [dragon refinement](docs/design/dragon-refinement-2026-10-12/provenance.md) and
  [complete block/item/physics review](docs/design/2d-refinement-2026-10-12/provenance.md).
- [ ] **Gameplay quality:** play a coherent five-minute encounter with readable
  attacks, useful rewards and a building/traversal payoff. Automated foundation
  tests do not establish that this target is met. Lantern Run is now available via
  `termcraft trial`; its normal-rule CLI completion is automated, human acceptance
  remains open.
- [ ] **Classic survival:** fresh world through dragon/credits; early resources,
  storage/workstations, death/respawn, v1–8 save upgrades, dimension travel and
  reopen. Developer loadouts/teleports do not count as a playthrough.
- [ ] **Physics feel:** walk/stop/sprint/jump, fast wall/ceiling impacts, stairs,
  sneaking at both ledges, ladders, knockback, fall damage, swimming/drowning,
  boats, falling blocks and fluids; repeat in supported shared play.
- [ ] **60-minute human co-op:** at least two people, distant exploration,
  contested edits/items/storage, combat/death, director, reconnect and restart.
  Include independent Nether/End travel, shared dragon progression and an actual
  remote SSH account/delayed link.
- [ ] **Existing-client game master:** keep Codex/Claude actively waiting; ask
  for a hard battle, passive mobs, snow and an accessible structure. Record
  request-to-application latency, repair count, accepted/applied status, fidelity
  and approval delays. Include cancellation and an unsupported request.
- [ ] **Optional agent play:** gather wood/craft/build under normal rules, save
  and leave, then human entry. Simultaneous human/LLM player entry is deferred.
- [ ] **Clean native platforms:** Linux x86_64, Windows x86_64, macOS ARM64/x86_64;
  build/extract/checksum archives, launch without Rust, and test keyboard, mouse,
  glyphs, colors, 120×40/200×60 resize, disconnect and orderly shutdown.
- [ ] **Backup/recovery:** follow the [recovery guide](docs/world-format.md#recovering-into-a-copy)
  on representative disposable worlds; exercise physical slow/full storage and
  whole-world interruption. The automated Linux checkpoint faults and native
  backup-copy overlay path above pass; broad disk/dimension consistency remains.
- [ ] **Onboarding/demo:** an uncoached player joins, gathers/builds and requests
  a change; record real shared play/screenshots. Label automated fixtures honestly.

## Release preparation after qualification

1. Fix blockers, rerun affected gates and retain the exact qualifying source and
   executable identity. Rebuild final archives after changes.
2. Align Cargo.toml/Cargo.lock, executable, crate, tag and archive versions.
   Remove development labels only after qualification; the packager currently
   marks every output `dev`.
3. Inspect license, startup instructions, manifest and `cargo package --list`.
   Exclude large videos/GIFs, private saves, bridge/reconnect credentials and keys.
4. Review Debian PR #4 and AUR guidance against the final version/executable.
5. Finalize the draft release notes. Retain protocol 15 matching-build and
   progression v9 / server v3 backup/migration notices and remaining gameplay limits.
6. Regular pushes to `beta/terminal-reborn` are user-authorized and have been
   performed, with Sebastian Pagel as sole author/committer. Obtain separate
   authorization for tagging, publishing crates/releases, manually dispatching
   workflows, announcing or provisioning hosting. Those actions remain unperformed.

## Entity persistence follow-up, 16 October

The current candidate uses native checkpoint **v9**, server checkpoint **v3** and
network protocol **14**. Native v1–8 and server v1/v2 upgrade on save. Mobs, boats,
loot/XP, partial director/trial encounters and End boss/crystal state now persist
across restarts and dimension travel, alongside inventories. Offline time pauses.
The newer runtime revision wins across native/hosted transitions. This supersedes
the earlier session-only limitation, without completing whole-world crash or human
play qualification. See [the review and evidence](docs/design/entity-persistence-2026-10-16/README.md).

## Slow-client and storage reliability follow-up, 16 October

Candidate source `52861f6` addresses the Intel Mac stalled-reader failure in
[run 38883123226](https://github.com/pagel-s/termcraft/actions/runs/38883123226).
Socket writes now drain bounded per-player queues on separate threads; a blocked
reader cannot hold the simulation in a socket write. Disconnection saves are
batched, and rejection frames are delivered before closing. Protocol 14 and save
formats v9/v3 are unchanged.

716 unit checks pass on stable/Rust 1.88, with strict Clippy on both. Five real
TCP/MCP tests, a two-minute eight-player soak, native/shared input and action PTYs,
encrypted OpenSSH joining, Linux archive runtime, offline crate verification and
checkpoint fault/recovery checks pass. The local executable is updated. Exact
source/binary identities, platform results and limits are in the
[reliability review](docs/design/reliability-2026-10-16/README.md).

The networking source passes general CI and the previously failing Intel Mac
co-op/MCP step; Windows and ARM Mac finish their native jobs. Linux exposes an
unrelated generated-village home reassignment in a door-test fixture. `b35fe15`
isolates that fixture and its neighboring route tests; 60 repeat checks and both
716-test suites pass. The [fresh native matrix](https://github.com/pagel-s/termcraft/actions/runs/38889843430)
is still running at evidence collection. Pending stages are not qualified.

## Fluid follow-up, 16 October

The [fluid review](docs/design/fluid-flow-2026-10-16/README.md) covers gravity-first
falls, nearby-drop routing, drainage, mixing, range/cadence, legacy zero levels,
chunk-order independence and depth shared by rendering/immersion/breathing/boats.
Actual bucket placement/pickup and save/reopen are exercised in disposable worlds.
Include source removal, thin runoff, deep swimming and shoreline exits in the
remaining human physics gate. This remains a block/source model; finite-volume
pressure and waterlogged blocks are not implemented.

## Source-height basin follow-up, 16 October

The [basin review](docs/design/water-basins-2026-10-16/README.md) adds communicating
vessels, wide/deep containers, source-height limits, spillways/terraces, drain
opening/closure, barriers, chunk seams, tick-window movement and separated co-op
regions. Native save/reopen and real TCP replication/pickup/restart are checked.
Require matching protocol **15** builds. Human swimming/shoreline/boat feel,
finite conserved volume, currents and waterlogged blocks remain separate gates.

## View and input follow-up, 16 October

Direct zoom and visible view names replace hidden terminal-size downgrades.
Detailed characters are exercised through real native/co-op input and 80×24 /
120×40 / 160×48 resizing. Walking, intentional double-tap sprint, delayed legacy
repeat, run/jump, release and focus loss are checked in the same PTY harness.
Frame pacing includes rendering/flush work, and co-op F3 records client timings.
Pressure follows actual wet connections; the movement fixture is isolated from
external generated sources. Fresh platform CI must qualify this correction to
the previous Windows/ARM Mac water-route comparison failure. Human Warp feel
remains a separate acceptance step. [Review](docs/design/view-controls-2026-10-16/README.md).

## Deep ore distribution follow-up, 16 October

The [ore review](docs/design/deep-ore-balance-2026-10-16/README.md) checks eight-row
depth bands, cave exposure, ore amounts in bottom sections, connected veins across
chunk seams and all five recipe terrains. Coal/iron taper near bedrock and rare
finds remain discoverable. The 750-test stable/MSRV suites, strict Clippy and six
real TCP/MCP integrations pass. The preceding view/input native matrix is fully
green; fresh CI must qualify the ore source. Test fresh chunks or a new world for
human mining pace and resource progression, since saved terrain is preserved.
