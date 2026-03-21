#!/usr/bin/env bash
set -euo pipefail

echo "[1/5] cargo fmt --all --check"
cargo fmt --all --check

echo "[2/5] cargo check"
cargo check

echo "[3/5] cargo test -q"
cargo test -q

echo "[4/5] cargo clippy -q"
cargo clippy -q

echo "[5/5] headless server startup smoke"
log_file="$(mktemp)"
set +e
timeout 2s cargo run --quiet -- server 127.0.0.1:25568 >"$log_file" 2>&1
status=$?
set -e

if [[ $status -ne 0 && $status -ne 124 ]]; then
    if rg -q "Operation not permitted" "$log_file"; then
        rm -f "$log_file"
        echo "release smoke: server startup skipped due sandbox permission limits"
        echo "release smoke checks passed (server startup skipped in sandbox)"
        exit 0
    fi
    cat "$log_file" >&2
    rm -f "$log_file"
    echo "release smoke: server startup failed with exit code $status" >&2
    exit $status
fi

if ! rg -q "Starting headless server on 127.0.0.1:25568" "$log_file"; then
    cat "$log_file" >&2
    rm -f "$log_file"
    echo "release smoke: startup banner missing" >&2
    exit 1
fi

rm -f "$log_file"
echo "release smoke checks passed"
