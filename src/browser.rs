//! Browser entry points use the same simulation and cell renderer as native play.
use crate::{
    engine::{command::ClientCommand, state::GameState},
    platform::fs,
    renderer::Renderer,
    worlds::WorldStore,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn initialise() {
    console_error_panic_hook::set_once();
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[wasm_bindgen]
pub struct BrowserGame {
    state: GameState,
    renderer: Renderer,
    columns: u16,
    rows: u16,
    accumulated_ms: f64,
    previous_drag_slot: Option<usize>,
    direction_pressed: [bool; 2],
    direction_released: [bool; 2],
    primary_pressed: bool,
    primary_released: bool,
}

#[wasm_bindgen]
impl BrowserGame {
    #[wasm_bindgen(constructor)]
    pub fn new(columns: u16, rows: u16, saved: Option<String>) -> Result<BrowserGame, JsValue> {
        fs::reset();
        if let Some(saved) = saved {
            fs::restore(&saved).map_err(js_error)?;
        }
        let store = WorldStore::new("saves/worlds".into());
        let root = match store.open("lantern-run") {
            Ok(root) => root,
            Err(_) => {
                GameState::create_lantern_trial(&store, "lantern-run").map_err(js_error)?;
                store.open("lantern-run").map_err(js_error)?
            }
        };
        let mut state = GameState::open_in_directory(&root).map_err(js_error)?;
        state.dismiss_startup_splash();
        let columns = columns.clamp(100, 360);
        let rows = rows.clamp(36, 120);
        Ok(Self {
            state,
            renderer: Renderer::browser_view(columns, rows),
            columns,
            rows,
            accumulated_ms: 0.0,
            previous_drag_slot: None,
            direction_pressed: [false; 2],
            direction_released: [false; 2],
            primary_pressed: false,
            primary_released: false,
        })
    }

    /// Fixed 20 Hz physics with bounded catch-up, independent of browser paint rate.
    pub fn frame(&mut self, elapsed_ms: f64) -> Result<Vec<u32>, JsValue> {
        let elapsed_ms = if elapsed_ms.is_finite() {
            elapsed_ms.clamp(0.0, 250.0)
        } else {
            0.0
        };
        self.accumulated_ms += elapsed_ms;
        let mut steps = 0;
        while self.accumulated_ms >= 50.0 && steps < 5 {
            self.renderer.capture_motion_step(&self.state);
            let (x, y) =
                self.renderer
                    .screen_to_world(&self.state, self.state.mouse_x, self.state.mouse_y);
            self.state.update(x, y);
            self.primary_pressed = false;
            if self.primary_released {
                self.state
                    .apply_client_command(ClientCommand::SetPrimaryAction(false));
                self.primary_released = false;
            }
            for direction in 0..2 {
                self.direction_pressed[direction] = false;
                if self.direction_released[direction] {
                    self.state.apply_client_command(if direction == 0 {
                        ClientCommand::SetMoveLeft(false)
                    } else {
                        ClientCommand::SetMoveRight(false)
                    });
                    self.direction_released[direction] = false;
                }
            }
            self.accumulated_ms -= 50.0;
            steps += 1;
        }
        self.renderer
            .browser_frame(&self.state, self.accumulated_ms / 50.0, elapsed_ms / 1000.0)
            .map_err(js_error)
    }

    pub fn release_controls(&mut self) {
        self.state
            .apply_client_command(ClientCommand::ClearDirectionalInput);
        self.state
            .apply_client_command(ClientCommand::SetJumpHeld(false));
        self.state
            .apply_client_command(ClientCommand::SetSneakHeld(false));
        self.state
            .apply_client_command(ClientCommand::SetPrimaryAction(false));
        self.previous_drag_slot = None;
        self.direction_pressed.fill(false);
        self.direction_released.fill(false);
        self.primary_pressed = false;
        self.primary_released = false;
        self.accumulated_ms = 0.0;
    }

    /// Returns true when the page should pause; release events always reach play.
    pub fn key(&mut self, code: &str, down: bool, shift: bool) -> bool {
        let command = match code {
            "KeyA" | "ArrowLeft" => {
                self.direction_key(0, down);
                None
            }
            "KeyD" | "ArrowRight" => {
                self.direction_key(1, down);
                None
            }
            "KeyW" | "ArrowUp" | "Space" if !self.state.is_settings_menu_open() => {
                if down {
                    self.state.apply_client_command(ClientCommand::QueueJump);
                }
                Some(ClientCommand::SetJumpHeld(down))
            }
            "ShiftLeft" | "ShiftRight" => Some(ClientCommand::SetSneakHeld(down)),
            _ if !down => None,
            "KeyE" => Some(ClientCommand::ToggleInventory),
            "Enter" if self.state.inventory_open => Some(ClientCommand::CraftGrid(shift)),
            "Delete" if self.state.inventory_open => Some(ClientCommand::ClearCraftGrid),
            "Tab" if self.state.inventory_open => {
                self.renderer.toggle_recipe_book();
                None
            }
            "PageDown" if self.state.inventory_open => {
                self.renderer.scroll_recipe_book(8);
                None
            }
            "PageUp" if self.state.inventory_open => {
                self.renderer.scroll_recipe_book(-8);
                None
            }
            "KeyV" => Some(ClientCommand::DropItem(shift)),
            "KeyO" => Some(ClientCommand::ToggleSettingsMenu),
            "ArrowUp" if self.state.is_settings_menu_open() => Some(ClientCommand::SettingsMoveUp),
            "ArrowDown" if self.state.is_settings_menu_open() => {
                Some(ClientCommand::SettingsMoveDown)
            }
            "Enter" if self.state.is_settings_menu_open() => Some(ClientCommand::SettingsApply),
            "Enter" | "KeyR" if self.state.is_showing_death_screen() => {
                Some(ClientCommand::RespawnFromDeathScreen)
            }
            "Enter" if self.state.is_showing_credits() => {
                Some(ClientCommand::SkipCompletionCredits)
            }
            "Escape" | "KeyQ" if self.state.inventory_open => Some(ClientCommand::ToggleInventory),
            "Escape" | "KeyQ" if self.state.is_settings_menu_open() => {
                Some(ClientCommand::ToggleSettingsMenu)
            }
            "Escape" | "KeyQ" => {
                self.release_controls();
                return true;
            }
            "KeyF" => {
                let (x, y) = self.renderer.screen_to_world(
                    &self.state,
                    self.state.mouse_x,
                    self.state.mouse_y,
                );
                Some(ClientCommand::UseAt(x, y))
            }
            "F4" => {
                self.renderer.toggle_zoom();
                None
            }
            "Equal" | "NumpadAdd" => {
                self.renderer.zoom_in();
                None
            }
            "Minus" | "NumpadSubtract" => {
                self.renderer.zoom_out();
                None
            }
            _ => code
                .strip_prefix("Digit")
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|n| (1..=9).contains(n))
                .map(|n| ClientCommand::SelectHotbarSlot(n - 1)),
        };
        if let Some(command) = command {
            self.state.apply_client_command(command);
        }
        false
    }

    pub fn mouse(&mut self, x: u16, y: u16, button: u8, phase: u8, shift: bool) {
        self.state.mouse_x = x.min(self.columns - 1);
        self.state.mouse_y = y.min(self.rows - 1);
        if phase == 0 {
            self.previous_drag_slot = None;
            if button == 0 {
                if self.primary_pressed && !self.state.inventory_open {
                    self.primary_released = true;
                } else {
                    self.state
                        .apply_client_command(ClientCommand::SetPrimaryAction(false));
                }
            }
            return;
        }
        if phase == 2 && !self.state.inventory_open {
            return;
        }
        if self.state.inventory_open {
            if let Some(slot) = self.renderer.get_inventory_click(
                &self.state,
                self.state.mouse_x,
                self.state.mouse_y,
            ) {
                if phase == 2 && self.previous_drag_slot == Some(slot) {
                    return;
                }
                self.previous_drag_slot = Some(slot);
                let kind = if shift {
                    2
                } else if button == 2 {
                    1
                } else if phase == 2 {
                    3
                } else {
                    0
                };
                self.state
                    .apply_client_command(ClientCommand::InventoryClick(slot as u16, kind));
            } else if phase == 1
                && let Some(recipe) = self.renderer.get_recipe_click(
                    &self.state,
                    self.state.mouse_x,
                    self.state.mouse_y,
                )
            {
                self.state.apply_client_command(if shift {
                    ClientCommand::CraftRecipeMax(recipe as u16)
                } else {
                    ClientCommand::CraftRecipe(recipe as u16)
                });
            }
        } else if phase == 1 {
            if button == 0 {
                self.primary_pressed = true;
                self.primary_released = false;
                self.state
                    .apply_client_command(ClientCommand::SetPrimaryAction(true));
            } else if button == 2 {
                let (x, y) = self.renderer.screen_to_world(
                    &self.state,
                    self.state.mouse_x,
                    self.state.mouse_y,
                );
                self.state.apply_client_command(ClientCommand::UseAt(x, y));
            }
        }
    }

    pub fn aim(&mut self, x: u16, y: u16) {
        self.state.mouse_x = x.min(self.columns - 1);
        self.state.mouse_y = y.min(self.rows - 1);
    }
    pub fn wheel(&mut self, down: bool) {
        if self.state.inventory_open {
            self.renderer.scroll_recipe_book(if down { 1 } else { -1 });
            return;
        }
        let slot = (self.state.hotbar_index as i16 + if down { 1 } else { 8 }) % 9;
        self.state
            .apply_client_command(ClientCommand::SelectHotbarSlot(slot as u8));
    }
    pub fn snapshot(&mut self) -> Result<String, JsValue> {
        self.state
            .try_persist_world_and_progress()
            .map_err(js_error)?;
        fs::snapshot().map_err(js_error)
    }
    pub fn status(&self) -> String {
        serde_json::json!({"x":self.state.player.x,"y":self.state.player.y,"health":self.state.player.health,"inventory_open":self.state.inventory_open,"objective":self.state.trial_controls_hint(),"dimension":format!("{:?}",self.state.current_dimension),"columns":self.columns,"rows":self.rows}).to_string()
    }
}

impl BrowserGame {
    // Preserve a real tap shorter than one 50 ms simulation step. Focus loss
    // clears this buffer immediately, so it cannot revive movement on resume.
    fn direction_key(&mut self, direction: usize, down: bool) {
        let command = |active| {
            if direction == 0 {
                ClientCommand::SetMoveLeft(active)
            } else {
                ClientCommand::SetMoveRight(active)
            }
        };
        if down {
            if self.direction_released[direction] {
                self.state.apply_client_command(command(false));
            }
            self.direction_pressed[direction] = true;
            self.direction_released[direction] = false;
            self.state.apply_client_command(command(true));
        } else if self.direction_pressed[direction] {
            self.direction_released[direction] = true;
        } else {
            self.state.apply_client_command(command(false));
        }
    }
}
