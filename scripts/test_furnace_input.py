#!/usr/bin/env python3
"""Real native/co-op mouse furnace use and F4 view persistence across restart.

Fixtures only stage supplies/terrain. All furnace and view actions go through
terminal input; save records are inspected after the game/server exits.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
from test_terminal_input import TerminalGame


def check(binary, fixture, root, mode):
    saves = root / mode / 'saves'
    saves.parent.mkdir(parents=True)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        address = '127.0.0.1:' + str(listener.getsockname()[1])
    subprocess.run([str(fixture), '--furnace-fixture', str(saves), address], check=True)
    def launch():
        return TerminalGame(binary, saves, root / mode / 'captures', 'terminal-input',
                            address if mode == 'coop' else None)
    game = launch()
    def click(x, y, button=0):
        game.send(f'\x1b[<{button};{x+1};{y+1}M'.encode()); game.pump(0.10)
        game.send(f'\x1b[<{button};{x+1};{y+1}m'.encode()); game.pump(0.10)
    try:
        game.send(b'--');game.pump(0.2)  # Detail -> Overview -> Wide for this mouse fixture.
        assert 'Wide' in game.screen.text().splitlines()[0]
        found = False
        for y in range(29,34):
            for x in range(82,86):
                game.send(f'\x1b[<35;{x+1};{y+1}M'.encode());game.pump(0.06)
                if 'Furnace' in game.screen.text():
                    click(x,y,2);found=True;break
            if found:break
        assert found,game.screen.text()
        text=game.screen.text()
        assert all(label in text for label in ['INPUT','FUEL','OUTPUT']),text
        click(71,20);click(83,24)  # Raw iron -> INPUT.
        game.pump(0.3)
        assert 'NO FUEL' in game.screen.text(),game.screen.text()
        click(77,20);click(83,28)  # Coal -> FUEL.
        assert 'HEAT ON' in game.screen.text(),game.screen.text()
        game.capture('furnace-cooking')
        game.resize(80,24)
        assert all(label in game.screen.text() for label in ['INPUT','FUEL','OUTPUT'])
        game.capture('furnace-compact')
        game.resize(160,48)
        for _ in range(22):game.pump(0.5)
        # Take first output. Further queued input may already be cooking.
        click(109,26,4)
        game.capture('furnace-collected')
        game.send(b'e==');game.pump(0.2)
        assert 'Detail' in game.screen.text().splitlines()[0]
        game.finish()
    finally:
        game.close()
    assert json.loads((saves/'view.json').read_text())['view']==2
    world=saves/'worlds/world-terminal-input'
    if mode=='coop':
        session=next(iter(json.loads((world/'server-state.json').read_text())['players'].values()))
    else:
        session=json.loads(subprocess.check_output([str(fixture),'--session-inspect',str(world)]))
    bag=session['inventory']['slots']; slots=session['furnace_slots']; job=session['furnace_job']
    ingots=sum(s['count'] for s in bag if s and s['item_type']=='IronIngot')
    assert ingots>=1,(mode,session)
    total=ingots+sum(s['count'] for s in slots if s and s['item_type'] in ['RawIron','IronIngot'])
    if job:total+=job['input_count']
    assert total==3,(mode,total,session)
    assert slots[1] and slots[1]['count']==1,(mode,slots)
    game=launch()
    try:
        assert 'Detail' in game.screen.text().splitlines()[0],game.screen.text()
        game.send(b'-');game.pump(0.15)
        assert 'Overview' in game.screen.text().splitlines()[0]
        game.finish()
    finally:game.close()
    assert json.loads((saves/'view.json').read_text())['view']==0
    return dict(mode=mode,explicit_slots=True,compact_slots=True,smelt_and_collect=True,
                conserved_input=total,view_survived_restart=True)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--fixture',type=Path,required=True)
    p.add_argument('--report',type=Path,required=True)
    p.add_argument('--keep-artifacts',action='store_true')
    a=p.parse_args();root=Path(tempfile.mkdtemp(prefix='tc-furnace-input-'))
    try:
        report=dict(status='passed',binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                    checks=[check(a.binary.resolve(),a.fixture.resolve(),root,m) for m in ['native','coop']],
                    limits='Local PTYs and loopback co-op; excludes Warp/GPU timing and human feel.')
        if a.keep_artifacts:report['artifacts']=str(root)
        a.report.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
    finally:
        if not a.keep_artifacts:shutil.rmtree(root)
if __name__=='__main__':main()
