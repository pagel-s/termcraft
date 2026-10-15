//! Native AI game master. Model work is off-thread; only typed, preflighted
//! actions can change the authoritative state. No shell or generated code runs.
use super::state::{GameState, WeatherType};
use crate::entities::{
    chicken::Chicken, cow::Cow, pig::Pig, sheep::Sheep, skeleton::Skeleton, spider::Spider,
    zombie::Zombie,
};
use crate::world::{
    Dimension, World,
    block::BlockType,
    environment,
    recipe::{BlockRect, Region, Terrain, WorldRecipe},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use std::time::{Duration, Instant};

const MAX_PROMPT: usize = 2000;
const COOLDOWN: Duration = Duration::from_secs(120);
const MAX_PROACTIVE: u8 = 3;
const CONFIG_FILE: &str = "director.json";

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatView {
    #[serde(skip)]
    pub open: bool,
    #[serde(skip)]
    pub draft: String,
    pub lines: VecDeque<String>,
    pub pending: bool,
    pub status: String,
    pub mode: String,
}

impl ChatView {
    pub fn push(&mut self, text: impl AsRef<str>) {
        let text: String = text
            .as_ref()
            .chars()
            .filter(|c| !c.is_control())
            .take(1200)
            .collect();
        self.status = text.clone();
        self.lines.push_back(text);
        while self.lines.len() > 16 {
            self.lines.pop_front();
        }
    }
    pub fn insert(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.draft.len() + c.len_utf8() > MAX_PROMPT {
                break;
            }
            self.draft.push(c);
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Mob {
    Zombie,
    Skeleton,
    Spider,
    Cow,
    Sheep,
    Pig,
    Chicken,
}
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Side {
    East,
    West,
}
impl Side {
    fn sign(self) -> i32 {
        if matches!(self, Self::East) { 1 } else { -1 }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Weather {
    Clear,
    Rain,
    Storm,
}
impl Weather {
    fn game(self) -> WeatherType {
        match self {
            Self::Clear => WeatherType::Clear,
            Self::Rain => WeatherType::Rain,
            Self::Storm => WeatherType::Thunderstorm,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Day,
    Night,
    Dawn,
    Dusk,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Spawn {
        mob: Mob,
        count: u8,
        side: Side,
        distance: u8,
    },
    Weather {
        weather: Weather,
    },
    Time {
        phase: Phase,
    },
    Environment {
        terrain: Terrain,
        radius: u8,
    },
    Structure {
        name: String,
        side: Side,
        distance: u8,
        blocks: Vec<BlockRect>,
    },
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
struct Plan {
    message: String,
    actions: Vec<Action>,
}

impl Plan {
    fn validate(&self) -> Result<(), String> {
        if self.message.is_empty()
            || self.message.len() > 500
            || self.message.chars().any(char::is_control)
        {
            return Err("message needs 1–500 bytes without control characters".into());
        }
        if self.actions.len() > 4 {
            return Err("at most four actions per request".into());
        }
        let mut mobs = 0;
        let mut structures = 0;
        let mut area = 0u32;
        for action in &self.actions {
            match action {
                Action::Spawn {
                    count, distance, ..
                } => {
                    if !(1..=12).contains(count) || !(8..=40).contains(distance) {
                        return Err("spawn count 1–12; distance 8–40".into());
                    }
                    mobs += *count as u16;
                }
                Action::Environment { radius, .. } if !(8..=48).contains(radius) => {
                    return Err("environment radius 8–48".into());
                }
                Action::Structure {
                    name,
                    distance,
                    blocks,
                    ..
                } => {
                    structures += 1;
                    if name.trim().is_empty()
                        || name.len() > 80
                        || name.chars().any(char::is_control)
                        || !(8..=40).contains(distance)
                        || blocks.is_empty()
                        || blocks.len() > 16
                    {
                        return Err(
                            "structure needs a short name, distance 8–40 and 1–16 rectangles"
                                .into(),
                        );
                    }
                    for rect in blocks {
                        if rect.x < 0
                            || rect.x > 15
                            || rect.y < -12
                            || rect.y > -1
                            || rect.width == 0
                            || rect.height == 0
                            || rect.width > 16
                            || rect.height > 12
                            || rect.x + rect.width as i32 > 16
                            || rect.y + rect.height as i32 > 0
                        {
                            return Err(
                                "structure rectangles must fit x=0..15, y=-12..-1 above ground"
                                    .into(),
                            );
                        }
                        area += rect.width * rect.height;
                    }
                }
                _ => {}
            }
        }
        if mobs > 24 || structures > 1 || area > 512 {
            return Err("request exceeds 24 mobs, one structure or 512 rectangle cells".into());
        }
        Ok(())
    }
}

pub fn schema() -> Value {
    fn variant(name: &str, props: Value) -> Value {
        let mut props = props.as_object().unwrap().clone();
        props.insert("type".into(), json!({"type":"string","const":name}));
        let required: Vec<_> = props.keys().cloned().collect();
        json!({"type":"object","additionalProperties":false,"required":required,"properties":props})
    }
    let side = json!({"type":"string","enum":["east","west"]});
    let distance = json!({"type":"integer","minimum":8,"maximum":40});
    let mut rect =
        WorldRecipe::schema()["properties"]["landmarks"]["items"]["properties"]["blocks"]["items"]
            .clone();
    rect["properties"]["x"] = json!({"type":"integer","minimum":0,"maximum":15});
    rect["properties"]["y"] = json!({"type":"integer","minimum":-12,"maximum":-1});
    rect["properties"]["width"] = json!({"type":"integer","minimum":1,"maximum":16});
    rect["properties"]["height"] = json!({"type":"integer","minimum":1,"maximum":12});
    json!({"type":"object","additionalProperties":false,"required":["message","actions"],"properties":{
        "message":{"type":"string","minLength":1,"maxLength":500},
        "actions":{"type":"array","maxItems":4,"items":{"anyOf":[
            variant("spawn", json!({"mob":{"type":"string","enum":["zombie","skeleton","spider","cow","sheep","pig","chicken"]},"count":{"type":"integer","minimum":1,"maximum":12},"side":side,"distance":distance})),
            variant("weather", json!({"weather":{"type":"string","enum":["clear","rain","storm"]}})),
            variant("time", json!({"phase":{"type":"string","enum":["day","night","dawn","dusk"]}})),
            variant("environment", json!({"terrain":{"type":"string","enum":["plains","forest","desert","tundra","mountains"]},"radius":{"type":"integer","minimum":8,"maximum":48}})),
            variant("structure", json!({"name":{"type":"string","minLength":1,"maxLength":80},"side":side,"distance":distance,"blocks":{"type":"array","minItems":1,"maxItems":16,"items":rect}}))
        ]}}
    }})
}

pub fn example() -> Value {
    json!({"message":"A snowy storm rolls in.","actions":[{"type":"environment","terrain":"tundra","radius":24},{"type":"weather","weather":"storm"}]})
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Turn {
    request: String,
    result: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    x: i32,
    y: i32,
    before: BlockType,
    after: BlockType,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    version: u32,
    proactive: Option<String>,
    recent: Vec<Turn>,
    edits: Vec<Vec<Edit>>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            proactive: None,
            recent: Vec::new(),
            edits: Vec::new(),
        }
    }
}

impl Config {
    fn read(root: &Path) -> Result<Self, String> {
        let file = match std::fs::File::open(root.join(CONFIG_FILE)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(512 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 512 * 1024 {
            return Err("director save exceeds 512 KiB".into());
        }
        let config: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if config.version != 1
            || config.recent.len() > 6
            || config.edits.len() > 8
            || config
                .proactive
                .as_ref()
                .is_some_and(|s| !["exploration", "combat", "building"].contains(&s.as_str()))
            || config
                .recent
                .iter()
                .any(|t| t.request.len() > MAX_PROMPT || t.result.len() > 1200)
            || config.edits.iter().any(|edits| {
                edits.is_empty()
                    || edits.len() > 192
                    || edits.iter().any(|e| {
                        e.x.abs_diff(0) > 100_000
                            || !(1..128).contains(&e.y)
                            || e.before != BlockType::Air
                    })
            })
        {
            return Err("invalid director save; existing file was preserved".into());
        }
        Ok(config)
    }
    fn write(&self, root: &Path) -> Result<(), String> {
        let data = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let temp = root.join(format!(
            ".director-{}-{}.tmp",
            std::process::id(),
            rand::random::<u64>()
        ));
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&data)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temp, root.join(CONFIG_FILE))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result.map_err(|e| format!("cannot save director preferences/history: {e}"))
    }
}

struct Pending {
    receiver: Receiver<Result<Plan, String>>,
    cancelled: Arc<AtomicBool>,
    root: PathBuf,
    dimension: Dimension,
    x: i32,
    request: String,
    started: Instant,
    proactive: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct ExternalRequest {
    id: String,
    data: Value,
    sender: Option<mpsc::Sender<Result<Plan, String>>>,
}

pub struct Director {
    bridge: Option<super::director_bridge::Host>,
    external: Option<ExternalRequest>,
    bridge_result: Value,
    program: Option<PathBuf>,
    config: Config,
    pending: Option<Pending>,
    last_event: Instant,
    last_area_x: i32,
    proactive_count: u8,
    encounter: Option<u64>,
    toast_until: Instant,
    config_valid: bool,
    config_dirty: bool,
}

impl Director {
    pub fn new(state: &mut GameState) -> Self {
        let mut config_valid = true;
        let config = Config::read(state.world.save_root()).unwrap_or_else(|e| {
            config_valid = false;
            state
                .chat
                .push(format!("Could not load game master preferences: {e}"));
            Config::default()
        });
        state.chat.mode = config
            .proactive
            .clone()
            .unwrap_or_else(|| "request-only".into());
        Self {
            bridge: None,
            external: None,
            bridge_result: Value::Null,
            program: std::env::var_os("TERMCRAFT_DIRECTOR")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from),
            config,
            pending: None,
            last_event: Instant::now(),
            last_area_x: state.player.x.floor() as i32,
            proactive_count: 0,
            encounter: None,
            toast_until: Instant::now(),
            config_valid,
            config_dirty: false,
        }
    }
    fn tell(&mut self, state: &mut GameState, message: impl AsRef<str>) {
        state.chat.push(message);
        self.toast_until = Instant::now() + Duration::from_secs(12);
    }
    pub fn cancel(&mut self, state: &mut GameState) {
        if let Some(external) = self.external.take() {
            self.bridge_result = json!({"request_id":external.id,"status":"cancelled"});
        }
        if self.pending.take().is_some() {
            self.tell(state, "Request cancelled.");
        }
        state.chat.pending = false;
    }
    pub fn submit(&mut self, state: &mut GameState, request: String) {
        let request = request.trim().to_string();
        if request.is_empty() {
            return;
        }
        state.chat.push(format!("You: {request}"));
        let result = if request.starts_with('/') {
            self.command(state, &request)
        } else {
            self.start(state, &request, false, Instant::now())
        };
        if let Err(error) = result {
            self.tell(state, error);
        }
    }
    fn command(&mut self, state: &mut GameState, request: &str) -> Result<(), String> {
        if !self.config_valid && !matches!(request, "/help" | "/cancel" | "/reset") {
            return Err("Existing director preferences are unreadable. /reset backs them up and starts fresh.".into());
        }
        match request {
            "/reset" => {
                self.cancel(state);
                let root=state.world.save_root();
                let path=root.join(CONFIG_FILE);
                if path.try_exists().map_err(|e|e.to_string())? {
                    std::fs::rename(path,root.join(format!("director.backup-{}-{}.json",std::process::id(),rand::random::<u64>()))).map_err(|e|e.to_string())?;
                }
                let next=Config::default();next.write(root)?;self.config=next;self.config_valid=true;self.config_dirty=false;
                state.chat.mode="request-only".into();self.tell(state,"Director preferences/history reset; previous file backed up. World changes remain.");
            }
            "/connect" => {
                self.cancel(state);
                self.bridge=Some(super::director_bridge::Host::start(state.world.save_root())?);
                self.tell(state,"Live game-master editing enabled for your local LLM client. Use director_connect in Codex/Claude, then ask here or there. /disconnect revokes access.");
            }
            "/disconnect" => {
                self.cancel(state);self.bridge=None;
                self.tell(state,"Live game-master access disconnected.");
            }
            "/help" => self.tell(state, "Ask for battles, mobs, weather, local ambience or structures. /auto exploration|combat|building, /pause, /cancel, /undo. T: chat; Enter: send; Esc: close."),
            "/cancel" => self.cancel(state),
            "/pause" | "/request-only" => {
                let mut next = self.config.clone(); next.proactive = None; next.write(state.world.save_root())?; self.config = next;
                if self.pending.as_ref().is_some_and(|p| p.proactive) { self.cancel(state); }
                state.chat.mode = "request-only".into(); self.tell(state, "Proactive events paused. You can still request changes.");
            }
            "/undo" => {
                if state.current_dimension != Dimension::Overworld { return Err("Return to the Overworld to undo its structure.".into()); }
                if self.pending.is_some() { return Err("Cancel the pending request before undoing a structure.".into()); }
                let edits = self.config.edits.last().ok_or("No structure to undo in this world.")?.clone();
                state.world.load_chunks_for_spawn_search(edits[0].x, 32);
                state.world.check_storage().map_err(|e| e.to_string())?;
                let mut changed = 0;
                for edit in &edits {
                    if !matches!(edit.after,BlockType::Chest|BlockType::Furnace|BlockType::CraftingTable)
                        && state.world.get_block(edit.x, edit.y) == edit.after {
                        state.world.set_block(edit.x, edit.y, edit.before); changed += 1;
                    }
                }
                // Keep the journal if saving fails, so the operation can be retried.
                state.try_persist_world_and_progress().map_err(|e| format!("Undo is in memory; save failed: {e}"))?;
                self.config.edits.pop(); self.config_dirty=true; self.config.write(state.world.save_root())?; self.config_dirty=false;
                self.tell(state, format!("Removed {changed} unchanged structure blocks; kept {} edited/storage blocks. Weather, ambience and mobs are not undone.", edits.len() - changed));
            }
            _ if request.starts_with("/auto ") => {
                let preference = request.trim_start_matches("/auto ");
                if !["exploration", "combat", "building"].contains(&preference) { return Err("Use /auto exploration, /auto combat or /auto building.".into()); }
                if self.program.is_none() && self.bridge.is_none() { return Err("AI game master is not configured. See docs/agents.md for setup.".into()); }
                let mut next = self.config.clone(); next.proactive = Some(preference.into()); next.write(state.world.save_root())?; self.config = next;
                state.chat.mode = preference.into(); self.last_event = Instant::now(); self.last_area_x = state.player.x.floor() as i32; self.proactive_count = 0;
                self.tell(state, format!("Proactive {preference}: up to three events this session, at least two minutes apart, after exploring a new area. /pause stops them."));
            }
            _ => return Err("Unknown chat command. Type /help.".into()),
        }
        Ok(())
    }
    fn start(
        &mut self,
        state: &mut GameState,
        request: &str,
        proactive: bool,
        now: Instant,
    ) -> Result<(), String> {
        if !self.config_valid {
            return Err("Existing director preferences are unreadable. /reset backs them up and starts fresh.".into());
        }
        if self.pending.is_some() {
            return Err("Already planning. Use /cancel before another request.".into());
        }
        if request.is_empty() || request.len() > MAX_PROMPT || request.chars().any(char::is_control)
        {
            return Err("Use at most 2000 bytes without control characters.".into());
        }
        if state.current_dimension != Dimension::Overworld {
            return Err(
                "The experimental game master currently changes the Overworld only.".into(),
            );
        }
        if state.is_showing_death_screen() || state.is_showing_credits() {
            return Err("Resume normal play before changing the world.".into());
        }
        if self.program.is_none() && self.bridge.is_none() {
            return Err("Use /connect for your existing LLM client, or configure a provider runner. See docs/agents.md.".into());
        }
        let x = state.player.x.floor() as i32;
        if x.abs_diff(0) > 99_900 {
            return Err("Game master changes currently require x within ±99900.".into());
        }
        let context = json!({"player":{"x":x,"y":state.player.y,"health":state.player.health,"hunger":state.player.hunger},
            "dimension":"overworld","weather":format!("{:?}",state.weather),"biome":format!("{:?}",state.world.get_biome(x)),
            "difficulty":state.difficulty_name(),"nearby_mobs":mob_count(state),"encounter_active":self.encounter.is_some(),
            "preference":self.config.proactive,"recent":self.config.recent,"proactive":proactive,
            "terrain":(-12..=12).map(|i| {let px=x+i*4; json!({"x":px,"ground_y":surface(&state.world,px)})}).collect::<Vec<_>>()});
        let request_data = json!({"protocol":1,"task":"director","prompt":request,"context":context,"schema":schema(),"example":example(),
            "instructions":"You are Termcraft's AI game master. Return only a typed world-change plan for the CURRENT Overworld. Coordinates x right, y down. Use existing enemies for a finite battle; choose spawn distance 8–40 and total <=24 mobs. Proactive events must match preferences and player health, and may return no actions if nothing is appropriate. Never ambush low-health players proactively. Environment changes ambience, precipitation and biome-based spawning only; it does NOT replace terrain or create trees/mountains. Weather follows game rules, not a guaranteed duration. Structures fit 16 columns wide and 12 high above ground; rectangles apply in order, air hollows only the new blueprint. Keep a two-block-high entrance at ground level. Existing blocks/builds cannot be overwritten; placement needs flat clear land and may fail. Do not invent mechanics, quests, loot scripts or new entities. Acknowledge unsupported requests in message with empty actions. Use recent requests/results for follow-ups."});
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        if self.bridge.is_some() {
            self.external = Some(ExternalRequest {
                id: super::server_save::token(),
                data: request_data,
                sender: Some(sender),
            });
        } else if let Some(program) = self.program.clone() {
            std::thread::spawn(move || {
                let result = design(request_data, |data| {
                    crate::generator::run_program_cancellable(
                        &program,
                        data,
                        Duration::from_secs(90),
                        &worker_cancel,
                    )
                });
                let _ = sender.send(result);
            });
        }
        self.pending = Some(Pending {
            receiver,
            cancelled: cancel,
            root: state.world.save_root().to_path_buf(),
            dimension: state.current_dimension,
            x,
            request: request.into(),
            started: now,
            proactive,
        });
        state.chat.pending = true;
        self.tell(
            state,
            if proactive {
                "Game master is planning an event…"
            } else {
                "Game master is planning your request…"
            },
        );
        Ok(())
    }
    pub fn enable_bridge(&mut self, state: &mut GameState) -> Result<(), String> {
        self.command(state, "/connect")
    }
    pub(crate) fn host_left(&mut self, state: &mut GameState) {
        self.cancel(state);
        self.bridge = None;
        self.config.proactive = None;
        self.config_dirty = true;
        state.chat.mode = "request-only".into();
        self.tell(state,"Host disconnected. Live game-master access and proactive events stopped; the next host can /connect.");
    }
    fn bridge_observation(&self, state: &GameState) -> Value {
        let x = state.player.x.floor() as i32;
        json!({"context":{"dimension":format!("{:?}",state.current_dimension),
            "player":{"x":state.player.x,"y":state.player.y,"health":state.player.health,"hunger":state.player.hunger},
            "weather":format!("{:?}",state.weather),"nearby_mobs":mob_count(state),
            "terrain":(-12..=12).map(|i| {let px=x+i*4;json!({"x":px,"ground_y":surface(&state.world,px)})}).collect::<Vec<_>>()},
            "schema":schema(),"example":example(),"pending":self.external.as_ref().map(|e|json!({"request_id":e.id,"request":e.data,"submitted":e.sender.is_none()})),
            "last_result":self.bridge_result,
            "workflow":"Read pending.request.prompt/context. Submit the supported plan with its exact request_id. For a direct intention, director_request first. Plans apply after modal UI closes. Observe the result. /disconnect revokes access."})
    }
    fn poll_bridge(&mut self, state: &mut GameState) {
        if self
            .bridge
            .as_ref()
            .is_some_and(|host| host.root != state.world.save_root())
        {
            self.cancel(state);
            self.bridge = None;
            self.tell(state,"Live game-master access disconnected after changing worlds. /connect here to re-enable.");
        }
        for _ in 0..4 {
            let Some(request) = self
                .bridge
                .as_ref()
                .and_then(|h| h.requests.try_recv().ok())
            else {
                break;
            };
            use super::director_bridge::Command;
            let result = (|| -> Result<Value, String> {
                match request.command {
                    Command::Observe => Ok(self.bridge_observation(state)),
                    Command::Poll => Ok(
                        json!({"ready":self.external.as_ref().is_some_and(|e|e.sender.is_some())}),
                    ),
                    Command::Request { prompt } => {
                        let prompt = prompt.trim().to_string();
                        self.start(state, &prompt, false, Instant::now())?;
                        state.chat.push(format!("Client: {prompt}"));
                        Ok(self.bridge_observation(state))
                    }
                    Command::Apply { request_id, plan } => {
                        if serde_json::to_vec(&plan).map_err(|e| e.to_string())?.len() > 32768 {
                            return Err("plan exceeds 32 KiB".into());
                        }
                        let plan: Plan = serde_json::from_value(plan).map_err(|e| e.to_string())?;
                        plan.validate()?;
                        let external = self
                            .external
                            .as_mut()
                            .filter(|e| e.id == request_id)
                            .ok_or("stale or cancelled request_id; observe again")?;
                        external
                            .sender
                            .take()
                            .ok_or("plan already submitted; observe its result")?
                            .send(Ok(plan))
                            .map_err(|_| "request closed".to_string())?;
                        Ok(
                            json!({"accepted":true,"applied":false,"request_id":request_id,"next":"director_observe"}),
                        )
                    }
                    Command::Cancel => {
                        self.cancel(state);
                        Ok(self.bridge_observation(state))
                    }
                }
            })();
            let _ = request.reply.send(result);
        }
    }
    pub fn poll(&mut self, state: &mut GameState, now: Instant) {
        if self.external.is_some()
            && self
                .pending
                .as_ref()
                .is_some_and(|p| now.duration_since(p.started) >= Duration::from_secs(90))
        {
            self.cancel(state);
            self.tell(
                state,
                "Live game-master request timed out; send it again when your client is ready.",
            );
        }
        self.poll_bridge(state);
        if self.pending.as_ref().is_some_and(|p| {
            p.dimension != state.current_dimension
                || p.root != state.world.save_root()
                || state.is_showing_death_screen()
        }) {
            self.cancel(state);
        }
        if let Some(pending) = &self.pending {
            state.chat.status = format!(
                "Game master: planning… {:.0}s (T: chat, /cancel)",
                pending.started.elapsed().as_secs_f32()
            );
            if state.chat.open || state.inventory_open || state.is_settings_menu_open() {
                return;
            }
            let result = match pending.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Game master worker stopped.".into()))
                }
            };
            if let Some(result) = result {
                let pending = self.pending.take().unwrap();
                state.chat.pending = false;
                let elapsed = pending.started.elapsed().as_secs_f32();
                let result = result.and_then(|plan| {
                    if pending.proactive && state.player.health < state.player.max_health*0.7 {return Err("Proactive event skipped because your health dropped.".into());}
                    if pending.proactive && self.config.proactive.as_deref()!=Some("combat")
                        && plan.actions.iter().any(|a|matches!(a,Action::Spawn{mob:Mob::Zombie|Mob::Skeleton|Mob::Spider,..}|Action::Weather{weather:Weather::Storm})) {
                        return Err("Proactive hostile encounters/storms require the combat preference.".into());
                    }
                    if (state.player.x.floor() - pending.x as f64).abs() > 48.0 { return Err("You moved too far from the requested area; send the request again here.".into()); }
                    self.apply(state, &plan, pending.x, &pending.request)
                });
                if let Some(external) = self.external.take() {
                    self.bridge_result = match &result {
                        Ok(message) => {
                            json!({"request_id":external.id,"status":"applied","message":message})
                        }
                        Err(error) => {
                            json!({"request_id":external.id,"status":"failed","error":error})
                        }
                    };
                }
                match result {
                    Ok(message) => self.tell(state, format!("{message} ({elapsed:.1}s)")),
                    Err(error) => {
                        self.tell(state, format!("Request failed: {error} ({elapsed:.1}s)"))
                    }
                }
            }
        }
        if let Some(event) = self.encounter
            && state.current_dimension == Dimension::Overworld
        {
            let remaining = state
                .zombies
                .iter()
                .filter(|m| m.director_event == event)
                .count()
                + state
                    .skeletons
                    .iter()
                    .filter(|m| m.director_event == event)
                    .count()
                + state
                    .spiders
                    .iter()
                    .filter(|m| m.director_event == event)
                    .count();
            if remaining == 0 {
                self.encounter = None;
                self.tell(
                    state,
                    "Encounter ended: no enemies from that wave remain nearby.",
                );
            }
        }
        if self.pending.is_none() && now >= self.toast_until {
            state.chat.status.clear();
        }
        self.consider_proactive(state, now);
    }
    fn consider_proactive(&mut self, state: &mut GameState, now: Instant) {
        if self.config.proactive.is_none()
            || self.proactive_count >= MAX_PROACTIVE
            || self.pending.is_some()
            || self.encounter.is_some()
            || state.chat.open
            || state.inventory_open
            || state.is_settings_menu_open()
            || state.is_showing_death_screen()
            || state.is_showing_credits()
            || state.is_showing_startup_splash()
            || state.current_dimension != Dimension::Overworld
            || state.player.health < state.player.max_health * 0.7
            || now.duration_since(self.last_event) < COOLDOWN
            || (state.player.x - self.last_area_x as f64).abs() < 64.0
        {
            return;
        }
        self.last_event = now;
        self.last_area_x = state.player.x.floor() as i32;
        self.proactive_count += 1;
        if let Err(error) = self.start(state, "The player entered a new area. Plan one small event matching their preference, or no actions if this is a poor moment.", true, now) {
            self.tell(state, error);
        }
    }
    fn apply(
        &mut self,
        state: &mut GameState,
        plan: &Plan,
        origin: i32,
        request: &str,
    ) -> Result<String, String> {
        plan.validate()?;
        let prepared = prepare(state, plan, origin)?;
        let mut next = self.config.clone();
        if !prepared.edits.is_empty() {
            next.edits.push(prepared.edits.clone());
            if next.edits.len() > 8 {
                next.edits.remove(0);
            }
            next.write(state.world.save_root())?; // Journal before changing blocks.
            self.config = next.clone();
        }
        if prepared.environments_changed {
            environment::write(state.world.save_root(), &prepared.environments)?;
            state.world.environments = prepared.environments;
        }
        // Attachments are applied after their room/floor surfaces, independent
        // of the model's rectangle order or the coordinate-sorted journal.
        for edit in prepared
            .edits
            .iter()
            .filter(|e| e.after != BlockType::Torch)
            .chain(
                prepared
                    .edits
                    .iter()
                    .filter(|e| e.after == BlockType::Torch),
            )
        {
            state.world.set_block(edit.x, edit.y, edit.after);
        }
        let event = self
            .encounter
            .unwrap_or_else(|| rand::random::<u64>().max(1));
        let mut hostiles = 0;
        for (mob, x, y) in prepared.spawns {
            match mob {
                Mob::Zombie => {
                    let mut m = Zombie::new(x, y);
                    m.director_event = event;
                    state.zombies.push(m);
                    hostiles += 1;
                }
                Mob::Skeleton => {
                    let mut m = Skeleton::new(x, y);
                    m.director_event = event;
                    state.skeletons.push(m);
                    hostiles += 1;
                }
                Mob::Spider => {
                    let mut m = Spider::new(x, y);
                    m.director_event = event;
                    state.spiders.push(m);
                    hostiles += 1;
                }
                Mob::Cow => state.cows.push(Cow::new(x, y)),
                Mob::Sheep => state.sheep.push(Sheep::new(x, y)),
                Mob::Pig => state.pigs.push(Pig::new(x, y)),
                Mob::Chicken => state.chickens.push(Chicken::new(x, y)),
            }
        }
        if hostiles > 0 {
            self.encounter = Some(event);
        }
        for action in &plan.actions {
            match action {
                Action::Weather { weather } => state.director_weather(weather.game()),
                Action::Time { phase } => {
                    state.time_of_day = match phase {
                        Phase::Day => 12000.0,
                        Phase::Night => 22000.0,
                        Phase::Dawn => 4000.0,
                        Phase::Dusk => 20000.0,
                    }
                }
                _ => {}
            }
        }
        let result = if prepared.summary.is_empty() {
            plan.message.clone()
        } else {
            format!("{} {}", plan.message, prepared.summary.join("; "))
        };
        next.recent.push(Turn {
            request: request.into(),
            result: {
                let mut text = String::new();
                for c in result.chars() {
                    if text.len() + c.len_utf8() > 1200 {
                        break;
                    }
                    text.push(c);
                }
                text
            },
        });
        while next.recent.len() > 6 {
            next.recent.remove(0);
        }
        self.config = next;
        self.config_dirty = true;
        let save = state
            .try_persist_world_and_progress()
            .map_err(|e| e.to_string())
            .and_then(|()| self.config.write(state.world.save_root()));
        match save {
            Ok(()) => {
                self.config_dirty = false;
                Ok(result)
            }
            Err(error) => {
                state.save_error.replace(Some(format!(
                    "Game master changes are in memory; save failed: {error}"
                )));
                Ok(format!(
                    "{result} Applied, but saving failed; keep this session open and retry saving."
                ))
            }
        }
    }
    pub fn save(&mut self, state: &GameState) -> Result<(), String> {
        if self.config_dirty && self.config_valid {
            self.config.write(state.world.save_root())?;
            self.config_dirty = false;
        }
        Ok(())
    }
    pub(crate) fn write_recovery(&self, root: &Path) -> Result<(), String> {
        if self.config_valid {
            self.config.write(root)
        } else {
            Ok(())
        }
    }
}

fn design(
    mut request: Value,
    mut runner: impl FnMut(&Value) -> Result<Vec<u8>, String>,
) -> Result<Plan, String> {
    for attempt in 1..=2 {
        let bytes = runner(&request)?;
        if bytes.len() > 32768 {
            return Err("game master response exceeds 32 KiB".into());
        }
        let parsed = serde_json::from_slice::<Value>(&bytes);
        let result = parsed
            .as_ref()
            .map_err(|e| e.to_string())
            .and_then(|v| serde_json::from_value::<Plan>(v.clone()).map_err(|e| e.to_string()))
            .and_then(|plan| {
                plan.validate()?;
                Ok(plan)
            });
        match result {
            Ok(plan) => return Ok(plan),
            Err(error) if attempt < 2 => {
                request["previous_design"] = parsed.unwrap_or(Value::Null);
                request["validation_error"] = json!(error);
            }
            Err(error) => return Err(format!("no valid plan after two attempts: {error}")),
        }
    }
    unreachable!()
}

fn mob_count(s: &GameState) -> usize {
    [
        s.zombies.len(),
        s.skeletons.len(),
        s.spiders.len(),
        s.creepers.len(),
        s.silverfish.len(),
        s.slimes.len(),
        s.endermen.len(),
        s.blazes.len(),
        s.pigmen.len(),
        s.ghasts.len(),
        s.cows.len(),
        s.sheep.len(),
        s.pigs.len(),
        s.chickens.len(),
        s.squids.len(),
        s.wolves.len(),
        s.ocelots.len(),
        s.villagers.len(),
    ]
    .into_iter()
    .sum()
}
fn surface(world: &World, x: i32) -> Option<i32> {
    // Find the upper ground before checking headroom. Searching for the first
    // *clear* ground can skip a blocked surface and silently choose a cave floor.
    let y = (0..128).find(|&y| {
        matches!(
            world.get_block(x, y),
            BlockType::Grass
                | BlockType::Dirt
                | BlockType::Stone
                | BlockType::Sand
                | BlockType::Snow
                | BlockType::Gravel
                | BlockType::Ice
        )
    })?;
    ((3..126).contains(&y)
        && world.get_block(x, y - 1) == BlockType::Air
        && world.get_block(x, y - 2) == BlockType::Air)
        .then_some(y)
}
struct Prepared {
    edits: Vec<Edit>,
    spawns: Vec<(Mob, f64, f64)>,
    environments: Vec<Region>,
    environments_changed: bool,
    summary: Vec<String>,
}
fn prepare(state: &mut GameState, plan: &Plan, origin: i32) -> Result<Prepared, String> {
    plan.validate()?;
    if state.current_dimension != Dimension::Overworld {
        return Err("Overworld only".into());
    }
    if origin.abs_diff(0) > 99_900
        && plan.actions.iter().any(|a| {
            matches!(
                a,
                Action::Spawn { .. } | Action::Structure { .. } | Action::Environment { .. }
            )
        })
    {
        return Err("World authoring currently requires a request area within x=±99900.".into());
    }
    if plan
        .actions
        .iter()
        .any(|a| matches!(a, Action::Spawn { .. } | Action::Structure { .. }))
    {
        state.world.load_chunks_for_spawn_search(origin, 80);
    }
    state.world.check_storage().map_err(|e| e.to_string())?;
    let mut out = Prepared {
        edits: Vec::new(),
        spawns: Vec::new(),
        environments: state.world.environments.clone(),
        environments_changed: false,
        summary: Vec::new(),
    };
    for action in &plan.actions {
        match action {
            Action::Spawn {
                mob,
                count,
                side,
                distance,
            } => {
                if matches!(mob, Mob::Zombie | Mob::Skeleton | Mob::Spider)
                    && state.difficulty_name() == "Peaceful"
                {
                    return Err("Hostile battles require a non-Peaceful difficulty; change it yourself with P.".into());
                }
                for i in 0..*count as i32 {
                    let x = origin + side.sign() * (*distance as i32 + i * 2);
                    let y = surface(&state.world, x)
                        .ok_or("No safe dry spawn site; try another area.")?;
                    let (half_width, height) = if matches!(mob, Mob::Spider) {
                        (0.6, 0.9)
                    } else {
                        (0.3, 1.9)
                    };
                    if state.entity_collides_with_world(
                        x as f64 + 0.5,
                        y as f64,
                        half_width,
                        height,
                    ) {
                        return Err(
                            "The mob's body would overlap a block; try a clearer area.".into()
                        );
                    }
                    if state.director_near_player(x as f64, 6.0) {
                        return Err(
                            "Spawn site is too close to the current player position.".into()
                        );
                    }
                    if out
                        .spawns
                        .iter()
                        .any(|(_, px, _)| (*px - x as f64 - 0.5).abs() < 1.0)
                    {
                        return Err(
                            "Spawn actions overlap; request one mixed wave with distinct sites."
                                .into(),
                        );
                    }
                    out.spawns.push((*mob, x as f64 + 0.5, y as f64));
                }
                out.summary
                    .push(format!("added {count} {mob:?} {side:?} of x={origin}"));
            }
            Action::Environment { terrain, radius } => {
                out.environments = environment::replace(
                    &out.environments,
                    Region {
                        start_x: origin - *radius as i32,
                        end_x: origin + *radius as i32,
                        terrain: *terrain,
                    },
                )?;
                out.environments_changed = true;
                out.summary.push(format!(
                    "{terrain:?} ambience x={}..{}; terrain/builds kept",
                    origin - *radius as i32,
                    origin + *radius as i32
                ));
            }
            Action::Structure {
                name,
                side,
                distance,
                blocks,
            } => {
                let mut cells = BTreeMap::new();
                for r in blocks {
                    for x in r.x..r.x + r.width as i32 {
                        for y in r.y..r.y + r.height as i32 {
                            cells.insert((x, y), r.block.block());
                        }
                    }
                }
                cells.retain(|_, b| *b != BlockType::Air);
                if cells.is_empty() {
                    return Err("Structure contains no blocks.".into());
                }
                let width = cells.keys().map(|(x, _)| *x).max().unwrap() + 1;
                let height = -cells.keys().map(|(_, y)| *y).min().unwrap();
                let entrance_clear = |dx: i32| {
                    [1, 2]
                        .into_iter()
                        .all(|dy| !cells.get(&(dx, -dy)).is_some_and(|b| b.is_solid()))
                };
                if !entrance_clear(0) && !entrance_clear(width - 1) {
                    return Err(
                        "Structure needs a two-block-high entrance at its left or right edge."
                            .into(),
                    );
                }
                let mut placement = None;
                for offset in 0..=12 {
                    let x = origin + side.sign() * (*distance as i32 + offset)
                        - if matches!(side, Side::West) {
                            width - 1
                        } else {
                            0
                        };
                    if state.director_near_player((x + width / 2) as f64, width as f64 / 2.0 + 4.0)
                    {
                        continue;
                    }
                    let Some(y) = surface(&state.world, x) else {
                        continue;
                    };
                    if y - height < 2 {
                        continue;
                    }
                    if (x..x + width).all(|px| {
                        surface(&state.world, px) == Some(y)
                            && (y - height..y)
                                .all(|py| state.world.get_block(px, py) == BlockType::Air)
                    }) {
                        placement = Some((x, y));
                        break;
                    }
                }
                let (x,y)=placement.ok_or("No flat, empty footprint for the structure; existing terrain/builds were kept.")?;
                if out
                    .spawns
                    .iter()
                    .any(|(_, px, _)| *px >= x as f64 - 2.0 && *px <= (x + width + 2) as f64)
                {
                    return Err(
                        "Structure and mob spawn sites overlap; separate these requests.".into(),
                    );
                }
                for ((dx, dy), block) in cells {
                    out.edits.push(Edit {
                        x: x + dx,
                        y: y + dy,
                        before: BlockType::Air,
                        after: block,
                    });
                }
                out.summary
                    .push(format!("placed {name} at x={x}, ground y={y}"));
            }
            Action::Weather { weather } => out
                .summary
                .push(format!("weather: {weather:?}; follows weather-cycle rules")),
            Action::Time { phase } => out.summary.push(format!("time: {phase:?}")),
        }
    }
    if mob_count(state) + out.spawns.len() > 120 {
        return Err(
            "The nearby mob population would exceed 120; finish the current encounter first."
                .into(),
        );
    }
    // Also catch overlap when a spawn action follows the structure action.
    if out.spawns.iter().any(|(_, x, y)| {
        out.edits
            .iter()
            .any(|e| (e.x as f64 + 0.5 - *x).abs() < 2.0 && (e.y as f64 - *y).abs() < 3.0)
    }) {
        return Err("Structure and mob spawn sites overlap.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BiomeType, recipe::BlueprintBlock};
    use crate::worlds::WorldStore;

    fn fixture() -> (PathBuf, GameState, Director) {
        let root = std::env::temp_dir().join(format!(
            "termcraft-director-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let mut recipe = WorldRecipe::example();
        recipe.terrain = Terrain::Plains;
        recipe.relief = 0;
        recipe.landmarks.clear();
        recipe.regions.clear();
        let path = WorldStore::new(root.clone())
            .create("test", "director fixture", &recipe)
            .unwrap();
        let mut state = GameState::open_in_directory(&path).unwrap();
        state.dismiss_startup_splash();
        let director = Director::new(&mut state);
        (root, state, director)
    }
    fn tower() -> Action {
        let r = |x, y, width, height, block| BlockRect {
            x,
            y,
            width,
            height,
            block,
        };
        Action::Structure {
            name: "Test tower".into(),
            side: Side::East,
            distance: 24,
            blocks: vec![
                r(0, -6, 7, 6, BlueprintBlock::StoneBricks),
                r(1, -5, 5, 5, BlueprintBlock::Air),
                r(0, -2, 1, 2, BlueprintBlock::Air),
                r(3, -1, 1, 1, BlueprintBlock::Chest),
            ],
        }
    }
    fn finish(root: PathBuf, state: GameState, director: Director) {
        drop(director);
        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn authoring_preserves_coop_space_and_rejects_spawn_plans_atomically() {
        let (root, mut state, mut director) = fixture();
        state.join_peer(1);
        state.join_peer(2);
        state.activate_peer(2);
        state.player.x = 25.0;
        state.activate_peer(1);
        let before = state.weather;
        let mobs = Plan {
            message: "Must not spawn on a guest".into(),
            actions: vec![
                Action::Weather {
                    weather: Weather::Rain,
                },
                Action::Spawn {
                    mob: Mob::Zombie,
                    count: 1,
                    side: Side::East,
                    distance: 24,
                },
            ],
        };
        assert!(director.apply(&mut state, &mobs, 0, "mob").is_err());
        assert_eq!(state.weather, before);
        assert!(state.zombies.is_empty());
        assert!(director.config.edits.is_empty());
        let structure = Plan {
            message: "Find room beyond the guest".into(),
            actions: vec![tower()],
        };
        director.apply(&mut state, &structure, 0, "tower").unwrap();
        assert!(!director.config.edits.is_empty());
        assert!(
            director.config.edits[0]
                .iter()
                .all(|e| (e.x as f64 - 25.0).abs() >= 4.0)
        );
        finish(root, state, director);
    }

    #[test]
    fn structure_torches_are_applied_after_their_new_loft_support() {
        let (root, mut state, mut director) = fixture();
        let mut action = tower();
        if let Action::Structure { blocks, .. } = &mut action {
            blocks.push(BlockRect {
                x: 2,
                y: -4,
                width: 1,
                height: 1,
                block: BlueprintBlock::Torch,
            });
            blocks.push(BlockRect {
                x: 2,
                y: -3,
                width: 1,
                height: 1,
                block: BlueprintBlock::Planks,
            });
        }
        let plan = Plan {
            message: "A supported light".into(),
            actions: vec![action],
        };
        director.apply(&mut state, &plan, 0, "loft light").unwrap();
        let torch = director.config.edits[0]
            .iter()
            .find(|e| e.after == BlockType::Torch)
            .unwrap();
        assert_eq!(state.world.get_block(torch.x, torch.y), BlockType::Torch);
        assert!(state.world.torch_attachment(torch.x, torch.y).is_some());
        assert!(
            !state
                .world
                .recent_environment_drops
                .iter()
                .any(|e| e.2 == crate::world::item::ItemType::Torch)
        );
        finish(root, state, director);
    }

    #[test]
    fn changes_current_world_and_preserves_edits_across_reopen_and_undo() {
        let (root, mut state, mut director) = fixture();
        let plan = Plan {
            message: "A tower rises in snowy rain.".into(),
            actions: vec![
                tower(),
                Action::Environment {
                    terrain: Terrain::Tundra,
                    radius: 32,
                },
                Action::Weather {
                    weather: Weather::Rain,
                },
            ],
        };
        let result = director.apply(&mut state, &plan, 0, "snowy tower").unwrap();
        assert!(result.contains("placed Test tower"));
        assert_eq!(state.weather, WeatherType::Rain);
        assert_eq!(state.world.get_biome(0), BiomeType::Tundra);
        let edit = director.config.edits[0]
            .iter()
            .find(|e| e.after == BlockType::StoneBricks)
            .unwrap()
            .clone();
        let chest = director.config.edits[0]
            .iter()
            .find(|e| e.after == BlockType::Chest)
            .unwrap()
            .clone();
        state
            .world
            .ensure_chest_inventory(chest.x, chest.y, 27)
            .unwrap()
            .add_item(crate::world::item::ItemType::Diamond, 3);
        state.world.set_block(edit.x, edit.y, BlockType::GoldOre);
        state.try_persist_world_and_progress().unwrap();
        let path = state.world.save_root().to_path_buf();
        drop(director);
        drop(state);
        let mut state = GameState::open_in_directory(&path).unwrap();
        let mut director = Director::new(&mut state);
        assert_eq!(state.world.get_biome(0), BiomeType::Tundra);
        assert_eq!(director.config.recent[0].request, "snowy tower");
        director.command(&mut state, "/undo").unwrap();
        assert_eq!(state.world.get_block(edit.x, edit.y), BlockType::GoldOre);
        assert_eq!(state.world.get_block(chest.x, chest.y), BlockType::Chest);
        assert_eq!(
            state
                .world
                .chest_inventory(chest.x, chest.y)
                .unwrap()
                .slots
                .iter()
                .flatten()
                .filter(|stack| stack.item_type == crate::world::item::ItemType::Diamond)
                .map(|stack| stack.count)
                .sum::<u32>(),
            3
        );
        assert!(director.config.edits.is_empty());
        finish(root, state, director);
    }
    #[test]
    fn rejects_entire_plan_when_placement_or_environment_save_fails() {
        let (root, mut state, mut director) = fixture();
        state.world.load_chunks_for_spawn_search(0, 80);
        for x in 20..=64 {
            for y in 1..128 {
                state.world.set_block(x, y, BlockType::Stone);
            }
        }
        let plan = Plan {
            message: "Storm and tower".into(),
            actions: vec![
                Action::Weather {
                    weather: Weather::Storm,
                },
                tower(),
            ],
        };
        assert!(director.apply(&mut state, &plan, 0, "tower").is_err());
        assert_eq!(state.weather, WeatherType::Clear);
        assert!(director.config.edits.is_empty());
        std::fs::create_dir(state.world.save_root().join("environment.json")).unwrap();
        let plan = Plan {
            message: "Storm and snow".into(),
            actions: vec![
                Action::Weather {
                    weather: Weather::Storm,
                },
                Action::Environment {
                    terrain: Terrain::Tundra,
                    radius: 24,
                },
            ],
        };
        assert!(director.apply(&mut state, &plan, 0, "snow").is_err());
        assert_eq!(state.weather, WeatherType::Clear);
        assert_eq!(state.world.get_biome(0), BiomeType::Plains);
        finish(root, state, director);
    }
    #[test]
    fn validates_bounds_and_repairs_once_without_retrying_transport_failure() {
        let mut calls = 0;
        let plan=design(json!({}),|request|{
            calls+=1;
            if calls==1 {Ok(br#"{"message":"bad","actions":[{"type":"spawn","mob":"zombie","count":99,"side":"east","distance":12}]}"#.to_vec())}
            else {assert!(request["validation_error"].as_str().unwrap().contains("count"));Ok(serde_json::to_vec(&example()).unwrap())}
        }).unwrap();
        assert_eq!(calls, 2);
        assert_eq!(plan.actions.len(), 2);
        let mut calls = 0;
        assert!(
            design(json!({}), |_| {
                calls += 1;
                Err("offline".into())
            })
            .is_err()
        );
        assert_eq!(calls, 1);
        assert!(design(json!({}), |_| Ok(vec![b' '; 32769])).is_err());
        let mut plan: Plan = serde_json::from_value(example()).unwrap();
        plan.actions.push(Action::Environment {
            terrain: Terrain::Plains,
            radius: 200,
        });
        assert!(plan.validate().is_err());
        assert!(
            serde_json::from_value::<Plan>(json!({"message":"bad","actions":[],"execute":"shell"}))
                .is_err()
        );
    }
    #[test]
    fn mobs_are_real_bounded_encounters_and_have_an_end() {
        let (root, mut state, mut director) = fixture();
        let plan = Plan {
            message: "A battle".into(),
            actions: vec![
                Action::Spawn {
                    mob: Mob::Zombie,
                    count: 4,
                    side: Side::West,
                    distance: 16,
                },
                Action::Time {
                    phase: Phase::Night,
                },
            ],
        };
        director.apply(&mut state, &plan, 0, "battle").unwrap();
        assert_eq!(state.zombies.len(), 4);
        assert!(director.encounter.is_some());
        assert!(
            state
                .zombies
                .iter()
                .all(|m| m.director_event != 0 && m.x < -6.0)
        );
        state.zombies.clear();
        director.poll(&mut state, Instant::now());
        assert!(director.encounter.is_none());
        assert!(state.chat.status.contains("Encounter ended"));
        // A spider is wider than a column: clear headroom at its center alone
        // must not allow spawning its body into a neighboring player wall.
        let y = surface(&state.world, 16).unwrap();
        state.world.set_block(17, y - 1, BlockType::GoldOre);
        let blocked = Plan {
            message: "Spider".into(),
            actions: vec![Action::Spawn {
                mob: Mob::Spider,
                count: 1,
                side: Side::East,
                distance: 16,
            }],
        };
        assert!(director.apply(&mut state, &blocked, 0, "spider").is_err());
        assert!(state.spiders.is_empty());
        assert_eq!(state.world.get_block(17, y - 1), BlockType::GoldOre);
        assert!(prepare(&mut state, &blocked, i32::MAX).is_err());
        finish(root, state, director);
    }
    #[test]
    fn blocked_surface_does_not_redirect_a_mob_request_into_a_cave() {
        let (root, mut state, mut director) = fixture();
        let ground = surface(&state.world, 16).unwrap();
        state.world.set_block(16, ground - 1, BlockType::Ladder);
        // A clear cave below the blocked surface was previously accepted.
        state.world.set_block(16, ground + 8, BlockType::Stone);
        state.world.set_block(16, ground + 7, BlockType::Air);
        state.world.set_block(16, ground + 6, BlockType::Air);
        assert_eq!(surface(&state.world, 16), None);
        let before_time = state.time_of_day;
        let plan = Plan {
            message: "A surface battle".into(),
            actions: vec![
                Action::Time {
                    phase: Phase::Night,
                },
                Action::Spawn {
                    mob: Mob::Zombie,
                    count: 1,
                    side: Side::East,
                    distance: 16,
                },
            ],
        };
        assert!(director.apply(&mut state, &plan, 0, "battle").is_err());
        assert!(state.zombies.is_empty());
        assert_eq!(
            state.time_of_day, before_time,
            "whole plan must remain unapplied"
        );
        finish(root, state, director);
    }
    #[test]
    fn pending_requests_cancel_on_dimension_change_and_ignore_late_results() {
        let (root, mut state, mut director) = fixture();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        director.pending = Some(Pending {
            receiver: rx,
            cancelled: cancel.clone(),
            root: state.world.save_root().to_path_buf(),
            dimension: Dimension::Overworld,
            x: 0,
            request: "storm".into(),
            started: Instant::now(),
            proactive: false,
        });
        state.chat.pending = true;
        state.current_dimension = Dimension::Nether;
        director.poll(&mut state, Instant::now());
        assert!(cancel.load(Ordering::Relaxed));
        assert!(!state.chat.pending);
        assert!(
            tx.send(Ok(serde_json::from_value(example()).unwrap()))
                .is_err()
        );
        assert_eq!(state.weather, WeatherType::Clear);
        finish(root, state, director);
    }
    #[test]
    fn proactive_events_obey_preferences_cooldown_budget_and_pause() {
        let (root, mut state, mut director) = fixture();
        // A fake in-flight planner exercises scheduling without API/process calls.
        director.program = Some(PathBuf::from("/nonexistent-termcraft-test-runner"));
        director.command(&mut state, "/auto exploration").unwrap();
        let now = director.last_event + COOLDOWN + Duration::from_secs(1);
        state.player.x = 80.5;
        state.chat.open = true;
        director.consider_proactive(&mut state, now);
        assert!(director.pending.is_none());
        state.chat.open = false;
        state.player.health = 2.0;
        director.consider_proactive(&mut state, now);
        assert!(director.pending.is_none());
        state.player.health = 20.0;
        director.consider_proactive(&mut state, now);
        assert!(director.pending.as_ref().unwrap().proactive);
        assert_eq!(director.proactive_count, 1);
        director.command(&mut state, "/pause").unwrap();
        assert!(director.pending.is_none());
        assert!(director.config.proactive.is_none());
        director.config.proactive = Some("exploration".into());
        director.proactive_count = 3;
        state.player.x = 180.0;
        director.consider_proactive(&mut state, now + COOLDOWN);
        assert!(director.pending.is_none());
        finish(root, state, director);
    }
    #[test]
    fn proactive_result_waits_for_chat_and_rechecks_health_and_preference() {
        let (root, mut state, mut director) = fixture();
        director.config.proactive = Some("exploration".into());
        let queue = |director: &mut Director, state: &mut GameState, plan: Plan| {
            let (tx, rx) = mpsc::channel();
            tx.send(Ok(plan)).unwrap();
            director.pending = Some(Pending {
                receiver: rx,
                cancelled: Arc::new(AtomicBool::new(false)),
                root: state.world.save_root().to_path_buf(),
                dimension: Dimension::Overworld,
                x: 0,
                request: "new area event".into(),
                started: Instant::now(),
                proactive: true,
            });
            state.chat.pending = true;
        };
        queue(
            &mut director,
            &mut state,
            Plan {
                message: "Rain arrives".into(),
                actions: vec![Action::Weather {
                    weather: Weather::Rain,
                }],
            },
        );
        state.chat.open = true;
        director.poll(&mut state, Instant::now());
        assert_eq!(state.weather, WeatherType::Clear);
        assert!(director.pending.is_some());
        state.chat.open = false;
        director.poll(&mut state, Instant::now());
        assert_eq!(state.weather, WeatherType::Rain);
        assert!(director.pending.is_none());
        queue(
            &mut director,
            &mut state,
            Plan {
                message: "Storm".into(),
                actions: vec![Action::Weather {
                    weather: Weather::Storm,
                }],
            },
        );
        director.poll(&mut state, Instant::now());
        assert_eq!(state.weather, WeatherType::Rain);
        assert!(state.chat.status.contains("require the combat preference"));
        director.config.proactive = Some("combat".into());
        state.player.health = 2.0;
        queue(
            &mut director,
            &mut state,
            Plan {
                message: "Storm".into(),
                actions: vec![Action::Weather {
                    weather: Weather::Storm,
                }],
            },
        );
        director.poll(&mut state, Instant::now());
        assert_eq!(state.weather, WeatherType::Rain);
        assert!(state.chat.status.contains("health dropped"));
        finish(root, state, director);
    }

    #[test]
    fn typed_chat_pauses_physics_and_does_not_run_actions() {
        let (root, mut state, director) = fixture();
        state.chat.open = true;
        state.moving_right = true;
        state.player.vx = 0.5;
        let position = (state.player.x, state.player.y);
        state.chat.insert("I want a battle\u{1b}\n");
        state.update(0, 0);
        assert_eq!((state.player.x, state.player.y), position);
        assert_eq!(state.chat.draft, "I want a battle");
        state.chat.insert(&"x".repeat(3000));
        assert!(state.chat.draft.len() <= MAX_PROMPT);
        finish(root, state, director);
    }
    #[test]
    fn corrupt_preferences_are_preserved_until_explicit_reset() {
        let (root, mut state, director) = fixture();
        drop(director);
        let path = state.world.save_root().join(CONFIG_FILE);
        std::fs::write(&path, b"bad preferences").unwrap();
        let mut director = Director::new(&mut state);
        assert!(!director.config_valid);
        assert!(director.command(&mut state, "/pause").is_err());
        director.save(&state).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"bad preferences");
        director.command(&mut state, "/reset").unwrap();
        assert!(director.config_valid);
        assert!(
            std::fs::read_dir(state.world.save_root())
                .unwrap()
                .any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("director.backup"))
        );
        finish(root, state, director);
    }
}
