#!/usr/bin/env python3
"""Run the entire boss finale in real solo/co-op terminals and keep controls usable.

The disposable fixture stages the lethal hit. Terminal input changes views and
moves the player; MCP only observes position. No real worlds are opened.
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
import time
from test_terminal_input import TerminalGame

BANNERS = ['THE DRAGON FALLS', 'THE END HOLDS BREATH', 'THE SKY BREAKS', 'VICTORY IN THE END']


def check(binary, fixture, root, mode):
    saves=root/mode/'saves';saves.parent.mkdir(parents=True)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1',0));address='127.0.0.1:'+str(listener.getsockname()[1])
    subprocess.run([str(fixture),'--pty-fixture',str(saves),address,'End','--victory'],check=True)
    game=TerminalGame(binary,saves,root/mode/'captures','terminal-input',address if mode=='coop' else None)
    seen=set();timings=[]
    try:
        assert any(b in game.screen.text() for b in BANNERS),game.screen.text()
        game.send(b'\x1b[100;1:3u');game.pump(0.1)
        before=game.x();game.send(b'\x1b[100;1:1u');game.pump(0.2)
        game.send(b'\x1b[100;1:3u');game.pump(0.15)
        after=game.x();assert after>before+0.05,(mode,before,after)
        game.send(b'\x1bOS');game.pump(0.12)
        assert 'Wide' in game.screen.text().splitlines()[0]
        game.send(b'\x1bOS');game.pump(0.12)
        assert 'Detail' in game.screen.text().splitlines()[0]
        game.send(b'\x1b[13~');game.pump(0.1)
        start=time.monotonic()
        while time.monotonic()-start<18:
            game.pump(0.3);text=game.screen.text()
            for banner in BANNERS:
                if banner in text and banner not in seen:
                    seen.add(banner);game.capture(banner.lower().replace(' ','-'))
            match=re.search(r'p95 tick ([\d.]+)ms render ([\d.]+)ms',text)
            if match:timings.append(dict(tick_p95_ms=float(match[1]),render_p95_ms=float(match[2])))
            if not any(b in text for b in BANNERS):break
        assert not any(b in game.screen.text() for b in BANNERS),'finale did not finish'
        assert set(BANNERS[1:])<=seen,(mode,seen)
        assert 'Detail' in game.screen.text().splitlines()[0],'finale changed selected resolution'
        assert 'autosave failed' not in game.screen.text().lower()
        assert timings,'missing live renderer timings'
        game.capture('finale-finished')
        outcome=game.finish()
        return dict(mode=mode,stages=sorted(seen),movement_during_finale=after-before,
                    selected_view_preserved=True,sequence_completed=True,
                    max_recorded_render_p95_ms=max(s['render_p95_ms'] for s in timings),
                    **outcome)
    finally:game.close()


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--fixture',type=Path,required=True)
    p.add_argument('--report',type=Path,required=True);p.add_argument('--keep-artifacts',action='store_true')
    a=p.parse_args();root=Path(tempfile.mkdtemp(prefix='tc-finale-input-'))
    try:
        report=dict(status='passed',binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                    checks=[check(a.binary.resolve(),a.fixture.resolve(),root,m) for m in ['native','coop']],
                    limits='Actual PTY output and local TCP; staged lethal hit. Excludes Warp/GPU timing, WAN and human judgment.')
        if a.keep_artifacts:report['artifacts']=str(root)
        a.report.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
    finally:
        if not a.keep_artifacts:shutil.rmtree(root)
if __name__=='__main__':main()
