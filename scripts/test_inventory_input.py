#!/usr/bin/env python3
"""Native/co-op inventory and crafting via actual PTY mouse/keyboard input.

Offline fixtures stage supplies. Actions use game input; saves are inspected only
after their owners exit. Requires Unix PTYs and Pillow.
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
from test_terminal_input import TerminalGame
from test_storage_recovery import Mcp


def check(binary, fixture, root, mode):
    saves = root / mode / 'saves'
    saves.parent.mkdir(parents=True)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        address = '127.0.0.1:' + str(listener.getsockname()[1])
    subprocess.run([str(fixture), '--inventory-fixture', str(saves), address], check=True)
    game = TerminalGame(binary, saves, root / mode / 'captures', 'terminal-input',
                        address if mode == 'coop' else None)
    def click(x, y, button=0):
        game.send(f'\x1b[<{button};{x+1};{y+1}M'.encode())
        game.pump(0.12)
        game.send(f'\x1b[<{button};{x+1};{y+1}m'.encode())
        game.pump(0.12)
    try:
        game.send(b'e')
        game.pump(0.2)
        assert 'RECIPE BOOK' in game.screen.text()
        # 160x48: separate recipe panel and bag, main storage above the hotbar.
        click(83, 20)  # Cobblestone x20.
        click(77, 20)  # Merge into x60, preserving a selected remainder x16.
        assert 'Cobblestone x64' in game.screen.text(), game.screen.text()
        click(89, 15)  # Move the remainder into an empty main-storage slot.
        click(35, 16)  # One wood -> four planks.
        click(35, 16, 4)  # Shift-click: craft the remaining wood, output fits.
        game.capture('recipes-and-merged-stacks')
        # Manual crafting: select planks, right-click one in each vertical cell.
        click(77, 15)
        game.send(b'\x1b[<2;100;24M')
        game.pump(0.12)
        game.send(b'\x1b[<34;100;26M')  # Actual right drag into the second cell.
        game.pump(0.12)
        game.send(b'\x1b[<2;100;26m')
        game.pump(0.12)
        click(117, 25)  # Take four sticks.
        game.capture('manual-stick-craft')
        game.send(b'\x1b[<65;35;17M')
        game.pump(0.1)
        assert '2-9 /' in game.screen.text()
        game.send(b'\x1b[<64;35;17M')
        game.pump(0.1)
        game.resize(80, 24)
        assert 'RECIPE BOOK' in game.screen.text()
        game.send(b'\t')
        game.pump(0.15)
        assert 'INVENTORY & CRAFTING' in game.screen.text()
        game.capture('compact-bag')
        game.send(b'\t')
        game.pump(0.15)
        assert 'RECIPE BOOK' in game.screen.text()
        game.capture('compact-recipes')
        game.resize(160, 48)
        game.send(b'e4')
        game.pump(0.15)
        click(5, 5, 2)
        assert re.search(r'Food\s+##########', game.screen.text()), game.screen.text()
        click(5, 5, 2)  # A fractional drain must not immediately spend another meal.
        assert 'Held: Cooked Beef x3' in game.screen.text(), game.screen.text()
        game.capture('full-food-after-eating')
        outcome = game.finish()
    finally:
        game.close()
    world = saves / 'worlds/world-terminal-input'
    if mode == 'coop':
        saved = json.loads((world / 'server-state.json').read_text())
        assert len(saved['players']) == 1
        slots = next(iter(saved['players'].values()))['inventory']['slots']
    else:
        mcp = Mcp(binary, saves)
        try:
            slots = mcp.call('enter_world', dict(id='terminal-input'))['inventory']
            mcp.call('leave_world')
        finally:
            mcp.close()
    counts = {}
    for stack in slots:
        if stack:
            counts[stack['item_type']] = counts.get(stack['item_type'], 0) + stack['count']
    for item, expected in dict(Wood=0, Planks=78, Stick=8, Cobblestone=80, CookedBeef=3, Coal=4).items():
        assert counts.get(item, 0) == expected, (mode, item, counts)
    return dict(mode=mode, status='passed', saved_counts=counts, recipe_click_and_batch=True,
                manual_grid=True, merge_remainder=True, compact_resize_and_tab=True,
                food_stays_visibly_full=True, duplicate_meal_prevented=True, **outcome)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--keep-artifacts', action='store_true')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='termcraft-inventory-input-'))
    try:
        report = dict(status='passed', binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                      checks=[check(args.binary.resolve(), args.fixture.resolve(), root, mode)
                              for mode in ['native', 'coop']],
                      limits='Disposable staged supplies, real native/co-op PTY inputs and saved inventories. Excludes actual Warp GPU presentation and human UX acceptance.')
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
