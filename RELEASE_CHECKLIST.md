# Release Checklist

Date: 2026-03-13

Use this checklist before cutting a release tag.

## Preflight

1. Verify clean branch and up-to-date remote:
   - `git status -sb`
   - `git fetch --all --prune`
   - `git pull --ff-only`
2. Run full smoke gates:
   - `./scripts/release_smoke.sh`
3. Optional long-run soak for stability:
   - `timeout 120s cargo run --quiet -- server 127.0.0.1:25569`

## Sanity Play Checks

1. Local mode:
   - `cargo run`
   - Confirm movement, mining/placement, inventory toggle, death/respawn, dimension transfer.
2. Multiplayer mode:
   - Terminal A: `cargo run -- server 127.0.0.1:25565`
   - Terminal B: `cargo run -- client 127.0.0.1:25565`
   - Confirm movement sync, chunk streaming, dimension switch sync.

## Release Notes + Tag

1. Summarize player-facing changes from the latest commits.
2. Include migration notes if save/network protocol behavior changed.
3. Create annotated tag:
   - `git tag -a vX.Y.Z -m "termcraft vX.Y.Z"`
4. Push commit + tag:
   - `git push origin main`
   - `git push origin vX.Y.Z`
