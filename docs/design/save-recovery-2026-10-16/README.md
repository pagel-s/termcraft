# Save recovery review — 16 October 2026

An interrupted save could previously publish a chest's reduced inventory before
the player receiving its item was saved. The regression reproduced chest 3 /
player 0 instead of the previous chest 4 / player 0. Per-file atomic rename did
not prevent this cross-file loss. The captured pre-fix failure is retained in
`before-regression.txt`.

## Implementation

Feature source: `325bfb6`; Windows initialization warning fixed in `c71d1fc`.
Builder mutability is now confined to Unix; creation behavior is unchanged. Windows had passed its tests before strict Clippy
reported that warning. Fresh platform qualification is recorded in the evidence.

`src/checkpoint.rs` supplies an undo journal under the world's exclusive lease.
Before-images and a checksummed manifest are synced before world files change.
All dimensions and native/server records are flushed before the completion
marker. Checked startup rolls unfinished work back before loading any sessions.
Rollback preflights every image, uses separate restore temporaries, and remains
retryable after another interruption. Completed cleanup is retired atomically
before deletion; reserved orphan temporaries are removed after checked recovery.

`src/world/chunk.rs` covers autosave and dirty cache eviction. Native persistence
is in `src/engine/state.rs`; hosted saves finish in `server_save.rs` after
`ServerSimulation::save` flushes all dimensions. World and trial
sidecars join that checkpoint. No existing payload version or network protocol
changes. The journal is version 1 and requires a matching build for recovery.

## Qualification

The Linux fault interposer only targets disposable test roots. It neither fills
a real disk nor edits an existing player world.

- Six normal CLI/process crafting cases: ENOSPC, partial/slow writes, fsync failure,
  and process exit before/after player-file rename. Publishing that file alone
  does not complete the world checkpoint.
- Twenty-four staged all-dimension cases use real serializers and actual SIGKILL
  at partial writes, selected rename boundaries and before/after completion.
  Storage errors and a second SIGKILL during rollback leave recovery retryable.
  Each case reopens twice and conserves 17 diamonds across player, dropped loot
  and three chests, with matching builds and new-chunk presence.
- Six real TCP server cases place blocks through normal `UseAt` commands, with
  three identities in Overworld/Nether/End. SIGKILL after chunk/server publication
  restores all three old inventories/builds; killing after completion retains all
  three new ones. Each case reconnects twice with the same credentials/dimensions.
- Native PTY failure restores terminal modes and creates a private emergency
  copy. A backup taken after failure first resolves its pending journal in a
  second copy, then receives the emergency overlay and reopens twice.
- Library regressions cover dirty eviction, all entity families in all visited
  dimensions, clocks, boss health and portal history; corrupt before-images,
  deleted manifest entries, symlink rejection and interrupted cleanup.

Reproduce with:

```sh
cargo build --release --locked --bin termcraft --example storage_review
python3 scripts/test_storage_recovery.py --binary target/release/termcraft \
  --fixture target/release/examples/storage_review --native-recovery \
  --report /tmp/termcraft-storage.json
TERMCRAFT_HOSTED_STORAGE_REPORT=/tmp/termcraft-hosted-storage.json \
  cargo test --locked --test multiplayer_tcp -- --ignored
```

The expanded Linux matrix joins ordinary CI and `scripts/release_smoke.sh`.
The optional native PTY check requires Pillow and terminal allocation.

## Limits

These are process-crash and injected-storage-error checks, not hardware power
cuts. Files are synced on every platform; directory fsync is Unix-only. Actual
controller/filesystem durability and physical full/slow devices remain manual
qualification. Checkpoints preserve the last completed save, so later progress
may be lost. Emergency native exports remain partial loaded-terrain snapshots;
they are not whole-world backups. Preserve hidden journal files with backups,
verify a crashed owner is dead before clearing its lease, and follow the
[copy recovery guide](../../world-format.md#recovering-into-a-copy).

Exact final source, executable identities and validation results are recorded in
`evidence.json`; individual fault outcomes are in `storage-faults.json` and
`hosted-storage.json`.
