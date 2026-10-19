#!/usr/bin/env python3
"""Export engine-driven dragon finale frames, with bounded animation-export memory."""
import argparse
import hashlib
import html
import json
from pathlib import Path
from PIL import Image, ImageDraw
from capture_terminal_art import Screen, raster

SAMPLES = [0, 24, 56, 92, 128, 142, 170, 200, 228, 252, 280]
VIEWS = [('overview', 160, 48), ('compact', 120, 40), ('detail', 160, 48), ('wide', 200, 60)]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['captures', 'binary', 'output', 'before']:
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=True)
    metadata = json.loads((a.captures / 'victory.json').read_text())
    inputs = {}
    outputs = {}
    timings = {}
    contacts = []

    def read(path):
        raw = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(raw).hexdigest()
        return raw

    for view, w, h in VIEWS:
        paths = [a.captures / f'victory-{view}-{t:03}-{w}x{h}.ansi'
                 for t in range(metadata['frames'])]
        screen = Screen(w, h)
        samples = {}
        # First decode builds a shared palette from every act, without retaining
        # hundreds of full RGB frames. PNGs remain unquantized ground truth.
        for t, path in enumerate(paths):
            screen.feed(read(path))
            if t in SAMPLES:
                samples[t] = raster(screen)
                if view == 'overview' or t in [92, 142, 170, 228]:
                    samples[t].save(a.output / f'{view}-{t:03}.png')
        sample = next(iter(samples.values()))
        sheet = Image.new('RGB', (sample.width, sample.height * len(samples)))
        for i, im in enumerate(samples.values()):
            sheet.paste(im, (0, i * sample.height))
        palette = sheet.quantize(colors=256)
        del sheet
        screen = Screen(w, h)
        frames = []
        for path in paths:
            screen.feed(path.read_bytes())
            frames.append(raster(screen).quantize(palette=palette, dither=Image.Dither.NONE))
        gif = a.output / f'{view}.gif'
        frames[0].save(gif, save_all=True, append_images=frames[1:], duration=50,
                       loop=0, optimize=True)
        del frames
        with Image.open(gif) as im:
            duration = 0
            for f in range(im.n_frames):
                im.seek(f)
                im.load()
                duration += im.info['duration']
            assert duration == metadata['frames'] * 50, (view, duration)
            timings[view] = dict(engine_frames=metadata['frames'], gif_frames=im.n_frames,
                                 duration_ms=duration, columns=w, rows=h)
        if view == 'overview':
            contacts = [(t, samples[t]) for t in [24, 92, 142, 170, 228, 252]]
        del samples

    width, height = contacts[0][1].size
    sheet = Image.new('RGB', (width * 2, (height + 28) * 3), '#14101c')
    draw = ImageDraw.Draw(sheet)
    for i, (tick, im) in enumerate(contacts):
        x, y = i % 2 * width, i // 2 * (height + 28)
        draw.text((x + 10, y + 7), f'{tick / 20:.1f} seconds', fill='white')
        sheet.paste(im, (x, y + 28))
    sheet.thumbnail((1800, 2100), Image.Resampling.NEAREST)
    sheet.save(a.output / 'sequence.png')
    (a.output / 'before.gif').write_bytes(read(a.before))
    decoded = 0
    for path in a.output.iterdir():
        if path.suffix in ['.png', '.gif']:
            with Image.open(path) as im:
                for f in range(getattr(im, 'n_frames', 1)):
                    im.seek(f)
                    im.load()
                    decoded += 1
            outputs[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    proof = dict(binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                 simulation=metadata, views=timings, inputs=inputs, outputs=outputs,
                 decoded_frames=decoded,
                 source_sha256={str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in
                                [Path('src/renderer/victory.rs'), Path('src/renderer/dragon.rs'),
                                 Path('src/renderer/sprites.rs'), Path('src/renderer/terminal.rs'),
                                 Path('examples/encounter_review.rs')]},
                 limits='Actual engine death/ticks and terminal renderer. Seed, player and lethal hit staged in a disposable world. GIF time is simulation time; render CPU timings exclude terminal/GPU output and human combat.')
    (a.output / 'capture-evidence.json').write_text(json.dumps(proof, indent=2) + '\n')
    cards = ''.join(f'<h2>{html.escape(view.title())} · {w} × {h}</h2>'
                    f'<img loading="lazy" src="{view}.gif" alt="Dragon finale in {view} view">'
                    for view, w, h in VIEWS)
    page = ('<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">'
            '<title>Dragon finale review</title><style>body{background:#130f1c;color:#eee;'
            'font:16px system-ui;max-width:1100px;margin:24px auto;padding:16px}'
            'img{max-width:100%;height:auto;image-rendering:pixelated}a{color:#d7a6ff}</style>'
            '<h1>The dragon’s last stand</h1><p>Actual terminal renderer, staged lethal hit, '
            '14 seconds at 20 engine ticks/s. See README and capture-evidence.json for limits.</p>'
            + cards + '<h2>Previous animation</h2><img src="before.gif" alt="Previous finale">')
    (a.output / 'gallery.html').write_text(page + '\n')
    print(json.dumps(dict(images=len(outputs), decoded_frames=decoded,
                          binary_sha256=proof['binary_sha256'], views=timings)))


if __name__ == '__main__':
    main()
