#!/usr/bin/env python3
"""Offline fixture for trying native chat. This script is not an LLM."""
import json
import sys

request = json.load(sys.stdin)
assert request.get('task') == 'director', 'This fixture only accepts director requests'
prompt = request['prompt'].lower()
actions = []
if request.get('context', {}).get('proactive'):
    actions = [{'type': 'weather', 'weather': 'rain'}]
elif any(word in prompt for word in ['battle', 'hard', 'mobs', 'enemies']):
    actions = [{'type': 'spawn', 'mob': 'zombie', 'count': 4, 'side': 'west', 'distance': 16},
               {'type': 'time', 'phase': 'night'}]
elif any(word in prompt for word in ['tower', 'structure', 'castle']):
    def rect(x, y, width, height, block):
        return dict(x=x, y=y, width=width, height=height, block=block)
    actions = [{'type': 'structure', 'name': 'Demo tower', 'side': 'east', 'distance': 24,
                'blocks': [rect(0, -6, 7, 6, 'stone_bricks'), rect(1, -5, 5, 5, 'air'),
                           rect(0, -2, 1, 2, 'air'), rect(3, -1, 1, 1, 'chest')]}]
elif any(word in prompt for word in ['snow', 'tundra', 'desert', 'forest']):
    terrain = 'tundra' if 'snow' in prompt or 'tundra' in prompt else 'desert' if 'desert' in prompt else 'forest'
    actions = [{'type': 'environment', 'terrain': terrain, 'radius': 32},
               {'type': 'weather', 'weather': 'clear' if terrain == 'desert' else 'rain'}]
elif 'storm' in prompt or 'rain' in prompt:
    actions = [{'type': 'weather', 'weather': 'storm' if 'storm' in prompt else 'rain'}]
print(json.dumps({'message': 'Offline demo fixture.' if actions else
                 'Offline demo: try snow, desert, forest, battle, tower, rain or storm.',
                  'actions': actions}))
