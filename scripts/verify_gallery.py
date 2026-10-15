#!/usr/bin/env python3
"""Validate every gallery image, optionally fetching all resources over HTTP.

This checks actual bytes, dimensions, content revisions and every GIF frame;
it does not automate a browser or measure gameplay latency.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
from html.parser import HTMLParser
from io import BytesIO
import json
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import ProxyHandler, Request, build_opener
from PIL import Image


class Page(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.links, self.images = [], []
        self.in_catalog = False
        self.catalog = ''
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'script':
            self.in_catalog = attrs.get('id') == 'atlas-data'
        if tag == 'img' and attrs.get('src'):
            self.images.append(attrs['src'])
        for key in ('src', 'href'):
            if attrs.get(key):
                self.links.append(attrs[key])

    def handle_endtag(self, tag):
        if tag == 'script':
            self.in_catalog = False

    def handle_data(self, data):
        if self.in_catalog:
            self.catalog += data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1] / 'docs/design')
    parser.add_argument('--base-url', help='HTTP root containing both gallery folders')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    world = root / 'world-atlas-2026-10-15'
    materials = root / 'world-refinement-2026-10-15'
    resources, revisions, dimensions = {}, {}, {}

    def add(folder, url):
        ref = urlsplit(url)
        if ref.scheme or not ref.path:
            return
        file = (folder / ref.path).resolve()
        assert file.is_relative_to(root) and file.is_file(), f'Missing gallery resource: {file}'
        path = file.relative_to(root).as_posix()
        resources[path + ('?' + ref.query if ref.query else '')] = file

    for folder in (world, materials):
        page = Page((folder / 'index.html').read_text())
        for url in page.links + ['index.html', 'evidence.json']:
            add(folder, url)
        if folder == world:
            catalog = json.loads((world / 'catalog.json').read_text())
            assert json.loads(page.catalog) == catalog, 'Embedded catalog is stale'
            assert len(page.images) == len(catalog['scenes']), 'Static covers missing'
            assert not catalog['coverage']['native_scene_gaps']
            assert catalog['coverage']['generation_issue'] is None
            for scene in catalog['scenes']:
                for view in scene['images']:
                    path = f'{world.name}/{view["file"]}'
                    assert view['revision'], f'Missing content revision: {path}'
                    revisions[path] = view['revision']
                    dimensions[path] = (view['columns'] * 9, view['rows'] * 18)
                    add(world, view['file'] + '?v=' + view['revision'])
            for id_ in ('nether-shrine', 'bridge-study', 'reliquary-study'):
                assert next(s for s in catalog['scenes'] if s['id'] == id_)['origin'] == 'Generated'
        else:
            assert len(page.images) == 97 * 3
            for url in page.images:
                path = f'{materials.name}/{urlsplit(url).path}'
                dimensions[path] = (144, 144)
                revisions[path] = urlsplit(url).query.removeprefix('v=')
            # Include the environment comparisons linked from the review notes.
            for file in materials.glob('*-before.png'):
                if not file.name.startswith('block-'):
                    add(materials, file.name)
            for file in materials.glob('*-now.png'):
                if not file.name.startswith('block-'):
                    add(materials, file.name)

    def check(entry):
        url, file = entry
        local = file.read_bytes()
        if args.base_url:
            request = Request(args.base_url.rstrip('/') + '/' + url, headers={'Cache-Control': 'no-cache'})
            with build_opener(ProxyHandler({})).open(request, timeout=20) as response:
                assert response.status == 200, (url, response.status)
                data = response.read()
            assert data == local, f'Server returned stale/different bytes: {url}'
        else:
            data = local
        path = file.relative_to(root).as_posix()
        digest = hashlib.sha256(data).hexdigest()
        if path in revisions:
            assert digest.startswith(revisions[path]), f'Stale content revision: {url}'
        frames = 0
        if file.suffix in ('.png', '.gif'):
            with Image.open(BytesIO(data)) as image:
                if path in dimensions:
                    assert image.size == dimensions[path], (path, image.size, dimensions[path])
                frames = image.n_frames
                for frame in range(frames):
                    image.seek(frame)
                    image.load()
                if file.suffix == '.gif':
                    assert frames > 1, f'Animation is static: {url}'
        return dict(url=url, sha256=digest, image_frames=frames)

    with ThreadPoolExecutor(max_workers=8) as executor:
        checks = list(executor.map(check, sorted(resources.items())))
    report = dict(status='passed', transport='HTTP' if args.base_url else 'filesystem',
                  resources=len(checks), images=sum(c['image_frames'] > 0 for c in checks),
                  decoded_frames=sum(c['image_frames'] for c in checks), checks=checks,
                  limits='HTTP byte/decode checks, not interactive browser automation.')
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k != 'checks'}))


if __name__ == '__main__':
    main()
