#!/usr/bin/env python3
"""Real solo/co-op PTY bow, missed swing and eating checks in disposable saves.

The compiled interaction_review example stages only the co-op inventory offline;
all actions use native keyboard/mouse input. MCP only observes the running game.
Requires Unix PTYs and Pillow, like test_terminal_input.py.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
from PIL import ImageChops
from capture_terminal_art import raster
from test_terminal_input import TerminalGame


def check(binary, fixture, root, mode):
    save = root / mode / 'saves'
    save.parent.mkdir(parents=True)
    address = None
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        fixture_address = '127.0.0.1:' + str(listener.getsockname()[1])
    # Both modes start hungry. Eating while visibly full should spend no food.
    subprocess.run([str(fixture), '--pty-fixture', str(save), fixture_address], check=True)
    if mode == 'coop':
        address = fixture_address
    game = TerminalGame(binary, save, root / mode / 'captures', 'terminal-input', address)
    try:
        if mode == 'native':
            game.send(b'\x1b[20~')  # F9, confined to this disposable solo save.
            game.pump(0.2)
        arrow_counts = []
        game.send(b'-')  # Coordinate fixtures start in Overview, independent of the live default.
        game.pump(0.15)
        for view in ['overview', 'wide', 'detail']:
            game.send(b'2')
            game.pump(0.2)
            game.capture(view + '-bow-idle')
            before = raster(game.screen)
            game.send(b'\x1b[<0;92;31M')
            game.pump(0.9)
            game.capture(view + '-bow-draw')
            drawn = raster(game.screen)
            assert 'Bow' in game.screen.text(), (mode, view, 'bow not selected')
            crop = (68 * 9, 23 * 18, 102 * 9, 38 * 18)
            assert ImageChops.difference(before.crop(crop), drawn.crop(crop)).getbbox(), (mode, view, 'bow pose unchanged')
            game.send(b'\x1b[<0;92;31m')
            game.pump(0.1)
            game.capture(view + '-bow-release')
            game.send(b'3')
            game.pump(0.2)
            count = re.search(r'Held: Arrow x(\d+)', game.screen.text())
            assert count, (mode, view, 'arrow count missing')
            arrow_counts.append(int(count.group(1)))
            assert arrow_counts[-1] == 63 - len(arrow_counts) + 1, (mode, view, arrow_counts)
            game.send(b'1')
            game.pump(0.2)
            game.send(b'\x1b[<0;92;31M')
            game.pump(0.18)
            game.capture(view + '-missed-sword')
            game.send(b'\x1b[<0;92;31m')
            game.pump(0.4)
            game.send(b'\x1bOS')
            game.pump(0.15)
        game.send(b'4')
        game.pump(0.15)
        game.send(b'\x1b[<2;5;5M')  # Distant terrain must not prevent self-use.
        game.pump(0.15)
        game.send(b'\x1b[<2;5;5m')
        game.capture('eating-away-from-terrain')
        assert 'Held: Cooked Beef x31' in game.screen.text(), (mode, 'food was not consumed')
        assert re.search(r'Food\s+##########', game.screen.text()), (mode, 'food bar lost its final half immediately after eating')
        return dict(mode=mode, views=['overview', 'wide', 'detail'], bow_pose_changed=True,
                    arrows_after_shots=arrow_counts, distant_food_use=True,
                    captures=str(game.artifacts), **game.finish())
    finally:
        game.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('termcraft'))
    parser.add_argument('--fixture', type=Path, required=True, help='Compiled examples/interaction_review')
    parser.add_argument('--report', type=Path)
    parser.add_argument('--keep-artifacts', action='store_true')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='termcraft-action-input-'))
    try:
        binary = args.binary.resolve()
        report = dict(status='passed', binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                      checks=[check(binary, args.fixture.resolve(), root, mode) for mode in ['native', 'coop']],
                      limits='Real PTY actions and screen captures; pose change, ammunition and food counts checked. Does not measure emulator latency or establish art acceptance.')
        if args.report:
            args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report))
    finally:
        if not args.keep_artifacts:
            shutil.rmtree(root)


if __name__ == '__main__':
    main()
