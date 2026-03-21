use super::command::ClientCommand;
use super::state::{GameState, WeatherType};
use crate::entities::player::Player;
use crate::world::Dimension;
use crate::world::block::BlockType;
use crate::world::chunk::CHUNK_WIDTH;
use crate::world::item::ItemType;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub const NET_PROTOCOL_VERSION: u16 = 6;
pub const SERVER_TICK_RATE: u64 = 20;
pub const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:25565";
pub const MAX_NET_FRAME_BYTES: usize = 256 * 1024;
pub const CHUNK_STREAM_RADIUS: i32 = 3;
const CHUNK_DELTA_MAX_CHUNKS_PER_PACKET: usize = 4;
const SERVER_MAX_CATCHUP_TICKS_PER_LOOP: u8 = 6;
const MAX_PENDING_INPUTS_PER_SCHEDULED_TICK: usize = 128;
const MAX_CLIENT_INPUT_LEAD_TICKS: u64 = SERVER_TICK_RATE * 4;
const CLIENT_SEQUENCE_PRUNE_INTERVAL_TICKS: u64 = SERVER_TICK_RATE * 15;
const CLIENT_SEQUENCE_STALE_TICKS: u64 = SERVER_TICK_RATE * 180;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientInputFrame {
    pub protocol: u16,
    pub client_id: u16,
    pub sequence: u32,
    pub tick: u64,
    pub command: ClientCommand,
}

impl ClientInputFrame {
    pub fn new(client_id: u16, sequence: u32, tick: u64, command: ClientCommand) -> Self {
        Self {
            protocol: NET_PROTOCOL_VERSION,
            client_id,
            sequence,
            tick,
            command,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotDimension {
    Overworld,
    Nether,
    End,
}

impl From<Dimension> for SnapshotDimension {
    fn from(value: Dimension) -> Self {
        match value {
            Dimension::Overworld => Self::Overworld,
            Dimension::Nether => Self::Nether,
            Dimension::End => Self::End,
        }
    }
}

impl From<SnapshotDimension> for Dimension {
    fn from(value: SnapshotDimension) -> Self {
        match value {
            SnapshotDimension::Overworld => Self::Overworld,
            SnapshotDimension::Nether => Self::Nether,
            SnapshotDimension::End => Self::End,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotWeather {
    Clear,
    Rain,
    Thunderstorm,
}

impl From<WeatherType> for SnapshotWeather {
    fn from(value: WeatherType) -> Self {
        match value {
            WeatherType::Clear => Self::Clear,
            WeatherType::Rain => Self::Rain,
            WeatherType::Thunderstorm => Self::Thunderstorm,
        }
    }
}

impl From<SnapshotWeather> for WeatherType {
    fn from(value: SnapshotWeather) -> Self {
        match value {
            SnapshotWeather::Clear => Self::Clear,
            SnapshotWeather::Rain => Self::Rain,
            SnapshotWeather::Thunderstorm => Self::Thunderstorm,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerSnapshot {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub sneaking: bool,
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub max_hunger: f32,
}

impl PlayerSnapshot {
    fn from_player(player: &Player) -> Self {
        Self {
            x: player.x,
            y: player.y,
            vx: player.vx,
            vy: player.vy,
            grounded: player.grounded,
            facing_right: player.facing_right,
            sneaking: player.sneaking,
            health: player.health,
            max_health: player.max_health,
            hunger: player.hunger,
            max_hunger: player.max_hunger,
        }
    }

    fn from_state(state: &GameState) -> Self {
        Self::from_player(&state.player)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PeerPlayerSnapshot {
    pub client_id: u16,
    pub player: PlayerSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerSnapshot {
    pub protocol: u16,
    pub tick: u64,
    pub dimension: SnapshotDimension,
    pub weather: SnapshotWeather,
    pub time_of_day: f32,
    pub player: PlayerSnapshot,
    pub remote_players: Vec<PeerPlayerSnapshot>,
    pub hotbar_index: u8,
    pub selected_hotbar_item: Option<ItemType>,
    pub inventory_open: bool,
    pub death_screen_active: bool,
    pub credits_active: bool,
}

impl ServerSnapshot {
    pub fn from_state(tick: u64, state: &GameState) -> Self {
        Self {
            protocol: NET_PROTOCOL_VERSION,
            tick,
            dimension: state.current_dimension.into(),
            weather: state.weather.into(),
            time_of_day: state.time_of_day,
            player: PlayerSnapshot::from_state(state),
            remote_players: Vec::new(),
            hotbar_index: state.hotbar_index,
            selected_hotbar_item: state.inventory.slots[state.hotbar_index as usize]
                .as_ref()
                .map(|s| s.item_type),
            inventory_open: state.inventory_open,
            death_screen_active: state.is_showing_death_screen(),
            credits_active: state.is_showing_credits(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ChunkColumnSnapshot {
    pub chunk_x: i32,
    pub revision: u64,
    pub blocks: Vec<BlockType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ChunkDeltaPacket {
    pub protocol: u16,
    pub tick: u64,
    pub dimension: SnapshotDimension,
    pub center_chunk_x: i32,
    pub chunks: Vec<ChunkColumnSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ServerWireMessage {
    Snapshot(ServerSnapshot),
    ChunkDelta(ChunkDeltaPacket),
}

#[derive(Debug)]
pub enum NetCodecError {
    Codec(Box<bincode::ErrorKind>),
    UnsupportedProtocol { found: u16, expected: u16 },
}

impl std::fmt::Display for NetCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetCodecError::Codec(err) => write!(f, "codec error: {err}"),
            NetCodecError::UnsupportedProtocol { found, expected } => {
                write!(
                    f,
                    "unsupported protocol version: found={found}, expected={expected}"
                )
            }
        }
    }
}

impl std::error::Error for NetCodecError {}

impl From<Box<bincode::ErrorKind>> for NetCodecError {
    fn from(value: Box<bincode::ErrorKind>) -> Self {
        Self::Codec(value)
    }
}

trait ProtocolTagged {
    fn protocol_version(&self) -> u16;
}

impl ProtocolTagged for ClientInputFrame {
    fn protocol_version(&self) -> u16 {
        self.protocol
    }
}

impl ProtocolTagged for ServerSnapshot {
    fn protocol_version(&self) -> u16 {
        self.protocol
    }
}

impl ProtocolTagged for ChunkDeltaPacket {
    fn protocol_version(&self) -> u16 {
        self.protocol
    }
}

impl ProtocolTagged for ServerWireMessage {
    fn protocol_version(&self) -> u16 {
        match self {
            ServerWireMessage::Snapshot(snapshot) => snapshot.protocol,
            ServerWireMessage::ChunkDelta(packet) => packet.protocol,
        }
    }
}

fn validate_protocol(version: u16) -> Result<(), NetCodecError> {
    if version != NET_PROTOCOL_VERSION {
        return Err(NetCodecError::UnsupportedProtocol {
            found: version,
            expected: NET_PROTOCOL_VERSION,
        });
    }
    Ok(())
}

fn encode_protocol_message<T>(message: &T) -> Result<Vec<u8>, NetCodecError>
where
    T: Serialize + ProtocolTagged,
{
    validate_protocol(message.protocol_version())?;
    Ok(bincode::serialize(message)?)
}

fn decode_protocol_message<T>(encoded: &[u8]) -> Result<T, NetCodecError>
where
    T: DeserializeOwned + ProtocolTagged,
{
    let message: T = bincode::deserialize(encoded)?;
    validate_protocol(message.protocol_version())?;
    Ok(message)
}

pub fn encode_client_input(frame: &ClientInputFrame) -> Result<Vec<u8>, NetCodecError> {
    encode_protocol_message(frame)
}

pub fn decode_client_input(encoded: &[u8]) -> Result<ClientInputFrame, NetCodecError> {
    decode_protocol_message(encoded)
}

pub fn encode_server_snapshot(snapshot: &ServerSnapshot) -> Result<Vec<u8>, NetCodecError> {
    encode_protocol_message(snapshot)
}

pub fn decode_server_snapshot(encoded: &[u8]) -> Result<ServerSnapshot, NetCodecError> {
    decode_protocol_message(encoded)
}

pub fn encode_server_message(message: &ServerWireMessage) -> Result<Vec<u8>, NetCodecError> {
    encode_protocol_message(message)
}

pub fn decode_server_message(encoded: &[u8]) -> Result<ServerWireMessage, NetCodecError> {
    decode_protocol_message(encoded)
}

fn codec_error_to_io(err: NetCodecError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

fn write_framed_payload<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let length_u32 = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "payload too large"))?;
    writer.write_all(&length_u32.to_be_bytes())?;
    writer.write_all(payload)?;
    Ok(())
}

fn read_framed_payload<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let frame_len = u32::from_be_bytes(len_buf) as usize;
    if frame_len == 0 || frame_len > MAX_NET_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid frame length: {frame_len}"),
        ));
    }
    let mut payload = vec![0u8; frame_len];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

pub fn write_framed_client_input<W: Write>(
    writer: &mut W,
    frame: &ClientInputFrame,
) -> io::Result<()> {
    let payload = encode_client_input(frame).map_err(codec_error_to_io)?;
    write_framed_payload(writer, &payload)
}

pub fn read_framed_client_input<R: Read>(reader: &mut R) -> io::Result<ClientInputFrame> {
    let payload = read_framed_payload(reader)?;
    decode_client_input(&payload).map_err(codec_error_to_io)
}

pub fn write_framed_server_snapshot<W: Write>(
    writer: &mut W,
    snapshot: &ServerSnapshot,
) -> io::Result<()> {
    let payload = encode_server_snapshot(snapshot).map_err(codec_error_to_io)?;
    write_framed_payload(writer, &payload)
}

pub fn read_framed_server_snapshot<R: Read>(reader: &mut R) -> io::Result<ServerSnapshot> {
    let payload = read_framed_payload(reader)?;
    decode_server_snapshot(&payload).map_err(codec_error_to_io)
}

pub fn write_framed_server_message<W: Write>(
    writer: &mut W,
    message: &ServerWireMessage,
) -> io::Result<()> {
    let payload = encode_server_message(message).map_err(codec_error_to_io)?;
    write_framed_payload(writer, &payload)
}

pub fn read_framed_server_message<R: Read>(reader: &mut R) -> io::Result<ServerWireMessage> {
    let payload = read_framed_payload(reader)?;
    decode_server_message(&payload).map_err(codec_error_to_io)
}

pub struct ServerSimulation {
    state: GameState,
    tick: u64,
    primary_client_id: Option<u16>,
    secondary_players: HashMap<u16, SimulatedClientAvatar>,
    pending_inputs: BTreeMap<u64, Vec<ClientInputFrame>>,
    last_applied_sequence_by_client: HashMap<u16, u32>,
    last_seen_input_tick_by_client: HashMap<u16, u64>,
}

impl Default for ServerSimulation {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerSimulation {
    pub fn new() -> Self {
        Self {
            state: GameState::new(),
            tick: 0,
            primary_client_id: None,
            secondary_players: HashMap::new(),
            pending_inputs: BTreeMap::new(),
            last_applied_sequence_by_client: HashMap::new(),
            last_seen_input_tick_by_client: HashMap::new(),
        }
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn register_client(&mut self, client_id: u16) {
        if self.primary_client_id.is_none() {
            self.primary_client_id = Some(client_id);
            return;
        }
        if self.secondary_players.contains_key(&client_id) {
            return;
        }
        let approx_x = self.state.player.x.round() as i32;
        let (spawn_x, spawn_y) = self.state.multiplayer_join_spawn_near(approx_x);
        self.secondary_players.insert(
            client_id,
            SimulatedClientAvatar::new_from_template(spawn_x, spawn_y, &self.state),
        );
    }

    pub fn unregister_client(&mut self, client_id: u16) {
        self.secondary_players.remove(&client_id);
        self.last_applied_sequence_by_client.remove(&client_id);
        self.last_seen_input_tick_by_client.remove(&client_id);

        if self.primary_client_id != Some(client_id) {
            return;
        }
        self.primary_client_id = None;
        if let Some(next_id) = self.secondary_players.keys().copied().min()
            && let Some(next_player) = self.secondary_players.remove(&next_id)
        {
            self.state.player = next_player.player;
            self.state.hotbar_index = next_player.hotbar_index;
            self.state.inventory_open = next_player.inventory_open;
            self.primary_client_id = Some(next_id);
        }
    }

    pub fn snapshot_for_client(&self, client_id: u16) -> Option<ServerSnapshot> {
        self.primary_client_id?;

        let mut remote_players = Vec::with_capacity(self.secondary_players.len());
        if self.primary_client_id != Some(client_id) {
            remote_players.push(PeerPlayerSnapshot {
                client_id: self.primary_client_id.unwrap_or_default(),
                player: PlayerSnapshot::from_state(&self.state),
            });
        }
        for (&other_id, avatar) in &self.secondary_players {
            if other_id == client_id {
                continue;
            }
            remote_players.push(PeerPlayerSnapshot {
                client_id: other_id,
                player: avatar.player_snapshot(),
            });
        }

        if self.primary_client_id == Some(client_id) {
            let mut snapshot = ServerSnapshot::from_state(self.tick, &self.state);
            snapshot.remote_players = remote_players;
            Some(snapshot)
        } else {
            self.secondary_players
                .get(&client_id)
                .map(|avatar| ServerSnapshot {
                    protocol: NET_PROTOCOL_VERSION,
                    tick: self.tick,
                    dimension: self.state.current_dimension.into(),
                    weather: self.state.weather.into(),
                    time_of_day: self.state.time_of_day,
                    player: avatar.player_snapshot(),
                    remote_players,
                    hotbar_index: avatar.hotbar_index,
                    selected_hotbar_item: avatar.selected_hotbar_item(),
                    inventory_open: avatar.inventory_open,
                    death_screen_active: false,
                    credits_active: false,
                })
        }
    }

    pub fn enqueue_input(&mut self, frame: ClientInputFrame) {
        if self.primary_client_id != Some(frame.client_id)
            && !self.secondary_players.contains_key(&frame.client_id)
        {
            self.register_client(frame.client_id);
        }
        let min_tick = self.tick.saturating_add(1);
        let max_tick = self.tick.saturating_add(MAX_CLIENT_INPUT_LEAD_TICKS);
        let scheduled_tick = frame.tick.clamp(min_tick, max_tick);
        let bucket = self.pending_inputs.entry(scheduled_tick).or_default();
        if bucket.len() >= MAX_PENDING_INPUTS_PER_SCHEDULED_TICK {
            return;
        }
        bucket.push(frame);
    }

    pub fn step(&mut self, mouse_target_x: i32, mouse_target_y: i32) -> ServerSnapshot {
        self.tick = self.tick.saturating_add(1);
        self.apply_inputs_for_tick(self.tick);
        self.prune_inactive_client_sequences();
        self.state.update(mouse_target_x, mouse_target_y);
        for avatar in self.secondary_players.values_mut() {
            let sneaking = avatar.sneaking();
            self.state.step_remote_player_body(
                &mut avatar.player,
                avatar.moving_left,
                avatar.moving_right,
                avatar.jump_held,
                &mut avatar.jump_buffer_ticks,
                sneaking,
            );
        }
        self.snapshot()
    }

    pub fn snapshot(&self) -> ServerSnapshot {
        ServerSnapshot::from_state(self.tick, &self.state)
    }

    fn apply_inputs_for_tick(&mut self, tick: u64) {
        let Some(mut frames) = self.pending_inputs.remove(&tick) else {
            return;
        };

        // Keep per-client receive order intact for same-tick command batches.
        frames.sort_by_key(|frame| frame.client_id);
        for frame in frames {
            if frame.protocol != NET_PROTOCOL_VERSION {
                continue;
            }
            self.last_seen_input_tick_by_client
                .insert(frame.client_id, tick);
            let is_new = self
                .last_applied_sequence_by_client
                .get(&frame.client_id)
                .is_none_or(|last| is_newer_sequence(*last, frame.sequence));
            if !is_new {
                continue;
            }
            if self.primary_client_id == Some(frame.client_id) {
                self.state.apply_client_command(frame.command);
            } else if let Some(avatar) = self.secondary_players.get_mut(&frame.client_id) {
                avatar.apply_command(frame.command);
            }
            self.last_applied_sequence_by_client
                .insert(frame.client_id, frame.sequence);
        }
    }

    fn prune_inactive_client_sequences(&mut self) {
        if !self
            .tick
            .is_multiple_of(CLIENT_SEQUENCE_PRUNE_INTERVAL_TICKS)
        {
            return;
        }

        let threshold = self.tick.saturating_sub(CLIENT_SEQUENCE_STALE_TICKS);
        self.last_seen_input_tick_by_client
            .retain(|client_id, last_seen_tick| {
                let keep = *last_seen_tick >= threshold;
                if !keep {
                    self.last_applied_sequence_by_client.remove(client_id);
                }
                keep
            });
    }
}

fn is_newer_sequence(last: u32, candidate: u32) -> bool {
    let diff = candidate.wrapping_sub(last);
    diff != 0 && diff < (1u32 << 31)
}

#[derive(Clone, Copy, Debug, Default)]
struct PredictionInputState {
    moving_left: bool,
    moving_right: bool,
    sneak_toggled: bool,
    sneak_held: bool,
    _jump_held: bool,
}

impl PredictionInputState {
    fn sneaking(self) -> bool {
        self.sneak_toggled || self.sneak_held
    }

    fn apply_command(&mut self, command: ClientCommand) {
        match command {
            ClientCommand::SetMoveLeft(active) => {
                self.moving_left = active;
                if active {
                    self.moving_right = false;
                }
            }
            ClientCommand::SetMoveRight(active) => {
                self.moving_right = active;
                if active {
                    self.moving_left = false;
                }
            }
            ClientCommand::ClearDirectionalInput => {
                self.moving_left = false;
                self.moving_right = false;
            }
            ClientCommand::ToggleSneak => self.sneak_toggled = !self.sneak_toggled,
            ClientCommand::SetSneakHeld(held) => self.sneak_held = held,
            ClientCommand::SetJumpHeld(held) => self._jump_held = held,
            _ => {}
        }
    }
}

struct SimulatedClientAvatar {
    player: Player,
    moving_left: bool,
    moving_right: bool,
    jump_held: bool,
    jump_buffer_ticks: u8,
    sneak_toggled: bool,
    sneak_held: bool,
    hotbar_index: u8,
    hotbar_items: [Option<ItemType>; 9],
    inventory_open: bool,
}

impl SimulatedClientAvatar {
    fn new_from_template(spawn_x: f64, spawn_y: f64, template: &GameState) -> Self {
        let mut player = Player::new(spawn_x, spawn_y);
        player.max_health = template.player.max_health;
        player.health = template.player.max_health;
        player.max_hunger = template.player.max_hunger;
        player.hunger = template.player.max_hunger;

        let mut hotbar_items = [None; 9];
        for (idx, item) in hotbar_items.iter_mut().enumerate() {
            *item = template.inventory.slots[idx]
                .as_ref()
                .map(|stack| stack.item_type);
        }

        Self {
            player,
            moving_left: false,
            moving_right: false,
            jump_held: false,
            jump_buffer_ticks: 0,
            sneak_toggled: false,
            sneak_held: false,
            hotbar_index: template.hotbar_index.min(8),
            hotbar_items,
            inventory_open: false,
        }
    }

    fn sneaking(&self) -> bool {
        self.sneak_toggled || self.sneak_held
    }

    fn selected_hotbar_item(&self) -> Option<ItemType> {
        self.hotbar_items[self.hotbar_index as usize]
    }

    fn player_snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot::from_player(&self.player)
    }

    fn apply_command(&mut self, command: ClientCommand) {
        match command {
            ClientCommand::QueueJump => {
                if !self.inventory_open {
                    self.jump_buffer_ticks = self.jump_buffer_ticks.max(2);
                }
            }
            ClientCommand::SetJumpHeld(held) => {
                self.jump_held = held && !self.inventory_open;
            }
            ClientCommand::SetMoveLeft(active) => {
                if self.inventory_open {
                    self.moving_left = false;
                    self.moving_right = false;
                    return;
                }
                self.moving_left = active;
                if active {
                    self.moving_right = false;
                }
            }
            ClientCommand::SetMoveRight(active) => {
                if self.inventory_open {
                    self.moving_left = false;
                    self.moving_right = false;
                    return;
                }
                self.moving_right = active;
                if active {
                    self.moving_left = false;
                }
            }
            ClientCommand::ClearDirectionalInput => {
                self.moving_left = false;
                self.moving_right = false;
            }
            ClientCommand::ToggleSneak => self.sneak_toggled = !self.sneak_toggled,
            ClientCommand::SetSneakHeld(held) => self.sneak_held = held,
            ClientCommand::SelectHotbarSlot(idx) => {
                if idx < 9 {
                    self.hotbar_index = idx;
                }
            }
            ClientCommand::ToggleInventory => {
                self.inventory_open = !self.inventory_open;
                if self.inventory_open {
                    self.moving_left = false;
                    self.moving_right = false;
                    self.jump_held = false;
                    self.jump_buffer_ticks = 0;
                }
            }
            ClientCommand::EquipDiamondLoadout => {
                self.hotbar_items = [
                    Some(ItemType::DiamondSword),
                    Some(ItemType::Bow),
                    Some(ItemType::Arrow),
                    Some(ItemType::CookedBeef),
                    Some(ItemType::Cobblestone),
                    Some(ItemType::DiamondPickaxe),
                    Some(ItemType::EnderPearl),
                    None,
                    None,
                ];
                self.hotbar_index = 0;
            }
            ClientCommand::RespawnFromDeathScreen => {
                self.player.health = self.player.max_health;
                self.player.hunger = self.player.max_hunger;
            }
            ClientCommand::SkipCompletionCredits
            | ClientCommand::CycleMovementProfile
            | ClientCommand::CycleDifficulty
            | ClientCommand::CycleGameRulesPreset
            | ClientCommand::ToggleRuleMobSpawning
            | ClientCommand::ToggleRuleDaylightCycle
            | ClientCommand::ToggleRuleWeatherCycle
            | ClientCommand::ToggleRuleKeepInventory
            | ClientCommand::ToggleSettingsMenu
            | ClientCommand::SettingsMoveUp
            | ClientCommand::SettingsMoveDown
            | ClientCommand::SettingsApply
            | ClientCommand::SetPrimaryAction(_)
            | ClientCommand::TravelToOverworld
            | ClientCommand::TravelToNether
            | ClientCommand::TravelToEnd
            | ClientCommand::TravelToSpawn
            | ClientCommand::UseAt(_, _) => {}
        }
        self.player.sneaking = self.sneaking();
    }
}

pub struct ClientPredictionState {
    client_id: u16,
    local_tick: u64,
    next_sequence: u32,
    authoritative_snapshot: Option<ServerSnapshot>,
    predicted_snapshot: Option<ServerSnapshot>,
    pending_inputs: Vec<ClientInputFrame>,
    outbound_inputs: Vec<ClientInputFrame>,
    input_history: Vec<ClientInputFrame>,
    replicated_chunks: HashMap<i32, ChunkColumnSnapshot>,
}

impl ClientPredictionState {
    pub fn new(client_id: u16) -> Self {
        Self {
            client_id,
            local_tick: 0,
            next_sequence: 1,
            authoritative_snapshot: None,
            predicted_snapshot: None,
            pending_inputs: Vec::new(),
            outbound_inputs: Vec::new(),
            input_history: Vec::new(),
            replicated_chunks: HashMap::new(),
        }
    }

    pub fn local_tick(&self) -> u64 {
        self.local_tick
    }

    pub fn authoritative_snapshot(&self) -> Option<&ServerSnapshot> {
        self.authoritative_snapshot.as_ref()
    }

    pub fn predicted_snapshot(&self) -> Option<&ServerSnapshot> {
        self.predicted_snapshot.as_ref()
    }

    pub fn pending_input_count(&self) -> usize {
        self.pending_inputs.len()
    }

    pub fn replicated_chunk_count(&self) -> usize {
        self.replicated_chunks.len()
    }

    pub fn replicated_chunk(&self, chunk_x: i32) -> Option<&ChunkColumnSnapshot> {
        self.replicated_chunks.get(&chunk_x)
    }

    pub fn queue_local_command(&mut self, command: ClientCommand) -> ClientInputFrame {
        let frame = ClientInputFrame::new(
            self.client_id,
            self.next_sequence,
            self.local_tick.saturating_add(1),
            command,
        );
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        self.pending_inputs.push(frame);
        self.outbound_inputs.push(frame);
        self.input_history.push(frame);
        if self.input_history.len() > 8192 {
            let drop = self.input_history.len() - 8192;
            self.input_history.drain(0..drop);
        }
        frame
    }

    pub fn take_outbound_inputs(&mut self) -> Vec<ClientInputFrame> {
        std::mem::take(&mut self.outbound_inputs)
    }

    pub fn advance_local_tick(&mut self) {
        self.local_tick = self.local_tick.saturating_add(1);
        if self.authoritative_snapshot.is_some() {
            self.rebuild_prediction_from_authoritative();
        }
    }

    pub fn apply_server_message(&mut self, message: ServerWireMessage) {
        match message {
            ServerWireMessage::Snapshot(snapshot) => self.apply_authoritative_snapshot(snapshot),
            ServerWireMessage::ChunkDelta(delta) => self.apply_chunk_delta(delta),
        }
    }

    fn apply_authoritative_snapshot(&mut self, snapshot: ServerSnapshot) {
        if let Some(current) = &self.authoritative_snapshot
            && snapshot.tick < current.tick
        {
            return;
        }
        let dimension_changed = self
            .authoritative_snapshot
            .as_ref()
            .is_some_and(|s| s.dimension != snapshot.dimension);
        if dimension_changed {
            self.replicated_chunks.clear();
        }

        self.local_tick = self.local_tick.max(snapshot.tick);
        self.pending_inputs.retain(|f| f.tick > snapshot.tick);
        self.authoritative_snapshot = Some(snapshot);
        self.rebuild_prediction_from_authoritative();
    }

    fn apply_chunk_delta(&mut self, delta: ChunkDeltaPacket) {
        if delta.protocol != NET_PROTOCOL_VERSION {
            return;
        }
        if let Some(authoritative) = &self.authoritative_snapshot
            && authoritative.dimension != delta.dimension
        {
            return;
        }
        for chunk in delta.chunks {
            self.replicated_chunks.insert(chunk.chunk_x, chunk);
        }
    }

    fn rebuild_prediction_from_authoritative(&mut self) {
        let Some(authoritative) = self.authoritative_snapshot.clone() else {
            return;
        };
        let mut predicted = authoritative.clone();
        let mut input_state = self.input_state_at_tick(authoritative.tick);
        let mut sim_tick = authoritative.tick;

        while sim_tick < self.local_tick {
            sim_tick += 1;
            let mut tick_inputs: Vec<ClientInputFrame> = self
                .pending_inputs
                .iter()
                .copied()
                .filter(|input| input.tick == sim_tick)
                .collect();
            tick_inputs.sort_by_key(|f| f.sequence);
            for input in tick_inputs {
                Self::apply_command_to_prediction(&mut predicted, &mut input_state, input.command);
            }
            Self::integrate_predicted_movement(&mut predicted, input_state);
            predicted.tick = sim_tick;
        }

        self.predicted_snapshot = Some(predicted);
    }

    fn input_state_at_tick(&self, tick: u64) -> PredictionInputState {
        let mut state = PredictionInputState::default();
        let mut history: Vec<ClientInputFrame> = self
            .input_history
            .iter()
            .copied()
            .filter(|frame| frame.tick <= tick)
            .collect();
        history.sort_by_key(|frame| (frame.tick, frame.sequence));
        for frame in history {
            state.apply_command(frame.command);
        }
        state
    }

    fn apply_command_to_prediction(
        snapshot: &mut ServerSnapshot,
        input_state: &mut PredictionInputState,
        command: ClientCommand,
    ) {
        input_state.apply_command(command);
        match command {
            ClientCommand::ToggleInventory => {
                snapshot.inventory_open = !snapshot.inventory_open;
                if snapshot.inventory_open {
                    input_state.moving_left = false;
                    input_state.moving_right = false;
                }
            }
            ClientCommand::SelectHotbarSlot(idx) => {
                if idx < 9 {
                    snapshot.hotbar_index = idx;
                }
            }
            ClientCommand::QueueJump => {
                if snapshot.player.grounded {
                    snapshot.player.vy = -0.5;
                    snapshot.player.grounded = false;
                }
            }
            ClientCommand::RespawnFromDeathScreen => {
                snapshot.death_screen_active = false;
                snapshot.player.health = snapshot.player.max_health;
                snapshot.player.hunger = snapshot.player.max_hunger;
            }
            ClientCommand::SkipCompletionCredits => {
                snapshot.credits_active = false;
            }
            ClientCommand::ClearDirectionalInput
            | ClientCommand::SetMoveLeft(_)
            | ClientCommand::SetMoveRight(_)
            | ClientCommand::ToggleSneak
            | ClientCommand::SetSneakHeld(_)
            | ClientCommand::SetJumpHeld(_)
            | ClientCommand::SetPrimaryAction(_)
            | ClientCommand::CycleMovementProfile
            | ClientCommand::CycleDifficulty
            | ClientCommand::CycleGameRulesPreset
            | ClientCommand::ToggleRuleMobSpawning
            | ClientCommand::ToggleRuleDaylightCycle
            | ClientCommand::ToggleRuleWeatherCycle
            | ClientCommand::ToggleRuleKeepInventory
            | ClientCommand::ToggleSettingsMenu
            | ClientCommand::SettingsMoveUp
            | ClientCommand::SettingsMoveDown
            | ClientCommand::SettingsApply
            | ClientCommand::TravelToOverworld
            | ClientCommand::TravelToNether
            | ClientCommand::TravelToEnd
            | ClientCommand::TravelToSpawn
            | ClientCommand::EquipDiamondLoadout
            | ClientCommand::UseAt(_, _) => {}
        }
        snapshot.player.sneaking = input_state.sneaking();
    }

    fn integrate_predicted_movement(
        snapshot: &mut ServerSnapshot,
        input_state: PredictionInputState,
    ) {
        let controls_blocked =
            snapshot.inventory_open || snapshot.death_screen_active || snapshot.credits_active;
        let input_dir = if controls_blocked || input_state.moving_left == input_state.moving_right {
            0.0
        } else if input_state.moving_left {
            -1.0
        } else {
            1.0
        };

        if input_dir != 0.0 {
            let desired_speed = if snapshot.player.sneaking { 0.22 } else { 0.62 } * input_dir;
            let accel = if snapshot.player.grounded { 0.48 } else { 0.20 };
            snapshot.player.vx += (desired_speed - snapshot.player.vx) * accel;
            snapshot.player.facing_right = input_dir > 0.0;
        }

        let drag = if input_dir != 0.0 {
            if snapshot.player.grounded {
                0.94
            } else {
                0.985
            }
        } else if snapshot.player.grounded {
            0.78
        } else {
            0.985
        };
        snapshot.player.vx *= drag;
        if snapshot.player.vx.abs() < 0.02 {
            snapshot.player.vx = 0.0;
        }
        snapshot.player.x += snapshot.player.vx;
    }
}

#[derive(Debug)]
struct ConnectedClient {
    client_id: u16,
    writer: TcpStream,
    known_chunk_revisions: HashMap<i32, u64>,
}

fn next_client_id(id: &mut u16) -> u16 {
    let current = *id;
    *id = id.wrapping_add(1);
    if *id == 0 {
        *id = 1;
    }
    current
}

fn spawn_client_reader_thread(
    client_id: u16,
    mut reader: TcpStream,
    input_tx: mpsc::Sender<ClientInputFrame>,
) {
    thread::spawn(move || {
        while let Ok(mut input) = read_framed_client_input(&mut reader) {
            input.client_id = client_id;
            if input_tx.send(input).is_err() {
                break;
            }
        }
    });
}

fn accept_pending_clients(
    listener: &TcpListener,
    input_tx: &mpsc::Sender<ClientInputFrame>,
    simulation: &mut ServerSimulation,
    clients: &mut Vec<ConnectedClient>,
    next_id_counter: &mut u16,
) -> io::Result<()> {
    loop {
        match listener.accept() {
            Ok((writer, _)) => {
                writer.set_nodelay(true)?;
                writer.set_write_timeout(Some(Duration::from_millis(8)))?;
                let reader = writer.try_clone()?;
                reader.set_nodelay(true)?;
                let client_id = next_client_id(next_id_counter);
                spawn_client_reader_thread(client_id, reader, input_tx.clone());
                simulation.register_client(client_id);
                clients.push(ConnectedClient {
                    client_id,
                    writer,
                    known_chunk_revisions: HashMap::new(),
                });
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn center_chunk_x(player_x: f64) -> i32 {
    (player_x.floor() as i32).div_euclid(CHUNK_WIDTH as i32)
}

fn build_chunk_delta_packet(
    state: &GameState,
    snapshot_tick: u64,
    center_player_x: f64,
    known_chunk_revisions: &mut HashMap<i32, u64>,
) -> Option<ChunkDeltaPacket> {
    let center = center_chunk_x(center_player_x);
    let mut changed_candidates = Vec::new();

    for chunk_x in (center - CHUNK_STREAM_RADIUS)..=(center + CHUNK_STREAM_RADIUS) {
        let Some((revision, blocks)) = state.world.chunk_column_snapshot(chunk_x) else {
            known_chunk_revisions.remove(&chunk_x);
            continue;
        };
        if known_chunk_revisions.get(&chunk_x).copied() == Some(revision) {
            continue;
        }
        changed_candidates.push(ChunkColumnSnapshot {
            chunk_x,
            revision,
            blocks,
        });
    }

    changed_candidates.sort_by_key(|chunk| (chunk.chunk_x.abs_diff(center), chunk.chunk_x));
    let mut changed_chunks = Vec::new();
    for chunk in changed_candidates
        .into_iter()
        .take(CHUNK_DELTA_MAX_CHUNKS_PER_PACKET)
    {
        known_chunk_revisions.insert(chunk.chunk_x, chunk.revision);
        changed_chunks.push(chunk);
    }

    let retention = CHUNK_STREAM_RADIUS + 4;
    known_chunk_revisions.retain(|chunk_x, _| (*chunk_x - center).abs() <= retention);

    if changed_chunks.is_empty() {
        None
    } else {
        Some(ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: snapshot_tick,
            dimension: state.current_dimension.into(),
            center_chunk_x: center,
            chunks: changed_chunks,
        })
    }
}

fn send_tick_messages_to_client(
    client: &mut ConnectedClient,
    state: &GameState,
    snapshot: &ServerSnapshot,
    snapshot_wire_payload: &[u8],
) -> io::Result<()> {
    write_framed_payload(&mut client.writer, snapshot_wire_payload)?;

    if let Some(delta) = build_chunk_delta_packet(
        state,
        snapshot.tick,
        snapshot.player.x,
        &mut client.known_chunk_revisions,
    ) {
        let delta_msg = ServerWireMessage::ChunkDelta(delta);
        write_framed_server_message(&mut client.writer, &delta_msg)?;
    }
    Ok(())
}

fn build_snapshot_wire_payload(snapshot: &ServerSnapshot) -> io::Result<Vec<u8>> {
    let message = ServerWireMessage::Snapshot(snapshot.clone());
    encode_server_message(&message).map_err(codec_error_to_io)
}

fn consume_due_server_ticks(
    now: Instant,
    last_tick: &mut Instant,
    tick_duration: Duration,
    max_catchup_ticks: u8,
) -> u8 {
    let mut due = 0u8;
    while now.duration_since(*last_tick) >= tick_duration && due < max_catchup_ticks {
        *last_tick += tick_duration;
        due = due.saturating_add(1);
    }
    if due == max_catchup_ticks && now.duration_since(*last_tick) >= tick_duration {
        // Resync clock if the server fell badly behind to avoid catch-up spirals.
        *last_tick = now;
    }
    due
}

pub fn run_headless_tcp_server(bind_addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(bind_addr)?;
    listener.set_nonblocking(true)?;

    let (input_tx, input_rx) = mpsc::channel::<ClientInputFrame>();
    let mut clients = Vec::<ConnectedClient>::new();
    let mut next_id_counter: u16 = 1;
    let mut simulation = ServerSimulation::new();

    let tick_duration = Duration::from_millis(1000 / SERVER_TICK_RATE);
    let mut last_tick = Instant::now();

    loop {
        accept_pending_clients(
            &listener,
            &input_tx,
            &mut simulation,
            &mut clients,
            &mut next_id_counter,
        )?;
        while let Ok(input) = input_rx.try_recv() {
            simulation.enqueue_input(input);
        }

        let now = Instant::now();
        let due_ticks = consume_due_server_ticks(
            now,
            &mut last_tick,
            tick_duration,
            SERVER_MAX_CATCHUP_TICKS_PER_LOOP,
        );

        if due_ticks > 0 {
            for _ in 0..due_ticks {
                simulation.step(0, 0);
                let mut idx = 0;
                while idx < clients.len() {
                    let Some(snapshot) = simulation.snapshot_for_client(clients[idx].client_id)
                    else {
                        idx += 1;
                        continue;
                    };
                    let snapshot_wire_payload = build_snapshot_wire_payload(&snapshot)?;
                    let send_res = send_tick_messages_to_client(
                        &mut clients[idx],
                        simulation.state(),
                        &snapshot,
                        &snapshot_wire_payload,
                    );
                    if send_res.is_err() {
                        let removed = clients.swap_remove(idx);
                        simulation.unregister_client(removed.client_id);
                    } else {
                        idx += 1;
                    }
                }
            }
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Cursor;

    #[test]
    fn client_input_roundtrip_codec() {
        let input = ClientInputFrame::new(7, 19, 42, ClientCommand::SetMoveRight(true));
        let encoded = encode_client_input(&input).expect("encoding should succeed");
        let decoded = decode_client_input(&encoded).expect("decoding should succeed");
        assert_eq!(decoded, input);
    }

    #[test]
    fn server_snapshot_roundtrip_codec() {
        let state = GameState::new();
        let snapshot = ServerSnapshot::from_state(3, &state);
        let encoded = encode_server_snapshot(&snapshot).expect("encoding should succeed");
        let decoded = decode_server_snapshot(&encoded).expect("decoding should succeed");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn server_wire_message_roundtrip_codec() {
        let message = ServerWireMessage::Snapshot(ServerSnapshot::from_state(5, &GameState::new()));
        let encoded = encode_server_message(&message).expect("encoding should succeed");
        let decoded = decode_server_message(&encoded).expect("decoding should succeed");
        assert_eq!(decoded, message);
    }

    #[test]
    fn snapshot_wire_payload_decodes_as_server_wire_message_snapshot() {
        let snapshot = ServerSnapshot::from_state(11, &GameState::new());
        let payload = build_snapshot_wire_payload(&snapshot).expect("payload should encode");
        let decoded = decode_server_message(&payload).expect("payload should decode");
        match decoded {
            ServerWireMessage::Snapshot(decoded_snapshot) => {
                assert_eq!(decoded_snapshot, snapshot);
            }
            ServerWireMessage::ChunkDelta(_) => {
                panic!("snapshot payload must decode as snapshot wire message");
            }
        }
    }

    #[test]
    fn server_sim_applies_inputs_for_target_tick() {
        let mut sim = ServerSimulation::new();
        assert!(!sim.state().inventory_open);

        sim.enqueue_input(ClientInputFrame::new(
            1,
            1,
            1,
            ClientCommand::ToggleInventory,
        ));
        let snapshot = sim.step(0, 0);

        assert_eq!(snapshot.tick, 1);
        assert!(snapshot.inventory_open);
        assert!(sim.state().inventory_open);
    }

    #[test]
    fn server_sim_ignores_duplicate_sequences_per_client() {
        let mut sim = ServerSimulation::new();

        sim.enqueue_input(ClientInputFrame::new(
            2,
            4,
            1,
            ClientCommand::ToggleInventory,
        ));
        sim.enqueue_input(ClientInputFrame::new(2, 4, 1, ClientCommand::ToggleSneak));
        sim.step(0, 0);

        assert!(sim.state().inventory_open);
        assert!(!sim.state().player.sneaking);
    }

    #[test]
    fn server_sim_accepts_wrapped_sequences_as_newer() {
        let mut sim = ServerSimulation::new();
        sim.last_applied_sequence_by_client.insert(11, u32::MAX - 1);
        sim.last_seen_input_tick_by_client.insert(11, 0);

        sim.enqueue_input(ClientInputFrame::new(
            11,
            u32::MAX,
            1,
            ClientCommand::ToggleInventory,
        ));
        sim.enqueue_input(ClientInputFrame::new(11, 1, 1, ClientCommand::ToggleSneak));
        sim.step(0, 0);

        assert!(sim.state().inventory_open);
        assert!(sim.state().player.sneaking);
        assert_eq!(sim.last_applied_sequence_by_client.get(&11), Some(&1));
    }

    #[test]
    fn server_sim_rejects_older_wrapped_sequence() {
        let mut sim = ServerSimulation::new();
        sim.last_applied_sequence_by_client.insert(12, 2);
        sim.last_seen_input_tick_by_client.insert(12, 0);

        sim.enqueue_input(ClientInputFrame::new(
            12,
            u32::MAX,
            1,
            ClientCommand::ToggleInventory,
        ));
        sim.step(0, 0);

        assert!(!sim.state().inventory_open);
        assert_eq!(sim.last_applied_sequence_by_client.get(&12), Some(&2));
    }

    #[test]
    fn server_sim_prunes_stale_client_sequence_state() {
        let mut sim = ServerSimulation::new();
        let prune_tick = CLIENT_SEQUENCE_PRUNE_INTERVAL_TICKS * 20;
        sim.tick = prune_tick;
        sim.last_applied_sequence_by_client.insert(20, 99);
        sim.last_seen_input_tick_by_client
            .insert(20, prune_tick - CLIENT_SEQUENCE_STALE_TICKS - 1);
        sim.last_applied_sequence_by_client.insert(21, 44);
        sim.last_seen_input_tick_by_client.insert(21, prune_tick);

        sim.prune_inactive_client_sequences();

        assert!(!sim.last_applied_sequence_by_client.contains_key(&20));
        assert!(!sim.last_seen_input_tick_by_client.contains_key(&20));
        assert_eq!(sim.last_applied_sequence_by_client.get(&21), Some(&44));
        assert_eq!(
            sim.last_seen_input_tick_by_client.get(&21),
            Some(&prune_tick)
        );
    }

    #[test]
    fn server_sim_caps_inputs_per_scheduled_tick_bucket() {
        let mut sim = ServerSimulation::new();
        for seq in 1..=((MAX_PENDING_INPUTS_PER_SCHEDULED_TICK as u32) + 24) {
            sim.enqueue_input(ClientInputFrame::new(
                91,
                seq,
                1,
                ClientCommand::SetMoveRight(seq % 2 == 0),
            ));
        }
        assert_eq!(
            sim.pending_inputs.get(&1).map(|frames| frames.len()),
            Some(MAX_PENDING_INPUTS_PER_SCHEDULED_TICK)
        );
    }

    #[test]
    fn late_inputs_roll_forward_to_next_tick() {
        let mut sim = ServerSimulation::new();
        sim.step(0, 0);
        assert_eq!(sim.tick(), 1);

        sim.enqueue_input(ClientInputFrame::new(
            3,
            1,
            1,
            ClientCommand::ToggleInventory,
        ));
        assert!(!sim.state().inventory_open);

        sim.step(0, 0);
        assert_eq!(sim.tick(), 2);
        assert!(sim.state().inventory_open);
    }

    #[test]
    fn secondary_client_receives_own_snapshot_and_primary_as_remote_player() {
        let mut sim = ServerSimulation::new();
        sim.register_client(1);
        sim.register_client(2);

        let snapshot = sim
            .snapshot_for_client(2)
            .expect("secondary client snapshot should exist");
        assert_eq!(snapshot.remote_players.len(), 1);
        assert_eq!(snapshot.remote_players[0].client_id, 1);
        assert_eq!(
            snapshot.remote_players[0].player,
            PlayerSnapshot::from_state(sim.state())
        );
    }

    #[test]
    fn secondary_client_moves_independently_of_primary_player() {
        let mut sim = ServerSimulation::new();
        sim.register_client(1);
        sim.register_client(2);

        let start_secondary_x = sim
            .snapshot_for_client(2)
            .expect("secondary client snapshot should exist")
            .player
            .x;
        let start_primary_x = sim.state().player.x;

        sim.enqueue_input(ClientInputFrame::new(
            2,
            1,
            1,
            ClientCommand::SetMoveRight(true),
        ));
        sim.step(0, 0);

        let end_secondary_x = sim
            .snapshot_for_client(2)
            .expect("secondary client snapshot should exist")
            .player
            .x;
        assert!(end_secondary_x > start_secondary_x);
        assert_eq!(sim.state().player.x, start_primary_x);
    }

    #[test]
    fn enqueue_input_clamps_far_future_ticks_to_lead_window() {
        let mut sim = ServerSimulation::new();
        sim.step(0, 0);
        assert_eq!(sim.tick(), 1);

        let far_future_tick = sim.tick() + MAX_CLIENT_INPUT_LEAD_TICKS + 500;
        sim.enqueue_input(ClientInputFrame::new(
            7,
            1,
            far_future_tick,
            ClientCommand::ToggleInventory,
        ));

        let clamped_tick = sim.tick() + MAX_CLIENT_INPUT_LEAD_TICKS;
        assert!(sim.pending_inputs.contains_key(&clamped_tick));
        assert!(!sim.pending_inputs.contains_key(&far_future_tick));
    }

    #[test]
    fn decoder_rejects_unknown_protocol_version() {
        let mut input = ClientInputFrame::new(9, 1, 1, ClientCommand::QueueJump);
        input.protocol = NET_PROTOCOL_VERSION + 1;
        let encoded = bincode::serialize(&input).expect("encoding should succeed");
        let err = decode_client_input(&encoded).expect_err("decode should fail");
        match err {
            NetCodecError::UnsupportedProtocol { found, expected } => {
                assert_eq!(found, NET_PROTOCOL_VERSION + 1);
                assert_eq!(expected, NET_PROTOCOL_VERSION);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn framed_client_input_roundtrip() {
        let input = ClientInputFrame::new(5, 9, 12, ClientCommand::SetMoveLeft(true));
        let mut wire = Vec::new();
        write_framed_client_input(&mut wire, &input).expect("framed write should succeed");

        let mut cursor = Cursor::new(wire);
        let decoded = read_framed_client_input(&mut cursor).expect("framed read should succeed");
        assert_eq!(decoded, input);
    }

    #[test]
    fn framed_snapshot_roundtrip() {
        let snapshot = ServerSnapshot::from_state(4, &GameState::new());
        let mut wire = Vec::new();
        write_framed_server_snapshot(&mut wire, &snapshot).expect("framed write should succeed");

        let mut cursor = Cursor::new(wire);
        let decoded = read_framed_server_snapshot(&mut cursor).expect("framed read should succeed");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn framed_server_message_roundtrip() {
        let message = ServerWireMessage::Snapshot(ServerSnapshot::from_state(7, &GameState::new()));
        let mut wire = Vec::new();
        write_framed_server_message(&mut wire, &message).expect("framed write should succeed");

        let mut cursor = Cursor::new(wire);
        let decoded = read_framed_server_message(&mut cursor).expect("framed read should succeed");
        assert_eq!(decoded, message);
    }

    #[test]
    fn invalid_frame_length_is_rejected() {
        let mut wire = Vec::new();
        wire.extend_from_slice(&((MAX_NET_FRAME_BYTES as u32) + 1).to_be_bytes());
        let mut cursor = Cursor::new(wire);
        let err = read_framed_payload(&mut cursor).expect_err("oversized frame must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn chunk_delta_only_streams_new_or_changed_chunks() {
        let mut state = GameState::new();
        let center_world_x = state.player.x.floor() as i32;
        state.world.load_chunks_around(center_world_x);
        let mut known = HashMap::<i32, u64>::new();
        let mut tick = 1;
        let first = build_chunk_delta_packet(&state, tick, state.player.x, &mut known)
            .expect("first packet should exist");
        assert!(!first.chunks.is_empty());
        tick += 1;

        while build_chunk_delta_packet(&state, tick, state.player.x, &mut known).is_some() {
            tick += 1;
            assert!(tick < 40, "chunk bootstrap should converge quickly");
        }

        let second = build_chunk_delta_packet(&state, tick, state.player.x, &mut known);
        assert!(second.is_none());

        let center_chunk = center_chunk_x(state.player.x);
        let target_x = center_chunk * CHUNK_WIDTH as i32 + 1;
        let existing = state.world.get_block(target_x, 10);
        let replacement = if existing == BlockType::Stone {
            BlockType::Dirt
        } else {
            BlockType::Stone
        };
        state.world.set_block(target_x, 10, replacement);
        let third = build_chunk_delta_packet(&state, tick + 1, state.player.x, &mut known)
            .expect("changed chunk should exist");
        assert!(
            third
                .chunks
                .iter()
                .any(|chunk| chunk.chunk_x == center_chunk)
        );
    }

    #[test]
    fn chunk_delta_builder_limits_chunk_count_and_prioritizes_center() {
        let mut state = GameState::new();
        let center = center_chunk_x(state.player.x);
        for chunk_x in (center - CHUNK_STREAM_RADIUS)..=(center + CHUNK_STREAM_RADIUS) {
            state.world.load_chunks_around(chunk_x * CHUNK_WIDTH as i32);
        }
        let mut known = HashMap::<i32, u64>::new();

        let first = build_chunk_delta_packet(&state, 1, state.player.x, &mut known)
            .expect("first packet should exist");
        assert!(!first.chunks.is_empty());
        assert!(first.chunks.len() <= CHUNK_DELTA_MAX_CHUNKS_PER_PACKET);
        assert_eq!(first.chunks[0].chunk_x, center);
        assert!(first.chunks.iter().any(|chunk| chunk.chunk_x == center));
    }

    #[test]
    fn chunk_delta_builder_retries_unsent_chunks_on_followup_ticks() {
        let mut state = GameState::new();
        let center = center_chunk_x(state.player.x);
        for chunk_x in (center - CHUNK_STREAM_RADIUS)..=(center + CHUNK_STREAM_RADIUS) {
            state.world.load_chunks_around(chunk_x * CHUNK_WIDTH as i32);
        }
        let expected_stream_width = (CHUNK_STREAM_RADIUS * 2 + 1) as usize;
        assert!(expected_stream_width > CHUNK_DELTA_MAX_CHUNKS_PER_PACKET);

        let mut known = HashMap::<i32, u64>::new();
        let mut streamed_chunks = HashSet::<i32>::new();
        for tick in 1..=16 {
            if let Some(delta) = build_chunk_delta_packet(&state, tick, state.player.x, &mut known)
            {
                for chunk in delta.chunks {
                    streamed_chunks.insert(chunk.chunk_x);
                }
            } else {
                break;
            }
        }
        assert_eq!(streamed_chunks.len(), expected_stream_width);
    }

    #[test]
    fn chunk_delta_skips_noop_block_write() {
        let mut state = GameState::new();
        let center_world_x = state.player.x.floor() as i32;
        state.world.load_chunks_around(center_world_x);
        let mut known = HashMap::<i32, u64>::new();
        let mut tick = 1;
        while build_chunk_delta_packet(&state, tick, state.player.x, &mut known).is_some() {
            tick += 1;
            assert!(tick < 40, "chunk bootstrap should converge quickly");
        }
        let center_chunk = center_chunk_x(state.player.x);
        let target_x = center_chunk * CHUNK_WIDTH as i32 + 1;
        let existing = state.world.get_block(target_x, 10);
        state.world.set_block(target_x, 10, existing);

        let delta = build_chunk_delta_packet(&state, tick + 1, state.player.x, &mut known);
        assert!(delta.is_none());
    }

    #[test]
    fn client_prediction_queues_outbound_frames() {
        let mut client = ClientPredictionState::new(23);
        let first = client.queue_local_command(ClientCommand::SetMoveRight(true));
        assert_eq!(first.client_id, 23);
        assert_eq!(first.sequence, 1);
        assert_eq!(first.tick, 1);
        assert_eq!(client.pending_input_count(), 1);

        let outbound = client.take_outbound_inputs();
        assert_eq!(outbound, vec![first]);
        assert!(client.take_outbound_inputs().is_empty());
    }

    #[test]
    fn client_prediction_reconciles_against_authoritative_snapshot() {
        let mut client = ClientPredictionState::new(1);
        let baseline = ServerSnapshot::from_state(0, &GameState::new());
        client.apply_server_message(ServerWireMessage::Snapshot(baseline.clone()));

        let move_input = client.queue_local_command(ClientCommand::SetMoveRight(true));
        client.advance_local_tick();
        let predicted_before = client
            .predicted_snapshot()
            .expect("prediction should exist after first snapshot")
            .player
            .x;
        assert!(predicted_before > baseline.player.x);

        let mut server = ServerSimulation::new();
        server.enqueue_input(move_input);
        let authoritative_tick_1 = server.step(0, 0);
        client.apply_server_message(ServerWireMessage::Snapshot(authoritative_tick_1.clone()));

        let predicted_after = client
            .predicted_snapshot()
            .expect("prediction should exist")
            .player
            .x;
        assert!((predicted_after - authoritative_tick_1.player.x).abs() < 1e-6);
        assert_eq!(client.pending_input_count(), 0);
        assert_eq!(
            client
                .authoritative_snapshot()
                .expect("authoritative snapshot should exist")
                .tick,
            1
        );
    }

    #[test]
    fn older_authoritative_snapshots_are_ignored() {
        let mut client = ClientPredictionState::new(4);
        let mut s5 = ServerSnapshot::from_state(5, &GameState::new());
        s5.tick = 5;
        client.apply_server_message(ServerWireMessage::Snapshot(s5));

        let mut s4 = ServerSnapshot::from_state(4, &GameState::new());
        s4.tick = 4;
        client.apply_server_message(ServerWireMessage::Snapshot(s4));

        assert_eq!(
            client
                .authoritative_snapshot()
                .expect("authoritative snapshot should exist")
                .tick,
            5
        );
    }

    #[test]
    fn client_prediction_applies_chunk_deltas() {
        let mut client = ClientPredictionState::new(9);
        client.apply_server_message(ServerWireMessage::Snapshot(ServerSnapshot::from_state(
            2,
            &GameState::new(),
        )));
        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 2,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 1234,
                blocks: vec![BlockType::Air; 16],
            }],
        };
        client.apply_server_message(ServerWireMessage::ChunkDelta(delta));

        assert_eq!(client.replicated_chunk_count(), 1);
        assert_eq!(
            client
                .replicated_chunk(0)
                .expect("chunk should be present")
                .revision,
            1234
        );
    }

    #[test]
    fn client_prediction_ignores_chunk_delta_for_other_dimension() {
        let mut client = ClientPredictionState::new(10);
        client.apply_server_message(ServerWireMessage::Snapshot(ServerSnapshot::from_state(
            2,
            &GameState::new(),
        )));
        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 2,
            dimension: SnapshotDimension::Nether,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 777,
                blocks: vec![BlockType::Air; 16],
            }],
        };
        client.apply_server_message(ServerWireMessage::ChunkDelta(delta));

        assert_eq!(client.replicated_chunk_count(), 0);
        assert!(client.replicated_chunk(0).is_none());
    }

    #[test]
    fn client_prediction_clears_replicated_chunks_on_dimension_change_snapshot() {
        let mut client = ClientPredictionState::new(11);
        client.apply_server_message(ServerWireMessage::Snapshot(ServerSnapshot::from_state(
            2,
            &GameState::new(),
        )));
        let overworld_delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 2,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 888,
                blocks: vec![BlockType::Air; 16],
            }],
        };
        client.apply_server_message(ServerWireMessage::ChunkDelta(overworld_delta));
        assert_eq!(client.replicated_chunk_count(), 1);

        let mut nether_state = GameState::new();
        nether_state.current_dimension = Dimension::Nether;
        let nether_snapshot = ServerSnapshot::from_state(3, &nether_state);
        client.apply_server_message(ServerWireMessage::Snapshot(nether_snapshot));

        assert_eq!(client.replicated_chunk_count(), 0);
    }

    #[test]
    fn chunk_delta_builder_prunes_far_known_revisions() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.player.x = 0.0;

        let center = center_chunk_x(state.player.x);
        let retention = CHUNK_STREAM_RADIUS + 4;
        let far_chunk = center + retention + 2;
        let mut known = HashMap::<i32, u64>::new();
        known.insert(center, 1);
        known.insert(far_chunk, 2);

        let _ = build_chunk_delta_packet(&state, 9, state.player.x, &mut known);
        assert!(known.contains_key(&center));
        assert!(!known.contains_key(&far_chunk));
    }

    #[test]
    fn consume_due_server_ticks_advances_without_dropping_when_caught_up() {
        let now = Instant::now();
        let tick_duration = Duration::from_millis(50);
        let mut last_tick = now - Duration::from_millis(150);

        let due = consume_due_server_ticks(now, &mut last_tick, tick_duration, 8);

        assert_eq!(due, 3);
        assert_eq!(last_tick, now);
    }

    #[test]
    fn consume_due_server_ticks_caps_and_resyncs_when_badly_behind() {
        let now = Instant::now();
        let tick_duration = Duration::from_millis(50);
        let mut last_tick = now - Duration::from_millis(1200);

        let due = consume_due_server_ticks(now, &mut last_tick, tick_duration, 5);

        assert_eq!(due, 5);
        assert_eq!(last_tick, now);
    }
}
