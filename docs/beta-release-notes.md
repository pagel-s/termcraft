# Next beta: development release notes

**Draft for proposed `v0.2.0-beta.1`; this is not a qualified or published beta.**
Local binaries still report `0.1.0` and archives are marked `dev`. The repository's
`BETA_RELEASE_PLAN.md` and `RELEASE_CHECKLIST.md` track qualification.

## October 19 refinement

- Rigid held tools, faster strike timing and corrected held axe orientation.
- Explicit furnace input/fuel/output slots with persistent contents and cooking.
- Persistent creature breath, lava/fire damage and visible burning; player air HUD
  and armor-independent drowning damage.
- Sight-based hostile targeting, interrupted bow windups behind walls, and corner
  occlusion shared by mining, interactions and projectiles.
- Persistent F4 view; suspended physics on unloaded terrain prevent loose loot/XP
  from falling into invalid save coordinates. Finite old loose-item falls outside
  the End recover at the same X above real terrain.
- Fourteen-second dragon charge, breakup and portal reveal with no camera shake.
- Native v11 / server v5 / protocol 18; genuine v9/v10 and v3/v4 migration fixtures.


## Throwing, damage feedback and the End finale

Press **V** to throw one held item, or **Shift+V** for its stack. In inventory,
select the item first. Stack counts, tool wear and enchantments survive pickup
and saving. A two-second delay prevents immediately collecting the throw.
The same normal action is available through MCP `drop_item`.

Ghast and blaze fireballs can be returned with a melee hit. Returned shots use
swept creature/terrain hits and damage creatures instead of the player.
Cactus contact responds immediately with a cooldown; regeneration no longer
masks the hit. Environmental damage now triggers the player hurt flash, and
co-op peers show it too. Held swords are slightly shorter at every pixel zoom.

Persistent villagers sleep at unloaded chunk boundaries. Finite villagers that
already fell below the world in an older running build return home, preserving
identity, health and age. Save validation remains strict and includes offending
coordinates/velocity when it rejects an entity.

The dragon patrols closer to the pillar crowns. Its fourteen-second death sequence
retains the dragon silhouette as it fractures, expands into light/shock fronts,
and highlights the return portal. Co-op snapshots carry the sequence; its clock
and origin already persist in checkpoints. The camera keeps the encounter framing
throughout the effect. Rewards and portal creation still occur once on death.

Nether materials/backdrop apply to existing terrain after restarting the updated
executable. New Nether floor fissures need newly generated chunks. The raised
slim obsidian towers are in the **End**; recognized untouched older generated
towers upgrade in place, while player-modified footprints remain intact.

[Actual combat and finale captures, including the seven-second animation](design/combat-and-finale-2026-10-17/README.md).

## Survive and build with friends over SSH

Host a shared world across the Overworld, Nether and End and join with
`termcraft join user@host --player alice`. Players have independent inventories,
crafting, workstations, combat and death/respawn; they share terrain, mobs, items
and storage. Progress survives reconnect and orderly server restart. The launcher
manages the SSH tunnel and stable reconnect identity and restores your terminal
when the connection ends.

Slow readers now have bounded output queues drained independently of simulation,
so a stalled connection cannot hold the world in a socket write. Queue overflow
disconnects that reader, and grouped disconnects share a checkpoint save.

Use an existing SSH account with forwarding permission and matching builds.
Keep the game port on loopback; no public hosting/accounts are provisioned.
See [co-op](multiplayer.md) and [joining](distribution.md).

## Ask your game master to change the world

Press **T** and ask for a battle, mobs, snow, weather/time or an accessible
building. Your model plans a bounded change while play continues. The engine
validates the whole plan, protects existing builds/players, and rejects stale or
cancelled requests. Shared requests are host-controlled and visible to everyone.

Use `/status` for client contact, request phase and deadline.
Connect an existing MCP-capable Codex/Claude client with `/connect` and
`termcraft mcp config`; Termcraft needs no second API key. Keep that client task
actively waiting: connecting does not wake an idle chat. An optional provider
executable is also supported. `/cancel`, `/pause` and conditional structure
`/undo` give you control; `/auto` opts into occasional events. Ordinary play
needs no model. See [agents and generated worlds](agents.md).

Plans use existing blocks/mobs: local biome ambience changes appearance and
spawning, rather than reshaping terrain. Waves are finite; structures need flat
empty natural ground and fit 16×12. No newly generated mechanics/quests/bosses.

## Independent co-op portal travel

Nether/End portals now move only the interacting player. Friends can follow the
same linked frame or remain in another dimension. Terrain, entities, hazards,
boats and client streaming stay local to that dimension. Saved identities reopen
in their own dimensions, and shared portal links/dragon completion survive restart.
The End encounter is shared; credits and respawn remain individual. Late commands
from a previous dimension cannot place/mine there after travel. Matching protocol-18
builds are required. Server checkpoint v5 reads v1–3; native checkpoint v11 reads v1–9.

## Faster rendering and clearer play

Lighting uses descending brightness buckets and cached attenuation; reusable
ANSI encoding writes each frame once. In controlled local comparisons with
identical visuals, 200×60 torch-cave renderer p95 falls from 1.811–1.940 ms to
0.672–0.706 ms. This excludes terminal presentation and is not a visible-FPS or
all-machines claim. [Measurements and methodology](performance.md).

The HUD has a readable background, separate sprint/target rows, mining progress
and reach feedback. Targets retain terrain colors, remote players have ID labels,
and cloud/End sparkle animation is quieter. **F3** shows rolling timing/cache
diagnostics. Exploration evicts distant saved chunks around all players; failed
writes retain data in memory for retry.

## Terminal pixel art and finer control

The active view is now labelled, with `=`/`+` and `-` for direct zoom. Full Detail
remains available at compact terminal sizes and in the End; enlarging the terminal
font no longer silently swaps it for a reduced figure. Holding a direction walks;
double-tap-and-hold sprints. Delayed legacy OS repeat no longer triggers sprint
after an input timeout. The HUD includes the sprint gesture.

The close view uses a shared pixel atlas for blocks and creatures across all
dimensions. Detail shows 8×8 pixel blocks and proportioned humanoids;
cutaway interiors, terrain edges and fresh hut roofs improve structure readability.
Existing saved structures keep their geometry. **F4** switches close/wide views.

The follow-up art audit gives species separate silhouettes, natural body scales,
coordinated palettes and explicit equipment/damage/alert states. Continuous shared
lighting, ground-contact shadows and opaque terrain occlusion ground creatures in
the scene. Transparent torches/windows no longer retain old glyph fragments.
See the [before/after audit](design/art-audit-2026-10-11/provenance.md).

The Ender Dragon gains a larger horned silhouette, articulated wing poses, visible
ribs, curved tail and clearer eyes. Its health/phase banner preserves the armor
HUD; targeting moves clear of the airborne boss. This refines presentation while
retaining the existing encounter AI. Head/neck/torso hits now follow the figure;
decorative wings and tail do not add attack geometry. Fresh End towers use
proportioned columns and accessible base tunnels, with a wider boss view.

All 135 item types now share pixel icons between inventory, crafting, hotbar and
world drops. Crop growth, hydrated soil, spawners and redstone states are easier
to distinguish. Slabs/stairs share half-block geometry across movement, shadows,
occlusion and terrain hit checks. Half-block steps are walkable in both directions;
full-block climbs require jumping. New village roofs retain continuous support
above their half-height eaves. [Full catalog and world review](design/2d-refinement-2026-10-12/index.html).

Fresh ordinary Overworld generation now builds a connected cottage, workshop and
farmhouse with reachable loft storage, beds, a covered market gate, graded road and
irrigated garden. Distinct roofs/materials, framed windows and real loft lighting
replace the cramped single-hut layout. Partial lofts preserve cutaway back walls;
villagers pair opposite doors into homes and seek outdoor entry space by day.
Existing saved chunks retain their geometry. See the
[village comparison and route](design/village-refinement-2026-10-12/provenance.md).

After native terminal feedback, the default is a wider pixel overview; F4 cycles
wide pixels and optional close detail. Plain-key taps remain bounded; confirmed
key-release controls retain run+jump chords until actual release or focus/menu
reset. An individually lost key-up requires releasing again or changing focus.
Right-click dragging does not repeatedly toggle doors.
Synchronized frame updates and flush-aware pacing reduce avoidable partial painting
and output pressure. The sky uses stable designed clouds/sun/stars, and mining/hit
poses use a proportionate arm. Emulator latency and human quality acceptance still
need direct playtesting. See the [terminal correction](feel-and-art.md).

Rain and snow use distinct downward-moving particles, share roof/cave clipping
across views, and respect ceilings above the camera. Snowy biomes identify snow
in the HUD. Character art is unchanged by these corrective passes.

Classic walking is now 6.4 blocks/second, with sharper reversal and shorter jumps
when a held jump is released early. Collision-resolved local presentation and a
camera jump dead zone reduce visible stepping and unnecessary camera movement.
**F3** reports keyboard release support; repeat-only terminals retain limitations.
See the [captures and test route](feel-and-art.md). Human acceptance of final art
and movement feel remains open.

## A focused encounter and building payoff

`termcraft trial` creates or resumes **Lantern Run**, a separate world with a
camp, traversal course, three ruin guards, supply chest and a bridge/signal goal.
It needs no LLM and works with the usual named-world co-op hosting. Fresh shared
identities receive one starter kit; progress and emptied chest slots survive save
and reopen. This is an authored gameplay slice for judging the candidate's feel.

Melee enemies warn before striking and commit their facing; skeletons visibly
prepare and lock aim before firing. Hits interrupt attacks and preserve knockback
through AI recovery. Melee selects the nearest pointed-at body across species;
arrows sweep their full path and hit actors before later walls. Player hurt,
weapon swings and airborne legs now have distinct visual states. Idle targeting
panels no longer cover the scene. Chest-only inventory edits are saved correctly.

[Try the route and review the captures](feel-and-art.md#lantern-run-play-the-complete-loop).
Automated CLI completion establishes that the loop is playable under normal rules;
it does not establish human enjoyment or a polished full survival campaign.

## Easier inventory, food feedback and reactive blocks

The bag separates main storage from its numbered hotbar, shows full item names
on hover and keeps partial merge remainders selected. A recipe book supports
one-click crafting, Shift-click batches, wheel/Page Up/Page Down navigation and
Tab switching on compact terminals. Manual grids use whole-stack left placement,
one-item right placement, dragging and quick transfer in both solo and co-op.

Recipes preserve their ingredients when the complete output would not fit.
Completed furnace output waits safely for space, including after a restart.
Full-bag drops settle on the ground and can be picked up after space opens, or by
a nearby friend. Food/health icons round fractional values consistently, and a
visibly full hunger bar prevents wasting another meal on a tiny drain.

Redstone ore lights up when hit, used or stepped on and fades after activation.
Lava and flint/steel can start local fires in flammable material; water puts them
out. Lava/fire destroy loose items and flames respect fire resistance. New ore/fire
block variants and batch-craft commands require matching **protocol 18** builds.
[Controls and rules](gameplay.md#inventory-and-crafting).

## More predictable movement and safer saves

Partial-water swimming no longer stalls while holding up. Eight obstacle routes
compare local and shared motion at every tick. Release braking preserves later knockback, ladders support held climbing and
sneak holding, sneaking protects ledges during residual movement, and fall damage
includes the final landing tick. Swept collision covers fast walls/ceilings.
Actual terminal/network input feel still needs playtesting.

Native saves preserve the full player session, including crafting grids,
enchantments, potion timers and furnace work. Corrupt/unreadable existing chunks,
seeds and progression are rejected rather than silently regenerated. Failed
writes remain dirty, unsafe dimension travel is cancelled, and fatal native-loop
errors attempt a private **partial** recovery copy.

Interrupted world saves now roll back terrain, chests, player/server inventories,
entities and sidecars together across all dimensions. Startup recovers the last
completed checkpoint; later progress may be lost. Keep hidden checkpoint files
with backups and use a matching build for recovery. Process-kill and injected
storage-error tests pass; physical power-loss durability remains unqualified.
See the [recovery guide](world-format.md#recoverable-world-checkpoints).

Waterfalls now fall vertically until landing, runoff follows nearby drops, and
unsupported flow drains after removing or blocking supply. Water/lava mixing is
independent of chunk order. Thin flows have matching visible and physical depth:
breathing, immersion and boat height follow the surface, and ankle-deep runoff
allows land jumps. Water also fills contained basins toward its source height
and rises through connected submerged passages. Spillways limit each pool, with overflow into
lower terraces; opening/closing drains lowers/refills them. Full supplied water
is distinct from a persistent source, including after saving and in co-op.
These retain source-block rules rather than conserved fluid volume.
[Basin review](design/water-basins-2026-10-16/README.md).

**Back up saves before upgrading.** Progression v11 reads v1–10, then upgrades on
save; older builds cannot reopen v11. Entities and portal links share the player
checkpoint; co-op inventories and entities share server checkpoint v5. Animals,
villagers, boats, loot/XP and partial encounters resume across restarts and portal
travel. Clocks and item ages pause while offline. Writes remain atomic per file,
not across the whole world; recovery copies contain only loaded terrain in the
current dimension. [Save/recovery guide](world-format.md).

## Consistent creature collision and room presentation

Flying Ghasts/Blazes use full-body, gravity-free terrain sweeps, including shaped
blocks and fast hit impulses; spawn clearance now checks their full footprint.
Villagers preserve recoil during home/shelter steering, and NPC fluid movement
no longer inherits a human's jump hold. Farm animals wait through hit recovery
before voluntary wall jumps.

Room cutaways require a continuous roof and complete side boundaries, removing
patchy false walls from open ruins/shelters. Stone interiors use a masonry palette;
transparent props preserve the backdrop. Small terminals retain a complete trial
goal plus contextual climbing, looting and building controls. [Review](feel-and-art.md#further-physics-and-room-refinement).

## Let your LLM play or design a world

MCP exposes 16 stepped world/player tools and six live director tools. Optional
agents observe, move, mine, craft and build under normal rules, then save and
leave for a human. Typed deterministic world recipes support validation/repair
and named isolated saves. No model service or always-on agent is bundled.
Simultaneous human/LLM player entry remains deferred.

## Installation and current limits

Native development archives include an executable, documentation, license,
manifest and checksum; Rust is unnecessary at runtime. Linux extraction/launch and default Lantern Run launchers
are checked locally, including paths with spaces and no Rust in PATH. The
four-platform workflow builds development artifacts; interactive OS/terminal
qualification remains a separate release gate.

Shared play supports eight connections, 64 saved identities and independent travel
across all three dimensions. Protocol **17** requires matching builds. Classic survival through the dragon,
sustained human co-op, remote SSH/WAN behavior, live model quality and all-platform
terminals remain release gates. Automated results do not establish a completed
human campaign or a viral release.

The October 16 correction adds visible misses, distinct held-tool/bow/eat/use poses,
consistent pixels at every colour zoom, light-aware cave spawns, walk-in Nether
portals, source-preserving fluids and direct seed placement on farmland. Co-op
action poses require matching **protocol 18** clients and server. See the
[interaction review](design/interaction-review-2026-10-16/README.md).

Following ore/farming feedback, fresh terrain has more iron and rare ores and coal
veins capped at 12 blocks. New village gardens expose their irrigation at soil
level, with a walkable slab crossing. Buckets no longer fall back to hidden faces.
See the [ore and irrigation review](design/ore-irrigation-tuning-2026-10-16/README.md)
for the census, comparison images and native walkthrough.

A subsequent depth-balance pass replaces broad patches with independently seeded,
spaced deposits. Coal and iron taper near bedrock; deep regions have a more varied
mix of small redstone, gold and diamond finds. Recipe worlds share this balance
instead of using a separate scattered-ore formula. Existing chunks are preserved.
[Six-seed ordinary/recipe census and regression checks](design/deep-ore-balance-2026-10-16/README.md).

Entity/encounter persistence also retains partial Lantern Run kills and surviving
guards, game-master wave membership, destroyed End crystals and dragon health.
See the [persistence review](design/entity-persistence-2026-10-16/README.md).

The Nether/End refinement adds a leaner frontal dragon, predictive obstacle
clearance, reachable swoops and smaller generated pillars. Untouched pillars from
the preceding generator upgrade in old saves; modified pillars are preserved.
Nether backdrop/material improvements apply immediately, while contained volcanic
fissures appear in new cavern chunks. [Comparisons, flight and compatibility](design/nether-end-refinement-2026-10-16/README.md).

The [clearance follow-up](design/dragon-clearance-2026-10-17/README.md) raises slim
pillars to 9–12 blocks and fixes repeated crown approaches, overhang escapes and
swoops toward sheltered players. Shorter untouched saved pillars upgrade safely.
