#!/usr/bin/env python3
"""Export current captures as GitHub-readable galleries and optional offline HTML."""
import argparse
import base64
import copy
from io import BytesIO
import json
from pathlib import Path
import re
from PIL import Image


def markdown(world, materials):
    catalog = json.loads((world / 'catalog.json').read_text())
    lines = ['# Termcraft world gallery', '',
             f'{len(catalog["scenes"])} scenes and {sum(len(s["images"]) for s in catalog["scenes"])} views. Open a section to browse; select a view link for the original image.', '',
             '[All 97 block/state comparisons](../world-refinement-2026-10-15/README.md) · [Capture notes](provenance.md)', '']
    for category in dict.fromkeys(s['category'] for s in catalog['scenes']):
        scenes = [s for s in catalog['scenes'] if s['category'] == category]
        lines += ['<details>', f'<summary>{category} · {len(scenes)} scenes</summary>', '']
        for scene in scenes:
            lines += [f'### {scene["title"]}', '', f'{scene["dimension"]} · {scene["origin"]}', '',
                      f'![{scene["title"]}]({scene["images"][0]["file"]})', '', scene['caption'], '',
                      ' · '.join(f'[{v["label"]}]({v["file"]})' for v in scene['images']), '']
        lines += ['</details>', '']
    (world / 'README.md').write_text('\n'.join(lines))
    blocks = json.loads((materials / 'blocks.json').read_text())
    lines = ['# Termcraft block and state comparisons', '',
             'All 97 samples, captured through the actual renderer. Select an image to enlarge it.', '',
             '[World gallery](../world-atlas-2026-10-15/README.md) · [Review notes and ore census](provenance.md)', '',
             '| Block/state | Before | Current | Detail |', '| --- | --- | --- | --- |']
    for i, block in enumerate(blocks):
        images = ' | '.join(f'[![{mode}](block-{i:03}-{mode}.png)](block-{i:03}-{mode}.png)' for mode in ['before', 'now', 'detail'])
        lines.append(f'| `{block["name"]}` | {images} |')
    (materials / 'README.md').write_text('\n'.join(lines) + '\n')


def standalone(world, materials, output):
    catalog = copy.deepcopy(json.loads((world / 'catalog.json').read_text()))
    assets = {}

    def data(file):
        if file not in assets:
            raw = file.read_bytes()
            mime = 'image/gif' if file.suffix == '.gif' else 'image/png'
            with Image.open(BytesIO(raw)) as image:
                for frame in range(image.n_frames):
                    image.seek(frame)
                    image.load()
            assets[file] = 'data:' + mime + ';base64,' + base64.b64encode(raw).decode()
        return assets[file]

    for scene in catalog['scenes']:
        for view in scene['images']:
            view['embedded'] = data(world / view['file'])
    page = (world / 'index.html').read_text()
    page = re.sub(r'(<script type="application/json" id="atlas-data">).*?(</script>)',
                  lambda m: m[1] + json.dumps(catalog, separators=(',', ':')).replace('<', '\\u003c') + m[2], page, flags=re.S)
    page = page.replace("return view.file+'?v='+view.revision;", 'return view.embedded;')
    page = re.sub(r'(src|href)="([^"?]+)(?:\?v=[^"]*)?"',
                  lambda m: f'{m[1]}="{data(world / m[2])}"' if (world / m[2]) in assets else m[0], page)
    page = page.replace('href="../world-refinement-2026-10-15/index.html"', 'href="#block-gallery"')
    # Local file viewers may forbid history changes; that must not break the viewer.
    page = page.replace("history.replaceState(null,'','#'+scene.id);", "try{history.replaceState(null,'','#'+scene.id);}catch{}")
    page = page.replace("history.replaceState(null,'',location.pathname+location.search);", "try{history.replaceState(null,'',location.pathname+location.search);}catch{}")
    for name in ['provenance.md', 'catalog.json', 'evidence.json']:
        page = page.replace(f'href="{name}"', 'href="#coverage"')
    cards = re.findall(r'<article\b.*?</article>', (materials / 'index.html').read_text(), flags=re.S)
    assert len(cards) == 97
    cards = [re.sub(r'src="([^"?]+)(?:\?v=[^"]*)?"', lambda m: 'src="' + data(materials / m[1]) + '"', card) for card in cards]
    section = '<details id="block-gallery"><summary>All 97 block/state comparisons</summary><div class="block-grid">' + ''.join(cards) + '</div></details>'
    page = page.replace('</main>', section + '</main>')
    page = page.replace('</style>', '#block-gallery .block-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(290px,1fr));gap:18px}#block-gallery article{padding:12px;overflow-wrap:anywhere}#block-gallery .samples{display:flex;gap:8px}#block-gallery figure{margin:0;min-width:0;flex:1}#block-gallery img{width:100%;height:auto;image-rendering:pixelated}#block-gallery h2{font-size:16px}#block-gallery figcaption{font-size:12px}</style>')
    assert len(assets) == sum(len(s["images"]) for s in catalog["scenes"]) + len(cards) * 3
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(page)
    print(f'Embedded and decoded {len(assets)} images; {output.stat().st_size} bytes: {output}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    root = Path(__file__).resolve().parents[1] / 'docs/design'
    parser.add_argument('--world', type=Path, default=root / 'world-atlas-2026-10-15')
    parser.add_argument('--materials', type=Path, default=root / 'world-refinement-2026-10-15')
    parser.add_argument('--standalone', type=Path)
    args = parser.parse_args()
    markdown(args.world, args.materials)
    if args.standalone:
        standalone(args.world, args.materials, args.standalone)


if __name__ == '__main__':
    main()
