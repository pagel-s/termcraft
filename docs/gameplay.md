# Gameplay

> Version: `0.1.0`
>
> Status: early alpha. The intended survival loop is in place, but behavior is not yet bug-free.

## Core Loop

The intended progression is:

1. gather wood and basic blocks
2. craft tools and a furnace
3. mine stone and ore
4. survive nights and hostile mobs
5. explore caves, villages, dungeons, and strongholds
6. progress through Nether and End

## Core Controls

| Action | Control |
| --- | --- |
| Move | `A` / `D` or left/right arrows |
| Jump / swim up | `W`, `Up`, or `Space` |
| Sneak toggle | `X` |
| Sneak hold | `Shift` |
| Inventory | `E` |
| Hotbar | `1`-`9` |
| Mine / attack | `Left Click` |
| Place / interact | `Right Click` |
| Use hovered block fallback | `F` |
| Settings menu | `O` |
| Quit / close UI | `Q` or `Esc` |

## Settings And Rules

These are available in-game:

- `P`: cycle difficulty
- `G`: cycle gamerule preset
- `H`: toggle mob spawning
- `J`: toggle daylight cycle
- `K`: toggle weather cycle
- `L`: toggle keep inventory
- `O`: open the settings menu

## Inventory And Crafting

The inventory UI supports more than simple click-swap behavior.

- `Enter`: craft once from the current crafting grid
- `Shift+Enter`: craft max
- `Delete`: clear the crafting grid back into inventory
- left drag: spread one item per crossed slot
- right drag: also spread one item per crossed slot for precise layout
- shift-left-click on crafting output: craft max with mouse

The crafting grid changes based on context:

- inventory crafting: 2x2
- crafting table: 3x3

## Interaction Notes

- Right click places or interacts.
- `F` is the keyboard fallback for hovered-block interaction when terminal mouse support is inconsistent.
- Chests, furnaces, doors, portals, and other interactables use the same interaction path.

## Travel / Test Shortcuts

These exist in the current build and are especially useful for testing:

- `F5`: travel to Overworld
- `F6`: travel to Nether
- `F7`: travel to End
- `F8`: return to spawn
- `F9`: equip a diamond combat loadout

If you do not want these in the eventual public build, remove or gate them before launch.

## Scope Note

- The core gameplay target is local single-player survival.
- Networking exists for development/testing, but it is not yet documented as a polished feature path.
- For the currently implemented item and mob roster, see [Supported Content](supported-content.md).

## Death And Respawn

- death shows a dedicated respawn screen
- `R`, `Enter`, or `Space` respawns after the short delay
- `Q` or `Esc` quits from the death screen
- respawn gives brief grace time

## Saves

The game autosaves during play and persists local progression under `saves/`.
