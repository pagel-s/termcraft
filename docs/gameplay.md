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
| Performance diagnostics | `F3` |
| AI game master chat (local / co-op host) | `T` |
| Quit / close UI | `Q` or `Esc` |

The unreleased development build keeps detailed counters behind `F3`. See
[Performance and Foundation Checks](performance.md) for timing definitions and
local measurements. Save warnings appear in the HUD; travel is cancelled if the
source world cannot be saved.

The experimental native chat accepts requests for battles, mobs, local ambience,
weather and structures with a configured director. Typing pauses single-player;
Enter submits and resumes play while the provider works. Esc closes chat and
Ctrl+C cancels a pending request. See [AI game master setup](agents.md#in-game-ai-game-master)
for the offline demo, model configuration, proactive controls and current limits.

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

### Glowstone and portals

Glowstone is a solid, placeable light block. It lights nearby terrain and figures
with a warm glow, remains bright in caves and drops glowstone when mined. Removing
it removes its light on the next rendered frame.

For a Nether portal, build a 4-block-wide, 5-block-high obsidian outline with a
2×3 empty opening. The four corners are optional (ten obsidian blocks minimum).
Hold Flint and Steel and right-click or press `F` on the frame to ignite it.
Use the active opening or its obsidian frame within reach to travel; merely
standing in it does not transfer you. The target hint displays `R-click / F: travel`
or `Portal recharging...` during the two-second cooldown. Breaking a required frame
block extinguishes the whole opening. Repair the frame and ignite it again.

An End portal uses twelve frame blocks around a 2×2 opening. Fill each with an
Eye of Ender, then use the opening or frame. The bedrock return portal uses the
same interaction. Portal surfaces cannot be mined; their breakable frames can.
Reusing a Nether portal preserves nearby buildings, lights and chest contents;
safe End arrival sites are reused without clearing existing builds. Falling
momentum and accumulated fall distance reset during travel.

Dimension travel works in single-player and co-op. In co-op only the interacting
player travels; friends can follow or remain in another dimension. Terrain,
entities, inventories and saved portal links remain separate, with one shared
dragon encounter for players in the End. Use matching protocol-13 clients/server.

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

The game autosaves during play and persists local progression under the per-user save directory. Set `TERMCRAFT_SAVE_DIR` to override it.

## Movement and readability updates in the development build

Releasing movement now applies the same brake in native play, server simulation
and MCP actions. Duplicate releases do not erase later knockback. Holding jump
climbs ladders continuously; holding sneak stays on the ladder. Sneak also stops
residual sideways momentum at a supported ledge. Swept wall/ceiling collisions
prevent high-speed tunnelling, and landing damage includes the final partial tick
of a fall. These improve existing movement; fluids, boats and passive mobs still
need broader hands-on qualification.

The HUD uses a solid backdrop for health/food and key information. Sprint and
target information have separate rows. A mining bar shows progress; target tint
retains terrain color and out-of-reach targets say so. Co-op avatars display their
player ID above their head. F3 keeps diagnostics available. Cloud drift and End
sparkles update less often to reduce distracting repainting.

## Art and movement review

F4 switches between the close pixel-art view and the wide view. Use a color
terminal at least 140×44 for the detailed view; 160×48 is a useful starting size.
F3 reports keyboard release support. Try walking, abrupt reversal, tap/held jumps,
low ceilings and wall landings before comparing the camera in both views.
See [Movement and art review](feel-and-art.md) for current captures, the test route
and remaining limitations.
