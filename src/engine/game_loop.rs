use super::command::ClientCommand;
use super::state::GameState;
use crate::renderer::Renderer;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseButton, MouseEventKind,
};
use std::time::{Duration, Instant};

const TICKS_PER_SECOND: u64 = 20;
const TICK_DURATION: Duration = Duration::from_millis(1000 / TICKS_PER_SECOND);
const MAX_CATCHUP_TICKS_PER_FRAME: u8 = 5;
// Legacy terminals cannot distinguish a tap from a hold before OS repeat starts.
// These bounded fallback pulses favor stopping after taps. Once real key-up
// events arrive, hold state must survive OS repeat moving to a second key.
const HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(150);
const HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(96);
const HORIZONTAL_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(320);
const JUMP_INPUT_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(340);
const JUMP_INPUT_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(160);
const JUMP_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(420);
const LEGACY_JUMP_MOVEMENT_GRACE: Duration = Duration::from_millis(750);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(45);
const AUTOSAVE_CHUNK_BUDGET_PER_FRAME: usize = 1;
const TARGET_FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TARGET_FPS);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HorizontalDirection {
    Left,
    Right,
}

pub(super) fn confirms_control_key_releases(key: &crossterm::event::KeyEvent) -> bool {
    key.kind == KeyEventKind::Release
        && (GameLoop::horizontal_direction_from_key(&key.code).is_some()
            || GameLoop::is_jump_key(&key.code))
}

pub(super) fn input_stale_timeout(
    key_releases: bool,
    legacy_timeout: Duration,
) -> Option<Duration> {
    (!key_releases).then_some(legacy_timeout)
}

pub(super) fn frame_deadline(started: Instant, finished: Instant, budget: Duration) -> Instant {
    // Render/flush time consumes this frame's budget. A slow terminal must not
    // incur another full sleep, or accumulate debt for a catch-up paint burst.
    (started + budget).max(finished)
}

#[derive(Default)]
pub(super) struct LegacyJumpGrace {
    until: Option<Instant>,
    used: bool,
}

impl LegacyJumpGrace {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn begin(&mut self, now: Instant, repeating_movement: bool) {
        // Only directional input can re-arm this. Space repeats cannot renew it,
        // and an old tap is never revived after its directional state expires.
        if repeating_movement && !self.used {
            self.until = Some(now + LEGACY_JUMP_MOVEMENT_GRACE);
            self.used = true;
        }
    }

    pub(super) fn active(&self, now: Instant) -> bool {
        self.until.is_some_and(|until| now <= until)
    }

    pub(super) fn directional_event(&mut self) {
        // Real direction input confirms movement again, even just before the
        // prior grace expires. Preserve that grace, but allow the next jump.
        self.used = false;
    }
}

pub struct GameLoop {
    state: GameState,
    director: super::director::Director,
    left_last_input_at: Option<Instant>,
    right_last_input_at: Option<Instant>,
    left_repeat_observed: bool,
    right_repeat_observed: bool,
    jump_last_input_at: Option<Instant>,
    jump_repeat_observed: bool,
    jump_held_active: bool,
    key_releases: bool,
    jump_movement_grace: LegacyJumpGrace,
    last_inventory_left_drag_slot: Option<usize>,
    last_inventory_right_drag_slot: Option<usize>,
    last_autosave_at: Instant,
    autosave_in_progress: bool,
    cache_retry_after: Instant,
}

impl Default for GameLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLoop {
    fn hovered_block(&self, renderer: &Renderer) -> (i32, i32) {
        renderer.screen_to_world(&self.state, self.state.mouse_x, self.state.mouse_y)
    }

    fn use_hovered_block(&mut self, renderer: &Renderer) {
        let (bx, by) = self.hovered_block(renderer);
        self.state.interact_block(bx, by, false);
    }

    pub fn new() -> Self {
        Self::with_state(GameState::new())
    }

    pub fn with_state(mut state: GameState) -> Self {
        let director = super::director::Director::new(&mut state);
        Self {
            state,
            director,
            left_last_input_at: None,
            right_last_input_at: None,
            left_repeat_observed: false,
            right_repeat_observed: false,
            jump_last_input_at: None,
            jump_repeat_observed: false,
            jump_held_active: false,
            key_releases: false,
            jump_movement_grace: LegacyJumpGrace::default(),
            last_inventory_left_drag_slot: None,
            last_inventory_right_drag_slot: None,
            last_autosave_at: Instant::now(),
            autosave_in_progress: false,
            cache_retry_after: Instant::now(),
        }
    }

    fn persist_session(&mut self) -> std::io::Result<()> {
        self.state.try_persist_world_and_progress()?;
        self.director
            .save(&self.state)
            .map_err(std::io::Error::other)
    }

    pub fn run(&mut self, renderer: &mut Renderer) -> std::io::Result<()> {
        let result = self.run_inner(renderer);
        if let Err(error) = result {
            let root = std::env::temp_dir().join(format!(
                "termcraft-recovery-{}",
                super::server_save::token()
            ));
            let recovery = self.state.write_recovery_directory(&root).and_then(|()| {
                self.director
                    .write_recovery(&root)
                    .map_err(std::io::Error::other)
            });
            return Err(std::io::Error::other(match recovery {
                Ok(()) => format!(
                    "{error}. Partial recovery of loaded chunks and player state saved to {}. Original files preserved; read RECOVERY.txt before restoring.",
                    root.display()
                ),
                Err(recovery) => format!("{error}. Recovery snapshot also failed: {recovery}"),
            }));
        }
        Ok(())
    }

    fn run_inner(&mut self, renderer: &mut Renderer) -> std::io::Result<()> {
        self.key_releases = renderer.reports_key_releases();
        let mut last_tick = Instant::now();
        let mut next_frame_deadline = Instant::now();
        let mut running = true;

        while running {
            self.state.world.check_storage()?;
            self.director.poll(&mut self.state, Instant::now());
            // Handle Input (Non-blocking)
            while event::poll(Duration::from_millis(0))? {
                let ev = event::read()?;
                if matches!(&ev, Event::Key(key) if confirms_control_key_releases(key)) {
                    self.key_releases = true;
                    renderer.note_key_release();
                }
                match ev {
                    Event::Key(key_event) => {
                        if self.state.is_showing_startup_splash() {
                            if key_event.kind == KeyEventKind::Press {
                                match key_event.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        self.persist_session()?;
                                        running = false;
                                    }
                                    _ => self.state.dismiss_startup_splash(),
                                }
                            }
                            continue;
                        }
                        if self.state.is_showing_death_screen() {
                            if key_event.kind == KeyEventKind::Press {
                                match key_event.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        self.persist_session()?;
                                        running = false;
                                    }
                                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('r')
                                        if self.state.can_respawn_from_death_screen() =>
                                    {
                                        self.state.apply_client_command(
                                            ClientCommand::RespawnFromDeathScreen,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                            continue;
                        }
                        if self.state.is_showing_credits() {
                            if key_event.kind == KeyEventKind::Press {
                                match key_event.code {
                                    KeyCode::Char('q') => {
                                        self.persist_session()?;
                                        running = false;
                                    }
                                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                                        self.state.apply_client_command(
                                            ClientCommand::SkipCompletionCredits,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                            continue;
                        }
                        if self.state.is_settings_menu_open() {
                            match key_event.kind {
                                KeyEventKind::Press | KeyEventKind::Repeat => {
                                    match key_event.code {
                                        KeyCode::Char(c)
                                            if c.eq_ignore_ascii_case(&'q')
                                                || c.eq_ignore_ascii_case(&'o') =>
                                        {
                                            if key_event.kind == KeyEventKind::Press {
                                                self.state.apply_client_command(
                                                    ClientCommand::ToggleSettingsMenu,
                                                );
                                                self.clear_horizontal_input_state();
                                            }
                                        }
                                        KeyCode::Esc => {
                                            if key_event.kind == KeyEventKind::Press {
                                                self.state.apply_client_command(
                                                    ClientCommand::ToggleSettingsMenu,
                                                );
                                                self.clear_horizontal_input_state();
                                            }
                                        }
                                        KeyCode::Up => {
                                            self.state.apply_client_command(
                                                ClientCommand::SettingsMoveUp,
                                            );
                                        }
                                        KeyCode::Down => {
                                            self.state.apply_client_command(
                                                ClientCommand::SettingsMoveDown,
                                            );
                                        }
                                        KeyCode::Char(c) if c.eq_ignore_ascii_case(&'w') => {
                                            self.state.apply_client_command(
                                                ClientCommand::SettingsMoveUp,
                                            );
                                        }
                                        KeyCode::Char(c) if c.eq_ignore_ascii_case(&'s') => {
                                            self.state.apply_client_command(
                                                ClientCommand::SettingsMoveDown,
                                            );
                                        }
                                        KeyCode::Enter | KeyCode::Char(' ')
                                            if key_event.kind == KeyEventKind::Press =>
                                        {
                                            self.state
                                                .apply_client_command(ClientCommand::SettingsApply);
                                        }
                                        _ => {}
                                    }
                                }
                                KeyEventKind::Release => {}
                            }
                            continue;
                        }
                        if self.state.chat.open {
                            if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                            {
                                match key_event.code {
                                    KeyCode::Esc => self.state.chat.open = false,
                                    KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                                        self.state.chat.open = false;
                                        let request = std::mem::take(&mut self.state.chat.draft);
                                        self.director.submit(&mut self.state, request);
                                    }
                                    KeyCode::Backspace => {
                                        self.state.chat.draft.pop();
                                    }
                                    KeyCode::Char('c')
                                        if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                                    {
                                        self.director.cancel(&mut self.state)
                                    }
                                    KeyCode::Char(c)
                                        if !key_event.modifiers.intersects(
                                            KeyModifiers::CONTROL | KeyModifiers::ALT,
                                        ) =>
                                    {
                                        self.state.chat.insert(&c.to_string())
                                    }
                                    _ => {}
                                }
                            }
                            continue;
                        }
                        if self.handle_inventory_shortcut_key(&key_event) {
                            continue;
                        }
                        match key_event.kind {
                            KeyEventKind::Press => {
                                match key_event.code {
                                    KeyCode::F(4) => renderer.toggle_zoom(),
                                    KeyCode::Char('+' | '=') => renderer.zoom_in(),
                                    KeyCode::Char('-') => renderer.zoom_out(),
                                    KeyCode::F(3) => {
                                        self.state.show_debug_overlay =
                                            !self.state.show_debug_overlay;
                                    }
                                    KeyCode::Char('t' | 'T') if !self.state.inventory_open => {
                                        self.clear_horizontal_input_state();
                                        self.clear_jump_hold_state();
                                        self.state.apply_client_command(
                                            ClientCommand::SetPrimaryAction(false),
                                        );
                                        self.state.apply_client_command(
                                            ClientCommand::SetSneakHeld(false),
                                        );
                                        self.state.chat.open = true;
                                    }
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        if self.state.inventory_open {
                                            self.state.apply_client_command(
                                                ClientCommand::ToggleInventory,
                                            );
                                        } else {
                                            self.persist_session()?;
                                            running = false;
                                        }
                                    }
                                    KeyCode::Char('e') => {
                                        self.state
                                            .apply_client_command(ClientCommand::ToggleInventory);
                                        if self.state.inventory_open {
                                            self.clear_horizontal_input_state();
                                        }
                                    }
                                    KeyCode::Up | KeyCode::Char(' ') => {
                                        self.handle_jump_press(Instant::now(), false);
                                    }
                                    KeyCode::Char(c) if c.eq_ignore_ascii_case(&'w') => {
                                        self.handle_jump_press(Instant::now(), false);
                                    }
                                    KeyCode::F(5) => {
                                        self.state
                                            .apply_client_command(ClientCommand::TravelToOverworld);
                                    }
                                    KeyCode::F(6) => {
                                        self.state
                                            .apply_client_command(ClientCommand::TravelToNether);
                                    }
                                    KeyCode::F(7) => {
                                        self.state.apply_client_command(ClientCommand::TravelToEnd);
                                    }
                                    KeyCode::F(8) => {
                                        self.state
                                            .apply_client_command(ClientCommand::TravelToSpawn);
                                    }
                                    KeyCode::F(9) => {
                                        self.state.apply_client_command(
                                            ClientCommand::EquipDiamondLoadout,
                                        );
                                    }
                                    KeyCode::Char(c) if c.eq_ignore_ascii_case(&'f') => {
                                        if !self.state.inventory_open {
                                            self.use_hovered_block(renderer);
                                        }
                                    }
                                    KeyCode::Char('m') => {
                                        self.state.apply_client_command(
                                            ClientCommand::CycleMovementProfile,
                                        );
                                    }
                                    KeyCode::Char('p') => {
                                        self.state
                                            .apply_client_command(ClientCommand::CycleDifficulty);
                                    }
                                    KeyCode::Char('g') => {
                                        self.state.apply_client_command(
                                            ClientCommand::CycleGameRulesPreset,
                                        );
                                    }
                                    KeyCode::Char('h') => {
                                        self.state.apply_client_command(
                                            ClientCommand::ToggleRuleMobSpawning,
                                        );
                                    }
                                    KeyCode::Char('j') => {
                                        self.state.apply_client_command(
                                            ClientCommand::ToggleRuleDaylightCycle,
                                        );
                                    }
                                    KeyCode::Char('k') => {
                                        self.state.apply_client_command(
                                            ClientCommand::ToggleRuleWeatherCycle,
                                        );
                                    }
                                    KeyCode::Char('l') => {
                                        self.state.apply_client_command(
                                            ClientCommand::ToggleRuleKeepInventory,
                                        );
                                    }
                                    KeyCode::Char(c) if c.eq_ignore_ascii_case(&'o') => {
                                        self.state.apply_client_command(
                                            ClientCommand::ToggleSettingsMenu,
                                        );
                                        self.clear_horizontal_input_state();
                                    }
                                    KeyCode::Char('x') => {
                                        self.state.apply_client_command(ClientCommand::ToggleSneak);
                                    }
                                    KeyCode::Char('1') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(0)),
                                    KeyCode::Char('2') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(1)),
                                    KeyCode::Char('3') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(2)),
                                    KeyCode::Char('4') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(3)),
                                    KeyCode::Char('5') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(4)),
                                    KeyCode::Char('6') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(5)),
                                    KeyCode::Char('7') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(6)),
                                    KeyCode::Char('8') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(7)),
                                    KeyCode::Char('9') => self
                                        .state
                                        .apply_client_command(ClientCommand::SelectHotbarSlot(8)),
                                    KeyCode::Modifier(ModifierKeyCode::LeftShift)
                                    | KeyCode::Modifier(ModifierKeyCode::RightShift)
                                    | KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift)
                                    | KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift) => {
                                        self.state.apply_client_command(
                                            ClientCommand::SetSneakHeld(true),
                                        );
                                    }
                                    _ => {}
                                }
                                if !self.state.inventory_open
                                    && let Some(direction) =
                                        Self::horizontal_direction_from_key(&key_event.code)
                                {
                                    self.handle_horizontal_press(direction, Instant::now(), false);
                                }
                                if !Self::is_shift_modifier_key(&key_event.code) {
                                    self.sync_sneak_hold_from_modifiers(key_event.modifiers);
                                }
                            }
                            KeyEventKind::Repeat => {
                                if Self::is_jump_key(&key_event.code) {
                                    self.handle_jump_hold_repeat(Instant::now());
                                }
                                if !self.state.inventory_open
                                    && let Some(direction) =
                                        Self::horizontal_direction_from_key(&key_event.code)
                                {
                                    self.handle_horizontal_press(direction, Instant::now(), true);
                                }
                                if !Self::is_shift_modifier_key(&key_event.code) {
                                    self.sync_sneak_hold_from_modifiers(key_event.modifiers);
                                }
                            }
                            KeyEventKind::Release => {
                                if let Some(direction) =
                                    Self::horizontal_direction_from_key(&key_event.code)
                                {
                                    self.handle_horizontal_release(direction);
                                } else if Self::is_jump_key(&key_event.code) {
                                    self.clear_jump_hold_state();
                                } else if key_event.code == KeyCode::Null {
                                    // Some terminals report key-up with `Null` code even when
                                    // the original key was WASD/arrow. Treat it as a safe
                                    // "release movement keys" fallback.
                                    self.clear_horizontal_input_state();
                                    self.clear_jump_hold_state();
                                }
                                if Self::is_shift_modifier_key(&key_event.code) {
                                    self.state
                                        .apply_client_command(ClientCommand::SetSneakHeld(false));
                                } else {
                                    self.sync_sneak_hold_from_modifiers(key_event.modifiers);
                                }
                            }
                        }
                    }
                    Event::Paste(text) if self.state.chat.open => self.state.chat.insert(&text),
                    Event::Mouse(mouse_event) => {
                        if self.state.chat.open {
                            continue;
                        }
                        if self.state.is_showing_startup_splash() {
                            self.state.mouse_x = mouse_event.column;
                            self.state.mouse_y = mouse_event.row;
                            if matches!(
                                mouse_event.kind,
                                MouseEventKind::Down(_)
                                    | MouseEventKind::Drag(_)
                                    | MouseEventKind::Up(_)
                            ) {
                                self.state.dismiss_startup_splash();
                            }
                            continue;
                        }
                        if self.state.is_showing_credits()
                            || self.state.is_showing_death_screen()
                            || self.state.is_settings_menu_open()
                        {
                            continue;
                        }
                        self.state.mouse_x = mouse_event.column;
                        self.state.mouse_y = mouse_event.row;

                        match mouse_event.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                self.last_inventory_left_drag_slot = None;
                                self.last_inventory_right_drag_slot = None;
                                if self.state.inventory_open {
                                    if let Some(slot_idx) = renderer.get_inventory_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        if mouse_event.modifiers.contains(KeyModifiers::SHIFT) {
                                            self.state.handle_inventory_shift_click(slot_idx);
                                        } else {
                                            self.state.handle_inventory_click(slot_idx);
                                            self.last_inventory_left_drag_slot = Some(slot_idx);
                                        }
                                    } else if let Some(option_idx) = renderer
                                        .get_enchant_option_click(
                                            &self.state,
                                            mouse_event.column,
                                            mouse_event.row,
                                        )
                                    {
                                        self.state.attempt_enchant_option(option_idx);
                                    } else if renderer.get_anvil_action_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        self.state.attempt_anvil_combine();
                                    } else if let Some(option_idx) = renderer
                                        .get_brewing_option_click(
                                            &self.state,
                                            mouse_event.column,
                                            mouse_event.row,
                                        )
                                    {
                                        self.state.attempt_brew_option(option_idx);
                                    } else if let Some(recipe_idx) = renderer.get_recipe_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        self.state.attempt_craft(recipe_idx);
                                    }
                                } else {
                                    self.state.apply_client_command(
                                        ClientCommand::SetPrimaryAction(true),
                                    );
                                    let (bx, by) = renderer.screen_to_world(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    );
                                    self.state.interact_block(bx, by, true);
                                }
                            }
                            MouseEventKind::Drag(MouseButton::Left) => {
                                self.last_inventory_right_drag_slot = None;
                                if self.state.inventory_open {
                                    if let Some(slot_idx) = renderer.get_inventory_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        if self.last_inventory_left_drag_slot != Some(slot_idx) {
                                            self.state.handle_inventory_drag_place(slot_idx);
                                            self.last_inventory_left_drag_slot = Some(slot_idx);
                                        }
                                    } else {
                                        self.last_inventory_left_drag_slot = None;
                                    }
                                } else {
                                    self.state.apply_client_command(
                                        ClientCommand::SetPrimaryAction(true),
                                    );
                                    let (bx, by) = renderer.screen_to_world(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    );
                                    self.state.interact_block(bx, by, true);
                                }
                            }
                            MouseEventKind::Down(MouseButton::Right) => {
                                self.last_inventory_left_drag_slot = None;
                                if self.state.inventory_open {
                                    if let Some(slot_idx) = renderer.get_inventory_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        self.state.handle_inventory_right_click(slot_idx);
                                        self.last_inventory_right_drag_slot = Some(slot_idx);
                                    } else {
                                        self.last_inventory_right_drag_slot = None;
                                    }
                                } else {
                                    let (bx, by) = renderer.screen_to_world(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    );
                                    self.state.interact_block(bx, by, false);
                                    self.last_inventory_right_drag_slot = None;
                                }
                            }
                            MouseEventKind::Drag(MouseButton::Right) => {
                                if self.state.inventory_open
                                    && let Some(slot_idx) = renderer.get_inventory_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    )
                                    && self.last_inventory_right_drag_slot != Some(slot_idx)
                                {
                                    self.state.handle_inventory_right_click(slot_idx);
                                    self.last_inventory_right_drag_slot = Some(slot_idx);
                                }
                            }
                            MouseEventKind::Up(MouseButton::Left) => {
                                self.last_inventory_left_drag_slot = None;
                                self.state
                                    .apply_client_command(ClientCommand::SetPrimaryAction(false));
                            }
                            MouseEventKind::Up(MouseButton::Right) => {
                                self.last_inventory_right_drag_slot = None;
                            }
                            _ => {}
                        }
                        self.sync_sneak_hold_from_modifiers(mouse_event.modifiers);
                    }
                    Event::FocusLost => {
                        self.clear_horizontal_input_state();
                        self.state
                            .apply_client_command(ClientCommand::SetPrimaryAction(false));
                        self.state
                            .apply_client_command(ClientCommand::SetSneakHeld(false));
                        self.clear_jump_hold_state();
                        self.last_inventory_right_drag_slot = None;
                    }
                    _ => {}
                }
            }
            let now = Instant::now();
            self.clear_stale_horizontal_input(now);
            self.clear_stale_jump_input(now);
            let save_started = Instant::now();
            self.maybe_autosave(now);
            if now >= self.cache_retry_after {
                let mut viewers = vec![self.state.player.x.floor() as i32];
                viewers.extend(self.state.remote_players.iter().map(|p| p.x.floor() as i32));
                if let Err(error) = self.state.world.trim_chunk_cache(&viewers, 1) {
                    self.state.save_error.replace(Some(format!(
                        "Chunk save failed; keeping terrain in memory: {error}"
                    )));
                    self.cache_retry_after = now + Duration::from_secs(5);
                }
            }
            self.state
                .performance
                .save_and_cache
                .record(save_started.elapsed());
            let now = Instant::now();

            // Fixed-step update with bounded catch-up keeps motion cadence stable.
            let mut catchup_ticks = 0u8;
            while now.duration_since(last_tick) >= TICK_DURATION
                && catchup_ticks < MAX_CATCHUP_TICKS_PER_FRAME
            {
                let (bx, by) =
                    renderer.screen_to_world(&self.state, self.state.mouse_x, self.state.mouse_y);
                renderer.capture_motion_step(&self.state);
                self.update_logic(bx, by);
                self.state.world.check_storage()?;
                last_tick += TICK_DURATION;
                catchup_ticks = catchup_ticks.saturating_add(1);
            }
            if catchup_ticks == MAX_CATCHUP_TICKS_PER_FRAME
                && now.duration_since(last_tick) >= TICK_DURATION
            {
                // Drop excess lag rather than spiraling under heavy terminal load.
                last_tick = now;
                self.state.performance.dropped_catchups += 1;
            }

            // Render once per frame budget to stabilize terminal pacing.
            if now >= next_frame_deadline {
                let frame_alpha = (now.duration_since(last_tick).as_secs_f64()
                    / TICK_DURATION.as_secs_f64())
                .clamp(0.0, 1.0);
                let render_started = Instant::now();
                renderer.render(&self.state, frame_alpha)?;
                self.state
                    .performance
                    .render
                    .record(render_started.elapsed());
                next_frame_deadline =
                    frame_deadline(render_started, Instant::now(), FRAME_DURATION);
            }
            // poll wakes for input without consuming it. Input is processed immediately,
            // while rendering stays bounded and simulation retains its fixed 20 Hz step.
            let wake_at = next_frame_deadline.min(last_tick + TICK_DURATION);
            event::poll(wake_at.saturating_duration_since(Instant::now()))?;
        }

        Ok(())
    }

    fn update_logic(&mut self, target_bx: i32, target_by: i32) {
        let started = Instant::now();
        self.state.update(target_bx, target_by);
        self.state.performance.tick.record(started.elapsed());
    }

    fn maybe_autosave(&mut self, now: Instant) {
        if !self.autosave_in_progress
            && now.duration_since(self.last_autosave_at) < AUTOSAVE_INTERVAL
        {
            return;
        }
        let result = self
            .state
            .world
            .try_save_dirty_chunk_budget(AUTOSAVE_CHUNK_BUDGET_PER_FRAME)
            .and_then(|pending| {
                if !pending {
                    self.state.try_save_progression()?;
                }
                Ok(pending)
            });
        match result {
            Ok(pending) => {
                self.autosave_in_progress = pending;
                if !pending {
                    self.state.save_error.replace(None);
                    self.last_autosave_at = now;
                }
            }
            Err(error) => {
                self.state
                    .save_error
                    .replace(Some(format!("Autosave failed; retrying: {error}")));
                self.autosave_in_progress = false;
                self.last_autosave_at = now;
            }
        }
    }

    fn sync_sneak_hold_from_modifiers(&mut self, modifiers: KeyModifiers) {
        self.state.apply_client_command(ClientCommand::SetSneakHeld(
            modifiers.contains(KeyModifiers::SHIFT),
        ));
    }

    fn handle_inventory_shortcut_key(&mut self, key_event: &crossterm::event::KeyEvent) -> bool {
        if !self.state.inventory_open || key_event.kind != KeyEventKind::Press {
            return false;
        }

        match key_event.code {
            KeyCode::Enter => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    self.state.attempt_craft_from_grid_max();
                } else {
                    self.state.attempt_craft_from_grid();
                }
                true
            }
            KeyCode::Backspace | KeyCode::Delete => {
                self.state.clear_active_crafting_grid();
                true
            }
            _ => false,
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

    fn is_jump_key(code: &KeyCode) -> bool {
        matches!(code, KeyCode::Up | KeyCode::Char(' '))
            || matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'w'))
    }

    fn horizontal_direction_from_key(code: &KeyCode) -> Option<HorizontalDirection> {
        match code {
            KeyCode::Left => Some(HorizontalDirection::Left),
            KeyCode::Right => Some(HorizontalDirection::Right),
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'a') => Some(HorizontalDirection::Left),
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'d') => Some(HorizontalDirection::Right),
            _ => None,
        }
    }

    fn handle_horizontal_press(
        &mut self,
        direction: HorizontalDirection,
        now: Instant,
        explicit_repeat: bool,
    ) {
        let active = match direction {
            HorizontalDirection::Left => self.state.moving_left,
            HorizontalDirection::Right => self.state.moving_right,
        };
        if !active {
            self.jump_movement_grace.reset();
        } else {
            self.jump_movement_grace.directional_event();
        }
        let last_for_direction = self.direction_last_input_at(direction);
        let repeat_like_press = last_for_direction
            .is_some_and(|ts| now.duration_since(ts) <= HORIZONTAL_PRESS_REPEAT_WINDOW);
        let repeat_observed = explicit_repeat || repeat_like_press;
        self.set_direction_repeat_observed(direction, repeat_observed);
        self.set_direction_last_input_at(direction, Some(now));
        match direction {
            HorizontalDirection::Left => self
                .state
                .apply_client_command(ClientCommand::SetMoveLeft(true)),
            HorizontalDirection::Right => self
                .state
                .apply_client_command(ClientCommand::SetMoveRight(true)),
        }
    }

    fn handle_horizontal_release(&mut self, direction: HorizontalDirection) {
        match direction {
            HorizontalDirection::Left => self
                .state
                .apply_client_command(ClientCommand::SetMoveLeft(false)),
            HorizontalDirection::Right => self
                .state
                .apply_client_command(ClientCommand::SetMoveRight(false)),
        }
        self.set_direction_last_input_at(direction, None);
        self.set_direction_repeat_observed(direction, false);
    }

    fn handle_jump_press(&mut self, now: Instant, explicit_repeat: bool) {
        if !self.key_releases && !explicit_repeat && !self.jump_held_active {
            self.jump_movement_grace.begin(
                now,
                (self.state.moving_left
                    && self.left_repeat_observed
                    && self.left_last_input_at.is_some_and(|at| {
                        now.duration_since(at) <= HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT
                    }))
                    || (self.state.moving_right
                        && self.right_repeat_observed
                        && self.right_last_input_at.is_some_and(|at| {
                            now.duration_since(at) <= HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT
                        })),
            );
        }
        let repeat_like_press = self
            .jump_last_input_at
            .is_some_and(|ts| now.duration_since(ts) <= JUMP_PRESS_REPEAT_WINDOW);
        self.jump_repeat_observed = explicit_repeat || repeat_like_press;
        self.jump_last_input_at = Some(now);
        if !self.jump_held_active {
            self.state
                .apply_client_command(ClientCommand::SetJumpHeld(true));
            self.jump_held_active = true;
        }
        if !explicit_repeat {
            self.state.apply_client_command(ClientCommand::QueueJump);
        }
    }

    fn handle_jump_hold_repeat(&mut self, now: Instant) {
        self.handle_jump_press(now, true);
    }

    fn clear_jump_hold_state(&mut self) {
        if self.jump_held_active {
            self.state
                .apply_client_command(ClientCommand::SetJumpHeld(false));
        }
        self.jump_held_active = false;
        self.jump_last_input_at = None;
        self.jump_repeat_observed = false;
    }

    fn clear_horizontal_input_state(&mut self) {
        self.jump_movement_grace.reset();
        self.state
            .apply_client_command(ClientCommand::ClearDirectionalInput);
        self.left_last_input_at = None;
        self.right_last_input_at = None;
        self.left_repeat_observed = false;
        self.right_repeat_observed = false;
    }

    fn jump_stale_timeout(&self) -> Duration {
        if self.jump_repeat_observed {
            JUMP_INPUT_STALE_TIMEOUT_REPEAT
        } else {
            JUMP_INPUT_STALE_TIMEOUT_INITIAL
        }
    }

    fn direction_last_input_at(&self, direction: HorizontalDirection) -> Option<Instant> {
        match direction {
            HorizontalDirection::Left => self.left_last_input_at,
            HorizontalDirection::Right => self.right_last_input_at,
        }
    }

    fn set_direction_last_input_at(
        &mut self,
        direction: HorizontalDirection,
        value: Option<Instant>,
    ) {
        match direction {
            HorizontalDirection::Left => self.left_last_input_at = value,
            HorizontalDirection::Right => self.right_last_input_at = value,
        }
    }

    fn direction_repeat_observed(&self, direction: HorizontalDirection) -> bool {
        match direction {
            HorizontalDirection::Left => self.left_repeat_observed,
            HorizontalDirection::Right => self.right_repeat_observed,
        }
    }

    fn set_direction_repeat_observed(&mut self, direction: HorizontalDirection, value: bool) {
        match direction {
            HorizontalDirection::Left => self.left_repeat_observed = value,
            HorizontalDirection::Right => self.right_repeat_observed = value,
        }
    }

    fn stale_timeout_for_direction(&self, direction: HorizontalDirection) -> Option<Duration> {
        let legacy_timeout = if self.direction_repeat_observed(direction) {
            HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT
        } else {
            HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL
        };
        input_stale_timeout(self.key_releases, legacy_timeout)
    }

    fn clear_stale_direction(&mut self, direction: HorizontalDirection) -> bool {
        match direction {
            HorizontalDirection::Left => {
                if !self.state.moving_left {
                    return false;
                }
                self.state.apply_client_command(if self.state.moving_right {
                    ClientCommand::SetMoveLeft(false)
                } else {
                    ClientCommand::ClearDirectionalInput
                });
                self.left_last_input_at = None;
                self.left_repeat_observed = false;
                true
            }
            HorizontalDirection::Right => {
                if !self.state.moving_right {
                    return false;
                }
                self.state.apply_client_command(if self.state.moving_left {
                    ClientCommand::SetMoveRight(false)
                } else {
                    ClientCommand::ClearDirectionalInput
                });
                self.right_last_input_at = None;
                self.right_repeat_observed = false;
                true
            }
        }
    }

    fn clear_stale_horizontal_input(&mut self, now: Instant) {
        if self.jump_movement_grace.active(now) {
            return;
        }
        let left_stale = self.state.moving_left
            && self.left_last_input_at.is_none_or(|ts| {
                self.stale_timeout_for_direction(HorizontalDirection::Left)
                    .is_some_and(|timeout| now.duration_since(ts) > timeout)
            });
        let right_stale = self.state.moving_right
            && self.right_last_input_at.is_none_or(|ts| {
                self.stale_timeout_for_direction(HorizontalDirection::Right)
                    .is_some_and(|timeout| now.duration_since(ts) > timeout)
            });

        if left_stale {
            self.clear_stale_direction(HorizontalDirection::Left);
        }
        if right_stale {
            self.clear_stale_direction(HorizontalDirection::Right);
        }
    }

    fn clear_stale_jump_input(&mut self, now: Instant) {
        if !self.jump_held_active {
            return;
        }
        if self.jump_last_input_at.is_none_or(|ts| {
            input_stale_timeout(self.key_releases, self.jump_stale_timeout())
                .is_some_and(|timeout| now.duration_since(ts) > timeout)
        }) {
            self.clear_jump_hold_state();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_consumes_the_frame_budget_without_extra_sleep_or_catchup_debt() {
        let start = Instant::now();
        assert_eq!(
            frame_deadline(start, start + Duration::from_millis(8), FRAME_DURATION),
            start + FRAME_DURATION
        );
        let slow = start + Duration::from_millis(40);
        assert_eq!(frame_deadline(start, slow, FRAME_DURATION), slow);
        let next_slow = slow + Duration::from_millis(40);
        assert_eq!(frame_deadline(slow, next_slow, FRAME_DURATION), next_slow);
    }

    #[test]
    fn release_reporting_terminal_keeps_held_movement_and_jump_until_key_up() {
        let mut game = GameLoop::new();
        game.key_releases =
            confirms_control_key_releases(&crossterm::event::KeyEvent::new_with_kind(
                KeyCode::Char('d'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ));
        let now = Instant::now();
        game.handle_horizontal_press(HorizontalDirection::Right, now, false);
        game.handle_jump_press(now, false);
        // Includes the normal OS initial-repeat gap and a long uninterrupted hold.
        for delay in [160, 500, 1500, 2500, 10000] {
            game.clear_stale_horizontal_input(now + Duration::from_millis(delay));
            game.clear_stale_jump_input(now + Duration::from_millis(delay));
            assert!(game.state.moving_right);
            assert!(game.jump_held_active);
        }
        game.handle_horizontal_release(HorizontalDirection::Right);
        game.clear_jump_hold_state();
        assert!(!game.state.moving_right);
        assert!(!game.jump_held_active);
    }

    #[test]
    fn first_tap_stops_without_any_terminal_key_up_events() {
        let mut game = GameLoop::new();
        let now = Instant::now();
        game.handle_horizontal_press(HorizontalDirection::Right, now, false);
        game.clear_stale_horizontal_input(now + Duration::from_millis(151));
        assert!(!game.state.moving_right);
        assert!(!game.key_releases);
    }

    #[test]
    fn legacy_repeat_after_initial_timeout_does_not_become_a_sprint_tap() {
        for direction in [HorizontalDirection::Left, HorizontalDirection::Right] {
            let mut game = GameLoop::with_state(GameState::network_view());
            let now = Instant::now();
            game.handle_horizontal_press(direction, now, false);
            game.clear_stale_horizontal_input(now + Duration::from_millis(151));
            game.handle_horizontal_press(direction, now + Duration::from_millis(300), false);
            assert!(!game.state.is_sprinting(), "OS repeat became a double tap");
            // An actual release and second press still intentionally sprint.
            game.handle_horizontal_release(direction);
            game.handle_horizontal_press(direction, now + Duration::from_millis(350), false);
            assert!(game.state.is_sprinting());
        }
    }

    #[test]
    fn unrelated_releases_do_not_disable_conservative_movement_timeout() {
        let mut game = GameLoop::new();
        let now = Instant::now();
        for code in [KeyCode::Char('e'), KeyCode::F(3), KeyCode::Null] {
            let release = crossterm::event::KeyEvent::new_with_kind(
                code,
                KeyModifiers::NONE,
                KeyEventKind::Release,
            );
            game.key_releases |= confirms_control_key_releases(&release);
        }
        game.handle_horizontal_press(HorizontalDirection::Left, now, false);
        game.clear_stale_horizontal_input(now + Duration::from_millis(151));
        assert!(!game.state.moving_left);
        assert!(!game.key_releases);
    }

    #[test]
    fn confirmed_chord_keeps_running_after_jump_release_without_direction_repeats() {
        for direction in [HorizontalDirection::Left, HorizontalDirection::Right] {
            let mut game = GameLoop::with_state(GameState::network_view());
            game.key_releases = true;
            let now = Instant::now();
            game.handle_horizontal_press(direction, now, false);
            game.handle_jump_press(now + Duration::from_millis(100), false);
            game.clear_jump_hold_state();
            game.clear_stale_horizontal_input(now + Duration::from_secs(10));
            assert!(game.state.moving_left || game.state.moving_right);
            game.handle_horizontal_release(direction);
            assert!(!game.state.moving_left && !game.state.moving_right);
            // The same reset used by focus loss still clears the entire chord.
            game.handle_horizontal_press(direction, now, false);
            game.handle_jump_press(now, false);
            game.clear_horizontal_input_state();
            game.clear_jump_hold_state();
            assert!(!game.state.moving_left && !game.state.moving_right);
            assert!(!game.jump_held_active);
        }
    }

    #[test]
    fn legacy_jump_grace_preserves_takeoff_but_cannot_be_renewed_by_space() {
        let mut game = GameLoop::with_state(GameState::network_view());
        let now = Instant::now();
        game.handle_horizontal_press(HorizontalDirection::Right, now, false);
        game.handle_horizontal_press(
            HorizontalDirection::Right,
            now + Duration::from_millis(30),
            false,
        );
        let jump = now + Duration::from_millis(50);
        game.handle_jump_press(jump, false);
        game.clear_stale_horizontal_input(jump + Duration::from_millis(500));
        assert!(game.state.moving_right);
        // Legacy repeat is also delivered as Press. Let the jump pulse expire,
        // then send more Space presses: they cannot extend the original grace.
        game.clear_stale_jump_input(jump + Duration::from_millis(400));
        game.handle_jump_press(jump + Duration::from_millis(500), false);
        game.clear_stale_horizontal_input(
            jump + LEGACY_JUMP_MOVEMENT_GRACE + Duration::from_millis(1),
        );
        assert!(!game.state.moving_right);
        game.clear_jump_hold_state();
        game.handle_jump_press(jump + Duration::from_secs(2), false);
        assert!(!game.state.moving_right);
        // A single directional tap has no established repeat and gets no grace.
        game.clear_jump_hold_state();
        game.handle_horizontal_press(
            HorizontalDirection::Left,
            jump + Duration::from_secs(3),
            false,
        );
        game.handle_jump_press(jump + Duration::from_secs(3), false);
        game.clear_stale_horizontal_input(
            jump + Duration::from_secs(3) + Duration::from_millis(151),
        );
        assert!(!game.state.moving_left);
        // Event batches can arrive before stale cleanup after a frame stall.
        // Space must not grant grace to an already stale repeating direction.
        let mut delayed = GameLoop::with_state(GameState::network_view());
        delayed.handle_horizontal_press(HorizontalDirection::Right, now, false);
        delayed.handle_horizontal_press(
            HorizontalDirection::Right,
            now + Duration::from_millis(30),
            false,
        );
        delayed.handle_jump_press(now + Duration::from_millis(500), false);
        delayed.clear_stale_horizontal_input(now + Duration::from_millis(500));
        assert!(!delayed.state.moving_right);
        // A later real directional repeat re-arms grace for the next jump,
        // even if the run has stayed active throughout.
        let mut repeated = GameLoop::with_state(GameState::network_view());
        repeated.handle_horizontal_press(HorizontalDirection::Right, now, false);
        repeated.handle_horizontal_press(
            HorizontalDirection::Right,
            now + Duration::from_millis(30),
            false,
        );
        repeated.handle_jump_press(now + Duration::from_millis(50), false);
        repeated.clear_jump_hold_state();
        repeated.handle_horizontal_press(
            HorizontalDirection::Right,
            now + Duration::from_millis(780),
            true,
        );
        repeated.handle_jump_press(now + Duration::from_millis(820), false);
        repeated.clear_stale_horizontal_input(now + Duration::from_millis(1100));
        assert!(repeated.state.moving_right);
    }

    #[test]
    fn autosave_reports_failure_and_retries_without_clearing_unsaved_state() {
        let root = std::env::temp_dir().join(format!(
            "termcraft-autosave-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let store = crate::worlds::WorldStore::new(root.clone());
        let path = store
            .create(
                "test",
                "autosave fixture",
                &crate::world::recipe::WorldRecipe::example(),
            )
            .unwrap();
        let mut game = GameLoop::with_state(GameState::open_in_directory(&path).unwrap());
        game.state.world.try_save_all().unwrap();
        let blocked = path.join("player_progression.bin.tmp");
        std::fs::create_dir(&blocked).unwrap();
        let due = game.last_autosave_at + AUTOSAVE_INTERVAL;
        game.maybe_autosave(due);
        assert!(
            game.state
                .save_error
                .borrow()
                .as_ref()
                .unwrap()
                .contains("Autosave failed")
        );
        assert_eq!(game.last_autosave_at, due);
        assert!(!path.join("player_progression.bin").exists());
        game.state.player.health = 13.0;
        std::fs::remove_dir(blocked).unwrap();
        game.maybe_autosave(due + AUTOSAVE_INTERVAL);
        assert!(game.state.save_error.borrow().is_none());
        let reopened = GameState::open_in_directory(&path).unwrap();
        assert_eq!(reopened.player.health, 13.0);
        drop(reopened);
        drop(game);
        std::fs::remove_dir_all(root).unwrap();
    }
    use crate::engine::state::CRAFT_GRID_UI_OFFSET;
    use crate::world::item::{Inventory, ItemStack, ItemType};

    const TEST_PLAYER_INVENTORY_CAPACITY: usize = 27;

    #[test]
    fn stale_horizontal_input_is_cleared() {
        let mut loop_state = GameLoop::new();
        loop_state.state.moving_right = true;
        loop_state.right_last_input_at = Some(
            Instant::now() - HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL - Duration::from_millis(1),
        );

        loop_state.clear_stale_horizontal_input(Instant::now());

        assert!(!loop_state.state.moving_left);
        assert!(!loop_state.state.moving_right);
    }

    #[test]
    fn stale_jump_input_is_cleared() {
        let mut loop_state = GameLoop::new();
        let t0 = Instant::now();
        loop_state.handle_jump_press(t0, false);

        loop_state.clear_stale_jump_input(
            t0 + JUMP_INPUT_STALE_TIMEOUT_INITIAL + Duration::from_millis(1),
        );

        assert!(!loop_state.jump_held_active);
        assert_eq!(loop_state.jump_last_input_at, None);
        assert!(!loop_state.jump_repeat_observed);
    }

    #[test]
    fn repeat_jump_input_remains_active_within_repeat_timeout() {
        let mut loop_state = GameLoop::new();
        let t0 = Instant::now();
        loop_state.handle_jump_press(t0, false);
        loop_state.handle_jump_hold_repeat(t0 + Duration::from_millis(100));

        loop_state.clear_stale_jump_input(
            t0 + Duration::from_millis(100) + JUMP_INPUT_STALE_TIMEOUT_REPEAT
                - Duration::from_millis(1),
        );

        assert!(loop_state.jump_held_active);
        assert!(loop_state.jump_repeat_observed);
    }

    #[test]
    fn fresh_horizontal_input_remains_active() {
        let mut loop_state = GameLoop::new();
        loop_state.state.moving_left = true;
        loop_state.left_last_input_at = Some(Instant::now());

        loop_state.clear_stale_horizontal_input(
            Instant::now() + HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL - Duration::from_millis(1),
        );

        assert!(loop_state.state.moving_left);
        assert!(!loop_state.state.moving_right);
    }

    #[test]
    fn repeat_input_uses_shorter_but_safe_timeout() {
        let mut loop_state = GameLoop::new();
        loop_state.state.moving_right = true;
        loop_state.right_repeat_observed = true;
        loop_state.right_last_input_at =
            Some(Instant::now() - HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT - Duration::from_millis(1));

        loop_state.clear_stale_horizontal_input(Instant::now());

        assert!(!loop_state.state.moving_left);
        assert!(!loop_state.state.moving_right);
    }

    #[test]
    fn repeat_input_remains_active_within_repeat_timeout() {
        let mut loop_state = GameLoop::new();
        let now = Instant::now();
        loop_state.state.moving_right = true;
        loop_state.right_repeat_observed = true;
        loop_state.right_last_input_at =
            Some(now - HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT + Duration::from_millis(1));

        loop_state.clear_stale_horizontal_input(now);

        assert!(loop_state.state.moving_right);
        assert!(!loop_state.state.moving_left);
    }

    #[test]
    fn stale_clear_applies_horizontal_brake() {
        let mut loop_state = GameLoop::new();
        loop_state.state.moving_right = true;
        loop_state.state.player.grounded = true;
        loop_state.state.player.vx = 0.72;
        loop_state.right_last_input_at = Some(
            Instant::now() - HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL - Duration::from_millis(1),
        );

        loop_state.clear_stale_horizontal_input(Instant::now());

        assert!(!loop_state.state.moving_left);
        assert!(!loop_state.state.moving_right);
        assert!(loop_state.state.player.vx > 0.0);
        assert!(loop_state.state.player.vx < 0.2);
    }

    #[test]
    fn horizontal_release_applies_horizontal_brake() {
        let mut loop_state = GameLoop::new();
        loop_state.state.moving_left = true;
        loop_state.state.player.grounded = true;
        loop_state.state.player.vx = -0.66;

        loop_state.handle_horizontal_release(HorizontalDirection::Left);

        assert!(!loop_state.state.moving_left);
        assert!(!loop_state.state.moving_right);
        assert!(loop_state.state.player.vx < 0.0);
        assert!(loop_state.state.player.vx > -0.2);
    }

    #[test]
    fn rapid_presses_without_repeat_kind_are_treated_as_repeat() {
        let mut loop_state = GameLoop::new();
        let t0 = Instant::now();
        loop_state.handle_horizontal_press(HorizontalDirection::Left, t0, false);
        assert!(!loop_state.left_repeat_observed);

        loop_state.handle_horizontal_press(
            HorizontalDirection::Left,
            t0 + Duration::from_millis(100),
            false,
        );
        assert!(loop_state.left_repeat_observed);
    }

    #[test]
    fn stale_clear_only_affects_stale_direction() {
        let mut loop_state = GameLoop::new();
        let now = Instant::now();
        loop_state.state.moving_left = true;
        loop_state.state.moving_right = true;
        loop_state.left_last_input_at =
            Some(now - HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL - Duration::from_millis(1));
        loop_state.right_last_input_at = Some(now);

        loop_state.clear_stale_horizontal_input(now);

        assert!(!loop_state.state.moving_left);
        assert!(loop_state.state.moving_right);
    }

    #[test]
    fn uppercase_wasd_keys_map_to_horizontal_directions() {
        assert_eq!(
            GameLoop::horizontal_direction_from_key(&KeyCode::Char('A')),
            Some(HorizontalDirection::Left)
        );
        assert_eq!(
            GameLoop::horizontal_direction_from_key(&KeyCode::Char('D')),
            Some(HorizontalDirection::Right)
        );
    }

    #[test]
    fn null_release_clears_horizontal_state() {
        let mut loop_state = GameLoop::new();
        loop_state.state.moving_right = true;
        loop_state.right_last_input_at = Some(Instant::now());
        loop_state.right_repeat_observed = true;

        let ev = crossterm::event::KeyEvent::new_with_kind(
            KeyCode::Null,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );

        if let Some(direction) = GameLoop::horizontal_direction_from_key(&ev.code) {
            loop_state.handle_horizontal_release(direction);
        } else if ev.code == KeyCode::Null {
            loop_state.clear_horizontal_input_state();
        }

        assert!(!loop_state.state.moving_left);
        assert!(!loop_state.state.moving_right);
        assert!(!loop_state.left_repeat_observed);
        assert!(!loop_state.right_repeat_observed);
        assert_eq!(loop_state.left_last_input_at, None);
        assert_eq!(loop_state.right_last_input_at, None);
    }

    #[test]
    fn sneak_toggle_and_hold_stack() {
        let mut loop_state = GameLoop::new();
        loop_state
            .state
            .apply_client_command(ClientCommand::ToggleSneak);
        loop_state
            .state
            .apply_client_command(ClientCommand::SetSneakHeld(false));
        assert!(loop_state.state.player.sneaking);

        loop_state
            .state
            .apply_client_command(ClientCommand::ToggleSneak);
        loop_state
            .state
            .apply_client_command(ClientCommand::SetSneakHeld(true));
        assert!(loop_state.state.player.sneaking);

        loop_state
            .state
            .apply_client_command(ClientCommand::SetSneakHeld(false));
        assert!(!loop_state.state.player.sneaking);
    }

    #[test]
    fn sneak_hold_modifier_sync_follows_shift_bit() {
        let mut loop_state = GameLoop::new();
        loop_state.sync_sneak_hold_from_modifiers(KeyModifiers::SHIFT);
        assert!(loop_state.state.player.sneaking);

        loop_state.sync_sneak_hold_from_modifiers(KeyModifiers::NONE);
        assert!(!loop_state.state.player.sneaking);
    }

    #[test]
    fn explicit_shift_key_press_sets_sneak_without_shift_modifier_flag() {
        let mut loop_state = GameLoop::new();
        let ev = crossterm::event::KeyEvent::new_with_kind(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::NONE,
            KeyEventKind::Press,
        );

        if GameLoop::is_shift_modifier_key(&ev.code) {
            loop_state
                .state
                .apply_client_command(ClientCommand::SetSneakHeld(true));
        } else {
            loop_state.sync_sneak_hold_from_modifiers(ev.modifiers);
        }

        assert!(loop_state.state.player.sneaking);
    }

    #[test]
    fn inventory_enter_shortcut_crafts_once() {
        let mut loop_state = GameLoop::new();
        loop_state.state.inventory = Inventory::new(TEST_PLAYER_INVENTORY_CAPACITY);
        loop_state.state.inventory_open = true;
        loop_state.state.at_crafting_table = false;
        loop_state.state.at_furnace = false;
        loop_state.state.at_chest = false;
        loop_state.state.at_enchanting_table = false;
        loop_state.state.at_anvil = false;
        loop_state.state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 2,
            durability: None,
        });
        loop_state.state.handle_inventory_click(0);
        loop_state
            .state
            .handle_inventory_click(CRAFT_GRID_UI_OFFSET);
        loop_state
            .state
            .handle_inventory_click(CRAFT_GRID_UI_OFFSET + 3);

        let key_event = crossterm::event::KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        );

        assert!(loop_state.handle_inventory_shortcut_key(&key_event));
        assert!(loop_state.state.inventory.has_item(ItemType::Stick, 4));
    }

    #[test]
    fn inventory_shift_enter_shortcut_crafts_max() {
        let mut loop_state = GameLoop::new();
        loop_state.state.inventory = Inventory::new(TEST_PLAYER_INVENTORY_CAPACITY);
        loop_state.state.inventory_open = true;
        loop_state.state.at_crafting_table = false;
        loop_state.state.at_furnace = false;
        loop_state.state.at_chest = false;
        loop_state.state.at_enchanting_table = false;
        loop_state.state.at_anvil = false;
        loop_state.state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 16,
            durability: None,
        });
        loop_state.state.handle_inventory_click(0);
        loop_state
            .state
            .handle_inventory_click(CRAFT_GRID_UI_OFFSET);
        loop_state
            .state
            .handle_inventory_click(CRAFT_GRID_UI_OFFSET + 3);

        let key_event = crossterm::event::KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::SHIFT,
            KeyEventKind::Press,
        );

        assert!(loop_state.handle_inventory_shortcut_key(&key_event));
        assert!(loop_state.state.inventory.has_item(ItemType::Stick, 16));
    }

    #[test]
    fn inventory_delete_shortcut_returns_crafting_items() {
        let mut loop_state = GameLoop::new();
        loop_state.state.inventory = Inventory::new(TEST_PLAYER_INVENTORY_CAPACITY);
        loop_state.state.inventory_open = true;
        loop_state.state.at_crafting_table = false;
        loop_state.state.at_furnace = false;
        loop_state.state.at_chest = false;
        loop_state.state.at_enchanting_table = false;
        loop_state.state.at_anvil = false;
        loop_state.state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 8,
            durability: None,
        });
        loop_state.state.handle_inventory_click(0);
        loop_state
            .state
            .handle_inventory_click(CRAFT_GRID_UI_OFFSET);
        loop_state
            .state
            .handle_inventory_click(CRAFT_GRID_UI_OFFSET + 3);

        let key_event = crossterm::event::KeyEvent::new_with_kind(
            KeyCode::Delete,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        );

        assert!(loop_state.handle_inventory_shortcut_key(&key_event));
        assert!(loop_state.state.inventory.has_item(ItemType::Planks, 8));
        assert!(loop_state.state.crafting_output_preview().is_none());
    }
}
