#!/usr/bin/env python3
"""Walk a generated review village with real native terminal input in a copied save.

Run examples/village_review.rs first and pass its village.json. Requires Pillow
for the software capture; MCP is used only to read the live player's position.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import struct
import subprocess
import tempfile
import termios
import time
from capture_terminal_art import Screen, raster

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, default=Path('termcraft'))
parser.add_argument('--fixture', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
binary = str(args.binary.resolve())
fixture = json.loads(args.fixture.read_text())
root = Path(tempfile.mkdtemp(prefix='termcraft-village-native-'))
shutil.copytree(fixture['review_root'], root / 'save')
env = dict(os.environ, TERMCRAFT_SAVE_DIR=str(root / 'save'), TERM='xterm-256color')
env.pop('NO_COLOR', None)
env.pop('TERMCRAFT_DIRECTOR', None)
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))
before = termios.tcgetattr(slave)

def setup():
    os.setsid()
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

p = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=setup)
mcp = None
screen = Screen(160, 48)
restored = queried = False
seq = 0

def pump(seconds):
    global restored, queried
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if select.select([master], [], [], .005)[0]:
            data = os.read(master, 65536)
            screen.feed(data)
            restored |= b'\x1b[?1049l' in data
            if not queried and b'\x1b[?u' in data:
                queried = True
                os.write(master, b'\x1b[?0u\x1b[?1;2c')

def call(name, arguments):
    global seq
    seq += 1
    mcp.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': seq, 'method': 'tools/call',
                               'params': {'name': name, 'arguments': arguments}}) + '\n')
    mcp.stdin.flush()
    until = time.monotonic() + 6
    while not select.select([mcp.stdout], [], [], 0)[0]:
        assert time.monotonic() < until, ('MCP timeout', name)
        pump(.005)
    result = json.loads(mcp.stdout.readline())['result']
    assert not result.get('isError'), result
    return result['structuredContent']

try:
    pump(.5)
    os.write(master, b' ')
    pump(.8)
    assert queried, 'Terminal enhancement query missing'
    os.write(master, b't')
    pump(.1)
    os.write(master, b'/connect\r')
    pump(.3)
    mcp = subprocess.Popen([binary, 'mcp'], env=env, stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    call('director_connect', {})
    os.write(master, b'\x1b[100;1:1u')
    pump(.035)
    os.write(master, b'\x1b[100;1:3u')
    pump(.1)
    start = call('director_observe', {})['context']['player']
    doors = sorted(x for b in fixture['buildings'] for x in (b['left'], b['right']))
    clicked = []
    heights = []
    os.write(master, b'\x1b[100;1:1u')  # Enhanced D press, followed by normal OS repeat events.
    begun = time.monotonic()
    last_repeat = begun
    while time.monotonic() - begun < 28:
        if time.monotonic() - last_repeat > .4:
            os.write(master, b'\x1b[100;1:2u')
            last_repeat = time.monotonic()
        player = call('director_observe', {})['context']['player']
        x = player['x']
        heights.append(player['y'])
        if x >= fixture['center_x'] + 46:
            break
        for door in doors:
            if door not in clicked and .35 < door + .5 - x < 2.3:
                # Aim at the settled image, not at a live state observation
                # that can be ahead of the frame currently visible in the PTY.
                os.write(master, b'\x1b[100;1:3u')
                pump(.16)
                settled = call('director_observe', {})['context']['player']
                col = round((door + .5) * 4 - round(settled['x'] * 4) + 80) + 1
                os.write(master, f'\x1b[<2;{col};34M\x1b[<2;{col};34m'.encode())
                pump(.1)
                clicked.append(door)
                os.write(master, b'\x1b[100;1:1u')
                last_repeat = time.monotonic()
        pump(.07)
    else:
        raise AssertionError(('Native village walk stalled', player, clicked, doors))
    os.write(master, b'\x1b[100;1:3u')
    pump(.2)
    end = call('director_observe', {})['context']['player']
    assert clicked == doors, (clicked, doors)
    assert max(abs(y - fixture['ground_y']) for y in heights) < .15, heights
    raster(screen).save(root / 'native-village-160x48.png')
    os.write(master, b'q')
    pump(.4)
    p.wait(timeout=5)
    assert p.returncode == 0 and restored and before == termios.tcgetattr(slave)
    report = {'status': 'passed', 'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
              'start': start, 'end': end, 'walked_blocks': end['x'] - start['x'],
              'doors_opened_with_mouse': clicked, 'jump_events': 0, 'pauses_to_aim_at_doors': True,
              'max_feet_deviation': max(abs(y - fixture['ground_y']) for y in heights),
              'terminal_restored': True, 'artifacts': str(root),
              'limits': 'Local PTY with enhanced held-key and actual SGR mouse input. Read-only director observations. Copied generated review save, spawning/daylight disabled. Excludes emulator/WAN latency and human enjoyment.'}
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
finally:
    if p.poll() is None:
        p.kill()
        p.wait()
    if mcp is not None:
        mcp.kill()
        mcp.wait()
    os.close(master)
    os.close(slave)
    shutil.rmtree(root / 'save')
