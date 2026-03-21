# World Format

> Version: `0.1.0`
>
> Status: early alpha. This format description reflects the current implementation, not a frozen external save contract.

This page documents the current on-disk save layout as implemented in the repo today. Treat it as a technical description, not a stable public save-format guarantee.

## Save Directory Layout

Current local save files live in `saves/`:

```text
saves/
  overworld_chunk_<x>.bin
  nether_chunk_<x>.bin
  end_chunk_<x>.bin
  player_progression.bin
```

Chunk saves are namespaced by dimension. Older legacy overworld chunk paths are still migrated on load.

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

Chunk writes are atomic via a temporary file and rename.

## Player Progression File

`player_progression.bin` is a separate versioned bincode payload.

Current progression version: `6`

The progression payload currently contains:

- player position and velocity
- grounded/facing/sneak state
- health, hunger, drowning, burning, fall distance
- inventory and armor slots
- selected hotbar slot
- spawn point
- current dimension
- time of day and weather state
- dragon defeat / credits state
- movement profile
- portal cooldown
- XP state
- difficulty and gamerules

Unlike chunk files, the progression file is currently plain bincode rather than the `MCCF` deflate envelope.

Progression writes are also atomic via a temporary file and rename.

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
