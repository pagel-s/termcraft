# Beta qualification and release handoff

Updated 12 October 2026. Candidate: `0.2.0-beta.1`; current development
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

## Human, model and platform gates

Record OS, terminal, executable hash, world/backup, participants, duration and
failures. Test copies of saves: older builds cannot open progression v7.

- [x] **Art direction:** terminal pixel art selected from the initial comparison.
- [ ] **Art readability and consistency:** assess the coordinated characters,
  creatures, terrain, structures and HUD in real play. Review the
  [entity/proportion/shadow audit](docs/design/art-audit-2026-10-11/provenance.md),
  including small-view identities and night/cave lighting, plus the
  [dragon refinement](docs/design/dragon-refinement-2026-10-12/provenance.md) and
  [complete block/item/physics review](docs/design/2d-refinement-2026-10-12/provenance.md).
- [ ] **Gameplay quality:** play a coherent five-minute encounter with readable
  attacks, useful rewards and a building/traversal payoff. Automated foundation
  tests do not establish that this target is met.
- [ ] **Classic survival:** fresh world through dragon/credits; early resources,
  storage/workstations, death/respawn, v1–6 save upgrades, dimension travel and
  reopen. Developer loadouts/teleports do not count as a playthrough.
- [ ] **Physics feel:** walk/stop/sprint/jump, fast wall/ceiling impacts, stairs,
  sneaking at both ledges, ladders, knockback, fall damage, swimming/drowning,
  boats, falling blocks and fluids; repeat in supported shared play.
- [ ] **60-minute human co-op:** at least two people, distant exploration,
  contested edits/items/storage, combat/death, director, reconnect and restart.
  Use Overworld only and include an actual remote SSH account/delayed link.
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
  on disposable copies; exercise interrupted writes and slow/full storage.
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
5. Finalize the draft release notes. Retain protocol 11 matching-build and
   progression v7 backup/migration notices and remaining gameplay limits.
6. Regular pushes to `beta/terminal-reborn` are user-authorized and have been
   performed, with Sebastian Pagel as sole author/committer. Obtain separate
   authorization for tagging, publishing crates/releases, manually dispatching
   workflows, announcing or provisioning hosting. Those actions remain unperformed.
