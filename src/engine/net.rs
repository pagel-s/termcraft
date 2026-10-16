use super::command::ClientCommand;
use super::replication::RuntimeEntities;
use super::server_save::{ServerSave, token, valid_token};
use super::state::{GameState, PlayerSession, WeatherType};
use crate::entities::player::Player;
use crate::world::Dimension;
use crate::world::block::BlockType;
use crate::world::chunk::CHUNK_WIDTH;
use crate::world::item::ItemType;
use bincode::Options;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub const NET_PROTOCOL_VERSION: u16 = 14;
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
    pub dimension: SnapshotDimension,
    pub command: ClientCommand,
}

impl ClientInputFrame {
    pub fn new(client_id: u16, sequence: u32, tick: u64, command: ClientCommand) -> Self {
        Self {
            protocol: NET_PROTOCOL_VERSION,
            client_id,
            sequence,
            tick,
            dimension: SnapshotDimension::Overworld,
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
    pub action: Option<super::action::ActionVisual>,
    pub held_item: Option<ItemType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerSnapshot {
    pub protocol: u16,
    pub acknowledged_sequence: u32,
    pub tick: u64,
    pub dimension: SnapshotDimension,
    pub weather: SnapshotWeather,
    pub time_of_day: f32,
    pub player: PlayerSnapshot,
    pub action: Option<super::action::ActionVisual>,
    pub chat: super::director::ChatView,
    pub settings: super::state::SharedSettings,
    pub session: PlayerSession,
    pub chest: Option<(i32, i32, crate::world::item::Inventory)>,
    pub entities: RuntimeEntities,
    pub trial: Option<super::state::Trial>,
    pub thunder_flash: u8,
    pub rain_intensity: f32,
    pub wind_intensity: f32,
    pub thunder_intensity: f32,
    pub remote_players: Vec<PeerPlayerSnapshot>,
    pub hotbar_index: u8,
    pub selected_hotbar_item: Option<ItemType>,
    pub inventory_open: bool,
    pub death_screen_active: bool,
    pub credits_active: bool,
}

impl ServerSnapshot {
    pub fn from_state(tick: u64, state: &GameState) -> Self {
        let session = PlayerSession::capture(state);
        let chest = session.chest(state);
        Self {
            protocol: NET_PROTOCOL_VERSION,
            acknowledged_sequence: 0,
            tick,
            dimension: state.current_dimension.into(),
            weather: state.weather.into(),
            time_of_day: state.time_of_day,
            player: PlayerSnapshot::from_state(state),
            action: state.action_visual(0.0),
            chat: state.chat.clone(),
            settings: super::state::SharedSettings::capture(state),
            session,
            chest,
            entities: RuntimeEntities::capture(state, state.player.x),
            trial: state.trial.clone(),
            thunder_flash: state.thunder_flash_timer,
            rain_intensity: state.weather_rain_intensity,
            wind_intensity: state.weather_wind_intensity,
            thunder_intensity: state.weather_thunder_intensity,
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
    pub biomes: Vec<crate::world::BiomeType>,
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
pub struct ClientHello {
    pub protocol: u16,
    pub token: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Welcome {
    pub protocol: u16,
    pub client_id: u16,
    pub server_id: String,
    pub token: String,
}
impl ProtocolTagged for ClientHello {
    fn protocol_version(&self) -> u16 {
        self.protocol
    }
}
pub fn write_client_hello(writer: &mut impl Write, hello: &ClientHello) -> io::Result<()> {
    write_framed_payload(
        writer,
        &encode_protocol_message(hello).map_err(codec_error_to_io)?,
    )
}
fn read_client_hello(reader: &mut impl Read) -> io::Result<ClientHello> {
    decode_protocol_message(&read_framed_payload(reader)?).map_err(codec_error_to_io)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ServerWireMessage {
    Welcome(Welcome),
    Rejected(u16, String),
    Snapshot(Box<ServerSnapshot>),
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
            ServerWireMessage::Welcome(welcome) => welcome.protocol,
            ServerWireMessage::Rejected(protocol, _) => *protocol,
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

fn codec_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_NET_FRAME_BYTES as u64)
        .reject_trailing_bytes()
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
    let message: T = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_NET_FRAME_BYTES as u64)
        .reject_trailing_bytes()
        .deserialize(encoded)?;
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
    if payload.len() > MAX_NET_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "network frame too large",
        ));
    }
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

#[derive(Serialize, Deserialize)]
enum ClientWireMessage {
    Input(ClientInputFrame),
    Chat(String),
}
pub fn write_framed_client_input<W: Write>(
    writer: &mut W,
    frame: &ClientInputFrame,
) -> io::Result<()> {
    validate_protocol(frame.protocol).map_err(codec_error_to_io)?;
    write_framed_payload(
        writer,
        &codec_options()
            .serialize(&ClientWireMessage::Input(*frame))
            .map_err(io::Error::other)?,
    )
}
pub fn write_framed_client_chat(writer: &mut impl Write, text: &str) -> io::Result<()> {
    if text.len() > 2000 || text.chars().any(char::is_control) {
        return Err(io::Error::other(
            "chat needs at most 2000 bytes without controls",
        ));
    }
    write_framed_payload(
        writer,
        &codec_options()
            .serialize(&ClientWireMessage::Chat(text.into()))
            .map_err(io::Error::other)?,
    )
}
fn read_framed_client_message(reader: &mut impl Read) -> io::Result<ClientWireMessage> {
    codec_options()
        .deserialize(&read_framed_payload(reader)?)
        .map_err(io::Error::other)
}
pub fn read_framed_client_input<R: Read>(reader: &mut R) -> io::Result<ClientInputFrame> {
    match read_framed_client_message(reader)? {
        ClientWireMessage::Input(frame) if frame.protocol == NET_PROTOCOL_VERSION => Ok(frame),
        _ => Err(io::Error::other("expected compatible input frame")),
    }
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
    director: super::director::Director,
    state: GameState,
    tick: u64,
    primary_client_id: Option<u16>,
    targets: HashMap<u16, (i32, i32)>,
    saved: ServerSave,
    tokens: HashMap<u16, String>,
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
        Self::with_state(GameState::new())
    }

    pub fn with_state(mut state: GameState) -> Self {
        let director = super::director::Director::new(&mut state);
        let saved = ServerSave::fresh(&state);
        Self {
            director,
            saved,
            tokens: HashMap::new(),
            state,
            tick: 0,
            primary_client_id: None,
            targets: HashMap::new(),
            pending_inputs: BTreeMap::new(),
            last_applied_sequence_by_client: HashMap::new(),
            last_seen_input_tick_by_client: HashMap::new(),
        }
    }

    pub fn load_persistent(&mut self) -> io::Result<()> {
        let saved = ServerSave::read(&self.state)?;
        saved
            .settings
            .apply(&mut self.state)
            .map_err(io::Error::other)?;
        self.state
            .restore_dimension_settings(&saved.worlds)
            .map_err(io::Error::other)?;
        saved
            .progression
            .apply(&mut self.state)
            .map_err(io::Error::other)?;
        self.saved = saved;
        self.save()
    }
    pub fn authenticate(&mut self, id: u16, resume: Option<&str>) -> io::Result<Welcome> {
        if self.state.peer_ids().len() >= 8 {
            return Err(io::Error::other("server is full (8 players)"));
        }
        let identity = match resume {
            Some(t) if !valid_token(t) || !self.saved.players.contains_key(t) => {
                return Err(io::Error::other(
                    "unrecognized player credential; select a new profile to join fresh",
                ));
            }
            Some(t) if self.tokens.values().any(|s| s == t) => {
                return Err(io::Error::other(
                    "this player is already connected; use another profile",
                ));
            }
            Some(t) => t.to_string(),
            None if self.saved.players.len() >= 64 => {
                return Err(io::Error::other(
                    "server has reached its saved-player limit",
                ));
            }
            None => token(),
        };
        self.register_client(id);
        if let Some(session) = self.saved.players.get(&identity) {
            self.state.restore_peer_dimension(
                id,
                self.saved
                    .dimensions
                    .get(&identity)
                    .copied()
                    .unwrap_or(Dimension::Overworld),
            );
            self.state.restore_peer(id, session.clone());
        }
        self.tokens.insert(id, identity.clone());
        if let Err(e) = self.save() {
            self.unregister_client(id);
            return Err(e);
        }
        Ok(Welcome {
            protocol: NET_PROTOCOL_VERSION,
            client_id: id,
            server_id: self.saved.id.clone(),
            token: identity,
        })
    }
    pub fn save(&mut self) -> io::Result<()> {
        for (&id, token) in &self.tokens {
            if let Some(session) = self.state.peer_session(id) {
                self.saved.players.insert(token.clone(), session);
                self.saved
                    .dimensions
                    .insert(token.clone(), self.state.peer_dimension(id));
            }
        }
        self.saved.worlds = self.state.save_dimension_worlds()?;
        self.saved.progression = super::state::SharedProgress::capture(&self.state);
        self.state.save_trial()?;
        self.director.save(&self.state).map_err(io::Error::other)?;
        self.saved.settings = self
            .saved
            .worlds
            .get(&Dimension::Overworld)
            .cloned()
            .unwrap_or_else(|| super::state::SharedSettings::capture(&self.state));
        self.saved.write(self.state.world.save_root())
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }
    /// An isolated replica composed through the native client's actual handlers.
    /// Useful for renderer review; this does not measure transport/presentation latency.
    pub fn client_view(&mut self, id: u16) -> Option<GameState> {
        let snapshot = self.snapshot_for_client(id)?;
        let mut view = GameState::network_view();
        super::network_client::sync_state_from_snapshot(&mut view, &snapshot);
        let world = self.state.world_in_dimension(snapshot.dimension.into())?;
        let mut revisions = HashMap::new();
        while let Some(delta) =
            build_world_chunk_delta_packet(world, snapshot.tick, snapshot.player.x, &mut revisions)
        {
            super::network_client::apply_chunk_delta_to_state(&mut view, &delta);
        }
        Some(view)
    }
    pub fn maintain_chunk_cache(&mut self) -> io::Result<()> {
        self.state.maintain_dimension_caches()
    }

    pub fn register_client(&mut self, client_id: u16) {
        if self.state.peer_ids().len() >= 8 || self.state.peer_ids().contains(&client_id) {
            return;
        }
        self.state.join_peer(client_id);
        if self.primary_client_id.is_none() {
            self.primary_client_id = Some(client_id);
        }
    }
    pub fn unregister_client(&mut self, client_id: u16) {
        // Drop queued work before a client ID can be reused; disconnected inputs
        // must never re-register a player or continue an attack.
        for inputs in self.pending_inputs.values_mut() {
            inputs.retain(|f| f.client_id != client_id);
        }
        if let Some(session) = self.state.disconnect_peer_session(client_id)
            && let Some(token) = self.tokens.remove(&client_id)
        {
            self.saved
                .dimensions
                .insert(token.clone(), self.state.peer_dimension(client_id));
            self.saved.players.insert(token, session);
        }
        self.state.leave_peer(client_id);
        self.targets.remove(&client_id);
        self.last_applied_sequence_by_client.remove(&client_id);
        self.last_seen_input_tick_by_client.remove(&client_id);
        if self.primary_client_id == Some(client_id) {
            self.director.host_left(&mut self.state);
            self.primary_client_id = self.state.peer_ids().first().copied();
        }
        if let Some(primary) = self.primary_client_id {
            self.state.activate_peer(primary);
        }
    }
    pub fn snapshot_for_client(&mut self, client_id: u16) -> Option<ServerSnapshot> {
        let selected = self.state.selected_peer_id();
        if !self.state.activate_peer(client_id) {
            return None;
        }
        let mut snapshot = ServerSnapshot::from_state(self.tick, &self.state);
        if self.primary_client_id != Some(client_id) {
            snapshot.chat.mode = "host-only".into();
        }
        snapshot.acknowledged_sequence = self
            .last_applied_sequence_by_client
            .get(&client_id)
            .copied()
            .unwrap_or(0);
        snapshot.remote_players = self
            .state
            .peer_ids()
            .into_iter()
            .filter(|id| *id != client_id)
            .filter(|&id| self.state.peer_dimension(id) == self.state.current_dimension)
            .filter_map(|id| {
                self.state.peer_session(id).map(|p| PeerPlayerSnapshot {
                    client_id: id,
                    player: PlayerSnapshot::from_player(p.player()),
                    action: self.state.peer_action_visual(id, p.player().age, 0.0),
                    held_item: p.inventory().slots[p.hotbar_index() as usize]
                        .as_ref()
                        .map(|s| s.item_type),
                })
            })
            .collect();
        if let Some(id) = selected {
            self.state.activate_peer(id);
        }
        Some(snapshot)
    }

    pub fn enqueue_input(&mut self, frame: ClientInputFrame) {
        if self.primary_client_id != Some(frame.client_id)
            && !self.state.peer_ids().contains(&frame.client_id)
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
        for id in self.state.peer_ids() {
            if self
                .last_seen_input_tick_by_client
                .get(&id)
                .is_some_and(|last| self.tick.saturating_sub(*last) > SERVER_TICK_RATE)
            {
                self.state.activate_peer(id);
                for cmd in [
                    ClientCommand::ClearDirectionalInput,
                    ClientCommand::SetPrimaryAction(false),
                    ClientCommand::SetJumpHeld(false),
                    ClientCommand::SetSneakHeld(false),
                ] {
                    self.state.apply_client_command(cmd);
                }
            }
        }
        let ids = self.state.peer_ids();
        for id in &ids {
            self.state.activate_peer(*id);
            let (x, y) = self
                .targets
                .get(id)
                .copied()
                .unwrap_or((mouse_target_x, mouse_target_y));
            self.state.tick_cooperative_player(x, y);
        }
        let mut occupied = BTreeMap::new();
        for &id in &ids {
            occupied.entry(self.state.peer_dimension(id)).or_insert(id);
        }
        for id in occupied.values() {
            self.state.activate_peer(*id);
            self.state.tick_cooperative_boats();
            self.state.update(mouse_target_x, mouse_target_y);
        }
        for id in ids {
            self.state.activate_peer(id);
            self.state.finish_cooperative_damage();
        }
        if let Some(primary) = self.primary_client_id {
            self.state.activate_peer(primary);
        }
        self.director.poll(&mut self.state, Instant::now());
        self.snapshot()
    }

    fn submit_chat(&mut self, id: u16, text: String) {
        if self.primary_client_id != Some(id) {
            return;
        }
        if text.len() > 2000 || text.chars().any(char::is_control) {
            return;
        }
        self.state.activate_peer(id);
        self.director.submit(&mut self.state, text);
    }

    pub fn snapshot(&mut self) -> ServerSnapshot {
        if let Some(primary) = self.primary_client_id {
            self.state.activate_peer(primary);
        }
        let mut snapshot = ServerSnapshot::from_state(self.tick, &self.state);
        snapshot.acknowledged_sequence = self
            .primary_client_id
            .and_then(|id| self.last_applied_sequence_by_client.get(&id))
            .copied()
            .unwrap_or(0);
        snapshot
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
            if self.state.activate_peer(frame.client_id) {
                let release = matches!(
                    frame.command,
                    ClientCommand::SetMoveLeft(false)
                        | ClientCommand::SetMoveRight(false)
                        | ClientCommand::SetJumpHeld(false)
                        | ClientCommand::SetSneakHeld(false)
                        | ClientCommand::SetPrimaryAction(false)
                        | ClientCommand::ClearDirectionalInput
                );
                if frame.dimension != self.state.current_dimension.into() && !release {
                    self.last_applied_sequence_by_client
                        .insert(frame.client_id, frame.sequence);
                    continue;
                }
                let before = self.state.current_dimension;
                match frame.command {
                    ClientCommand::AimAt(x, y) => {
                        if x.abs_diff(0) <= 100_000 && (0..128).contains(&y) {
                            self.targets.insert(frame.client_id, (x, y));
                        }
                    }
                    ClientCommand::TravelToOverworld
                    | ClientCommand::TravelToNether
                    | ClientCommand::TravelToEnd
                    | ClientCommand::TravelToSpawn
                    | ClientCommand::EquipDiamondLoadout
                    | ClientCommand::CycleDifficulty
                    | ClientCommand::CycleGameRulesPreset
                    | ClientCommand::ToggleRuleMobSpawning
                    | ClientCommand::ToggleRuleDaylightCycle
                    | ClientCommand::ToggleRuleWeatherCycle
                    | ClientCommand::ToggleRuleKeepInventory
                    | ClientCommand::ToggleSettingsMenu
                    | ClientCommand::SettingsMoveUp
                    | ClientCommand::SettingsMoveDown
                    | ClientCommand::SettingsApply => {}
                    command => self.state.apply_client_command(command),
                }
                if self.state.current_dimension != before {
                    for command in [
                        ClientCommand::ClearDirectionalInput,
                        ClientCommand::SetPrimaryAction(false),
                        ClientCommand::SetJumpHeld(false),
                        ClientCommand::SetSneakHeld(false),
                    ] {
                        self.state.apply_client_command(command);
                    }
                    self.targets.insert(
                        frame.client_id,
                        (
                            self.state.player.x.floor() as i32,
                            (self.state.player.y - 1.0).floor() as i32,
                        ),
                    );
                }
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

pub struct ClientPredictionState {
    client_id: u16,
    local_tick: u64,
    next_sequence: u32,
    authoritative_snapshot: Option<ServerSnapshot>,
    predicted_snapshot: Option<ServerSnapshot>,
    pending_inputs: Vec<ClientInputFrame>,
    outbound_inputs: Vec<ClientInputFrame>,
    physics: GameState,
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
            physics: GameState::network_view(),
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
        let mut frame = ClientInputFrame::new(
            self.client_id,
            self.next_sequence,
            self.local_tick.saturating_add(1),
            command,
        );
        frame.dimension = self
            .authoritative_snapshot
            .as_ref()
            .map_or(SnapshotDimension::Overworld, |s| s.dimension);
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        self.pending_inputs.push(frame);
        self.outbound_inputs.push(frame);
        self.rebuild_prediction_from_authoritative();
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
            ServerWireMessage::Welcome(welcome) => {
                self.client_id = welcome.client_id;
            }
            ServerWireMessage::Rejected(_, _) => {}
            ServerWireMessage::Snapshot(snapshot) => self.apply_authoritative_snapshot(*snapshot),
            ServerWireMessage::ChunkDelta(delta) => self.apply_chunk_delta(delta),
        }
    }

    fn apply_authoritative_snapshot(&mut self, snapshot: ServerSnapshot) {
        if let Some(current) = &self.authoritative_snapshot
            && snapshot.tick < current.tick
        {
            return;
        }
        let dimension_changed = self.physics.current_dimension != snapshot.dimension.into();
        if dimension_changed {
            self.replicated_chunks.clear();
            self.physics.current_dimension = snapshot.dimension.into();
            self.physics.world = crate::world::World::network_replica(snapshot.dimension.into());
            // Old coordinates and held presses must not predict actions into
            // a different world while the transition acknowledgement arrives.
            self.pending_inputs
                .retain(|f| f.dimension == snapshot.dimension);
            self.outbound_inputs
                .retain(|f| f.dimension == snapshot.dimension);
        }

        let center = (snapshot.player.x.floor() as i32).div_euclid(CHUNK_WIDTH as i32);
        self.replicated_chunks
            .retain(|x, _| x.abs_diff(center) <= 6);
        let _ = self
            .physics
            .world
            .trim_chunk_cache(&[snapshot.player.x.floor() as i32], 0);
        self.local_tick = self.local_tick.max(snapshot.tick);
        self.pending_inputs
            .retain(|f| is_newer_sequence(snapshot.acknowledged_sequence, f.sequence));
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
            self.physics
                .world
                .apply_replica_biomes(chunk.chunk_x, &chunk.biomes);
            let _ = self
                .physics
                .world
                .apply_chunk_column_snapshot(chunk.chunk_x, &chunk.blocks);
            self.replicated_chunks.insert(chunk.chunk_x, chunk);
        }
    }

    fn rebuild_prediction_from_authoritative(&mut self) {
        let Some(authoritative) = &self.authoritative_snapshot else {
            return;
        };
        let mut predicted = authoritative.clone();
        authoritative.session.apply(&mut self.physics);
        let _ = authoritative.settings.apply(&mut self.physics);
        // Public snapshots may also be constructed independently of their session.
        self.physics.player.x = authoritative.player.x;
        self.physics.player.y = authoritative.player.y;
        self.physics.player.vx = authoritative.player.vx;
        self.physics.player.vy = authoritative.player.vy;
        self.physics.player.grounded = authoritative.player.grounded;
        self.physics.inventory_open = authoritative.inventory_open;
        self.physics.set_remote_ui_modal_state(
            authoritative.death_screen_active,
            authoritative.credits_active,
        );
        let end_tick = self
            .local_tick
            .max(authoritative.tick + u64::from(!self.pending_inputs.is_empty()))
            .min(authoritative.tick.saturating_add(10));
        for tick in authoritative.tick.saturating_add(1)..=end_tick {
            for input in &self.pending_inputs {
                if input.tick.max(authoritative.tick.saturating_add(1)) == tick {
                    match input.command {
                        ClientCommand::ToggleInventory => {
                            predicted.inventory_open = !predicted.inventory_open;
                            self.physics.inventory_open = predicted.inventory_open;
                            if predicted.inventory_open {
                                self.physics
                                    .apply_client_command(ClientCommand::ClearDirectionalInput);
                                self.physics
                                    .apply_client_command(ClientCommand::SetJumpHeld(false));
                            }
                        }
                        ClientCommand::SelectHotbarSlot(slot) if slot < 9 => {
                            predicted.hotbar_index = slot;
                            predicted.selected_hotbar_item =
                                authoritative.session.inventory().slots[slot as usize]
                                    .as_ref()
                                    .map(|s| s.item_type);
                        }
                        _ => {}
                    }
                    // Prediction only changes the local actor. Terrain edits, combat,
                    // inventory transactions and survival remain server-authoritative.
                    if matches!(
                        input.command,
                        ClientCommand::SetMoveLeft(_)
                            | ClientCommand::SetMoveRight(_)
                            | ClientCommand::ClearDirectionalInput
                            | ClientCommand::QueueJump
                            | ClientCommand::SetJumpHeld(_)
                            | ClientCommand::SetSneakHeld(_)
                            | ClientCommand::ToggleSneak
                            | ClientCommand::CycleMovementProfile
                    ) {
                        self.physics.apply_client_command(input.command);
                    }
                }
            }
            self.physics.predict_motion_tick();
            predicted.tick = tick;
        }
        // Only publish predicted motion. Replaying movement must not erase the
        // authoritative mining/bow animation or predict damage/inventory/survival.
        let motion = PlayerSnapshot::from_state(&self.physics);
        predicted.player.x = motion.x;
        predicted.player.y = motion.y;
        predicted.player.vx = motion.vx;
        predicted.player.vy = motion.vy;
        predicted.player.grounded = motion.grounded;
        predicted.player.facing_right = motion.facing_right;
        predicted.player.sneaking = motion.sneaking;
        self.predicted_snapshot = Some(predicted);
    }
}

#[derive(Debug)]
struct ConnectedClient {
    known_dimension: Option<SnapshotDimension>,
    client_id: u16,
    writer: TcpStream,
    known_chunk_revisions: HashMap<i32, u64>,
    authenticated: bool,
}
impl Drop for ConnectedClient {
    fn drop(&mut self) {
        let _ = self.writer.shutdown(std::net::Shutdown::Both);
    }
}
fn next_client_id(id: &mut u16) -> u16 {
    let current = *id;
    *id = id.wrapping_add(1);
    if *id == 0 {
        *id = 1;
    }
    current
}
enum ServerEvent {
    Hello(u16, ClientHello),
    Input(ClientInputFrame),
    Chat(u16, String),
    Disconnected(u16),
}
fn spawn_client_reader_thread(
    client_id: u16,
    mut reader: TcpStream,
    tx: mpsc::SyncSender<ServerEvent>,
) {
    thread::spawn(move || {
        if let Ok(hello) = read_client_hello(&mut reader)
            && tx.send(ServerEvent::Hello(client_id, hello)).is_ok()
        {
            let _ = reader.set_read_timeout(None);
            while let Ok(message) = read_framed_client_message(&mut reader) {
                let event = match message {
                    ClientWireMessage::Input(mut input) => {
                        input.client_id = client_id;
                        ServerEvent::Input(input)
                    }
                    ClientWireMessage::Chat(text)
                        if text.len() <= 2000 && !text.chars().any(char::is_control) =>
                    {
                        ServerEvent::Chat(client_id, text)
                    }
                    _ => break,
                };
                if tx.send(event).is_err() {
                    break;
                }
            }
        }
        let _ = tx.send(ServerEvent::Disconnected(client_id));
    });
}
fn accept_pending_clients(
    listener: &TcpListener,
    tx: &mpsc::SyncSender<ServerEvent>,
    clients: &mut Vec<ConnectedClient>,
    counter: &mut u16,
) -> io::Result<()> {
    loop {
        match listener.accept() {
            Ok((writer, _)) => {
                if clients.len() >= 8 {
                    let _ = writer.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                // BSD/macOS may inherit the listener's nonblocking flag. The
                // framed reader needs a blocking stream, with bounded writes;
                // otherwise a quiet connection is dropped on WouldBlock.
                writer.set_nonblocking(false)?;
                writer.set_nodelay(true)?;
                writer.set_write_timeout(Some(Duration::from_millis(16)))?;
                let reader = writer.try_clone()?;
                reader.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut id = next_client_id(counter);
                while clients.iter().any(|c| c.client_id == id) {
                    id = next_client_id(counter);
                }
                spawn_client_reader_thread(id, reader, tx.clone());
                clients.push(ConnectedClient {
                    known_dimension: None,
                    client_id: id,
                    writer,
                    known_chunk_revisions: HashMap::new(),
                    authenticated: false,
                });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn center_chunk_x(player_x: f64) -> i32 {
    (player_x.floor() as i32).div_euclid(CHUNK_WIDTH as i32)
}

#[cfg(test)]
fn build_chunk_delta_packet(
    state: &GameState,
    snapshot_tick: u64,
    center_player_x: f64,
    known_chunk_revisions: &mut HashMap<i32, u64>,
) -> Option<ChunkDeltaPacket> {
    build_world_chunk_delta_packet(
        &state.world,
        snapshot_tick,
        center_player_x,
        known_chunk_revisions,
    )
}

fn build_world_chunk_delta_packet(
    world: &crate::world::World,
    snapshot_tick: u64,
    center_player_x: f64,
    known_chunk_revisions: &mut HashMap<i32, u64>,
) -> Option<ChunkDeltaPacket> {
    let center = center_chunk_x(center_player_x);
    let mut changed_candidates = Vec::new();

    for chunk_x in (center - CHUNK_STREAM_RADIUS)..=(center + CHUNK_STREAM_RADIUS) {
        let Some((revision, blocks)) = world.chunk_column_snapshot(chunk_x) else {
            known_chunk_revisions.remove(&chunk_x);
            continue;
        };
        if known_chunk_revisions.get(&chunk_x).copied() == Some(revision) {
            continue;
        }
        changed_candidates.push(ChunkColumnSnapshot {
            chunk_x,
            revision,
            biomes: (0..CHUNK_WIDTH)
                .map(|x| world.get_biome(chunk_x * CHUNK_WIDTH as i32 + x as i32))
                .collect(),
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
            dimension: world.dimension.into(),
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
    if client.known_dimension != Some(snapshot.dimension) {
        client.known_chunk_revisions.clear();
        client.known_dimension = Some(snapshot.dimension);
    }
    write_framed_payload(&mut client.writer, snapshot_wire_payload)?;

    let world = state
        .world_in_dimension(snapshot.dimension.into())
        .ok_or_else(|| io::Error::other("missing dimension for client terrain"))?;
    if let Some(delta) = build_world_chunk_delta_packet(
        world,
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
    let message = ServerWireMessage::Snapshot(Box::new(snapshot.clone()));
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
    let root = crate::save_path::save_dir()
        .join("multiplayer")
        .join("default");
    std::fs::create_dir_all(&root)?;
    let _lease = crate::worlds::WorldLease::acquire(&root).map_err(io::Error::other)?;
    run_headless_tcp_server_with_state(bind_addr, GameState::open_in_directory(&root)?)
}
pub fn run_headless_tcp_server_with_state(bind_addr: &str, state: GameState) -> io::Result<()> {
    let mut simulation = ServerSimulation::with_state(state);
    simulation.load_persistent()?;
    let listener = TcpListener::bind(bind_addr)?;
    listener.set_nonblocking(true)?;
    eprintln!(
        "Starting headless server on {} (experimental local co-op, protocol {})",
        listener.local_addr()?,
        NET_PROTOCOL_VERSION
    );
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(unix)]
    {
        signal_hook::flag::register(signal_hook::consts::SIGINT, stop.clone())?;
        signal_hook::flag::register(signal_hook::consts::SIGTERM, stop.clone())?;
    }
    let (tx, rx) = mpsc::sync_channel(1024);
    let mut clients = Vec::new();
    let mut counter = 1;
    let duration = Duration::from_millis(1000 / SERVER_TICK_RATE);
    let mut last_tick = Instant::now();
    let mut last_save = Instant::now();
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        accept_pending_clients(&listener, &tx, &mut clients, &mut counter)?;
        for _ in 0..1024 {
            let Ok(event) = rx.try_recv() else {
                break;
            };
            match event {
                ServerEvent::Hello(id, hello) => {
                    if let Some(client) = clients.iter_mut().find(|c| c.client_id == id) {
                        let auth = simulation
                            .authenticate(id, hello.token.as_deref())
                            .and_then(|welcome| {
                                write_framed_server_message(
                                    &mut client.writer,
                                    &ServerWireMessage::Welcome(welcome),
                                )
                            });
                        if let Err(e) = auth {
                            eprintln!("Join rejected: {e}");
                            let _ = write_framed_server_message(
                                &mut client.writer,
                                &ServerWireMessage::Rejected(NET_PROTOCOL_VERSION, e.to_string()),
                            );
                            clients.retain(|c| c.client_id != id);
                            simulation.unregister_client(id);
                        } else {
                            client.authenticated = true;
                        }
                    }
                }
                ServerEvent::Input(frame) => {
                    if clients
                        .iter()
                        .any(|c| c.client_id == frame.client_id && c.authenticated)
                    {
                        simulation.enqueue_input(frame);
                    }
                }
                ServerEvent::Chat(id, text) => {
                    if clients.iter().any(|c| c.client_id == id && c.authenticated) {
                        simulation.submit_chat(id, text);
                    }
                }
                ServerEvent::Disconnected(id) => {
                    clients.retain(|c| c.client_id != id);
                    simulation.unregister_client(id);
                    if let Err(e) = simulation.save() {
                        eprintln!("Server save failed; retaining player data in memory: {e}");
                    }
                }
            }
        }
        let due = consume_due_server_ticks(
            Instant::now(),
            &mut last_tick,
            duration,
            SERVER_MAX_CATCHUP_TICKS_PER_LOOP,
        );
        for _ in 0..due {
            if simulation.primary_client_id.is_none() {
                continue;
            }
            simulation.step(0, 0);
            if let Err(e) = simulation.maintain_chunk_cache() {
                if simulation.state.check_dimension_storage().is_err() {
                    return Err(e);
                }
                eprintln!("Chunk eviction save failed; terrain retained: {e}");
            }
            let mut index = 0;
            while index < clients.len() {
                if !clients[index].authenticated {
                    index += 1;
                    continue;
                }
                let snapshot = simulation
                    .snapshot_for_client(clients[index].client_id)
                    .ok_or_else(|| io::Error::other("missing player"))?;
                let payload = build_snapshot_wire_payload(&snapshot)?;
                if send_tick_messages_to_client(
                    &mut clients[index],
                    simulation.state(),
                    &snapshot,
                    &payload,
                )
                .is_err()
                {
                    let id = clients[index].client_id;
                    clients.swap_remove(index);
                    simulation.unregister_client(id);
                    if let Err(e) = simulation.save() {
                        eprintln!("Server disconnect save failed; data retained: {e}");
                    }
                } else {
                    index += 1;
                }
            }
        }
        if last_save.elapsed() >= Duration::from_secs(45) {
            if let Err(e) = simulation.save() {
                eprintln!("Server autosave failed; retaining data: {e}");
            }
            last_save = Instant::now();
        }
        if due == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }
    simulation.save()
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
        let message =
            ServerWireMessage::Snapshot(Box::new(ServerSnapshot::from_state(5, &GameState::new())));
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
                assert_eq!(*decoded_snapshot, snapshot);
            }
            ServerWireMessage::Welcome(_)
            | ServerWireMessage::Rejected(_, _)
            | ServerWireMessage::ChunkDelta(_) => {
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
        let message =
            ServerWireMessage::Snapshot(Box::new(ServerSnapshot::from_state(7, &GameState::new())));
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
    fn delayed_snapshot_does_not_acknowledge_an_unreceived_input_by_tick() {
        let mut client = ClientPredictionState::new(1);
        let mut state = GameState::network_view();
        state.player.x = 0.5;
        state.player.y = 20.0;
        state.player.grounded = true;
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            ServerSnapshot::from_state(0, &state),
        )));
        let command = client.queue_local_command(ClientCommand::SetMoveRight(true));
        // The server advanced beyond this input's original tick before receiving it.
        let mut delayed = ServerSnapshot::from_state(4, &state);
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(delayed.clone())));
        assert_eq!(client.pending_input_count(), 1);
        assert!(client.predicted_snapshot().unwrap().player.x > state.player.x);
        delayed.tick = 5;
        delayed.acknowledged_sequence = command.sequence;
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(delayed)));
        assert_eq!(client.pending_input_count(), 0);
    }

    #[test]
    fn movement_prediction_preserves_authoritative_actions_and_survival() {
        let mut state = GameState::network_view();
        state.player.mining_timer = 0.65;
        state.player.health = 13.0;
        state.player.hunger = 8.0;
        state.player.grounded = true;
        let snapshot = ServerSnapshot::from_state(0, &state);
        let session = snapshot.session.clone();
        let mut client = ClientPredictionState::new(1);
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(snapshot)));
        client.queue_local_command(ClientCommand::SetMoveRight(true));
        client.advance_local_tick();
        let predicted = client.predicted_snapshot().unwrap();
        assert_eq!(predicted.session, session);
        assert_eq!(predicted.player.health, 13.0);
        assert_eq!(predicted.player.hunger, 8.0);
    }

    #[test]
    fn local_inventory_and_hotbar_feedback_do_not_wait_for_a_server_roundtrip() {
        let state = GameState::network_view();
        let mut client = ClientPredictionState::new(1);
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            ServerSnapshot::from_state(0, &state),
        )));
        client.queue_local_command(ClientCommand::ToggleInventory);
        client.queue_local_command(ClientCommand::SelectHotbarSlot(2));
        let predicted = client.predicted_snapshot().unwrap();
        assert!(predicted.inventory_open);
        assert_eq!(predicted.hotbar_index, 2);
        assert_eq!(predicted.session.inventory(), &state.inventory);
    }

    #[test]
    fn predicted_jump_lands_and_running_cannot_pass_through_a_wall() {
        let mut state = GameState::network_view();
        state.player.x = 2.5;
        state.player.y = 20.0;
        state.player.grounded = true;
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT];
        for y in 20..crate::world::chunk::CHUNK_HEIGHT {
            for x in 0..CHUNK_WIDTH {
                blocks[y * CHUNK_WIDTH + x] = BlockType::Stone;
            }
        }
        for y in 10..20 {
            blocks[y * CHUNK_WIDTH + 7] = BlockType::Stone;
        }
        let mut client = ClientPredictionState::new(1);
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            ServerSnapshot::from_state(0, &state),
        )));
        client.apply_server_message(ServerWireMessage::ChunkDelta(ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 0,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 1,
                biomes: vec![crate::world::BiomeType::Plains; CHUNK_WIDTH],
                blocks,
            }],
        }));
        client.queue_local_command(ClientCommand::QueueJump);
        client.queue_local_command(ClientCommand::SetMoveRight(true));
        assert!(client.predicted_snapshot().unwrap().player.y < 20.0);
        for _ in 0..10 {
            client.advance_local_tick();
        }
        let player = &client.predicted_snapshot().unwrap().player;
        assert!(
            player.x <= 6.751,
            "predicted player crossed wall: {player:?}"
        );
        // A fresh authoritative airborne state permits continued prediction to landing.
        let next = client.predicted_snapshot().unwrap().clone();
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(ServerSnapshot {
            acknowledged_sequence: 2,
            ..next
        })));
        for _ in 0..10 {
            client.advance_local_tick();
        }
        let player = &client.predicted_snapshot().unwrap().player;
        assert!(
            (player.y - 20.0).abs() < 0.002,
            "jump did not land: {player:?}"
        );
        assert!(player.grounded);
    }

    #[test]
    fn client_prediction_reconciles_against_authoritative_snapshot() {
        let mut client = ClientPredictionState::new(1);
        let baseline = ServerSnapshot::from_state(0, &GameState::new());
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(baseline.clone())));

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
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            authoritative_tick_1.clone(),
        )));

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
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(s5)));

        let mut s4 = ServerSnapshot::from_state(4, &GameState::new());
        s4.tick = 4;
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(s4)));

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
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            ServerSnapshot::from_state(2, &GameState::new()),
        )));
        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 2,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                biomes: vec![crate::world::BiomeType::Plains; CHUNK_WIDTH],
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
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            ServerSnapshot::from_state(2, &GameState::new()),
        )));
        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 2,
            dimension: SnapshotDimension::Nether,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                biomes: vec![crate::world::BiomeType::Plains; CHUNK_WIDTH],
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
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(
            ServerSnapshot::from_state(2, &GameState::new()),
        )));
        let overworld_delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 2,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                biomes: vec![crate::world::BiomeType::Plains; CHUNK_WIDTH],
                revision: 888,
                blocks: vec![BlockType::Air; 16],
            }],
        };
        client.apply_server_message(ServerWireMessage::ChunkDelta(overworld_delta));
        assert_eq!(client.replicated_chunk_count(), 1);

        let mut nether_state = GameState::new();
        nether_state.current_dimension = Dimension::Nether;
        let nether_snapshot = ServerSnapshot::from_state(3, &nether_state);
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(nether_snapshot)));

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

#[cfg(test)]
#[path = "cooperative_dimensions_tests.rs"]
mod cooperative_dimensions_tests;

#[cfg(test)]
mod dimension_prediction_tests {
    use super::*;
    #[test]
    fn first_nether_snapshot_resets_the_replica_and_drops_source_world_presses() {
        let mut client = ClientPredictionState::new(1);
        client.queue_local_command(ClientCommand::SetMoveRight(true));
        let mut state = GameState::network_view();
        state.current_dimension = Dimension::Nether;
        let mut snapshot = ServerSnapshot::from_state(1, &state);
        snapshot.acknowledged_sequence = 0;
        client.apply_server_message(ServerWireMessage::Snapshot(Box::new(snapshot)));
        assert_eq!(client.physics.current_dimension, Dimension::Nether);
        assert_eq!(client.physics.world.dimension, Dimension::Nether);
        assert_eq!(client.pending_input_count(), 0);
        assert!(client.take_outbound_inputs().is_empty());
        assert_eq!(
            client
                .queue_local_command(ClientCommand::UseAt(0, 8))
                .dimension,
            SnapshotDimension::Nether
        );
    }
}
#[cfg(test)]
#[path = "cooperative_tests.rs"]
mod cooperative_tests;
