#!/usr/bin/env python3
"""Export the raised-pillar and covered-player flight review from actual ANSI."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

from PIL import Image
from capture_terminal_art import Screen, raster


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['normal', 'covered', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    inputs = {}
    samples = [
        ('raised-arena', args.normal, 'end-0000-z1-200x60.ansi', 200, 60),
        ('raised-overview', args.normal, 'end-0240-z0-160x48.ansi', 160, 48),
        ('raised-small', args.normal, 'end-0240-z0-120x40.ansi', 120, 40),
        ('covered-start', args.covered, 'end-0000-z1-200x60.ansi', 200, 60),
        ('covered-later', args.covered, 'end-0480-z1-200x60.ansi', 200, 60),
    ]
    for name, directory, file, width, height in samples:
        path = directory / file
        raw = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(raw).hexdigest()
        screen = Screen(width, height)
        screen.feed(raw)
        raster(screen).save(args.output / (name + '.png'))
    screen = Screen(200, 60)
    frames = []
    for index, path in enumerate(sorted(args.covered.glob('flight-*-200x60.ansi'))):
        raw = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(raw).hexdigest()
        screen.feed(raw)  # Incremental output must share one decoder/screen.
        if index % 2 == 0:
            frames.append(raster(screen).convert('P', palette=Image.Palette.ADAPTIVE, colors=128))
    frames[0].save(args.output / 'covered-flight.gif', save_all=True, append_images=frames[1:], duration=400, loop=0, optimize=True)
    for label, directory in [('normal', args.normal), ('covered', args.covered)]:
        for name in ['trajectory', 'simulation']:
            shutil.copyfile(directory / (name + '.json'), args.output / (label + '-' + name + '.json'))
    previous = Path(__file__).resolve().parents[1] / 'docs/design/nether-end-refinement-2026-10-16/end-after.png'
    shutil.copyfile(previous, args.output / 'compact-before.png')
    outputs = {}
    for path in args.output.iterdir():
        if path.suffix in ['.png', '.gif']:
            with Image.open(path) as image:
                image.verify()
            outputs[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    (args.output / 'capture-hashes.json').write_text(json.dumps(dict(inputs=inputs, outputs=outputs), indent=2) + '\n')


if __name__ == '__main__':
    main()
