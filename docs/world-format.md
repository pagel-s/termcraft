# World Format

> Version: `0.1.0`
>
> Status: early alpha. This format description reflects the current implementation, not a frozen external save contract.

This page documents the current on-disk save layout as implemented in the repo today. Treat it as a technical description, not a stable public save-format guarantee.

## Save Directory Layout

Current local save files live in the configured save directory. By default this is the per-user data directory:

- Linux: `$XDG_DATA_HOME/termcraft/saves`, or `~/.local/share/termcraft/saves` when `XDG_DATA_HOME` is unset
- macOS: `~/Library/Application Support/termcraft/saves`
- Windows: `%APPDATA%\termcraft\saves`

Set `TERMCRAFT_SAVE_DIR` to override this location. Existing repo-local `./saves` files are copied into the default user data directory on first run.

```text
<save-dir>/
  overworld_chunk_<x>.bin
  nether_chunk_<x>.bin
  end_chunk_<x>.bin
  player_progression.bin
```

Chunk saves are namespaced by dimension. Older legacy overworld chunk filenames are still migrated on load.

The unreleased generated-world prototype uses independent directories at
`<save-dir>/worlds/world-<ID>/`. Each contains a declarative `world.json` recipe,
`request.json` recording the original prompt, and its own dimension chunk and
player progression files. A `.session.lock` prevents concurrent writers. See
[Agents and Generated Worlds](agents.md) for commands and format limitations.

The experimental native game master can add two sidecars to either classic or
named world roots:

- `environment.json`, format version 1: up to 16 nonoverlapping local biome
  ambience regions. These affect appearance, precipitation and biome-based
  spawning; they do not replace terrain blocks or change the world recipe.
- `director.json`, format version 1: the chosen proactive preference, the last
  six request/results and conditional edit records for the last eight structures.
  `/undo` removes only unchanged structure blocks, preserving storage/workstations
  and subsequent player edits. `/reset` backs up this sidecar and resets the
  preferences/history, leaving world changes in place.

Both sidecars use synced temporary files and rename. Corrupt environment data
blocks checked world startup; corrupt director metadata disables authoring until
explicit reset while allowing ordinary play. Neither these files nor the chunk/
progression files form a whole-world transaction. Structures and ambience survive
reopening. Weather/time use progression persistence and then follow normal game
cycles. Spawned mobs and the director's active finite encounter are session-only;
they are not restored when reopening a save. The proactive cooldown and attempt
budget reset on entry; the selected preference persists.

## Chunk Geometry

Chunk dimensions are fixed in code:

- width: `32` blocks
- height: `128` blocks

These values come from `src/world/chunk.rs`.

## Chunk File Envelope

Chunk files are versioned and compressed.

Current structure:

1. 4-byte magic header: `MCCF`
2. 1-byte codec tag: `1` means deflate
3. deflate-compressed bincode payload

Current chunk payload version: `2`

Payload fields:

- `version: u8`
- `blocks: Vec<BlockType>` with `32 * 128 = 4096` entries
- `chests: Vec<ChestSaveData>`

Each chest entry stores:

- local chunk `x`
- local chunk `y`
- the chest `Inventory`

Chunk writes sync the temporary file before renaming it. A failed write leaves
the in-memory chunk dirty for retry. Each compressed input and expanded payload
is bounded to 8 MiB when loading.

The unreleased build distinguishes an absent file from an unreadable or corrupt
chunk. Only absence creates new terrain. Native and MCP world owners report
loading failures and preserve the existing file. Corrupt world seed and player
progression files are also rejected by the checked startup path. Runtime chunk
load errors stop native simulation; edits since the last successful save may
remain unsaved at the source. On a fatal native-loop error the game attempts a
separate emergency recovery directory; see below.

## Player Progression File

`player_progression.bin` is a separate versioned bincode payload.

Current progression version: `8`

The progression payload currently contains:

- player position and velocity
- grounded/facing/sneak state
- health, hunger, drowning, burning, fall distance
- complete player session, including inventory, armor, crafting grid, enchantment
  levels, potion timers and furnace job/progress
- selected hotbar slot
- spawn point
- current dimension
- time of day and weather state
- dragon defeat / credits state
- movement profile
- portal cooldown
- paired Nether portal anchors, including their source dimension and destination
- XP state
- difficulty and gamerules

Version 8 is a plain bincode envelope with its version, legacy progression/world
metadata, the complete player session and a sorted list of portal links. Links
share the atomic checkpoint write, so returning through an underground or elevated
portal after reopening reuses its original frame. Version 7 had the complete
session but did not persist portal links. Versions 1–7 remain readable and are
upgraded on the next save. Held inputs, mining/bow actions and transient boat/
fishing references are released on reopening. Invalid full sessions are rejected
without replacing the original file. It does not use the chunk `MCCF` envelope.

Progression writes also sync a temporary file before rename. Autosave errors are
shown in the native HUD and retried on the next save interval. Dimension travel
is cancelled when saving the source world fails. Individual atomic files do not
form a transaction across chunks, progression and dimensions; filesystem/directory
durability and crash recovery still need qualification.

## Compatibility Notes

The codebase already contains migration paths for older chunk and progression versions.

Important constraints if you change the format:

- appended enum variants are preferred to preserve bincode compatibility where feasible
- chunk block counts must still match the fixed chunk geometry
- progression loaders reject unknown versions

## Public Documentation Guidance

If this project becomes public-facing and external tools are expected to read saves, do this first:

1. define a compatibility policy
2. freeze a documented save schema
3. decide whether raw bincode remains acceptable for external tooling
4. add sample save files for testing

## Emergency recovery copies

A fatal native-loop error attempts to write `termcraft-recovery-<random>` in the
system temporary directory, reports its absolute path after restoring the terminal,
and leaves the original save files intact. On Unix the recovery directory is
private (`0700`). It contains all **currently loaded chunks in the current
dimension**, player progression, environment/seed/recipe data and valid director
metadata, plus `RECOVERY.txt` explaining the limits.

This is a **partial emergency copy**, not a full-world backup. It excludes unloaded
regions, other dimensions, loose items and mobs. Preserve a separate backup of the
original world, then inspect/overlay recovered files into a copy; never replace
the whole save directory with this partial snapshot. A failed copy is reported
alongside the original error. Disk failure can also prevent recovery. Temporary
directories may be cleaned by the OS; copy a useful recovery elsewhere promptly.

Regular backups and whole-world transaction/crash qualification remain necessary.
Classic local play now also holds `.session.lock`; verify no owner is running
before manually removing a stale lock after a crash. A live authoring bridge does
not acquire a second world lease or edit files independently.

### Recovering into a copy

1. Close the game/server and any stepped MCP owner. Retain the complete terminal
   error and the reported `termcraft-recovery-...` path. Copy that temporary
   directory somewhere durable before the OS cleans it.
2. Locate the original world directory: the save root for classic play,
   `worlds/world-ID` for a named world, or `multiplayer/default` for the default
   shared world. Make a complete backup, then a **second** copy for recovery.
   Work only on the second copy. Native recovery is not a multiplayer backup.
3. Read `RECOVERY.txt` and inspect which dimension/chunks were captured. Overlay
   only the recovered chunk/progression/seed/environment/recipe/director files
   that you intend to restore. Keep the copy's unloaded chunks and other
   dimensions. Do not copy leases, bridge credentials or reconnect identities.
4. If the copy has `.session.lock`, verify its original owner is gone before
   removing **the copy's** stale lock. Leave the original and backup untouched.
5. Open the recovered world as a standalone copy with the matching build:

   ```sh
   TERMCRAFT_SAVE_DIR=/absolute/path/to/recovered-world ./termcraft
   ```

   `recovered-world` must directly contain `player_progression.bin`, chunk files
   and (for a generated world) `world.json`. The checked loader reads this recipe
   from the root. Do not point this command at the parent `worlds` directory.
6. Check position, inventory/crafting/armor, nearby builds/storage, weather,
   dimension travel and save/reopen. If loading reports corruption, preserve the
   offending file and error; do not replace it with new terrain. Resolve the
   underlying storage problem before returning to normal play.

Recovery combines files from different points in time. Inspect neighboring
chunks and progression carefully; it cannot restore a whole-world transaction
or session-only mobs/items. A successful open is not proof of consistency.
