#!/usr/bin/env python3
"""Build a searchable world gallery from actual ANSI captures; requires Pillow.

Usage: build_world_gallery.py RAW_DIRECTORY NEW_OUTPUT_DIRECTORY
Run world_review, village_review, entity_review, weather_review, lantern_review,
pixel_review and nether_review into named subfolders first. No game saves are opened.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
from capture_terminal_art import Screen, raster


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('raw', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    native = json.loads((args.raw / 'worlds/catalog.json').read_text())
    scenes = []
    input_hashes = {}

    def add(record, group, images):
        record = dict(record)
        record['images'] = []
        for mode, title, filename, width, height in images:
            path = args.raw / group / filename
            if not path.is_file():
                raise FileNotFoundError(path)
            data = path.read_bytes()
            input_hashes[f'{group}/{filename}'] = hashlib.sha256(data).hexdigest()
            screen = Screen(width, height)
            screen.feed(data)
            index = len(record['images'])
            name = f'{record["id"]}-{index:02}.png'
            raster(screen).save(args.output / name)
            record['images'].append(dict(mode=mode, label=title, file=name, columns=width, rows=height))
        if record['title'] == 'ExtremeHills':
            record['title'] = 'Extreme hills'
        scenes.append(record)

    for record in native['scenes']:
        images = [(v['view'], v['view'].title(), v['ansi'], v['width'], v['height']) for v in record['images']]
        preferred = 'panorama' if 'panorama' in record['id'] else 'tall' if 'vault' in record['id'] or 'roof' in record['id'] else 'overview'
        images.sort(key=lambda v: v[0] != preferred)
        add(record, 'worlds', images)

    def extra(id_, title, dimension, category, origin, caption, group, files):
        source = {'villages': 'village_review', 'entities': 'entity_review', 'trial': 'lantern_review',
                  'weather': 'weather_review', 'pixel': 'pixel_review', 'nether': 'nether_review'}[group]
        images = []
        for mode, label, filename in files:
            size = re.search(r'-(\d+)x(\d+)\.ansi$', filename)
            width, height = map(int, size.groups()) if size else (160, 48)
            images.append((mode, label, filename, width, height))
        add(dict(id=id_, title=title, dimension=dimension, category=category, origin=origin,
                 caption=caption, source=f'examples/{source}.rs'), group, images)

    for role in ['cottage', 'workshop', 'farmhouse']:
        extra(f'village-{role}', f'Village {role}', 'Overworld', 'Villages', 'Generated',
              'Native wooden settlement building. The view choices include daytime, a smaller terminal and night lighting. Player and villager positions are staged.',
              'villages', [('overview', 'Day · 160×48', f'{role}-160x48.ansi'), ('small', 'Small · 120×40', f'{role}-120x40.ansi'), ('night', 'Night · 160×48', f'{role}-night-160x48.ansi')])
    for id_, title, filename in [
        ('village-panorama', 'Wooden village · full settlement', 'settlement-480x32.ansi'),
        ('village-square', 'Village square', 'square-200x40.ansi'),
        ('village-garden', 'Village garden and crops', 'garden-160x48.ansi'),
        ('desert-village', 'Desert village · full settlement', 'desert-settlement-480x32.ansi'),
        ('desert-cottage', 'Desert village cottage', 'desert-cottage-160x48.ansi')]:
        extra(id_, title, 'Overworld', 'Villages', 'Generated',
              'Actual generated settlement at the fixed review seeds. Positions and time of day are staged; buildings, crops and roads are generated.',
              'villages', [('panorama' if '480x32' in filename else 'overview', 'Full settlement' if '480x32' in filename else 'Overview', filename)])
    for name, title in [('humanoids', 'Hostile mobs and villagers'), ('farm', 'Farm animals'), ('small', 'Small cave and water creatures'),
                        ('nether', 'Nether creature lineup'), ('props', 'Boat, drops and projectiles'), ('cave', 'Torch-lit creature encounter'),
                        ('night', 'Night creature encounter'), ('poses', 'Damage, crouching and alert poses'), ('farm_states', 'Sheared sheep and alert wolf')]:
        extra(f'entities-{name}', title, 'Nether' if name == 'nether' else 'Overworld', 'Creatures', 'Staged actors',
              'Explicitly staged creature and prop lineup, rendered at their actual game scales. This is an art review rather than a naturally spawned encounter.',
              'entities', [('overview', 'Overview · 200×60', f'{name}-200x60.ansi'), ('normal', 'Overview · 160×48', f'{name}-160x48.ansi'), ('small', 'Small · 120×40', f'{name}-120x40.ansi')])
    for id_, title, poses in [('trial-camp', 'Lantern Run · starting camp', ['camp']),
                              ('trial-combat', 'Lantern Run · combat sequence', ['windup', 'hurt', 'strike', 'follow-through', 'recovery']),
                              ('trial-jump', 'Lantern Run · jumping', ['ascent', 'descent']),
                              ('trial-ravine', 'Lantern Run · ravine route', ['ravine'])]:
        extra(id_, title, 'Overworld', 'Play scenarios', 'Scripted gameplay',
              'Real trial terrain and game updates, with staged positions and scripted actions. Switch poses to inspect combat or airborne presentation; this is not a human playthrough.',
              'trial', [(pose, pose.replace('-', ' ').title(), f'{pose}-160x48.ansi') for pose in poses])
    for name, title in [('rain', 'Rain and sheltered interior'), ('snow', 'Snow and sheltered interior')]:
        extra(f'shelter-{name}', title, 'Overworld', 'Weather & time', 'Staged weather',
              'Supported roof and torch fixture with staged weather. Precipitation is rendered outdoors and clipped by cover.',
              'weather', [('overview', 'Overview', f'{name}-overview-160x48.ansi'), ('detail', 'Detail', f'{name}-detail-160x48.ansi')])
    extra('mining-materials', 'Mining materials, water and torchlight', 'Overworld', 'Play scenarios', 'Staged build',
          'Staged ore deposits, water and supported torches for a readable material inspection. The natural cave scenes are listed separately.',
          'pixel', [('overview', 'Overview', 'cave.ansi')])
    extra('nether-encounter', 'Nether encounter and creature motion', 'Nether', 'Creatures', 'Staged actors',
          'Actual generated cavern, with explicitly staged pigman, blaze and ghast positions. The animation stages ages and attack-ready flags at fixed positions.',
          'nether', [('overview', 'Overview', 'encounter-200x60.ansi'), ('detail', 'Detail', 'encounter-detail-160x48.ansi')])

    animations = []
    for name, directory, pattern, count, duration in [
        ('nether-creatures', args.raw / 'nether', 'motion-*.ansi', 72, [20, 10, 20] * 24),
        ('shelter-rain', args.raw / 'weather/rain-motion', 'frame-*.ansi', 48, 50),
        ('shelter-snow', args.raw / 'weather/snow-motion', 'frame-*.ansi', 48, 50)]:
        screen = Screen(160, 48)
        frames = []
        for path in sorted(directory.glob(pattern)):
            data = path.read_bytes()
            input_hashes[path.relative_to(args.raw).as_posix()] = hashlib.sha256(data).hexdigest()
            screen.feed(data)
            frames.append(raster(screen))
        assert len(frames) == count, name
        filename = name + '.gif'
        frames[0].save(args.output / filename, save_all=True, append_images=frames[1:], duration=duration, loop=0, optimize=True)
        record = next(s for s in scenes if s['id'] == name or s['id'] == name.replace('creatures', 'encounter'))
        record['images'].append(dict(mode='animation', label='Motion', file=filename, columns=160, rows=48))
        animations.append(dict(file=filename, frames=count, limits='Staged renderer samples, not achieved FPS or an emulator recording.'))

    ids = [s['id'] for s in scenes]
    assert len(ids) == len(set(ids))
    coverage = dict(biomes=10, dimensions=['Overworld', 'Nether', 'End'], native_scene_gaps=native['missing'],
                    generation_issue=native['generation_issue'], animations=animations,
                    limits='Coverage follows the current generator and representative gameplay systems; structure studies and art previews are explicitly marked.')
    catalog = dict(scenes=scenes, coverage=coverage, native_seed=native['native_seed'], recipe_seed=42)
    (args.output / 'catalog.json').write_text(json.dumps(catalog, indent=2) + '\n')
    (args.output / 'capture-inputs.json').write_text(json.dumps(input_hashes, indent=2) + '\n')
    data = json.dumps(catalog, separators=(',', ':')).replace('<', '\\u003c')
    (args.output / 'index.html').write_text(PAGE.replace('@DATA@', data))

    # Contact sheets are inspection aids, outside the delivered image catalog.
    qa = args.raw.parent / 'contact-sheets'
    qa.mkdir(exist_ok=True)
    font = ImageFont.truetype('DejaVuSansMono.ttf', 13)
    for page, offset in enumerate(range(0, len(scenes), 16)):
        sheet = Image.new('RGB', (1440, 1024), (16, 18, 28))
        draw = ImageDraw.Draw(sheet)
        for i, record in enumerate(scenes[offset:offset + 16]):
            x, y = i % 4 * 360, i // 4 * 256
            frame = Image.open(args.output / record['images'][0]['file'])
            frame.thumbnail((360, 216), Image.Resampling.NEAREST)
            sheet.paste(frame, (x, y + 30))
            draw.text((x + 4, y + 4), record['id'][:39], font=font, fill=(220, 223, 232))
        sheet.save(qa / f'page-{page + 1}.png')
    print(json.dumps(dict(scenes=len(scenes), images=sum(len(s['images']) for s in scenes), categories=len({s['category'] for s in scenes}), gaps=native['missing'])))


PAGE = r'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Termcraft · World atlas</title><style>
:root{color-scheme:dark;--bg:#10121b;--panel:#191d2a;--line:#333b4e;--text:#e8edf3;--muted:#acb8cb;--accent:#b7dfc7}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:16px system-ui,sans-serif;line-height:1.5}main{max-width:1680px;margin:auto;padding:30px}h1{font-size:clamp(34px,5vw,60px);line-height:1.1;margin:12px 0 18px;letter-spacing:-.03em}h1 span{color:var(--accent)}p{color:var(--muted);max-width:1050px}a{color:var(--accent)}.eyebrow{letter-spacing:.15em;font-size:12px;color:var(--accent);text-transform:uppercase}.stats{display:flex;gap:12px;flex-wrap:wrap;margin:24px 0}.stats span,.badge{border:1px solid var(--line);background:var(--panel);border-radius:5px;padding:6px 12px;font-size:13px}.toolbar{position:sticky;top:0;background:#10121bf5;backdrop-filter:blur(10px);padding:16px 0;z-index:2;border-block:1px solid var(--line);display:flex;gap:12px;align-items:center;flex-wrap:wrap}input,select,button{font:inherit;background:var(--panel);color:var(--text);border:1px solid var(--line);border-radius:5px;padding:9px 12px}input{width:min(100%,310px)}button,a,select{touch-action:manipulation}button{cursor:pointer}button:hover,button[aria-pressed=true]{border-color:var(--accent);color:var(--accent)}:focus-visible{outline:2px solid #f1d299;outline-offset:3px}.worlds{display:flex;gap:6px}.status{margin:18px 0;font-size:14px;color:var(--muted)}.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(330px,1fr));gap:20px}.card{display:flex;flex-direction:column;background:var(--panel);border:1px solid var(--line);border-radius:8px;overflow:hidden}.card>a{display:block;background:#0a0c12;aspect-ratio:5/3}.card img{display:block;width:100%;height:100%;object-fit:contain;image-rendering:pixelated}.card-body{padding:16px}.card h2{font-size:19px;line-height:1.3;margin:10px 0}.card h2 a{color:var(--text);text-decoration:none}.card p{font-size:14px;margin-bottom:0}.meta{display:flex;gap:7px;flex-wrap:wrap;font-size:11px;color:#cad4e3}.meta span{padding:3px 6px;border-radius:3px;background:#242d40}.meta .ow{color:#bce0b6}.meta .nether{color:#ffc09e}.meta .end{color:#d9b5f3}.meta .special{color:#f0d29a}.count{margin-left:auto;font-size:13px;color:var(--muted)}details{border:1px solid var(--line);border-radius:6px;padding:14px 18px;margin:26px 0;color:var(--muted)}summary{cursor:pointer;color:var(--text)}.empty{padding:60px 0;color:var(--muted)}footer{margin-top:36px;padding-top:20px;border-top:1px solid var(--line);font-size:14px;color:var(--muted)}dialog{padding:0;width:min(1540px,96vw);max-height:95vh;background:#141923;color:var(--text);border:1px solid #58647d;border-radius:8px}dialog::backdrop{background:#04060ce8}.modal-head{display:flex;align-items:center;gap:16px;padding:14px 20px;border-bottom:1px solid var(--line)}.modal-head h2{font-size:20px;margin:0;flex:1}.image-wrap{background:#080b11;text-align:center}.image-wrap img{display:block;margin:auto;max-height:64vh;max-width:100%;width:auto;height:auto;image-rendering:pixelated}.modal-body{padding:16px 20px}.modal-controls{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.modal-body p{margin:12px 0}.modal-body details{margin:12px 0;font-size:13px}.pill{font-size:12px;color:var(--muted)}[hidden]{display:none!important}@media(max-width:700px){main{padding:16px}.toolbar{position:static}.worlds{flex-wrap:wrap}.grid{grid-template-columns:1fr}.count{margin-left:0}.modal-head,.modal-body{padding:12px}.image-wrap img{max-height:54vh}h1{font-size:40px}}
</style></head><body><main><div class="eyebrow">Termcraft · current renderer · October 2026</div><h1>Three worlds.<br><span>A complete visual tour.</span></h1><p>Explore the implemented environments, major structures and representative gameplay scenarios. Open a card for full-size views, close-ups, panoramas and motion where available. Generated places, staged scenarios and design studies are labelled throughout.</p><div class="stats" id="stats"></div><div class="toolbar"><nav class="worlds" aria-label="Dimension"><button type="button" data-world="All" aria-pressed="true">All worlds</button><button type="button" data-world="Overworld" aria-pressed="false">Overworld</button><button type="button" data-world="Nether" aria-pressed="false">Nether</button><button type="button" data-world="End" aria-pressed="false">The End</button></nav><label><span class="pill">Find a scene</span><br><input type="search" id="search" placeholder="Village, portal, snow, dragon…"></label><label><span class="pill">Category</span><br><select id="category"><option value="All">All categories</option></select></label><label><span class="pill">Scene source</span><br><select id="origin"><option value="All">All sources</option></select></label><button type="button" id="reset">Reset</button><span class="count" id="count" aria-live="polite"></span></div><p class="status">Overview gives world context. Detail uses the game's closer view. Panoramas and tall context are available for larger structures.</p><div class="grid" id="grid"></div><p class="empty" id="empty" hidden>No scenes match these filters. Try Reset or a broader search.</p><details id="coverage"><summary>Coverage and a generation finding</summary><p>This tour covers all ten current Overworld biomes, the three dimensions, village building roles, both dungeon types, the stronghold rooms, Nether fortress sections, the route-landmark family, End towers/portals/islands and representative survival, creature, weather and UI scenarios.</p><p><strong>Nether route variants:</strong> the current marker-seed arithmetic always selects Shrine. Bridge and reliquary cards are labelled source-derived structure studies. The gallery preserves this finding; gameplay generation was not changed.</p><p>The front-facing dragon is an opt-in art preview, shown over real End terrain. The current playable dragon is separately labelled. The deep lava-layer scene is a cutaway inspection, rather than a safe spawn. All images come from the renderer; the scene labels identify staged positions, light and UI.</p><p><a href="provenance.md">Reproduction and limitations</a> · <a href="catalog.json">Scene catalog</a> · <a href="evidence.json">Build and capture evidence</a> · <a href="../world-refinement-2026-10-15/index.html">All blocks: before / after</a></p></details><footer>Fresh renderer captures, software rasterized from ANSI. Static scenes and staged animation do not measure terminal latency or establish gameplay/art acceptance. <span id="footer-count"></span></footer></main><dialog id="viewer" aria-labelledby="viewer-title"><div class="modal-head"><h2 id="viewer-title"></h2><button type="button" id="close" aria-label="Close scene">Close ✕</button></div><div class="image-wrap"><img id="large" alt=""></div><div class="modal-body"><div class="modal-controls"><button type="button" id="prev">← Previous</button><label>View <select id="view"></select></label><button type="button" id="next">Next →</button><a id="original" target="_blank" rel="noopener">Open original image ↗</a></div><p id="caption"></p><div class="meta" id="modal-meta"></div><details><summary>Location and source</summary><p id="location"></p><p id="source"></p></details></div></dialog><script type="application/json" id="atlas-data">@DATA@</script><script>
const atlas=JSON.parse(document.getElementById('atlas-data').textContent),scenes=atlas.scenes,$=id=>document.getElementById(id),viewer=$('viewer');let world='All',visible=scenes,active=null;
function textElement(tag,text,className){const e=document.createElement(tag);e.textContent=text;if(className)e.className=className;return e;}
function meta(scene){const box=document.createElement('div');box.className='meta';box.append(textElement('span',scene.dimension,scene.dimension==='Nether'?'nether':scene.dimension==='End'?'end':'ow'),textElement('span',scene.category),textElement('span',scene.origin,/study|preview|Inspection/i.test(scene.origin)?'special':''));return box;}
for(const id of ['category','origin']){const field=id==='category'?'category':'origin';for(const value of [...new Set(scenes.map(s=>s[field]))].sort()){const option=new Option(value,value);$(id).add(option);}}
for(const value of [`${scenes.length} scenes`,'3 dimensions','10 biomes',`${scenes.reduce((n,s)=>n+s.images.length,0)} views`])$('stats').append(textElement('span',value));
function render(){const q=$('search').value.trim().toLowerCase();visible=scenes.filter(s=>(world==='All'||s.dimension===world)&&($('category').value==='All'||s.category===$('category').value)&&($('origin').value==='All'||s.origin===$('origin').value)&&`${s.title} ${s.category} ${s.caption} ${s.origin}`.toLowerCase().includes(q));$('grid').replaceChildren();for(const scene of visible){const card=document.createElement('article');card.className='card';const link=document.createElement('a');link.href='#'+scene.id;link.setAttribute('aria-label','Open '+scene.title);const img=document.createElement('img');img.src=scene.images[0].file;img.alt=scene.title;img.loading='lazy';img.decoding='async';link.append(img);link.addEventListener('click',e=>{e.preventDefault();open(scene.id);});const body=document.createElement('div');body.className='card-body';body.append(meta(scene));const h=document.createElement('h2'),title=document.createElement('a');title.href=link.href;title.textContent=scene.title;title.addEventListener('click',e=>{e.preventDefault();open(scene.id);});h.append(title);body.append(h,textElement('p',scene.caption));card.append(link,body);$('grid').append(card);}$('count').textContent=`${visible.length} / ${scenes.length} scenes`;$('empty').hidden=visible.length!==0;}
function image(){const i=Number($('view').value),v=active.images[i];$('large').src=v.file;$('large').alt=`${active.title} — ${v.label}`;$('original').href=v.file;}
function open(id){const scene=scenes.find(s=>s.id===id);if(!scene)return;active=scene;$('viewer-title').textContent=scene.title;$('caption').textContent=scene.caption;$('view').replaceChildren();scene.images.forEach((v,i)=>$('view').add(new Option(`${v.label} · ${v.columns}×${v.rows}`,i)));$('modal-meta').replaceChildren(...meta(scene).children);$('location').textContent=scene.x===undefined?'Positions are documented in the source fixture.':`Player/camera: x=${scene.x}, y=${scene.y}. Native world seeds: ${atlas.native_seed.join(', ')}. Recipe world seed: 42.`;$('source').textContent=`Capture fixture: ${scene.source}. Exact source and image hashes are in evidence.json.`;image();if(!viewer.open)viewer.showModal();history.replaceState(null,'','#'+scene.id);}
function close(){viewer.close();history.replaceState(null,'',location.pathname+location.search);}
function move(delta){const pool=visible.length?visible:scenes;let i=pool.findIndex(s=>s.id===active.id);if(i<0)i=0;open(pool[(i+delta+pool.length)%pool.length].id);}
for(const button of document.querySelectorAll('[data-world]'))button.addEventListener('click',()=>{world=button.dataset.world;for(const b of document.querySelectorAll('[data-world]'))b.setAttribute('aria-pressed',String(b===button));render();});
$('search').addEventListener('input',render);$('category').addEventListener('change',render);$('origin').addEventListener('change',render);$('reset').addEventListener('click',()=>{world='All';$('search').value='';$('category').value=$('origin').value='All';for(const b of document.querySelectorAll('[data-world]'))b.setAttribute('aria-pressed',String(b.dataset.world==='All'));render();});$('close').addEventListener('click',close);$('view').addEventListener('change',image);$('prev').addEventListener('click',()=>move(-1));$('next').addEventListener('click',()=>move(1));viewer.addEventListener('cancel',e=>{e.preventDefault();close();});viewer.addEventListener('click',e=>{if(e.target===viewer){const r=viewer.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)close();}});document.addEventListener('keydown',e=>{if(viewer.open&&e.target.tagName!=='SELECT'){if(e.key==='ArrowRight'){e.preventDefault();move(1);}if(e.key==='ArrowLeft'){e.preventDefault();move(-1);}}});render();$('footer-count').textContent=`${scenes.length} scenes with labelled provenance.`;if(location.hash)open(decodeURIComponent(location.hash.slice(1)));
</script></body></html>'''

if __name__ == '__main__':
    main()
