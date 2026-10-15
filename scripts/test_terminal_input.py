#!/usr/bin/env python3
"""Exercise real native/co-op terminal input in disposable local PTYs.

Requires Unix PTYs and Pillow (through capture_terminal_art). This controller
advertises keyboard protocol support but deliberately sends legacy taps first.
The optional compiled village_review example adds door picking/drag checks in
all three views. Director tools only observe the running game; they never edit
the player, blocks, game rules, or inventory.
"""

import argparse
import atexit
import errno
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import pty
import select
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import termios
import time

from capture_terminal_art import Screen, raster


class TerminalGame:
    def __init__(self, binary, save_root, artifacts, world_id=None, address=None):
        self.binary = str(binary)
        self.artifacts = artifacts
        artifacts.mkdir(parents=True, exist_ok=True)
        self.env = dict(os.environ, TERM="xterm-256color", TERMCRAFT_SAVE_DIR=str(save_root))
        self.env.pop("NO_COLOR", None)
        self.env.pop("TERMCRAFT_DIRECTOR", None)
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 48, 160, 0, 0))
        self.before = termios.tcgetattr(self.slave)
        self.screen = Screen(160, 48)
        self.raw = bytearray()
        self.queries = 0
        self.mcp = None
        self.seq = 0
        self.server = None
        self.process = None
        self.closed = False
        atexit.register(self.close)

        def controlling_terminal():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

        if address:
            self.server = subprocess.Popen(
                [self.binary, "world", "serve", world_id, address], env=self.env,
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            host, port = address.split(":")
            deadline = time.monotonic() + 6
            while True:
                try:
                    with socket.create_connection((host, int(port)), timeout=0.1):
                        break
                except OSError:
                    assert time.monotonic() < deadline, "local co-op server did not start"
                    time.sleep(0.02)
            command = [self.binary, "client", address, "--player", "terminal-input-test"]
        else:
            command = [self.binary]
            if world_id:
                command += ["world", "play", world_id]
        self.process = subprocess.Popen(
            command, env=self.env, stdin=self.slave, stdout=self.slave, stderr=self.slave,
            preexec_fn=controlling_terminal,
        )
        self.pump(0.4)
        if not address:
            self.send(b" ")
            self.pump(0.25)
        assert self.queries, "terminal keyboard capability query missing"
        self.send(b"t")
        self.pump(0.1)
        self.send(b"/connect\r")
        self.pump(0.3)
        self.mcp = subprocess.Popen(
            [self.binary, "mcp"], env=self.env, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        )
        self.call("director_connect", {"world_id": world_id} if world_id else {})

    def send(self, data):
        os.write(self.master, data)

    def pump(self, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if not select.select([self.master], [], [], 0.005)[0]:
                continue
            try:
                data = os.read(self.master, 65536)
            except OSError as error:
                if error.errno == errno.EIO and self.process.poll() is not None:
                    return
                raise
            if not data:
                return
            self.raw.extend(data)
            self.screen.feed(data)
            # A positive capability response is not evidence of working key-up.
            count = self.raw.count(b"\x1b[?u")
            while self.queries < count:
                self.send(b"\x1b[?0u\x1b[?1;2c")
                self.queries += 1

    def call(self, name, arguments):
        self.seq += 1
        self.mcp.stdin.write(json.dumps({
            "jsonrpc": "2.0", "id": self.seq, "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        }) + "\n")
        self.mcp.stdin.flush()
        deadline = time.monotonic() + 6
        while not select.select([self.mcp.stdout], [], [], 0)[0]:
            assert time.monotonic() < deadline, f"read-only MCP call timed out: {name}"
            self.pump(0.01)
        response = json.loads(self.mcp.stdout.readline())["result"]
        assert not response.get("isError"), response
        return response["structuredContent"]

    def player(self):
        return self.call("director_observe", {})["context"]["player"]

    def x(self):
        return self.player()["x"]

    def capture(self, name):
        raster(self.screen).save(self.artifacts / (name + ".png"))

    def finish(self):
        self.send(b"q")
        self.pump(0.4)
        self.process.wait(timeout=6)
        assert self.process.returncode == 0, self.process.returncode
        assert b"\x1b[?1049l" in self.raw, "alternate screen was not restored"
        assert self.before == termios.tcgetattr(self.slave), "terminal input mode was not restored"
        return {
            "terminal_restored": True,
            "keyboard_queries": self.queries,
            "bytes": len(self.raw),
            "synchronized_frames_started": self.raw.count(b"\x1b[?2026h"),
            "synchronized_frames_finished": self.raw.count(b"\x1b[?2026l"),
        }

    def close(self):
        if self.closed:
            return
        self.closed = True
        if self.process is not None and self.process.poll() is None:
            self.process.kill()
            self.process.wait()
        if self.mcp is not None:
            self.mcp.kill()
            self.mcp.wait()
        if self.server is not None:
            self.server.send_signal(signal.SIGINT)
            self.server.wait(timeout=8)
        os.close(self.master)
        os.close(self.slave)


def make_flat_world(binary, root):
    env = dict(os.environ, TERMCRAFT_SAVE_DIR=str(root))
    recipe = json.loads(subprocess.check_output([str(binary), "world", "schema"]))["example"]
    recipe.update(terrain="plains", relief=0, regions=[], landmarks=[])
    recipe_file = root.parent / "input-recipe.json"
    recipe_file.write_text(json.dumps(recipe))
    subprocess.run(
        [str(binary), "world", "create", "terminal-input", str(recipe_file)],
        env=env, check=True, capture_output=True,
    )


def check_input(binary, root, mode):
    save_root = root / mode / "saves"
    save_root.parent.mkdir(parents=True)
    make_flat_world(binary, save_root)
    address = None
    if mode == "coop":
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            address = "127.0.0.1:" + str(listener.getsockname()[1])
    game = TerminalGame(binary, save_root, root / mode / "captures", "terminal-input", address)
    try:
        start = game.x()
        game.send(b"d")
        game.pump(0.3)
        tap_end = game.x()
        game.pump(0.5)
        stopped = game.x()
        assert 0.15 < tap_end - start < 1.8, (mode, "tap duration", start, tap_end)
        assert abs(stopped - tap_end) < 0.3, (mode, "ASCII tap kept running", tap_end, stopped)
        # An inventory key release must not enable indefinite movement holds.
        game.send(b"\x1b[101;1:3u")
        game.send(b"a")
        game.pump(0.3)
        unrelated_end = game.x()
        game.pump(0.4)
        unrelated_stopped = game.x()
        assert abs(unrelated_stopped - unrelated_end) < 0.3, (mode, "unrelated release promoted input")
        game.send(b"\x1b[13~")
        game.pump(0.1)
        assert "keys:repeat" in game.screen.text(), "capability claim was treated as key-up proof"
        game.capture("legacy-tap-stopped")
        game.send(b"\x1b[13~")
        # Legacy OS repeat shifts to Space. Preserve established movement
        # through takeoff, but never renew it forever from Space alone.
        legacy_start = game.player()
        for _ in range(3):
            game.send(b"d")
            game.pump(0.04)
        game.send(b" ")
        game.pump(0.25)
        legacy_air = game.player()
        assert legacy_air["x"] - legacy_start["x"] > 1.3, (mode, "legacy jump braked direction", legacy_start, legacy_air)
        assert legacy_air["y"] < legacy_start["y"] - 0.2, (mode, "legacy jump did not take off")
        for _ in range(5):
            game.send(b" ")
            game.pump(0.2)
        # Repeated jump requests can leave ordinary airborne momentum after
        # direction has stopped. End the jump input and allow a normal landing.
        game.pump(1.0)
        legacy_end = game.x()
        legacy_landed = game.player()
        game.pump(0.3)
        assert abs(game.x() - legacy_end) < 0.3, (mode, "legacy movement did not stop after landing", legacy_end, game.player())
        assert abs(legacy_landed["y"] - legacy_start["y"]) < 0.05, (mode, "legacy jump never landed")
        # Prime only after checking the first tap: this is actual control key-up.
        game.send(b"\x1b[100;1:3u")
        game.send(b"\x1b[100;1:1u")
        held_start = game.x()
        game.pump(0.6)
        held_end = game.x()
        assert held_end - held_start > 2.5, (mode, "confirmed hold was interrupted")
        game.send(b"\x1b[100;1:3u")
        game.pump(0.1)
        released = game.x()
        game.pump(0.3)
        release_stopped = game.x()
        assert release_stopped - released < 0.35, (mode, "key-up did not stop movement")
        # OS repeat can stay on Space, or cease entirely after Space-up,
        # while D is still physically held. Neither case may expire D.
        game.send(b"\x1b[100;1:1u")
        game.pump(0.15)
        chord_start = game.player()
        game.send(b"\x1b[32;1:1u")
        game.pump(0.2)
        chord_air = game.player()
        assert chord_air["y"] < chord_start["y"] - 0.2, (mode, "confirmed chord did not jump")
        for _ in range(5):
            game.send(b"\x1b[32;1:2u")
            game.pump(0.45)
        chord_end = game.x()
        assert chord_end - chord_start["x"] > 11, (mode, "Space repeat cancelled running")
        game.send(b"\x1b[32;1:3u")
        game.pump(0.7)
        after_jump = game.x()
        assert after_jump - chord_end > 2.5, (mode, "jump release cancelled running")
        game.send(b"\x1b[100;1:3u")
        game.pump(0.1)
        chord_released = game.x()
        game.pump(0.3)
        assert abs(game.x() - chord_released) < 0.35, (mode, "direction release did not stop chord")
        # Repeat the opposite direction with no repeat events at all.
        game.send(b"\x1b[97;1:1u")
        game.pump(0.15)
        left_start = game.player()
        game.send(b"\x1b[32;1:1u")
        game.pump(0.2)
        assert game.player()["y"] < left_start["y"] - 0.2, (mode, "leftward chord did not jump")
        game.send(b"\x1b[32;1:3u")
        game.pump(2.3)
        left_end = game.x()
        assert left_start["x"] - left_end > 12, (mode, "quiet left hold expired after jump")
        game.send(b"\x1b[97;1:3u")
        game.pump(0.2)
        # Focus loss must still clear a genuine held key immediately.
        game.send(b"\x1b[100;1:1u")
        game.pump(0.15)
        game.send(b"\x1b[O")
        game.pump(0.1)
        focus_end = game.x()
        game.pump(0.3)
        assert abs(game.x() - focus_end) < 0.35, (mode, "focus loss left movement held")
        game.capture("confirmed-controls")
        return {
            "mode": mode, "status": "passed", "capability_positive_ascii_tap_delta": tap_end - start,
            "tap_rest_drift": stopped - tap_end, "unrelated_release_rest_drift": unrelated_stopped - unrelated_end,
            "confirmed_hold_delta": held_end - held_start, "key_up_drift": release_stopped - released,
            "legacy_run_jump_delta": legacy_air["x"] - legacy_start["x"],
            "run_jump_repeat_delta": chord_end - chord_start["x"],
            "run_after_jump_release_delta": after_jump - chord_end,
            "quiet_left_run_jump_delta": left_start["x"] - left_end,
            "focus_loss_cleared": True,
            **game.finish(),
        }
    finally:
        game.close()


def check_village_doors(binary, example, root):
    fixture = root / "village-fixture"
    env = dict(os.environ)
    env.pop("NO_COLOR", None)
    subprocess.run([str(example), str(fixture)], env=env, check=True, capture_output=True)
    metadata = json.loads((fixture / "village.json").read_text())
    original = Path(metadata["review_root"])
    results = []
    try:
        for view_index, (name, scale) in enumerate([("overview", 2), ("wide", 1), ("close", 4)]):
            save_root = root / ("door-" + name) / "saves"
            shutil.copytree(original, save_root)
            game = TerminalGame(binary, save_root, root / ("door-" + name) / "captures")
            try:
                for _ in range(view_index):
                    game.send(b"\x1bOS")  # F4 cycles overview -> wide -> close.
                    game.pump(0.1)
                door = metadata["buildings"][0]["left"]
                ground = metadata["ground_y"]
                game.send(b"\x1b[100;1:3u")
                game.send(b"\x1b[100;1:1u")
                deadline = time.monotonic() + 3
                while game.x() < door - 1.5:
                    assert time.monotonic() < deadline, (name, "door approach stalled")
                    game.pump(0.03)
                game.send(b"\x1b[100;1:3u")
                game.pump(0.3)
                player = game.player()
                assert abs(player["y"] - ground) < 0.01, (name, "door approach left road")
                # Grounded player/camera, exact physical viewport geometry.
                center_x = (160 // 2 // scale) * scale
                center_y = math.ceil((48 - 4) * 3 / 4 / scale) * scale if scale > 1 else 48 * 2 // 3
                column = round((door + 0.5) * 2 * scale - round(player["x"] * 2 * scale) + center_x)
                row = center_y - scale + scale // 2
                game.send(f"\x1b[<2;{column + 1};{row + 1}M".encode())
                game.pump(0.1)
                # Tiny pointer motion while pressed must not re-close the door.
                for offset in [1, 0, -1]:
                    game.send(f"\x1b[<34;{column + offset + 1};{row + 1}M".encode())
                    game.pump(0.04)
                game.send(f"\x1b[<2;{column + 1};{row + 1}m".encode())
                game.capture("door-after-drag")
                game.send(b"\x1b[100;1:1u")
                game.pump(0.85)
                game.send(b"\x1b[100;1:3u")
                game.pump(0.2)
                end = game.player()
                assert end["x"] > door + 2, (name, "mouse use/drag left door closed", player, end)
                assert abs(end["y"] - ground) < 0.01, (name, "walking through door changed feet height")
                game.capture("walked-through-door")
                results.append({"view": name, "scale": scale, "door_x": door,
                                "mouse_cell": [column, row], "start": player, "end": end,
                                "right_drag_kept_door_open": True, **game.finish()})
            finally:
                game.close()
    finally:
        # The example created this fresh private review save for this test.
        shutil.rmtree(original)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("termcraft"))
    parser.add_argument("--mode", choices=["native", "coop", "both"], default="both")
    parser.add_argument("--village-example", type=Path, help="Compiled examples/village_review for all-view door checks")
    parser.add_argument("--report", type=Path)
    parser.add_argument("--keep-artifacts", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = Path(tempfile.mkdtemp(prefix="termcraft-terminal-input-"))
    try:
        modes = ["native", "coop"] if args.mode == "both" else [args.mode]
        report = {
            "status": "passed", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "input": [check_input(binary, root, mode) for mode in modes],
            "doors": check_village_doors(binary, args.village_example.resolve(), root) if args.village_example else [],
            "limits": "Disposable Unix PTYs with synthetic capability/keypress/mouse streams and read-only director observations. Excludes emulator presentation latency, WANs, Windows native input, and human feel/art approval.",
        }
        if args.keep_artifacts:
            report["artifacts"] = str(root)
        if args.report:
            args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report))
    finally:
        if not args.keep_artifacts:
            shutil.rmtree(root)


if __name__ == "__main__":
    main()
