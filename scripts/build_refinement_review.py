#!/usr/bin/env python3
"""Build a review gallery from actual ANSI fixtures; requires Pillow.

Pass material_review, entity_review, translation_review and pixel_review output
folders, then a NEW output folder. Course frames are incremental and are decoded
in order. Nothing opens a game save or invents pixels outside the captured frames.
"""
import argparse
import html
import json
import re
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
from capture_terminal_art import Screen, raster


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('materials', 'entities', 'translation', 'worlds', 'output'):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    out = args.output
    out.mkdir(parents=True, exist_ok=True)
    catalog = json.loads((args.materials / 'catalog.json').read_text())
    assert len(catalog['items']) == 135 and len(catalog['blocks']) == 97
    images = {}
    for group, root in [('materials', args.materials), ('entities', args.entities),
                        ('translation', args.translation), ('worlds', args.worlds)]:
        for path in sorted(root.glob('*.ansi')):
            if path.name.startswith(('course-', 'motion-')):
                continue
            match = re.search(r'-(\d+)x(\d+)$', path.stem)
            w, h = map(int, match.groups()) if match else (160, 48)
            screen = Screen(w, h)
            screen.feed(path.read_bytes())
            name = f'{group}-{path.stem}.png'
            images[name] = raster(screen)
            images[name].save(out / name)
    frames = []
    screen = Screen(160, 48)
    for path in sorted(args.translation.glob('course-*.ansi')):
        screen.feed(path.read_bytes())
        frames.append(raster(screen).resize((960, 576), Image.Resampling.NEAREST))
    assert len(frames) == 168
    frames[0].save(out / 'course.gif', save_all=True, append_images=frames[1:],
                   duration=[20, 10, 20] * 56, loop=0, optimize=True)
    (out / 'course.json').write_bytes((args.translation / 'course.json').read_bytes())
    (out / 'catalog.json').write_text(json.dumps(catalog, indent=2) + '\n')
    font = ImageFont.truetype('DejaVuSansMono.ttf', 12)
    sheets = []
    cards = {'blocks': [], 'items': []}
    for category, records, per_sheet in [('blocks', catalog['blocks'], 48),
                                          ('items', catalog['items'], 45)]:
        for index, record in enumerate(records):
            if category == 'blocks':
                frame = images[f'materials-blocks-{record["page"]:02}-160x48.png']
                left = (record['x'] * 8 - 4 + 80) * 9
                top = (record['y'] * 8 - 272 + 64) * 9
                crop = frame.crop((left, top, left + 72, top + 72)).resize((90, 90), Image.Resampling.NEAREST)
            else:
                page, slot = divmod(index, 9)
                frame = images[f'materials-drops-{page:02}-160x48.png']
                center = 16 + slot * 16
                crop = frame.crop(((center - 5) * 9, 23 * 9, (center + 5) * 9, 35 * 9))
            name = f'{category}-{index:03}.png'
            crop.save(out / name)
            record['crop'] = name
            cards[category].append(f'<figure data-name="{html.escape(record["name"].lower())}"><img src="{name}" alt="{html.escape(record["name"])}"><figcaption>{index + 1}. {html.escape(record["name"])}</figcaption></figure>')
        for page, start in enumerate(range(0, len(records), per_sheet)):
            batch = records[start:start + per_sheet]
            cols, cell_w, cell_h = (8, 180, 155) if category == 'blocks' else (9, 160, 158)
            sheet = Image.new('RGB', (cols * cell_w, ((len(batch) + cols - 1) // cols) * cell_h), (20, 25, 35))
            draw = ImageDraw.Draw(sheet)
            for n, record in enumerate(batch):
                x, y = n % cols * cell_w, n // cols * cell_h
                crop = Image.open(out / record['crop'])
                sheet.paste(crop, (x + (cell_w - crop.width) // 2, y + 4))
                label = f'{start + n + 1}. {record["name"]}'
                # Block states can be long; retain the complete label in HTML.
                for line, pos in enumerate(range(0, min(len(label), 66), 22)):
                    draw.text((x + 5, y + 113 + line * 13), label[pos:pos + 22], font=font, fill=(212, 224, 227))
            name = f'{category}-sheet-{page:02}.png'
            sheet.save(out / name)
            sheets.append(name)
    scenes = []
    for name in sorted(images):
        if name.startswith(('entities-', 'worlds-', 'translation-')):
            scenes.append(f'<figure class="scene"><a href="{name}"><img loading="lazy" src="{name}" alt="{name}"></a><figcaption>{name}</figcaption></figure>')
    page = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Termcraft · 2D refinement review</title><style>
body{margin:0;background:#111722;color:#e0e8ee;font:16px system-ui,sans-serif}main{max-width:1500px;margin:auto;padding:32px}h1{font-size:32px}p{max-width:900px;line-height:1.6}a{color:#b7e0cd}nav{display:flex;gap:24px;flex-wrap:wrap;padding:16px 0;position:sticky;top:0;background:#111722;z-index:1}input{font:inherit;padding:12px;width:min(90%,480px);background:#202b3b;border:1px solid #708494;color:inherit}.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:12px}figure{margin:0;padding:12px;background:#202937;border-radius:6px}figure img{display:block;margin:auto;image-rendering:pixelated}figcaption{font-size:12px;overflow-wrap:anywhere;margin-top:8px;line-height:1.4}.scenes{display:grid;gap:20px;grid-template-columns:repeat(auto-fit,minmax(440px,1fr))}.scene img{width:100%;height:auto}.motion{max-width:960px;width:100%;image-rendering:pixelated}section{scroll-margin-top:80px;margin:44px 0}[hidden]{display:none} @media(max-width:500px){main{padding:16px}.scenes{grid-template-columns:1fr}}
</style><main><h1>Termcraft · 2D refinement review</h1><p>Actual implemented renderer, temporary deterministic worlds. Review rounds cover terrain proportions and traversal, every item type, representative states of every non-air block variant, entity poses, lighting and shadows. These software captures demonstrate rendering and a physics course; they do not measure terminal latency or establish human gameplay acceptance.</p><nav><a href="#motion">Movement</a><a href="#blocks">97 block/state samples</a><a href="#items">135 items</a><a href="#scenes">Worlds & entities</a><a href="provenance.md">Checks & limitations</a></nav>
<section id="motion"><h2>Slabs → stairs → full blocks → descent → stop</h2><img class="motion" src="course.gif" alt="Actual physics course rendered at 60 samples per second"><p>168 incremental ANSI frames decoded in order; 56 simulation ticks. <a href="course.json">Position and grounded trace</a>. End columns and walking tunnels below use real seeded terrain, rather than a hand-built placeholder.</p></section><label>Find a block or item <input id="search" type="search" placeholder="e.g. diamond, stairs, piston"></label>
<section id="blocks"><h2>All blocks and representative states</h2><div class="grid">BLOCKS</div></section><section id="items"><h2>Every item as a world drop</h2><p>The same source atlas supplies inventory, crafting and hotbar icons. Full inventory frames are available in this folder as materials-inventory-*.png.</p><div class="grid">ITEMS</div></section><section id="scenes"><h2>Generated End, village, cave and all entity groups</h2><p>Click any scene for the complete image. Entity fixtures cover 120×40, 160×48 and 200×60. The village is generated; Nether/cave and entity lineups are staged.</p><div class="scenes">SCENES</div></section><script>document.querySelector('#search').addEventListener('input',e=>{const q=e.target.value.toLowerCase();document.querySelectorAll('[data-name]').forEach(card=>card.hidden=!card.dataset.name.includes(q))})</script></main></html>'''
    (out / 'index.html').write_text(page.replace('BLOCKS', ''.join(cards['blocks'])).replace('ITEMS', ''.join(cards['items'])).replace('SCENES', ''.join(scenes)))
    print(json.dumps({'items': len(catalog['items']), 'block_samples': len(catalog['blocks']), 'full_frames': len(images), 'course_frames': len(frames), 'sheets': sheets}))


if __name__ == '__main__':
    main()
