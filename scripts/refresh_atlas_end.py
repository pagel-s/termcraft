#!/usr/bin/env python3
"""Refresh only generated/staged End scenes in an existing world atlas."""
import argparse
import hashlib
import json
from pathlib import Path
import re

from capture_terminal_art import Screen, raster


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('raw', type=Path)
    parser.add_argument('atlas', type=Path)
    args = parser.parse_args()
    native = json.loads((args.raw / 'catalog.json').read_text())
    catalog = json.loads((args.atlas / 'catalog.json').read_text())
    assert native['native_seed'] == catalog['native_seed']
    scenes = {r['id']: r for r in native['scenes'] if r['dimension'] == 'End'}
    page = (args.atlas / 'index.html').read_text()
    hashes = {}
    for scene in catalog['scenes']:
        if scene['id'] not in scenes:
            continue
        raw = scenes[scene['id']]
        views = {r['view']: r for r in raw['images']}
        scene.update(x=raw['x'], y=raw['y'])
        for picture in scene['images']:
            view = views[picture['mode']]
            assert [picture['columns'], picture['rows']] == [view['width'], view['height']]
            source = args.raw / view['ansi']
            screen = Screen(view['width'], view['height'])
            screen.feed(source.read_bytes())
            target = args.atlas / picture['file']
            raster(screen).save(target)
            revision = hashlib.sha256(target.read_bytes()).hexdigest()[:16]
            page = page.replace(picture['file'] + '?v=' + picture['revision'], picture['file'] + '?v=' + revision)
            picture['revision'] = revision
            hashes[picture['file']] = dict(ansi_sha256=hashlib.sha256(source.read_bytes()).hexdigest(), png_sha256=hashlib.sha256(target.read_bytes()).hexdigest())
    catalog['end_refresh'] = 'end-refresh.json'
    data = json.dumps(catalog, separators=(',', ':')).replace('<', '\\u003c')
    page, count = re.subn(r'(<script type="application/json" id="atlas-data">).*?(</script>)', lambda m: m[1] + data + m[2], page, count=1, flags=re.S)
    assert count == 1
    (args.atlas / 'index.html').write_text(page)
    (args.atlas / 'catalog.json').write_text(json.dumps(catalog, indent=2) + '\n')
    (args.atlas / 'end-refresh.json').write_text(json.dumps(dict(date='2026-10-17', source='examples/world_review.rs', scope='End scene captures only; other scenes retain their recorded provenance.', images=hashes), indent=2) + '\n')
    print('Refreshed', len(hashes), 'End images and their embedded catalog/cache revisions.')


if __name__ == '__main__':
    main()
