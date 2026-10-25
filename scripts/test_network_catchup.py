#!/usr/bin/env python3
"""Paused terminal output and mouse-event bursts must not leave co-op behind.

Uses two real clients, a disposable server and Unix PTYs; requires Pillow.
This checks application catch-up, not physical Wi-Fi or a specific terminal app.
"""
import argparse
import json
import os
from pathlib import Path
import re
import socket
import tempfile
import termios
import threading
import time

from test_terminal_input import TerminalGame, make_flat_world


def values(game):
    text = game.screen.text()
    tick = re.search(r"server tick (\d+)", text)
    skipped = re.search(r"skipped (\d+)", text)
    assert tick and skipped, text
    return int(tick.group(1)), int(skipped.group(1))


def check(binary, root):
    save = root / "saves"
    make_flat_world(binary, save)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        address = "127.0.0.1:" + str(listener.getsockname()[1])
    reference = TerminalGame(binary, save, root / "reference", "terminal-input", address,
                             observe=False, profile="catchup-reference")
    guest = None
    stop = threading.Event()
    flood = None
    mouse_bytes_written = 0
    mouse_partial_writes = 0
    try:
        guest = TerminalGame(binary, save, root / "guest", "terminal-input", address,
                             observe=False, start_server=False, profile="catchup-guest")
        for game in (reference, guest):
            game.send(b"\x1bOR")
            game.pump(0.2)
        before, _ = values(guest)
        # Stop reading one PTY so terminal output blocks its UI thread. The
        # background reader must keep the newest state without a FIFO backlog.
        termios.tcflow(guest.slave, termios.TCOOFF)
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            reference.pump(0.05)
        resumed = time.monotonic()
        termios.tcflow(guest.slave, termios.TCOON)
        guest.pump(0.4)
        reference.pump(0.05)
        after, skipped = values(guest)
        current, _ = values(reference)
        assert after - before >= 90, (before, after)
        assert skipped >= 60, skipped
        assert current - after <= 10, (current, after)
        catchup_seconds = time.monotonic() - resumed
        # High-rate mouse motion must not monopolize event handling, prediction
        # rebuilding, command writes or incoming authoritative updates.
        os.set_blocking(guest.master, False)
        packet = (b"\x1b[<35;60;20M\x1b[<35;70;20M") * 16
        def flood_mouse():
            nonlocal mouse_bytes_written, mouse_partial_writes
            pending = b""
            # PTY writes can be partial. Preserve every suffix and finish the
            # last packet on stop, or a truncated mouse escape can consume the
            # next real key (including Q) instead of delivering a key event.
            while pending or not stop.is_set():
                if not pending:
                    pending = packet
                try:
                    written = os.write(guest.master, pending)
                    mouse_bytes_written += written
                    mouse_partial_writes += written < len(pending)
                    pending = pending[written:]
                except BlockingIOError:
                    pass
                except OSError:
                    break
                time.sleep(0.001)
        flood = threading.Thread(target=flood_mouse, daemon=True)
        flood.start()
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            guest.pump(0.03)
            reference.pump(0.03)
            assert guest.process.poll() is None
        stop.set()
        flood.join(timeout=2)
        assert not flood.is_alive()
        assert mouse_bytes_written >= 100_000, mouse_bytes_written
        assert mouse_bytes_written % len(packet) == 0, mouse_bytes_written
        guest.pump(0.3)
        reference.pump(0.05)
        final, _ = values(guest)
        current, _ = values(reference)
        assert final - after >= 45, (after, final)
        assert current - final <= 10, (current, final)
        os.set_blocking(guest.master, True)
        quitting = time.monotonic()
        guest.finish()
        guest_quit_seconds = time.monotonic() - quitting
        reference.finish()
        return dict(status="passed", blocked_terminal_seconds=6,
                    recovery_seconds=catchup_seconds, superseded_snapshots=skipped,
                    guest_tick_advance=after-before, continuous_mouse_seconds=3,
                    mouse_bytes_written=mouse_bytes_written,
                    mouse_partial_writes=mouse_partial_writes,
                    guest_quit_seconds=guest_quit_seconds,
                    final_tick_gap=current-final,
                    both_clients_saved_and_restored=True,
                    limits="Local TCP and PTYs; does not reproduce radio packet loss or Ghostty itself.")
    finally:
        stop.set()
        if flood: flood.join(timeout=2)
        if guest:
            termios.tcflow(guest.slave, termios.TCOON)
            guest.close()
        reference.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="termcraft-catchup-") as directory:
        result = check(args.binary.resolve(), Path(directory))
    if args.report: args.report.write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
