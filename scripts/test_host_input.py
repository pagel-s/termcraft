#!/usr/bin/env python3
"""One-terminal hosting, personal shortcuts and host settings in real PTYs."""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import termios
import time
from test_terminal_input import TerminalGame


def check(binary, root):
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        address = '127.0.0.1:' + str(listener.getsockname()[1])
    save = root / 'saves'
    host = TerminalGame(binary, save, root / 'host', observe=False,
                        command_override=[str(binary), 'host', address])
    guest = third = None
    try:
        guest = TerminalGame(binary, save, root / 'guest', address=address,
                             observe=False, start_server=False, profile='host-test-guest')
        duplicate = subprocess.run(
            [str(binary), 'client', address, '--player', 'host-test-guest'],
            env=guest.env, capture_output=True, text=True, timeout=6)
        assert duplicate.returncode != 0
        assert 'already connected' in duplicate.stderr and '--player charlie' in duplicate.stderr
        third = TerminalGame(binary, save, root / 'third', address=address,
                             observe=False, start_server=False, profile='host-test-third')
        third.send(b'tthird player here\r'); third.pump(0.15)
        host.pump(0.15); guest.pump(0.15)
        assert 'third player here' in host.screen.text()
        assert 'third player here' in guest.screen.text()
        host.send(b'o'); host.pump(0.15)
        assert 'PvP combat' in host.screen.text()
        host.send(b'\x1b')
        guest.send(b'\x1b[17~'); guest.pump(0.3)
        assert 'Overworld' in guest.screen.text().splitlines()[0], 'F6 was enabled by default'
        guest.send(b't/gamemode\r'); guest.pump(0.2)
        host.pump(0.2)
        assert 'shortcuts are ON' in guest.screen.text()
        host.send(b't'); host.pump(0.15)
        assert '/gamemode' not in host.screen.text(), 'private command was broadcast'
        host.send(b'\x1b')
        guest.send(b'o'); guest.pump(0.15)
        assert 'Travel/loadout' in guest.screen.text()
        assert 'PvP combat' not in guest.screen.text(), 'guest exposed host-only control'
        guest.send(b'\x1b'); guest.pump(0.15)
        guest.send(b'\x1b[17~')
        deadline=time.monotonic()+6
        while 'Nether' not in guest.screen.text().splitlines()[0] and time.monotonic()<deadline:
            guest.pump(0.05)
        assert 'Nether' in guest.screen.text().splitlines()[0]
        host.send(b'\x1b[17~'); host.pump(0.3)
        assert 'Overworld' in host.screen.text().splitlines()[0], 'guest enabled host shortcuts'
        guest.send(b't/gamemode off\r'); guest.pump(0.15)
        guest.send(b'\x1b[15~'); guest.pump(0.3)
        assert 'Nether' in guest.screen.text().splitlines()[0], 'disabled shortcuts still travelled'
        host.send(b'o'); host.pump(0.1)
        host.send(b'ssssss\r'); host.pump(0.2)
        assert 'PvP combat        : OFF' in host.screen.text()
        host.capture('host-settings')
        host.send(b'\x1b'); host.pump(0.1)
        host.finish()
        for player in (guest, third):
            deadline = time.monotonic() + 6
            while player.process.poll() is None and time.monotonic() < deadline:
                player.pump(0.05)
            player.pump(0.1)
            assert player.process.poll() is not None, 'guest stayed open after host stopped'
            assert 'Disconnected from' in player.raw.decode(errors='replace')
            assert b'\x1b[?1049l' in player.raw, 'disconnected guest did not restore its terminal'
            assert player.before == termios.tcgetattr(player.slave)
        world = save / 'multiplayer/default'
        assert not (world / '.session.lock').exists(), 'managed host left its server running'
        state = json.loads((world / 'server-state.json').read_text())
        assert state['settings']['pvp'] is False, 'PvP host setting was not saved'
        return dict(one_terminal_host=True, private_per_player_shortcuts=True,
                    guest_host_permissions=True, host_quit_saves_and_stops_server=True,
                    three_live_terminal_players=True, duplicate_profile_explained=True,
                    guests_restore_terminal_on_host_exit=True,
                    persisted_pvp_setting=True)
    finally:
        if third: third.close()
        if guest: guest.close()
        host.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='termcraft-host-') as directory:
        result=dict(status='passed', **check(args.binary.resolve(), Path(directory)))
    if args.report: args.report.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result))

if __name__ == '__main__': main()
