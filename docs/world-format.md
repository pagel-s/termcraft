# Saving and recovery

Save formats can change between development builds. Back up the whole save
directory before upgrading, including hidden files. Close every game/server
using that world before making the backup.

## Save locations

- Linux: `$XDG_DATA_HOME/termcraft/saves`, or `~/.local/share/termcraft/saves`.
- macOS: `~/Library/Application Support/termcraft/saves`.
- Windows: `%APPDATA%\termcraft\saves`.

`TERMCRAFT_SAVE_DIR` overrides the location. Classic survival uses the save root;
named worlds and Lantern Run use `worlds/world-ID/`. The default shared world
uses `multiplayer/default/`. Named co-op servers use their named world directory.
The saved view lives in `view.json`; co-op reconnect profiles live under `clients/`.

A world contains terrain files, player/server records, metadata and possibly a
hidden `.world-checkpoint/` directory. Keep them together. A `.session.lock`
prevents two processes from writing the same world. Close a local game before
hosting that world.

## Compatibility

The current build writes native checkpoint version **11**, server version **5**
and chunk version **2**, with network protocol **19**. Native loaders accept the
older native formats through version 10; server loaders accept versions 1–4.
All co-op participants need matching builds. Older executables may reject saves
written by newer builds.

Saved terrain and player edits are preserved. New terrain, villages and ore
layouts appear in fresh chunks. Weather, world clocks, entities and encounters
resume from saved state; the world does not advance while offline.

## Interrupted saves

Autosave uses a recoverable world checkpoint. If a write is interrupted, the next
checked startup rolls the world back to the last completed save. Progress since
that save can be lost. A failed recovery is reported and its files are preserved;
corrupt data is not silently replaced by new terrain.

After a crash, verify the previous owner has stopped. Remove only its stale
`.session.lock` if necessary, then reopen with the matching build. **Do not delete
`.world-checkpoint/`** to force a load. Resolve storage-space or permission errors
and retry. Keep an independent backup; process-crash recovery does not guarantee
survival of disk or power failures.

## Emergency copies

A fatal native-loop error attempts to write `termcraft-recovery-<random>` in the
system temporary directory. The terminal reports its absolute path after cleanup.
Copy it somewhere durable before temporary files are cleared.

This is a **partial emergency copy**, not a full-world backup. It includes the
currently loaded chunks in the current dimension, player progression, seed/recipe
and valid metadata, with `RECOVERY.txt` describing the limits. Unloaded terrain
and terrain in other dimensions are absent. Saved entities cannot recreate that
missing terrain. Preserve the original world and recover into a separate copy.

## Recovering into a copy

1. Close the game/server and any other save owner. Retain the complete terminal
   error and the reported `termcraft-recovery-...` path. Copy that temporary
   directory somewhere durable before the OS cleans it.
2. Locate the original world directory: the save root for classic play,
   `worlds/world-ID` for a named world, or `multiplayer/default` for the default
   shared world. Make a complete backup, then a **second** copy for recovery.
   Work only on the second copy. Native recovery is not a multiplayer backup.
   If the copy has `.session.lock`, verify its original owner is gone before
   removing **the copy's** stale lock. Leave the original and backup untouched.
3. Before overlaying anything, open **the second copy** with the matching build
   and quit normally, allowing its pending checkpoint to roll back. Otherwise
   startup could roll your newly overlaid emergency files back too. If recovery
   fails, preserve the copy and error; do not delete its journal to force entry.
4. Read `RECOVERY.txt` and inspect which dimension/chunks were captured. Overlay
   only the recovered chunk/progression/seed and metadata files
   that you intend to restore. Keep the copy's unloaded chunks and other
   dimensions. Do not copy leases or reconnect identities.
5. Open the overlaid world as a standalone copy with the matching build:

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
or missing terrain/entity history from before the last checkpoint. A successful
open is not proof of consistency.
