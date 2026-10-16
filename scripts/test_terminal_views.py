#!/usr/bin/env python3
"""Live native/co-op zoom, resize and movement timings in disposable PTYs.

Uses real input and renderer output. MCP only observes the active player.
Requires Unix PTYs and Pillow; does not measure a terminal emulator's GPU work.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import socket
import tempfile

from test_terminal_input import TerminalGame, make_flat_world


def check(binary, root, mode):
    save = root / mode / 'saves'
    save.parent.mkdir(parents=True)
    make_flat_world(binary, save)
    address = None
    if mode == 'coop':
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            address = '127.0.0.1:' + str(listener.getsockname()[1])
    game = TerminalGame(binary, save, root / mode / 'captures', 'terminal-input', address)
    samples = []
    try:
        assert 'Overview' in game.screen.text().splitlines()[0]
        game.send(b'=')
        game.pump(0.2)
        assert 'Detail' in game.screen.text().splitlines()[0]
        for columns, rows in [(160, 48), (120, 40), (80, 24), (160, 48)]:
            game.resize(columns, rows)
            assert 'Detail' in game.screen.text().splitlines()[0], (mode, columns, rows)
            game.capture(f'detail-{columns}x{rows}')
        # Relative controls stop at their limits; F4 keeps its existing cycle.
        for key, name in [(b'=', 'Detail'), (b'-', 'Overview'), (b'-', 'Wide'),
                          (b'-', 'Wide'), (b'=', 'Overview'), (b'\x1bOS', 'Wide'),
                          (b'\x1bOS', 'Detail')]:
            game.send(key)
            game.pump(0.12)
            assert name in game.screen.text().splitlines()[0], (mode, key, name)
        game.send(b'\x1b[100;1:3u')  # Establish actual movement release reporting.
        for key, name in [(b'-', 'overview'), (b'-', 'wide'), (b'=', 'overview'), (b'=', 'detail')]:
            game.send(key)
            game.pump(0.1)
            game.send(b'\x1b[100;1:1u')
            game.pump(1.1)
            game.send(b'\x1b[100;1:3u')
            game.pump(0.12)
            game.send(b'\x1b[13~')  # F3 shows the timings collected during movement.
            game.pump(0.1)
            text = game.screen.text()
            match = re.search(r'p95 tick ([\d.]+)ms render ([\d.]+)ms save/cache ([\d.]+)ms', text)
            assert match and 'keys:release' in text, (mode, name, text)
            assert float(match[2]) > 0, (mode, name, 'render timing was never recorded')
            samples.append(dict(view=name, tick_p95_ms=float(match[1]),
                                render_p95_ms=float(match[2]), save_cache_p95_ms=float(match[3])))
            game.capture(name + '-moving-diagnostics')
            game.send(b'\x1b[13~')
            game.pump(0.1)
        return dict(mode=mode, detail_survived_resize=True, zoom_keys_and_f4=True,
                    moving_timings=samples, captures=str(game.artifacts), **game.finish())
    finally:
        game.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('termcraft'))
    parser.add_argument('--report', type=Path)
    parser.add_argument('--keep-artifacts', action='store_true')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='termcraft-view-input-'))
    try:
        binary = args.binary.resolve()
        report = dict(status='passed', binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                      checks=[check(binary, root, mode) for mode in ['native', 'coop']],
                      limits='Live flat-world PTYs, actual input/resize and renderer output. Excludes Warp/GPU presentation, WAN latency, busy scenes and human feel.')
        if args.keep_artifacts:
            report['artifacts'] = str(root)
        if args.report:
            args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report))
    finally:
        if not args.keep_artifacts:
            shutil.rmtree(root)


if __name__ == '__main__':
    main()
