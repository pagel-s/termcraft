# Performance and foundation checks

These are local measurements of the unreleased development build on 10 October
and 11 October 2026, not beta qualification or a comparison with a published release.

## Exploration cache

Native single-player now retains chunks within six chunk columns of each viewer.
Distant clean chunks are discarded. Distant dirty chunks are saved before being
discarded, with at most one such write per frame. Save failures keep the data in
memory and show a warning; disk failure can therefore exceed the normal working
set. MCP sessions trim clean distant chunks after their checked saves. The server now trims against all active players, retaining failed dirty writes;
streaming and slow readers have a repeatable loopback test below; WAN and human input
feel still need qualification.

This removes the previous growth in resident chunk data as exploration continues.
It trades a small amount of budgeted disk work for a smaller working set. It does
not establish an FPS improvement, a fixed process-RAM limit, or good performance
on a slower disk.

## Reproduce the headless profile

```sh
cargo build --release --locked --example profile
./target/release/examples/profile
./target/release/examples/profile --trim
./target/release/examples/profile --classic
./target/release/examples/profile --classic --trim
```

Use the corresponding target path if `CARGO_TARGET_DIR` is set. The example owns
fresh temporary save directories and removes them after a successful run; it
never opens the user's saves. Each run visits 240 consecutive chunk centers and
measures 600 actual simulation ticks with 32 zombies, frozen nighttime and natural
spawning disabled. It reports percentile work times, startup time, resident chunk
counts and remaining zombies. Terrain seeds are fixed; runtime mob AI still uses
randomness. A death screen invalidates the fixture rather than making the timing
look artificially fast.

Three sequential runs per configuration on Linux x86_64, Intel Core i9-13900K,
Rust 1.98.1, optimized build, temporary local save storage:

| Terrain | Cache trimming | Peak loaded chunks | Exploration p95 work, range across runs | Dense-mob tick p95, range across runs |
| --- | --- | --- | --- | --- |
| Recipe, seed 42 | Off | 244 | 0.071–0.102 ms | 0.220–0.255 ms |
| Recipe, seed 42 | On | 9 | 0.126–0.173 ms | 0.221–0.235 ms |
| Classic, fixed seeds | Off | 244 | 2.906–2.916 ms | 0.256–0.281 ms |
| Classic, fixed seeds | On | 8 | 2.900–2.939 ms | 0.248–0.260 ms |

All runs ended with all 32 benchmark zombies present. The cache toggle is the
controlled difference within the same build. Async prefetch scheduling can change
exact resident counts and timing. The measurements exclude terminal rendering,
networking and model inference. See the [raw results](assets/performance-2026-10-10.json)
for individual runs, p99/max values and limitations.

## In-game diagnostics

Press **F3** to toggle diagnostics. Normal play shows a short dimension/weather/
difficulty bar instead of the internal chunk counters. Diagnostics include rolling
p95 tick work, terminal rendering work (including writing/flushing output),
save/cache maintenance work, resident/evicted chunks and catch-up drops. Windows
retain the last 300 samples, not the whole session. Maintenance samples include
frames without a save, so occasional disk stalls can be hidden by its p95; use
targeted save profiling as well.

Check at least 120×40 and 200×60, then record terminal, OS, hardware and scene
before comparing results. A PTY smoke check covers F3, resize, native entry/exit
and terminal restoration. PTY captures are not measurements of an actual terminal
emulator's presentation speed.

## Renderer measurements, 11 October

`examples/render_profile.rs` runs the real composition, lighting and ANSI diff
encoding paths at 120×40 and 200×60. Each scene has 210 frames, with 30 warmup
frames excluded from timing. A counting writer measures bytes and write calls;
it excludes kernel I/O and actual terminal-emulator presentation. Geometry is
fixed across runs, including a 96-torch cave, dense mobs, storm, Nether and End.

```sh
cargo run --release --locked --example render_profile
# Optional ANSI captures; disk capture timings are not benchmark evidence:
cargo run --release --locked --example render_profile -- /tmp/termcraft-frames
```

On the same i9/Linux host described above, three alternating runs with normal
colors enabled gave the p95 renderer work ranges below. The baseline is a
temporary copy of the current source with the original 18-pass lighting and
small-write encoder restored from commit `1df974f`. All current visuals, scene
fixtures and other code are identical, so this isolates those two optimizations;
it is not a comparison with the complete published release.

| Scene | Viewport | Original algorithms, ms | Optimized, ms |
| --- | --- | ---: | ---: |
| Daylight | 120×40 | 0.576–0.580 | 0.423–0.444 |
| Daylight | 200×60 | 1.182–1.259 | 0.931–0.960 |
| Storm | 120×40 | 0.654–0.711 | 0.542–0.574 |
| Storm | 200×60 | 1.439–1.490 | 1.238–1.287 |
| Torch cave | 120×40 | 0.867–0.925 | 0.322–0.337 |
| Torch cave | 200×60 | 1.811–1.940 | 0.672–0.706 |
| Dense mobs | 120×40 | 0.575–0.589 | 0.422–0.445 |
| Dense mobs | 200×60 | 1.241–1.255 | 0.915–0.961 |
| Nether | 120×40 | 0.619–0.639 | 0.328–0.354 |
| Nether | 200×60 | 1.252–1.318 | 0.714–0.760 |
| End | 120×40 | 0.326–0.346 | 0.281–0.297 |
| End | 200×60 | 0.677–0.720 | 0.579–0.612 |

Lighting now uses descending brightness buckets and precomputed material
attenuation instead of 18 relaxation passes with repeated world lookups. A
randomized reference regression checks equivalent light fields. Frame encoding
reuses a buffer and sends one write per frame. Output byte counts match for all
12 scenes across all six color-enabled runs; write calls fall from up to 3.45
million to 210 over each fixture. These are work/output measurements, not a claim
of the same percentage increase in visible FPS. Real-terminal qualification remains.

An earlier paired run inherited `NO_COLOR` from the execution environment. It is
retained and explicitly labeled in the [raw results](assets/performance-2026-10-11.json),
including the comparison before visual changes. In that fixture, slower End
sparkles reduced final output bytes by 19–23%; clouds preserve mean drift speed
with fewer updates. Color-enabled captures were reviewed separately from timings.

## Accelerated shared exploration

```sh
cargo run --release --locked --example coop_profile -- 72000
```

Eight normal empty-inventory actors move in two opposing groups for 72,000 ticks
(60 **simulated** minutes), with natural spawning disabled. Every tick includes
shared simulation, budgeted cache maintenance and encoding all eight snapshots;
45-second simulated intervals save. The local run completed in 15.67 seconds,
with p95 0.248 ms, p99 0.334 ms and max 3.454 ms. Peak resident chunks were 22;
peak sampled Linux RSS was 7,064 KiB. It encoded 580 MB total snapshots, about
161 KB/s across eight actors per simulated second before terrain/transport overhead.
That bandwidth needs real network measurement and is not a compression claim.

This catches runaway chunk retention and shared-tick work in the specified
fixture. It is **not** a 60-minute wall-clock soak, delayed-network test, full
survival session or peak process-memory bound. The runtime never calls a model
from physics or chunk generation.

The 15 October co-op follow-up removes duplicate player-session/entity captures
while constructing each snapshot. Three local runs before and after show p95
work times of **0.244–0.247 ms** and **0.237–0.239 ms** respectively for the same
eight-player terrain/encoding workload. This is a small CPU saving, not a claim
of visibly smoother play. Random simulation/weather are not seeded and output
sizes vary slightly; it is not an exact output-parity benchmark. One trial failed
without retained stderr and was retried; its cause is unknown. The
[raw runs](assets/coop-snapshot-profile-2026-10-15.json) preserve these limits.

Further qualification remains: slow/full storage, actual terminals and SSH links,
a wall-clock co-op soak, natural entity populations, fluids/redstone, complete
progression and model latency/task quality. Unit regressions cover ladder holding,
ledge momentum, release braking, fall damage and high-speed collisions; movement
feel, swimming, boats and passive-mob parity still need hands-on play.

## Wall-clock network and SSH checks, 11 October

`tests/multiplayer_tcp.rs` now includes an eight-player process/socket test with
normal inventory, flat terrain and natural spawning disabled. One ordered byte
relay adds 80/100/120 ms delay per direction without a per-read bandwidth sleep.
A 120-second wall-clock run receives about 2,400 snapshots per actor, with direct
and delayed snapshot-gap p95 of 55.9 and 76.0 ms. Seven authenticated clients then
stop reading; they are removed in 11.4 seconds. The worst healthy-client snapshot
gap during that phase is 164 ms. Saved shutdown and reconnect after restart pass.

```sh
TERMCRAFT_NETWORK_SOAK_SECONDS=120 \
TERMCRAFT_NETWORK_REPORT=/tmp/termcraft-network.json \
cargo test --locked --test multiplayer_tcp eight_players -- --ignored --nocapture
```

The default duration is 12 seconds; 6–3600 seconds are supported. These are
application snapshot-arrival gaps sampled by the test, not ping or input latency.
Ordered delayed TCP buffers do not simulate packet loss. No terminals/models run
in this fixture, and two minutes do not qualify the planned human hour.

The separate `scripts/test_ssh_join.py` uses real OpenSSH with key authentication
and encryption against a disposable loopback Paramiko endpoint. Key/auth/forward
rejection, restart/reconnect, dropped-link terminal restoration and tunnel cleanup
pass. Setup is in `RELEASE_CHECKLIST.md` in the source checkout. These tests do
not change host services or qualify remote accounts/WAN behavior. Raw reports
and the interactive Codex-director check: [qualification evidence](assets/qualification-2026-10-11.json).
