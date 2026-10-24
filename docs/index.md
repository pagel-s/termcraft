# termcraft — Terminal Reborn

**Survive, build and explore a block world in your terminal. Bring friends over SSH.**

Termcraft is a 2D sandbox survival game written in Rust. Gather wood, build a
shelter, mine and smelt better tools, explore the Nether and defeat the Ender
Dragon. Terminal pixel art, shaped terrain and room cutaways adapt the world to
a side-on view.

> These pages describe the `0.2.0-beta.1` candidate on `beta/terminal-reborn`.
> Source builds and native archives report that version. Publication is pending;
> the published `0.1.0` alpha predates these changes.

![Generated village cottage in the beta renderer](assets/termcraft.png)

## Start playing

- [Installation](installation.md): native Linux, Windows and macOS archives, or a source build.
- [Gameplay](gameplay.md): movement, inventory, crafting, farming and portals.
- [Multiplayer setup](multiplayer.md): one computer, one network or separate networks, with connection checks.
- [Safe hosting](hosting.md): a rootless Linux server, restricted network access, backups and restore.
- [SSH joining](distribution.md#join-through-ssh): play remotely through an existing SSH account.
- [Release notes](https://github.com/pagel-s/termcraft/blob/beta/terminal-reborn/RELEASE_NOTES.md): changes since the published alpha and current limits.

Launch **Lantern Run** for a focused combat, traversal and building route, or
start classic survival for the open world. The route has its own save.

## What is in the beta

- Coordinated pixel art for blocks, items and creatures across all three dimensions.
- Detail by default; saved Overview/Wide selections are respected. Use **-** for more world context.
- Refined walking, sprinting, jumping, ladders, collision and swimming.
- A recipe book, manual crafting, explicit furnace slots, storage and item dropping.
- Generated villages, caves, strongholds and Nether fortresses; functional portals.
- Source-height water basins, irrigation, lava/fire, drowning and animal shore escape.
- A front-facing dragon, slim pillars and a fourteen-second defeat sequence.
- Shared building and combat with independent inventories and saved reconnect profiles.

## Before upgrading

Back up your full save directory, including hidden checkpoint data. Existing
terrain remains intact; fresh village geometry and ore layouts need new chunks.
Newer checkpoints cannot be opened by older executables. See
[save formats and recovery](world-format.md).

The four-platform pipeline builds and tests native archives. Automated checks do
not replace manual terminal testing or full survival/co-op playthroughs. See
[distribution status](distribution.md) and [supported content](supported-content.md).

## Project

[Media credits](media-credits.md) · [GitHub](https://github.com/pagel-s/termcraft)

Unofficial fan project. Not affiliated with Mojang or Microsoft.
For feedback, contact `pagel.sebastian.1@gmail.com` or open a GitHub issue.
