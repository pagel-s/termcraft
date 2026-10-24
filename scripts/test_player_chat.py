#!/usr/bin/env python3
"""Exercise normal solo/co-op chat through real terminal input, without opt-ins.

Uses two actual clients and a disposable server. Requires Unix PTYs and Pillow.
"""
import argparse
import json
from pathlib import Path
import signal
import socket
import tempfile

from test_terminal_input import TerminalGame, make_flat_world


def check(binary, root):
    save = root / "saves"
    make_flat_world(binary, save)
    solo = TerminalGame(binary, save, root / "solo", "terminal-input", observe=False)
    try:
        assert "T: chat" not in solo.screen.text()
        solo.send(b"t")
        solo.pump(0.2)
        assert "GAME MASTER" not in solo.screen.text()
        assert "PLAYER CHAT" not in solo.screen.text()
        assert "COMMANDS" in solo.screen.text()
        solo.send(b"\x1b")
        solo.pump(0.15)
        solo.finish()
    finally:
        solo.close()

    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        address = "127.0.0.1:" + str(listener.getsockname()[1])
    host = TerminalGame(binary, save, root / "host", "terminal-input", address,
                        observe=False, profile="chat-host")
    guest = None
    try:
        guest = TerminalGame(binary, save, root / "guest", "terminal-input", address,
                             observe=False, start_server=False, profile="chat-guest")
        host.pump(0.3)
        assert "T: chat" in host.screen.text()
        # Confirm key-up support, then hold right without releasing it. Opening
        # chat must stop the hold; typing movement keys must stay in the draft.
        host.send(b"\x1b[100;1:3ud")
        host.pump(0.15)
        host.send(b"t")
        host.pump(0.15)
        assert "PLAYER CHAT | You are P" in host.screen.text()
        host.send(b"ad space is chat\r")
        host.pump(0.3)
        guest.pump(0.3)
        assert "ad space is chat" in guest.screen.text()
        assert "PLAYER CHAT" not in host.screen.text()
        guest.send(b"tHello from the guest\r")
        guest.pump(0.2)
        host.pump(0.3)
        assert "Hello from the guest" in host.screen.text()
        # Draft survives Esc; Ctrl+C discards it without sending a command.
        host.send(b"tkeep this draft\x1b")
        host.pump(0.2)
        host.send(b"t")
        host.pump(0.2)
        assert "> keep this draft_" in host.screen.text()
        host.send(b"\x03")
        host.pump(0.2)
        host.send(b"t")
        host.pump(0.2)
        assert "keep this draft" not in host.screen.text()
        assert "PgUp/PgDn" in host.screen.text()
        assert all(hidden not in host.screen.text() for hidden in ["LLM", "/connect", "GAME MASTER"])
        host.capture("player-chat")
        host.send(b"\x1b")
        host.pump(0.15)
        host.finish()
        guest.finish()
        host.server.send_signal(signal.SIGINT)
        assert host.server.wait(timeout=8) == 0
        host.server = None
        world = save / "worlds" / "world-terminal-input"
        saved = json.loads((world / "server-state.json").read_text())
        positions = [session["player"]["x"] for session in saved["players"].values()]
        assert len(positions) == 2, positions
        assert max(positions) - min(positions) < 3, ("typing continued movement", positions)
        assert not (world / ".director-bridge.json").exists()
        return {"default_solo_has_no_chat": True, "two_real_clients_exchange_messages": True,
                "opening_chat_stops_held_movement": True, "draft_close_and_cancel": True,
                "normal_chat_has_no_experimental_controls": True, "saved_player_x": positions}
    finally:
        if guest:
            guest.close()
        host.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="termcraft-player-chat-") as directory:
        report = dict(status="passed", **check(args.binary.resolve(), Path(directory)))
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
