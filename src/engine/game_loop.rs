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
// These are only fallback timeouts for terminals that fail to emit key-release
// events. Keep them comfortably above common OS repeat delays so held movement
// does not stutter between the initial press and the first repeat packet.
const HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(150);
const HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(96);
const HORIZONTAL_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(320);
const JUMP_INPUT_STALE_TIMEOUT_INITIAL: Duration = Duration::from_millis(340);
const JUMP_INPUT_STALE_TIMEOUT_REPEAT: Duration = Duration::from_millis(160);
const JUMP_PRESS_REPEAT_WINDOW: Duration = Duration::from_millis(420);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(45);
const AUTOSAVE_CHUNK_BUDGET_PER_FRAME: usize = 1;
const TARGET_FPS: u64 = 30;
const FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TARGET_FPS);
const GROUNDED_HORIZONTAL_RELEASE_BRAKE: f64 = 0.18;
const AIR_HORIZONTAL_RELEASE_BRAKE: f64 = 0.28;
const GROUNDED_STALE_CLEAR_BRAKE: f64 = 0.08;
const AIR_STALE_CLEAR_BRAKE: f64 = 0.3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HorizontalDirection {
    Left,
    Right,
}

pub struct GameLoop {
    state: GameState,
    left_last_input_at: Option<Instant>,
    right_last_input_at: Option<Instant>,
    left_repeat_observed: bool,
    right_repeat_observed: bool,
    jump_last_input_at: Option<Instant>,
    jump_repeat_observed: bool,
    jump_held_active: bool,
    last_inventory_left_drag_slot: Option<usize>,
    last_inventory_right_drag_slot: Option<usize>,
    last_autosave_at: Instant,
    autosave_in_progress: bool,
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
        Self {
            state: GameState::new(),
            left_last_input_at: None,
            right_last_input_at: None,
            left_repeat_observed: false,
            right_repeat_observed: false,
            jump_last_input_at: None,
            jump_repeat_observed: false,
            jump_held_active: false,
            last_inventory_left_drag_slot: None,
            last_inventory_right_drag_slot: None,
            last_autosave_at: Instant::now(),
            autosave_in_progress: false,
        }
    }

    pub fn run(&mut self, renderer: &mut Renderer) -> std::io::Result<()> {
        let mut last_tick = Instant::now();
        let mut next_frame_deadline = Instant::now();
        let mut running = true;

        while running {
            // Handle Input (Non-blocking)
            while event::poll(Duration::from_millis(0))? {
                let ev = event::read()?;
                match ev {
                    Event::Key(key_event) => {
                        if self.state.is_showing_startup_splash() {
                            if key_event.kind == KeyEventKind::Press {
                                match key_event.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        self.state.persist_world_and_progress();
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
                                        self.state.persist_world_and_progress();
                                        running = false;
                                    }
                                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('r') => {
                                        if self.state.can_respawn_from_death_screen() {
                                            self.state.apply_client_command(
                                                ClientCommand::RespawnFromDeathScreen,
                                            );
                                        }
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
                                        self.state.persist_world_and_progress();
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
                                        KeyCode::Enter | KeyCode::Char(' ') => {
                                            if key_event.kind == KeyEventKind::Press {
                                                self.state.apply_client_command(
                                                    ClientCommand::SettingsApply,
                                                );
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                KeyEventKind::Release => {}
                            }
                            continue;
                        }
                        if self.handle_inventory_shortcut_key(&key_event) {
                            continue;
                        }
                        match key_event.kind {
                            KeyEventKind::Press => {
                                match key_event.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        if self.state.inventory_open {
                                            self.state.apply_client_command(
                                                ClientCommand::ToggleInventory,
                                            );
                                        } else {
                                            self.state.persist_world_and_progress();
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
                    Event::Mouse(mouse_event) => {
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
                                if self.state.inventory_open {
                                    if let Some(slot_idx) = renderer.get_inventory_click(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) && self.last_inventory_right_drag_slot != Some(slot_idx)
                                    {
                                        self.state.handle_inventory_right_click(slot_idx);
                                        self.last_inventory_right_drag_slot = Some(slot_idx);
                                    }
                                } else {
                                    let (bx, by) = renderer.screen_to_world(
                                        &self.state,
                                        mouse_event.column,
                                        mouse_event.row,
                                    );
                                    self.state.interact_block(bx, by, false);
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
            self.maybe_autosave(now);

            // Fixed-step update with bounded catch-up keeps motion cadence stable.
            let mut catchup_ticks = 0u8;
            while now.duration_since(last_tick) >= TICK_DURATION
                && catchup_ticks < MAX_CATCHUP_TICKS_PER_FRAME
            {
                let (bx, by) =
                    renderer.screen_to_world(&self.state, self.state.mouse_x, self.state.mouse_y);
                self.update_logic(bx, by);
                last_tick += TICK_DURATION;
                catchup_ticks = catchup_ticks.saturating_add(1);
            }
            if catchup_ticks == MAX_CATCHUP_TICKS_PER_FRAME
                && now.duration_since(last_tick) >= TICK_DURATION
            {
                // Drop excess lag rather than spiraling under heavy terminal load.
                last_tick = now;
            }

            // Render once per frame budget to stabilize terminal pacing.
            let frame_alpha = (now.duration_since(last_tick).as_secs_f64()
                / TICK_DURATION.as_secs_f64())
            .clamp(0.0, 1.0);
            renderer.render(&self.state, frame_alpha)?;

            next_frame_deadline += FRAME_DURATION;
            let after_render = Instant::now();
            if after_render < next_frame_deadline {
                std::thread::sleep(next_frame_deadline - after_render);
            } else if after_render.duration_since(next_frame_deadline) > FRAME_DURATION {
                // Resync when the terminal stalls for too long.
                next_frame_deadline = after_render;
            }
        }

        Ok(())
    }

    fn update_logic(&mut self, target_bx: i32, target_by: i32) {
        self.state.update(target_bx, target_by);
    }

    fn maybe_autosave(&mut self, now: Instant) {
        if self.autosave_in_progress {
            if !self
                .state
                .autosave_world_step(AUTOSAVE_CHUNK_BUDGET_PER_FRAME)
            {
                self.state.save_progression();
                self.autosave_in_progress = false;
                self.last_autosave_at = now;
            }
            return;
        }

        if now.duration_since(self.last_autosave_at) >= AUTOSAVE_INTERVAL {
            if self
                .state
                .autosave_world_step(AUTOSAVE_CHUNK_BUDGET_PER_FRAME)
            {
                self.autosave_in_progress = true;
            } else {
                self.state.save_progression();
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

        if !self.state.moving_left && !self.state.moving_right {
            self.apply_horizontal_brake(
                GROUNDED_HORIZONTAL_RELEASE_BRAKE,
                AIR_HORIZONTAL_RELEASE_BRAKE,
            );
        }
    }

    fn handle_jump_press(&mut self, now: Instant, explicit_repeat: bool) {
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

    fn stale_timeout_for_direction(&self, direction: HorizontalDirection) -> Duration {
        if self.direction_repeat_observed(direction) {
            HORIZONTAL_INPUT_STALE_TIMEOUT_REPEAT
        } else {
            HORIZONTAL_INPUT_STALE_TIMEOUT_INITIAL
        }
    }

    fn clear_stale_direction(&mut self, direction: HorizontalDirection) -> bool {
        match direction {
            HorizontalDirection::Left => {
                if !self.state.moving_left {
                    return false;
                }
                self.state
                    .apply_client_command(ClientCommand::SetMoveLeft(false));
                self.left_last_input_at = None;
                self.left_repeat_observed = false;
                true
            }
            HorizontalDirection::Right => {
                if !self.state.moving_right {
                    return false;
                }
                self.state
                    .apply_client_command(ClientCommand::SetMoveRight(false));
                self.right_last_input_at = None;
                self.right_repeat_observed = false;
                true
            }
        }
    }

    fn clear_stale_horizontal_input(&mut self, now: Instant) {
        let left_stale = self.state.moving_left
            && self.left_last_input_at.is_none_or(|ts| {
                now.duration_since(ts) > self.stale_timeout_for_direction(HorizontalDirection::Left)
            });
        let right_stale = self.state.moving_right
            && self.right_last_input_at.is_none_or(|ts| {
                now.duration_since(ts)
                    > self.stale_timeout_for_direction(HorizontalDirection::Right)
            });

        let mut cleared_any = false;
        if left_stale {
            cleared_any = self.clear_stale_direction(HorizontalDirection::Left) || cleared_any;
        }
        if right_stale {
            cleared_any = self.clear_stale_direction(HorizontalDirection::Right) || cleared_any;
        }

        if cleared_any && !self.state.moving_left && !self.state.moving_right {
            self.apply_horizontal_brake(GROUNDED_STALE_CLEAR_BRAKE, AIR_STALE_CLEAR_BRAKE);
        }
    }

    fn clear_stale_jump_input(&mut self, now: Instant) {
        if !self.jump_held_active {
            return;
        }
        if self
            .jump_last_input_at
            .is_none_or(|ts| now.duration_since(ts) > self.jump_stale_timeout())
        {
            self.clear_jump_hold_state();
        }
    }

    fn apply_horizontal_brake(&mut self, grounded_factor: f64, air_factor: f64) {
        let factor = if self.state.player.grounded {
            grounded_factor
        } else {
            air_factor
        };
        self.state.player.vx *= factor;
        if self.state.player.vx.abs() < 0.02 {
            self.state.player.vx = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
