#!/usr/bin/env python3
"""Export actual renderer captures and sequential flight frames for review."""
import argparse
import hashlib
import html
import json
from pathlib import Path
import shutil

from PIL import Image
from capture_terminal_art import Screen, raster


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['before', 'after', 'dragon-before', 'dragon-after', 'nether-before', 'nether-after', 'output']:
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=True)
    samples = [
        ('end-before', 'Generated arena before', a.before / 'end-0000-z1-200x60.ansi', 200, 60),
        ('end-after', 'Generated arena now', a.after / 'end-0000-z1-200x60.ansi', 200, 60),
        ('dragon-before', 'Frontal art before — staged pose', a.dragon_before / 'raised-160x48.ansi', 160, 48),
        ('dragon-after', 'Leaner frontal art — staged pose', a.dragon_after / 'raised-160x48.ansi', 160, 48),
        ('cavern-before', 'Generated Nether cavern before', a.nether_before / 'cavern-160x48.ansi', 160, 48),
        ('cavern-after', 'Generated Nether cavern now', a.nether_after / 'cavern-160x48.ansi', 160, 48),
        ('fortress-before', 'Generated fortress before', a.nether_before / 'fortress-160x48.ansi', 160, 48),
        ('fortress-after', 'Generated fortress now', a.nether_after / 'fortress-160x48.ansi', 160, 48),
        ('fissure-before', 'Generated route before — x=96', a.before / 'nether-96-z0-160x48.ansi', 160, 48),
        ('fissure-after', 'Generated route now — nearest safe bank', a.after / 'nether-96-z0-160x48.ansi', 160, 48),
        ('encounter-after', 'Staged Nether creatures over generated terrain', a.nether_after / 'encounter-160x48.ansi', 160, 48),
        ('small-end', 'Generated arena — 120×40 overview', a.after / 'end-0240-z0-120x40.ansi', 120, 40),
    ]
    inputs = {}
    cards = []
    for name, title, path, w, h in samples:
        raw = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(raw).hexdigest()
        screen = Screen(w, h)
        screen.feed(raw)
        raster(screen).save(a.output / (name + '.png'))
        cards.append(f'<figure><figcaption>{html.escape(title)}</figcaption><img loading="lazy" src="{name}.png" alt="{html.escape(title)}"></figure>')
    # Renderer output is incremental. Decode every frame into ONE screen; only
    # rasterize alternate frames. At 20 ticks/s, four ticks per input frame and
    # eight per exported frame, the animation uses 400 ms frame durations.
    screen = Screen(200, 60)
    frames = []
    for index, path in enumerate(sorted(a.after.glob('flight-*-200x60.ansi'))):
        raw = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(raw).hexdigest()
        screen.feed(raw)
        if index % 2 == 0:
            frames.append(raster(screen).convert('P', palette=Image.Palette.ADAPTIVE, colors=128))
    frames[0].save(a.output / 'flight.gif', save_all=True, append_images=frames[1:], duration=400, loop=0, optimize=True)
    for name in ['trajectory.json', 'simulation.json', 'nether-positions.json']:
        shutil.copyfile(a.after / name, a.output / name)
    page = '<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Nether and End review</title><style>body{background:#15131d;color:#eee;font:16px system-ui;max-width:1200px;margin:32px auto;padding:16px}img{max-width:100%;image-rendering:pixelated}figure{margin:28px 0}figcaption{margin-bottom:8px}a{color:#cba3ff}</style><h1>Nether and End review</h1><p>Actual game renderer. Fixed-position invulnerable player for simulated flight; no human boss fight or smoothness claim. See the companion README for reproduction details and limits.</p><h2>Engine-driven flight — 40 seconds, sampled at 2.5 fps</h2><img src="flight.gif" alt="Simulated dragon patrol and swoop">' + ''.join(cards)
    (a.output / 'comparison.html').write_text(page + '\n')
    outputs = {}
    for path in a.output.glob('*'):
        if path.suffix in ['.png', '.gif']:
            with Image.open(path) as image:
                image.verify()
            outputs[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    (a.output / 'capture-hashes.json').write_text(json.dumps(dict(inputs=inputs, outputs=outputs), indent=2) + '\n')


if __name__ == '__main__':
    main()
