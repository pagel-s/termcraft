#!/usr/bin/env bash
# Unix qualification checks. Socket tests own temporary worlds and fail on errors.
set -euo pipefail

echo "[1/5] Formatting"
cargo fmt --all -- --check

echo "[2/5] Unit tests"
cargo test --locked

echo "[3/5] Strict all-target Clippy"
cargo clippy --all-targets --locked -- -D warnings

echo "[4/5] Optimized executable"
cargo build --release --locked

echo "[5/5] Real loopback co-op and live MCP regressions"
cargo test --locked --test multiplayer_tcp -- --ignored

echo "Local release smoke checks passed. The optional encrypted SSH fixture, native platforms, client/model quality and human play require separate checks."
