#!/usr/bin/env bash
# Unix qualification checks. Socket tests own temporary worlds and fail on errors.
set -euo pipefail

echo "[1/6] Formatting"
cargo fmt --all -- --check

echo "[2/6] Unit tests"
cargo test --locked

echo "[3/6] Strict all-target Clippy"
cargo clippy --all-targets --locked -- -D warnings

echo "[4/6] Optimized executable"
cargo build --release --locked

echo "[5/6] Real loopback co-op and live MCP regressions"
cargo test --locked --test multiplayer_tcp -- --ignored

echo "[6/6] Normal-rule Lantern Run playthrough"
python3 scripts/test_lantern_trial.py --binary "${CARGO_TARGET_DIR:-target}/release/termcraft"

echo "Local release smoke checks passed. The optional encrypted SSH fixture, native platforms, client/model quality and human play require separate checks."
