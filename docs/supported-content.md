# Supported Content

> Version: `0.1.0`
>
> Status: early alpha. This page describes the content that is currently implemented in the codebase today. It is a practical support list, not a promise of full vanilla parity.

## Positioning

- The game is trying to capture a classic early-2012 survival arc in a 2D terminal format.
- A lot is already implemented.
- Not everything from the source formula exists yet, and some implemented systems are still rough.

## Supported Mobs

### Hostile mobs

- Zombie
- Creeper
- Skeleton
- Spider
- Silverfish
- Slime
- Enderman
- Blaze
- Zombie Pigman
- Ghast
- Ender Dragon
- End Crystal

### Passive and neutral mobs

- Cow
- Sheep
- Pig
- Chicken
- Squid
- Wolf
- Ocelot
- Villager

### Vehicle / rideable entity

- Boat

## Supported Item And System Families

The item roster is broad enough to support the main survival loop already. Current implemented families include:

### Basic blocks and materials

- dirt, stone, sand, gravel, cobblestone, planks
- wood, birch wood, leaves, birch leaves
- glass, wool, snow, ice, cactus, sugar cane
- netherrack, soul sand, glowstone, end stone, obsidian

### Ores and resource progression

- coal
- raw iron and iron ingots
- raw gold and gold ingots
- diamond
- redstone dust
- flint

### Tools and weapons

- wooden, stone, iron, and diamond pickaxes
- wooden, stone, iron, and diamond swords
- wooden, stone, iron, and diamond hoes
- wooden, stone, iron, and diamond axes
- wooden, stone, iron, and diamond shovels
- bow
- fishing rod
- flint and steel
- shears

### Utility and placeables

- torch
- crafting table
- furnace
- chest
- bed
- wood door
- ladder
- lever
- stone button
- redstone torch
- redstone repeater
- piston
- TNT
- enchanting table
- anvil
- brewing stand
- bookshelf
- boat

### Food and drops

- bread
- raw and cooked beef
- raw and cooked mutton
- raw and cooked porkchop
- raw and cooked chicken
- raw and cooked fish
- wheat and wheat seeds
- rotten flesh
- bone
- string
- leather
- feather
- egg
- gunpowder
- slimeball
- blaze rod
- blaze powder
- ghast tear

### Buckets, bottles, and potion chain

- bucket
- water bucket
- lava bucket
- glass bottle
- water bottle
- awkward potion
- healing potion
- strength potion
- regeneration potion
- fire resistance potion
- magma cream
- nether wart
- bone meal

### End progression items

- ender pearl
- eye of ender

### Crafting and progression chains currently present

- armor sets: leather, iron, diamond
- books and bookshelves
- paper from sugar cane
- arrows from flint, stick, and feather
- boats
- flint and steel for manual Nether portal ignition

## Important Current Gaps

This list is not exhaustive, but these are examples of content families that are still missing or not yet documented as complete parity:

- many later-era sandbox items and mechanics
- broader transport systems beyond boats
- full polished client/server parity
- full vanilla recipe coverage across every niche utility item

## Public Documentation Guidance

If you publish the repo soon, describe the game like this:

- local single-player survival sandbox first
- early alpha
- broad but incomplete content surface
- experimental networking

That is materially more accurate than presenting it as a finished full-parity clone.
