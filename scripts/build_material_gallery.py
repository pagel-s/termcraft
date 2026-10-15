#!/usr/bin/env python3
"""Compare every block/state from actual renderer pixels.

Usage: build_material_gallery.py BEFORE_MATERIALS AFTER_MATERIALS OUTPUT
Before captures use the identical after fixture's per-block screen bounds.
Only cropping and nearest-neighbor enlargement occur.
"""
import argparse
import hashlib
import html
import json
import re
from pathlib import Path
from PIL import Image
from capture_terminal_art import Screen, raster


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('before', 'after', 'output'):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    catalog = json.loads((args.after / 'catalog.json').read_text())
    cache, hashes, cards = {}, {}, []
    refined = {'Sand', 'Gravel', 'Bedrock', 'BirchLeaves', 'Snow', 'Ice', 'Wool',
               'Obsidian', 'SugarCane', 'Sapling', 'BirchSapling', 'IronOre',
               'CoalOre', 'GoldOre', 'DiamondOre', 'RedstoneOre', 'Ladder',
               'RedstoneTorch(false)', 'RedstoneTorch(true)'}
    for index, block in enumerate(catalog['blocks']):
        samples = []
        for mode, root, suffix in [('Before', args.before, '-160x48.ansi'),
                                   ('Now', args.after, '-160x48.ansi'),
                                   ('Detail', args.after, '-detail-200x72.ansi')]:
            file = next(f for f in block['views'] if f.endswith(suffix))
            key = (mode, file)
            if key not in cache:
                data = (root / file).read_bytes()
                width, height = map(int, re.search(r'(\d+)x(\d+)\.ansi$', file).groups())
                screen = Screen(width, height)
                screen.feed(data)
                cache[key] = raster(screen)
                hashes[f'{mode}/{file}'] = hashlib.sha256(data).hexdigest()
            x0, y0, x1, y1 = block['views'][file]
            sample = cache[key].crop((x0 * 9, y0 * 18, x1 * 9, y1 * 18))
            assert sample.width == sample.height, (block['name'], mode)
            sample = sample.resize((144, 144), Image.Resampling.NEAREST)
            filename = f'block-{index:03}-{mode.lower()}.png'
            sample.save(args.output / filename)
            name = html.escape(block['name'])
            revision = hashlib.sha256((args.output / filename).read_bytes()).hexdigest()[:16]
            samples.append(f'<figure><img src="{filename}?v={revision}" width="144" height="144" alt="{name} · {mode}"><figcaption>{mode}</figcaption></figure>')
        changed = block['name'] in refined
        cards.append(f'<article data-name="{name.lower()}" data-refined="{str(changed).lower()}"><h2>{name}</h2><span>{"Refined" if changed else "Reviewed · retained"}</span><div class="samples">{"".join(samples)}</div></article>')
    assert len(cards) == 97
    (args.output / 'index.html').write_text(PAGE.replace('@CARDS@', '\n'.join(cards)))
    (args.output / 'blocks.json').write_text(json.dumps(catalog['blocks'], indent=2) + '\n')
    (args.output / 'material-inputs.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print(f'Rendered {len(cards)} block/state comparisons')


PAGE = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Termcraft · block audit</title><style>
:root{color-scheme:dark;font:16px system-ui;background:#0f141d;color:#dce7e8}body{margin:0 auto;max-width:1480px;padding:40px 24px}a{color:#81ded0}h1{font-size:clamp(32px,5vw,64px);margin:16px 0}p{max-width:900px;line-height:1.6;color:#b4c6cf}.controls{display:flex;gap:24px;align-items:center;margin:28px 0;flex-wrap:wrap}input[type=search]{padding:12px;background:#1a2533;border:1px solid #526476;color:inherit;border-radius:8px;font:inherit}#grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(330px,1fr));gap:16px}article{background:#18222e;border:1px solid #344353;border-radius:12px;padding:18px;overflow:hidden}h2{font-size:18px;margin:0 0 6px;overflow-wrap:anywhere}article>span{font-size:13px;color:#93cbbd}.samples{display:flex;gap:10px;margin-top:18px}figure{margin:0;flex:1;min-width:0}img{width:100%;image-rendering:pixelated}figcaption{font-size:13px;margin-top:8px;color:#b4c6cf}[hidden]{display:none!important}
</style><a href="../world-atlas-2026-10-15/index.html">← World gallery</a><h1>Every block, inspected.</h1>
<p>97 block/state samples through the actual game renderer. Before and Now use the same overview; Detail uses the detailed zoom. Enlarged with nearest-neighbor pixels. Backgrounds and shadows come from the staged, daytime fixture. Tiny switches, transparent glass and half blocks preserve their silhouettes.</p>
<p>Refinements concentrate on material identity: stone versus bedrock, sand versus gravel, birch foliage, snow versus ice, wool, reeds, saplings, ore inclusions and quieter obsidian. Retained blocks are included for a complete audit.</p>
<div class="controls"><input id="search" type="search" aria-label="Find a block" placeholder="Find a block…"><label><input id="refined" type="checkbox"> Refined blocks only</label><output id="count"></output></div><div id="grid">@CARDS@</div>
<p><a href="provenance.md">Review findings and verification</a> · <a href="blocks.json">Block catalog</a> · <a href="ores-before.json">Ore census before</a> · <a href="ores-after.json">Ore census now</a></p>
<script>const cards=[...document.querySelectorAll('article')],search=document.getElementById('search'),refined=document.getElementById('refined');function filter(){let n=0;for(const c of cards){c.hidden=!c.dataset.name.includes(search.value.toLowerCase())||(refined.checked&&c.dataset.refined!=='true');if(!c.hidden)n++;}document.getElementById('count').textContent=n+' / '+cards.length+' samples';}search.addEventListener('input',filter);refined.addEventListener('change',filter);filter();</script></html>'''


if __name__ == '__main__':
    main()
