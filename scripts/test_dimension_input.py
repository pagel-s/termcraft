#!/usr/bin/env python3
"""Visit generated Nether/End with real solo and co-op terminal controls.

Disposable saves only. MCP observes players; movement/travel use keyboard input.
Software PTY captures do not measure Warp's presentation or qualify a boss fight.
"""
import argparse
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import tempfile

from test_terminal_input import TerminalGame, make_flat_world


def check(binary, fixture, root, mode, start_dimension=None):
    if start_dimension:
        root = root / start_dimension.lower()
    save = root / mode / 'saves'
    save.parent.mkdir(parents=True)
    address = None
    if mode == 'coop':
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            address = '127.0.0.1:' + str(listener.getsockname()[1])
        subprocess.run([str(fixture), '--pty-fixture', str(save), address, start_dimension], check=True)
    else:
        make_flat_world(binary, save)
    game = TerminalGame(binary, save, root / mode / 'captures', 'terminal-input', address)
    visits = []
    try:
        if mode == 'native':
            game.send(b'\x1b[20~')  # F9: ordinary debug loadout.
            game.pump(0.15)
        destinations = [(None, start_dimension)] if start_dimension else [(b'\x1b[17~', 'Nether'), (b'\x1b[18~', 'End')]
        for key, dimension in destinations:
            if key:
                game.send(key)
            game.pump(0.7)
            assert dimension in game.screen.text().splitlines()[0], (mode, dimension, game.screen.text().splitlines()[0])
            start = game.player()
            game.send(b'\x1b[100;1:1u')
            game.pump(0.05)
            game.send(b'\x1b[100;1:3u')
            game.pump(0.1)
            game.send(b'\x1b[100;1:1u')
            game.pump(0.2)
            game.send(b'\x1b[32;1:1u')
            game.pump(0.1)
            game.send(b'\x1b[32;1:3u')
            game.pump(0.3)
            game.send(b'\x1b[100;1:3u')
            game.pump(0.2)
            end = game.player()
            assert end['x'] > start['x'] + 0.2, (mode, dimension, start, end)
            game.capture(dimension.lower())
            visits.append(dict(dimension=dimension, start=start, end=end))
        return dict(mode=mode, visits=visits, terminal=game.finish())
    finally:
        game.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('termcraft'))
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='termcraft-dimension-input-'))
    binary = args.binary.resolve()
    fixture = args.fixture.resolve()
    checks = [check(binary, fixture, root, 'native')]
    checks += [check(binary, fixture, root, 'coop', dimension) for dimension in ['Nether', 'End']]
    report = dict(status='passed', binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  artifacts=str(root), checks=checks,
                  limits=__doc__)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
