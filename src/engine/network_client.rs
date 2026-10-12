#[path = "client_identity.rs"]
mod identity;
use super::command::ClientCommand;
use super::game_loop::{confirms_control_key_releases, input_stale_timeout};
use super::net::{
    ChunkDeltaPacket, ClientHello, ClientInputFrame, ClientPredictionState, NET_PROTOCOL_VERSION,
    SERVER_TICK_RATE, ServerSnapshot, ServerWireMessage, read_framed_server_message,
    write_client_hello, write_framed_client_chat, write_framed_client_input,
};
use super::state::GameState;
use crate::renderer::Renderer;

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
// Match native input: conservative pulses until a control release is observed,
// then a bounded watchdog for missing key-up events.
const REMOTE_HORIZONTAL_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(150);
const REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(96);
const REMOTE_HORIZONTAL_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(320);
const REMOTE_JUMP_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(340);
const REMOTE_JUMP_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(160);
const REMOTE_JUMP_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(420);

fn queue_client_command(
    prediction: &mut ClientPredictionState,
    pending_outbound: &mut VecDeque<ClientInputFrame>,
    command: ClientCommand,
) {
    prediction.queue_local_command(command);
    pending_outbound.extend(prediction.take_outbound_inputs());
}

fn flush_outbound_commands(
    writer: &mut TcpStream,
    pending_outbound: &mut VecDeque<ClientInputFrame>,
) -> io::Result<()> {
    while let Some(frame) = pending_outbound.pop_front() {
        write_framed_client_input(writer, &frame)?;
    }
    Ok(())
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

fn sync_state_from_snapshot(state: &mut GameState, snapshot: &ServerSnapshot) {
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

fn apply_chunk_delta_to_state(state: &mut GameState, delta: &ChunkDeltaPacket) {
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

fn spawn_server_message_reader(
    mut reader: TcpStream,
    tx: mpsc::SyncSender<io::Result<ServerWireMessage>>,
) {
    thread::spawn(move || {
        loop {
            match read_framed_server_message(&mut reader) {
                Ok(message) => {
                    if tx.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx.send(Err(err));
                    break;
                }
            }
        }
    });
}

pub fn run_tcp_client(connect_addr: &str) -> io::Result<()> {
    run_tcp_client_with_profile(connect_addr, "player")
}

pub fn run_tcp_client_with_profile(connect_addr: &str, profile: &str) -> io::Result<()> {
    run_tcp_client_with_identity(connect_addr, profile, connect_addr)
}
pub(crate) fn run_tcp_client_with_identity(
    connect_addr: &str,
    profile: &str,
    identity_addr: &str,
) -> io::Result<()> {
    let previous = identity::read(identity_addr, profile)?;
    let mut writer = TcpStream::connect(connect_addr)?;
    writer.set_nodelay(true)?;
    writer.set_write_timeout(Some(Duration::from_millis(16)))?;
    writer.set_read_timeout(Some(Duration::from_secs(5)))?;
    write_client_hello(
        &mut writer,
        &ClientHello {
            protocol: NET_PROTOCOL_VERSION,
            token: previous.as_ref().map(|p| p.token.clone()),
        },
    )?;
    let welcome = match read_framed_server_message(&mut writer)? {
        ServerWireMessage::Welcome(w) => w,
        ServerWireMessage::Rejected(_, message) => return Err(io::Error::other(message)),
        _ => return Err(io::Error::other("server did not send a welcome")),
    };
    identity::store(identity_addr, profile, previous.as_ref(), &welcome)?;
    writer.set_read_timeout(None)?;
    let reader = writer.try_clone()?;
    reader.set_nodelay(true)?;

    let (server_msg_tx, server_msg_rx) = mpsc::sync_channel::<io::Result<ServerWireMessage>>(128);
    let mut renderer = Renderer::new()?;
    if let Err(error) = renderer.init() {
        let _ = renderer.restore();
        return Err(error);
    }

    spawn_server_message_reader(reader, server_msg_tx);
    let result = run_tcp_client_loop(
        &mut writer,
        &server_msg_rx,
        &mut renderer,
        welcome.client_id,
    );
    let _ = writer.shutdown(std::net::Shutdown::Both);
    result.and(renderer.restore())
}

fn run_tcp_client_loop(
    writer: &mut TcpStream,
    server_msg_rx: &mpsc::Receiver<io::Result<ServerWireMessage>>,
    renderer: &mut Renderer,
    client_id: u16,
) -> io::Result<()> {
    let mut state = GameState::network_view();
    clear_remote_runtime_entities(&mut state);
    state.inventory.slots.fill(None);
    state.inventory_open = false;
    state.mouse_x = 0;
    state.mouse_y = 0;

    let mut prediction = ClientPredictionState::new(client_id);
    let mut pending_outbound = VecDeque::<ClientInputFrame>::new();

    let mut left_active = false;
    let mut right_active = false;
    let mut left_last_input_at: Option<Instant> = None;
    let mut right_last_input_at: Option<Instant> = None;
    let mut left_repeat_observed = false;
    let mut right_repeat_observed = false;
    let mut jump_held_sent = false;
    let mut jump_last_input_at: Option<Instant> = None;
    let mut jump_repeat_observed = false;
    let mut sneak_held_sent = false;

    let mut last_tick = Instant::now();
    let mut next_frame_deadline = Instant::now();
    let mut running = true;
    let mut last_heartbeat = Instant::now();
    let mut key_releases = renderer.reports_key_releases();

    while running {
        while event::poll(Duration::from_millis(0))? {
            let ev = event::read()?;
            if matches!(&ev, Event::Key(key) if confirms_control_key_releases(key)) {
                key_releases = true;
                renderer.note_key_release();
            }
            if state.chat.open {
                match &ev {
                    Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                        KeyCode::Esc => state.chat.open = false,
                        KeyCode::Enter => {
                            if state.chat.mode == "host-only" {
                                state.chat.open = false;
                                continue;
                            }
                            let prompt = std::mem::take(&mut state.chat.draft);
                            flush_outbound_commands(writer, &mut pending_outbound)?;
                            write_framed_client_chat(writer, &prompt)?;
                            state.chat.open = false;
                        }
                        KeyCode::Backspace => {
                            state.chat.draft.pop();
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if state.chat.mode != "host-only" {
                                flush_outbound_commands(writer, &mut pending_outbound)?;
                                write_framed_client_chat(writer, "/cancel")?;
                            }
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
            match ev {
                Event::Key(key_event) => match key_event.kind {
                    KeyEventKind::Press => match key_event.code {
                        KeyCode::Char('t' | 'T') if !state.inventory_open => {
                            for command in [
                                ClientCommand::ClearDirectionalInput,
                                ClientCommand::SetPrimaryAction(false),
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
                            state.chat.open = true;
                        }
                        KeyCode::F(4) => renderer.toggle_zoom(),
                        KeyCode::F(3) => state.show_debug_overlay = !state.show_debug_overlay,
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if state.inventory_open {
                                queue_client_command(
                                    &mut prediction,
                                    &mut pending_outbound,
                                    ClientCommand::ToggleInventory,
                                );
                            } else {
                                running = false;
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
                        KeyCode::F(5) => queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::TravelToOverworld,
                        ),
                        KeyCode::F(6) => queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::TravelToNether,
                        ),
                        KeyCode::F(7) => queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::TravelToEnd,
                        ),
                        KeyCode::F(8) => queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::TravelToSpawn,
                        ),
                        KeyCode::F(9) => queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::EquipDiamondLoadout,
                        ),
                        KeyCode::Char(c) if c.eq_ignore_ascii_case(&'f') => {
                            if !state.inventory_open {
                                let (bx, by) =
                                    renderer.screen_to_world(&state, state.mouse_x, state.mouse_y);
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
                                now.duration_since(last) <= REMOTE_HORIZONTAL_PRESS_REPEAT_WINDOW
                            });
                            left_last_input_at = Some(now);
                            if !left_active {
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
                                now.duration_since(last) <= REMOTE_HORIZONTAL_PRESS_REPEAT_WINDOW
                            });
                            right_last_input_at = Some(now);
                            if !right_active {
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
                                if !left_active {
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
                                if !right_active {
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
                },
                Event::Mouse(mouse_event) => {
                    state.mouse_x = mouse_event.column;
                    state.mouse_y = mouse_event.row;
                    let (bx, by) = renderer.screen_to_world(&state, state.mouse_x, state.mouse_y);
                    if !state.inventory_open {
                        queue_client_command(
                            &mut prediction,
                            &mut pending_outbound,
                            ClientCommand::AimAt(bx, by),
                        );
                    }
                    match mouse_event.kind {
                        MouseEventKind::Down(button) if state.inventory_open => {
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
                                        .map(|i| ClientCommand::CraftRecipe(i as u16))
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

        let now = Instant::now();
        if left_active
            && left_last_input_at.is_none_or(|ts| {
                now.duration_since(ts)
                    > input_stale_timeout(
                        key_releases,
                        if left_repeat_observed {
                            REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT
                        } else {
                            REMOTE_HORIZONTAL_STALE_TIMEOUT_INITIAL
                        },
                    )
            })
        {
            left_active = false;
            left_last_input_at = None;
            left_repeat_observed = false;
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                ClientCommand::SetMoveLeft(false),
            );
        }
        if right_active
            && right_last_input_at.is_none_or(|ts| {
                now.duration_since(ts)
                    > input_stale_timeout(
                        key_releases,
                        if right_repeat_observed {
                            REMOTE_HORIZONTAL_STALE_TIMEOUT_REPEAT
                        } else {
                            REMOTE_HORIZONTAL_STALE_TIMEOUT_INITIAL
                        },
                    )
            })
        {
            right_active = false;
            right_last_input_at = None;
            right_repeat_observed = false;
            queue_client_command(
                &mut prediction,
                &mut pending_outbound,
                ClientCommand::SetMoveRight(false),
            );
        }
        if jump_held_sent
            && jump_last_input_at.is_none_or(|ts| {
                now.duration_since(ts)
                    > input_stale_timeout(
                        key_releases,
                        if jump_repeat_observed {
                            REMOTE_JUMP_STALE_TIMEOUT_REPEAT
                        } else {
                            REMOTE_JUMP_STALE_TIMEOUT_INITIAL
                        },
                    )
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
            renderer.capture_motion_step(&state);
            prediction.advance_local_tick();
            if let Some(predicted) = prediction.predicted_snapshot() {
                sync_state_from_snapshot(&mut state, predicted);
            }
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
        flush_outbound_commands(writer, &mut pending_outbound)?;

        loop {
            match server_msg_rx.try_recv() {
                Ok(Ok(message)) => {
                    prediction.apply_server_message(message.clone());
                    match message {
                        ServerWireMessage::Welcome(_) => {
                            return Err(io::Error::other("unexpected second welcome"));
                        }
                        ServerWireMessage::Rejected(_, message) => {
                            return Err(io::Error::other(message));
                        }
                        ServerWireMessage::Snapshot(snapshot) => {
                            sync_state_from_snapshot(&mut state, &snapshot);
                        }
                        ServerWireMessage::ChunkDelta(delta) => {
                            apply_chunk_delta_to_state(&mut state, &delta);
                        }
                    }
                }
                Ok(Err(err)) => return Err(err),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "server reader disconnected",
                    ));
                }
            }
        }

        if let Some(predicted) = prediction.predicted_snapshot() {
            sync_state_from_snapshot(&mut state, predicted);
        } else if let Some(authoritative) = prediction.authoritative_snapshot() {
            sync_state_from_snapshot(&mut state, authoritative);
        }

        if now >= next_frame_deadline {
            let frame_alpha = (now.duration_since(last_tick).as_secs_f64()
                / CLIENT_TICK_DURATION.as_secs_f64())
            .clamp(0.0, 1.0);
            renderer.render(&state, frame_alpha)?;
            next_frame_deadline = Instant::now() + CLIENT_FRAME_DURATION;
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
