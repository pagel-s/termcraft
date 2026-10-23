#!/usr/bin/env bash
# Unix qualification checks. Socket tests own temporary worlds and fail on errors.
set -euo pipefail

echo "[1/7] Formatting"
cargo fmt --all -- --check

echo "[2/7] Unit tests"
cargo test --locked

echo "[3/7] Strict all-target Clippy"
cargo clippy --all-targets --locked -- -D warnings

echo "[4/7] Optimized executable"
cargo build --release --locked

echo "[5/7] Real loopback co-op and live MCP regressions"
cargo test --locked --test multiplayer_tcp -- --ignored

echo "[6/7] Normal-rule Lantern Run playthrough"
python3 scripts/test_lantern_trial.py --binary "${CARGO_TARGET_DIR:-target}/release/termcraft"

echo "[7/7] Linux checkpoint storage-fault checks"
if [[ "${OSTYPE:-}" == linux* ]]; then
    cargo build --release --locked --example storage_review
    python3 scripts/test_storage_recovery.py --binary "${CARGO_TARGET_DIR:-target}/release/termcraft" \
        --fixture "${CARGO_TARGET_DIR:-target}/release/examples/storage_review"
else
    echo "Skipped Linux LD_PRELOAD faults on this platform."
fi

echo "Local release smoke checks passed. Native platforms, remote SSH and human gameplay require separate checks."
