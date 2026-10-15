# Agents and generated worlds

This is an **unreleased experimental build**. The native game's chat can request
changes to the current world through your existing LLM client or a configured
provider executable. Separately, an
MCP client can design worlds and play a saved single-player session through
structured tools. Humans can then open that same save in the terminal.
Human co-op includes shared host-controlled game-master chat and an SSH joining
command. Simultaneous human–agent player entry remains a later convenience. LLM play is opt-in: use your existing
client and model; Termcraft does not start an LLM player or select a model service.

## In-game AI game master

Build with `cargo build --release`, then use `target/release/termcraft` below
(or the updated `./termcraft` executable in this working checkout).

To try the chat without an API key, use the included **offline demo fixture**:

```sh
TERMCRAFT_DIRECTOR="$PWD/scripts/demo_director.py" ./termcraft
# Or open a named world:
TERMCRAFT_DIRECTOR="$PWD/scripts/demo_director.py" ./termcraft world play YOUR_WORLD_ID
```

This fixture is a keyword-based test program, **not an LLM**. Press **T**, type
one request, then **Enter**. Try “make this area snowy”, “build a tower” and
“give me a hard battle”. Each demo request performs one preset effect; it cannot
interpret arbitrary combinations. Structures need flat, empty ground ahead; if
placement fails, move somewhere clearer and retry. Battles require non-Peaceful
difficulty (change with P). Snow is local ambience, not replaced terrain.

For an actual model, the same optional Python runner used for whole-world
generation also accepts current-session plans:

```sh
export TERMCRAFT_MODEL_PROVIDER=openai   # or anthropic
export TERMCRAFT_MODEL=your-supported-model-id
# Supply OPENAI_API_KEY or ANTHROPIC_API_KEY securely in your environment.
export TERMCRAFT_DIRECTOR="$PWD/scripts/generate_world.py"
./termcraft
```

No provider/model/key is selected automatically. API access is separate from
Claude/Codex MCP access or a coding-client subscription. Provider adapters have
offline contract tests; live model quality and latency have not been qualified.
Use a trusted executable wrapper to connect another backend. On Windows an
executable wrapper is also needed to start Python; that platform is unqualified.

Chat pauses local single-player and releases held movement/attack inputs. Enter
sends and resumes play; inference runs in the background. Esc closes chat without
cancelling. Ctrl+C in chat or `/cancel` cancels pending work. Results wait while
chat, inventory or settings are open. A world/dimension change, death or moving
over 48 blocks from the request area prevents a stale plan from applying.

| Request | Current effect and bounds |
| --- | --- |
| A battle / more mobs | Existing zombies, skeletons, spiders, cows, sheep, pigs or chickens. At most 24 per request, 12 per spawn action, population cap 120. Hostiles form a finite wave; no new bosses, quests or reward scripts. |
| Weather / time | Clear, rain, storm; day, night, dawn or dusk. Normal weather/daylight cycles continue. |
| Snow / desert / forest / plains / mountains | Persistent local biome ambience within radius 8–48 columns. Affects precipitation, appearance and biome-based spawning; does not grow trees, reshape hills or replace blocks. |
| A tower / building | One structure, up to 16×12 blocks, with a two-block-high edge entrance. Needs flat natural ground and an entirely empty footprint. Existing blocks/builds are preserved. |

The model receives compact terrain/player context, active encounter information,
explicit preferences and six recent request/results. It returns a typed plan of
at most four actions. The engine validates and preflights the whole plan before
changing the world. Unsupported or blocked requests produce a chat explanation.
The local and shared Overworld are supported. In co-op only the host may issue
requests; everyone sees the results and the server continues while chat is open.
Local authoring requires the request area to be within x=±99900.

Proactive events are **opt-in**. Type `/auto exploration`, `/auto building` or
`/auto combat`. After travelling 64 blocks into a new area, a healthy player can
trigger planning at least two minutes apart, with at most three attempts per
session. Modal UI, an active wave and health below 70% block the trigger. Only
combat preference permits unsolicited hostiles/storms. These categories are
explicit preferences; the director does not learn a player personality.

Use `/pause` (or `/request-only`) to stop proactive planning while keeping manual
requests available. `/help` lists controls. `/undo` conditionally removes the most
recent structure's unchanged blocks, preserving edited blocks, chests, furnaces
and crafting tables. It does not undo mobs, weather or ambience. Eight structure
records are retained. `/reset` backs up preferences/history and starts fresh,
without undoing world changes.

Structures, ambience, preferences and recent context persist. Spawned mobs and
active encounters are session-only and are not restored on reopening. Weather
and time persist through the normal progression save, then follow game cycles.
Saves are atomic per file, not across the entire plan/world; reported save errors
need attention before closing. See [World Format](world-format.md).

A director invocation receives JSON on stdin with `protocol: 1`, `task: "director"`,
`prompt`, `context`, `schema`, `example` and `instructions`. It returns one plan
object on stdout. `termcraft director schema` prints the schema/example. Invalid
designs get one repair attempt; process/transport failures stop immediately. Each
attempt has a 90-second deadline, 32 KiB accepted plan limit and bounded process
output. The bundled runner makes one API request per attempt, so one intention
can incur up to two provider requests. Cancellation stops the local process and
ignores its result; it cannot reverse work/charges already accepted by a provider.

## Connect your existing LLM

For an MCP-capable client, print a configuration entry for this exact executable:

```sh
./termcraft mcp config                 # JSON entry for clients using mcpServers
./termcraft mcp config --format toml   # Codex config.toml entry
```

Merge the entry into your client's MCP configuration and enable the server.
The command includes an absolute executable path. If `TERMCRAFT_SAVE_DIR` is set,
it includes that save directory as an absolute path too. It prints configuration;
it does not change client settings, start a model or copy API credentials. Keep
the executable at that location, or regenerate the entry after moving it.

Ask the connected client: “Use Termcraft to enter my world, gather wood, craft
planks and build a shelter. Observe first, use normal game rules, then save and
leave.” There is no requirement to keep an LLM running for ordinary gameplay.

### Use your client as the live game master

1. Start the native game (or join as the first co-op player). Press **T**, enter
   `/connect`, and press Enter. This explicitly enables local authoring tools.
2. In your connected LLM client, ask:

   > Act as my Termcraft game master. Call director_connect, then use director_wait
   > to await requests. Read each pending request and its context/schema, create a
   > supported plan, and submit it with director_apply and the exact request_id.
   > Observe until it applies or fails, explain the result, then wait for the next
   > request. Do not enter_world or write save files directly.

   For a named world pass `{"world_id":"YOUR_WORLD_ID"}` to `director_connect`.
   For the default server pass `{"multiplayer":true}`. For classic local play use
   `{}`. The game and MCP process must use the same save directory.
3. Return to the game and request a battle, environment or structure through T.
   Alternatively tell the LLM your intention directly; it calls `director_request`
   first to obtain the same bounded request context.

This uses your client's model access. Termcraft needs no separate API key, and
its simulation remains the only world writer. The bridge listens on loopback,
uses a random bearer credential stored locally with Unix mode `0600`, and is
revoked by `/disconnect`, changing worlds or the co-op host leaving. Do not share
`.director-bridge.json`. A remote game requires the MCP process on the server
host, for example a trusted SSH stdio MCP command running that host's Termcraft
executable with the same save directory.

The six live tools are `director_connect`, `director_observe`, `director_wait`,
`director_request`, `director_apply` and `director_cancel`. Wait blocks up to
1–50 seconds without repeated model round trips, then returns a request or a
`timed_out` result. **The LLM task must remain active:** connecting does not wake
an idle Codex/Claude chat. Tell it to keep waiting, or resume it when you submit a
request. Each request expires after 90 seconds. The game continues while waiting.
Client tool approval prompts and model latency can consume that deadline.

In game, `/status` reports whether the bridge or provider is configured, the last
client contact, and the pending request's elapsed time/deadline. It distinguishes
waiting for an LLM client from a submitted plan waiting to apply. A recent contact
does not prove the model is still working; keep the client task waiting. Close
chat/inventory/settings so a received plan can apply. The live observation also
includes the request phase, elapsed seconds and timeout.

Submission returns `accepted`, not proof of application. Observe `last_result`
for `applied`, `failed` or `cancelled`; modal UI and stale-area checks still apply.
Invalid plans can be repaired before the deadline. Duplicate/cancelled request IDs
are rejected. In co-op, the first player is host; departure cancels pending work,
revokes access and pauses proactive events before another player becomes host.
Guests can read shared chat but cannot change the world through it.

Live authoring and stepped player tools are separate modes. Leave a stepped world
before connecting as director; restart the MCP process to switch back to player
mode. Neither mode starts an always-on AI player.

A local interactive check on 11 October used the current Codex task to author
snow/dawn, farm animals, a watchtower and a battle through real stdio MCP. All
four applied without repairs in 6.0–13.6 seconds including orchestration/capture.
An earlier request expired during geometry inspection, demonstrating that long
client turns count against the 90-second deadline. This is one automated Linux
session, not an uncoached client registration test or model comparison; Claude
and broader prompt fidelity remain unqualified. [Raw report](assets/qualification-2026-10-11.json).

### Claude Code or Codex CLI

Build from this checkout with `cargo build --release`. Register the absolute path
to the resulting executable, so the integration does not depend on its working
directory:

```sh
codex mcp add termcraft -- /absolute/path/to/termcraft mcp
claude mcp add --transport stdio termcraft -- /absolute/path/to/termcraft mcp
```

These use a local stdio MCP server. Termcraft itself needs no provider key in this
mode; model access and tool approvals come from the client you use. Configuration
details: [Codex MCP](https://developers.openai.com/codex/mcp) and
[Claude Code MCP](https://code.claude.com/docs/en/mcp).

Try this prompt in the connected client:

> Use Termcraft to create a new world called frostvale: snowy plains, a forest
> to the west, and a ruined stone tower to the east with an accessible doorway.
> Read world_schema first, validate your design, and repair any errors before
> creating it. Enter the world, gather wood, craft planks, and build a small
> shelter using normal game rules. Save and leave when finished.

The tools include world schema/validation/creation/listing, entering/leaving a
world, observing nearby tiles/entities and the complete inventory, moving,
jumping, mining/attacking, selecting items, interacting/placing, listing/crafting
recipes, clicking inventory/container slots, closing inventory, respawning, and saving. They do not expose developer
loadout or teleport commands.

`act` advances 1–40 ticks, with 20 ticks per simulated second. Time pauses between
tool calls. Inputs release at the end of each call, while velocity and unfinished
mining progress follow the engine's usual rules. Observe again before acting;
coordinates increase right and down. Observations expose a bounded local map,
including nearby underground blocks; they are not a strict line-of-sight view.
Interactions still obey the normal reach, collision, ingredients, and line of
sight checks. Recipe IDs are session-local lookup indexes, not stable content IDs.

`inventory_click` uses the ordinary open inventory/container UI. Pass `slot` and
`kind`: 0 swaps with the cursor, 1 splits, and 2 quick-moves. Slot numbering starts
with container slots when a chest is open. `observe` includes open chest contents,
nearby entity health, and the optional trial objective/progress. `act(jump: true)`
queues takeoff as well as holding the jump, so normal grounded jumps work.

To let your client try the authored route, first run `termcraft trial create demo`,
then ask it to enter `demo`, observe, follow the objective, collect chest supplies,
build the bridge and light the signal under normal rules. Stepped tools remain
separate from a live human session.

After the agent leaves:

```sh
termcraft world list
termcraft world play frostvale
```

One process owns a generated world at a time. If a process crashes, a
`.session.lock` may remain. Verify that the world is closed before removing that
file. Successful game-changing tool calls save progress; save failures return a
tool error. MCP stdout contains only newline-delimited JSON-RPC.

## Generate from a terminal prompt

The CLI can run a trusted designer executable, validate its answer, and return
validation errors for up to three design attempts:

```sh
termcraft world generate frostvale "Snowy ruins and a forest to the west" \
  --generator /absolute/path/to/designer
termcraft world play frostvale
```

Alternatively set `TERMCRAFT_WORLD_GENERATOR` to the executable path. The runner
receives one JSON object on stdin: `protocol` (1), `prompt`, `schema`, `example`,
`instructions`, `previous_design`, and `validation_error`. It must return one JSON
recipe on stdout; diagnostics belong on stderr. It receives data, not a shell
command. Use an executable wrapper if arguments are needed.

Each attempt has a 90-second deadline and limits of 256 KiB stdout and 8 KiB
stderr. Transport/process failures stop immediately; invalid JSON or invalid
designs can be repaired. No world is committed until validation passes. An
existing world ID is never overwritten.

An optional Python 3 runner is included at `scripts/generate_world.py`. It uses
the OpenAI Responses API or Anthropic Messages API, selected explicitly through
environment variables:

```sh
export TERMCRAFT_MODEL_PROVIDER=openai   # or anthropic
export TERMCRAFT_MODEL=your-supported-model-id
# Supply OPENAI_API_KEY or ANTHROPIC_API_KEY securely in your environment.
export TERMCRAFT_WORLD_GENERATOR=/absolute/path/to/scripts/generate_world.py
termcraft world generate frostvale "Snowy ruins and a forest to the west"
```

The included script runs directly as an executable on Unix. On Windows, use an
executable runner wrapper that starts Python with this script. Windows and macOS
builds of this prototype have not yet been qualified.

The runner makes one API request per attempt. API access is separate from your
coding client's account/subscription; up to three requests may incur provider
charges. No provider, model, or key is selected automatically. Models must support
the respective JSON-schema/tool interface. The provider adapters have offline
contract tests; live provider requests have not been qualified.

## What the generator can build

The recipe controls a deterministic seed, plains/forest/desert/tundra/mountain
terrain, regional terrain overrides, and named landmarks made from ordered block
rectangles. `air` rectangles hollow rooms. Landmark coordinates are relative to
the terrain surface at the landmark's x coordinate. The engine supplies caves,
ores, bedrock, and a protected starter tree. Read the current supported block list
with `termcraft world schema` or the `world_schema` tool.

Validation enforces known fields/blocks, bounded coordinates and work, disjoint
terrain regions, a clear spawn at x=0, and an intact starter tree. It does not prove
landmark accessibility, good gameplay, theme fidelity, or a complete survival
campaign. This generator does not place the classic generator's villages,
strongholds, portal resources, or progression structures automatically. Nether
and End terrain still use the existing generator. Models cannot add executable
code, new block types, mobs, physics, or game rules through a recipe.

For a provider-free example:

```sh
termcraft world schema > world-schema.json
# Copy the "example" object from that output into frostvale.json.
termcraft world create frostvale frostvale.json "Example snowy ruins"
termcraft world play frostvale
```

Generated saves live under `<save-dir>/worlds/world-<ID>/`, with `world.json`,
`request.json` (the original prompt), dimension chunk files, and
`player_progression.bin`. `TERMCRAFT_SAVE_DIR` controls the parent save directory.
Classic saves remain at their existing location. Dimension travel retains the
selected world's save root. Do not edit recipes after a world has been played:
already saved chunks would disagree with newly generated chunks. Recipe format 1
and the tools are experimental and may change before the beta.
