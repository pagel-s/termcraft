//! Provider-independent game tools shared by the stdio MCP adapter and local clients.
use super::{command::ClientCommand, state::GameState};
use crate::world::chunk::CHUNK_HEIGHT;
use crate::world::item::Recipe;
use crate::world::recipe::WorldRecipe;
use crate::worlds::{WorldLease, WorldStore};
use serde::Deserialize;
use serde_json::{Value, json};

pub struct AgentSession {
    pub id: String,
    pub state: GameState,
    _lease: WorldLease,
}

pub struct AgentTools {
    pub store: WorldStore,
    pub session: Option<AgentSession>,
    director: Option<super::director_bridge::Client>,
}

impl Default for AgentTools {
    fn default() -> Self {
        Self::new(WorldStore::default())
    }
}

impl AgentTools {
    pub fn new(store: WorldStore) -> Self {
        Self {
            store,
            session: None,
            director: None,
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        if let Some(session) = &mut self.session {
            session
                .state
                .try_persist_world_and_progress()
                .map_err(|e| format!("save failed: {e}"))?;
        }
        Ok(())
    }

    pub fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let result = match name {
            "director_connect" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Connect {
                    #[serde(default)]
                    world_id: Option<String>,
                    #[serde(default)]
                    multiplayer: bool,
                }
                let args: Connect = decode(arguments)?;
                if self.session.is_some() {
                    return Err("leave_world before connecting to a live game master".into());
                }
                let root = match args.world_id {
                    Some(id) => self.store.open(&id)?,
                    None if args.multiplayer => crate::save_path::save_dir()
                        .join("multiplayer")
                        .join("default"),
                    None => crate::save_path::save_dir(),
                };
                let client = super::director_bridge::Client::connect(&root)?;
                let observed = client.call(super::director_bridge::Command::Observe)?;
                self.director = Some(client);
                Ok(observed)
            }
            "director_observe" => {
                empty(arguments)?;
                self.director_call(super::director_bridge::Command::Observe)
            }
            "director_wait" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Wait {
                    #[serde(default = "wait_seconds")]
                    timeout_seconds: u8,
                }
                fn wait_seconds() -> u8 {
                    45
                }
                let args: Wait = decode(arguments)?;
                if !(1..=50).contains(&args.timeout_seconds) {
                    return Err("wait timeout needs 1–50 seconds".into());
                }
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_secs(args.timeout_seconds as u64);
                loop {
                    if self.director_call(super::director_bridge::Command::Poll)?["ready"] == true {
                        break self.director_call(super::director_bridge::Command::Observe);
                    }
                    if std::time::Instant::now() >= deadline {
                        break Ok(json!({"timed_out":true,"pending":null}));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
            "director_request" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Request {
                    prompt: String,
                }
                let args: Request = decode(arguments)?;
                self.director_call(super::director_bridge::Command::Request {
                    prompt: args.prompt,
                })
            }
            "director_apply" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Apply {
                    request_id: String,
                    plan: Value,
                }
                let args: Apply = decode(arguments)?;
                self.director_call(super::director_bridge::Command::Apply {
                    request_id: args.request_id,
                    plan: args.plan,
                })
            }
            "director_cancel" => {
                empty(arguments)?;
                self.director_call(super::director_bridge::Command::Cancel)
            }
            "world_schema" => {
                empty(arguments)?;
                Ok(json!({
                    "schema":WorldRecipe::schema(), "example":WorldRecipe::example(),
                    "coordinates":"2D side view. x increases right; y increases down. Landmark rectangle y is relative to the terrain surface at the landmark x. Apply rectangles in order; use air to hollow rooms. Reserve x=-7..7 for spawn and the starter tree at x=8..12 above its surface.",
                    "workflow":"Design a world from the player's prompt, validate_world, inspect errors, revise, then create_world and enter_world. Uses existing game blocks and rules; cannot add code or new entities.",
                    "limits":"32 regions, 32 landmarks, 64 rectangles per landmark, 100000 rectangle cells. Validation checks schema, bounds and clear spawn; it does not prove every structure is reachable or the world is fun."
                }))
            }
            "validate_world" => {
                let args: Design = decode(arguments)?;
                args.recipe.validate()?;
                Ok(
                    json!({"valid":true,"name":args.recipe.name,"spawn":{"x":0,"surface_y":args.recipe.surface_y}}),
                )
            }
            "create_world" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Create {
                    id: String,
                    prompt: String,
                    recipe: WorldRecipe,
                }
                let args: Create = decode(arguments)?;
                self.store.create(&args.id, &args.prompt, &args.recipe)?;
                Ok(json!({"created":args.id,"name":args.recipe.name,"next":"enter_world"}))
            }
            "list_worlds" => {
                empty(arguments)?;
                Ok(json!({"worlds":self.store.list()?}))
            }
            "enter_world" => {
                if self.director.is_some() {
                    return Err("restart this MCP connection before entering a stepped world; live authoring is connected".into());
                }
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Enter {
                    id: String,
                }
                let args: Enter = decode(arguments)?;
                if self.session.as_ref().is_some_and(|s| s.id == args.id) {
                    return self.observe(8);
                }
                let path = self.store.open(&args.id)?;
                let lease = WorldLease::acquire(&path)?;
                let mut state = GameState::open_in_directory(&path).map_err(|e| e.to_string())?;
                state.dismiss_startup_splash();
                self.save()?;
                self.session = Some(AgentSession {
                    id: args.id,
                    state,
                    _lease: lease,
                });
                self.observe(8)
            }
            "leave_world" => {
                empty(arguments)?;
                self.save()?;
                self.session = None;
                Ok(json!({"left":true}))
            }
            "observe" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Observe {
                    #[serde(default = "default_radius")]
                    radius: u8,
                }
                let args: Observe = decode(arguments)?;
                self.observe(args.radius)
            }
            "act" => self.act(decode(arguments)?),
            "use" => {
                let target: Target = decode(arguments)?;
                let state = self.state()?;
                require_alive(state)?;
                target.validate(state)?;
                state.interact_block(target.x, target.y, false);
                self.observe(8)
            }
            "inventory_click" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Click {
                    slot: u16,
                    #[serde(default)]
                    kind: u8,
                }
                let args: Click = decode(arguments)?;
                let state = self.state()?;
                require_alive(state)?;
                if !state.inventory_open || args.kind > 2 {
                    return Err(
                        "open inventory/container first; kind 0=swap, 1=split, 2=quick move".into(),
                    );
                }
                if args.slot as usize >= state.active_ui_slot_count() {
                    return Err("UI slot out of range".into());
                }
                state.apply_client_command(ClientCommand::InventoryClick(args.slot, args.kind));
                self.observe(8)
            }
            "select_slot" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Select {
                    slot: usize,
                }
                let args: Select = decode(arguments)?;
                let state = self.state()?;
                require_alive(state)?;
                if args.slot >= state.inventory.slots.len() {
                    return Err("inventory slot out of range".into());
                }
                if args.slot < 9 {
                    state.apply_client_command(ClientCommand::SelectHotbarSlot(args.slot as u8));
                } else {
                    state.move_inventory_slot_to_hotbar(args.slot);
                }
                self.observe(8)
            }
            "recipes" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Recipes {
                    #[serde(default = "default_true")]
                    craftable_only: bool,
                }
                let args: Recipes = decode(arguments)?;
                let state = self.state()?;
                let recipes: Vec<Value> = Recipe::all().iter().enumerate().filter_map(|(id,r)| {
                    let craftable = if state.at_furnace { state.can_start_furnace_recipe(id) }
                        else { r.can_craft(&state.inventory, state.at_crafting_table, false) };
                    if args.craftable_only && !craftable { return None; }
                    Some(json!({"id":id,"result":r.result,"count":r.result_count,"ingredients":r.ingredient_requirements(),
                        "crafting_table":r.needs_crafting_table,"furnace":r.needs_furnace,"craftable":craftable}))
                }).collect();
                Ok(json!({"recipes":recipes}))
            }
            "craft" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Craft {
                    recipe_id: usize,
                }
                let args: Craft = decode(arguments)?;
                let state = self.state()?;
                require_alive(state)?;
                let recipes = Recipe::all();
                let recipe = recipes
                    .get(args.recipe_id)
                    .ok_or("unknown recipe ID; call recipes")?;
                if state.at_chest
                    || state.at_anvil
                    || state.at_enchanting_table
                    || state.at_brewing_stand
                {
                    return Err("close the current container before crafting".into());
                }
                let craftable = if state.at_furnace {
                    state.can_start_furnace_recipe(args.recipe_id)
                } else {
                    recipe.can_craft(&state.inventory, state.at_crafting_table, false)
                };
                if !craftable {
                    return Err("missing ingredients or required workstation".into());
                }
                // Don't let the recipe helper discard a result when inventory is full.
                let mut inventory = state.inventory.clone();
                for (item, amount) in recipe.ingredient_requirements() {
                    inventory.remove_item(item, amount);
                }
                if inventory.add_item(recipe.result, recipe.result_count) != 0 {
                    return Err("inventory has no room for this recipe's result".into());
                }
                state.attempt_craft(args.recipe_id);
                self.observe(8)
            }
            "close_inventory" => {
                empty(arguments)?;
                let state = self.state()?;
                if state.inventory_open {
                    state.apply_client_command(ClientCommand::ToggleInventory);
                }
                self.observe(8)
            }
            "respawn" => {
                empty(arguments)?;
                let state = self.state()?;
                if !state.can_respawn_from_death_screen() {
                    return Err("respawn is not available yet; advance time with act".into());
                }
                state.apply_client_command(ClientCommand::RespawnFromDeathScreen);
                self.observe(8)
            }
            "save_world" => {
                empty(arguments)?;
                self.save()?;
                Ok(json!({"saved":true}))
            }
            _ => Err(format!("unknown tool {name}")),
        }?;
        if matches!(
            name,
            "act" | "use" | "select_slot" | "craft" | "close_inventory" | "respawn"
        ) {
            self.save()?;
            let state = self.state()?;
            state
                .world
                .trim_chunk_cache(&[state.player.x.floor() as i32], 0)
                .map_err(|e| e.to_string())?;
        }
        Ok(result)
    }

    fn state(&mut self) -> Result<&mut GameState, String> {
        self.session
            .as_mut()
            .map(|s| &mut s.state)
            .ok_or_else(|| "enter a world first".into())
    }

    fn director_call(&self, command: super::director_bridge::Command) -> Result<Value, String> {
        self.director
            .as_ref()
            .ok_or("director_connect first")?
            .call(command)
    }

    fn observe(&self, radius: u8) -> Result<Value, String> {
        if !(1..=12).contains(&radius) {
            return Err("observation radius must be 1–12".into());
        }
        let session = self.session.as_ref().ok_or("enter a world first")?;
        let s = &session.state;
        let x = s.player.x.floor() as i32;
        let y = s.player.y.floor() as i32;
        let radius = i32::from(radius);
        let top = (y - radius).max(0);
        let bottom = (y + radius).min(CHUNK_HEIGHT as i32 - 1);
        let rows: Vec<Vec<_>> = (top..=bottom)
            .map(|wy| {
                ((x - radius)..=(x + radius))
                    .map(|wx| s.world.get_block(wx, wy))
                    .collect()
            })
            .collect();
        let mut entities = Vec::new();
        macro_rules! nearby {
            ($collection:expr, $kind:literal) => {
                for entity in &$collection {
                    if (entity.x-s.player.x).abs() <= radius as f64 && (entity.y-s.player.y).abs() <= radius as f64 {
                        entities.push(json!({"kind":$kind,"x":entity.x,"y":entity.y,"health":entity.health}));
                    }
                }
            };
        }
        nearby!(s.zombies, "zombie");
        nearby!(s.creepers, "creeper");
        nearby!(s.skeletons, "skeleton");
        nearby!(s.spiders, "spider");
        nearby!(s.cows, "cow");
        nearby!(s.sheep, "sheep");
        nearby!(s.pigs, "pig");
        nearby!(s.chickens, "chicken");
        nearby!(s.villagers, "villager");
        nearby!(s.endermen, "enderman");
        nearby!(s.slimes, "slime");
        nearby!(s.silverfish, "silverfish");
        nearby!(s.blazes, "blaze");
        nearby!(s.pigmen, "zombie_pigman");
        nearby!(s.ghasts, "ghast");
        nearby!(s.wolves, "wolf");
        nearby!(s.ocelots, "ocelot");
        nearby!(s.squids, "squid");
        let drops: Vec<_> = s
            .item_entities
            .iter()
            .filter(|e| {
                (e.x - s.player.x).abs() <= radius as f64
                    && (e.y - s.player.y).abs() <= radius as f64
            })
            .map(|e| json!({"x":e.x,"y":e.y,"item":e.item_type}))
            .collect();
        Ok(json!({
            "world":session.id,"dimension":format!("{:?}",s.world.dimension),
            "objective":s.trial.as_ref().map(|t|t.objective()),"trial":s.trial,
            "clock":"stepped: simulation advances only during act calls, 20 ticks per simulated second",
            "player":{"x":s.player.x,"y":s.player.y,"health":s.player.health,"hunger":s.player.hunger,
                "grounded":s.player.grounded,"experience":s.player.experience_level},
            "inventory":s.inventory.slots,"selected_slot":s.hotbar_index,
            "ui":{"inventory_open":s.inventory_open,"crafting_table":s.at_crafting_table,
                "furnace":s.at_furnace,"furnace_status":s.furnace_status_line(),"chest":s.at_chest,"dead":s.is_showing_death_screen(),
                "respawn_wait_ticks":s.death_respawn_ticks_remaining(),"credits":s.is_showing_credits()},
            "view":{"origin_x":x-radius,"origin_y":top,"rows":rows},
            "entities":entities,"drops":drops,
            "container":s.open_container_slots()
        }))
    }

    fn act(&mut self, action: Action) -> Result<Value, String> {
        if !(1..=40).contains(&action.ticks) {
            return Err("ticks must be 1–40".into());
        }
        let state = self.state()?;
        if action.primary || action.jump || !matches!(action.direction, Direction::Stay) {
            require_alive(state)?;
        }
        if let Some(target) = &action.target {
            target.validate(state)?;
        }
        if action.primary && action.target.is_none() {
            return Err("primary action requires target coordinates".into());
        }
        state.apply_client_command(ClientCommand::ClearDirectionalInput);
        match action.direction {
            Direction::Left => state.apply_client_command(ClientCommand::SetMoveLeft(true)),
            Direction::Right => state.apply_client_command(ClientCommand::SetMoveRight(true)),
            Direction::Stay => {}
        }
        state.apply_client_command(ClientCommand::SetJumpHeld(action.jump));
        if action.jump {
            state.apply_client_command(ClientCommand::QueueJump);
        }
        state.apply_client_command(ClientCommand::SetPrimaryAction(action.primary));
        let target = action.target.unwrap_or(Target {
            x: state.player.x.floor() as i32,
            y: state.player.y.floor() as i32,
        });
        for _ in 0..action.ticks {
            state.update(target.x, target.y);
        }
        state.apply_client_command(ClientCommand::ClearDirectionalInput);
        state.apply_client_command(ClientCommand::SetJumpHeld(false));
        state.apply_client_command(ClientCommand::SetPrimaryAction(false));
        self.observe(8)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Design {
    recipe: WorldRecipe,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    x: i32,
    y: i32,
}
impl Target {
    fn validate(&self, state: &GameState) -> Result<(), String> {
        if !(0..CHUNK_HEIGHT as i32).contains(&self.y)
            || (f64::from(self.x) - state.player.x).abs() > 12.0
            || (f64::from(self.y) - state.player.y).abs() > 12.0
        {
            return Err("target must be inside the world and within 12 cells; interactions still obey normal reach and line of sight".into());
        }
        Ok(())
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Direction {
    Left,
    Right,
    #[default]
    Stay,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    ticks: u8,
    #[serde(default)]
    direction: Direction,
    #[serde(default)]
    jump: bool,
    #[serde(default)]
    primary: bool,
    target: Option<Target>,
}
fn default_radius() -> u8 {
    8
}
fn default_true() -> bool {
    true
}
fn require_alive(state: &GameState) -> Result<(), String> {
    if state.player.health <= 0.0 || state.is_showing_death_screen() || state.is_showing_credits() {
        return Err("player cannot interact while dead or showing credits".into());
    }
    Ok(())
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("invalid arguments: {e}"))
}
fn empty(value: Value) -> Result<(), String> {
    if value.as_object().is_some_and(|o| o.is_empty()) {
        Ok(())
    } else {
        Err("this tool takes an empty object".into())
    }
}

pub fn tool_catalog() -> Vec<Value> {
    fn tool(
        name: &str,
        description: &str,
        properties: Value,
        required: &[&str],
        read_only: bool,
    ) -> Value {
        json!({"name":name,"description":description,
            "inputSchema":{"type":"object","additionalProperties":false,"properties":properties,"required":required},
            "annotations":{"readOnlyHint":read_only,"destructiveHint":matches!(name,"act" | "use" | "craft"),"openWorldHint":false}})
    }
    let target = json!({"type":"object","additionalProperties":false,"properties":{"x":{"type":"integer"},"y":{"type":"integer"}},"required":["x","y"]});
    vec![
        tool(
            "inventory_click",
            "Interact with an open inventory/container under normal rules. UI slots: chest slots first, then player inventory. kind 0=swap, 1=split, 2=quick move.",
            json!({"slot":{"type":"integer","minimum":0,"maximum":128},"kind":{"type":"integer","minimum":0,"maximum":2}}),
            &["slot"],
            false,
        ),
        tool(
            "director_connect",
            "Attach to a running native game whose human enabled /connect. Omitting world_id uses the classic world. Does not acquire its save lease.",
            json!({"world_id":{"type":"string"},"multiplayer":{"type":"boolean","default":false}}),
            &[],
            false,
        ),
        tool(
            "director_wait",
            "Wait up to 50 seconds for an in-game intention, without model polling calls. Returns context/schema when ready, or timed_out. Keep the client task running to service chat; idle apps are not awakened automatically.",
            json!({"timeout_seconds":{"type":"integer","minimum":1,"maximum":50,"default":45}}),
            &[],
            true,
        ),
        tool(
            "director_observe",
            "Read live world context, pending in-game requests, supported plan schema and the last application result.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "director_request",
            "Start a current-world change requested in this client. Read the returned pending request, then submit a typed plan. The human must enable /connect.",
            json!({"prompt":{"type":"string","maxLength":2000}}),
            &["prompt"],
            false,
        ),
        tool(
            "director_apply",
            "Submit a validated plan for the exact pending request_id. The game rechecks current terrain, applies on its simulation thread and reports the outcome in director_observe. Never retry blindly after a timeout.",
            json!({"request_id":{"type":"string"},"plan":super::director::schema()}),
            &["request_id", "plan"],
            false,
        ),
        tool(
            "director_cancel",
            "Cancel the live pending intention; late plans are rejected. Ordinary gameplay continues.",
            json!({}),
            &[],
            false,
        ),
        tool(
            "world_schema",
            "Read the supported world design schema and example before generating a world.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "validate_world",
            "Check a design. Repair validation errors before create_world. Does not write saves.",
            json!({"recipe":WorldRecipe::schema()}),
            &["recipe"],
            true,
        ),
        tool(
            "create_world",
            "Create a new named world from a validated design and the original player prompt. Never overwrites a world.",
            json!({"id":{"type":"string"},"prompt":{"type":"string","maxLength":4000},"recipe":WorldRecipe::schema()}),
            &["id", "prompt", "recipe"],
            false,
        ),
        tool(
            "list_worlds",
            "List saved generated worlds.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "enter_world",
            "Save and leave the current world, then load a named world. Player state is separate per world.",
            json!({"id":{"type":"string"}}),
            &["id"],
            false,
        ),
        tool(
            "leave_world",
            "Save and release the world, allowing a human to open it with termcraft world play ID.",
            json!({}),
            &[],
            false,
        ),
        tool(
            "observe",
            "Inspect the local tile map, entities and complete player inventory. Coordinates increase right/down.",
            json!({"radius":{"type":"integer","minimum":1,"maximum":12,"default":8}}),
            &[],
            true,
        ),
        tool(
            "act",
            "Advance 1–40 simulation ticks (20/s). Move, jump, mine or attack using normal game rules. Held inputs release when the call finishes.",
            json!({
                "ticks":{"type":"integer","minimum":1,"maximum":40},
                "direction":{"type":"string","enum":["left","right","stay"],"default":"stay"},
                "jump":{"type":"boolean","default":false},"primary":{"type":"boolean","default":false},"target":target
            }),
            &["ticks"],
            false,
        ),
        tool(
            "use",
            "Place the selected block or interact with a workstation/chest at x,y; normal reach and line of sight apply.",
            json!({"x":{"type":"integer"},"y":{"type":"integer"}}),
            &["x", "y"],
            false,
        ),
        tool(
            "select_slot",
            "Select a hotbar slot (0–8), or move another inventory slot into the selected hotbar slot.",
            json!({"slot":{"type":"integer","minimum":0}}),
            &["slot"],
            false,
        ),
        tool(
            "recipes",
            "List recipe IDs, ingredients, station requirements and availability.",
            json!({"craftable_only":{"type":"boolean","default":true}}),
            &[],
            true,
        ),
        tool(
            "craft",
            "Craft one recipe or start its furnace job. Requires materials, output space and the appropriate workstation.",
            json!({"recipe_id":{"type":"integer","minimum":0}}),
            &["recipe_id"],
            false,
        ),
        tool(
            "close_inventory",
            "Close inventory/workstation UI before movement or further interaction.",
            json!({}),
            &[],
            false,
        ),
        tool(
            "respawn",
            "Respawn after the normal death-screen delay.",
            json!({}),
            &[],
            false,
        ),
        tool(
            "save_world",
            "Persist player progression and changed chunks, reporting write errors.",
            json!({}),
            &[],
            false,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{block::BlockType, item::ItemType};

    #[test]
    fn agent_jump_queues_takeoff_under_normal_rules() {
        let root = std::env::temp_dir().join(format!("tc-agent-jump-{}", rand::random::<u64>()));
        let store = WorldStore::new(root.clone());
        store
            .create("jump", "jump test", &WorldRecipe::example())
            .unwrap();
        let mut tools = AgentTools::new(store);
        tools.call("enter_world", json!({"id":"jump"})).unwrap();
        let s = &mut tools.session.as_mut().unwrap().state;
        s.world.load_chunks_for_spawn_search(0, 8);
        for x in -4..=4 {
            for y in 0..24 {
                s.world.set_block(
                    x,
                    y,
                    if y >= 20 {
                        crate::world::block::BlockType::Stone
                    } else {
                        crate::world::block::BlockType::Air
                    },
                );
            }
        }
        s.player.x = 0.5;
        s.player.y = 20.0;
        s.player.grounded = true;
        let observed = tools.call("act", json!({"ticks":1,"jump":true})).unwrap();
        assert!(observed["player"]["y"].as_f64().unwrap() < 20.0);
        tools.call("leave_world", json!({})).unwrap();
        drop(tools);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn tool_save_failures_are_reported_and_dead_players_cannot_interact() {
        let root = std::env::temp_dir().join(format!(
            "termcraft-agent-errors-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut tools = AgentTools::new(WorldStore::new(root.clone()));
        tools
            .call(
                "create_world",
                json!({"id":"a","prompt":"snow","recipe":WorldRecipe::example()}),
            )
            .unwrap();
        tools.call("enter_world", json!({"id":"a"})).unwrap();
        tools.session.as_mut().unwrap().state.player.health = 0.0;
        assert!(
            tools
                .call("use", json!({"x":1,"y":63}))
                .unwrap_err()
                .contains("dead")
        );
        assert!(
            tools
                .call("craft", json!({"recipe_id":0}))
                .unwrap_err()
                .contains("dead")
        );
        assert!(
            tools
                .call("select_slot", json!({"slot":0}))
                .unwrap_err()
                .contains("dead")
        );
        assert!(
            tools
                .call(
                    "act",
                    json!({"ticks":1,"primary":true,"target":{"x":1,"y":63}})
                )
                .unwrap_err()
                .contains("dead")
        );
        let temporary = tools
            .store
            .path("a")
            .unwrap()
            .join("player_progression.bin.tmp");
        std::fs::create_dir(&temporary).unwrap();
        assert!(
            tools
                .call("save_world", json!({}))
                .unwrap_err()
                .contains("save failed")
        );
        assert!(tools.call("leave_world", json!({})).is_err());
        assert!(tools.session.is_some()); // A failed save must not abandon the active world.
        std::fs::remove_dir(temporary).unwrap();
        tools.call("leave_world", json!({})).unwrap();
        let path = tools
            .store
            .path("a")
            .unwrap()
            .join("player_progression.bin");
        std::fs::write(path, b"corrupt save").unwrap();
        assert!(
            tools
                .call("enter_world", json!({"id":"a"}))
                .unwrap_err()
                .contains("not reset")
        );
        assert!(
            !tools
                .store
                .path("a")
                .unwrap()
                .join(".session.lock")
                .exists()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn agent_mines_collects_crafts_places_and_resumes_without_cheats() {
        let root = std::env::temp_dir().join(format!(
            "termcraft-agent-play-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut tools = AgentTools::new(WorldStore::new(root.clone()));
        let mut recipe = WorldRecipe::example();
        recipe.terrain = crate::world::recipe::Terrain::Plains;
        recipe.relief = 0;
        recipe.regions.clear();
        recipe.landmarks.clear();
        tools
            .call(
                "create_world",
                json!({"id":"a","prompt":"a plain with a tree","recipe":recipe}),
            )
            .unwrap();
        let initial = tools.call("enter_world", json!({"id":"a"})).unwrap();
        assert_eq!(initial["player"]["x"], 0.5);
        assert!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .inventory
                .slots
                .iter()
                .all(Option::is_none)
        );
        let age = tools.session.as_ref().unwrap().state.player.age;
        assert!(tools.call("act", json!({"ticks":41})).is_err());
        assert_eq!(tools.session.as_ref().unwrap().state.player.age, age);
        assert!(tools.call("use", json!({"x":5000,"y":63})).is_err());
        assert!(
            tools
                .call("act", json!({"ticks":1,"teleport":true}))
                .is_err()
        );

        tools
            .call("act", json!({"ticks":24,"direction":"right"}))
            .unwrap();
        assert!(tools.session.as_ref().unwrap().state.player.x > 4.0);
        for _ in 0..3 {
            tools
                .call(
                    "act",
                    json!({"ticks":40,"primary":true,"target":{"x":10,"y":63}}),
                )
                .unwrap();
        }
        assert_eq!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .world
                .get_block(10, 63),
            BlockType::Air
        );
        // Explicitly approach the drop; releasing movement now brakes consistently
        // with native and remote controls instead of coasting into pickup range.
        tools
            .call("act", json!({"ticks":3,"direction":"right"}))
            .unwrap();
        tools.call("act", json!({"ticks":40})).unwrap();
        assert!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .inventory
                .has_item(ItemType::Wood, 1)
        );
        let planks = Recipe::all()
            .iter()
            .position(|r| r.result == ItemType::Planks)
            .unwrap();
        tools.call("craft", json!({"recipe_id":planks})).unwrap();
        assert!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .inventory
                .has_item(ItemType::Planks, 4)
        );
        assert!(tools.call("craft", json!({"recipe_id":planks})).is_err());
        let state = &tools.session.as_ref().unwrap().state;
        let slot = state
            .inventory
            .slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.item_type == ItemType::Planks))
            .unwrap();
        let target_x = state.player.x.floor() as i32 - 2;
        tools.call("select_slot", json!({"slot":slot})).unwrap();
        tools.call("use", json!({"x":target_x,"y":63})).unwrap();
        assert_eq!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .world
                .get_block(target_x, 63),
            BlockType::Planks
        );
        assert!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .inventory
                .has_item(ItemType::Planks, 3)
        );
        tools.call("leave_world", json!({})).unwrap();
        assert!(tools.call("observe", json!({})).is_err());
        tools.call("enter_world", json!({"id":"a"})).unwrap();
        assert_eq!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .world
                .get_block(target_x, 63),
            BlockType::Planks
        );
        assert!(
            tools
                .session
                .as_ref()
                .unwrap()
                .state
                .inventory
                .has_item(ItemType::Planks, 3)
        );
        tools.call("leave_world", json!({})).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
