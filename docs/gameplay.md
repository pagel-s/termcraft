# Gameplay

> This guide describes Termcraft `0.2.0-beta.1`.

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
| Sprint | Tap, release, then quickly hold the same direction again |
| Jump / swim up | `W`, `Up`, or `Space` |
| Sneak toggle | `X` |
| Sneak hold | `Shift` |
| Inventory | `E` |
| Hotbar | `1`-`9` |
| Drop one / stack | `V` / `Shift+V` |
| Mine / attack | `Left Click` |
| Place / interact | `Right Click` |
| Use hovered block fallback | `F` |
| Settings menu | `O` |
| Performance diagnostics | `F3` |
| Zoom in / out | `=` or `+` / `-` |
| Cycle world view | `F4` |
| Player chat / solo commands | `T`; `Enter` sends, `Esc` closes |
| Quit / close UI | `Q` or `Esc` |

**F3** shows input mode and rolling simulation/render timings. Save warnings appear
in the HUD; travel is cancelled if the source world cannot be saved. In co-op,
the tick counter measures local prediction, not the remote server. Terminal font
zoom is independent of the selected game view.

Hold a direction once to walk. Double-tap and hold it to sprint; releasing,
reversing, sneaking or dropping to three food icons stops sprinting. Shift is
sneak, not sprint. The HUD shows `Sprint ON` / `OFF`. A legacy input timeout
clears the pending double tap so delayed OS repeat does not start sprinting.
Intentional double-tap sprint requires directional key-up events (`keys:release`
in F3); the repeat-only fallback cannot reliably distinguish that gesture.

## Settings And Rules

F5–F9 travel/loadout shortcuts are disabled by default. In **T**, `/gamemode`
privately toggles them for your player and unlocks the **Travel/loadout** setting
in **O** for the current session. `/gamemode on` and `/gamemode off` also work.
This applies in both solo and co-op; it does not switch survival rules to Creative.

In solo, these are available in-game:

- `P`: cycle difficulty
- `G`: cycle gamerule preset
- `H`: toggle mob spawning
- `J`: toggle daylight cycle
- `K`: toggle weather cycle
- `L`: toggle keep inventory
- `O`: open the settings menu

In multiplayer, the host changes difficulty and shared rules in **O**. Select a
row with the arrow keys and press **Enter** to change it. Difficulty cycles
Peaceful → Easy → Normal → Hard. Guests cannot change these shared settings.

## Surface travel and nights

New Overworld terrain has longer dry stretches, fewer river crossings, gentler
slopes and banks that descend toward water. Oceans, rivers and wetland pools
remain; bring building blocks for crossings. Saved chunks keep their terrain.

Hostile mobs can spawn on unlit dry ground at night. Outdoor attempts stay on the
surface, keep at least twelve blocks from players, and use a closer range suited
to the side view. Cave populations share a total limit but do not consume the
entire surface refill allowance. Lit areas remain protected. In **O** settings,
check that difficulty is above Peaceful and mob spawning is enabled if nights
seem empty. In solo, **P** cycles difficulty and **H** toggles mob spawning;
in multiplayer, the host uses **O**.

## Mining and ore depth

Fresh terrain uses small, spaced ore deposits. Coal and iron supply early and
middle-depth mining, then taper near bedrock. Redstone, gold and diamond become
more relevant deeper down, in smaller deposits. Ordinary and custom recipe worlds
share the distribution, including redstone in recipe worlds. Going to the bottom
does not keep increasing every ore's abundance.

This applies to newly generated chunks. Saved terrain and already mined areas
keep their contents. For an immediate comparison, use a new world or explore
beyond previously generated terrain.

## Farming and breeding

Use a hoe on grass or dirt, then right-click the tilled soil with wheat seeds.
Water within four blocks horizontally and one block vertically keeps the soil
hydrated. Wheat needs light: outdoors it grows during daylight; torches or other
lights let indoor farms grow at night. Stay near the field so it is simulated.
Growth has seven stages and usually takes a few minutes on watered, lit soil.
Bone meal finishes a crop immediately.

Harvest ripe wheat to get one wheat and **two or three seeds**: keep one seed to
replant and use the surplus to expand. An early harvest returns only one seed.

Hold **wheat** to attract cows, sheep, or pigs; hold **wheat seeds** for chickens.
Animals follow visible nearby players, including guests in multiplayer. Right-click
an adult with its food to feed it. Feed two of the same species, keep them close,
and their hearts show a thirty-second breeding window. They need about three
seconds together to produce one young animal. Walls and water prevent breeding.
Parents then have a five-minute cooldown; young animals mature after ten minutes
of nearby simulated play. Babies are smaller, follow adults, cannot breed or be
sheared, and do not give meat, wool, or XP when killed. Young chickens do not lay eggs.

Fed animals and their offspring persist when you leave and when the server restarts.
Unloaded farms pause their movement and lifecycle timers. Breeding is limited to
128 farm animals per dimension. Sprinting and jumping use more food than walking;
idle drain and the cost of holding swim-up are much lower than earlier beta builds.

## Inventory And Crafting

Open the bag with **E**. Main storage is above the numbered hotbar. Hover an
item to read its name; the footer identifies the stack you are moving and its
remainder after a partial merge.

- Left-click a stack, then its destination: move, merge or swap it. Left-click
  into an empty crafting cell moves the whole stack; right-click places one.
- Right-click without a selected source splits a stack into an available slot.
  Drag from a selected stack to distribute one item per crossed slot.
- Shift-click quickly transfers between storage, hotbar, armor, grid or chest.
- The recipe book crafts directly from bag supplies. Click for one result batch;
  Shift-click crafts as much as the available ingredients and output space allow.
- Scroll the book with the wheel or **Page Up/Page Down**. **Tab** toggles it;
  compact terminals show either the book or bag. A table unlocks 3×3 recipes.
- For manual 2×2 / table 3×3 patterns, **Enter** takes one output and
  **Shift+Enter** or Shift-click on the output crafts the maximum.
- **Delete** returns the grid to the bag when it all fits. Closing a container
  returns grid items and leaves overflow on the ground.

To throw an item, select its hotbar slot and press **V** for one or **Shift+V**
for its stack. Inside inventory, click the item first, then use the same keys.
Thrown tools retain wear and enchantments. Everyone waits two seconds before
picking up a thrown item; throw away from lava or fire if you want to keep it.

Right-click a furnace to open **INPUT**, **FUEL** and **OUTPUT** slots. Click a
bag stack and its destination to move it; right-click places one, and Shift-click
routes ingredients/fuel into the appropriate slot or collects results into the bag.
Coal, logs, planks and sticks work as fuel. The progress bar and heat indicator
show cooking; cooking pauses if fuel runs out or the output is blocked. On wide
terminals the recipe book remains a shortcut for loading a batch; compact
terminals keep the three slots visible. Closing/reopening or saving preserves
queued input, fuel, cooked output and the in-progress job. Each placed furnace has
its own storage. Everyone using that furnace shares its slots and cooking progress,
including after disconnecting or restarting the server. Breaking it spills its
remaining contents. Older personal furnace queues transfer when first opening a furnace.

Right-click a bed in the Overworld to set your respawn point. At night, solo play
skips to dawn. In multiplayer, every living player currently in the Overworld needs
their own bed; the status shows how many are sleeping. Move or jump to leave the
bed. Damage, a broken bed or dimension travel interrupts sleep; disconnected players
and players in other dimensions do not block dawn. Beds explode in the Nether/End.
Sleeping requires the daylight cycle to be enabled.

In multiplayer, **Q** or **Esc** asks before leaving. **Enter/Y** confirms;
**Esc/N** keeps playing. Quitting a session started with `termcraft host` also
stops its server, so everyone disconnects.

Crafting refuses to consume ingredients when its complete output would not fit.
Full-bag drops settle on the ground instead of orbiting the player; they become
collectible when space opens. A nearby friend with capacity can collect them.
Lava and fire destroy loose items. Unloaded terrain suspends loose-item/XP physics.

Air bubbles appear below the status panel as breath runs out. Fully submerged
players and breathing creatures have fifteen seconds of air, then take damage
every second. Leaving water replenishes air gradually; armor does not prevent
drowning. Squid breathe water and suffocate on land. Undead do not need air.
Lava damages ordinary creatures and players and leaves an afterburn; water
extinguishes it. Blazes, ghasts and zombie pigmen resist fire/lava.

**F4** (and **+/-**) view selection persists in `view.json` in the save root.
Terminal font zoom is independent of the selected game view.

## Interaction Notes

- Right click places or interacts.
- `F` is the keyboard fallback for hovered-block interaction when terminal mouse support is inconsistent.
- Chests, furnaces, doors, portals, and other interactables use the same interaction path.

### Food, ore activation and fire

Food can fill the last hunger point. The HUD rounds fractional hunger upward to
its displayed half-icon, so a tiny simulation drain after eating no longer shows
an immediately missing half. Further meals are refused while that display is full.

Redstone ore glows and lights nearby terrain when hit, used or stepped on. The
emission fades about five seconds after the last activation; mining requirements
and drops match unlit ore. It does not supply power to redstone wiring.

Lava ignites exposed wooden blocks, leaves, wool and other flammable materials;
submerged wood is consumed by direct lava contact too. Fire spreads locally
through exposed fuel, burns it away, primes TNT, and goes out when its fuel is
spent or water reaches it. Flint and steel can light a supported empty cell;
portal ignition still takes priority on a valid frame. Netherrack supports lasting
fire. Stone and chests are not fuel. Flames damage players unless protected by
fire resistance. Only active loaded regions advance this simulation.

### Glowstone and portals

Glowstone is a solid, placeable light block. It lights nearby terrain and figures
with a warm glow, remains bright in caves and drops glowstone when mined. Removing
it removes its light on the next rendered frame.

For a Nether portal, build a 4-block-wide, 5-block-high obsidian outline with a
2×3 empty opening. The four corners are optional (ten obsidian blocks minimum).
Hold Flint and Steel and right-click or press `F` on the frame to ignite it.
Stand inside the active opening for two continuous seconds, or use the opening/frame
within reach to travel immediately. Walking or sprinting past it does not trigger
automatic travel. Leaving the opening resets the delay. Active uprights sit behind the 2D walking plane; unlit or broken
frames remain solid. Arrival does not automatically bounce you back. The target hint displays `R-click / F: travel`
or `Portal recharging...` during the two-second cooldown. Breaking a required frame
block extinguishes the whole opening. Repair the frame and ignite it again.
Obsidian resists normal blasts, including a direct fireball impact on the frame.

An End portal uses twelve frame blocks around a 2×2 opening. Fill each with an
Eye of Ender, then use the opening or frame. The bedrock return portal uses the
same interaction. Portal surfaces cannot be mined; their breakable frames can.
Reusing a Nether portal preserves nearby buildings, lights and chest contents;
safe End arrival sites are reused without clearing existing builds. Falling
momentum and accumulated fall distance reset during travel.

Dimension travel works in single-player and co-op. In co-op only the interacting
player travels; friends can follow or remain in another dimension. Terrain,
entities, inventories and saved portal links remain separate, with one shared
dragon encounter for players in the End. Use matching protocol-23 clients/server.

Hold an Ender Pearl and right-click or press `F` to throw toward the cursor.
The visible teal pearl follows an arc with a purple trail. Aim higher for longer
throws; distant cursor positions choose a direction rather than an instant
destination. The first terrain or creature impact teleports only the thrower,
with room checked for the player's body on the incoming side of the surface.
Landing costs five health points and resets falling momentum. Throws have a
one-second cooldown. Other players see the flight and purple impact sparks.
Death, disconnecting, dimension travel or closing the world cancels pending pearls;
they are not restored into another connection after loading a save.

Incoming ghast and blaze fireballs can be hit with a normal left-click attack
while within 2.6 blocks. Aim at the projectile: a successful return reverses its
flight, deals 10 damage to the first creature hit, and cannot pass through walls.

## Personal travel shortcuts

These shortcuts work in solo and co-op after you enable them with `/gamemode on`
in **T**. They start disabled for each new session:

- `F5`: travel to Overworld
- `F6`: travel to Nether
- `F7`: travel to End
- `F8`: return to spawn
- `F9`: equip a diamond combat loadout

`/gamemode off` disables them again. This setting affects only your player.

## Scope Note

- Classic survival and the separate Lantern Run route work locally.
- Co-op supports shared building/combat and independent travel across dimensions;
  use matching builds and see [setup and limitations](multiplayer.md).
- For the currently implemented item and mob roster, see [Supported Content](supported-content.md).

## Death And Respawn

- death shows a dedicated respawn screen
- `R`, `Enter`, or `Space` respawns after the short delay
- `Q` or `Esc` quits from the death screen
- respawn gives brief grace time

With **keepInventory OFF**, respawning clears your inventory, equipped armor
and experience. Items are currently removed rather than dropped at the death
location. With **keepInventory ON**, they are retained. Change this rule in
**O: settings**; in multiplayer, only the host can change it for the world.

## Saves

The game autosaves during play and persists local progression under the per-user save directory. Set `TERMCRAFT_SAVE_DIR` to override it.

## Choosing a view

Fresh setups start in **Detail**, with the full character artwork. A valid saved
view takes precedence. Use `-` for Overview, then Wide, and `=` or `+` to zoom
back in. `F4` cycles Overview, Wide and Detail. The header names the active view.
Each color view retains its resolution when the terminal font or window changes: Detail stays detailed at
80×24, 120×40 and larger sizes, including in the End. A smaller window shows less
world instead of silently replacing the figure. Use Overview/Wide for arena
context; 160×48 is a useful starting size. `NO_COLOR` retains the explicitly
labelled glyph mode and disables color-view zoom.
## Irrigating a field

Dig a one-block-deep trench beside farmland and pour a water bucket into the open
cell. Water within four blocks hydrates the soil. New village gardens show a
channel at soil level between two crop beds, with a half-block slab crossing.
Older saved fields with shallow buried irrigation remain hydrated.

A bucket aimed at a solid block uses the facing surface only. If that surface is
occupied, clear a visible trench or aim at another exposed surface; the bucket
will stay full rather than pouring underneath or behind the block. Click farmland
with seeds to plant directly above it.

## Water and lava flow

Fluids fall first. A waterfall spreads where it lands, rather than fanning out
sideways through the air. On supported ground, runoff prefers the nearest drop;
on a flat floor it can spread both ways. Uncontained water runoff travels up to
seven horizontal blocks, Overworld/End lava three, and Nether lava seven. Lava
outside the Nether also updates more slowly.

Contained water fills from the bottom toward the surface height of its source.
Connected vessels reach the same surface through a submerged tunnel; a wall
blocks that connection. Water cannot rise above its source. Lower outlets limit
the level, and filled pools spill into lower terraces. Opening a drain lowers
the pool; closing it lets the source refill it. Basin filling extends beyond the
seven-block runoff range.

A placed bucket creates a source. Picking up that source drains the flow it fed.
Two water sources can refill the supported gap between them, so remove both to
drain an infinite-water arrangement. Natural pools may contain many source cells.
Water touching lava from above or the side cools the lava into obsidian (source)
or cobblestone (flow); lava falling into water forms stone.

Flowing water has a visible shallow surface. Immersion, breathing and boat height
use that surface too; ankle-deep runoff allows a normal ground jump. Crops washed
away by a flow drop their harvest/seeds. Filled basin cells remain supplied water,
so collecting the sole source or sealing off its supply makes them recede too.
This retains the source-based game
model: water volume is not conserved after source removal. Hydraulic levels use
loaded geometry; unknown chunk edges act as outlets. Finite-volume displacement,
waterlogged slabs/stairs and physical currents are not modeled.

### Building into water and animal swimming

A solid block can replace water beside a support. If the aimed water cell has
no support, placement follows the aim to a submerged bank or floor within normal
reach. It still requires a real support and cannot place inside either player.
Water continues flowing around the placed block.

Cows, sheep, pigs, chickens, wolves and ocelots avoid walking into water/lava,
paddle upward if submerged and seek a nearby reachable bank. They can climb a
bank one block above the surface. This is local steering: animals can still drown
under sealed roofs or where no air is reachable.
