#!/usr/bin/env python3
"""Throw through real native/co-op terminal input and verify saved conservation."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile

from test_terminal_input import TerminalGame
from test_storage_recovery import Mcp


def check(binary, fixture, root, mode):
    saves = root / mode / 'saves'
    saves.parent.mkdir(parents=True)
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        address = '127.0.0.1:' + str(reservation.getsockname()[1])
    subprocess.run([str(fixture), '--inventory-fixture', str(saves), address], check=True)
    game = TerminalGame(binary, saves, root / mode / 'captures', 'terminal-input',
                        address if mode == 'coop' else None)
    try:
        game.send(b'1v')
        game.pump(0.15)
        assert 'Held: Wood x15' in game.screen.text(), game.screen.text()
        game.send(b'e')
        game.pump(0.12)
        assert 'Shift+V: stack' in game.screen.text()
        game.send(b'\x1b[<0;72;21M')  # Hotbar slot 1 in the 160x48 inventory.
        game.pump(0.08)
        game.send(b'\x1b[<0;72;21m')
        game.pump(0.08)
        assert 'Wood x15' in game.screen.text()
        game.send(b'V')  # Legacy terminals report uppercase rather than a modifier.
        game.pump(0.15)
        game.capture('thrown-stack')
        game.send(b'e')
        game.pump(0.05)
        outcome = game.finish()
    finally:
        game.close()
    world = saves / 'worlds/world-terminal-input'
    if mode == 'coop':
        saved = json.loads((world / 'server-state.json').read_text())
        slots = next(iter(saved['players'].values()))['inventory']['slots']
        entities = saved['runtime']['dimensions']['Overworld']['item_entities']
        drops = [dict(item=e['item_type'], count=(e.get('payload') or {}).get('stack', {}).get('count', 1))
                 for e in entities]
    else:
        mcp = Mcp(binary, saves)
        try:
            observed = mcp.call('enter_world', dict(id='terminal-input'))
            slots = observed['inventory']
            drops = observed['drops']
            mcp.call('leave_world')
        finally:
            mcp.close()
    held = sum(s['count'] for s in slots if s and s['item_type'] == 'Wood')
    thrown = sum(e['count'] for e in drops if e['item'] == 'Wood')
    assert held == 0 and thrown == 16, (mode, held, thrown, drops)
    return dict(mode=mode, status='passed', single_key=True, selected_stack_key=True,
                saved_inventory_wood=held, saved_loose_wood=thrown, **outcome)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--keep-artifacts', action='store_true')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='termcraft-drop-input-'))
    try:
        report = dict(status='passed', binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                      checks=[check(args.binary.resolve(), args.fixture.resolve(), root, mode)
                              for mode in ['native', 'coop']],
                      limits='Disposable supplied world, real terminal inputs and saved item totals; excludes actual Warp rendering and human UX acceptance.')
        if args.keep_artifacts:
            report['artifacts'] = str(root)
        args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(dict(status=report['status'], checks=len(report['checks']))))
    finally:
        if not args.keep_artifacts:
            shutil.rmtree(root)


if __name__ == '__main__':
    main()
