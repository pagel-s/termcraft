#!/usr/bin/env python3
"""Decode water_review's actual incremental terminal frames and record evidence."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image
from capture_terminal_art import Screen, raster


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['captures', 'binary', 'output']:
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=True)
    meta = json.loads((a.captures / 'water.json').read_text())
    inputs = {}

    def decode(screen, name):
        raw = (a.captures / name).read_bytes()
        inputs[name] = hashlib.sha256(raw).hexdigest()
        screen.feed(raw)
        return raster(screen)

    screen = Screen(meta['columns'], meta['rows'])
    frames = []
    for tick in range(0, 241, 2):
        im = decode(screen, f'swim-{tick:03}.ansi')
        if tick in [0, 40, 80, 120, 160, 240]:
            im.save(a.output / f'swim-{tick:03}.png')
        if tick % 4 == 0:
            frames.append(im)
    strip = Image.new('RGB', (frames[0].width, frames[0].height * 6))
    for i, frame in enumerate([frames[n] for n in [0, 10, 20, 30, 40, 60]]):
        strip.paste(frame, (0, i * frame.height))
    palette = strip.quantize(colors=256)
    frames = [f.quantize(palette=palette, dither=Image.Dither.NONE) for f in frames]
    frames[0].save(a.output / 'swimming.gif', save_all=True, append_images=frames[1:], duration=200, loop=0)
    screen = Screen(meta['columns'], meta['rows'])
    for name in ['placement-before', 'placement-after']:
        decode(screen, name + '.ansi').save(a.output / (name + '.png'))
    outputs = {}
    for path in a.output.iterdir():
        if path.suffix in ['.png', '.gif']:
            with Image.open(path) as im:
                for n in range(getattr(im, 'n_frames', 1)):
                    im.seek(n)
                    im.load()
            outputs[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    evidence = dict(binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(), simulation=meta,
                    inputs=inputs, outputs=outputs,
                    source_sha256={f: hashlib.sha256(Path(f).read_bytes()).hexdigest() for f in
                                   ['src/engine/animal_water.rs', 'src/engine/state.rs', 'examples/water_review.rs']})
    (a.output / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(dict(images=len(outputs), reached_air=meta['reached_air'], reached_bank=meta['reached_bank'])))


if __name__ == '__main__':
    main()
