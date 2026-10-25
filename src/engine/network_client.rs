#[path = "client_identity.rs"]
mod identity;
#[path = "server_inbox.rs"]
mod server_inbox;
use super::command::ClientCommand;
use super::game_loop::{
    LegacyJumpGrace, confirms_control_key_releases, frame_deadline, input_stale_timeout,
};
use super::net::{
    ChunkDeltaPacket, ClientHello, ClientInputFrame, ClientPredictionState, NET_PROTOCOL_VERSION,
    SERVER_TICK_RATE, SOCKET_WRITE_TIMEOUT, ServerSnapshot, ServerWireMessage, Welcome,
    read_framed_server_message, write_client_hello, write_framed_client_chat,
    write_framed_client_input,
};
use super::state::GameState;
use crate::renderer::Renderer;
use server_inbox::ServerInbox;

use crate::world::{Dimension, World};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseEventKind,
};
use std::collections::VecDeque;
use std::io;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const CLIENT_TICK_DURATION: Duration = Duration::from_millis(1000 / SERVER_TICK_RATE);
const CLIENT_MAX_CATCHUP_TICKS_PER_FRAME: u8 = 5;
const CLIENT_TARGET_FPS: u64 = 60;
const CLIENT_FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / CLIENT_TARGET_FPS);
const MAX_INPUT_EVENTS_PER_FRAME: usize = 64;
const INPUT_TIME_BUDGET: Duration = Duration::from_millis(2);
// Match native input: conservative pulses until a control release is observed,
// then hold until actual release or a focus/menu/connection reset.
const REMOTE_HORIZONTAL_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(150);
const REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(96);
const REMOTE_HORIZONTAL_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(320);
const REMOTE_JUMP_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(340);
const REMOTE_JUMP_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(160);
const REMOTE_JUMP_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(420);

fn connection_error(error: io::Error, address: &str, joining: bool) -> io::Error {
    let message = match error.kind() {
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::UnexpectedEof
        | io::ErrorKind::NotConnected => {
            if joining {
                format!(
                    "The server at {address} closed the connection while joining. Check that the host is still running and every player uses the same build; restart the host after updating. ({error})"
                )
            } else {
                format!(
                    "Disconnected from {address}: {error}. Check the host's host.log for the disconnect reason. Quitting 'termcraft host' ends the shared session; reconnect when it is running again."
                )
            }
        }
        io::ErrorKind::ConnectionRefused => format!(
            "No Termcraft server is accepting connections at {address}. Start 'termcraft host' and use the address shown in its T panel. ({error})"
        ),
        io::ErrorKind::InvalidData if error.to_string().contains("protocol") => format!(
            "Client and server builds do not match. Update all players and restart the host. ({error})"
        ),
        _ => return error,
    };
    io::Error::new(error.kind(), message)
}

fn connect_to_server(address: &str, token: Option<String>) -> io::Result<(TcpStream, Welcome)> {
    let connect = || {
        let mut writer = TcpStream::connect(address)?;
        writer.set_nodelay(true)?;
        writer.set_write_timeout(Some(Duration::from_secs(5)))?;
        writer.set_read_timeout(Some(Duration::from_secs(5)))?;
        write_client_hello(
            &mut writer,
            &ClientHello {
                protocol: NET_PROTOCOL_VERSION,
                token,
            },
        )?;
        let welcome = match read_framed_server_message(&mut writer)? {
            ServerWireMessage::Welcome(w) => w,
            ServerWireMessage::Rejected(_, message) => {
                let help = if message.contains("already connected") {
                    " Each terminal on one computer needs a different --player profile, for example --player bob or --player charlie."
                } else {
                    ""
                };
                return Err(io::Error::other(format!("Join rejected: {message}{help}")));
            }
            _ => return Err(io::Error::other("server did not send a welcome")),
        };
        writer.set_write_timeout(Some(SOCKET_WRITE_TIMEOUT))?;
        writer.set_read_timeout(None)?;
        Ok((writer, welcome))
    };
    connect().map_err(|error| connection_error(error, address, true))
}

fn queue_client_command(
    prediction: &mut ClientPredictionState,
    pending_outbound: &mut VecDeque<ClientInputFrame>,
    command: ClientCommand,
) {
    prediction.queue_local_command(command);
    pending_outbound.extend(prediction.take_outbound_inputs());
}

enum ClientPacket {
    Input(ClientInputFrame),
    Chat(String),
}

struct ClientWriter {
    outgoing: mpsc::SyncSender<ClientPacket>,
    errors: mpsc::Receiver<io::Error>,
}

impl ClientWriter {
    fn spawn(mut writer: TcpStream) -> Self {
        let (outgoing, incoming) = mpsc::sync_channel(256);
        let (errors_tx, errors) = mpsc::sync_channel(1);
        thread::spawn(move || {
            while let Ok(packet) = incoming.recv() {
                let result = match packet {
                    ClientPacket::Input(frame) => write_framed_client_input(&mut writer, &frame),
                    ClientPacket::Chat(text) => write_framed_client_chat(&mut writer, &text),
                };
                if let Err(error) = result {
                    let _ = errors_tx.send(error);
                    let _ = writer.shutdown(std::net::Shutdown::Both);
                    break;
                }
            }
        });
        Self { outgoing, errors }
    }

    fn check(&self) -> io::Result<()> {
        match self.errors.try_recv() {
            Ok(error) => Err(error),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "client socket writer closed",
            )),
            Err(mpsc::TryRecvError::Empty) => Ok(()),
        }
    }

    fn send(&self, packet: ClientPacket) -> io::Result<()> {
        self.check()?;
        self.outgoing.try_send(packet).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => io::Error::new(
                io::ErrorKind::WouldBlock,
                "connection stalled: client input queue is full",
            ),
            mpsc::TrySendError::Disconnected(_) => {
                io::Error::new(io::ErrorKind::BrokenPipe, "client socket writer closed")
            }
        })
    }

    fn finish(self) -> io::Result<()> {
        // On a normal quit, finish ordered inputs before closing the socket so
        // the server checkpoints the player's final inventory/settings changes.
        drop(self.outgoing);
        match self.errors.recv_timeout(SOCKET_WRITE_TIMEOUT) {
            Ok(error) => Err(error),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection stalled while finishing queued inputs",
            )),
        }
    }
}

fn flush_outbound_commands(
    writer: &ClientWriter,
    pending_outbound: &mut VecDeque<ClientInputFrame>,
) -> io::Result<()> {
    while let Some(frame) = pending_outbound.pop_front() {
        writer.send(ClientPacket::Input(frame))?;
    }
    writer.check()
}

fn clear_remote_runtime_entities(state: &mut GameState) {
    state.remote_players.clear();
    state.zombies.clear();
    state.creepers.clear();
    state.skeletons.clear();
    state.spiders.clear();
    state.silverfish.clear();
    state.slimes.clear();
    state.endermen.clear();
    state.blazes.clear();
    state.pigmen.clear();
    state.ghasts.clear();
    state.cows.clear();
    state.sheep.clear();
    state.pigs.clear();
    state.chickens.clear();
    state.squids.clear();
    state.wolves.clear();
    state.ocelots.clear();
    state.villagers.clear();
    state.item_entities.clear();
    state.experience_orbs.clear();
    state.arrows.clear();
    state.fireballs.clear();
    state.end_crystals.clear();
    state.ender_dragon = None;
    state.lightning_bolts.clear();
    state.at_crafting_table = false;
    state.at_furnace = false;
    state.at_chest = false;
    state.at_enchanting_table = false;
    state.at_anvil = false;
    state.at_brewing_stand = false;
    state.selected_inventory_slot = None;
    state.left_click_down = false;
}

pub(crate) fn sync_state_from_snapshot(state: &mut GameState, snapshot: &ServerSnapshot) {
    state.local_player_id = (snapshot.client_id != 0).then_some(snapshot.client_id);
    state.host_client_id = snapshot.host_client_id;
    state.install_shortcuts(snapshot.shortcuts);
    let target_dimension: Dimension = snapshot.dimension.into();
    if state.current_dimension != target_dimension {
        state.current_dimension = target_dimension;
        state.world = World::network_replica(target_dimension);
        clear_remote_runtime_entities(state);
    }

    state.chat.lines = snapshot.chat.lines.clone();
    state.chat.status = snapshot.chat.status.clone();
    state.chat.mode = snapshot.chat.mode.clone();
    state.chat.pending = snapshot.chat.pending;
    let _ = snapshot.settings.apply(state);
    snapshot.session.apply(state);
    state.install_action_visual(snapshot.action);
    snapshot.entities.apply(state);
    state.trial = snapshot.trial.clone();
    state.thunder_flash_timer = snapshot.thunder_flash;
    state.weather_rain_intensity = snapshot.rain_intensity;
    state.weather_wind_intensity = snapshot.wind_intensity;
    state.weather_thunder_intensity = snapshot.thunder_intensity;
    state.weather = snapshot.weather.into();
    state.time_of_day = snapshot.time_of_day;
    state.inventory_open = snapshot.inventory_open;
    state.set_remote_ui_modal_state(snapshot.death_screen_active, snapshot.credits_active);

    state.player.x = snapshot.player.x;
    state.player.y = snapshot.player.y;
    state.player.vx = snapshot.player.vx;
    state.player.vy = snapshot.player.vy;
    state.player.grounded = snapshot.player.grounded;
    state.player.facing_right = snapshot.player.facing_right;
    state.player.sneaking = snapshot.player.sneaking;
    state.player.health = snapshot.player.health;
    state.player.max_health = snapshot.player.max_health;
    state.player.hunger = snapshot.player.hunger;
    state.player.max_hunger = snapshot.player.max_hunger;
    state.remote_players = snapshot
        .remote_players
        .iter()
        .map(|remote| crate::engine::state::RemotePlayerState {
            client_id: remote.client_id,
            armor: remote.armor,
            action: remote.action,
            held_item: remote.held_item,
            hurt_ticks: remote.hurt_ticks,
            x: remote.player.x,
            y: remote.player.y,
            vx: remote.player.vx,
            vy: remote.player.vy,
            grounded: remote.player.grounded,
            facing_right: remote.player.facing_right,
            sneaking: remote.player.sneaking,
            health: remote.player.health,
            max_health: remote.player.max_health,
            hunger: remote.player.hunger,
            max_hunger: remote.player.max_hunger,
        })
        .collect();
    state
        .world
        .load_chunks_for_motion(state.player.x, state.player.vx);

    state.hotbar_index = snapshot.hotbar_index.min(8);
    if let Some((x, y, inventory)) = &snapshot.chest
        && let Some(chest) = state
            .world
            .ensure_chest_inventory(*x, *y, inventory.capacity)
    {
        *chest = inventory.clone();
    }
    let _ = state
        .world
        .trim_chunk_cache(&[state.player.x.floor() as i32], 0);
}

fn is_jump_key(code: &KeyCode) -> bool {
    matches!(code, KeyCode::Up | KeyCode::Char(' '))
        || matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'w'))
}

pub(crate) fn apply_chunk_delta_to_state(state: &mut GameState, delta: &ChunkDeltaPacket) {
    if state.current_dimension != Dimension::from(delta.dimension) {
        return;
    }

    for chunk in &delta.chunks {
        state
            .world
            .apply_replica_biomes(chunk.chunk_x, &chunk.biomes);
        let _ = state
            .world
            .apply_chunk_column_snapshot(chunk.chunk_x, &chunk.blocks);
    }
}

fn is_shift_modifier_key(code: &KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Modifier(ModifierKeyCode::LeftShift)
            | KeyCode::Modifier(ModifierKeyCode::RightShift)
            | KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift)
            | KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift)
    )
}

fn sync_remote_sneak_hold(
    prediction: &mut ClientPredictionState,
    pending_outbound: &mut VecDeque<ClientInputFrame>,
    desired: bool,
    sent_state: &mut bool,
) {
    if *sent_state == desired {
        return;
    }
    *sent_state = desired;
    queue_client_command(
        prediction,
        pending_outbound,
        ClientCommand::SetSneakHeld(desired),
    );
}

fn spawn_server_message_reader(mut reader: TcpStream, inbox: ServerInbox) {
    thread::spawn(move || {
        loop {
            match read_framed_server_message(&mut reader) {
                Ok(message) => {
                    if !inbox.push(message) {
                        break;
                    }
                }
                Err(err) => {
                    inbox.fail(err);
                    break;
                }
            }
        }
    });
}

fn apply_server_updates(
    state: &mut GameState,
    prediction: &mut ClientPredictionState,
    server_inbox: &ServerInbox,
) -> io::Result<()> {
    let (updates, error) = server_inbox.take();
    for mut update in updates {
        if let Some(snapshot) = update.snapshot.take() {
            state
                .performance
                .network_inbox
                .record(update.received_at.elapsed());
            state.performance.network_received_at = Some(update.received_at);
            state.performance.network_tick = snapshot.tick;
            state.performance.network_superseded += update.superseded_snapshots;
            sync_state_from_snapshot(state, &snapshot);
            prediction.apply_server_message(ServerWireMessage::Snapshot(snapshot));
        }
        if let Some(delta) = update.take_terrain() {
            apply_chunk_delta_to_state(state, &delta);
            prediction.apply_server_message(ServerWireMessage::ChunkDelta(delta));
        }
    }
    error.map_or(Ok(()), Err)
}

pub fn run_tcp_client(connect_addr: &str) -> io::Result<()> {
    run_tcp_client_with_profile(connect_addr, "player")
}

pub fn run_tcp_client_with_profile(connect_addr: &str, profile: &str) -> io::Result<()> {
    run_tcp_client_with_identity(connect_addr, profile, connect_addr)
}
pub(crate) fn run_tcp_client_with_hint(
    connect_addr: &str,
    profile: &str,
    hint: &str,
) -> io::Result<()> {
    run_tcp_client_with_identity_and_hint(connect_addr, profile, connect_addr, hint)
}
pub(crate) fn run_tcp_client_with_identity(
    connect_addr: &str,
    profile: &str,
    identity_addr: &str,
) -> io::Result<()> {
    run_tcp_client_with_identity_and_hint(connect_addr, profile, identity_addr, identity_addr)
}
fn run_tcp_client_with_identity_and_hint(
    connect_addr: &str,
    profile: &str,
    identity_addr: &str,
    hint: &str,
) -> io::Result<()> {
    let previous = identity::read(identity_addr, profile)?;
    let (writer, welcome) =
        connect_to_server(connect_addr, previous.as_ref().map(|p| p.token.clone()))?;
    identity::store(identity_addr, profile, previous.as_ref(), &welcome)?;
    writer.set_read_timeout(None)?;
    let reader = writer.try_clone()?;
    reader.set_nodelay(true)?;
    let client_writer = ClientWriter::spawn(writer.try_clone()?);

    let server_inbox = ServerInbox::default();
    let mut renderer = Renderer::new()?;
    if let Err(error) = renderer.init() {
        let _ = renderer.restore();
        return Err(error);
    }

    spawn_server_message_reader(reader, server_inbox.clone());
    let result = run_tcp_client_loop(
        &client_writer,
        &server_inbox,
        &mut renderer,
        welcome.client_id,
        hint,
    );
    let result = result.and_then(|()| {
        client_writer
            .finish()
            .map_err(|error| connection_error(error, hint, false))
    });
    let _ = writer.shutdown(std::net::Shutdown::Both);
    result.and(renderer.restore())
}

fn run_tcp_client_loop(
    writer: &ClientWriter,
    server_inbox: &ServerInbox,
    renderer: &mut Renderer,
    client_id: u16,
    hint: &str,
) -> io::Result<()> {
    let mut state = GameState::network_view();
    state.connection_address = Some(hint.into());
    clear_remote_runtime_entities(&mut state);
    state.inventory.slots.fill(None);
    state.inventory_open = false;
    state.mouse_x = 0;
    state.mouse_y = 0;

    let mut prediction = ClientPredictionState::new(client_id);
    let mut pending_outbound = VecDeque::<ClientInputFrame>::new();
    let mut inventory_drag_slot = None;

    let mut left_active = false;
    let mut right_active = false;
    let mut left_last_input_at: Option<Instant> = None;
    let mut right_last_input_at: Option<Instant> = None;
    let mut left_repeat_observed = false;
    let mut right_repeat_observed = false;
    let mut jump_movement_grace = LegacyJumpGrace::default();
    let mut jump_held_sent = false;
    let mut jump_last_input_at: Option<Instant> = None;
    let mut jump_repeat_observed = false;
    let mut sneak_held_sent = false;

    let mut last_tick = Instant::now();
    let mut next_frame_deadline = Instant::now();
    let mut running = true;
    let mut last_heartbeat = Instant::now();
    let mut pending_aim = None;
    let mut last_aim_sent = Instant::now() - CLIENT_TICK_DURATION;
    let mut key_releases = renderer.reports_key_releases();

    while running {
        let input_started = Instant::now();
        for _ in 0..MAX_INPUT_EVENTS_PER_FRAME {
            if input_started.elapsed() >= INPUT_TIME_BUDGET
                || !event::poll(Duration::from_millis(0))?
            {
                break;
            }
            let ev = event::read()?;
            if matches!(&ev, Event::Key(key) if confirms_control_key_releases(key)) {
                key_releases = true;
                renderer.note_key_release();
            }
            if state.quit_confirmation {
                match ev {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Enter | KeyCode::Char('y' | 'Y') => running = false,
                        KeyCode::Esc | KeyCode::Char('n' | 'N') => state.quit_confirmation = false,
                        _ => {}
                    },
                    _ => {}
                }
                continue;
            }
            if state.chat.open {
                match &ev {
                    Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                        KeyCode::Esc => state.chat.open = false,
                        KeyCode::Enter if key.kind == KeyEventKind::Press => {
                            let prompt = std::mem::take(&mut state.chat.draft);
                            if !prompt.trim().is_empty() {
                                flush_outbound_commands(writer, &mut pending_outbound)
                                    .map_err(|error| connection_error(error, hint, false))?;
                                writer
                                    .send(ClientPacket::Chat(prompt))
                                    .map_err(|error| connection_error(error, hint, false))?;
                            }
                            state.chat.open = false;
                        }
                        KeyCode::Backspace => {
                            state.chat.draft.pop();
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            state.chat.draft.clear();
                            state.chat.open = false;
                        }
                        KeyCode::PageUp => {
                            state.chat.scroll = state.chat.scroll.saturating_add(8).min(8192)
                        }
                        KeyCode::PageDown => {
                            state.chat.scroll = state.chat.scroll.saturating_sub(8)
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            state.chat.insert(&c.to_string())
                        }
                        _ => {}
                    },
                    Event::Paste(text) => state.chat.insert(text),
                    _ => {}
                }
                continue;
            }
            if state.is_settings_menu_open() {
                if let Event::Key(key) = &ev
                    && key.kind != KeyEventKind::Release
                {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('o' | 'O' | 'q' | 'Q') => {
                            state.apply_client_command(ClientCommand::ToggleSettingsMenu)
                        }
                        KeyCode::Up | KeyCode::Char('w' | 'W') => {
                            state.apply_client_command(ClientCommand::SettingsMoveUp)
                        }
                        KeyCode::Down | KeyCode::Char('s' | 'S') => {
                            state.apply_client_command(ClientCommand::SettingsMoveDown)
                        }
                        KeyCode::Enter | KeyCode::Char(' ') if key.kind == KeyEventKind::Press => {
                            if let Some(command) = state.settings_selected_command() {
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    command,
                                );
                                state.apply_client_command(command);
                            } else {
                                state.apply_client_command(ClientCommand::ToggleSettingsMenu);
                            }
                        }
                        _ => {}
                    }
                }
                continue;
            }
            match ev {
                Event::Key(key_event) => {
                    match key_event.kind {
                        KeyEventKind::Press => match key_event.code {
                            KeyCode::Tab if state.inventory_open => renderer.toggle_recipe_book(),
                            KeyCode::PageDown if state.inventory_open => {
                                renderer.scroll_recipe_book(8)
                            }
                            KeyCode::PageUp if state.inventory_open => {
                                renderer.scroll_recipe_book(-8)
                            }
                            KeyCode::Char('t' | 'T' | 'o' | 'O') if !state.inventory_open => {
                                for command in [
                                    ClientCommand::ClearDirectionalInput,
                                    ClientCommand::SetPrimaryAction(false),
                                    ClientCommand::SetJumpHeld(false),
                                    ClientCommand::SetSneakHeld(false),
                                ] {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        command,
                                    );
                                }
                                left_active = false;
                                right_active = false;
                                left_last_input_at = None;
                                right_last_input_at = None;
                                left_repeat_observed = false;
                                right_repeat_observed = false;
                                jump_held_sent = false;
                                jump_last_input_at = None;
                                jump_repeat_observed = false;
                                sneak_held_sent = false;
                                if matches!(key_event.code, KeyCode::Char('o' | 'O')) {
                                    state.apply_client_command(ClientCommand::ToggleSettingsMenu);
                                } else {
                                    state.chat.open = true;
                                }
                                state.chat.scroll = 0;
                            }
                            KeyCode::F(4) => renderer.toggle_zoom(),
                            KeyCode::Char('+' | '=') => renderer.zoom_in(),
                            KeyCode::Char('-') => renderer.zoom_out(),
                            KeyCode::F(3) => state.show_debug_overlay = !state.show_debug_overlay,
                            KeyCode::Char('q' | 'Q') | KeyCode::Esc => {
                                if state.inventory_open {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::ToggleInventory,
                                    );
                                } else {
                                    state.quit_confirmation = true;
                                    left_active = false;
                                    right_active = false;
                                    jump_held_sent = false;
                                    sneak_held_sent = false;
                                    for command in [
                                        ClientCommand::ClearDirectionalInput,
                                        ClientCommand::SetPrimaryAction(false),
                                        ClientCommand::SetJumpHeld(false),
                                        ClientCommand::SetSneakHeld(false),
                                    ] {
                                        queue_client_command(
                                            &mut prediction,
                                            &mut pending_outbound,
                                            command,
                                        );
                                    }
                                }
                            }
                            KeyCode::Char('e') => {
                                for command in [
                                    ClientCommand::ClearDirectionalInput,
                                    ClientCommand::SetJumpHeld(false),
                                ] {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        command,
                                    );
                                }
                                left_active = false;
                                right_active = false;
                                left_last_input_at = None;
                                right_last_input_at = None;
                                left_repeat_observed = false;
                                right_repeat_observed = false;
                                jump_held_sent = false;
                                jump_last_input_at = None;
                                jump_repeat_observed = false;
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::ToggleInventory,
                                );
                            }
                            KeyCode::Up | KeyCode::Char(' ') => {
                                let now = Instant::now();
                                jump_repeat_observed = jump_last_input_at.is_some_and(|ts| {
                                    now.duration_since(ts) <= REMOTE_JUMP_PRESS_REPEAT_WINDOW
                                });
                                if !key_releases && !jump_held_sent {
                                    jump_movement_grace.begin(
                                    now,
                                    (left_active && left_repeat_observed
                                        && left_last_input_at.is_some_and(|at| now.duration_since(at) <= REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT))
                                        || (right_active && right_repeat_observed
                                            && right_last_input_at.is_some_and(|at| now.duration_since(at) <= REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT)),
                                );
                                }
                                jump_last_input_at = Some(now);
                                if !jump_held_sent {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetJumpHeld(true),
                                    );
                                    jump_held_sent = true;
                                }
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::QueueJump,
                                );
                            }
                            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'w') => {
                                let now = Instant::now();
                                jump_repeat_observed = jump_last_input_at.is_some_and(|ts| {
                                    now.duration_since(ts) <= REMOTE_JUMP_PRESS_REPEAT_WINDOW
                                });
                                if !key_releases && !jump_held_sent {
                                    jump_movement_grace.begin(
                                    now,
                                    (left_active && left_repeat_observed
                                        && left_last_input_at.is_some_and(|at| now.duration_since(at) <= REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT))
                                        || (right_active && right_repeat_observed
                                            && right_last_input_at.is_some_and(|at| now.duration_since(at) <= REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT)),
                                );
                                }
                                jump_last_input_at = Some(now);
                                if !jump_held_sent {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetJumpHeld(true),
                                    );
                                    jump_held_sent = true;
                                }
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::QueueJump,
                                );
                            }
                            KeyCode::F(5) if state.shortcuts_enabled() => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::TravelToOverworld,
                            ),
                            KeyCode::F(6) if state.shortcuts_enabled() => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::TravelToNether,
                            ),
                            KeyCode::F(7) if state.shortcuts_enabled() => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::TravelToEnd,
                            ),
                            KeyCode::F(8) if state.shortcuts_enabled() => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::TravelToSpawn,
                            ),
                            KeyCode::F(9) if state.shortcuts_enabled() => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::EquipDiamondLoadout,
                            ),
                            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'f') => {
                                if !state.inventory_open {
                                    let (bx, by) = renderer.screen_to_world(
                                        &state,
                                        state.mouse_x,
                                        state.mouse_y,
                                    );
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::UseAt(bx, by),
                                    );
                                }
                            }
                            KeyCode::Char('x') => {
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::ToggleSneak,
                                );
                            }
                            KeyCode::Char('v' | 'V') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::DropItem(
                                    key_event.modifiers.contains(KeyModifiers::SHIFT)
                                        || key_event.code == KeyCode::Char('V'),
                                ),
                            ),
                            KeyCode::Delete if state.inventory_open => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::ClearCraftGrid,
                            ),
                            KeyCode::Enter if state.inventory_open => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::CraftGrid(
                                    key_event.modifiers.contains(KeyModifiers::SHIFT),
                                ),
                            ),
                            KeyCode::Enter | KeyCode::Char('r') => {
                                if state.is_showing_death_screen() {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::RespawnFromDeathScreen,
                                    );
                                } else if state.is_showing_credits() {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SkipCompletionCredits,
                                    );
                                }
                            }
                            KeyCode::Left | KeyCode::Char('a' | 'A') if !state.inventory_open => {
                                let now = Instant::now();
                                left_repeat_observed = left_last_input_at.is_some_and(|last| {
                                    now.duration_since(last)
                                        <= REMOTE_HORIZONTAL_PRESS_REPEAT_WINDOW
                                });
                                left_last_input_at = Some(now);
                                jump_movement_grace.directional_event();
                                if !left_active {
                                    jump_movement_grace.reset();
                                    left_active = true;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetMoveLeft(true),
                                    );
                                }
                                if right_active {
                                    right_active = false;
                                    right_last_input_at = None;
                                    right_repeat_observed = false;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetMoveRight(false),
                                    );
                                }
                            }
                            KeyCode::Right | KeyCode::Char('d' | 'D') if !state.inventory_open => {
                                let now = Instant::now();
                                right_repeat_observed = right_last_input_at.is_some_and(|last| {
                                    now.duration_since(last)
                                        <= REMOTE_HORIZONTAL_PRESS_REPEAT_WINDOW
                                });
                                right_last_input_at = Some(now);
                                jump_movement_grace.directional_event();
                                if !right_active {
                                    jump_movement_grace.reset();
                                    right_active = true;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetMoveRight(true),
                                    );
                                }
                                if left_active {
                                    left_active = false;
                                    left_last_input_at = None;
                                    left_repeat_observed = false;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetMoveLeft(false),
                                    );
                                }
                            }
                            KeyCode::Char('1') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(0),
                            ),
                            KeyCode::Char('2') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(1),
                            ),
                            KeyCode::Char('3') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(2),
                            ),
                            KeyCode::Char('4') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(3),
                            ),
                            KeyCode::Char('5') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(4),
                            ),
                            KeyCode::Char('6') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(5),
                            ),
                            KeyCode::Char('7') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(6),
                            ),
                            KeyCode::Char('8') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(7),
                            ),
                            KeyCode::Char('9') => queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SelectHotbarSlot(8),
                            ),
                            KeyCode::Modifier(ModifierKeyCode::LeftShift)
                            | KeyCode::Modifier(ModifierKeyCode::RightShift)
                            | KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift)
                            | KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift) => {
                                sync_remote_sneak_hold(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    true,
                                    &mut sneak_held_sent,
                                );
                            }
                            _ => {}
                        },
                        KeyEventKind::Repeat => {
                            if is_jump_key(&key_event.code) {
                                let now = Instant::now();
                                jump_repeat_observed = true;
                                jump_last_input_at = Some(now);
                                if !jump_held_sent {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetJumpHeld(true),
                                    );
                                    jump_held_sent = true;
                                }
                            }
                            if !state.inventory_open
                                && matches!(
                                    key_event.code,
                                    KeyCode::Left
                                        | KeyCode::Right
                                        | KeyCode::Char('a')
                                        | KeyCode::Char('A')
                                        | KeyCode::Char('d')
                                        | KeyCode::Char('D')
                                )
                            {
                                if matches!(
                                    key_event.code,
                                    KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A')
                                ) {
                                    left_repeat_observed = true;
                                    left_last_input_at = Some(Instant::now());
                                    jump_movement_grace.directional_event();
                                    if !left_active {
                                        jump_movement_grace.reset();
                                        left_active = true;
                                        queue_client_command(
                                            &mut prediction,
                                            &mut pending_outbound,
                                            ClientCommand::SetMoveLeft(true),
                                        );
                                    }
                                    if right_active {
                                        right_active = false;
                                        right_last_input_at = None;
                                        right_repeat_observed = false;
                                        queue_client_command(
                                            &mut prediction,
                                            &mut pending_outbound,
                                            ClientCommand::SetMoveRight(false),
                                        );
                                    }
                                } else {
                                    right_repeat_observed = true;
                                    right_last_input_at = Some(Instant::now());
                                    jump_movement_grace.directional_event();
                                    if !right_active {
                                        jump_movement_grace.reset();
                                        right_active = true;
                                        queue_client_command(
                                            &mut prediction,
                                            &mut pending_outbound,
                                            ClientCommand::SetMoveRight(true),
                                        );
                                    }
                                    if left_active {
                                        left_active = false;
                                        left_last_input_at = None;
                                        left_repeat_observed = false;
                                        queue_client_command(
                                            &mut prediction,
                                            &mut pending_outbound,
                                            ClientCommand::SetMoveLeft(false),
                                        );
                                    }
                                }
                            }
                        }
                        KeyEventKind::Release => match key_event.code {
                            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                                if left_active {
                                    left_active = false;
                                    left_last_input_at = None;
                                    left_repeat_observed = false;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetMoveLeft(false),
                                    );
                                }
                            }
                            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                                if right_active {
                                    right_active = false;
                                    right_last_input_at = None;
                                    right_repeat_observed = false;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetMoveRight(false),
                                    );
                                }
                            }
                            KeyCode::Up
                            | KeyCode::Char('w')
                            | KeyCode::Char('W')
                            | KeyCode::Char(' ') => {
                                if jump_held_sent {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetJumpHeld(false),
                                    );
                                }
                                jump_held_sent = false;
                                jump_last_input_at = None;
                                jump_repeat_observed = false;
                            }
                            KeyCode::Null => {
                                if left_active || right_active {
                                    left_active = false;
                                    right_active = false;
                                    left_last_input_at = None;
                                    right_last_input_at = None;
                                    left_repeat_observed = false;
                                    right_repeat_observed = false;
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::ClearDirectionalInput,
                                    );
                                }
                                if jump_held_sent {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        ClientCommand::SetJumpHeld(false),
                                    );
                                }
                                jump_held_sent = false;
                                jump_last_input_at = None;
                                jump_repeat_observed = false;
                            }
                            KeyCode::Modifier(ModifierKeyCode::LeftShift)
                            | KeyCode::Modifier(ModifierKeyCode::RightShift)
                            | KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift)
                            | KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift) => {
                                sync_remote_sneak_hold(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    false,
                                    &mut sneak_held_sent,
                                );
                            }
                            _ => {}
                        },
                    }
                }
                Event::Mouse(mouse_event) => {
                    state.mouse_x = mouse_event.column;
                    state.mouse_y = mouse_event.row;
                    let (bx, by) = renderer.screen_to_world(&state, state.mouse_x, state.mouse_y);
                    if !state.inventory_open {
                        if matches!(
                            mouse_event.kind,
                            MouseEventKind::Moved | MouseEventKind::Drag(_)
                        ) {
                            pending_aim = Some((bx, by));
                        } else {
                            pending_aim = None;
                            last_aim_sent = Instant::now();
                            queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::AimAt(bx, by),
                            );
                        }
                    }
                    match mouse_event.kind {
                        MouseEventKind::ScrollDown if state.inventory_open => {
                            renderer.scroll_recipe_book(1)
                        }
                        MouseEventKind::ScrollUp if state.inventory_open => {
                            renderer.scroll_recipe_book(-1)
                        }
                        MouseEventKind::Down(button) if state.inventory_open => {
                            inventory_drag_slot = None;
                            if let Some(slot) = renderer.get_inventory_click(
                                &state,
                                mouse_event.column,
                                mouse_event.row,
                            ) {
                                let kind = if mouse_event.modifiers.contains(KeyModifiers::SHIFT) {
                                    2
                                } else if button == crossterm::event::MouseButton::Right {
                                    1
                                } else {
                                    0
                                };
                                if kind != 2 {
                                    inventory_drag_slot = Some(slot);
                                }
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::InventoryClick(slot as u16, kind),
                                );
                            } else {
                                let command = if let Some(i) = renderer.get_enchant_option_click(
                                    &state,
                                    mouse_event.column,
                                    mouse_event.row,
                                ) {
                                    Some(ClientCommand::Enchant(i as u8))
                                } else if renderer.get_anvil_action_click(
                                    &state,
                                    mouse_event.column,
                                    mouse_event.row,
                                ) {
                                    Some(ClientCommand::Anvil)
                                } else if let Some(i) = renderer.get_brewing_option_click(
                                    &state,
                                    mouse_event.column,
                                    mouse_event.row,
                                ) {
                                    Some(ClientCommand::Brew(i as u8))
                                } else {
                                    renderer
                                        .get_recipe_click(
                                            &state,
                                            mouse_event.column,
                                            mouse_event.row,
                                        )
                                        .map(|i| {
                                            if mouse_event.modifiers.contains(KeyModifiers::SHIFT) {
                                                ClientCommand::CraftRecipeMax(i as u16)
                                            } else {
                                                ClientCommand::CraftRecipe(i as u16)
                                            }
                                        })
                                };
                                if let Some(command) = command {
                                    queue_client_command(
                                        &mut prediction,
                                        &mut pending_outbound,
                                        command,
                                    );
                                }
                            }
                        }
                        MouseEventKind::Drag(button)
                            if state.inventory_open
                                && matches!(
                                    button,
                                    crossterm::event::MouseButton::Left
                                        | crossterm::event::MouseButton::Right
                                )
                                && !mouse_event.modifiers.contains(KeyModifiers::SHIFT) =>
                        {
                            if let Some(slot) = renderer.get_inventory_click(
                                &state,
                                mouse_event.column,
                                mouse_event.row,
                            ) && inventory_drag_slot != Some(slot)
                            {
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::InventoryClick(slot as u16, 3),
                                );
                                inventory_drag_slot = Some(slot);
                            }
                        }
                        MouseEventKind::Up(_) if state.inventory_open => {
                            inventory_drag_slot = None;
                        }
                        MouseEventKind::Down(crossterm::event::MouseButton::Left)
                            if !state.inventory_open =>
                        {
                            queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SetPrimaryAction(true),
                            )
                        }
                        MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                            queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::SetPrimaryAction(false),
                            )
                        }
                        MouseEventKind::Down(crossterm::event::MouseButton::Right)
                            if !state.inventory_open =>
                        {
                            queue_client_command(
                                &mut prediction,
                                &mut pending_outbound,
                                ClientCommand::UseAt(bx, by),
                            )
                        }
                        _ => {}
                    }
                    sync_remote_sneak_hold(
                        &mut prediction,
                        &mut pending_outbound,
                        mouse_event.modifiers.contains(KeyModifiers::SHIFT),
                        &mut sneak_held_sent,
                    );
                }
                Event::FocusLost => {
                    queue_client_command(
                        &mut prediction,
                        &mut pending_outbound,
                        ClientCommand::SetPrimaryAction(false),
                    );
                    if left_active || right_active {
                        left_active = false;
                        right_active = false;
                        left_last_input_at = None;
                        right_last_input_at = None;
                        left_repeat_observed = false;
                        right_repeat_observed = false;
                        queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::ClearDirectionalInput,
                        );
                    }
                    sync_remote_sneak_hold(
                        &mut prediction,
                        &mut pending_outbound,
                        false,
                        &mut sneak_held_sent,
                    );
                    if jump_held_sent {
                        queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::SetJumpHeld(false),
                        );
                    }
                    jump_held_sent = false;
                    jump_last_input_at = None;
                    jump_repeat_observed = false;
                }
                _ => {}
            }

            if let Event::Key(key_event) = ev
                && !is_shift_modifier_key(&key_event.code)
            {
                sync_remote_sneak_hold(
                    &mut prediction,
                    &mut pending_outbound,
                    key_event.modifiers.contains(KeyModifiers::SHIFT),
                    &mut sneak_held_sent,
                );
            }
        }

        if last_aim_sent.elapsed() >= CLIENT_TICK_DURATION
            && let Some((x, y)) = pending_aim.take()
        {
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                ClientCommand::AimAt(x, y),
            );
            last_aim_sent = Instant::now();
            last_heartbeat = last_aim_sent;
        }

        let now = Instant::now();
        if left_active
            && !jump_movement_grace.active(now)
            && left_last_input_at.is_none_or(|ts| {
                input_stale_timeout(
                    key_releases,
                    if left_repeat_observed {
                        REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT
                    } else {
                        REMOTE_HORIZONTAL_STALE_TIMEOUT_INITIAL
                    },
                )
                .is_some_and(|timeout| now.duration_since(ts) > timeout)
            })
        {
            left_active = false;
            left_last_input_at = None;
            left_repeat_observed = false;
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                if right_active {
                    ClientCommand::SetMoveLeft(false)
                } else {
                    ClientCommand::ClearDirectionalInput
                },
            );
        }
        if right_active
            && !jump_movement_grace.active(now)
            && right_last_input_at.is_none_or(|ts| {
                input_stale_timeout(
                    key_releases,
                    if right_repeat_observed {
                        REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT
                    } else {
                        REMOTE_HORIZONTAL_STALE_TIMEOUT_INITIAL
                    },
                )
                .is_some_and(|timeout| now.duration_since(ts) > timeout)
            })
        {
            right_active = false;
            right_last_input_at = None;
            right_repeat_observed = false;
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                if left_active {
                    ClientCommand::SetMoveRight(false)
                } else {
                    ClientCommand::ClearDirectionalInput
                },
            );
        }
        if jump_held_sent
            && jump_last_input_at.is_none_or(|ts| {
                input_stale_timeout(
                    key_releases,
                    if jump_repeat_observed {
                        REMOTE_JUMP_STALE_TIMEOUT_REPEAT
                    } else {
                        REMOTE_JUMP_STALE_TIMEOUT_INITIAL
                    },
                )
                .is_some_and(|timeout| now.duration_since(ts) > timeout)
            })
        {
            jump_held_sent = false;
            jump_last_input_at = None;
            jump_repeat_observed = false;
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                ClientCommand::SetJumpHeld(false),
            );
        }

        let mut catchup_ticks = 0u8;
        while now.duration_since(last_tick) >= CLIENT_TICK_DURATION
            && catchup_ticks < CLIENT_MAX_CATCHUP_TICKS_PER_FRAME
        {
            let tick_started = Instant::now();
            renderer.capture_motion_step(&state);
            prediction.advance_local_tick();
            if let Some(predicted) = prediction.predicted_snapshot() {
                sync_state_from_snapshot(&mut state, predicted);
            }
            state.performance.tick.record(tick_started.elapsed());
            last_tick += CLIENT_TICK_DURATION;
            catchup_ticks = catchup_ticks.saturating_add(1);
        }
        if catchup_ticks == CLIENT_MAX_CATCHUP_TICKS_PER_FRAME
            && now.duration_since(last_tick) >= CLIENT_TICK_DURATION
        {
            last_tick = now;
        }

        if now.duration_since(last_heartbeat) >= Duration::from_millis(250) {
            let (x, y) = renderer.screen_to_world(&state, state.mouse_x, state.mouse_y);
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                ClientCommand::AimAt(x, y),
            );
            last_heartbeat = now;
        }
        flush_outbound_commands(writer, &mut pending_outbound)
            .map_err(|error| connection_error(error, hint, false))?;

        apply_server_updates(&mut state, &mut prediction, server_inbox)
            .map_err(|error| connection_error(error, hint, false))?;

        if let Some(predicted) = prediction.predicted_snapshot() {
            sync_state_from_snapshot(&mut state, predicted);
        } else if let Some(authoritative) = prediction.authoritative_snapshot() {
            sync_state_from_snapshot(&mut state, authoritative);
        }

        if now >= next_frame_deadline {
            let frame_alpha = (now.duration_since(last_tick).as_secs_f64()
                / CLIENT_TICK_DURATION.as_secs_f64())
            .clamp(0.0, 1.0);
            let render_started = Instant::now();
            renderer.render(&state, frame_alpha)?;
            state.performance.render.record(render_started.elapsed());
            next_frame_deadline =
                frame_deadline(render_started, Instant::now(), CLIENT_FRAME_DURATION);
        }
        let wake_at = next_frame_deadline.min(last_tick + CLIENT_TICK_DURATION);
        event::poll(wake_at.saturating_duration_since(Instant::now()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::net::{
        ChunkColumnSnapshot, NET_PROTOCOL_VERSION, SnapshotDimension, SnapshotWeather,
    };
    use crate::world::block::BlockType;
    use crate::world::chunk::{CHUNK_HEIGHT, CHUNK_WIDTH};
    use crate::world::item::ItemType;

    #[test]
    fn catchup_installs_latest_inventory_edits_and_resets_returned_world_terrain() {
        let inbox = ServerInbox::default();
        let mut state = GameState::network_view();
        let mut prediction = ClientPredictionState::new(1);
        let mut blocks = vec![BlockType::Stone; CHUNK_WIDTH * CHUNK_HEIGHT];
        state.world.apply_chunk_column_snapshot(2, &blocks);
        for (tick, dimension, block) in [
            (1, SnapshotDimension::Overworld, BlockType::Stone),
            (2, SnapshotDimension::Nether, BlockType::Netherrack),
            (3, SnapshotDimension::Overworld, BlockType::Dirt),
        ] {
            let mut snapshot = ServerSnapshot::from_state(tick, &GameState::network_view());
            snapshot.dimension = dimension;
            inbox.push(ServerWireMessage::Snapshot(Box::new(snapshot)));
            blocks.fill(block);
            inbox.push(ServerWireMessage::ChunkDelta(ChunkDeltaPacket {
                protocol: NET_PROTOCOL_VERSION,
                tick,
                dimension,
                center_chunk_x: 0,
                chunks: vec![ChunkColumnSnapshot {
                    chunk_x: 0,
                    revision: tick,
                    biomes: Vec::new(),
                    blocks: blocks.clone(),
                }],
            }));
        }
        apply_server_updates(&mut state, &mut prediction, &inbox).unwrap();
        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!(state.world.get_block(0, 40), BlockType::Dirt);
        assert_eq!(
            state.world.get_block(2 * CHUNK_WIDTH as i32, 40),
            BlockType::Air
        );
        assert_eq!(
            prediction.replicated_chunk(0).unwrap().blocks[40 * CHUNK_WIDTH],
            BlockType::Dirt
        );
        let mut latest = GameState::network_view();
        latest.inventory.slots[0] = Some(crate::world::item::ItemStack {
            item_type: ItemType::Diamond,
            count: 17,
            durability: None,
        });
        for tick in 4..80 {
            let mut snapshot = ServerSnapshot::from_state(tick, &latest);
            snapshot.player.health = 13.0;
            snapshot.chat.lines = vec!["latest message".into()].into();
            inbox.push(ServerWireMessage::Snapshot(Box::new(snapshot)));
        }
        apply_server_updates(&mut state, &mut prediction, &inbox).unwrap();
        assert_eq!(state.inventory.slots[0].as_ref().unwrap().count, 17);
        assert_eq!(state.player.health, 13.0);
        assert_eq!(
            state.chat.lines.front().map(String::as_str),
            Some("latest message")
        );
        assert_eq!(state.performance.network_tick, 79);
        assert_eq!(state.performance.network_superseded, 75);
    }

    #[test]
    #[ignore = "requires local TCP sockets"]
    fn temporary_socket_client_writer_flushes_inputs_before_closing() {
        use crate::engine::net::read_framed_client_input;
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let writer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        writer
            .set_write_timeout(Some(SOCKET_WRITE_TIMEOUT))
            .unwrap();
        let (mut reader, _) = listener.accept().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let sender = ClientWriter::spawn(writer);
        let worker = thread::spawn(move || {
            for sequence in 0..100 {
                assert_eq!(
                    read_framed_client_input(&mut reader).unwrap().sequence,
                    sequence
                );
            }
            let mut expected = Vec::new();
            write_framed_client_chat(&mut expected, "last message").unwrap();
            let mut actual = vec![0; expected.len()];
            reader.read_exact(&mut actual).unwrap();
            assert_eq!(actual, expected);
        });
        for sequence in 0..100 {
            sender
                .send(ClientPacket::Input(ClientInputFrame::new(
                    1,
                    sequence,
                    0,
                    ClientCommand::ClearDirectionalInput,
                )))
                .unwrap();
        }
        sender
            .send(ClientPacket::Chat("last message".into()))
            .unwrap();
        sender.finish().unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn sync_state_from_snapshot_updates_player_and_dimension() {
        let mut state = GameState::new();
        let mut snapshot = ServerSnapshot::from_state(7, &GameState::new());
        snapshot.dimension = SnapshotDimension::Nether;
        snapshot.weather = SnapshotWeather::Thunderstorm;
        snapshot.time_of_day = 0.42;
        snapshot.player.x = 19.5;
        snapshot.player.y = 58.75;
        snapshot.player.vx = 0.31;
        snapshot.player.vy = -0.12;
        snapshot.player.grounded = false;
        snapshot.player.facing_right = false;
        snapshot.player.sneaking = true;
        snapshot.player.health = 14.0;
        snapshot.player.max_health = 20.0;
        snapshot.player.hunger = 9.0;
        snapshot.player.max_hunger = 20.0;
        snapshot.hotbar_index = 4;
        snapshot.selected_hotbar_item = Some(ItemType::Bow);
        let mut source = GameState::network_view();
        source.inventory.slots[4] = Some(crate::world::item::ItemStack {
            item_type: ItemType::Bow,
            count: 1,
            durability: None,
        });
        snapshot.session = crate::engine::state::PlayerSession::capture(&source);
        snapshot.inventory_open = true;
        snapshot.death_screen_active = true;

        sync_state_from_snapshot(&mut state, &snapshot);

        assert_eq!(state.current_dimension, Dimension::Nether);
        assert_eq!(
            state.weather,
            super::super::state::WeatherType::Thunderstorm
        );
        assert_eq!(state.time_of_day, 0.42);
        assert_eq!(state.player.x, 19.5);
        assert_eq!(state.player.y, 58.75);
        assert_eq!(state.player.vx, 0.31);
        assert_eq!(state.player.vy, -0.12);
        assert!(!state.player.grounded);
        assert!(!state.player.facing_right);
        assert!(state.player.sneaking);
        assert_eq!(state.player.health, 14.0);
        assert_eq!(state.player.hunger, 9.0);
        assert_eq!(state.hotbar_index, 4);
        assert!(state.inventory_open);
        assert!(state.is_showing_death_screen());
        assert_eq!(
            state.inventory.slots[4]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::Bow)
        );
    }

    #[test]
    fn apply_chunk_delta_to_state_writes_chunk_blocks() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[10 * CHUNK_WIDTH + 3] = BlockType::Stone;

        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 3,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 1,
                biomes: vec![crate::world::BiomeType::Plains; CHUNK_WIDTH],
                blocks,
            }],
        };

        apply_chunk_delta_to_state(&mut state, &delta);

        assert_eq!(state.world.get_block(3, 10), BlockType::Stone);
    }

    #[test]
    fn apply_chunk_delta_to_state_ignores_dimension_mismatch() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        state.world.set_block(3, 10, BlockType::Air);

        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[10 * CHUNK_WIDTH + 3] = BlockType::Stone;

        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 4,
            dimension: SnapshotDimension::Nether,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 1,
                biomes: vec![crate::world::BiomeType::Plains; CHUNK_WIDTH],
                blocks,
            }],
        };

        apply_chunk_delta_to_state(&mut state, &delta);
        assert_eq!(state.world.get_block(3, 10), BlockType::Air);
    }
    #[test]
    fn replica_uses_server_biomes_and_prunes_dirty_container_chunks_without_saving() {
        let mut state = GameState::network_view();
        assert!(!state.is_showing_startup_splash());
        assert_eq!(state.world.loaded_chunk_count(), 0);
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[10 * CHUNK_WIDTH + 3] = BlockType::Chest;
        let delta = ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick: 1,
            dimension: SnapshotDimension::Overworld,
            center_chunk_x: 0,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: 0,
                revision: 1,
                blocks,
                biomes: vec![crate::world::BiomeType::Desert; CHUNK_WIDTH],
            }],
        };
        apply_chunk_delta_to_state(&mut state, &delta);
        assert_eq!(state.world.get_biome(3), crate::world::BiomeType::Desert);
        state
            .world
            .ensure_chest_inventory(3, 10, 27)
            .unwrap()
            .add_item(ItemType::Diamond, 8);
        assert!(state.world.try_save_all().is_err());
        assert_eq!(state.world.trim_chunk_cache(&[640], 0).unwrap(), 1);
        assert_eq!(state.world.loaded_chunk_count(), 0);
    }
    #[test]
    fn full_snapshot_carries_inventory_counts_durability_and_live_entities() {
        let mut source = GameState::network_view();
        source.inventory.add_item(ItemType::Wood, 23);
        source.inventory.slots[1] = Some(crate::world::item::ItemStack {
            item_type: ItemType::Bow,
            count: 1,
            durability: Some(123),
        });
        source
            .zombies
            .push(crate::entities::zombie::Zombie::new(2.5, 10.0));
        let snapshot = ServerSnapshot::from_state(5, &source);
        let mut client = GameState::network_view();
        sync_state_from_snapshot(&mut client, &snapshot);
        assert_eq!(client.inventory, source.inventory);
        assert_eq!(client.zombies, source.zombies);
        assert_eq!(client.inventory.slots[0].as_ref().unwrap().count, 23);
        assert_eq!(
            client.inventory.slots[1].as_ref().unwrap().durability,
            Some(123)
        );
    }
}
