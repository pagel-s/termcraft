#!/usr/bin/env python3
"""Export actual ANSI renderer output as a phone-friendly combat/finale review."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image
from capture_terminal_art import Screen, raster


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['victory', 'actions', 'encounter', 'output', 'binary']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    inputs = {}

    def read(path):
        raw = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(raw).hexdigest()
        return raw

    for name, path, size in [
        ('cactus-before', args.actions / 'cactus-before-detail-160x48.ansi', (160, 48)),
        ('cactus-hurt', args.actions / 'cactus-hurt-detail-160x48.ansi', (160, 48)),
        ('returned-fireball', args.actions / 'fireball-return-overview-160x48.ansi', (160, 48)),
        ('returned-hit', args.actions / 'fireball-hit-overview-160x48.ansi', (160, 48)),
        ('end-patrol', args.encounter / 'end-0240-z0-160x48.ansi', (160, 48)),
        ('nether', args.encounter / 'nether-48-z0-160x48.ansi', (160, 48)),
    ]:
        screen = Screen(*size)
        screen.feed(read(path))
        raster(screen).save(args.output / (name + '.png'))
    screen = Screen(160, 48)
    frames = []
    for tick, path in enumerate(sorted(args.victory.glob('victory-*-160x48.ansi'))):
        screen.feed(read(path))  # Incremental frames require the same screen.
        im = raster(screen)
        frames.append(im)
        if tick in [0, 18, 36, 60, 90, 120, 140]:
            im.save(args.output / f'victory-{tick:03}.png')
    assert len(frames) == 141
    samples = Image.new('RGB', (frames[0].width, frames[0].height * 7))
    for i, tick in enumerate([0, 18, 36, 60, 90, 120, 140]):
        samples.paste(frames[tick], (0, i * frames[0].height))
    palette = samples.quantize(colors=256)
    gif = [im.quantize(palette=palette, dither=Image.Dither.NONE) for im in frames]
    gif[0].save(args.output / 'victory.gif', save_all=True, append_images=gif[1:],
                duration=50, loop=0, optimize=True)
    outputs = {}
    decoded = 0
    for path in sorted(args.output.iterdir()):
        if path.suffix in ['.png', '.gif']:
            with Image.open(path) as im:
                for frame in range(getattr(im, 'n_frames', 1)):
                    im.seek(frame)
                    im.load()
                    decoded += 1
            outputs[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    evidence = dict(binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                    inputs=inputs, outputs=outputs, decoded_frames=decoded,
                    native_save_version=10, server_save_version=4, network_protocol=17,
                    frame_duration_ms=50,
                    limits='Actual engine ticks and renderer, staged encounter/kill, deterministic captures. GIF timing represents 20 simulation ticks/s; not an emulator FPS or human-combat result.')
    (args.output / 'capture-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(dict(images=len(outputs), decoded_frames=decoded, binary_sha256=evidence['binary_sha256'])))


if __name__ == '__main__':
    main()
