# Slow-client isolation and checkpoint recovery — 16 October 2026

Source: [`b35fe15`](https://github.com/pagel-s/termcraft/commit/b35fe1556b5bf69134273f6699fa59384cd2ffac),
branch `beta/terminal-reborn`. Local optimized executable SHA256:
`8ac58c856dc2976ec8fdd8ef5354affcf71588973f8259d507fca3f42569eade`.
Native checkpoint v9, server checkpoint v3 and network protocol 14 are unchanged.
The local `./termcraft` was updated atomically; existing player saves were not opened.

## What changed

The previous [native run](https://github.com/pagel-s/termcraft/actions/runs/38883123226)
passed Linux, Windows and ARM Mac, but Intel Mac failed
`eight_players_with_delayed_link_and_stalled_readers_keep_server_responsive`:
seven clients that stopped reading could freeze the healthy player's updates.
This was a real failing gate after the earlier four-platform qualification.

[Server writes](https://github.com/pagel-s/termcraft/blob/52861f69e0275a54a53bd4785ad450d2f2e11e63/src/engine/net.rs)
now use a fourteen-packet bounded queue and dedicated writer per connection. Socket
writes, including partial frames and coarse platform timeouts, run outside the
simulation thread. Overflow removes only the slow client; ordered welcome,
snapshot, chunk and rejection frames are preserved. Rejections close after being
written. Reader/writer disconnect notices are idempotent, and simultaneous
disconnects batch their checkpoint save once per simulation loop. Disk saves
themselves still run synchronously.

Unit regressions deliberately block a writer while filling the bounded queue,
then verify ordered draining and close-after delivery. The real two-player TCP
test checks an unknown credential receives an intact rejection followed by EOF
without disrupting healthy players. The original **one-second** healthy-reader
assertion remains unchanged. A separate regression queues Welcome and a full
six-tick snapshot/terrain burst before the writer runs; transient catch-up must
not be mistaken for sustained backpressure.

## Save failure and recovery qualification

[`test_storage_recovery.py`](https://github.com/pagel-s/termcraft/blob/52861f69e0275a54a53bd4785ad450d2f2e11e63/scripts/test_storage_recovery.py)
compiles a Linux test-only interposer into a temporary directory. Only the test
child processes opt in, and it matches only `player_progression.bin.tmp` under
their explicitly supplied disposable world. No filesystem is filled or mounted;
normal game launches load no fault library.

Six cases use actual CLI/process entry, ordinary stick crafting, save and reopen:

| Fault | Checked result |
| --- | --- |
| ENOSPC before a write | Old checkpoint retained; live craft retained; retry/reopen restores it |
| Partial write then ENOSPC | Incomplete temporary file never replaces the checkpoint; retry succeeds |
| `fsync` EIO | Failed sync never publishes the new checkpoint; retry succeeds |
| Delayed write | Previous checkpoint stays published while temporary write is blocked |
| Process exit before rename | Old inventory reopens; uncommitted craft is absent |
| Process exit after rename | New inventory reopens; committed craft is retained |

The optional native test sends real movement and quit keys through a PTY, forces
the quit save to fail, and checks nonzero exit, restored terminal modes and a
private `0700` emergency directory. The original checkpoint and separate backup
remain unchanged. Following the documented recovery procedure, it overlays the
partial recovery into a second copy, then confirms the unsaved moved position
through two normal reopenings. Only the test child's own stale lease is removed,
after confirming that child exited. All worlds and keys belong to the fixture.

## Villager fixture correction

The follow-up [native run](https://github.com/pagel-s/termcraft/actions/runs/38888427314)
passes the previously failing Intel Mac real co-op/process step. Windows and ARM Mac
complete their full jobs. Linux instead exposes a pre-existing random door-test
failure: the minimal test hut sits in generated terrain, and a neighboring village
can reassign its villager's home. A diagnostic repetition reproduced this on the
fourth attempt, with the home moved to `(41, 30)` and the villager walking toward
the opposite wall. This was unrelated to socket writing.

The day/night door routes and one-block step test now use the existing isolated
replica fixture, with fixed clear weather/time and no unrelated settlement. Their
door-opening, traversal, outside/shelter and closing assertions remain intact.
[Sixty repetitions](villager-fixture-repeats.json), twenty per route, and both full
716-test suites pass. The diagnostic [reproduction log](villager-fixture-reproduction.log)
retains the failed home assignment. No production villager behavior was changed.

## Results

- 716 unit tests on stable Rust and Rust 1.88; strict all-target Clippy on both;
  formatting and offline crate package verification pass.
- All five real TCP/process process tests pass on networking source `52861f6`.
- The [120-second eight-player soak](network-soak.json) passes: direct/delayed
  snapshot-gap p95 is 55.8/76.0 ms; worst healthy gap during seven stalled-reader
  removals is **56.4 ms**. Saved restart/reconnect succeeds. The stalled phase,
  including filling socket buffers, lasts 11.4 seconds.
  This measurement is from `52861f6`; its production networking is unchanged by
  the later test-fixture-only correction.
- [Checkpoint faults and native backup-copy recovery](storage-faults.json) pass.
- Actual native/co-op [input](terminal-input.json) and [actions](terminal-actions.json)
  pass, including run/jump/continued running/release/focus loss, bow poses and arrow
  consumption in all three views, eating and terminal restoration.
- [Encrypted OpenSSH](ssh.json) checks pass for joining, restart/reconnect,
  rejected host keys/auth/forwarding, missing server and dropped-link cleanup.
- [Linux archive](native-archive.json) extraction, hashes, launchers/arguments
  from a path with spaces, no-Rust save/reopen and native restoration pass.
- [Lantern Run](lantern-run.json) completes and reopens through ordinary CLI/process
  inventory/crafting/combat/placement rules, with observed bridge landing.

CI and native platform results are retained in `ci-at-verification.json`; use the
exact source/run there when downloading artifacts. [The evidence manifest](evidence.json) records
source, executable, reports and check-log hashes. Temporary PTY capture directories
in reports are diagnostic paths, not gallery assets.

The [fresh native matrix](https://github.com/pagel-s/termcraft/actions/runs/38889843430)
for the isolated fixture is separate from the preceding run's Mac regression
success. Pending jobs in the snapshot are unqualified; the
Linux job above failed before building its archive or running the new storage test.

## Reproduce

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked --test multiplayer_tcp -- --ignored
cargo build --release --locked
TERMCRAFT_NETWORK_SOAK_SECONDS=120 TERMCRAFT_NETWORK_REPORT=/tmp/network.json cargo test --locked --test multiplayer_tcp eight_players -- --ignored --nocapture
python3 scripts/test_storage_recovery.py --binary target/release/termcraft --native-recovery --report /tmp/storage.json
```

Fault checks require Linux and a C compiler; the optional native check also needs
Pillow and PTY/loopback permission. CI exercises the six CLI faults; Linux native
artifact CI also exercises failed-save terminal restoration and backup-copy recovery.

## Limits and open gates

Application snapshot arrival gaps are not input latency. The network fixture has
flat terrain, spawning disabled, ordered delayed TCP and no terminal rendering or
packet loss. SSH uses a local encrypted test endpoint, not a real WAN account.
Automated movement/actions do not establish visual quality or human enjoyment.

Process-scoped checkpoint faults do not prove physical-disk power-loss durability,
directory sync behavior or atomicity across chunks, metadata and dimensions.
Recovery remains a partial current-dimension terrain copy requiring an intact
backup. Native/server checkpoints still keep their existing distinct player
identities. Human art/feel acceptance, full survival through the dragon, sustained
human co-op/WAN play, interactive native terminals and live model quality remain
release gates. No version bump, merge, tag or publication was performed.
